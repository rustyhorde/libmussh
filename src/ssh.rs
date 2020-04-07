// Copyright © 2018 libmussh developers
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

//! Multiplex commands over hosts.
use crate::config::Host;
use crate::error::{MusshErrKind, MusshResult};
use crate::utils::{convert_duration, CmdType, MultiplexMapType};
use chrono::Utc;
use getset::{Getters, Setters};
use indexmap::{IndexMap, IndexSet};
use slog::{error, info, trace, Logger};
use slog_try::{try_error, try_info, try_trace};
use ssh2::Session;
use std::collections::HashMap;
use std::env;
use std::io::{BufRead, BufReader};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};
use wait_group::WaitGroup;

type MultiplexResult = Vec<MusshResult<Metrics>>;

/// Execution metrics
#[derive(Clone, Debug, Eq, Getters, PartialEq)]
pub struct Metrics {
    /// The hostname where the command was run
    #[get = "pub"]
    hostname: String,
    /// The name of the command that was run
    #[get = "pub"]
    cmd_name: String,
    /// The duration of the execution
    #[get = "pub"]
    duration: Duration,
    /// The timestamp when this metric was created
    #[get = "pub"]
    timestamp: i64,
}

impl Default for Metrics {
    fn default() -> Self {
        Self {
            hostname: String::new(),
            cmd_name: String::new(),
            duration: Duration::new(0, 0),
            timestamp: 0,
        }
    }
}

/// Multiplex ssh commands
#[derive(Clone, Debug, Default, Getters, Setters)]
pub struct Multiplex {
    /// Is this going to be a dry run?
    #[get = "pub"]
    #[set = "pub"]
    dry_run: bool,
    /// Run the commands synchronously?
    #[get = "pub"]
    #[set = "pub"]
    synchronous: bool,
    /// stdout logging
    #[get = "pub"]
    #[set = "pub"]
    stdout: Option<Logger>,
    /// stderr logging
    #[get = "pub"]
    #[set = "pub"]
    stderr: Option<Logger>,
    /// command output logging
    #[get = "pub"]
    #[set = "pub"]
    host_loggers: HashMap<String, Option<Logger>>,
}

impl Multiplex {
    /// Multiplex the requested commands over the requested hosts
    #[must_use]
    pub fn multiplex(
        self,
        sync_hosts: &IndexSet<String>,
        hosts_map: MultiplexMapType,
    ) -> MultiplexResult {
        let wg = WaitGroup::new();
        let (tx, rx) = mpsc::channel();
        let count = hosts_map.len();
        let mut results = Vec::new();

        for (hostname, (host, cmd_map)) in hosts_map {
            // Setup the commands to run pre-sync
            let mut pre_cmds = IndexMap::new();
            if let Some(commands) = cmd_map.get(&CmdType::Cmd) {
                pre_cmds = commands.clone();
            }

            // Setup the commands to run post-sync
            let mut sync_cmds = IndexMap::new();
            if let Some(commands) = cmd_map.get(&CmdType::SyncCmd) {
                sync_cmds = commands.clone();
            }

            // If this is a sync host, add it to the wait group, and mark it
            let mut sync_host = false;
            if sync_hosts.contains(&hostname) {
                sync_host = true;
                wg.add(1);
            }

            if !self.dry_run {
                // Setup the clones to move into the thread
                let wg_cl = wg.clone();
                let tx_cl = tx.clone();
                let h_cl = host.clone();
                let stdout_cl = self.stdout.clone();
                let stderr_cl = self.stderr.clone();
                let cmd_cl = self.host_loggers.get(&hostname).unwrap_or(&None).clone();

                // The worker thread that will run the commands on the host
                let _ = thread::spawn(move || {
                    let mut results = execute(&stdout_cl, &stderr_cl, &cmd_cl, &h_cl, &pre_cmds);

                    if sync_host {
                        results.extend(execute(&stdout_cl, &stderr_cl, &cmd_cl, &h_cl, &sync_cmds));
                        wg_cl.done();
                    } else {
                        wg_cl.wait();
                        results.extend(execute(&stdout_cl, &stderr_cl, &cmd_cl, &h_cl, &sync_cmds));
                    }
                    tx_cl.send(results).expect("unable to send response");
                });

                if self.synchronous {
                    self.receive(&rx, &mut results);
                }
            }
        }

        if !self.dry_run && !self.synchronous {
            // Wait for all the threads to finish
            for _ in 0..count {
                self.receive(&rx, &mut results);
            }
        }

        results
    }

    fn receive(&self, rx: &Receiver<MultiplexResult>, output: &mut Vec<MusshResult<Metrics>>) {
        match rx.recv() {
            Ok(results) => output.extend(results),
            Err(e) => try_error!(self.stderr, "{}", e),
        }
    }
}

fn execute(
    stdout: &Option<Logger>,
    stderr: &Option<Logger>,
    cmd_logger: &Option<Logger>,
    host: &Host,
    cmds: &IndexMap<String, String>,
) -> MultiplexResult {
    cmds.iter()
        .map(|(cmd_name, cmd)| execute_on_host(stdout, stderr, cmd_logger, host, cmd_name, cmd))
        .collect()
}

fn execute_on_host(
    stdout: &Option<Logger>,
    stderr: &Option<Logger>,
    cmd_logger: &Option<Logger>,
    host: &Host,
    cmd_name: &str,
    cmd: &str,
) -> MusshResult<Metrics> {
    if host.hostname() == "localhost" {
        execute_on_localhost(stdout, stderr, cmd_logger, host, cmd_name, cmd)
    } else {
        execute_on_remote(stdout, stderr, cmd_logger, host, cmd_name, cmd)
    }
}

fn execute_on_localhost(
    stdout: &Option<Logger>,
    stderr: &Option<Logger>,
    cmd_logger: &Option<Logger>,
    host: &Host,
    cmd_name: &str,
    cmd: &str,
) -> MusshResult<Metrics> {
    if let Some(shell_path) = env::var_os("SHELL") {
        let timer = Instant::now();
        let fish = shell_path.to_string_lossy().to_string();
        let mut command = Command::new(&fish);
        let _ = command.arg("-c");
        let _ = command.arg(cmd);
        let _ = command.stdout(Stdio::piped());
        let _ = command.stderr(Stdio::piped());

        if let Ok(mut child) = command.spawn() {
            let child_stdout = child.stdout.take().ok_or_else(|| "Unable to get stdout")?;
            let stdout_reader = BufReader::new(child_stdout);
            for line in stdout_reader.lines() {
                if let Ok(line) = line {
                    try_trace!(cmd_logger, "{}", line);
                }
            }

            let status = child.wait()?;
            let duration = timer.elapsed();
            let hostname = host.hostname().clone();
            let elapsed_str = convert_duration(&duration);

            if status.success() {
                let mut metrics = Metrics::default();
                metrics.hostname = hostname;
                metrics.cmd_name = cmd_name.to_string();
                metrics.duration = duration;
                metrics.timestamp = Utc::now().timestamp_millis();
                try_info!(
                    stdout,
                    "execute";
                    "host" => host.hostname(),
                    "cmd" => cmd_name,
                    "duration" => elapsed_str
                );
                Ok(metrics)
            } else {
                try_error!(
                    stderr,
                    "execute";
                    "host" => host.hostname(),
                    "cmd" => cmd_name,
                    "duration" => elapsed_str
                );
                let err_msg = format!("Failed to run '{}' on '{}'", hostname, cmd_name);
                Err(MusshErrKind::NonZero(err_msg).into())
            }
        } else {
            Err(MusshErrKind::Spawn.into())
        }
    } else {
        Err(MusshErrKind::ShellNotFound.into())
    }
}

fn execute_on_remote(
    stdout: &Option<Logger>,
    stderr: &Option<Logger>,
    cmd_logger: &Option<Logger>,
    host: &Host,
    cmd_name: &str,
    cmd: &str,
) -> MusshResult<Metrics> {
    if let Ok(mut sess) = Session::new() {
        let timer = Instant::now();
        let host_tuple = (&host.hostname()[..], host.port().unwrap_or_else(|| 22));
        let tcp = TcpStream::connect(host_tuple)?;
        sess.set_tcp_stream(tcp);
        sess.handshake()?;
        if let Some(pem) = host.pem() {
            sess.userauth_pubkey_file(host.username(), None, Path::new(&pem), None)?;
        } else {
            sess.userauth_agent(host.username())?;
        }

        if sess.authenticated() {
            try_trace!(stdout, "execute"; "message" => "Authenticated");
            let mut channel = sess.channel_session()?;
            channel.exec(cmd)?;

            {
                let stdout_stream = channel.stream(0);
                let stdout_reader = BufReader::new(stdout_stream);

                for line in stdout_reader.lines() {
                    if let Ok(line) = line {
                        try_trace!(cmd_logger, "{}", line);
                    }
                }
            }

            let duration = timer.elapsed();
            let elapsed_str = convert_duration(&duration);

            match channel.exit_status() {
                Ok(code) => {
                    if code == 0 {
                        let mut metrics = Metrics::default();
                        metrics.hostname = host.hostname().to_string();
                        metrics.cmd_name = cmd_name.to_string();
                        metrics.duration = duration;
                        metrics.timestamp = Utc::now().timestamp_millis();

                        try_info!(
                            stdout,
                            "execute";
                            "host" => host.hostname(),
                            "cmd" => cmd_name,
                            "duration" => elapsed_str
                        );
                        Ok(metrics)
                    } else {
                        try_error!(
                            stderr,
                            "execute";
                            "host" => host.hostname(),
                            "cmd" => cmd_name,
                            "duration" => elapsed_str
                        );
                        let err_msg =
                            format!("Failed to run '{}' on '{}'", host.hostname(), cmd_name);
                        Err(MusshErrKind::NonZero(err_msg).into())
                    }
                }
                Err(e) => {
                    try_error!(
                        stderr,
                        "execute"; "hostname" => host.hostname(), "cmd" => cmd_name, "error" => format!("{}", e)
                    );
                    let err_msg = format!("Failed to run '{}' on '{}'", host.hostname(), cmd_name);
                    Err(MusshErrKind::SshExec(err_msg).into())
                }
            }
        } else {
            Err(MusshErrKind::SshAuthentication.into())
        }
    } else {
        Err(MusshErrKind::SshSession.into())
    }
}

#[cfg(test)]
mod tests {
    use super::Multiplex;
    use crate::config::test::test_cli;
    use crate::config::{HostsCmds, Mussh};
    use crate::error::MusshResult;

    crate const MUSSH_FULL_TOML: &str = r#"[hostlist.most]
hostnames = ["m1", "m2", "m3", "m4"]
[hostlist.m1]
hostnames = ["m1"]
[hostlist.m2]
hostnames = ["m2"]
[hostlist.m3]
hostnames = ["m3"]
[hostlist.m4]
hostnames = ["m4"]
[hosts.m1]
hostname = "localhost"
username = "jozias"

[[hosts.m1.alias]]
command = "ls.mac"
aliasfor = "ls"

[hosts.m2]
hostname = "localhost"
username = "jozias"

[hosts.m3]
hostname = "localhost"
username = "jozias"

[hosts.m4]
hostname = "localhost"
username = "jozias"

[cmd.bar]
command = "sleep 1"
[cmd.ls]
command = "ls -al"
[cmd.uname]
command = "uname -a"
"#;

    #[test]
    fn ssh_multiplex() -> MusshResult<()> {
        let config: Mussh = toml::from_str(&MUSSH_FULL_TOML)?;
        let cli = vec![
            "test", "-h", "most", "-c", "ls,uname", "-s", "m3,m4", "-y", "bar",
        ];
        let matches = test_cli().get_matches_from_safe(cli)?;
        let hosts_cmds = HostsCmds::from(&matches);
        let hosts_map = config.to_host_map(&hosts_cmds);
        let multiplex = Multiplex::default();
        let _ = multiplex.multiplex(hosts_cmds.sync_hosts(), hosts_map);
        Ok(())
    }
}

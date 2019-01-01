// Copyright (c) 2018 libmussh developers
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

//! Multiplex commands over hosts.
use crate::config::Host;
use crate::error::{MusshErrorKind, MusshResult};
use crate::utils::{convert_duration, CmdType, HostsMapType};
use indexmap::{IndexMap, IndexSet};
use std::env;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Instant;
use wait_group::WaitGroup;

/// Multiplex the requested commands over the requested hosts
pub fn multiplex(sync_hosts: &IndexSet<String>, hosts_map: HostsMapType) -> MusshResult<()> {
    let wg = WaitGroup::new();
    let (tx, rx) = mpsc::channel();
    let count = hosts_map.len();

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

        // Setup the clones to move into the thread
        let wg_cl = wg.clone();
        let tx_cl = tx.clone();
        let hn_cl = hostname.clone();
        let h_cl = host.clone();

        // The worker thread that will run the commands on the host
        let _ = thread::spawn(move || {
            if sync_host {
                println!("Running on sync host: {}", hn_cl);
            }
            let mut results: IndexMap<String, (String, MusshResult<()>)> =
                execute(&h_cl, &pre_cmds);

            if sync_host {
                results.extend(execute(&h_cl, &sync_cmds));
                println!("Done on '{}'", hn_cl);
                wg_cl.done();
            } else {
                println!("Waiting on sync commands on '{}'", hn_cl);
                wg_cl.wait();
                println!("Unblocked, running sync commands on '{}'", hn_cl);
                results.extend(execute(&h_cl, &sync_cmds));
            }
            tx_cl.send(results).expect("unable to send response");
        });
    }

    // Wait for all the threads to finish
    for _ in 0..count {
        match rx.recv() {
            Ok(results) => {
                for (cmd_name, (hostname, res)) in results {
                    if let Err(e) = res {
                        eprintln!("Failed to run '{}' on '{}': {}", cmd_name, hostname, e);
                        // try_error!(
                        //     self.stderr,
                        //     "Failed to run '{}' on '{}': {}",
                        //     cmd_name,
                        //     hostname,
                        //     e
                        // );
                    }
                }
            }
            Err(e) => eprintln!("{}", e),
            // try_error!(self.stderr, "{}", e);
        }
    }
    Ok(())
}

fn execute(
    host: &Host,
    cmds: &IndexMap<String, String>,
) -> IndexMap<String, (String, MusshResult<()>)> {
    cmds.iter()
        .map(|(cmd_name, cmd)| {
            (
                cmd_name.clone(),
                (
                    host.hostname().clone(),
                    execute_on_host(host, cmd_name, cmd),
                ),
            )
        })
        .collect()
}

fn execute_on_host(host: &Host, cmd_name: &str, cmd: &str) -> MusshResult<()> {
    if host.hostname() == "localhost" {
        execute_on_localhost(host, cmd_name, cmd)
    } else {
        execute_on_remote(host, cmd_name, cmd)
    }
}

fn execute_on_localhost(host: &Host, cmd_name: &str, cmd: &str) -> MusshResult<()> {
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
                    println!("{}", line);
                    // trace!(file_logger, "{}", line);
                }
            }

            let status = child.wait()?;
            let elapsed_str = convert_duration(timer.elapsed());

            if status.success() {
                println!("{}: {} => {}", host.hostname(), cmd_name, elapsed_str);
            //     try_info!(
            //         stdout,
            //         "execute";
            //         "host" => host.hostname(),
            //         "cmd" => cmd_name,
            //         "duration" => elapsed_str
            //     );
            } else {
                eprintln!(
                    "{}: {} ERROR! => {}",
                    host.hostname(),
                    cmd_name,
                    elapsed_str
                );
                // try_error!(
                //     stderr,
                //     "execute";
                //     "host" => host.hostname(),
                //     "cmd" => cmd_name,
                //     "duration" => elapsed_str
                // );
            }
        }
        Ok(())
    } else {
        Err(MusshErrorKind::ShellNotFound.into())
    }
}

fn execute_on_remote(_host: &Host, _cmd_name: &str, _cmd: &str) -> MusshResult<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::multiplex;
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
        let _ = multiplex(hosts_cmds.sync_hosts(), hosts_map);
        Ok(())
    }
}

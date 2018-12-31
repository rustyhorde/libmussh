// Copyright (c) 2018 libmussh developers
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

//! Multiplex commands over hosts.
use crate::config::Host;
use crate::utils::{CmdType, HostsMapType};
use failure::Fallible;
use indexmap::{IndexMap, IndexSet};
use std::sync::mpsc;
use std::thread;
use wait_group::WaitGroup;

/// Multiplex the requested commands over the requested hosts
pub fn multiplex(sync_hosts: &IndexSet<String>, hosts_map: HostsMapType) -> Fallible<()> {
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
            let mut results: IndexMap<String, (String, Fallible<()>)> = execute(&h_cl, &pre_cmds);

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
) -> IndexMap<String, (String, Fallible<()>)> {
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

fn execute_on_host(_host: &Host, _cmd_name: &str, _cmd: &str) -> Fallible<()> {
    // if host.hostname() == "localhost" {
    // execute_on_localhost(host, cmd_name, cmd)
    // } else {
    // execute_on_remote(host, cmd_name, cmd)
    // }

    Ok(())
}

#[cfg(test)]
mod tests {
    //     use super::Multiplex;
    //     use crate::config::{Alias, Command, Host, Mussh};
    //     use crate::utils::{as_set, CmdType, HostType};
    //     use clap::{App, Arg};
    //     use failure::Fallible;
    //     use indexmap::IndexMap;

    //     const MUSSH_TOML: &str = r#"[hostlist.most]
    // hostnames = ["m1", "m2", "m3", "m4"]
    // [hostlist.m1]
    // hostnames = ["m1"]
    // [hostlist.m2]
    // hostnames = ["m2"]
    // [hostlist.m3]
    // hostnames = ["m3"]
    // [hostlist.m4]
    // hostnames = ["m4"]
    // [hosts.m1]
    // hostname = "10.0.0.3"
    // username = "jozias"

    // [[hosts.m1.alias]]
    // command = "blah"
    // aliasfor = "ls"

    // [hosts.m2]
    // hostname = "10.0.0.4"
    // username = "jozias"

    // [hosts.m3]
    // hostname = "10.0.0.5"
    // username = "jozias"

    // [hosts.m4]
    // hostname = "10.0.0.60"
    // username = "jozias"

    // [cmd.bar]
    // command = "bar"
    // [cmd.ls]
    // command = "ls -al"
    // [cmd.uname]
    // command = "uname -a"
    // "#;

    //     macro_rules! string_set {
    //         ( $( $x:expr ),* )  => {
    //             as_set(vec![$($x),*].iter().map(|v| v.to_string()).collect::<Vec<String>>())
    //         };
    //     }

    //     fn test_cli<'a, 'b>() -> App<'a, 'b> {
    //         App::new(env!("CARGO_PKG_NAME"))
    //             .arg(
    //                 Arg::with_name("hosts")
    //                     .short("h")
    //                     .long("hosts")
    //                     .use_delimiter(true),
    //             )
    //             .arg(
    //                 Arg::with_name("commands")
    //                     .short("c")
    //                     .long("commands")
    //                     .use_delimiter(true),
    //             )
    //             .arg(
    //                 Arg::with_name("sync_hosts")
    //                     .short("s")
    //                     .long("sync_hosts")
    //                     .use_delimiter(true),
    //             )
    //             .arg(
    //                 Arg::with_name("sync_commands")
    //                     .short("y")
    //                     .long("sync_commands")
    //                     .use_delimiter(true),
    //             )
    //     }

    //     #[test]
    //     fn hosts_from_cli() -> Fallible<()> {
    //         let mut expected = Multiplex::default();
    //         expected.hosts = string_set!("m1", "m2", "m3");
    //         let cli = vec!["test", "-h", "m1,m2,m3,m1,m3"];
    //         let matches = test_cli().get_matches_from_safe(cli)?;

    //         assert_eq!(Multiplex::from(&matches), expected);
    //         Ok(())
    //     }

    //     #[test]
    //     fn sync_hosts_from_cli() -> Fallible<()> {
    //         let mut expected = Multiplex::default();
    //         expected.sync_hosts = string_set!("m1", "m2", "m3");
    //         let cli = vec!["test", "-s", "m1,m2,m3,m1,m3"];
    //         let matches = test_cli().get_matches_from_safe(cli)?;
    //         assert_eq!(Multiplex::from(&matches), expected);
    //         Ok(())
    //     }

    //     #[test]
    //     fn commands_from_cli() -> Fallible<()> {
    //         let mut expected = Multiplex::default();
    //         expected.commands = string_set!("foo", "bar", "baz");
    //         let cli = vec!["test", "-c", "foo,bar,foo,foo,baz,bar"];
    //         let matches = test_cli().get_matches_from_safe(cli)?;
    //         assert_eq!(Multiplex::from(&matches), expected);
    //         Ok(())
    //     }

    //     #[test]
    //     fn sync_commands_from_cli() -> Fallible<()> {
    //         let mut expected = Multiplex::default();
    //         expected.sync_commands = string_set!("foo", "bar", "baz");
    //         let cli = vec!["test", "-y", "foo,bar,foo,foo,baz,bar"];
    //         let matches = test_cli().get_matches_from_safe(cli)?;
    //         assert_eq!(Multiplex::from(&matches), expected);
    //         Ok(())
    //     }

    // #[test]
    // fn correct_hosts() -> Fallible<()> {
    //     let config: Mussh = toml::from_str(&MUSSH_TOML)?;
    //     let cli = vec![
    //         "test", "-h", "most,!m4", "-c", "ls", "-s", "m1,m2", "-y", "uname",
    //     ];
    //     let matches = test_cli().get_matches_from_safe(cli)?;
    //     let multiplex = Multiplex::from(&matches);

    //     // Setup expected results
    //     let mut expected_hosts = IndexMap::new();
    //     let mut m1_alias = Alias::default();
    //     let _ = m1_alias.set_command("blah".to_string());
    //     let _ = m1_alias.set_aliasfor("ls".to_string());
    //     let mut m1_host = Host::default();
    //     let _ = m1_host.set_hostname("10.0.0.3".to_string());
    //     let _ = m1_host.set_username("jozias".to_string());
    //     let _ = m1_host.set_alias(Some(vec![m1_alias]));
    //     let mut m2_host = Host::default();
    //     let _ = m2_host.set_hostname("10.0.0.4".to_string());
    //     let _ = m2_host.set_username("jozias".to_string());
    //     let mut m3_host = Host::default();
    //     let _ = m3_host.set_hostname("10.0.0.5".to_string());
    //     let _ = m3_host.set_username("jozias".to_string());
    //     let _ = expected_hosts.insert("m1".to_string(), m1_host.clone());
    //     let _ = expected_hosts.insert("m2".to_string(), m2_host.clone());
    //     let _ = expected_hosts.insert("m3".to_string(), m3_host);

    //     let mut expected_sync_hosts = IndexMap::new();
    //     let _ = expected_sync_hosts.insert("m1".to_string(), m1_host);
    //     let _ = expected_sync_hosts.insert("m2".to_string(), m2_host);

    //     let mut expected_cmds = IndexMap::new();
    //     let mut command = Command::default();
    //     let _ = command.set_command("ls -al".to_string());
    //     let _ = expected_cmds.insert("ls".to_string(), command);

    //     let mut expected_sync_cmds = IndexMap::new();
    //     let mut command = Command::default();
    //     let _ = command.set_command("uname -a".to_string());
    //     let _ = expected_sync_cmds.insert("uname".to_string(), command);

    //     // Setup actual results
    //     let actual_hosts = multiplex.actual_hosts(&config, HostType::Host);
    //     let actual_sync_hosts = multiplex.actual_hosts(&config, HostType::SyncHost);
    //     let actual_cmds = multiplex.actual_cmds(&config, CmdType::Cmd);
    //     let actual_sync_cmds = multiplex.actual_cmds(&config, CmdType::SyncCmd);

    //     // Asserts
    //     assert_eq!(expected_hosts, actual_hosts);
    //     assert_eq!(expected_sync_hosts, actual_sync_hosts);
    //     assert_eq!(expected_cmds, actual_cmds);
    //     assert_eq!(expected_sync_cmds, actual_sync_cmds);
    //     Ok(())
    // }

    // #[test]
    // fn multiplex() -> Fallible<()> {
    //     let config: Mussh = toml::from_str(&MUSSH_TOML)?;
    //     let cli = vec![
    //         "test", "-h", "most", "-c", "ls,uname", "-s", "m3,m4", "-y", "bar",
    //     ];
    //     let matches = test_cli().get_matches_from_safe(cli)?;
    //     let multiplex = Multiplex::from(&matches);
    //     let _ = multiplex.multiplex(&config);
    //     Ok(())
    // }
}

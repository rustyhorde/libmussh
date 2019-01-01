// Copyright (c) 2018 libmussh developers
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

use crate::error::{MusshErr, MusshResult};
use crate::utils::{self, CmdType, MultiplexMapType};
use clap::ArgMatches;
use getset::{Getters, Setters};
use indexmap::{IndexMap, IndexSet};
use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::PathBuf;

/// The runtime configuration for mussh
#[derive(Clone, Debug, Default, Eq, Getters, PartialEq, Setters)]
pub struct HostsCmds {
    /// The set of hosts to run the commands on
    #[get = "pub"]
    #[set = "pub"]
    hosts: IndexSet<String>,
    /// The set of hosts to run the sync commands on first
    #[get = "pub"]
    #[set = "pub"]
    sync_hosts: IndexSet<String>,
    /// The set of commands to run
    #[get = "pub"]
    #[set = "pub"]
    cmds: IndexSet<String>,
    /// The set of commands to run on sync hosts first, then
    /// regular hosts once those have completed.
    #[get = "pub"]
    #[set = "pub"]
    sync_cmds: IndexSet<String>,
}

impl From<&ArgMatches<'_>> for HostsCmds {
    fn from(matches: &ArgMatches<'_>) -> Self {
        let mut hosts_cmds = Self::default();
        hosts_cmds.hosts = utils::as_set(
            matches
                .values_of("hosts")
                .map_or_else(|| vec![], utils::map_vals),
        );

        hosts_cmds.sync_hosts = utils::as_set(
            matches
                .values_of("sync_hosts")
                .map_or_else(|| vec![], utils::map_vals),
        );

        hosts_cmds.cmds = utils::as_set(
            matches
                .values_of("commands")
                .map_or_else(|| vec![], utils::map_vals),
        );

        hosts_cmds.sync_cmds = utils::as_set(
            matches
                .values_of("sync_commands")
                .map_or_else(|| vec![], utils::map_vals),
        );

        hosts_cmds
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, Getters, PartialEq, Serialize)]
/// The base configuration.
pub struct Mussh {
    /// A list of hosts.
    #[serde(serialize_with = "toml::ser::tables_last")]
    #[get = "pub"]
    hostlist: BTreeMap<String, Hosts>,
    /// The hosts.
    #[serde(serialize_with = "toml::ser::tables_last")]
    #[get = "pub"]
    hosts: BTreeMap<String, Host>,
    /// A command.
    #[serde(serialize_with = "toml::ser::tables_last")]
    #[get = "pub"]
    cmd: BTreeMap<String, Command>,
}

impl Mussh {
    crate fn hostnames(&self, host: &str) -> Vec<String> {
        self.hostlist()
            .get(host)
            .map_or_else(|| vec![], |hosts| hosts.hostnames().clone())
    }

    fn configured_hostlists(&self) -> IndexSet<String> {
        utils::as_set(self.hostlist().keys().cloned())
    }

    crate fn configured_cmds(&self) -> IndexSet<String> {
        utils::as_set(self.cmd().keys().cloned())
    }

    fn requested(&self, commands: &IndexSet<String>) -> IndexSet<String> {
        utils::as_set(commands.iter().cloned())
    }

    fn expanded(&self, hosts: &IndexSet<String>) -> IndexSet<String> {
        utils::as_set(hosts.iter().flat_map(|host| self.hostnames(host)))
    }

    fn unwanted(&self, hosts: &IndexSet<String>) -> IndexSet<String> {
        utils::as_set(hosts.iter().filter_map(|host| utils::unwanted_host(host)))
    }

    fn host_tuple(&self, hostname: &str) -> Option<(String, Host)> {
        self.hosts()
            .get(hostname)
            .and_then(|host| Some((hostname.to_string(), host.clone())))
    }

    fn cmd_tuple(&self, cmd_name: &str) -> Option<(String, Command)> {
        self.cmd()
            .get(cmd_name)
            .and_then(|cmd| Some((cmd_name.to_string(), cmd.clone())))
    }

    fn actual_hosts(&self, hosts: &IndexSet<String>) -> IndexMap<String, Host> {
        let mut expanded = self.expanded(hosts);
        let unwanted = self.unwanted(hosts);
        expanded.retain(|x| !unwanted.contains(x));
        let configured = self.configured_hostlists();
        expanded
            .intersection(&configured)
            .filter_map(|hostname| self.host_tuple(hostname))
            .collect()
    }

    fn actual_cmds(&self, commands: &IndexSet<String>) -> IndexMap<String, Command> {
        let requested = self.requested(commands);
        let configured = self.configured_cmds();
        requested
            .intersection(&configured)
            .filter_map(|cmd_name| self.cmd_tuple(cmd_name))
            .collect()
    }

    fn actual_cmd_map(
        &self,
        target_host: &Host,
        expected_cmds: &IndexMap<String, Command>,
    ) -> IndexMap<String, String> {
        expected_cmds
            .iter()
            .map(|(cmd_name, command)| self.cmd_map_tuple(command, cmd_name, target_host))
            .collect()
    }

    fn cmd_map_tuple(&self, command: &Command, cmd_name: &str, host: &Host) -> (String, String) {
        (
            cmd_name.to_string(),
            if let Some(alias_vec) = host.alias() {
                let mut cmd = command.command().clone();
                for alias in alias_vec {
                    if alias.aliasfor() == cmd_name {
                        if let Some(int_command) = self.cmd().get(alias.command()) {
                            cmd = int_command.command().clone();
                            break;
                        }
                    }
                }
                cmd
            } else {
                command.command().clone()
            },
        )
    }

    /// Create a host map suitable for use with multiples from this config, and
    /// argument matches from clap.
    pub fn to_host_map(&self, host_cmds: &HostsCmds) -> MultiplexMapType {
        let actual_hosts = self.actual_hosts(host_cmds.hosts());
        let actual_cmds = self.actual_cmds(host_cmds.cmds());
        let actual_sync_hosts = self.actual_hosts(host_cmds.sync_hosts());
        let actual_sync_cmds = self.actual_cmds(host_cmds.sync_cmds());

        let mut hosts_map = IndexMap::new();

        for (hostname, host) in &actual_hosts {
            let cmd_tuple = hosts_map.entry(hostname.clone()).or_insert((
                host.clone(),
                IndexMap::<CmdType, IndexMap<String, String>>::new(),
            ));
            let cmds = self.actual_cmd_map(host, &actual_cmds);
            let sync_cmds = self.actual_cmd_map(host, &actual_sync_cmds);
            let _ = cmd_tuple.1.insert(CmdType::Cmd, cmds);
            let _ = cmd_tuple.1.insert(CmdType::SyncCmd, sync_cmds);
        }

        for (hostname, host) in &actual_sync_hosts {
            let cmd_tuple = hosts_map
                .entry(hostname.clone())
                .or_insert((host.clone(), IndexMap::new()));
            let cmds = self.actual_cmd_map(host, &actual_cmds);
            let sync_cmds = self.actual_cmd_map(host, &actual_sync_cmds);
            let _ = cmd_tuple.1.insert(CmdType::Cmd, cmds);
            let _ = cmd_tuple.1.insert(CmdType::SyncCmd, sync_cmds);
        }

        hosts_map
    }
}

impl TryFrom<PathBuf> for Mussh {
    type Error = MusshErr;

    fn try_from(path: PathBuf) -> MusshResult<Self> {
        let mut buf_reader = BufReader::new(File::open(path)?);
        let mut buffer = String::new();
        let _bytes_read = buf_reader.read_to_string(&mut buffer)?;
        Ok(toml::from_str(&buffer)?)
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, Getters, PartialEq, Serialize)]
/// hosts configuration
pub struct Hosts {
    /// The hostnames.
    #[get = "pub"]
    hostnames: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, Getters, PartialEq, Serialize, Setters)]
/// Host configuration.
pub struct Host {
    /// A hostname.
    #[get = "pub"]
    #[set = "pub"]
    hostname: String,
    /// A pem key.
    #[get = "pub"]
    pem: Option<String>,
    /// A port
    #[get = "pub"]
    port: Option<u16>,
    /// A username.
    #[get = "pub"]
    #[set = "pub"]
    username: String,
    /// A command alias.
    #[get = "pub"]
    #[set = "pub"]
    alias: Option<Vec<Alias>>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, Getters, PartialEq, Serialize, Setters)]
/// command configuration
pub struct Command {
    /// A Command.
    #[get = "pub"]
    #[set = "pub"]
    command: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, Getters, PartialEq, Serialize, Setters)]
/// command alias configuration.
pub struct Alias {
    /// A command alias.
    #[get = "pub"]
    #[set = "pub"]
    command: String,
    /// The command this is an alias for.
    #[get = "pub"]
    #[set = "pub"]
    aliasfor: String,
}

#[cfg(test)]
crate mod test {
    use super::{Alias, Command, Host, Hosts, HostsCmds, Mussh};
    use crate::error::MusshResult;
    use crate::utils::CmdType;
    use clap::{App, Arg};
    use indexmap::IndexMap;
    use lazy_static::lazy_static;
    use std::collections::BTreeMap;

    const ALIAS_TOML: &str = r#"command = "blah"
aliasfor = "dedah"
"#;
    const COMMAND_TOML: &str = r#"command = "blah"
"#;
    const HOST_TOML: &str = r#"hostname = "10.0.0.3"
pem = "abcdef"
port = 22
username = "jozias"

[[alias]]
command = "blah"
aliasfor = "dedah"
"#;
    const HOSTS_TOML: &str = r#"hostnames = ["m1", "m2", "m3"]
"#;

    const MUSSH_TOML: &str = r#"[hostlist.i686]
hostnames = ["m1", "m2", "m3"]
[hosts.m1]
hostname = "10.0.0.3"
pem = "abcdef"
port = 22
username = "jozias"

[[hosts.m1.alias]]
command = "blah"
aliasfor = "dedah"
[cmd.ls]
command = "blah"
"#;

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
hostname = "10.0.0.3"
username = "jozias"

[[hosts.m1.alias]]
command = "ls.mac"
aliasfor = "ls"

[hosts.m2]
hostname = "10.0.0.4"
username = "jozias"

[hosts.m3]
hostname = "10.0.0.5"
username = "jozias"

[hosts.m4]
hostname = "10.0.0.60"
username = "jozias"

[cmd.bar]
command = "bar"
[cmd.ls]
command = "ls -al"
[cmd.uname]
command = "uname -a"
"#;

    // macro_rules! string_set {
    //     ( $( $x:expr ),* )  => {
    //         as_set(vec![$($x),*].iter().map(|v| v.to_string()).collect::<Vec<String>>())
    //     };
    // }

    crate fn test_cli<'a, 'b>() -> App<'a, 'b> {
        App::new(env!("CARGO_PKG_NAME"))
            .arg(
                Arg::with_name("hosts")
                    .short("h")
                    .long("hosts")
                    .use_delimiter(true),
            )
            .arg(
                Arg::with_name("commands")
                    .short("c")
                    .long("commands")
                    .use_delimiter(true),
            )
            .arg(
                Arg::with_name("sync_hosts")
                    .short("s")
                    .long("sync_hosts")
                    .use_delimiter(true),
            )
            .arg(
                Arg::with_name("sync_commands")
                    .short("y")
                    .long("sync_commands")
                    .use_delimiter(true),
            )
    }

    lazy_static! {
        static ref ALIAS: Alias = Alias {
            command: "blah".to_string(),
            aliasfor: "dedah".to_string(),
        };
        static ref ALIAS_1: Alias = Alias {
            command: "ls.mac".to_string(),
            aliasfor: "ls".to_string(),
        };
        static ref COMMAND: Command = Command {
            command: "blah".to_string(),
        };
        static ref HOST_M1_DEF: Host = {
            let alias = ALIAS.clone();
            Host {
                hostname: "10.0.0.3".to_string(),
                pem: Some("abcdef".to_string()),
                port: Some(22),
                username: "jozias".to_string(),
                alias: Some(vec![alias]),
            }
        };
        static ref HOST_M1: Host = {
            let alias = ALIAS_1.clone();
            Host {
                hostname: "10.0.0.3".to_string(),
                pem: None,
                port: None,
                username: "jozias".to_string(),
                alias: Some(vec![alias]),
            }
        };
        static ref HOST_M2: Host = {
            Host {
                hostname: "10.0.0.4".to_string(),
                pem: None,
                port: None,
                username: "jozias".to_string(),
                alias: None,
            }
        };
        static ref HOST_M3: Host = {
            Host {
                hostname: "10.0.0.5".to_string(),
                pem: None,
                port: None,
                username: "jozias".to_string(),
                alias: None,
            }
        };
        static ref HOSTS: Hosts = Hosts {
            hostnames: vec!["m1".to_string(), "m2".to_string(), "m3".to_string()],
        };
        static ref MUSSH: Mussh = {
            let mut hostlist = BTreeMap::new();
            let _ = hostlist.insert("i686".to_string(), HOSTS.clone());
            let mut hosts = BTreeMap::new();
            let _ = hosts.insert("m1".to_string(), HOST_M1_DEF.clone());
            let mut cmd = BTreeMap::new();
            let _ = cmd.insert("ls".to_string(), COMMAND.clone());
            Mussh {
                hostlist: hostlist,
                hosts: hosts,
                cmd: cmd,
            }
        };
        static ref EMPTY_CMD_MAP: IndexMap<CmdType, IndexMap<String, String>> = {
            let mut cmd_map = IndexMap::new();
            let _ = cmd_map.insert(CmdType::Cmd, IndexMap::new());
            let _ = cmd_map.insert(CmdType::SyncCmd, IndexMap::new());
            cmd_map
        };
        static ref ALL_CMD_MAP: IndexMap<CmdType, IndexMap<String, String>> = {
            let mut cmd_map = IndexMap::new();
            let mut cmds_map = IndexMap::new();
            let _ = cmds_map.insert("ls".to_string(), "ls -al".to_string());
            let _ = cmds_map.insert("uname".to_string(), "uname -a".to_string());
            let _ = cmds_map.insert("bar".to_string(), "bar".to_string());
            let _ = cmd_map.insert(CmdType::Cmd, cmds_map);
            let _ = cmd_map.insert(CmdType::SyncCmd, IndexMap::new());
            cmd_map
        };
        static ref SYNC_CMD_MAP: IndexMap<CmdType, IndexMap<String, String>> = {
            let mut cmd_map = IndexMap::new();
            let mut cmds_map = IndexMap::new();
            let _ = cmds_map.insert("ls".to_string(), "ls -al".to_string());
            let _ = cmds_map.insert("uname".to_string(), "uname -a".to_string());
            let _ = cmds_map.insert("bar".to_string(), "bar".to_string());
            let _ = cmd_map.insert(CmdType::Cmd, IndexMap::new());
            let _ = cmd_map.insert(CmdType::SyncCmd, cmds_map);
            cmd_map
        };
    }

    #[test]
    fn de_alias() -> MusshResult<()> {
        let actual: Alias = toml::from_str(ALIAS_TOML)?;
        assert_eq!(*ALIAS, actual);
        Ok(())
    }

    #[test]
    fn ser_alias() -> MusshResult<()> {
        let expected = ALIAS_TOML;
        let actual = toml::to_string(&(*ALIAS))?;
        assert_eq!(expected, actual);
        Ok(())
    }

    #[test]
    fn de_command() -> MusshResult<()> {
        let actual: Command = toml::from_str(COMMAND_TOML)?;
        assert_eq!(*COMMAND, actual);
        Ok(())
    }

    #[test]
    fn ser_command() -> MusshResult<()> {
        let expected = COMMAND_TOML;
        let actual = toml::to_string(&(*COMMAND))?;
        assert_eq!(expected, actual);
        Ok(())
    }

    #[test]
    fn de_host() -> MusshResult<()> {
        let actual: Host = toml::from_str(HOST_TOML)?;
        assert_eq!(*HOST_M1_DEF, actual);
        Ok(())
    }

    #[test]
    fn ser_host() -> MusshResult<()> {
        let expected = HOST_TOML;
        let actual = toml::to_string(&(*HOST_M1_DEF))?;
        assert_eq!(expected, actual);
        Ok(())
    }

    #[test]
    fn de_hosts() -> MusshResult<()> {
        let actual: Hosts = toml::from_str(HOSTS_TOML)?;
        assert_eq!(*HOSTS, actual);
        Ok(())
    }

    #[test]
    fn ser_hosts() -> MusshResult<()> {
        let expected = HOSTS_TOML;
        let actual = toml::to_string(&(*HOSTS))?;
        assert_eq!(expected, actual);
        Ok(())
    }

    #[test]
    fn de_mussh() -> MusshResult<()> {
        let actual: Mussh = toml::from_str(MUSSH_TOML)?;
        assert_eq!(*MUSSH, actual);
        Ok(())
    }

    #[test]
    fn ser_mussh() -> MusshResult<()> {
        let expected = MUSSH_TOML;
        let actual = toml::to_string(&(*MUSSH))?;
        assert_eq!(expected, actual);
        Ok(())
    }

    #[test]
    fn hosts_from_cli() -> MusshResult<()> {
        let mut expected = IndexMap::new();
        let _ = expected.insert("m1".to_string(), (HOST_M1.clone(), EMPTY_CMD_MAP.clone()));
        let _ = expected.insert("m2".to_string(), (HOST_M2.clone(), EMPTY_CMD_MAP.clone()));
        let _ = expected.insert("m3".to_string(), (HOST_M3.clone(), EMPTY_CMD_MAP.clone()));
        let config: Mussh = toml::from_str(MUSSH_FULL_TOML)?;
        let cli = vec!["test", "-h", "m1,m2,m3,m1,m3"];
        let matches = test_cli().get_matches_from_safe(cli)?;
        let hosts_cmds = HostsCmds::from(&matches);
        assert_eq!(config.to_host_map(&hosts_cmds), expected);
        Ok(())
    }

    #[test]
    fn sync_hosts_from_cli() -> MusshResult<()> {
        let mut expected = IndexMap::new();
        let _ = expected.insert("m1".to_string(), (HOST_M1.clone(), EMPTY_CMD_MAP.clone()));
        let _ = expected.insert("m2".to_string(), (HOST_M2.clone(), EMPTY_CMD_MAP.clone()));
        let _ = expected.insert("m3".to_string(), (HOST_M3.clone(), EMPTY_CMD_MAP.clone()));
        let config: Mussh = toml::from_str(MUSSH_FULL_TOML)?;
        let cli = vec!["test", "-s", "m1,m2,m3,m1,m3"];
        let matches = test_cli().get_matches_from_safe(cli)?;
        let hosts_cmds = HostsCmds::from(&matches);
        assert_eq!(config.to_host_map(&hosts_cmds), expected);
        Ok(())
    }

    #[test]
    fn commands_from_cli() -> MusshResult<()> {
        let mut expected = IndexMap::new();
        let _ = expected.insert("m1".to_string(), (HOST_M1.clone(), ALL_CMD_MAP.clone()));
        let config: Mussh = toml::from_str(MUSSH_FULL_TOML)?;
        let cli = vec!["test", "-h", "m1", "-c", "ls,uname,bar,bar,ls,uname,bar"];
        let matches = test_cli().get_matches_from_safe(cli)?;
        let hosts_cmds = HostsCmds::from(&matches);
        assert_eq!(config.to_host_map(&hosts_cmds), expected);
        Ok(())
    }

    #[test]
    fn sync_commands_from_cli() -> MusshResult<()> {
        let mut expected = IndexMap::new();
        let _ = expected.insert("m1".to_string(), (HOST_M1.clone(), SYNC_CMD_MAP.clone()));
        let config: Mussh = toml::from_str(MUSSH_FULL_TOML)?;
        let cli = vec!["test", "-h", "m1", "-y", "ls,uname,bar,bar,ls,uname,bar"];
        let matches = test_cli().get_matches_from_safe(cli)?;
        let hosts_cmds = HostsCmds::from(&matches);
        assert_eq!(config.to_host_map(&hosts_cmds), expected);
        Ok(())
    }
}

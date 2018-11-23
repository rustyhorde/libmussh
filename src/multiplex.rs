// Copyright (c) 2018 libmussh developers
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

//! Multiplex commands over hosts.
use crate::config::Mussh;
use crate::utils::{self, CmdType, HostType};
use clap::ArgMatches;
use getset::{Getters, Setters};
use indexmap::IndexSet;

/// Multiplex struct
#[derive(Clone, Debug, Default, Getters, Eq, PartialEq, Setters)]
pub struct Multiplex {
    /// Hosts
    #[get = "pub"]
    hosts: IndexSet<String>,
    /// Host that need to complete `sync_commands` before run on
    /// other hosts
    #[get = "pub"]
    sync_hosts: IndexSet<String>,
    /// Commands
    #[get = "pub"]
    commands: IndexSet<String>,
    /// Commands that need to be run on `sync_hosts` before run
    /// on other hosts
    #[get = "pub"]
    sync_commands: IndexSet<String>,
}

impl<'a> From<&'a ArgMatches<'a>> for Multiplex {
    fn from(matches: &'a ArgMatches<'a>) -> Self {
        let mut run = Self::default();
        run.hosts = utils::as_set(
            matches
                .values_of("hosts")
                .map_or_else(|| vec![], utils::map_vals),
        );
        run.sync_hosts = utils::as_set(
            matches
                .values_of("sync_hosts")
                .map_or_else(|| vec![], utils::map_vals),
        );
        run.commands = utils::as_set(
            matches
                .values_of("commands")
                .map_or_else(|| vec![], utils::map_vals),
        );
        run.sync_commands = utils::as_set(
            matches
                .values_of("sync_commands")
                .map_or_else(|| vec![], utils::map_vals),
        );
        run
    }
}

impl Multiplex {
    fn requested(&self, cmd_type: &CmdType) -> IndexSet<String> {
        let cmds = match cmd_type {
            CmdType::Cmd => self.commands(),
            CmdType::SyncCmd => self.sync_commands(),
        };
        utils::as_set(cmds.iter().cloned())
    }
    fn expanded(&self, config: &Mussh, host_type: &HostType) -> IndexSet<String> {
        let hosts = match host_type {
            HostType::Host => self.hosts(),
            HostType::SyncHost => self.sync_hosts(),
        };
        utils::as_set(hosts.iter().flat_map(|host| config.hostnames(host)))
    }

    fn unwanted(&self, host_type: &HostType) -> IndexSet<String> {
        let hosts = match host_type {
            HostType::Host => self.hosts(),
            HostType::SyncHost => self.sync_hosts(),
        };
        utils::as_set(hosts.iter().filter_map(|host| utils::unwanted_host(host)))
    }

    fn actual_hosts(&self, config: &Mussh, host_type: &HostType) -> IndexSet<String> {
        let mut expanded = self.expanded(config, host_type);
        let unwanted = self.unwanted(host_type);
        expanded.retain(|x| !unwanted.contains(x));
        let configured = config.configured_hostlists();
        expanded.intersection(&configured).cloned().collect()
    }

    fn actual_cmds(&self, config: &Mussh, cmd_type: &CmdType) -> IndexSet<String> {
        let requested = self.requested(cmd_type);
        let configured = config.configured_cmds();
        requested.intersection(&configured).cloned().collect()
    }

    /// Multiplex the requested commands over the requested hosts
    pub fn multiplex(&self, config: &Mussh) {
        let _actual_hosts = self.actual_hosts(config, &HostType::Host);
        let _sync_hosts = self.actual_hosts(config, &HostType::SyncHost);
        let _actual_cmds = self.actual_cmds(config, &CmdType::Cmd);
        let _actual_sync_cmds = self.actual_cmds(config, &CmdType::SyncCmd);
    }
}

#[cfg(test)]
mod tests {
    use super::Multiplex;
    use crate::config::Mussh;
    use crate::utils::{as_set, CmdType, HostType};
    use clap::{App, Arg};
    use failure::Fallible;

    const MUSSH_TOML: &str = r#"[hostlist.most]
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
command = "blah"
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

[cmd.ls]
command = "ls -al"
[cmd.uname]
command = "uname -a"
"#;

    macro_rules! string_set {
        ($v:expr) => {
            as_set($v.iter().map(|v| v.to_string()).collect::<Vec<String>>())
        };
    }

    fn test_cli<'a, 'b>() -> App<'a, 'b> {
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

    #[test]
    fn hosts_from_cli() -> Fallible<()> {
        let mut expected = Multiplex::default();
        expected.hosts = string_set!(vec!["m1", "m2", "m3"]);
        let cli = vec!["test", "-h", "m1,m2,m3,m1,m3"];
        let matches = test_cli().get_matches_from_safe(cli)?;

        assert_eq!(Multiplex::from(&matches), expected);
        Ok(())
    }

    #[test]
    fn sync_hosts_from_cli() -> Fallible<()> {
        let mut expected = Multiplex::default();
        expected.sync_hosts = string_set!(vec!["m1", "m2", "m3"]);
        let cli = vec!["test", "-s", "m1,m2,m3,m1,m3"];
        let matches = test_cli().get_matches_from_safe(cli)?;
        assert_eq!(Multiplex::from(&matches), expected);
        Ok(())
    }

    #[test]
    fn commands_from_cli() -> Fallible<()> {
        let mut expected = Multiplex::default();
        expected.commands = string_set!(vec!["foo", "bar", "baz"]);
        let cli = vec!["test", "-c", "foo,bar,foo,foo,baz,bar"];
        let matches = test_cli().get_matches_from_safe(cli)?;
        assert_eq!(Multiplex::from(&matches), expected);
        Ok(())
    }

    #[test]
    fn sync_commands_from_cli() -> Fallible<()> {
        let mut expected = Multiplex::default();
        expected.sync_commands = string_set!(vec!["foo", "bar", "baz"]);
        let cli = vec!["test", "-y", "foo,bar,foo,foo,baz,bar"];
        let matches = test_cli().get_matches_from_safe(cli)?;
        assert_eq!(Multiplex::from(&matches), expected);
        Ok(())
    }

    #[test]
    fn correct_hosts() -> Fallible<()> {
        let config: Mussh = toml::from_str(&MUSSH_TOML)?;
        let cli = vec![
            "test", "-h", "most,!m4", "-c", "ls", "-s", "m1,m2", "-y", "uname",
        ];
        let matches = test_cli().get_matches_from_safe(cli)?;
        let multiplex = Multiplex::from(&matches);

        let expected_hosts = string_set!(vec!["m1", "m2", "m3"]);
        let expected_sync_hosts = string_set!(vec!["m1", "m2"]);
        let expected_cmds = string_set!(vec!["ls"]);
        let expected_sync_cmds = string_set!(vec!["uname"]);
        let actual_hosts = multiplex.actual_hosts(&config, &HostType::Host);
        let actual_sync_hosts = multiplex.actual_hosts(&config, &HostType::SyncHost);
        let actual_cmds = multiplex.actual_cmds(&config, &CmdType::Cmd);
        let actual_sync_cmds = multiplex.actual_cmds(&config, &CmdType::SyncCmd);
        assert_eq!(expected_hosts, actual_hosts);
        assert_eq!(expected_sync_hosts, actual_sync_hosts);
        assert_eq!(expected_cmds, actual_cmds);
        assert_eq!(expected_sync_cmds, actual_sync_cmds);
        Ok(())
    }
}

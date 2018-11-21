// Copyright (c) 2018 libmussh developers
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

use crate::utils;
use failure::{Error, Fallible};
use getset::Getters;
use indexmap::IndexSet;
use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::PathBuf;

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

    crate fn configured_hostlists(&self) -> IndexSet<String> {
        utils::as_set(self.hostlist().keys().cloned())
    }

    crate fn configured_cmds(&self) -> IndexSet<String> {
        utils::as_set(self.cmd().keys().cloned())
    }
}

impl TryFrom<PathBuf> for Mussh {
    type Error = Error;

    fn try_from(path: PathBuf) -> Fallible<Self> {
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

#[derive(Clone, Debug, Default, Deserialize, Eq, Getters, PartialEq, Serialize)]
/// Host configuration.
pub struct Host {
    /// A hostname.
    #[get = "pub"]
    hostname: String,
    /// A pem key.
    #[get = "pub"]
    pem: Option<String>,
    /// A port
    #[get = "pub"]
    port: Option<u16>,
    /// A username.
    #[get = "pub"]
    username: String,
    /// A command alias.
    #[get = "pub"]
    alias: Option<Vec<Alias>>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, Getters, PartialEq, Serialize)]
/// command configuration
pub struct Command {
    /// A Command.
    #[get = "pub"]
    command: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, Getters, PartialEq, Serialize)]
/// command alias configuration.
pub struct Alias {
    /// A command alias.
    #[get = "pub"]
    command: String,
    /// The command this is an alias for.
    #[get = "pub"]
    aliasfor: String,
}

#[cfg(test)]
mod test {
    use super::{Alias, Command, Host, Hosts, Mussh};
    use failure::Fallible;
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

    lazy_static! {
        static ref ALIAS: Alias = Alias {
            command: "blah".to_string(),
            aliasfor: "dedah".to_string(),
        };
        static ref COMMAND: Command = Command {
            command: "blah".to_string(),
        };
        static ref HOST: Host = {
            let alias = ALIAS.clone();
            Host {
                hostname: "10.0.0.3".to_string(),
                pem: Some("abcdef".to_string()),
                port: Some(22),
                username: "jozias".to_string(),
                alias: Some(vec![alias]),
            }
        };
        static ref HOSTS: Hosts = Hosts {
            hostnames: vec!["m1".to_string(), "m2".to_string(), "m3".to_string()],
        };
        static ref MUSSH: Mussh = {
            let mut hostlist = BTreeMap::new();
            let _ = hostlist.insert("i686".to_string(), HOSTS.clone());
            let mut hosts = BTreeMap::new();
            let _ = hosts.insert("m1".to_string(), HOST.clone());
            let mut cmd = BTreeMap::new();
            let _ = cmd.insert("ls".to_string(), COMMAND.clone());
            Mussh {
                hostlist: hostlist,
                hosts: hosts,
                cmd: cmd,
            }
        };
    }

    #[test]
    fn de_alias() -> Fallible<()> {
        let actual: Alias = toml::from_str(ALIAS_TOML)?;
        assert_eq!(*ALIAS, actual);
        Ok(())
    }

    #[test]
    fn ser_alias() -> Fallible<()> {
        let expected = ALIAS_TOML;
        let actual = toml::to_string(&(*ALIAS))?;
        assert_eq!(expected, actual);
        Ok(())
    }

    #[test]
    fn de_command() -> Fallible<()> {
        let actual: Command = toml::from_str(COMMAND_TOML)?;
        assert_eq!(*COMMAND, actual);
        Ok(())
    }

    #[test]
    fn ser_command() -> Fallible<()> {
        let expected = COMMAND_TOML;
        let actual = toml::to_string(&(*COMMAND))?;
        assert_eq!(expected, actual);
        Ok(())
    }

    #[test]
    fn de_host() -> Fallible<()> {
        let actual: Host = toml::from_str(HOST_TOML)?;
        assert_eq!(*HOST, actual);
        Ok(())
    }

    #[test]
    fn ser_host() -> Fallible<()> {
        let expected = HOST_TOML;
        let actual = toml::to_string(&(*HOST))?;
        assert_eq!(expected, actual);
        Ok(())
    }

    #[test]
    fn de_hosts() -> Fallible<()> {
        let actual: Hosts = toml::from_str(HOSTS_TOML)?;
        assert_eq!(*HOSTS, actual);
        Ok(())
    }

    #[test]
    fn ser_hosts() -> Fallible<()> {
        let expected = HOSTS_TOML;
        let actual = toml::to_string(&(*HOSTS))?;
        assert_eq!(expected, actual);
        Ok(())
    }

    #[test]
    fn de_mussh() -> Fallible<()> {
        let actual: Mussh = toml::from_str(MUSSH_TOML)?;
        assert_eq!(*MUSSH, actual);
        Ok(())
    }

    #[test]
    fn ser_mussh() -> Fallible<()> {
        let expected = MUSSH_TOML;
        let actual = toml::to_string(&(*MUSSH))?;
        assert_eq!(expected, actual);
        Ok(())
    }
}

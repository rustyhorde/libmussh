// Copyright (c) 2018 libmussh developers
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

use failure::{Error, Fallible};
use getset::Getters;
use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::PathBuf;

#[derive(Clone, Debug, Default, Deserialize, Eq, Getters, PartialEq, Serialize)]
/// The base configuration.
crate struct Mussh {
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
crate struct Hosts {
    /// The hostnames.
    #[get = "pub"]
    hostnames: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, Getters, PartialEq, Serialize)]
/// Host configuration.
crate struct Host {
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
crate struct Command {
    /// A Command.
    #[get = "pub"]
    command: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, Getters, PartialEq, Serialize)]
/// command alias configuration.
crate struct Alias {
    /// A command alias.
    #[get = "pub"]
    command: String,
    /// The command this is an alias for.
    #[get = "pub"]
    aliasfor: String,
}

#[cfg(test)]
mod test {
    use super::{Alias, Command};
    use failure::Fallible;

    const ALIAS: &str = r#"command = "blah"
aliasfor = "dedah"
"#;
    const COMMAND: &str = r#"command = "blah"
"#;
    #[allow(dead_code)]
    const HOST: &str = r#"hostname = "10.0.0.3"
port = 22
pem = "abcdef"
username = "jozias"
[[alias]]
command = "blah"
aliasfor = "dedah"
"#;

    #[test]
    fn de_alias() -> Fallible<()> {
        let expected = Alias {
            command: "blah".to_string(),
            aliasfor: "dedah".to_string(),
        };
        let actual: Alias = toml::from_str(ALIAS)?;
        assert_eq!(expected, actual);
        Ok(())
    }

    #[test]
    fn ser_alias() -> Fallible<()> {
        let expected = ALIAS;
        let actual = toml::to_string(&Alias {
            command: "blah".to_string(),
            aliasfor: "dedah".to_string(),
        })?;
        assert_eq!(expected, actual);
        Ok(())
    }

    #[test]
    fn de_command() -> Fallible<()> {
        let expected = Command {
            command: "blah".to_string(),
        };
        let actual: Command = toml::from_str(COMMAND)?;
        assert_eq!(expected, actual);
        Ok(())
    }

    #[test]
    fn ser_command() -> Fallible<()> {
        let expected = COMMAND;
        let actual = toml::to_string(&Command {
            command: "blah".to_string(),
        })?;
        assert_eq!(expected, actual);
        Ok(())
    }
}

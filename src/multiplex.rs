// Copyright (c) 2018 libmussh developers
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

//! Multiplex commands over hosts.
use clap::ArgMatches;
use crate::utils;
use getset::{Getters, Setters};
use indexmap::IndexSet;

/// Multiplex struct
#[derive(Clone, Debug, Default, Getters, Eq, PartialEq, Setters)]
pub struct Multiplex {
    /// Hosts
    hosts: IndexSet<String>,
    /// Host that need to complete `sync_commands` before run on
    /// other hosts
    sync_hosts: IndexSet<String>,
    /// Commands
    commands: IndexSet<String>,
    ///s Commands that need to be run on `sync_hosts` before run
    /// on other hosts
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

#[cfg(test)]
mod tests {
    use super::Multiplex;
    use clap::{App, Arg};
    use crate::utils::as_set;
    use failure::Fallible;

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
        expected.hosts = as_set(
            vec!["m1", "m2", "m3"]
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<String>>(),
        );
        let cli = vec!["test", "-h", "m1,m2,m3,m1,m3"];
        let matches = test_cli().get_matches_from_safe(cli)?;
        assert_eq!(Multiplex::from(&matches), expected);
        Ok(())
    }

    #[test]
    fn sync_hosts_from_cli() -> Fallible<()> {
        let mut expected = Multiplex::default();
        expected.sync_hosts = as_set(
            vec!["m1", "m2", "m3"]
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<String>>(),
        );
        let cli = vec!["test", "-s", "m1,m2,m3,m1,m3"];
        let matches = test_cli().get_matches_from_safe(cli)?;
        assert_eq!(Multiplex::from(&matches), expected);
        Ok(())
    }

    #[test]
    fn commands_from_cli() -> Fallible<()> {
        let mut expected = Multiplex::default();
        expected.commands = as_set(
            vec!["foo", "bar", "baz"]
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<String>>(),
        );
        let cli = vec!["test", "-c", "foo,bar,foo,foo,baz,bar"];
        let matches = test_cli().get_matches_from_safe(cli)?;
        assert_eq!(Multiplex::from(&matches), expected);
        Ok(())
    }

    #[test]
    fn sync_commands_from_cli() -> Fallible<()> {
        let mut expected = Multiplex::default();
        expected.sync_commands = as_set(
            vec!["foo", "bar", "baz"]
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<String>>(),
        );
        let cli = vec!["test", "-y", "foo,bar,foo,foo,baz,bar"];
        let matches = test_cli().get_matches_from_safe(cli)?;
        assert_eq!(Multiplex::from(&matches), expected);
        Ok(())
    }

}

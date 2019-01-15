// Copyright © 2018 libmussh developers
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

//! Utilities
use crate::config::Host;
use clap::Values;
use indexmap::{IndexMap, IndexSet};
use std::fmt;
use std::hash::Hash;
use std::iter::FromIterator;
use std::time::Duration;

/// Type used by multiplex to run commands on hosts
///
/// This is a map of the following: `Host Name` to `Command Tuple`
/// The `Command Tuple` consists of a `Host` and a `CmdType` map
/// The `CmdType` map contains a map of `Command Name` to actual `Command`
pub type MultiplexMapType = IndexMap<String, (Host, IndexMap<CmdType, IndexMap<String, String>>)>;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[allow(dead_code)]
crate enum HostType {
    Host,
    SyncHost,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CmdType {
    Cmd,
    SyncCmd,
}

impl fmt::Display for CmdType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                CmdType::Cmd => "cmd",
                CmdType::SyncCmd => "sync_cmd",
            }
        )
    }
}

crate fn unwanted_host(host: &str) -> Option<String> {
    if host.starts_with('!') {
        Some((*host).split_at(1).1.to_string())
    } else {
        None
    }
}

crate fn as_set<S, T>(iter: T) -> IndexSet<S>
where
    T: IntoIterator<Item = S>,
    S: Hash + Eq,
{
    IndexSet::from_iter(iter)
}

crate fn map_vals(values: Values<'_>) -> Vec<String> {
    values.map(|v| v.to_string()).collect()
}

crate fn convert_duration(duration: &Duration) -> String {
    let seconds = duration.as_secs();
    let millis = duration.subsec_millis();
    if seconds < 1 {
        format!("00:00:00.{:03}", duration.as_millis())
    } else if seconds < 60 {
        format!("00:00:{:02}.{:03}", seconds, millis)
    } else if seconds < 3600 {
        let minutes = seconds / 60;
        let seconds = seconds % 60;
        format!("00:{:02}:{:02}.{:03}", minutes, seconds, millis)
    } else if seconds < 86400 {
        let total_minutes = seconds / 60;
        let seconds = seconds % 60;
        let hours = total_minutes / 60;
        let minutes = total_minutes % 60;
        format!("{}:{:02}:{:02}.{:03}", hours, minutes, seconds, millis)
    } else {
        format!("{}s", seconds)
    }
}

#[cfg(test)]
mod test {
    use super::as_set;
    use indexmap::IndexSet;

    #[test]
    fn nums_as_set() {
        let expected: IndexSet<_> = vec![1, 2, 3, 4, 5].into_iter().collect();
        let nums = vec![1, 2, 1, 4, 3, 2, 4, 5];
        assert_eq!(as_set(nums), expected)
    }

    #[test]
    fn strings_as_set() {
        let expected: IndexSet<_> = vec!["one", "two", "three"].into_iter().collect();
        let actual = vec!["one", "three", "three", "two", "one", "two", "two"];
        assert_eq!(as_set(actual), expected);
    }
}

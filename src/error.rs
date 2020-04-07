// Copyright © 2018 libmussh developers
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

//! Error Handling
use std::error::Error;
use std::fmt;

/// A result that includes a `mussh::Error`
pub type MusshResult<T> = Result<T, MusshErr>;

/// An error thrown by the mussh library
#[derive(Debug)]
pub struct MusshErr {
    /// The kind of error
    inner: MusshErrKind,
}

impl Error for MusshErr {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.inner)
    }
}

impl fmt::Display for MusshErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let err: &(dyn Error) = self;
        let mut iter = err.chain();
        let _skip_me = iter.next();
        write!(f, "libmussh error")?;

        for e in iter {
            writeln!(f)?;
            write!(f, "{}", e)?;
        }
        Ok(())
    }
}

macro_rules! external_error {
    ($error:ty, $kind:expr) => {
        impl From<$error> for MusshErr {
            fn from(inner: $error) -> Self {
                Self {
                    inner: $kind(inner),
                }
            }
        }
    };
}

impl From<MusshErrKind> for MusshErr {
    fn from(inner: MusshErrKind) -> Self {
        Self { inner }
    }
}

impl From<&str> for MusshErr {
    fn from(inner: &str) -> Self {
        Self {
            inner: MusshErrKind::Str(inner.to_string()),
        }
    }
}

external_error!(clap::Error, MusshErrKind::Clap);
external_error!(ssh2::Error, MusshErrKind::Ssh2);
external_error!(std::io::Error, MusshErrKind::Io);
external_error!(toml::de::Error, MusshErrKind::TomlDe);
external_error!(toml::ser::Error, MusshErrKind::TomlSer);

#[derive(Debug)]
crate enum MusshErrKind {
    Clap(clap::Error),
    Io(std::io::Error),
    NonZero(String),
    ShellNotFound,
    Ssh2(ssh2::Error),
    SshAuthentication,
    SshExec(String),
    SshSession,
    Spawn,
    Str(String),
    TomlDe(toml::de::Error),
    TomlSer(toml::ser::Error),
}

impl Error for MusshErrKind {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            MusshErrKind::Clap(inner) => inner.source(),
            MusshErrKind::Io(inner) => inner.source(),
            MusshErrKind::Ssh2(inner) => inner.source(),
            MusshErrKind::TomlDe(inner) => inner.source(),
            MusshErrKind::TomlSer(inner) => inner.source(),
            _ => None,
        }
    }
}

impl fmt::Display for MusshErrKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MusshErrKind::Clap(inner) => write!(f, "{}", inner),
            MusshErrKind::Io(inner) => write!(f, "{}", inner),
            MusshErrKind::Ssh2(inner) => write!(f, "{}", inner),
            MusshErrKind::TomlDe(inner) => write!(f, "{}", inner),
            MusshErrKind::TomlSer(inner) => write!(f, "{}", inner),
            _ => Ok(()),
        }
    }
}

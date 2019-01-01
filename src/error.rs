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
    fn description(&self) -> &str {
        "Mussh Error"
    }

    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.inner)
    }
}

impl fmt::Display for MusshErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.description())?;

        if let Some(source) = self.source() {
            write!(f, ": {}", source)?;
        }
        write!(f, "")
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
    SshSession,
    SshAuthentication,
    ShellNotFound,
    Ssh2(ssh2::Error),
    Str(String),
    TomlDe(toml::de::Error),
    TomlSer(toml::ser::Error),
}

impl Error for MusshErrKind {
    fn description(&self) -> &str {
        match self {
            MusshErrKind::Clap(inner) => inner.description(),
            MusshErrKind::Io(inner) => inner.description(),
            MusshErrKind::SshAuthentication => "ssh authentication",
            MusshErrKind::SshSession => "ssh session",
            MusshErrKind::ShellNotFound => "no acceptable shell found",
            MusshErrKind::Ssh2(inner) => inner.description(),
            MusshErrKind::Str(inner) => &inner[..],
            MusshErrKind::TomlDe(inner) => inner.description(),
            MusshErrKind::TomlSer(inner) => inner.description(),
        }
    }

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
        write!(f, "{}", self.description())
    }
}

use std::error::Error;
use std::fmt;

/// A result that includes a `MusshError`
pub type MusshResult<T> = Result<T, MusshError>;

/// An error thrown by the mussh library
#[derive(Debug)]
pub struct MusshError {
    /// The kind of error
    inner: MusshErrorKind,
}

impl Error for MusshError {
    fn description(&self) -> &str {
        "Mussh Error"
    }

    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.inner)
    }
}

impl fmt::Display for MusshError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.description())?;

        if let Some(source) = self.source() {
            write!(f, ": {}", source)?;
        }
        write!(f, "")
    }
}

impl From<MusshErrorKind> for MusshError {
    fn from(inner: MusshErrorKind) -> Self {
        Self { inner }
    }
}

impl From<&str> for MusshError {
    fn from(inner: &str) -> Self {
        Self {
            inner: MusshErrorKind::Str(inner.to_string()),
        }
    }
}

impl From<std::io::Error> for MusshError {
    fn from(inner: std::io::Error) -> Self {
        Self {
            inner: MusshErrorKind::Io(inner),
        }
    }
}

impl From<toml::de::Error> for MusshError {
    fn from(inner: toml::de::Error) -> Self {
        Self {
            inner: MusshErrorKind::TomlDe(inner),
        }
    }
}

impl From<toml::ser::Error> for MusshError {
    fn from(inner: toml::ser::Error) -> Self {
        Self {
            inner: MusshErrorKind::TomlSer(inner),
        }
    }
}

impl From<clap::Error> for MusshError {
    fn from(inner: clap::Error) -> Self {
        Self {
            inner: MusshErrorKind::Clap(inner),
        }
    }
}

#[derive(Debug)]
crate enum MusshErrorKind {
    Clap(clap::Error),
    Io(std::io::Error),
    SshSession,
    SshAuthentication,
    ShellNotFound,
    Str(String),
    TomlDe(toml::de::Error),
    TomlSer(toml::ser::Error),
}

impl Error for MusshErrorKind {
    fn description(&self) -> &str {
        match self {
            MusshErrorKind::Clap(inner) => inner.description(),
            MusshErrorKind::Io(inner) => inner.description(),
            MusshErrorKind::SshAuthentication => "ssh authentication",
            MusshErrorKind::SshSession => "ssh session",
            MusshErrorKind::ShellNotFound => "no acceptable shell found",
            MusshErrorKind::Str(inner) => &inner[..],
            MusshErrorKind::TomlDe(inner) => inner.description(),
            MusshErrorKind::TomlSer(inner) => inner.description(),
        }
    }

    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            MusshErrorKind::Clap(inner) => inner.source(),
            MusshErrorKind::Io(inner) => inner.source(),
            MusshErrorKind::TomlDe(inner) => inner.source(),
            MusshErrorKind::TomlSer(inner) => inner.source(),
            _ => None,
        }
    }
}

impl fmt::Display for MusshErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

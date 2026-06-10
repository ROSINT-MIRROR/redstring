//! The crate's one error vocabulary. Every fallible operation returns
//! `RsResult` so callers always see a named, matchable failure — the status
//! bar prints it, the validator lists it, nothing gets erased into a String.

use std::path::PathBuf;

pub type RsResult<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    /// A filesystem operation failed at a known path.
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    /// A RON document failed to parse.
    RonParse {
        path: PathBuf,
        source: ron::error::SpannedError,
    },
    /// A value failed to serialize to RON.
    RonSer { source: ron::Error },
    /// An entity image failed to decode.
    Image {
        path: PathBuf,
        source: image::ImageError,
    },
    /// Neither XDG_CONFIG_HOME nor HOME is usable.
    NoConfigDir,
}

impl Error {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    pub fn ron_parse(path: impl Into<PathBuf>, source: ron::error::SpannedError) -> Self {
        Self::RonParse {
            path: path.into(),
            source,
        }
    }

    pub fn image(path: impl Into<PathBuf>, source: image::ImageError) -> Self {
        Self::Image {
            path: path.into(),
            source,
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::RonParse { path, source } => {
                write!(f, "{}: bad RON: {source}", path.display())
            }
            Self::RonSer { source } => write!(f, "RON serialize: {source}"),
            Self::Image { path, source } => {
                write!(f, "{}: bad image: {source}", path.display())
            }
            Self::NoConfigDir => {
                write!(f, "no usable config dir — XDG_CONFIG_HOME and HOME are unset")
            }
        }
    }
}

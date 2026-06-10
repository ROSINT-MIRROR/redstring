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
    /// The derived case.json failed to serialize.
    JsonSer { source: serde_json::Error },
    /// An entity image failed to decode.
    Image {
        path: PathBuf,
        source: image::ImageError,
    },
    /// Neither XDG_CONFIG_HOME nor HOME is usable.
    NoConfigDir,
    /// The path is not a redstring case directory — no case file at its root.
    NotAProject { path: PathBuf },
    /// The case file was written by a newer redstring.
    FormatTooNew { found: u32, supported: u32 },
    /// A zip archive operation failed.
    Zip {
        path: PathBuf,
        source: zip::result::ZipError,
    },
    /// The bundle has no case file inside — not a redstring bundle.
    BundleMissingCase { path: PathBuf },
    /// The bundle's case cites a file that is not in the archive.
    BundleMissingRef { reference: String },
    /// Refusing to extract over an existing directory.
    TargetExists { path: PathBuf },
    /// The bundle already sits inside its own case directory.
    BundleInsideProject { path: PathBuf },
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

    pub fn zip(path: impl Into<PathBuf>, source: zip::result::ZipError) -> Self {
        Self::Zip {
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
            Self::JsonSer { source } => write!(f, "JSON serialize: {source}"),
            Self::Image { path, source } => {
                write!(f, "{}: bad image: {source}", path.display())
            }
            Self::NoConfigDir => {
                write!(f, "no usable config dir — XDG_CONFIG_HOME and HOME are unset")
            }
            Self::NotAProject { path } => {
                write!(f, "{}: not a redstring case (no case.redstring)", path.display())
            }
            Self::FormatTooNew { found, supported } => write!(
                f,
                "case format {found} is newer than this redstring supports ({supported}) — update redstring"
            ),
            Self::Zip { path, source } => write!(f, "{}: {source}", path.display()),
            Self::BundleMissingCase { path } => {
                write!(f, "{}: no case.redstring inside — not a redstring bundle", path.display())
            }
            Self::BundleMissingRef { reference } => {
                write!(f, "bundle integrity: cited file '{reference}' is missing from the archive")
            }
            Self::TargetExists { path } => {
                write!(f, "{}: already exists — move it away first", path.display())
            }
            Self::BundleInsideProject { path } => write!(
                f,
                "this bundle sits inside the case directory {} — open that case instead",
                path.display()
            ),
        }
    }
}

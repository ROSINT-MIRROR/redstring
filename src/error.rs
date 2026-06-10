use std::path::PathBuf;

pub type RsResult<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },

    RonParse {
        path: PathBuf,
        source: ron::error::SpannedError,
    },

    RonSer {
        source: ron::Error,
    },

    JsonSer {
        source: serde_json::Error,
    },

    Image {
        path: PathBuf,
        source: image::ImageError,
    },

    NoConfigDir,

    NotAProject {
        path: PathBuf,
    },

    FormatTooNew {
        found: u32,
        supported: u32,
    },

    Zip {
        path: PathBuf,
        source: zip::result::ZipError,
    },

    BundleMissingCase {
        path: PathBuf,
    },

    BundleMissingRef {
        reference: String,
    },

    TargetExists {
        path: PathBuf,
    },

    BundleInsideProject {
        path: PathBuf,
    },

    DuplicateSlug {
        slug: String,
    },
}

impl std::error::Error for Error {}

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
                write!(
                    f,
                    "no usable config dir — XDG_CONFIG_HOME and HOME are unset"
                )
            }
            Self::NotAProject { path } => {
                write!(
                    f,
                    "{}: not a redstring case (no case.redstring)",
                    path.display()
                )
            }
            Self::FormatTooNew { found, supported } => write!(
                f,
                "case format {found} is newer than this redstring supports ({supported}) — update redstring"
            ),
            Self::Zip { path, source } => write!(f, "{}: {source}", path.display()),
            Self::BundleMissingCase { path } => {
                write!(
                    f,
                    "{}: no case.redstring inside — not a redstring bundle",
                    path.display()
                )
            }
            Self::BundleMissingRef { reference } => {
                write!(
                    f,
                    "bundle integrity: cited file '{reference}' is missing from the archive"
                )
            }
            Self::TargetExists { path } => {
                write!(f, "{}: already exists — move it away first", path.display())
            }
            Self::BundleInsideProject { path } => write!(
                f,
                "this bundle sits inside the case directory {} — open that case instead",
                path.display()
            ),
            Self::DuplicateSlug { slug } => {
                write!(f, "duplicate slug '{slug}' — slugs must be case-unique")
            }
        }
    }
}

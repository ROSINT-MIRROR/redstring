use std::path::PathBuf;


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

    JsonParse {
        path: PathBuf,
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

    Clock {
        source: std::time::SystemTimeError,
    },

    Random {
        source: getrandom::Error,
    },

    SealedLeak {
        path: String,
        token: String,
    },

    UnknownExtractor {
        id: String,
    },

    BadExtractConfig {
        id: String,
        source: ron::Error,
    },

    BadRegex {
        pattern: String,
        source: regex::Error,
    },

    ShotArgs {
        what: String,
    },

    Render {
        what: String,
    },

    PathTrace {
        node: usize,
    },

    Timeline {
        what: String,
    },

    Alibi {
        what: String,
    },

    #[cfg(test)]
    InvariantViolated {
        what: String,
    },

    #[cfg(test)]
    Testkit {
        what: String,
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
            Self::JsonParse { path, source } => {
                write!(f, "{}: bad JSON: {source}", path.display())
            }
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
            Self::Clock { source } => {
                write!(f, "system clock is before the unix epoch: {source}")
            }
            Self::Random { source } => {
                write!(f, "no entropy available for a sealed alias: {source}")
            }
            Self::SealedLeak { path, token } => write!(
                f,
                "export aborted: '{path}' carries sealed material '{token}' — bundle not written"
            ),
            Self::UnknownExtractor { id } => {
                write!(f, "unknown extract strategy '{id}'")
            }
            Self::BadExtractConfig { id, source } => {
                write!(f, "extract '{id}' config: {source}")
            }
            Self::BadRegex { pattern, source } => {
                write!(f, "bad regex '{pattern}': {source}")
            }
            Self::ShotArgs { what } => write!(f, "--shot: {what}"),
            Self::Render { what } => write!(f, "offscreen render failed: {what}"),
            Self::PathTrace { node } => {
                write!(f, "path reconstruction hit a missing link at node {node}")
            }
            Self::Timeline { what } => write!(f, "timeline: {what}"),
            Self::Alibi { what } => write!(f, "alibi: {what}"),
            #[cfg(test)]
            Self::InvariantViolated { what } => write!(f, "invariant violated: {what}"),
            #[cfg(test)]
            Self::Testkit { what } => write!(f, "testkit: {what}"),
        }
    }
}

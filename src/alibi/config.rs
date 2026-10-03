use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;

use crate::error::Error;
use crate::prefs::Prefs;

pub const ALIBI_BIN: &str = "alibi";
pub const ARCHIVE_DEADLINE_SECS: u64 = 300;
pub const CHECK_WATCHDOG: Duration = Duration::from_secs(120);
pub const ARCHIVE_WATCHDOG: Duration = Duration::from_secs(ARCHIVE_DEADLINE_SECS + 60);
pub const PUMP_EVERY: Duration = Duration::from_millis(40);
pub const KEY_ENV: &str = "ARCHIVE_ORG_KEY";
pub const KEY_ID: &str = "web_archive";
pub const DETAIL_MAX: usize = 400;
pub const HOME_BINS: [&str; 3] = [".cargo/bin", ".local/share/cargo/bin", ".local/bin"];

pub struct SearchEnv {
    pub pinned: Option<PathBuf>,
    pub path: Vec<PathBuf>,
    pub cargo_home: Option<PathBuf>,
    pub home: Option<PathBuf>,
}

pub fn search_env(prefs: &Prefs) -> Result<SearchEnv, Error> {
    let raw_path: Option<OsString> = std::env::var_os("PATH");
    let mut path: Vec<PathBuf> = Vec::new();
    for joined in raw_path.iter() {
        path.extend(std::env::split_paths(joined));
    }
    Ok(SearchEnv {
        pinned: prefs.alibi_bin.as_deref().map(str::trim).filter(|p| !p.is_empty()).map(PathBuf::from),
        path,
        cargo_home: std::env::var_os("CARGO_HOME").map(PathBuf::from),
        home: std::env::var_os("HOME").map(PathBuf::from),
    })
}

pub enum ArchiveKey {
    FromPrefs(String),
    Inherited,
}

pub fn archive_key(prefs: &Prefs) -> Result<ArchiveKey, Error> {
    for (id, key) in &prefs.api_keys {
        if id == KEY_ID && !key.trim().is_empty() {
            return Ok(ArchiveKey::FromPrefs(key.trim().to_owned()));
        }
    }
    Ok(ArchiveKey::Inherited)
}

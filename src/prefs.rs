//! UI preferences — chrome state that belongs to this machine, not the case
//! file. A case must open identically on a colleague's box regardless of how
//! they like their sidebar. One RON file under XDG config; loaded once at
//! startup, written on change. Failures propagate — the caller decides what
//! a prefs problem means, and in the app that's a status-bar line.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{Error, RsResult};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)] // older prefs files keep loading as fields get added
pub struct Prefs {
    pub sidebar_collapsed: bool,
    /// Global UI scale, the egui zoom factor — bump on hi-DPI/4K monitors.
    pub ui_scale: f32,
    pub theme: Theme,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            sidebar_collapsed: false,
            ui_scale: 1.0,
            theme: Theme::Dark,
        }
    }
}

impl Prefs {
    /// XDG_CONFIG_HOME/redstring/prefs.ron, falling back to ~/.config.
    /// Unix-only by design.
    fn path() -> RsResult<PathBuf> {
        Ok(config_base()?.join("redstring").join("prefs.ron"))
    }

    /// Read prefs. A missing file is first-run, not an error — defaults.
    pub fn load() -> RsResult<Self> {
        let path = Self::path()?;
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(source) => return Err(Error::io(&path, source)),
        };
        ron::from_str(&text).map_err(|source| Error::ron_parse(&path, source))
    }

    /// Write prefs to disk, creating the config directory on first save.
    pub fn save(&self) -> RsResult<()> {
        let path = Self::path()?;
        let Some(dir) = path.parent() else {
            return Err(Error::NoConfigDir);
        };
        std::fs::create_dir_all(dir).map_err(|source| Error::io(dir, source))?;
        let text = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .map_err(|source| Error::RonSer { source })?;
        std::fs::write(&path, text).map_err(|source| Error::io(&path, source))?;
        Ok(())
    }
}

/// XDG config base when set, else HOME/.config.
fn config_base() -> RsResult<PathBuf> {
    let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) else {
        let Some(home) = std::env::var_os("HOME").filter(|v| !v.is_empty()) else {
            return Err(Error::NoConfigDir);
        };
        return Ok(PathBuf::from(home).join(".config"));
    };
    Ok(PathBuf::from(xdg))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Forward-compat: a prefs file from an older build (missing fields) and
    /// an empty one must both load with defaults filled in.
    #[test]
    fn partial_and_empty_files_load() {
        let p: Prefs = ron::from_str("(sidebar_collapsed: true)").unwrap();
        assert!(p.sidebar_collapsed);
        assert_eq!(p.ui_scale, Prefs::default().ui_scale);
        assert_eq!(p.theme, Theme::Dark);

        let p: Prefs = ron::from_str("()").unwrap();
        assert_eq!(p, Prefs::default());
    }

    #[test]
    fn round_trip() {
        let p = Prefs {
            sidebar_collapsed: true,
            ui_scale: 1.75,
            theme: Theme::Light,
        };
        let s = ron::ser::to_string_pretty(&p, ron::ser::PrettyConfig::default()).unwrap();
        let back: Prefs = ron::from_str(&s).unwrap();
        assert_eq!(p, back);
    }
}

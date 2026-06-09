//! UI preferences — chrome state that belongs to this machine, not the case
//! file (a case must open identically on a colleague's box regardless of how
//! they like their sidebar). One RON file under XDG config; loaded once at
//! startup, written on mutation. A failed prefs write never blocks the app.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)] // older prefs files keep loading as fields get added
pub struct Prefs {
    pub sidebar_collapsed: bool,
    /// Height of the add-entity kind grid (user-draggable).
    pub kind_picker_height: f32,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            sidebar_collapsed: false,
            kind_picker_height: 140.0,
        }
    }
}

impl Prefs {
    /// `$XDG_CONFIG_HOME/redstring/prefs.ron`, falling back to
    /// `~/.config/redstring/prefs.ron`. Unix-only by design (no Windows path).
    fn path() -> Option<PathBuf> {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
        Some(base.join("redstring").join("prefs.ron"))
    }

    pub fn load() -> Self {
        Self::path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| ron::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Best-effort write-on-mutation.
    pub fn save(&self) {
        let Some(path) = Self::path() else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(s) = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()) {
            let _ = std::fs::write(path, s);
        }
    }
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
        assert_eq!(p.kind_picker_height, Prefs::default().kind_picker_height);

        let p: Prefs = ron::from_str("()").unwrap();
        assert_eq!(p, Prefs::default());
    }

    #[test]
    fn round_trip() {
        let p = Prefs {
            sidebar_collapsed: true,
            kind_picker_height: 222.5,
        };
        let s = ron::ser::to_string_pretty(&p, ron::ser::PrettyConfig::default()).unwrap();
        let back: Prefs = ron::from_str(&s).unwrap();
        assert_eq!(p, back);
    }
}

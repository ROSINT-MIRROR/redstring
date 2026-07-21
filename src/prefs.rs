use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::Error;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

const MAX_RECENT: usize = 8;

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    pub sidebar_collapsed: bool,
    pub inspector_collapsed: bool,

    #[serde(default = "default_palette_width")]
    pub palette_width: f32,

    pub ui_scale: f32,
    pub theme: Theme,

    pub recent: Vec<String>,

    pub api_keys: Vec<(String, String)>,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            sidebar_collapsed: false,
            inspector_collapsed: false,
            palette_width: default_palette_width(),
            ui_scale: 1.0,
            theme: Theme::Dark,
            recent: Vec::new(),
            api_keys: Vec::new(),
        }
    }
}

impl Prefs {
    fn path() -> Result<PathBuf, Error> {
        Ok(config_base()?.join("redstring").join("prefs.ron"))
    }

    pub fn load() -> Result<Self, Error> {
        let path = Self::path()?;
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(source) => return Err(Error::Io { path: path.to_path_buf(), source }),
        };
        ron::from_str(&text).map_err(|source| Error::RonParse { path: path.to_path_buf(), source })
    }

    pub fn save(&self) -> Result<(), Error> {
        let path = Self::path()?;
        let Some(dir) = path.parent() else {
            return Err(Error::NoConfigDir);
        };
        std::fs::create_dir_all(dir).map_err(|source| Error::Io { path: dir.to_path_buf(), source })?;
        let text = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .map_err(|source| Error::RonSer { source })?;
        std::fs::write(&path, text).map_err(|source| Error::Io { path: path.to_path_buf(), source })?;
        Ok(())
    }

    pub fn remember_recent(&mut self, path: String) -> Result<(), Error> {
        push_recent(&mut self.recent, path);
        self.save()
    }

    pub fn forget_recent(&mut self, path: &str) -> Result<(), Error> {
        self.recent.retain(|r| r != path);
        self.save()
    }
}

fn push_recent(list: &mut Vec<String>, path: String) {
    list.retain(|r| r != &path);
    list.insert(0, path);
    list.truncate(MAX_RECENT);
}

fn default_palette_width() -> f32 {
    280.0
}

pub fn config_base() -> Result<PathBuf, Error> {
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
            inspector_collapsed: true,
            palette_width: 320.0,
            ui_scale: 1.75,
            theme: Theme::Light,
            recent: vec!["/tmp/case_a".into(), "/tmp/case_b".into()],
            api_keys: vec![("opensanctions".into(), "key123".into())],
        };
        let s = ron::ser::to_string_pretty(&p, ron::ser::PrettyConfig::default()).unwrap();
        let back: Prefs = ron::from_str(&s).unwrap();
        assert_eq!(p, back);
        assert_eq!(back.palette_width, 320.0);
    }

    #[test]
    fn palette_width_default_and_omitted() {
        assert_eq!(default_palette_width(), 280.0);
        assert_eq!(Prefs::default().palette_width, 280.0);

        let p: Prefs = ron::from_str("(sidebar_collapsed: true)").unwrap();
        assert_eq!(p.palette_width, 280.0);
    }

    #[test]
    fn recents_dedup_front_and_cap() {
        let mut list: Vec<String> = Vec::new();
        for i in 0..12 {
            push_recent(&mut list, format!("/tmp/case_{i}"));
        }
        assert_eq!(list.len(), MAX_RECENT);
        assert_eq!(list[0], "/tmp/case_11");

        push_recent(&mut list, "/tmp/case_7".into());
        assert_eq!(list[0], "/tmp/case_7");
        assert_eq!(list.iter().filter(|r| *r == "/tmp/case_7").count(), 1);

        list.retain(|r| r != "/tmp/case_7");
        assert!(!list.iter().any(|r| r == "/tmp/case_7"));
    }
}

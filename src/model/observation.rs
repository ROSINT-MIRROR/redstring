use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use egui::Color32;
use serde::{Deserialize, Serialize};

use crate::error::Error;

use super::source::Source;


pub const DEFAULT_CONFIDENCE: u8 = 100;


static SEQ: AtomicU64 = AtomicU64::new(0);


#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct ObsId(pub String);


#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Author {
    pub name: String,
    pub email: String,
}

impl Default for Author {
    fn default() -> Self {
        Self {
            name: "unknown".to_owned(),
            email: String::new(),
        }
    }
}


#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Verification {
    #[default]
    Unreviewed,
    MachineSuggested,
    AnalystAsserted,
    Corroborated,
    Rejected,
}


#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Origin {
    #[default]
    Manual,
    Lookup(String),
    Import(String),
    Inference,
}


#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum ObsValue {
    Text(String),
    Number(f64),
    Date(i64),
    Coord { lat: f64, lon: f64 },
    Bool(bool),
}

impl Default for ObsValue {
    fn default() -> Self {
        ObsValue::Text(String::new())
    }
}

impl ObsValue {
    pub fn text(&self) -> Result<String, Error> {
        Ok(match self {
            ObsValue::Text(s) => s.clone(),
            ObsValue::Number(n) => format!("{n}"),
            ObsValue::Date(d) => format!("{d}"),
            ObsValue::Coord { lat, lon } => format!("{lat},{lon}"),
            ObsValue::Bool(b) => format!("{b}"),
        })
    }
}


#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Observation {
    pub id: ObsId,
    pub name: String,
    #[serde(default)]
    pub value: ObsValue,
    #[serde(default)]
    pub origin: Origin,
    #[serde(default)]
    pub status: Verification,
    #[serde(default = "full_confidence")]
    pub confidence: u8,
    #[serde(default)]
    pub author: Author,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub sources: Vec<Source>,
}

fn full_confidence() -> u8 {
    DEFAULT_CONFIDENCE
}

impl Observation {
    pub fn stamp(
        name: String,
        value: ObsValue,
        origin: Origin,
        status: Verification,
        author: Author,
    ) -> Result<Self, Error> {
        Ok(Self {
            id: fresh_id(&author.email)?,
            name,
            value,
            origin,
            status,
            confidence: DEFAULT_CONFIDENCE,
            author,
            created_at: now_unix()?,
            sources: Vec::new(),
        })
    }
}


pub fn status_color(status: Verification) -> Result<Color32, Error> {
    Ok(match status {
        Verification::Rejected => Color32::from_rgb(235, 80, 80),
        Verification::Unreviewed => Color32::from_rgb(235, 150, 70),
        Verification::MachineSuggested => Color32::from_rgb(230, 200, 90),
        Verification::AnalystAsserted => Color32::from_rgb(110, 190, 200),
        Verification::Corroborated => Color32::from_rgb(120, 200, 140),
    })
}


pub fn status_label(status: Verification) -> Result<&'static str, Error> {
    Ok(match status {
        Verification::Unreviewed => "unreviewed",
        Verification::MachineSuggested => "machine",
        Verification::AnalystAsserted => "asserted",
        Verification::Corroborated => "corroborated",
        Verification::Rejected => "rejected",
    })
}


pub fn now_unix() -> Result<i64, Error> {
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|source| Error::Clock { source })?;
    Ok(d.as_secs() as i64)
}


pub fn fresh_id(author_email: &str) -> Result<ObsId, Error> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|source| Error::Clock { source })?
        .as_nanos();
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let salt = fnv1a(author_email).wrapping_add(seq);
    Ok(ObsId(format!("{nanos:024x}{salt:016x}")))
}


fn fnv1a(s: &str) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}


pub fn resolve_identity(root: Option<&Path>) -> Result<Author, Error> {
    let name = git_config("user.name", root);
    let email = git_config("user.email", root);
    let (Some(name), Some(email)) = (name, email) else {
        return Ok(Author::default());
    };
    Ok(Author { name, email })
}


#[allow(for_loops_over_fallibles)]
fn git_config(key: &str, root: Option<&Path>) -> Option<String> {
    let mut cmd = std::process::Command::new("git");
    for dir in root {
        cmd.arg("-C").arg(dir);
    }
    let Ok(out) = cmd.arg("config").arg("--get").arg(key).output() else {
        return None;
    };
    if !out.status.success() {
        return None;
    }
    let Ok(text) = String::from_utf8(out.stdout) else {
        return None;
    };
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_time_sortable() {
        let a = fresh_id("x@y.z").unwrap();
        let b = fresh_id("x@y.z").unwrap();
        assert_ne!(a, b, "two stamps must not collide");
        assert!(a.0 < b.0, "later id sorts after earlier: {} !< {}", a.0, b.0);
        assert_eq!(a.0.len(), b.0.len(), "fixed-width hex id");
    }

    #[test]
    fn stamp_defaults_full_confidence_and_carries_author() {
        let author = Author {
            name: "Z".to_owned(),
            email: "z@e".to_owned(),
        };
        let o = Observation::stamp(
            "dob".to_owned(),
            ObsValue::Text("1971-03-04".to_owned()),
            Origin::Manual,
            Verification::AnalystAsserted,
            author.clone(),
        )
        .unwrap();
        assert_eq!(o.confidence, DEFAULT_CONFIDENCE);
        assert_eq!(o.author, author);
        assert!(o.created_at > 0);
        assert!(o.sources.is_empty());
    }

    #[test]
    fn value_text_renders_each_variant() {
        assert_eq!(ObsValue::Text("hi".to_owned()).text().unwrap(), "hi");
        assert_eq!(ObsValue::Number(4.5).text().unwrap(), "4.5");
        assert_eq!(ObsValue::Bool(true).text().unwrap(), "true");
        assert_eq!(
            ObsValue::Coord { lat: 1.0, lon: 2.0 }.text().unwrap(),
            "1,2"
        );
    }
}

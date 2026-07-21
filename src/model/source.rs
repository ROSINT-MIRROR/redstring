use egui::Color32;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum SourceKind {
    Markdown,
    Url,
    Archive,
    Transcript,
    Document,
    #[default]
    Note,
}

pub struct SourceMeta {
    pub kind: SourceKind,
    pub label: &'static str,
    pub hint: &'static str,
    pub file_backed: bool,
}

const fn s(kind: SourceKind, label: &'static str, hint: &'static str, file_backed: bool) -> SourceMeta {
    SourceMeta {
        kind,
        label,
        hint,
        file_backed,
    }
}

#[rustfmt::skip]
pub const SOURCE_META: [SourceMeta; 6] = [
    s(SourceKind::Markdown, "Markdown ref", "notes.md#section", true),
    s(SourceKind::Url, "URL", "https://…", false),
    s(SourceKind::Archive, "Archive", "snapshot id / path", true),
    s(SourceKind::Transcript, "Transcript", "mm:ss", true),
    s(SourceKind::Document, "Document", "file · page", true),
    s(SourceKind::Note, "Note", "free text", false),
];

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum SourceTier {
    #[default]
    Unrated,
    Primary,
    Secondary,
    Reported,
}

pub struct SourceTierMeta {
    pub tier: SourceTier,
    pub label: &'static str,
    pub color: Color32,
}

const fn t(tier: SourceTier, label: &'static str, color: Color32) -> SourceTierMeta {
    SourceTierMeta { tier, label, color }
}

#[rustfmt::skip]
pub const SOURCE_TIER_META: [SourceTierMeta; 4] = [
    t(SourceTier::Unrated,   "unrated",   Color32::from_rgb(150, 155, 165)),
    t(SourceTier::Primary,   "primary",   Color32::from_rgb(110, 195, 130)),
    t(SourceTier::Secondary, "secondary", Color32::from_rgb(100, 170, 210)),
    t(SourceTier::Reported,  "reported",  Color32::from_rgb(225, 175, 85)),
];

fn tier_is_unrated(tier: &SourceTier) -> bool {
    *tier == SourceTier::Unrated
}

#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct Source {
    pub kind: SourceKind,
    pub reference: String,
    #[serde(default)]
    pub excerpt: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fetched_at: Option<i64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_org: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archive_url: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ots: Option<String>,

    #[serde(default, skip_serializing_if = "tier_is_unrated")]
    pub tier: SourceTier,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_mirrors_declaration_order() {
        for (i, m) in SOURCE_META.iter().enumerate() {
            assert_eq!(m.kind as usize, i, "SOURCE_META[{i}] is {:?}", m.kind);
        }
    }

    #[test]
    fn tier_table_mirrors_declaration_order_and_is_distinct() {
        for (i, m) in SOURCE_TIER_META.iter().enumerate() {
            assert_eq!(m.tier as usize, i, "SOURCE_TIER_META[{i}] is {:?}", m.tier);
        }
        let mut labels: Vec<&str> = SOURCE_TIER_META.iter().map(|m| m.label).collect();
        labels.sort();
        labels.dedup();
        assert_eq!(labels.len(), SOURCE_TIER_META.len(), "tier labels must be distinct");
    }

    #[test]
    fn legacy_line_parses_with_provenance_defaults() {
        let old = r#"(kind: Url, reference: "https://crt.sh/?q=x")"#;
        let s: Source = ron::from_str(old).expect("a pre-provenance source line must still parse");
        assert_eq!(s.kind, SourceKind::Url);
        assert_eq!(s.reference, "https://crt.sh/?q=x");
        assert!(s.excerpt.is_empty());
        assert!(s.sha256.is_none(), "sha256 defaults to None");
        assert!(s.fetched_at.is_none());
        assert!(s.source_org.is_none());
        assert!(s.archive_url.is_none());
        assert!(s.ots.is_none());
        assert_eq!(s.tier, SourceTier::Unrated, "tier defaults to Unrated");
    }

    #[test]
    fn provenance_source_round_trips() {
        let s = Source {
            kind: SourceKind::Document,
            reference: "evidence/docs/whois-x.txt".to_owned(),
            excerpt: "Registrar: Example".to_owned(),
            sha256: Some("a".repeat(64)),
            fetched_at: Some(1_700_000_000),
            source_org: Some("crt.sh".to_owned()),
            archive_url: Some("https://web.archive.org/web/x".to_owned()),
            ots: Some("evidence/x.ots".to_owned()),
            tier: SourceTier::Primary,
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: Source = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back, "a fully-populated source must survive save→load through JSON");
        let ron_s = ron::ser::to_string(&s).unwrap();
        let ron_back: Source = ron::from_str(&ron_s).unwrap();
        assert_eq!(s, ron_back, "and through RON");
    }

    #[test]
    fn defaults_are_omitted_from_serialization() {
        let bare = Source {
            kind: SourceKind::Url,
            reference: "https://x".to_owned(),
            ..Default::default()
        };
        let json = serde_json::to_string(&bare).unwrap();
        assert!(!json.contains("sha256"), "unset sha256 must not bloat the line: {json}");
        assert!(!json.contains("tier"), "Unrated tier must not bloat the line: {json}");
        assert!(!json.contains("fetched_at"), "unset fetched_at must not appear: {json}");
        assert!(!json.contains("archive_url"), "unset archive_url must not appear: {json}");
    }
}

//! Core investigation model: entities (nodes), relations (edges), and the
//! sources that justify each relation. The whole thesis of redstring lives on
//! the edge: a `Relation` with an empty `sources` vec is an unproven assertion.

use std::collections::BTreeMap;

use egui::Color32;
use serde::{Deserialize, Serialize};

/// What an entity *is*. Drives icon + colour on the board.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum EntityKind {
    Person,
    Org,
    Domain,
    Ip,
    Email,
    Phone,
    Account,
    Document,
    Event,
    Location,
    Asset,
    Money,
    Other,
}

impl EntityKind {
    pub const ALL: [EntityKind; 13] = [
        Self::Person,
        Self::Org,
        Self::Domain,
        Self::Ip,
        Self::Email,
        Self::Phone,
        Self::Account,
        Self::Document,
        Self::Event,
        Self::Location,
        Self::Asset,
        Self::Money,
        Self::Other,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Person => "Person",
            Self::Org => "Org",
            Self::Domain => "Domain",
            Self::Ip => "IP",
            Self::Email => "Email",
            Self::Phone => "Phone",
            Self::Account => "Account",
            Self::Document => "Document",
            Self::Event => "Event",
            Self::Location => "Location",
            Self::Asset => "Asset",
            Self::Money => "Money",
            Self::Other => "Other",
        }
    }

    /// Leading glyph for the node label. Sticks to glyphs egui's bundled
    /// NotoEmoji subset actually renders; ASCII fallback for the iffy ones.
    pub fn icon(self) -> &'static str {
        match self {
            Self::Person => "👤",
            Self::Org => "🏢",
            Self::Domain => "🌐",
            Self::Ip => "📡",
            Self::Email => "✉",
            Self::Phone => "☎",
            Self::Account => "@",
            Self::Document => "📄",
            Self::Event => "📅",
            Self::Location => "📍",
            Self::Asset => "📦",
            Self::Money => "💰",
            Self::Other => "?",
        }
    }

    pub fn color(self) -> Color32 {
        match self {
            Self::Person => Color32::from_rgb(100, 170, 255),
            Self::Org => Color32::from_rgb(255, 170, 80),
            Self::Domain => Color32::from_rgb(120, 210, 150),
            Self::Ip => Color32::from_rgb(90, 200, 200),
            Self::Email => Color32::from_rgb(200, 150, 255),
            Self::Phone => Color32::from_rgb(255, 200, 90),
            Self::Account => Color32::from_rgb(255, 140, 180),
            Self::Document => Color32::from_rgb(200, 200, 200),
            Self::Event => Color32::from_rgb(255, 120, 120),
            Self::Location => Color32::from_rgb(150, 220, 120),
            Self::Asset => Color32::from_rgb(210, 180, 140),
            Self::Money => Color32::from_rgb(240, 220, 90),
            Self::Other => Color32::from_rgb(170, 170, 170),
        }
    }
}

/// The verb on the red string.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum RelationKind {
    Controls,
    Owns,
    MemberOf,
    EmployedBy,
    CommunicatedWith,
    Financed,
    Transferred,
    LocatedAt,
    RegisteredTo,
    AliasOf,
    RelatedTo,
}

impl RelationKind {
    pub const ALL: [RelationKind; 11] = [
        Self::Controls,
        Self::Owns,
        Self::MemberOf,
        Self::EmployedBy,
        Self::CommunicatedWith,
        Self::Financed,
        Self::Transferred,
        Self::LocatedAt,
        Self::RegisteredTo,
        Self::AliasOf,
        Self::RelatedTo,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Controls => "controls",
            Self::Owns => "owns",
            Self::MemberOf => "member of",
            Self::EmployedBy => "employed by",
            Self::CommunicatedWith => "communicated with",
            Self::Financed => "financed",
            Self::Transferred => "transferred to",
            Self::LocatedAt => "located at",
            Self::RegisteredTo => "registered to",
            Self::AliasOf => "alias of",
            Self::RelatedTo => "related to",
        }
    }
}

/// Where a claim comes from. Points into the rest of the kit's corpus.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum SourceKind {
    Markdown,
    Url,
    Archive,
    Transcript,
    Document,
    Note,
}

impl SourceKind {
    pub const ALL: [SourceKind; 6] = [
        Self::Markdown,
        Self::Url,
        Self::Archive,
        Self::Transcript,
        Self::Document,
        Self::Note,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Markdown => "Markdown ref",
            Self::Url => "URL",
            Self::Archive => "Archive",
            Self::Transcript => "Transcript",
            Self::Document => "Document",
            Self::Note => "Note",
        }
    }

    /// Placeholder shown in the reference field.
    pub fn hint(self) -> &'static str {
        match self {
            Self::Markdown => "notes.md#section",
            Self::Url => "https://…",
            Self::Archive => "snapshot id / path",
            Self::Transcript => "mm:ss",
            Self::Document => "file · page",
            Self::Note => "free text",
        }
    }
}

/// One piece of evidence backing a relation.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Source {
    pub kind: SourceKind,
    pub reference: String,
    #[serde(default)]
    pub excerpt: String,
}

/// Node payload. The slug is the case-wide stable handle: relations and
/// display positions reference it, and it's what an external editor (human or
/// AI) uses to point at an entity.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entity {
    pub slug: String,
    pub kind: EntityKind,
    pub label: String,
    pub attrs: Vec<(String, String)>,
    pub note: String,
}

/// Edge payload — carries the sources. `is_sourced() == false` ⇒ unproven.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Relation {
    pub kind: RelationKind,
    pub sources: Vec<Source>,
}

impl Relation {
    pub fn new(kind: RelationKind) -> Self {
        Self {
            kind,
            sources: Vec::new(),
        }
    }

    pub fn is_sourced(&self) -> bool {
        !self.sources.is_empty()
    }
}

// ---- on-disk format ----
//
// One RON file, two structs: `data` is the investigation (what an AI edits —
// everything display-related is optional and the app heals it), `display` is
// machine-owned scenography (positions + camera) so a case opens looking the
// same every time. Editing contract for `data`: reference entities by slug;
// omit anything optional and the app fills it in on load.

#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct CaseFile {
    pub data: CaseData,
    #[serde(default)]
    pub display: CaseDisplay,
}

#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct CaseData {
    #[serde(default)]
    pub entities: Vec<EntityRec>,
    #[serde(default)]
    pub relations: Vec<RelationRec>,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct EntityRec {
    pub slug: String,
    pub kind: EntityKind,
    pub label: String,
    #[serde(default)]
    pub attrs: Vec<(String, String)>,
    #[serde(default)]
    pub note: String,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct RelationRec {
    pub from: String,
    pub to: String,
    pub kind: RelationKind,
    #[serde(default)]
    pub sources: Vec<Source>,
}

#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct CaseDisplay {
    #[serde(default)]
    pub camera: Camera,
    /// slug → (x, y). BTreeMap so saves serialize in sorted order → stable diffs.
    #[serde(default)]
    pub positions: BTreeMap<String, (f32, f32)>,
}

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct Camera {
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The AI-editing contract: a minimal hand-written file — no display, no
    /// sources, no attrs/note/excerpt — must load, with everything defaulted.
    #[test]
    fn minimal_external_edit_loads() {
        let s = r#"(
            data: (
                entities: [
                    (slug: "doe", kind: Person, label: "J. Doe"),
                    (slug: "shell", kind: Org, label: "Shell Co"),
                ],
                relations: [
                    (from: "doe", to: "shell", kind: Controls),
                ],
            ),
        )"#;
        let cf: CaseFile = ron::from_str(s).expect("minimal file must parse");
        assert_eq!(cf.data.entities.len(), 2);
        assert_eq!(cf.data.relations.len(), 1);
        assert!(cf.data.relations[0].sources.is_empty());
        assert!(cf.display.positions.is_empty());
        assert_eq!(cf.display.camera, Camera::default());
    }

    #[test]
    fn round_trip() {
        let mut cf = CaseFile::default();
        cf.data.entities.push(EntityRec {
            slug: "doe".into(),
            kind: EntityKind::Person,
            label: "J. Doe".into(),
            attrs: vec![("dob".into(), "1971-03-04".into())],
            note: "note".into(),
        });
        cf.data.relations.push(RelationRec {
            from: "doe".into(),
            to: "doe".into(),
            kind: RelationKind::AliasOf,
            sources: vec![Source {
                kind: SourceKind::Markdown,
                reference: "dossier.md#x".into(),
                excerpt: "quote".into(),
            }],
        });
        cf.display.positions.insert("doe".into(), (12.5, -40.0));
        cf.display.camera = Camera {
            x: 3.0,
            y: -7.5,
            zoom: 1.25,
        };

        let s = ron::ser::to_string_pretty(&cf, ron::ser::PrettyConfig::default()).unwrap();
        let back: CaseFile = ron::from_str(&s).unwrap();
        assert_eq!(cf, back);
    }
}

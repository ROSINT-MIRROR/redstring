//! The on-disk case: one RON document, two top-level blocks. `data` is the
//! investigation itself — what an external editor, human or AI, writes.
//! `display` is machine-owned scenography (positions, camera, node sizes) so
//! a case opens looking identical every time. Editing contract for `data`:
//! reference entities by slug, omit anything optional — the app heals missing
//! display state on load and writes it back on save.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::entity::{DEFAULT_NODE_SIZE, EntityKind};
use super::relation::RelationKind;
use super::source::Source;

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
    #[serde(default)]
    pub image: Option<String>,
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
    /// slug → (x, y). BTreeMap keeps saves in sorted order — stable diffs.
    #[serde(default)]
    pub positions: BTreeMap<String, (f32, f32)>,
    /// slug → node scale multiplier. Missing means the default size.
    #[serde(default)]
    pub sizes: BTreeMap<String, f32>,
}

impl CaseDisplay {
    /// Node scale for `slug`, defaulting when the file doesn't pin one.
    pub fn size_of(&self, slug: &str) -> f32 {
        let Some(s) = self.sizes.get(slug) else {
            return DEFAULT_NODE_SIZE;
        };
        *s
    }
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
    use crate::model::SourceKind;

    /// The AI-editing contract: a minimal hand-written file — no display, no
    /// sources, no attrs/note/excerpt — must load, with everything defaulted.
    #[test]
    fn minimal_external_edit_loads() {
        let s = r#"(
            data: (
                entities: [
                    (slug: "doe", kind: Person, label: "J. Doe"),
                    (slug: "shell", kind: ShellCompany, label: "Shell Co"),
                ],
                relations: [
                    (from: "doe", to: "shell", kind: Controls),
                ],
            ),
        )"#;
        let cf: CaseFile = ron::from_str(s).expect("minimal file must parse");
        assert_eq!(cf.data.entities.len(), 2);
        assert_eq!(cf.data.relations.len(), 1);
        assert!(cf.data.entities[0].image.is_none());
        assert!(cf.data.relations[0].sources.is_empty());
        assert!(cf.display.positions.is_empty());
        assert_eq!(cf.display.camera, Camera::default());
        assert_eq!(cf.display.size_of("doe"), DEFAULT_NODE_SIZE);
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
            image: Some("mugshots/doe.png".into()),
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
        cf.display.sizes.insert("doe".into(), 3.5);
        cf.display.camera = Camera {
            x: 3.0,
            y: -7.5,
            zoom: 1.25,
        };

        let s = ron::ser::to_string_pretty(&cf, ron::ser::PrettyConfig::default()).unwrap();
        let back: CaseFile = ron::from_str(&s).unwrap();
        assert_eq!(cf, back);
        assert_eq!(back.display.size_of("doe"), 3.5);
    }
}

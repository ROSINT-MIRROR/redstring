//! The on-disk case: one RON document with three top-level blocks. `meta`
//! names the case and carries the format version plus the full kind
//! vocabulary, so an external editor — human or AI — knows what it may write
//! without out-of-band docs. `data` is the investigation itself. `display`
//! is machine-owned scenography (positions, camera, node sizes) so a case
//! opens looking identical every time. Editing contract for `data`:
//! reference entities by slug, omit anything optional — the app heals missing
//! display state on load and writes it back on save. Entity notes live as
//! markdown files at entities/<slug>/note.md, not in this document.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::entity::{DEFAULT_NODE_SIZE, EntityKind};
use super::relation::RelationKind;
use super::source::Source;

/// Bumped only on breaking shape changes; additive fields don't count. The
/// loader refuses anything newer than it knows.
pub const FORMAT_VERSION: u32 = 1;

/// Case names are clamped to this many characters.
pub const MAX_CASE_NAME: usize = 120;

#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct CaseFile {
    #[serde(default)]
    pub meta: CaseMeta,
    pub data: CaseData,
    #[serde(default)]
    pub display: CaseDisplay,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct CaseMeta {
    pub name: String,
    pub format: u32,
    /// The redstring that wrote the file — informational only.
    pub app_version: String,
    /// The legal vocabulary, rewritten on every save. An AI editing the file
    /// reads kinds from here instead of guessing.
    pub ontology: Ontology,
}

impl Default for CaseMeta {
    fn default() -> Self {
        Self {
            name: "untitled case".to_owned(),
            format: FORMAT_VERSION,
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            ontology: Ontology::default(),
        }
    }
}

impl CaseMeta {
    /// Fresh meta for a save: clamped name, current format + app version,
    /// the full ontology.
    pub fn stamped(name: &str) -> Self {
        let clipped: String = name.trim().chars().take(MAX_CASE_NAME).collect();
        Self {
            name: if clipped.is_empty() {
                "untitled case".to_owned()
            } else {
                clipped
            },
            format: FORMAT_VERSION,
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            ontology: Ontology::current(),
        }
    }
}

#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Ontology {
    pub entity_kinds: Vec<String>,
    pub relation_kinds: Vec<String>,
}

impl Ontology {
    /// The vocabulary as it exists in this build — variant names exactly as
    /// they appear in the RON.
    pub fn current() -> Self {
        Self {
            entity_kinds: EntityKind::ALL.iter().map(|k| format!("{k:?}")).collect(),
            relation_kinds: RelationKind::ALL.iter().map(|k| format!("{k:?}")).collect(),
        }
    }
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
    /// Project-relative path to the node image, e.g. entities/doe/portrait.png.
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

    /// The AI-editing contract: a minimal hand-written file — no meta, no
    /// display, no sources, no attrs/excerpt — must load with everything
    /// defaulted, including a current format stamp.
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
        assert_eq!(cf.meta.format, FORMAT_VERSION);
    }

    /// Stamped meta carries the whole vocabulary and clamps silly names.
    #[test]
    fn stamped_meta_has_ontology_and_clamped_name() {
        let m = CaseMeta::stamped("Asfalt fantomă");
        assert_eq!(m.name, "Asfalt fantomă");
        assert_eq!(m.format, FORMAT_VERSION);
        assert_eq!(m.ontology.entity_kinds.len(), EntityKind::ALL.len());
        assert_eq!(m.ontology.relation_kinds.len(), RelationKind::ALL.len());
        assert!(m.ontology.entity_kinds.contains(&"ShellCompany".to_owned()));
        assert!(m.ontology.relation_kinds.contains(&"LaunderedThrough".to_owned()));

        let long = "x".repeat(500);
        assert_eq!(CaseMeta::stamped(&long).name.chars().count(), MAX_CASE_NAME);
        assert_eq!(CaseMeta::stamped("   ").name, "untitled case");
    }

    #[test]
    fn round_trip() {
        let mut cf = CaseFile::default();
        cf.meta = CaseMeta::stamped("trip");
        cf.data.entities.push(EntityRec {
            slug: "doe".into(),
            kind: EntityKind::Person,
            label: "J. Doe".into(),
            attrs: vec![("dob".into(), "1971-03-04".into())],
            image: Some("entities/doe/portrait.png".into()),
        });
        cf.data.relations.push(RelationRec {
            from: "doe".into(),
            to: "doe".into(),
            kind: RelationKind::AliasOf,
            sources: vec![Source {
                kind: SourceKind::Markdown,
                reference: "notes/dossier.md#x".into(),
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

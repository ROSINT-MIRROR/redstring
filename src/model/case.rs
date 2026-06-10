









use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::Error;

use super::entity::{EntityKind, KIND_META};
use super::relation::{RELATION_META, RelationKind};
use super::source::Source;



pub const FORMAT_VERSION: u32 = 1;


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
    
    pub app_version: String,
    
    
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
    
    
    pub fn stamped(name: &str) -> Result<Self, Error> {
        let clipped: String = name.trim().chars().take(MAX_CASE_NAME).collect();
        Ok(Self {
            name: if clipped.is_empty() {
                "untitled case".to_owned()
            } else {
                clipped
            },
            format: FORMAT_VERSION,
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            ontology: current_ontology(),
        })
    }
}

#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Ontology {
    pub entity_kinds: Vec<String>,
    pub relation_kinds: Vec<String>,
}

fn current_ontology() -> Ontology {
    Ontology {
        entity_kinds: KIND_META.iter().map(|m| format!("{:?}", m.kind)).collect(),
        relation_kinds: RELATION_META.iter().map(|m| format!("{:?}", m.kind)).collect(),
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
    
    #[serde(default)]
    pub positions: BTreeMap<String, (f32, f32)>,
    
    #[serde(default)]
    pub sizes: BTreeMap<String, f32>,
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
        assert!(cf.display.sizes.is_empty());
        assert_eq!(cf.meta.format, FORMAT_VERSION);
    }

    
    #[test]
    fn stamped_meta_has_ontology_and_clamped_name() {
        let m = CaseMeta::stamped("Asfalt fantomă").unwrap();
        assert_eq!(m.name, "Asfalt fantomă");
        assert_eq!(m.format, FORMAT_VERSION);
        assert_eq!(m.ontology.entity_kinds.len(), KIND_META.len());
        assert_eq!(m.ontology.relation_kinds.len(), RELATION_META.len());
        assert!(m.ontology.entity_kinds.contains(&"ShellCompany".to_owned()));
        assert!(m.ontology.relation_kinds.contains(&"LaunderedThrough".to_owned()));

        let long = "x".repeat(500);
        assert_eq!(CaseMeta::stamped(&long).unwrap().name.chars().count(), MAX_CASE_NAME);
        assert_eq!(CaseMeta::stamped("   ").unwrap().name, "untitled case");
    }

    #[test]
    fn round_trip() {
        let mut cf = CaseFile::default();
        cf.meta = CaseMeta::stamped("trip").unwrap();
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
        assert_eq!(back.display.sizes.get("doe").copied(), Some(3.5));
    }
}

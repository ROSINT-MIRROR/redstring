









use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::Error;

use super::entity::{EntityKind, KIND_META};
use super::facet::{Facet, FacetValue};
use super::observation::{Author, DEFAULT_CONFIDENCE, DateVal, Observation, Origin, Verification};
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
    pub value: String,

    #[serde(default, skip_serializing_if = "sealed_is_false")]
    pub sealed: bool,
    #[serde(default)]
    pub observations: Vec<Observation>,

    #[serde(default, skip_serializing_if = "facets_empty")]
    pub facets: BTreeMap<Facet, FacetValue>,

    #[serde(default)]
    pub image: Option<String>,
}

fn facets_empty(facets: &BTreeMap<Facet, FacetValue>) -> bool {
    facets.is_empty()
}

fn sealed_is_false(sealed: &bool) -> bool {
    !*sealed
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct RelationRec {
    pub from: String,
    pub to: String,
    pub kind: RelationKind,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_from: Option<DateVal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_to: Option<DateVal>,
    #[serde(default, skip_serializing_if = "sealed_is_false")]
    pub sealed: bool,
}

fn full_confidence() -> u8 {
    DEFAULT_CONFIDENCE
}

#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct CaseDisplay {
    #[serde(default)]
    pub camera: Camera,
    
    #[serde(default)]
    pub positions: BTreeMap<String, (f32, f32)>,
    
    #[serde(default)]
    pub sizes: BTreeMap<String, f32>,

    #[serde(default)]
    pub size_by_degree: bool,

    #[serde(default)]
    pub collapsed: std::collections::BTreeSet<String>,
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
    use crate::model::{Facet, FacetValue, OccupationIcon, Role, SourceKind};

    
    
    
    #[test]
    fn minimal_external_edit_loads() {
        let s = r#"(
            data: (
                entities: [
                    (slug: "doe", kind: Person, label: "J. Doe"),
                    (slug: "shell", kind: Organization, label: "Shell Co"),
                ],
                relations: [
                    (from: "doe", to: "shell", kind: Controls),
                ],
            ),
        )"#;
        let cf: CaseFile = ron::from_str(s).expect("minimal file must parse");
        assert_eq!(cf.data.entities.len(), 2);
        assert_eq!(cf.data.relations.len(), 1);
        assert!(!cf.data.entities[0].sealed, "sealed omitted → false (existing files stay valid)");
        assert!(cf.data.entities[0].facets.is_empty(), "facets omitted → empty");
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
        assert!(m.ontology.entity_kinds.contains(&"Court".to_owned()));
        assert!(m.ontology.entity_kinds.contains(&"Bank".to_owned()));
        for folded in ["CriminalOrg", "LawFirm", "MediaOutlet", "PoliticalParty"] {
            assert!(
                !m.ontology.entity_kinds.contains(&folded.to_owned()),
                "{folded} folded into Organization+OrgType — it must not appear in the ontology"
            );
        }
        assert!(m.ontology.relation_kinds.contains(&"LaunderedThrough".to_owned()));
        for kind in ["WebPage", "Endpoint", "Parameter", "Technology", "TlsCertificate", "DnsRecord", "Asn", "TrackerId"] {
            assert!(m.ontology.entity_kinds.contains(&kind.to_owned()), "ontology missing {kind}");
        }
        for rel in ["Serves", "HasEndpoint", "PresentsCert", "InAsn", "UsesTracker", "Leaks"] {
            assert!(m.ontology.relation_kinds.contains(&rel.to_owned()), "ontology missing {rel}");
        }

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
            value: "John Doe".into(),
            sealed: true,
            observations: vec![
                crate::model::Observation::stamp(
                    "dob".into(),
                    crate::model::ObsValue::Text("1971-03-04".into()),
                    Origin::Manual,
                    Verification::AnalystAsserted,
                    Author::default(),
                )
                .unwrap(),
            ],
            facets: {
                let mut m = BTreeMap::new();
                m.insert(Facet::Role, FacetValue::Indexed(Role::Suspect as u16));
                m.insert(
                    Facet::Occupation,
                    FacetValue::Freeform(crate::model::Occupation {
                        label: "Judge".into(),
                        icon: OccupationIcon::Scales,
                    }),
                );
                m
            },
            image: Some("entities/doe/portrait.png".into()),
        });
        cf.data.relations.push(RelationRec {
            from: "doe".into(),
            to: "doe".into(),
            kind: RelationKind::AliasOf,
            origin: Origin::Manual,
            status: Verification::AnalystAsserted,
            confidence: 100,
            author: Author::default(),
            created_at: 0,
            sources: vec![Source {
                kind: SourceKind::Markdown,
                reference: "notes/dossier.md#x".into(),
                excerpt: "quote".into(),
                sha256: Some("a".repeat(64)),
                fetched_at: Some(1_700_000_000),
                source_org: Some("DOJ".into()),
                archive_url: Some("https://web.archive.org/web/y".into()),
                ots: Some("evidence/y.ots".into()),
                tier: crate::model::SourceTier::Primary,
                sealed: true,
            }],
            valid_from: Some(DateVal {
                secs: 1_325_376_000,
                precision: crate::model::DatePrecision::Year,
            }),
            valid_to: Some(DateVal {
                secs: 1_356_998_400,
                precision: crate::model::DatePrecision::Year,
            }),
            sealed: true,
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
        assert!(back.data.entities[0].sealed, "a sealed entity must round-trip through save→load");
        assert!(
            back.data.relations[0].sealed,
            "a sealed relation must round-trip through save→load"
        );
        assert!(
            back.data.relations[0].sources[0].sealed,
            "a sealed source must round-trip through save→load"
        );
        assert_eq!(
            back.data.entities[0].facets.get(&Facet::Role),
            Some(&FacetValue::Indexed(Role::Suspect as u16)),
            "role facet must round-trip"
        );
        assert_eq!(
            back.data.entities[0].facets.get(&Facet::Occupation),
            Some(&FacetValue::Freeform(crate::model::Occupation {
                label: "Judge".into(),
                icon: OccupationIcon::Scales,
            })),
            "occupation facet must round-trip"
        );
    }

    #[test]
    fn unsealed_entity_head_stays_byte_identical() {
        let rec = EntityRec {
            slug: "doe".into(),
            kind: EntityKind::Person,
            label: "J. Doe".into(),
            value: String::new(),
            sealed: false,
            observations: vec![],
            facets: BTreeMap::new(),
            image: None,
        };
        let ron_s = ron::ser::to_string(&rec).unwrap();
        assert!(!ron_s.contains("sealed"), "an unsealed entity head must not gain a sealed key: {ron_s}");
        let mut sealed = rec.clone();
        sealed.sealed = true;
        let ron_s = ron::ser::to_string(&sealed).unwrap();
        assert!(ron_s.contains("sealed"), "a sealed entity head must carry the flag: {ron_s}");
    }

    #[test]
    fn unsealed_relation_line_stays_byte_identical() {
        let rec = RelationRec {
            from: "a".into(),
            to: "b".into(),
            kind: RelationKind::Controls,
            origin: Origin::Manual,
            status: Verification::Unreviewed,
            confidence: DEFAULT_CONFIDENCE,
            author: Author::default(),
            created_at: 0,
            sources: vec![],
            valid_from: None,
            valid_to: None,
            sealed: false,
        };
        let json = serde_json::to_string(&rec).unwrap();
        assert!(!json.contains("sealed"), "an unsealed relation line must not gain a sealed key: {json}");
        assert!(
            !json.contains("valid_from") && !json.contains("valid_to"),
            "a relation with no validity window must not bloat the line: {json}"
        );
        let mut sealed = rec.clone();
        sealed.sealed = true;
        let json = serde_json::to_string(&sealed).unwrap();
        assert!(json.contains("sealed"), "a sealed relation line must carry the flag: {json}");
        let back: RelationRec = serde_json::from_str(&json).unwrap();
        assert!(back.sealed, "a sealed relation must survive save→load");
    }

    #[test]
    fn temporal_validity_round_trips_through_the_relation_line_byte_stable() {
        let rec = RelationRec {
            from: "colonel".into(),
            to: "base".into(),
            kind: RelationKind::Manages,
            origin: Origin::Manual,
            status: Verification::AnalystAsserted,
            confidence: DEFAULT_CONFIDENCE,
            author: Author::default(),
            created_at: 0,
            sources: vec![],
            valid_from: Some(DateVal {
                secs: 1_325_376_000,
                precision: crate::model::DatePrecision::Year,
            }),
            valid_to: Some(DateVal {
                secs: 1_356_998_400,
                precision: crate::model::DatePrecision::Year,
            }),
            sealed: false,
        };
        let json = serde_json::to_string(&rec).unwrap();
        assert!(json.contains("valid_from"), "a windowed relation carries valid_from: {json}");
        assert!(json.contains("valid_to"), "a windowed relation carries valid_to: {json}");
        let back: RelationRec = serde_json::from_str(&json).unwrap();
        assert_eq!(back, rec, "the validity window must survive an ndjson round-trip");
        assert_eq!(
            serde_json::to_string(&back).unwrap(),
            json,
            "re-serializing the reloaded relation is byte-identical"
        );

        let open = RelationRec {
            valid_to: None,
            ..rec.clone()
        };
        let json = serde_json::to_string(&open).unwrap();
        assert!(
            json.contains("valid_from") && !json.contains("valid_to"),
            "an open-ended window keeps the known bound and omits the absent one: {json}"
        );
        let back: RelationRec = serde_json::from_str(&json).unwrap();
        assert_eq!(back, open, "an open-ended window must survive an ndjson round-trip");
    }
}

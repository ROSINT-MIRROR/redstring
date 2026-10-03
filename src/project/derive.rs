




use std::collections::{BTreeMap, BTreeSet};

use crate::error::Error;
use crate::model::{CaseFile, Footing, KIND_META, RELATION_META};

use super::layout::Project;

pub fn write_json(project: &Project, cf: &CaseFile) -> Result<(), Error> {
    let path = project.root.join("case.json");
    let public = redact(cf)?;
    let text = serde_json::to_string_pretty(&public).map_err(|source| Error::JsonSer { source })?;
    std::fs::write(&path, text).map_err(|source| Error::Io { path: path.to_path_buf(), source })?;
    Ok(())
}

pub fn write_graph_md(project: &Project, cf: &CaseFile) -> Result<(), Error> {
    let path = project.root.join("graph.md");
    let public = redact(cf)?;
    std::fs::write(&path, graph_md(&public))
        .map_err(|source| Error::Io { path: path.to_path_buf(), source })?;
    Ok(())
}

pub const SEALED_LABEL: &str = "[sealed]";
pub const SEALED_ALIAS_PREFIX: &str = "sealed_";
const ALIAS_BYTES: usize = 8;

pub struct AliasBook {
    map: BTreeMap<String, String>,
}

impl AliasBook {
    pub fn new() -> Result<Self, Error> {
        Ok(Self { map: BTreeMap::new() })
    }

    pub fn alias(&mut self, slug: &str) -> Result<String, Error> {
        let Some(existing) = self.map.get(slug) else {
            let fresh = random_alias()?;
            self.map.insert(slug.to_owned(), fresh.clone());
            return Ok(fresh);
        };
        Ok(existing.clone())
    }
}

fn random_alias() -> Result<String, Error> {
    let mut raw = [0u8; ALIAS_BYTES];
    getrandom::fill(&mut raw).map_err(|source| Error::Random { source })?;
    let mut out = String::from(SEALED_ALIAS_PREFIX);
    for byte in raw {
        out.push(hex_digit(byte >> 4)?);
        out.push(hex_digit(byte & 0x0f)?);
    }
    Ok(out)
}

fn hex_digit(nibble: u8) -> Result<char, Error> {
    let digits = b"0123456789abcdef";
    let Some(found) = digits.get(usize::from(nibble)) else {
        return Err(Error::Render { what: "hex nibble out of range".to_owned() });
    };
    Ok(char::from(*found))
}

pub fn redact(cf: &CaseFile) -> Result<CaseFile, Error> {
    let mut book = AliasBook::new()?;
    redact_with(cf, &mut book)
}

pub fn redact_with(cf: &CaseFile, book: &mut AliasBook) -> Result<CaseFile, Error> {
    let mut out = cf.clone();
    let mut remap: BTreeMap<String, String> = BTreeMap::new();
    let mut sealed_slugs: BTreeSet<String> = BTreeSet::new();
    for e in &mut out.data.entities {
        if e.sealed {
            sealed_slugs.insert(e.slug.clone());
            let alias = book.alias(&e.slug)?;
            remap.insert(e.slug.clone(), alias.clone());
            e.slug = alias;
            e.label = SEALED_LABEL.to_owned();
            e.value = String::new();
            e.image = None;
            e.facets.clear();
            e.observations.clear();
            e.sealed = false;
        } else {
            e.observations.retain(|o| !o.sealed);
            for o in &mut e.observations {
                o.sources.retain(|s| !s.sealed);
            }
        }
    }
    out.data
        .relations
        .retain(|r| !r.sealed && !sealed_slugs.contains(&r.from) && !sealed_slugs.contains(&r.to));
    for r in &mut out.data.relations {
        r.from = resolve(&remap, &r.from);
        r.to = resolve(&remap, &r.to);
        r.sources.retain(|s| !s.sealed);
    }
    out.display.positions = remapped_map(&out.display.positions, &remap);
    out.display.sizes = remapped_map(&out.display.sizes, &remap);
    out.display.collapsed = remapped_set(&out.display.collapsed, &remap);
    Ok(out)
}

pub fn sealed_slugs(cf: &CaseFile) -> Result<BTreeSet<String>, Error> {
    Ok(cf
        .data
        .entities
        .iter()
        .filter(|e| e.sealed)
        .map(|e| e.slug.clone())
        .collect())
}

fn resolve(remap: &BTreeMap<String, String>, slug: &str) -> String {
    let Some(alias) = remap.get(slug) else {
        return slug.to_owned();
    };
    alias.clone()
}

fn remapped_map<V: Clone>(
    src: &BTreeMap<String, V>,
    remap: &BTreeMap<String, String>,
) -> BTreeMap<String, V> {
    src.iter().map(|(k, v)| (resolve(remap, k), v.clone())).collect()
}

fn remapped_set(src: &BTreeSet<String>, remap: &BTreeMap<String, String>) -> BTreeSet<String> {
    src.iter().map(|k| resolve(remap, k)).collect()
}





fn graph_md(cf: &CaseFile) -> String {
    let mut out = String::new();
    out.push_str(&format!("# {}\n\n```mermaid\ngraph LR\n", label_safe(&cf.meta.name)));
    for e in &cf.data.entities {
        out.push_str(&format!(
            "    {}[\"{} {}\"]\n",
            e.slug,
            KIND_META[e.kind as usize].icon,
            label_safe(&e.label)
        ));
    }
    for r in &cf.data.relations {
        let arrow = if r.sources.is_empty() {
            "-.->"
        } else if Footing::STRONG.contains(&Footing::from(r)) {
            "==>"
        } else {
            "-->"
        };
        let warn = if r.sources.is_empty() { " ⚠" } else { "" };
        out.push_str(&format!(
            "    {} {arrow}|{}{warn}| {}\n",
            r.from,
            RELATION_META[r.kind as usize].label,
            r.to
        ));
    }
    out.push_str("```\n\nRegenerated by redstring on every save — do not edit.\n");
    out
}


fn label_safe(s: &str) -> String {
    s.replace(['"', '`', '|', '[', ']', '{', '}', '(', ')'], "'")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        Author, CaseMeta, EntityKind, EntityRec, ObsValue, Observation, Origin, RelationKind,
        RelationRec, Source, SourceKind, Verification,
    };

    fn obs(name: &str, value: &str) -> Observation {
        Observation::stamp(
            name.to_owned(),
            ObsValue::Text(value.to_owned()),
            Origin::Manual,
            Verification::AnalystAsserted,
            Author::default(),
        )
        .unwrap()
    }

    fn ent(slug: &str, kind: EntityKind, label: &str, value: &str, sealed: bool) -> EntityRec {
        EntityRec {
            slug: slug.to_owned(),
            kind,
            label: label.to_owned(),
            value: value.to_owned(),
            sealed,
            observations: vec![],
            facets: Default::default(),
            image: None,
        }
    }

    fn alias_in(public: &CaseFile) -> String {
        for e in &public.data.entities {
            if e.slug.starts_with(SEALED_ALIAS_PREFIX) {
                return e.slug.clone();
            }
        }
        String::new()
    }

    fn rel(from: &str, to: &str, kind: RelationKind) -> RelationRec {
        RelationRec {
            from: from.to_owned(),
            to: to.to_owned(),
            kind,
            origin: Origin::Manual,
            status: Verification::AnalystAsserted,
            confidence: 100,
            author: Author::default(),
            created_at: 0,
            sources: vec![],
            valid_from: None,
            valid_to: None,
            sealed: false,
        }
    }

    #[test]
    fn mermaid_shape_and_escaping() {
        let mut cf = CaseFile::default();
        cf.meta = CaseMeta::stamped("asfalt").unwrap();
        cf.data.entities.push(EntityRec {
            slug: "radu".into(),
            kind: EntityKind::Person,
            label: "Radu \"the Brick\" P.".into(),
            value: String::new(),
            sealed: false,
            observations: vec![],
            facets: Default::default(),
            image: None,
        });
        cf.data.entities.push(EntityRec {
            slug: "strada_sigur".into(),
            kind: EntityKind::Organization,
            label: "Strada Sigur SRL".into(),
            value: String::new(),
            sealed: false,
            observations: vec![],
            facets: Default::default(),
            image: None,
        });
        cf.data.relations.push(RelationRec {
            from: "radu".into(),
            to: "strada_sigur".into(),
            kind: RelationKind::Owns,
            origin: Origin::Manual,
            status: Verification::AnalystAsserted,
            confidence: 100,
            author: Author::default(),
            created_at: 0,
            sources: vec![Source {
                kind: SourceKind::Document,
                reference: "evidence/docs/onrc.pdf".into(),
                excerpt: String::new(),
                ..Default::default()
            }],
            valid_from: None,
            valid_to: None,
            sealed: false,
        });
        cf.data.relations.push(RelationRec {
            from: "strada_sigur".into(),
            to: "radu".into(),
            kind: RelationKind::Paid,
            origin: Origin::Manual,
            status: Verification::Unreviewed,
            confidence: 100,
            author: Author::default(),
            created_at: 0,
            sources: vec![],
            valid_from: None,
            valid_to: None,
            sealed: false,
        });

        let md = graph_md(&cf);
        assert!(md.contains("graph LR"), "{md}");
        
        assert!(md.contains("Radu 'the Brick' P."), "{md}");
        
        assert!(md.contains("radu -->|owns| strada_sigur"), "{md}");
        assert!(md.contains("strada_sigur -.->|paid ⚠| radu"), "{md}");
    }

    #[test]
    fn sealed_entity_redacted_from_all_derived_outputs() {
        let mut cf = CaseFile::default();
        cf.meta = CaseMeta::stamped("protected").unwrap();

        let mut victim = ent(
            "jane_victim",
            EntityKind::Person,
            "Jane Victim",
            "jane.victim@example.com",
            true,
        );
        victim.observations = vec![obs("dob", "2009-05-01")];
        victim.image = Some("entities/jane_victim/face.png".into());
        cf.data.entities.push(victim);
        cf.data
            .entities
            .push(ent("acme", EntityKind::Organization, "Acme Care Home", "", false));
        cf.data.relations.push(rel("acme", "jane_victim", RelationKind::Owns));
        cf.display.positions.insert("jane_victim".into(), (10.0, 20.0));

        let public = redact(&cf).unwrap();
        let md = graph_md(&public);
        let json = serde_json::to_string_pretty(&public).unwrap();
        let alias = alias_in(&public);

        for leak in [
            "Jane Victim",
            "jane.victim@example.com",
            "jane_victim",
            "2009-05-01",
            "face.png",
        ] {
            assert!(!md.contains(leak), "graph.md leaked sealed '{leak}':\n{md}");
            assert!(!json.contains(leak), "case.json leaked sealed '{leak}':\n{json}");
        }

        assert!(json.contains("[sealed]"), "case.json must show the placeholder: {json}");
        assert!(md.contains(&alias), "graph.md must carry the anonymized id: {md}");
        assert!(md.contains("Acme Care Home"), "an unsealed node must survive: {md}");

        let redacted = public
            .data
            .entities
            .iter()
            .find(|e| e.slug == alias)
            .expect("the sealed entity keeps a stable anonymized slug");
        assert_eq!(redacted.label, SEALED_LABEL);
        assert!(redacted.value.is_empty());
        assert!(redacted.observations.is_empty(), "sealed obs must be omitted");
        assert!(redacted.image.is_none(), "the image path leaks the name — drop it");

        assert!(
            public.data.relations.is_empty(),
            "an edge touching the sealed entity is withheld — keeping it would re-link the anon node by triangulation: {:?}",
            public.data.relations
        );
        assert!(
            !md.contains("owns"),
            "the incident edge's verb must not survive in graph.md: {md}"
        );
        assert_eq!(
            public.display.positions.get(&alias).copied(),
            Some((10.0, 20.0)),
            "display keys must follow the alias, not leak the real slug"
        );
        assert!(
            public.display.positions.get("jane_victim").is_none(),
            "the real slug must not survive as a display key"
        );
    }

    #[test]
    fn unsealed_derive_output_is_byte_identical() {
        let mut cf = CaseFile::default();
        cf.meta = CaseMeta::stamped("open case").unwrap();
        let mut d = ent("doe", EntityKind::Person, "J. Doe", "", false);
        d.observations = vec![obs("dob", "1971-03-04")];
        cf.data.entities.push(d);
        cf.data
            .entities
            .push(ent("shell", EntityKind::Organization, "Shell Co", "", false));
        cf.data.relations.push(rel("doe", "shell", RelationKind::Controls));
        cf.display.positions.insert("doe".into(), (1.0, 2.0));
        cf.display.sizes.insert("doe".into(), 3.0);
        cf.display.collapsed.insert("shell".into());

        let before_md = graph_md(&cf);
        let before_json = serde_json::to_string_pretty(&cf).unwrap();
        let public = redact(&cf).unwrap();

        assert_eq!(public, cf, "redact is a no-op when nothing is sealed");
        assert_eq!(graph_md(&public), before_md, "graph.md stays byte-identical");
        assert_eq!(
            serde_json::to_string_pretty(&public).unwrap(),
            before_json,
            "case.json stays byte-identical"
        );
    }

    #[test]
    fn sealed_observation_dropped_from_unsealed_entity() {
        let mut cf = CaseFile::default();
        cf.meta = CaseMeta::stamped("mixed").unwrap();
        let mut auditor = ent("auditor", EntityKind::Person, "The Auditor", "", false);
        let mut secret = obs("informant_name", "Deep Throat");
        secret.sealed = true;
        auditor.observations = vec![secret, obs("role_title", "Compliance Auditor")];
        cf.data.entities.push(auditor);

        let json = serde_json::to_string_pretty(&redact(&cf).unwrap()).unwrap();
        assert!(!json.contains("Deep Throat"), "sealed obs content must not leak: {json}");
        assert!(!json.contains("informant_name"), "sealed obs key must not leak: {json}");
        assert!(json.contains("Compliance Auditor"), "an unsealed obs survives: {json}");
    }

    #[test]
    fn sealed_source_dropped_from_relation() {
        let mut cf = CaseFile::default();
        cf.meta = CaseMeta::stamped("wire").unwrap();
        cf.data
            .entities
            .push(ent("a", EntityKind::Person, "A", "", false));
        cf.data
            .entities
            .push(ent("b", EntityKind::Person, "B", "", false));
        let mut edge = rel("a", "b", RelationKind::Controls);
        edge.sources = vec![
            Source {
                kind: SourceKind::Note,
                reference: "confidential-wire-tap".into(),
                excerpt: "the intercept transcript".into(),
                sealed: true,
                ..Default::default()
            },
            Source {
                kind: SourceKind::Url,
                reference: "https://public.example/filing".into(),
                ..Default::default()
            },
        ];
        cf.data.relations.push(edge);

        let public = redact(&cf).unwrap();
        let json = serde_json::to_string_pretty(&public).unwrap();
        assert!(!json.contains("confidential-wire-tap"), "sealed source ref must not leak: {json}");
        assert!(!json.contains("intercept transcript"), "sealed source excerpt must not leak: {json}");
        assert!(json.contains("public.example/filing"), "an unsealed source survives: {json}");
        assert_eq!(public.data.relations[0].sources.len(), 1, "only the sealed source is dropped");
    }

    #[test]
    fn orphanage_assault_edge_never_leaks_verb_or_pairing() {
        let mut cf = CaseFile::default();
        cf.meta = CaseMeta::stamped("orphanage").unwrap();
        cf.data
            .entities
            .push(ent("minor", EntityKind::Person, "A. Child", "child@example.org", true));
        cf.data
            .entities
            .push(ent("soldier", EntityKind::Person, "Sgt Marku", "", false));
        cf.data
            .relations
            .push(rel("minor", "soldier", RelationKind::SexuallyAssaulted));

        let public = redact(&cf).unwrap();
        let md = graph_md(&public);
        let json = serde_json::to_string_pretty(&public).unwrap();
        let rel_json = serde_json::to_string(&public.data.relations).unwrap();
        let alias = alias_in(&public);

        assert!(
            public.data.relations.is_empty(),
            "the assault edge touches a sealed minor — it must not survive: {:?}",
            public.data.relations
        );
        assert!(
            !md.contains("sexually assaulted"),
            "graph.md must not render the sensitive verb: {md}"
        );
        assert!(
            !rel_json.contains("SexuallyAssaulted"),
            "no surviving relation record may carry the sensitive verb: {rel_json}"
        );
        assert!(!md.contains("A. Child"), "the victim name must not leak: {md}");
        assert!(!json.contains("child@example.org"), "the victim value must not leak: {json}");
        assert!(md.contains("Sgt Marku"), "the named perpetrator node still ships: {md}");
        for line in md.lines() {
            assert!(
                !(line.contains(&alias) && line.contains("soldier")),
                "no line may re-link the anon victim to the named soldier: {line}"
            );
        }
    }

    #[test]
    fn explicitly_sealed_relation_dropped_between_named_entities() {
        let mut cf = CaseFile::default();
        cf.meta = CaseMeta::stamped("hush").unwrap();
        cf.data
            .entities
            .push(ent("acme", EntityKind::Organization, "Acme SRL", "", false));
        cf.data
            .entities
            .push(ent("mark", EntityKind::Person, "Mark", "", false));

        let mut hush = rel("acme", "mark", RelationKind::Paid);
        hush.sealed = true;
        hush.sources = vec![Source {
            kind: SourceKind::Note,
            reference: "off-books-ledger".into(),
            excerpt: "settlement kept quiet".into(),
            ..Default::default()
        }];
        cf.data.relations.push(hush);
        cf.data.relations.push(rel("mark", "acme", RelationKind::EmployeeOf));

        let public = redact(&cf).unwrap();
        let md = graph_md(&public);
        let json = serde_json::to_string_pretty(&public).unwrap();
        let rel_json = serde_json::to_string(&public.data.relations).unwrap();

        assert_eq!(public.data.relations.len(), 1, "only the explicitly-sealed edge is dropped");
        assert_eq!(public.data.relations[0].kind, RelationKind::EmployeeOf);
        assert!(!rel_json.contains("Paid"), "the sealed verb must not survive on any relation: {rel_json}");
        assert!(!md.contains("|paid"), "graph.md must not render the sealed edge: {md}");
        assert!(!json.contains("off-books-ledger"), "the sealed edge's source ref must not leak: {json}");
        assert!(!json.contains("settlement kept quiet"), "the sealed edge's excerpt must not leak: {json}");
    }
}

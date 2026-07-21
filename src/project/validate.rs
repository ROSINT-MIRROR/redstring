




use std::collections::HashSet;
use std::path::Path;

use crate::error::Error;
use crate::model::{CaseFile, Origin, RELATION_META, SOURCE_META};

use super::ignore::RsIgnore;
use super::io;
use super::layout::Project;

pub fn validate(project: &Project) -> Result<Vec<String>, Error> {
    let cf = io::read_case(project)?;
    let ignore = RsIgnore::load(&project.root)?;
    let mut findings: Vec<String> = Vec::new();

    let mut slugs: HashSet<&str> = HashSet::new();
    for e in &cf.data.entities {
        slugs.insert(&e.slug);
    }
    for r in &cf.data.relations {
        for end in [&r.from, &r.to] {
            if !slugs.contains(end.as_str()) {
                findings.push(format!(
                    "relation {} -> {} references unknown slug '{end}'",
                    r.from, r.to
                ));
            }
        }
    }

    for (what, rel) in cited_paths(&cf)? {
        check_path(project, &ignore, &mut findings, &what, &rel)?;
    }

    Ok(findings)
}

pub fn source_findings(project: &Project) -> Result<Vec<String>, Error> {
    let cf = io::read_case(project)?;
    let mut findings: Vec<String> = Vec::new();
    source_lints(&cf, &mut findings);
    Ok(findings)
}

fn source_lints(cf: &CaseFile, findings: &mut Vec<String>) {
    for r in &cf.data.relations {
        let verb = RELATION_META[r.kind as usize].label;
        let n = r.sources.len();
        if n == 0 {
            findings.push(format!(
                "edge {} --{verb}--> {} has no source (unproven assertion)",
                r.from, r.to
            ));
        } else if n == 1 {
            findings.push(format!(
                "edge {} --{verb}--> {} has a single source (needs corroboration)",
                r.from, r.to
            ));
        }
    }
    for e in &cf.data.entities {
        for o in &e.observations {
            if !o.sources.is_empty() {
                continue;
            }
            let claim = match &o.origin {
                Origin::Manual => "manual",
                Origin::Inference => "inferred",
                Origin::Lookup(_) | Origin::Import(_) => continue,
            };
            findings.push(format!(
                "observation '{}' on {} has no source (uncited {claim} claim)",
                o.name, e.slug
            ));
        }
    }
}





pub fn cited_paths(cf: &CaseFile) -> Result<Vec<(String, String)>, Error> {
    let mut v: Vec<(String, String)> = Vec::new();
    for e in &cf.data.entities {
        let Some(img) = &e.image else { continue };
        v.push((format!("image of '{}'", e.slug), img.clone()));
    }
    for r in &cf.data.relations {
        for s in &r.sources {
            if !SOURCE_META[s.kind as usize].file_backed {
                continue;
            }
            let Some(path_part) = s.reference.split('#').next() else {
                continue;
            };
            if !path_part.contains('/') || path_part.starts_with("http") {
                continue;
            }
            v.push((
                format!("source on {} -> {}", r.from, r.to),
                path_part.to_owned(),
            ));
        }
    }
    Ok(v)
}

fn check_path(
    project: &Project,
    ignore: &RsIgnore,
    findings: &mut Vec<String>,
    what: &str,
    rel: &str,
) -> Result<(), Error> {
    if Path::new(rel).is_absolute() {
        findings.push(format!(
            "{what}: absolute path '{rel}' — case files must cite project-relative paths"
        ));
        return Ok(());
    }
    if rel.split('/').any(|seg| seg == "..") {
        findings.push(format!("{what}: '{rel}' escapes the case directory"));
        return Ok(());
    }
    if !project.resolve(rel)?.exists() {
        findings.push(format!("{what}: missing file '{rel}'"));
        return Ok(());
    }
    if ignore.matches(rel)? {
        findings.push(format!(
            "{what}: '{rel}' is matched by .rsignore — it would vanish from bundles"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        Author, CaseMeta, EntityKind, EntityRec, ObsValue, Observation, Origin, RelationKind,
        RelationRec, Source, SourceKind, Verification,
    };

    fn project(name: &str) -> Project {
        let d = std::env::temp_dir().join(format!("redstring_val_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        Project::create(&d).unwrap()
    }

    fn rec(slug: &str) -> EntityRec {
        EntityRec {
            slug: slug.into(),
            kind: EntityKind::Person,
            label: slug.into(),
            value: String::new(),
            sealed: false,
            observations: vec![],
            facets: Default::default(),
            image: None,
        }
    }

    fn rel(from: &str, to: &str, sources: Vec<Source>) -> RelationRec {
        RelationRec {
            from: from.into(),
            to: to.into(),
            kind: RelationKind::Paid,
            origin: Origin::Manual,
            status: Verification::Unreviewed,
            confidence: 100,
            author: Author::default(),
            created_at: 0,
            sources,
            sealed: false,
        }
    }

    fn src() -> Source {
        Source {
            kind: SourceKind::Url,
            reference: "https://example.com/receipt".into(),
            excerpt: String::new(),
            ..Default::default()
        }
    }

    fn obs(name: &str, origin: Origin, sources: Vec<Source>) -> Observation {
        let mut o = Observation::stamp(
            name.into(),
            ObsValue::Text("x".into()),
            origin,
            Verification::AnalystAsserted,
            Author::default(),
        )
        .unwrap();
        o.sources = sources;
        o
    }

    fn full_check(p: &Project) -> Vec<String> {
        let mut findings = validate(p).unwrap();
        findings.extend(source_findings(p).unwrap());
        findings
    }

    #[test]
    fn findings_cover_the_contract() {
        let p = project("contract");
        let mut cf = CaseFile::default();
        cf.meta = CaseMeta::stamped("validate me").unwrap();
        cf.data.entities.push(rec("doe"));
        cf.data.entities.push(rec("doe")); 
        let mut shady = rec("shady");
        shady.image = Some("entities/shady/missing.png".into()); 
        cf.data.entities.push(shady);
        cf.data.relations.push(RelationRec {
            from: "doe".into(),
            to: "ghost".into(),
            kind: RelationKind::Paid,
            origin: Origin::Manual,
            status: Verification::Unreviewed,
            confidence: 100,
            author: Author::default(),
            created_at: 0,
            sources: vec![
                Source {
                    kind: SourceKind::Document,
                    reference: "evidence/docs/contract.pdf".into(), 
                    excerpt: String::new(),
                    ..Default::default()
                },
                Source {
                    kind: SourceKind::Transcript,
                    reference: "12:34".into(), 
                    excerpt: String::new(),
                    ..Default::default()
                },
                Source {
                    kind: SourceKind::Url,
                    reference: "https://example.com/article".into(),
                    excerpt: String::new(),
                    ..Default::default()
                },
            ],
            sealed: false,
        });
        io::write_case(&p, &cf).unwrap();

        let findings = validate(&p).unwrap();
        let joined = findings.join("\n");
        assert!(joined.contains("unknown slug 'ghost'"), "{joined}");
        assert!(joined.contains("missing.png"), "{joined}");
        assert!(joined.contains("contract.pdf"), "{joined}");
        assert_eq!(findings.len(), 3, "{joined}");
        std::fs::remove_dir_all(p.root).unwrap();
    }

    #[test]
    fn clean_case_validates_clean() {
        let p = project("clean");
        let img = p.resolve("entities/doe/portrait.png").unwrap();
        std::fs::create_dir_all(img.parent().unwrap()).unwrap();
        std::fs::write(&img, b"png").unwrap();
        let mut cf = CaseFile::default();
        cf.meta = CaseMeta::stamped("clean").unwrap();
        let mut doe = rec("doe");
        doe.image = Some("entities/doe/portrait.png".into());
        cf.data.entities.push(doe);
        io::write_case(&p, &cf).unwrap();
        assert!(validate(&p).unwrap().is_empty());
        std::fs::remove_dir_all(p.root).unwrap();
    }

    #[test]
    fn sourcing_lints_flag_unsourced_single_and_uncited_obs() {
        let p = project("sourcing");
        let mut cf = CaseFile::default();
        cf.meta = CaseMeta::stamped("sourcing").unwrap();
        let mut a = rec("a");
        a.observations = vec![
            obs("dob", Origin::Manual, vec![]),
            obs("alias", Origin::Inference, vec![]),
            obs("email", Origin::Lookup("holehe".into()), vec![]),
            obs("phone", Origin::Manual, vec![src()]),
        ];
        cf.data.entities.push(a);
        cf.data.entities.push(rec("b"));
        cf.data.entities.push(rec("c"));
        cf.data.entities.push(rec("d"));
        cf.data.relations.push(rel("a", "b", vec![]));
        cf.data.relations.push(rel("a", "c", vec![src()]));
        cf.data.relations.push(rel("a", "d", vec![src(), src()]));
        io::write_case(&p, &cf).unwrap();

        let findings = full_check(&p);
        let joined = findings.join("\n");
        assert!(
            joined.contains("edge a --paid--> b has no source (unproven assertion)"),
            "{joined}"
        );
        assert!(
            joined.contains("edge a --paid--> c has a single source (needs corroboration)"),
            "{joined}"
        );
        assert!(!joined.contains("--paid--> d"), "well-sourced edge must stay silent: {joined}");
        assert!(
            joined.contains("observation 'dob' on a has no source (uncited manual claim)"),
            "{joined}"
        );
        assert!(
            joined.contains("observation 'alias' on a has no source (uncited inferred claim)"),
            "{joined}"
        );
        assert!(!joined.contains("'email'"), "lookup-origin obs is auto-sourced: {joined}");
        assert!(!joined.contains("'phone'"), "a sourced obs must stay silent: {joined}");
        assert_eq!(findings.len(), 4, "exactly the four sourcing findings: {joined}");
        std::fs::remove_dir_all(p.root).unwrap();
    }

    #[test]
    fn fully_sourced_case_stays_clean() {
        let p = project("fullysourced");
        let mut cf = CaseFile::default();
        cf.meta = CaseMeta::stamped("fully sourced").unwrap();
        let mut a = rec("a");
        a.observations = vec![
            obs("dob", Origin::Manual, vec![src()]),
            obs("email", Origin::Lookup("holehe".into()), vec![]),
        ];
        cf.data.entities.push(a);
        cf.data.entities.push(rec("b"));
        cf.data.relations.push(rel("a", "b", vec![src(), src()]));
        io::write_case(&p, &cf).unwrap();

        assert!(
            full_check(&p).is_empty(),
            "a fully-sourced case must produce zero findings (exit 0)"
        );
        std::fs::remove_dir_all(p.root).unwrap();
    }
}

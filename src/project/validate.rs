//! The case validator: the contract an external editor — human or AI — can
//! check its work against mechanically. Hard failures (unreadable, unparsable,
//! future format) come back as errors; consistency problems come back as
//! findings, one line each, so an edit/check/fix loop needs no parsing smarts.

use std::collections::HashSet;
use std::path::Path;

use crate::error::RsResult;
use crate::model::CaseFile;

use super::ignore::RsIgnore;
use super::io;
use super::layout::Project;

pub fn validate(project: &Project) -> RsResult<Vec<String>> {
    let cf = io::read_case(project)?;
    let ignore = RsIgnore::load(project.root())?;
    let mut findings: Vec<String> = Vec::new();

    let mut slugs: HashSet<&str> = HashSet::new();
    for e in &cf.data.entities {
        if !slugs.insert(&e.slug) {
            findings.push(format!("duplicate slug '{}'", e.slug));
        }
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

    for (what, rel) in cited_paths(&cf) {
        check_path(project, &ignore, &mut findings, &what, &rel);
    }

    Ok(findings)
}

/// Every file the case cites, as (description, project-relative path).
/// Heuristic for sources: only file-backed kinds whose reference's path part
/// — before any `#fragment` — contains a `/`; bare timestamps and page refs
/// are not paths. Shared with bundle integrity so both agree on what counts.
pub fn cited_paths(cf: &CaseFile) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = Vec::new();
    for e in &cf.data.entities {
        let Some(img) = &e.image else { continue };
        v.push((format!("image of '{}'", e.slug), img.clone()));
    }
    for r in &cf.data.relations {
        for s in &r.sources {
            if !s.kind.is_file_backed() {
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
    v
}

fn check_path(
    project: &Project,
    ignore: &RsIgnore,
    findings: &mut Vec<String>,
    what: &str,
    rel: &str,
) {
    if Path::new(rel).is_absolute() {
        findings.push(format!(
            "{what}: absolute path '{rel}' — case files must cite project-relative paths"
        ));
        return;
    }
    if rel.split('/').any(|seg| seg == "..") {
        findings.push(format!("{what}: '{rel}' escapes the case directory"));
        return;
    }
    if !project.resolve(rel).exists() {
        findings.push(format!("{what}: missing file '{rel}'"));
        return;
    }
    if ignore.matches(rel) {
        findings.push(format!(
            "{what}: '{rel}' is matched by .rsignore — it would vanish from bundles"
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CaseMeta, EntityKind, EntityRec, RelationKind, RelationRec, Source, SourceKind};

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
            attrs: vec![],
            image: None,
        }
    }

    #[test]
    fn findings_cover_the_contract() {
        let p = project("contract");
        let mut cf = CaseFile::default();
        cf.meta = CaseMeta::stamped("validate me");
        cf.data.entities.push(rec("doe"));
        cf.data.entities.push(rec("doe")); // duplicate
        let mut shady = rec("shady");
        shady.image = Some("entities/shady/missing.png".into()); // missing file
        cf.data.entities.push(shady);
        cf.data.relations.push(RelationRec {
            from: "doe".into(),
            to: "ghost".into(), // unknown slug
            kind: RelationKind::Paid,
            sources: vec![
                Source {
                    kind: SourceKind::Document,
                    reference: "evidence/docs/contract.pdf".into(), // missing file
                    excerpt: String::new(),
                },
                Source {
                    kind: SourceKind::Transcript,
                    reference: "12:34".into(), // not a path — never flagged
                    excerpt: String::new(),
                },
                Source {
                    kind: SourceKind::Url,
                    reference: "https://example.org/x".into(), // not file-backed
                    excerpt: String::new(),
                },
            ],
        });
        io::write_case(&p, &cf).unwrap();

        let findings = validate(&p).unwrap();
        let joined = findings.join("\n");
        assert!(joined.contains("duplicate slug 'doe'"), "{joined}");
        assert!(joined.contains("unknown slug 'ghost'"), "{joined}");
        assert!(joined.contains("missing.png"), "{joined}");
        assert!(joined.contains("contract.pdf"), "{joined}");
        assert_eq!(findings.len(), 4, "{joined}");
        std::fs::remove_dir_all(p.root()).unwrap();
    }

    #[test]
    fn clean_case_validates_clean() {
        let p = project("clean");
        let img = p.resolve("entities/doe/portrait.png");
        std::fs::create_dir_all(img.parent().unwrap()).unwrap();
        std::fs::write(&img, b"png").unwrap();
        let mut cf = CaseFile::default();
        cf.meta = CaseMeta::stamped("clean");
        let mut doe = rec("doe");
        doe.image = Some("entities/doe/portrait.png".into());
        cf.data.entities.push(doe);
        io::write_case(&p, &cf).unwrap();
        assert!(validate(&p).unwrap().is_empty());
        std::fs::remove_dir_all(p.root()).unwrap();
    }
}

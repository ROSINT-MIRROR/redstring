




use std::collections::HashSet;
use std::path::Path;

use crate::error::Error;
use crate::model::{
    CaseFile, Facet, FacetValue, KIND_META, Origin, RELATION_META, ROLE_META, SOURCE_META,
    Verification, subtype_value,
};

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

    facet_lints(&cf, &mut findings)?;

    Ok(findings)
}

fn facet_lints(cf: &CaseFile, findings: &mut Vec<String>) -> Result<(), Error> {
    for e in &cf.data.entities {
        let meta = &KIND_META[e.kind as usize];
        for (facet, value) in &e.facets {
            if !meta.facets.contains(facet) {
                findings.push(format!(
                    "entity {}: facet {facet:?} is not declared by kind '{}'",
                    e.slug, meta.label
                ));
                continue;
            }
            let FacetValue::Indexed(n) = value else {
                continue;
            };
            if !facet_index_ok(*facet, *n)? {
                findings.push(format!("entity {}: facet {facet:?} index {n} out of range", e.slug));
            }
        }
    }
    Ok(())
}

fn facet_index_ok(facet: Facet, index: u16) -> Result<bool, Error> {
    match facet {
        Facet::Role => Ok((index as usize) < ROLE_META.len()),
        Facet::Occupation => Ok(false),
        Facet::OrgType
        | Facet::AccountType
        | Facet::DomainType
        | Facet::SiteType
        | Facet::ClaimType => Ok(subtype_value(facet, index)?.is_some()),
    }
}

pub struct SourcingSummary {
    pub unproven_edges: usize,
    pub single_source_edges: usize,
    pub uncited_obs: usize,
}

pub fn source_findings(project: &Project) -> Result<Vec<String>, Error> {
    Ok(source_report(project)?.0)
}

pub fn source_report(project: &Project) -> Result<(Vec<String>, SourcingSummary), Error> {
    let cf = io::read_case(project)?;
    let mut findings: Vec<String> = Vec::new();
    let summary = source_lints(&cf, &mut findings);
    Ok((findings, summary))
}

fn source_lints(cf: &CaseFile, findings: &mut Vec<String>) -> SourcingSummary {
    let mut summary = SourcingSummary {
        unproven_edges: 0,
        single_source_edges: 0,
        uncited_obs: 0,
    };
    for r in &cf.data.relations {
        if !r.sources.is_empty() {
            continue;
        }
        let verb = RELATION_META[r.kind as usize].label;
        summary.unproven_edges += 1;
        findings.push(format!(
            "UNPROVEN edge {} --{verb}--> {}: 0 sources — an unproven assertion, cite it or cut it",
            r.from, r.to
        ));
    }
    for r in &cf.data.relations {
        if r.sources.len() != 1 {
            continue;
        }
        let verb = RELATION_META[r.kind as usize].label;
        summary.single_source_edges += 1;
        findings.push(format!(
            "single-source edge {} --{verb}--> {}: 1 source — needs corroboration",
            r.from, r.to
        ));
    }
    for e in &cf.data.entities {
        for o in &e.observations {
            if !o.sources.is_empty() {
                continue;
            }
            if matches!(o.status, Verification::NoPublicRecord | Verification::DeniedByTarget) {
                continue;
            }
            let claim = match &o.origin {
                Origin::Manual => "manual",
                Origin::Inference => "inferred",
                Origin::Lookup(_) | Origin::Import(_) => continue,
            };
            summary.uncited_obs += 1;
            findings.push(format!(
                "observation '{}' on {} has no source (uncited {claim} claim)",
                o.name, e.slug
            ));
        }
    }
    summary
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
            valid_from: None,
            valid_to: None,
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
            valid_from: None,
            valid_to: None,
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
            joined.contains("UNPROVEN edge a --paid--> b: 0 sources"),
            "0-source edges must be marked UNPROVEN and stand out: {joined}"
        );
        assert!(
            joined.contains("single-source edge a --paid--> c: 1 source — needs corroboration"),
            "{joined}"
        );
        let unproven_at = joined.find("UNPROVEN edge a").expect("unproven finding present");
        let single_at = joined.find("single-source edge a").expect("single-source finding present");
        assert!(
            unproven_at < single_at,
            "unproven (0-source) findings must be listed before single-source ones: {joined}"
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

    fn obs_status(name: &str, origin: Origin, status: Verification) -> Observation {
        Observation::stamp(
            name.into(),
            ObsValue::Text("x".into()),
            origin,
            status,
            Author::default(),
        )
        .unwrap()
    }

    #[test]
    fn searched_but_absent_observations_are_not_flagged_uncited() {
        let p = project("absent");
        let mut cf = CaseFile::default();
        cf.meta = CaseMeta::stamped("absent").unwrap();
        let mut a = rec("a");
        a.observations = vec![
            obs_status("criminal_record", Origin::Manual, Verification::NoPublicRecord),
            obs_status("denied_alias", Origin::Manual, Verification::DeniedByTarget),
            obs_status("dob", Origin::Manual, Verification::AnalystAsserted),
        ];
        cf.data.entities.push(a);
        io::write_case(&p, &cf).unwrap();

        let findings = source_findings(&p).unwrap();
        let joined = findings.join("\n");
        assert!(
            !joined.contains("'criminal_record'"),
            "a NoPublicRecord finding has no positive source to cite by definition: {joined}"
        );
        assert!(
            !joined.contains("'denied_alias'"),
            "a DeniedByTarget claim must not be flagged uncited: {joined}"
        );
        assert!(
            joined.contains("observation 'dob' on a has no source (uncited manual claim)"),
            "a genuinely uncited manual claim is still flagged: {joined}"
        );
        assert_eq!(findings.len(), 1, "only the real uncited obs is flagged: {joined}");
        std::fs::remove_dir_all(p.root).unwrap();
    }

    #[test]
    fn source_report_counts_each_tier() {
        let p = project("report");
        let mut cf = CaseFile::default();
        cf.meta = CaseMeta::stamped("report").unwrap();
        let mut a = rec("a");
        a.observations = vec![obs("dob", Origin::Manual, vec![])];
        cf.data.entities.push(a);
        cf.data.entities.push(rec("b"));
        cf.data.entities.push(rec("c"));
        cf.data.entities.push(rec("d"));
        cf.data.relations.push(rel("a", "b", vec![]));
        cf.data.relations.push(rel("a", "c", vec![]));
        cf.data.relations.push(rel("a", "d", vec![src()]));
        io::write_case(&p, &cf).unwrap();

        let (lines, summary) = source_report(&p).unwrap();
        assert_eq!(summary.unproven_edges, 2, "two 0-source edges");
        assert_eq!(summary.single_source_edges, 1, "one single-source edge");
        assert_eq!(summary.uncited_obs, 1, "one uncited manual observation");
        assert_eq!(lines.len(), 4, "one line per finding across all tiers");
        std::fs::remove_dir_all(p.root).unwrap();
    }

    #[test]
    fn facet_index_and_declaration_are_validated() {
        let p = project("facets");
        let mut cf = CaseFile::default();
        cf.meta = CaseMeta::stamped("facets").unwrap();

        let mut good = rec("good");
        good.facets.insert(Facet::Role, FacetValue::Indexed(2));
        cf.data.entities.push(good);

        let mut bad_index = rec("badidx");
        bad_index.facets.insert(Facet::Role, FacetValue::Indexed(99));
        cf.data.entities.push(bad_index);

        let mut undeclared = rec("undeclared");
        undeclared.facets.insert(Facet::OrgType, FacetValue::Indexed(0));
        cf.data.entities.push(undeclared);

        io::write_case(&p, &cf).unwrap();

        let findings = validate(&p).unwrap();
        let joined = findings.join("\n");
        assert!(
            joined.contains("entity badidx: facet Role index 99 out of range"),
            "an out-of-range Role index is a structural finding: {joined}"
        );
        assert!(
            joined.contains("entity undeclared: facet OrgType is not declared by kind 'Person'"),
            "a facet the kind does not declare is a structural finding: {joined}"
        );
        assert!(
            !joined.contains("entity good:"),
            "an in-range declared facet produces no finding: {joined}"
        );
        std::fs::remove_dir_all(p.root).unwrap();
    }

    #[test]
    fn findings_split_into_structural_and_advisory_tiers() {
        let structural = project("split_structural");
        let mut cf = CaseFile::default();
        cf.meta = CaseMeta::stamped("s").unwrap();
        cf.data.entities.push(rec("a"));
        cf.data.relations.push(rel("a", "ghost", vec![src(), src()]));
        io::write_case(&structural, &cf).unwrap();
        assert!(
            !validate(&structural).unwrap().is_empty(),
            "an unknown-slug edge is a STRUCTURAL finding (would classify exit 2)"
        );
        assert!(
            source_findings(&structural).unwrap().is_empty(),
            "the well-sourced edge raises no advisory noise — the finding is purely structural"
        );
        std::fs::remove_dir_all(structural.root).unwrap();

        let advisory = project("split_advisory");
        let mut cf = CaseFile::default();
        cf.meta = CaseMeta::stamped("a").unwrap();
        cf.data.entities.push(rec("a"));
        cf.data.entities.push(rec("b"));
        cf.data.relations.push(rel("a", "b", vec![]));
        io::write_case(&advisory, &cf).unwrap();
        assert!(
            validate(&advisory).unwrap().is_empty(),
            "an unsourced edge is structurally valid — no structural finding"
        );
        assert!(
            !source_findings(&advisory).unwrap().is_empty(),
            "but it IS an advisory sourcing finding (would classify exit 1)"
        );
        std::fs::remove_dir_all(advisory.root).unwrap();

        let clean = project("split_clean");
        let mut cf = CaseFile::default();
        cf.meta = CaseMeta::stamped("c").unwrap();
        cf.data.entities.push(rec("a"));
        cf.data.entities.push(rec("b"));
        cf.data.relations.push(rel("a", "b", vec![src(), src()]));
        io::write_case(&clean, &cf).unwrap();
        assert!(validate(&clean).unwrap().is_empty(), "no structural findings");
        assert!(source_findings(&clean).unwrap().is_empty(), "no advisory findings — exit 0");
        std::fs::remove_dir_all(clean.root).unwrap();
    }
}

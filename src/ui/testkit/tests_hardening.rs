#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use crate::model::{
        Author, CaseFile, CaseMeta, DatePrecision, DateVal, EntityKind, EntityRec, Facet,
        FacetValue, ObsValue, Observation, Occupation, OccupationIcon, Origin, RelationKind,
        RelationRec, Source, SourceKind, Verification, days_from_civil, fnv1a,
    };
    use crate::project::{
        Project, export, read_case, write_case, write_graph_md, write_json, write_note,
    };
    use crate::ui::testkit::{graph, harness};

    fn temp_project(tag: &str) -> Project {
        let d = std::env::temp_dir().join(format!("redstring_harden_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        Project::create(&d).unwrap()
    }

    fn cleanup(p: Project) {
        let _ = std::fs::remove_dir_all(&p.root);
    }

    fn slurp(path: PathBuf) -> String {
        std::fs::read_to_string(path).unwrap()
    }

    fn case_json(p: &Project, cf: &CaseFile) -> String {
        write_json(p, cf).unwrap();
        slurp(p.root.join("case.json"))
    }

    fn graph_md(p: &Project, cf: &CaseFile) -> String {
        write_graph_md(p, cf).unwrap();
        slurp(p.root.join("graph.md"))
    }

    fn bundle_blob(zip_path: &Path) -> String {
        let f = std::fs::File::open(zip_path).unwrap();
        let mut archive = zip::ZipArchive::new(f).unwrap();
        let mut blob = String::new();
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).unwrap();
            blob.push_str(entry.name());
            blob.push('\n');
            let mut bytes = Vec::new();
            std::io::Read::read_to_end(&mut entry, &mut bytes).unwrap();
            blob.push_str(&String::from_utf8_lossy(&bytes));
            blob.push('\n');
        }
        blob
    }

    fn alias(slug: &str) -> String {
        format!("sealed_{:016x}", fnv1a(slug).unwrap())
    }

    fn named(slug: &str, kind: EntityKind, label: &str, value: &str) -> EntityRec {
        EntityRec {
            slug: slug.to_owned(),
            kind,
            label: label.to_owned(),
            value: value.to_owned(),
            sealed: false,
            observations: vec![],
            facets: Default::default(),
            image: None,
        }
    }

    fn text_obs(name: &str, value: &str, sealed: bool) -> Observation {
        let mut o = Observation::stamp(
            name.to_owned(),
            ObsValue::Text(value.to_owned()),
            Origin::Manual,
            Verification::AnalystAsserted,
            Author::default(),
        )
        .unwrap();
        o.sealed = sealed;
        o
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

    fn note_src(reference: &str, excerpt: &str, sealed: bool) -> Source {
        Source {
            kind: SourceKind::Note,
            reference: reference.to_owned(),
            excerpt: excerpt.to_owned(),
            sealed,
            ..Default::default()
        }
    }

    fn build_case(mask: u32) -> CaseFile {
        let mut cf = CaseFile::default();
        cf.meta = CaseMeta::stamped("fuzz").unwrap();
        cf.data
            .entities
            .push(named("org_acme", EntityKind::Organization, "Acme Care Home", "ACMEPUBLICVALUE"));

        let mut mark = named("person_mark", EntityKind::Person, "Sgt Mark Named", "MARKPUBLICVALUE");
        if mask & 4 != 0 {
            mark.observations.push(text_obs("informant", "SEALEDOBSVALUE", true));
        }
        cf.data.entities.push(mark);

        if mask & 1 != 0 {
            let mut v = named("victim", EntityKind::Person, "SECRETVICTIMNAME", "secret@leak.example");
            v.sealed = true;
            v.image = Some("entities/victim/SEALEDFACEIMG.png".to_owned());
            v.observations = vec![text_obs("dob", "VICTIMDOBSENTINEL", false)];
            v.facets.insert(
                Facet::Occupation,
                FacetValue::Freeform(Occupation {
                    label: "SEALEDVICTIMJOB".to_owned(),
                    icon: OccupationIcon::Scales,
                }),
            );
            cf.data.entities.push(v);
            cf.data
                .relations
                .push(rel("victim", "person_mark", RelationKind::SexuallyAssaulted));
        }
        if mask & 2 != 0 {
            let mut r = rel("org_acme", "person_mark", RelationKind::Paid);
            r.sealed = true;
            r.sources = vec![note_src("SEALEDRELREF", "SEALEDRELEXCERPT", false)];
            cf.data.relations.push(r);
        }
        if mask & 8 != 0 {
            let mut r = rel("org_acme", "person_mark", RelationKind::Financed);
            r.sources = vec![
                Source {
                    kind: SourceKind::Url,
                    reference: "https://public.example/ok".to_owned(),
                    ..Default::default()
                },
                note_src("SEALEDRELSRCREF", "SEALEDRELSRCEXCERPT", true),
            ];
            cf.data.relations.push(r);
        }
        cf
    }

    fn absent(mask: u32) -> Vec<&'static str> {
        let mut v: Vec<&'static str> = Vec::new();
        if mask & 1 != 0 {
            v.extend_from_slice(&[
                "SECRETVICTIMNAME",
                "secret@leak.example",
                "VICTIMDOBSENTINEL",
                "SEALEDVICTIMJOB",
                "SEALEDFACEIMG",
                "sexually assaulted",
            ]);
        }
        if mask & 2 != 0 {
            v.extend_from_slice(&["SEALEDRELREF", "SEALEDRELEXCERPT"]);
        }
        if mask & 4 != 0 {
            v.push("SEALEDOBSVALUE");
        }
        if mask & 8 != 0 {
            v.extend_from_slice(&["SEALEDRELSRCREF", "SEALEDRELSRCEXCERPT"]);
        }
        v
    }

    #[test]
    fn every_seal_combination_redacts_from_derived_case_json_and_graph_md() {
        let p = temp_project("fuzz_derive");
        for mask in 0u32..16 {
            let cf = build_case(mask);
            let json = case_json(&p, &cf);
            let md = graph_md(&p, &cf);
            assert!(
                json.contains("ACMEPUBLICVALUE"),
                "positive control gone for mask={mask}: redact over-redacted an unsealed value"
            );
            assert!(
                md.contains("Acme Care Home"),
                "positive control gone for mask={mask}: an unsealed label vanished from graph.md"
            );
            for s in absent(mask) {
                assert!(!json.contains(s), "LEAK mask={mask}: '{s}' survived into case.json:\n{json}");
                assert!(!md.contains(s), "LEAK mask={mask}: '{s}' survived into graph.md:\n{md}");
            }
        }
        cleanup(p);
    }

    #[test]
    fn every_seal_combination_redacts_from_exported_bundle() {
        for mask in 0u32..16 {
            let p = temp_project(&format!("fuzz_bundle_{mask}"));
            let cf = build_case(mask);
            write_case(&p, &cf).unwrap();
            let (zip_path, _n, _sealed) = export(&p).unwrap();
            let blob = bundle_blob(&zip_path);
            assert!(
                blob.contains("ACMEPUBLICVALUE"),
                "positive control gone from bundle for mask={mask}"
            );
            for s in absent(mask) {
                assert!(!blob.contains(s), "LEAK mask={mask}: '{s}' survived into the exported bundle");
            }
            cleanup(p);
        }
    }

    #[test]
    fn triangulation_multiple_sealed_victims_never_relink_to_named_perpetrator() {
        let p = temp_project("triang");
        let mut cf = CaseFile::default();
        cf.meta = CaseMeta::stamped("triang").unwrap();
        let mut v1 = named("v1", EntityKind::Person, "SECRETVICTIMONE", "v1leak@example");
        v1.sealed = true;
        let mut v2 = named("v2", EntityKind::Person, "SECRETVICTIMTWO", "v2leak@example");
        v2.sealed = true;
        cf.data.entities.push(v1);
        cf.data.entities.push(v2);
        cf.data
            .entities
            .push(named("soldier", EntityKind::Person, "Sgt Named Marku", "SOLDIERPUBLIC"));
        cf.data
            .entities
            .push(named("unit", EntityKind::Organization, "Named Unit", "UNITPUBLIC"));
        cf.data.relations.push(rel("v1", "soldier", RelationKind::SexuallyAssaulted));
        cf.data.relations.push(rel("v2", "soldier", RelationKind::SexuallyAssaulted));
        cf.data.relations.push(rel("soldier", "unit", RelationKind::EmployeeOf));
        write_case(&p, &cf).unwrap();

        let json = case_json(&p, &cf);
        let md = graph_md(&p, &cf);
        let (zip_path, _n, _sealed) = export(&p).unwrap();
        let blob = bundle_blob(&zip_path);
        let a1 = alias("v1");
        let a2 = alias("v2");

        for out in [&json, &md, &blob] {
            for leak in [
                "SECRETVICTIMONE",
                "SECRETVICTIMTWO",
                "v1leak@example",
                "v2leak@example",
                "sexually assaulted",
            ] {
                assert!(!out.contains(leak), "triangulation leak '{leak}'");
            }
            assert!(out.contains("Sgt Named Marku"), "the named perpetrator must still ship");
        }

        for line in md.lines() {
            assert!(
                !(line.contains(&a1) && line.contains("soldier")),
                "no graph.md line may re-link anon victim v1 to the named soldier: {line}"
            );
            assert!(
                !(line.contains(&a2) && line.contains("soldier")),
                "no graph.md line may re-link anon victim v2 to the named soldier: {line}"
            );
        }

        let public: CaseFile = serde_json::from_str(&json).unwrap();
        assert_eq!(
            public.data.relations.len(),
            1,
            "only the non-incident soldier->unit edge survives: {:?}",
            public.data.relations
        );
        assert_eq!(public.data.relations[0].kind, RelationKind::EmployeeOf);
        assert!(
            public
                .data
                .relations
                .iter()
                .all(|r| r.from != a1 && r.to != a1 && r.from != a2 && r.to != a2),
            "no surviving relation may touch either anonymized victim"
        );
        assert!(
            public.data.relations.iter().all(|r| r.kind != RelationKind::SexuallyAssaulted),
            "the sensitive verb must not survive on any relation record"
        );
        cleanup(p);
    }

    #[test]
    fn sealed_node_survives_as_anonymized_alias_but_carries_no_edges() {
        let p = temp_project("alias");
        let mut cf = CaseFile::default();
        cf.meta = CaseMeta::stamped("alias").unwrap();
        let mut victim = named("victim", EntityKind::Person, "SECRETVICTIMNAME", "secret@leak.example");
        victim.sealed = true;
        victim.observations = vec![text_obs("dob", "VICTIMDOBSENTINEL", false)];
        victim.facets.insert(
            Facet::Occupation,
            FacetValue::Freeform(Occupation {
                label: "SEALEDVICTIMJOB".to_owned(),
                icon: OccupationIcon::Scales,
            }),
        );
        cf.data.entities.push(victim);
        cf.data
            .entities
            .push(named("org", EntityKind::Organization, "Owner Org", "ORGPUBLIC"));
        cf.data
            .entities
            .push(named("acct", EntityKind::Organization, "Bank Account X", "ACCTPUBLIC"));
        cf.data.relations.push(rel("org", "victim", RelationKind::Controls));
        cf.data.relations.push(rel("victim", "acct", RelationKind::UsesAccount));
        write_case(&p, &cf).unwrap();
        write_note(&p, "victim", "SEALED_NOTE_BODY do not publish\n").unwrap();

        let json = case_json(&p, &cf);
        let (zip_path, _n, sealed) = export(&p).unwrap();
        assert_eq!(sealed, 1, "one sealed entity reported redacted");
        let blob = bundle_blob(&zip_path);
        let a = alias("victim");

        let public: CaseFile = serde_json::from_str(&json).unwrap();
        assert_eq!(
            public.data.entities.len(),
            3,
            "the sealed node is preserved as an anonymized node, not dropped"
        );
        let ghost = public.data.entities.iter().find(|e| e.slug == a).unwrap();
        assert_eq!(ghost.label, "[sealed]");
        assert!(ghost.value.is_empty());
        assert!(ghost.observations.is_empty());
        assert!(ghost.facets.is_empty());
        assert!(ghost.image.is_none());
        assert!(ghost.slug.starts_with("sealed_"), "the anon slug must be the opaque alias form");
        assert!(!ghost.slug.contains("victim"), "the alias must not embed the real slug");
        assert!(
            public.data.relations.is_empty(),
            "both edges touching the sealed node drop consistently: {:?}",
            public.data.relations
        );

        for out in [&json, &blob] {
            for leak in [
                "SECRETVICTIMNAME",
                "secret@leak.example",
                "VICTIMDOBSENTINEL",
                "SEALEDVICTIMJOB",
                "SEALED_NOTE_BODY",
            ] {
                assert!(!out.contains(leak), "alias leak '{leak}'");
            }
            assert!(out.contains(&a), "the anonymized id must be present");
        }
        cleanup(p);
    }

    #[test]
    fn unsealed_rich_case_publishes_every_real_datum_and_is_derivation_stable() {
        let p = temp_project("noop");
        let mut cf = build_case(0);
        let mut m = named("person_mark2", EntityKind::Person, "Second Named", "SECONDPUBLIC");
        m.observations = vec![text_obs("dob", "OPENDOBSENTINEL", false)];
        cf.data.entities.push(m);
        let mut r = rel("org_acme", "person_mark2", RelationKind::Financed);
        r.sources = vec![note_src("OPENSOURCEREF", "OPENSOURCEEXCERPT", false)];
        cf.data.relations.push(r);

        let json1 = case_json(&p, &cf);
        let json2 = case_json(&p, &cf);
        assert_eq!(json1, json2, "deriving an unchanged case twice is byte-identical");

        for present in [
            "ACMEPUBLICVALUE",
            "SECONDPUBLIC",
            "OPENDOBSENTINEL",
            "OPENSOURCEREF",
            "OPENSOURCEEXCERPT",
        ] {
            assert!(json1.contains(present), "redact over-redacted an unsealed datum '{present}'");
        }

        let back: CaseFile = serde_json::from_str(&json1).unwrap();
        assert_eq!(back.data, cf.data, "an unsealed derive is a pure identity on case data");
        cleanup(p);
    }

    #[test]
    fn redaction_stays_leak_free_under_double_derivation() {
        let p = temp_project("double");
        let cf = build_case(1 | 2 | 4 | 8);
        let j1 = case_json(&p, &cf);
        let cf2: CaseFile = serde_json::from_str(&j1).unwrap();
        let j2 = case_json(&p, &cf2);

        for j in [&j1, &j2] {
            for leak in absent(1 | 2 | 4 | 8) {
                assert!(!j.contains(leak), "re-derivation re-leaked '{leak}'");
            }
            assert!(j.contains("[sealed]"), "the anonymized placeholder must persist across re-derivation");
        }
        let twice: CaseFile = serde_json::from_str(&j2).unwrap();
        assert!(
            twice.data.entities.iter().any(|e| e.label == "[sealed]"),
            "the sealed node stays anonymized after a second redaction"
        );
        assert!(
            twice.data.relations.iter().all(|r| r.kind != RelationKind::SexuallyAssaulted),
            "the incident edge stays dropped after a second redaction"
        );
        cleanup(p);
    }

    #[test]
    fn sealed_source_on_relation_is_dropped_from_derived_output() {
        let p = temp_project("relsrc");
        let mut cf = CaseFile::default();
        cf.meta = CaseMeta::stamped("relsrc").unwrap();
        cf.data.entities.push(named("a", EntityKind::Person, "A", ""));
        cf.data.entities.push(named("b", EntityKind::Person, "B", ""));
        let mut r = rel("a", "b", RelationKind::Controls);
        r.sources = vec![
            Source {
                kind: SourceKind::Url,
                reference: "https://public.example/keepme".to_owned(),
                ..Default::default()
            },
            note_src("RELSEALEDREF", "RELSEALEDEXCERPT", true),
        ];
        cf.data.relations.push(r);

        let json = case_json(&p, &cf);
        assert!(!json.contains("RELSEALEDREF"), "a sealed source on a relation must be dropped: {json}");
        assert!(!json.contains("RELSEALEDEXCERPT"), "a sealed relation-source excerpt must be dropped");
        assert!(json.contains("public.example/keepme"), "the unsealed sibling source survives");
        cleanup(p);
    }

    #[test]
    fn sealed_source_on_unsealed_observation_is_redacted() {
        let p = temp_project("obssrc");
        let mut cf = CaseFile::default();
        cf.meta = CaseMeta::stamped("obssrc").unwrap();
        let mut auditor = named("auditor", EntityKind::Person, "The Auditor", "");
        let mut o = text_obs("met_at", "2021", false);
        o.sources = vec![note_src("OBSSEALEDREF", "OBSSEALEDEXCERPT", true)];
        auditor.observations = vec![o];
        cf.data.entities.push(auditor);
        write_case(&p, &cf).unwrap();

        let json = case_json(&p, &cf);
        let (zip_path, _n, _sealed) = export(&p).unwrap();
        let blob = bundle_blob(&zip_path);

        assert!(
            !json.contains("OBSSEALEDREF"),
            "a sealed source reference on an unsealed observation must be redacted from case.json:\n{json}"
        );
        assert!(!json.contains("OBSSEALEDEXCERPT"), "a sealed obs-source excerpt must be redacted from case.json");
        assert!(!blob.contains("OBSSEALEDREF"), "a sealed obs-source reference must be redacted from the bundle");
        assert!(!blob.contains("OBSSEALEDEXCERPT"), "a sealed obs-source excerpt must be redacted from the bundle");
        cleanup(p);
    }

    #[test]
    fn redact_reapplication_keeps_the_sealed_alias_stable() {
        let p = temp_project("stable");
        let cf = build_case(1);
        let j1 = case_json(&p, &cf);
        let cf2: CaseFile = serde_json::from_str(&j1).unwrap();
        let j2 = case_json(&p, &cf2);
        let a = alias("victim");
        let first: CaseFile = serde_json::from_str(&j1).unwrap();
        let second: CaseFile = serde_json::from_str(&j2).unwrap();
        assert!(first.data.entities.iter().any(|e| e.slug == a));
        assert!(
            second.data.entities.iter().any(|e| e.slug == a),
            "the anonymized slug must be identical after a second redaction"
        );
        cleanup(p);
    }

    fn yv(y: i64) -> DateVal {
        DateVal {
            secs: days_from_civil(y, 1, 1).unwrap() * 86_400,
            precision: DatePrecision::Year,
        }
    }

    fn mv(y: i64, m: i64) -> DateVal {
        DateVal {
            secs: days_from_civil(y, m, 1).unwrap() * 86_400,
            precision: DatePrecision::Month,
        }
    }

    fn dv(y: i64, m: i64, d: i64) -> DateVal {
        DateVal {
            secs: days_from_civil(y, m, d).unwrap() * 86_400,
            precision: DatePrecision::Day,
        }
    }

    fn decade(y: i64) -> DateVal {
        DateVal {
            secs: days_from_civil(y, 1, 1).unwrap() * 86_400,
            precision: DatePrecision::Decade,
        }
    }

    fn century(y: i64) -> DateVal {
        DateVal {
            secs: days_from_civil(y, 1, 1).unwrap() * 86_400,
            precision: DatePrecision::Century,
        }
    }

    fn circa(y: i64) -> DateVal {
        DateVal {
            secs: days_from_civil(y, 1, 1).unwrap() * 86_400,
            precision: DatePrecision::Circa,
        }
    }

    fn range_text(start: Option<DateVal>, end: Option<DateVal>) -> String {
        ObsValue::DateRange { start, end }.text().unwrap()
    }

    #[test]
    fn relation_validity_window_round_trips_through_project_ndjson() {
        let p = temp_project("temporal_rt");
        let mut cf = CaseFile::default();
        cf.meta = CaseMeta::stamped("t").unwrap();
        cf.data.entities.push(named("colonel", EntityKind::Person, "Colonel", ""));
        cf.data.entities.push(named("base", EntityKind::Organization, "Base", ""));
        let mut windowed = rel("colonel", "base", RelationKind::Manages);
        windowed.valid_from = Some(yv(2012));
        windowed.valid_to = Some(yv(2017));
        cf.data.relations.push(windowed.clone());
        cf.data.relations.push(rel("base", "colonel", RelationKind::Owns));
        write_case(&p, &cf).unwrap();

        let ndjson = slurp(p.relations_path().unwrap());
        assert!(ndjson.contains("valid_from"), "the windowed edge line must carry valid_from: {ndjson}");
        assert!(ndjson.contains("valid_to"), "the windowed edge line must carry valid_to: {ndjson}");

        let back = read_case(&p).unwrap();
        let w = back.data.relations.iter().find(|r| r.kind == RelationKind::Manages).unwrap();
        assert_eq!(w.valid_from, Some(yv(2012)));
        assert_eq!(w.valid_to, Some(yv(2017)));
        assert_eq!(
            serde_json::to_string(w).unwrap(),
            serde_json::to_string(&windowed).unwrap(),
            "the reloaded windowed relation re-serializes byte-identically"
        );
        let plain = back.data.relations.iter().find(|r| r.kind == RelationKind::Owns).unwrap();
        assert!(plain.valid_from.is_none() && plain.valid_to.is_none());
        assert!(
            !serde_json::to_string(plain).unwrap().contains("valid_"),
            "a None validity window must serde-skip both keys entirely"
        );
        cleanup(p);
    }

    #[test]
    fn date_range_renders_civil_spans_across_precisions_and_open_ends() {
        assert_eq!(range_text(Some(yv(2012)), Some(yv(2013))), "2012–2013");
        assert_eq!(
            range_text(Some(mv(2012, 11)), Some(dv(2013, 4, 12))),
            "2012-11–2013-04-12",
            "each bound renders at its own precision"
        );
        assert_eq!(range_text(Some(decade(2010)), Some(yv(2017))), "2010s–2017");
        assert_eq!(range_text(Some(century(1850)), Some(century(1975))), "19th c.–20th c.");
        assert_eq!(range_text(Some(circa(2024)), None), "~2024–…");
        assert_eq!(range_text(Some(yv(2000)), Some(yv(2000))), "2000–2000", "a degenerate span still renders");
        assert_eq!(range_text(Some(yv(2017)), None), "2017–…");
        assert_eq!(range_text(None, Some(yv(2017))), "…–2017");
        assert_eq!(range_text(None, None), "…", "a fully-open range never panics");
    }

    #[test]
    fn date_range_pre_1970_and_bce_render_without_double_floor() {
        assert_eq!(range_text(Some(yv(1965)), Some(yv(1975))), "1965–1975", "a span crossing the epoch");
        assert_eq!(range_text(Some(yv(1901)), Some(yv(1969))), "1901–1969", "a wholly pre-epoch span");
        assert_eq!(
            range_text(Some(dv(1965, 6, 15)), Some(dv(1972, 3, 4))),
            "1965-06-15–1972-03-04",
            "day precision across the epoch is exact"
        );
        assert_eq!(range_text(Some(yv(-44)), Some(yv(100))), "-44–100", "a BCE lower bound renders sanely");

        let pre = range_text(Some(yv(1965)), Some(yv(1969)));
        assert!(
            !pre.contains(&yv(1965).secs.to_string()) && !pre.contains(&yv(1969).secs.to_string()),
            "the raw negative epoch must never leak through a range bound: {pre}"
        );
    }

    #[test]
    fn date_range_serde_is_byte_stable_and_omits_open_ends() {
        let full = ObsValue::DateRange {
            start: Some(yv(2012)),
            end: Some(yv(2013)),
        };
        let j = serde_json::to_string(&full).unwrap();
        let back: ObsValue = serde_json::from_str(&j).unwrap();
        assert_eq!(back, full);
        assert_eq!(serde_json::to_string(&back).unwrap(), j, "a closed range re-serializes byte-identically");

        let open_start = ObsValue::DateRange {
            start: None,
            end: Some(yv(2013)),
        };
        let j = serde_json::to_string(&open_start).unwrap();
        assert!(!j.contains("start"), "an open start must omit the key: {j}");
        assert!(j.contains("end"));
        assert_eq!(serde_json::from_str::<ObsValue>(&j).unwrap(), open_start);

        let both_open = ObsValue::DateRange { start: None, end: None };
        let j = serde_json::to_string(&both_open).unwrap();
        assert!(!j.contains("start") && !j.contains("end"), "a fully-open range serializes no bound keys: {j}");
        assert_eq!(serde_json::from_str::<ObsValue>(&j).unwrap(), both_open);
    }

    fn fp(app: &crate::ui::RedstringApp) -> String {
        crate::ui::fingerprint::compute_fingerprint(&app.g, "case", false).unwrap()
    }

    fn set_valid_from(app: &mut crate::ui::RedstringApp, e: petgraph::graph::EdgeIndex, v: Option<DateVal>) {
        app.g.edge_mut(e).unwrap().payload_mut().valid_from = v;
    }

    fn set_valid_to(app: &mut crate::ui::RedstringApp, e: petgraph::graph::EdgeIndex, v: Option<DateVal>) {
        app.g.edge_mut(e).unwrap().payload_mut().valid_to = v;
    }

    #[test]
    fn case_fingerprint_flips_on_edge_validity_and_is_otherwise_stable() {
        let mut app = harness::app().unwrap();
        let a = graph::add(&mut app, EntityKind::Person, "A").unwrap();
        let b = graph::add(&mut app, EntityKind::Person, "B").unwrap();
        let e = graph::link(&mut app, a, b, RelationKind::Controls).unwrap();

        let base = fp(&app);
        assert_eq!(base, fp(&app), "recomputing an unchanged graph is stable");

        set_valid_from(&mut app, e, Some(yv(2012)));
        let with_from = fp(&app);
        assert_ne!(base, with_from, "adding valid_from flips the case fingerprint");

        set_valid_from(&mut app, e, Some(mv(2012, 1)));
        assert_ne!(
            with_from,
            fp(&app),
            "changing only the valid_from precision (same secs) flips the fingerprint"
        );

        set_valid_from(&mut app, e, None);
        assert_eq!(base, fp(&app), "clearing valid_from returns to the original fingerprint");

        set_valid_from(&mut app, e, Some(yv(2012)));
        set_valid_to(&mut app, e, Some(yv(2013)));
        let forward = fp(&app);
        set_valid_from(&mut app, e, Some(yv(2013)));
        set_valid_to(&mut app, e, Some(yv(2012)));
        assert_ne!(forward, fp(&app), "swapping valid_from and valid_to yields a distinct fingerprint");
    }

    #[test]
    fn relation_rec_line_flips_on_validity_and_is_stable_when_unchanged() {
        let mut a = rel("x", "y", RelationKind::Controls);
        a.valid_from = Some(yv(2012));
        let b = a.clone();
        let ja = serde_json::to_string(&a).unwrap();
        assert_eq!(
            ja,
            serde_json::to_string(&b).unwrap(),
            "identical relations serialize identically, so an unchanged edge stays fingerprint-stable"
        );

        let mut prec = a.clone();
        prec.valid_from = Some(mv(2012, 1));
        assert_ne!(ja, serde_json::to_string(&prec).unwrap(), "a precision-only valid_from change flips the line");

        let mut added = a.clone();
        added.valid_to = Some(yv(2013));
        assert_ne!(ja, serde_json::to_string(&added).unwrap(), "adding valid_to flips the line");
    }
}

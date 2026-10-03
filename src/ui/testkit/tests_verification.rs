#[cfg(test)]
mod tests {
    use petgraph::graph::EdgeIndex;

    use crate::model::{
        EntityKind, Footing, Origin, Relation, RelationKind, Source, SourceKind, SourceTier,
        Verification,
    };
    use crate::project::{read_case, source_findings, write_dossiers, write_graph_md};
    use crate::ui::RedstringApp;
    use crate::ui::testkit::{graph, harness, invariant};

    fn src(reference: &str, org: &str, tier: SourceTier) -> Source {
        Source {
            kind: SourceKind::Url,
            reference: reference.to_owned(),
            source_org: Some(org.to_owned()),
            tier,
            ..Default::default()
        }
    }

    fn team_page() -> Source {
        src(
            "https://org.example/team",
            "Example Org",
            SourceTier::Primary,
        )
    }

    fn case() -> (harness::TempDir, RedstringApp, EdgeIndex) {
        let (td, project) = harness::temp_project().unwrap();
        let mut app = harness::app().unwrap();
        app.project = Some(project);
        let ceo = graph::add(&mut app, EntityKind::Person, "Jane Roe").unwrap();
        let org = graph::add(&mut app, EntityKind::Organization, "Example Org").unwrap();
        let e = graph::link_sourced(&mut app, ceo, org, RelationKind::CeoOf, team_page()).unwrap();
        (td, app, e)
    }

    fn rel(app: &mut RedstringApp, e: EdgeIndex) -> &mut Relation {
        app.g.edge_mut(e).unwrap().payload_mut()
    }

    fn footing(app: &mut RedstringApp, e: EdgeIndex) -> Footing {
        Footing::from(&*rel(app, e))
    }

    fn findings(app: &RedstringApp) -> String {
        source_findings(app.project.as_ref().unwrap())
            .unwrap()
            .join("\n")
    }

    fn collect(shape: &egui::Shape, out: &mut Vec<String>) {
        match shape {
            egui::Shape::Text(t) => out.push(t.galley.text().to_owned()),
            egui::Shape::Vec(v) => v.iter().for_each(|s| collect(s, out)),
            _ => {}
        }
    }

    fn inspector_text(app: &mut RedstringApp, e: EdgeIndex) -> String {
        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(420.0, 1400.0),
        ));
        let out = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show_inside(ui, |ui| {
                app.edge_inspector(ui, e).unwrap();
            });
        });
        let mut texts = Vec::new();
        for cs in &out.shapes {
            collect(&cs.shape, &mut texts);
        }
        texts.join("\n")
    }

    #[test]
    fn rating_a_source_primary_never_verifies_the_relation_by_itself() {
        let (_td, mut app, e) = case();
        assert_eq!(rel(&mut app, e).status, Verification::AnalystAsserted);
        assert_eq!(
            footing(&mut app, e),
            Footing::Sourced,
            "a primary tier is not a verdict"
        );
        assert!(!Footing::STRONG.contains(&footing(&mut app, e)));
        let text = inspector_text(&mut app, e);
        assert!(
            text.contains("standing"),
            "the relation inspector has a standing section: {text}"
        );
        assert!(text.contains("sourced claim"), "{text}");
        assert!(
            !text.contains("red string"),
            "nothing is drawn strong before the analyst acts: {text}"
        );
    }

    #[test]
    fn one_primary_source_confirms_a_narrow_claim_and_draws_the_red_string() {
        let (_td, mut app, e) = case();
        let r = rel(&mut app, e);
        assert!(Footing::would_hold(Verification::Confirmed, &r.origin, &r.sources).unwrap());
        assert!(
            !Footing::would_hold(Verification::Corroborated, &r.origin, &r.sources).unwrap(),
            "one official page cannot be offered as independent corroboration"
        );
        rel(&mut app, e).status = Verification::Confirmed;
        assert_eq!(footing(&mut app, e), Footing::Confirmed);
        assert!(Footing::STRONG.contains(&footing(&mut app, e)));
        let text = inspector_text(&mut app, e);
        assert!(
            text.contains("confirmed · primary source — red string"),
            "{text}"
        );
        assert!(text.contains("not independent corroboration"), "{text}");

        harness::save(&mut app).unwrap();
        let joined = findings(&app);
        assert!(
            !joined.contains("needs corroboration"),
            "a primary-confirmed claim is not nagged for corroboration: {joined}"
        );
        assert!(!joined.contains("OVERCLAIMED"), "{joined}");
        invariant::check_case_dir(app.project.as_ref().unwrap()).unwrap();
        invariant::check_graph(&app).unwrap();
    }

    #[test]
    fn confirmation_without_a_primary_source_is_guarded_and_flagged() {
        let (_td, mut app, e) = case();
        rel(&mut app, e).sources =
            vec![src("https://news.example/a", "News", SourceTier::Reported)];
        let r = rel(&mut app, e);
        assert!(!Footing::would_hold(Verification::Confirmed, &r.origin, &r.sources).unwrap());
        r.status = Verification::Confirmed;
        assert_eq!(
            footing(&mut app, e),
            Footing::Sourced,
            "the status alone draws nothing strong"
        );
        let text = inspector_text(&mut app, e);
        assert!(
            text.contains("but that needs a source rated primary"),
            "{text}"
        );
        assert!(!text.contains("red string"), "{text}");

        harness::save(&mut app).unwrap();
        let joined = findings(&app);
        assert!(
            joined.contains("OVERCLAIMED edge jane_roe --CEO of--> example_org"),
            "{joined}"
        );
        assert!(joined.contains("needs a source rated primary"), "{joined}");
    }

    #[test]
    fn an_unsourced_relation_never_holds_a_strong_status() {
        let (_td, mut app, e) = case();
        rel(&mut app, e).sources.clear();
        for status in [Verification::Confirmed, Verification::Corroborated] {
            let r = rel(&mut app, e);
            assert!(
                !Footing::would_hold(status, &r.origin, &r.sources).unwrap(),
                "{status:?}"
            );
            r.status = status;
            assert_eq!(
                footing(&mut app, e),
                Footing::Unproven,
                "{status:?} is not a source"
            );
            let text = inspector_text(&mut app, e);
            assert!(text.contains("UNSOURCED"), "{text}");
            assert!(text.contains("but that needs"), "{text}");
            assert!(!text.contains("red string"), "{text}");
        }
        rel(&mut app, e).origin = Origin::Inference;
        rel(&mut app, e).sources = vec![team_page()];
        rel(&mut app, e).status = Verification::Confirmed;
        assert_eq!(
            footing(&mut app, e),
            Footing::Unproven,
            "an inference stays a hypothesis"
        );
    }

    #[test]
    fn corroboration_needs_independent_origins_and_reads_apart_from_confirmation() {
        let (_td, mut app, e) = case();
        let press = src(
            "https://org.example/press",
            "example org",
            SourceTier::Primary,
        );
        rel(&mut app, e).sources.push(press);
        let r = rel(&mut app, e);
        assert!(
            !Footing::would_hold(Verification::Corroborated, &r.origin, &r.sources).unwrap(),
            "two pages of the same organisation are one origin"
        );
        r.status = Verification::Corroborated;
        assert_eq!(footing(&mut app, e), Footing::Sourced);
        harness::save(&mut app).unwrap();
        let joined = findings(&app);
        assert!(joined.contains("OVERCLAIMED"), "{joined}");
        assert!(joined.contains("2+ independent origins"), "{joined}");

        let registry = src(
            "https://registry.example/x",
            "Company Registry",
            SourceTier::Primary,
        );
        rel(&mut app, e).sources.push(registry);
        assert_eq!(footing(&mut app, e), Footing::Corroborated);
        assert_ne!(
            Footing::Corroborated.label().unwrap(),
            Footing::Confirmed.label().unwrap()
        );
        harness::save(&mut app).unwrap();
        assert!(!findings(&app).contains("OVERCLAIMED"));
    }

    #[test]
    fn relation_status_survives_save_and_load_as_a_named_variant() {
        let (_td, mut app, e) = case();
        rel(&mut app, e).status = Verification::Confirmed;
        harness::save(&mut app).unwrap();
        let project = app.project.clone().unwrap();
        let line = std::fs::read_to_string(project.root.join("relations.ndjson")).unwrap();
        assert!(line.contains("\"status\":\"Confirmed\""), "{line}");

        harness::load(&mut app).unwrap();
        let rels = graph::edge_relations(&app).unwrap();
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0].status, Verification::Confirmed);
        assert_eq!(Footing::from(&rels[0]), Footing::Confirmed);
        let back = read_case(&project).unwrap();
        assert_eq!(back.data.relations[0].status, Verification::Confirmed);
        invariant::check_case_file(&back).unwrap();
    }

    #[test]
    fn exports_draw_strength_from_the_sources_they_actually_publish() {
        let (_td, mut app, e) = case();
        rel(&mut app, e).status = Verification::Confirmed;
        harness::save(&mut app).unwrap();
        let project = app.project.clone().unwrap();
        let cf = read_case(&project).unwrap();
        write_graph_md(&project, &cf).unwrap();
        write_dossiers(&project, &cf).unwrap();
        let md = std::fs::read_to_string(project.root.join("graph.md")).unwrap();
        assert!(md.contains("jane_roe ==>|CEO of| example_org"), "{md}");
        let dossier =
            std::fs::read_to_string(project.root.join("entities/jane_roe/dossier.md")).unwrap();
        assert!(dossier.contains("confirmed · primary source"), "{dossier}");

        let mut sealed = cf.clone();
        sealed.data.relations[0].sources[0].sealed = true;
        sealed.data.relations[0].sources.push(src(
            "https://news.example/a",
            "News",
            SourceTier::Reported,
        ));
        write_graph_md(&project, &sealed).unwrap();
        write_dossiers(&project, &sealed).unwrap();
        let md = std::fs::read_to_string(project.root.join("graph.md")).unwrap();
        assert!(
            md.contains("jane_roe -->|CEO of| example_org"),
            "with its primary source sealed the public edge is only a sourced claim: {md}"
        );
        let dossier =
            std::fs::read_to_string(project.root.join("entities/jane_roe/dossier.md")).unwrap();
        assert!(dossier.contains("sourced claim"), "{dossier}");
        assert!(!dossier.contains("confirmed · primary source"), "{dossier}");
    }
}

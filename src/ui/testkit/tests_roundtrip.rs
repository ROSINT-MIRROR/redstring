#[cfg(test)]
mod tests {
    use crate::model::{
        CaseFile, CaseMeta, EntityKind, ObsValue, Observation, Origin, RelationKind, Source,
        SourceKind, Verification,
    };
    use crate::project::{Project, read_case};
    use crate::ui::testkit::{fixtures, graph, harness, invariant};

    #[test]
    fn sample_case_loads_and_validates_clean() {
        let td = harness::temp_dir().unwrap();
        let project = fixtures::open_sample_case(&td.path.join("loaded")).unwrap();
        let cf = read_case(&project).unwrap();
        assert_eq!(cf.data.entities.len(), 3);
        assert_eq!(cf.data.relations.len(), 2);

        let doe = cf.data.entities.iter().find(|e| e.slug == "j_doe").unwrap();
        assert_eq!(doe.kind, EntityKind::Person);
        assert_eq!(doe.value, "John Doe");
        assert_eq!(doe.observations.len(), 2);

        let domain = cf
            .data
            .entities
            .iter()
            .find(|e| e.slug == "evil_corp_example")
            .unwrap();
        assert_eq!(domain.kind, EntityKind::Domain);
        assert_eq!(domain.observations[0].origin, Origin::Lookup("whois".to_owned()));

        invariant::check_case_dir(&project).unwrap();
        drop(td);
    }

    #[test]
    fn sample_case_round_trips_through_disk() {
        let td = harness::temp_dir().unwrap();
        let project = fixtures::open_sample_case(&td.path.join("src")).unwrap();
        let cf = read_case(&project).unwrap();

        let (td2, project2) = harness::temp_project().unwrap();
        let back = harness::round_trip(&project2, &cf).unwrap();
        invariant::assert_round_trip(&cf, &back).unwrap();
        invariant::check_case_file(&back).unwrap();
        drop(td);
        drop(td2);
    }

    #[test]
    fn rich_case_file_round_trips_with_observations_and_sources() {
        let (td, project) = harness::temp_project().unwrap();
        let author = harness::test_author().unwrap();
        let mut cf = CaseFile::default();
        cf.meta = CaseMeta::stamped("rich").unwrap();
        let mut doe = graph::entity_rec(EntityKind::Person, "J. Doe", "John Doe").unwrap();
        doe.observations.push(
            Observation::stamp(
                "dob".to_owned(),
                ObsValue::Text("1971-03-04".to_owned()),
                Origin::Manual,
                Verification::AnalystAsserted,
                author.clone(),
            )
            .unwrap(),
        );
        cf.data.entities.push(doe);
        cf.data
            .entities
            .push(graph::entity_rec(EntityKind::Domain, "evil.example", "evil.example").unwrap());
        let mut r = graph::relation_rec("j_doe", "evil_example", RelationKind::OwnsDomain, &author)
            .unwrap();
        r.sources.push(Source {
            kind: SourceKind::Url,
            reference: "https://registry.example/record".to_owned(),
            excerpt: "sole registrant".to_owned(),
        });
        cf.data.relations.push(r);

        let back = harness::round_trip(&project, &cf).unwrap();
        invariant::assert_round_trip(&cf, &back).unwrap();
        invariant::check_case_file(&back).unwrap();
        let d = back.data.entities.iter().find(|e| e.slug == "j_doe").unwrap();
        assert_eq!(d.observations.len(), 1);
        assert_eq!(d.observations[0].value.text().unwrap(), "1971-03-04");
        drop(td);
    }

    #[test]
    fn rich_case_survives_the_app_save_and_load_path() {
        let (td, project) = harness::temp_project().unwrap();
        let root = project.root.clone();
        let mut app = harness::app().unwrap();
        app.project = Some(project);
        let doe = graph::add_valued(&mut app, EntityKind::Person, "J. Doe", "John Doe").unwrap();
        let dom =
            graph::add_valued(&mut app, EntityKind::Domain, "evil.example", "evil.example").unwrap();
        graph::observe(
            &mut app,
            doe,
            "dob",
            "1971-03-04",
            Origin::Manual,
            Verification::AnalystAsserted,
        )
        .unwrap();
        let src = graph::source(SourceKind::Url, "https://registry.example/x", "y").unwrap();
        graph::link_sourced(&mut app, doe, dom, RelationKind::OwnsDomain, src).unwrap();
        harness::save(&mut app).unwrap();

        let cf = read_case(&Project::open(&root).unwrap()).unwrap();
        assert_eq!(cf.data.entities.len(), 2, "app.save wrote both entities");

        let mut app2 = harness::app().unwrap();
        app2.project = Some(Project::open(&root).unwrap());
        harness::load(&mut app2).unwrap();
        assert_eq!(graph::node_count(&app2).unwrap(), 2);
        assert_eq!(graph::edge_count(&app2).unwrap(), 1);
        assert_eq!(graph::slugs(&app2).unwrap(), graph::slugs(&app).unwrap());
        let doe2 = graph::node_by_slug(&app2, "j_doe").unwrap().unwrap();
        assert_eq!(graph::value_of(&app2, doe2).unwrap(), "John Doe");
        assert_eq!(
            graph::observation(&app2, doe2, "dob")
                .unwrap()
                .unwrap()
                .value
                .text()
                .unwrap(),
            "1971-03-04"
        );
        invariant::check_graph(&app2).unwrap();
        drop(td);
    }

    #[test]
    fn missing_line_files_degrade_to_empty() {
        let (td, project) = harness::temp_project().unwrap();
        let baseline = read_case(&project).unwrap();
        assert!(baseline.data.entities.is_empty());
        assert!(baseline.data.relations.is_empty());

        std::fs::remove_file(project.relations_path().unwrap()).unwrap();
        std::fs::remove_file(project.display_path().unwrap()).unwrap();
        let cf = read_case(&project).unwrap();
        assert!(
            cf.data.relations.is_empty(),
            "a missing relations file degrades to empty, never errors"
        );
        assert_eq!(
            cf.display,
            crate::model::CaseDisplay::default(),
            "a missing display file degrades to defaults"
        );
        drop(td);
    }

    #[test]
    fn malformed_relations_surface_an_error_not_a_panic() {
        let (td, project) = harness::temp_project().unwrap();
        std::fs::write(project.relations_path().unwrap(), "this is not json at all\n").unwrap();
        assert!(
            read_case(&project).is_err(),
            "a corrupt line file must be a handled error, not a panic or silent drop"
        );
        drop(td);
    }

    #[test]
    fn entity_dir_without_a_head_is_skipped() {
        let (td, project) = harness::temp_project().unwrap();
        std::fs::create_dir_all(project.entity_dir("ghost").unwrap()).unwrap();
        let cf = read_case(&project).unwrap();
        assert!(
            cf.data.entities.iter().all(|e| e.slug != "ghost"),
            "a headless entity dir is skipped, not errored"
        );
        drop(td);
    }
}

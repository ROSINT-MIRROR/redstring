#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::model::{EntityKind, Origin, RelationKind, SourceKind, Verification};
    use crate::project::{Project, read_case};
    use crate::ui::RedstringApp;
    use crate::ui::testkit::{graph, harness, invariant};

    fn rich_project_app() -> (harness::TempDir, PathBuf, RedstringApp) {
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
        let src = graph::source(SourceKind::Url, "https://registry.example/x", "sole registrant").unwrap();
        graph::link_sourced(&mut app, doe, dom, RelationKind::OwnsDomain, src).unwrap();
        (td, root, app)
    }

    fn read_bytes(path: PathBuf) -> Vec<u8> {
        std::fs::read(&path).unwrap()
    }

    #[test]
    fn app_save_load_save_is_model_stable() {
        let (td, root, mut app) = rich_project_app();
        harness::save(&mut app).unwrap();
        let after_first = read_case(&Project::open(&root).unwrap()).unwrap();

        let mut app2 = harness::app().unwrap();
        app2.project = Some(Project::open(&root).unwrap());
        harness::load(&mut app2).unwrap();
        harness::save(&mut app2).unwrap();
        let after_second = read_case(&Project::open(&root).unwrap()).unwrap();

        invariant::assert_round_trip(&after_first, &after_second).unwrap();
        invariant::check_case_file(&after_second).unwrap();
        drop(td);
    }

    #[test]
    fn resaving_an_unchanged_case_is_byte_stable() {
        let (td, root, mut app) = rich_project_app();
        harness::save(&mut app).unwrap();
        let project = Project::open(&root).unwrap();
        let relations = read_bytes(project.relations_path().unwrap());
        let display = read_bytes(project.display_path().unwrap());
        let head = read_bytes(project.entity_dir("j_doe").unwrap().join("entity.ron"));
        let obs = read_bytes(project.entity_dir("j_doe").unwrap().join("observations.ndjson"));

        harness::save(&mut app).unwrap();
        assert_eq!(relations, read_bytes(project.relations_path().unwrap()), "relations bytes drift");
        assert_eq!(display, read_bytes(project.display_path().unwrap()), "display bytes drift");
        assert_eq!(head, read_bytes(project.entity_dir("j_doe").unwrap().join("entity.ron")), "head bytes drift");
        assert_eq!(obs, read_bytes(project.entity_dir("j_doe").unwrap().join("observations.ndjson")), "obs bytes drift");
        drop(td);
    }

    #[test]
    fn sealing_a_node_flips_dirty_and_round_trips_through_disk() {
        let (td, project) = harness::temp_project().unwrap();
        let root = project.root.clone();
        let mut app = harness::app().unwrap();
        app.project = Some(project);
        let doe =
            graph::add_valued(&mut app, EntityKind::Person, "Jane Victim", "jane@x.example").unwrap();
        harness::save(&mut app).unwrap();
        assert!(!app.dirty().unwrap(), "a freshly saved case is clean");

        app.g.node_mut(doe).unwrap().payload_mut().sealed = true;
        assert!(app.dirty().unwrap(), "toggling sealed must mark the case dirty");

        harness::save(&mut app).unwrap();
        let head_path = Project::open(&root)
            .unwrap()
            .entity_dir("jane_victim")
            .unwrap()
            .join("entity.ron");
        let head = String::from_utf8(read_bytes(head_path)).unwrap();
        assert!(head.contains("sealed"), "the sealed flag must persist to entity.ron: {head}");

        let mut app2 = harness::app().unwrap();
        app2.project = Some(Project::open(&root).unwrap());
        harness::load(&mut app2).unwrap();
        let idx = graph::node_by_slug(&app2, "jane_victim").unwrap().unwrap();
        assert!(
            app2.g.g().node_weight(idx).unwrap().payload().sealed,
            "sealed must round-trip through save→load"
        );
        drop(td);
    }

    #[test]
    fn validate_stays_clean_across_a_mutation_sequence() {
        let (td, project) = harness::temp_project().unwrap();
        let root = project.root.clone();
        let mut app = harness::app().unwrap();
        app.project = Some(project);

        let a = graph::add_valued(&mut app, EntityKind::Domain, "a.example", "a.example").unwrap();
        let b = graph::add(&mut app, EntityKind::Organization, "B Corp").unwrap();
        graph::link(&mut app, a, b, RelationKind::RegisteredAt).unwrap();
        harness::save(&mut app).unwrap();
        invariant::check_case_dir(&Project::open(&root).unwrap()).unwrap();

        graph::observe(&mut app, a, "registrar", "Acme", Origin::Manual, Verification::AnalystAsserted).unwrap();
        app.apply_label(b, "B Holdings").unwrap();
        let c = graph::add(&mut app, EntityKind::Organization, "B Holdings").unwrap();
        harness::save(&mut app).unwrap();
        invariant::check_case_dir(&Project::open(&root).unwrap()).unwrap();

        let removed = app.g.remove_node(c);
        assert!(removed.is_some());
        harness::save(&mut app).unwrap();
        invariant::check_case_dir(&Project::open(&root).unwrap()).unwrap();
        assert!(
            !Project::open(&root).unwrap().entity_dir("b_holdings_2").unwrap().exists(),
            "the deleted entity's dir is pruned on save"
        );
        drop(td);
    }

    #[test]
    fn rich_node_round_trips_through_the_app_save_load_path() {
        let (td, root, mut app) = rich_project_app();
        harness::save(&mut app).unwrap();

        let mut app2 = harness::app().unwrap();
        app2.project = Some(Project::open(&root).unwrap());
        harness::load(&mut app2).unwrap();

        let doe = graph::node_by_slug(&app2, "j_doe").unwrap().unwrap();
        assert_eq!(graph::label_of(&app2, doe).unwrap(), "J. Doe");
        assert_eq!(graph::value_of(&app2, doe).unwrap(), "John Doe");
        let dob = graph::observation(&app2, doe, "dob").unwrap().unwrap();
        assert_eq!(dob.value.text().unwrap(), "1971-03-04");
        assert_eq!(dob.status, Verification::AnalystAsserted);
        let dom = graph::node_by_slug(&app2, "evil_example").unwrap().unwrap();
        let srcs = graph::edge_sources(&app2, doe, dom, RelationKind::OwnsDomain).unwrap();
        assert_eq!(srcs.len(), 1, "the edge source survives the round trip");
        assert_eq!(srcs[0].reference, "https://registry.example/x");
        invariant::check_graph(&app2).unwrap();
        drop(td);
    }

    #[test]
    fn renaming_preserves_observations_and_edges() {
        let mut app = harness::app().unwrap();
        let p = graph::add(&mut app, EntityKind::Person, "Original Name").unwrap();
        let o = graph::add(&mut app, EntityKind::Organization, "Acme").unwrap();
        graph::observe(&mut app, p, "dob", "1980-01-01", Origin::Manual, Verification::AnalystAsserted).unwrap();
        graph::link(&mut app, p, o, RelationKind::MemberOf).unwrap();

        app.apply_label(p, "Renamed Person").unwrap();
        assert_eq!(graph::slug_of(&app, p).unwrap(), "renamed_person");
        let obs = graph::observations_of(&app, p).unwrap();
        assert_eq!(obs.len(), 1, "rename keeps observations");
        assert_eq!(obs[0].value.text().unwrap(), "1980-01-01");
        assert!(graph::has_edge(&app, p, o, RelationKind::MemberOf).unwrap(), "rename keeps edges");
        invariant::check_graph(&app).unwrap();
    }

    #[test]
    fn colliding_renames_round_trip_with_distinct_slugs() {
        let (td, project) = harness::temp_project().unwrap();
        let root = project.root.clone();
        let mut app = harness::app().unwrap();
        app.project = Some(project);
        let first = graph::add(&mut app, EntityKind::Person, "Shared Name").unwrap();
        let second = graph::add(&mut app, EntityKind::Person, "Distinct One").unwrap();
        app.apply_label(second, "Shared Name").unwrap();
        let s1 = graph::slug_of(&app, first).unwrap();
        let s2 = graph::slug_of(&app, second).unwrap();
        assert_ne!(s1, s2, "two same-label nodes must not share a slug");
        harness::save(&mut app).unwrap();

        let mut app2 = harness::app().unwrap();
        app2.project = Some(Project::open(&root).unwrap());
        harness::load(&mut app2).unwrap();
        let mut slugs = graph::slugs(&app2).unwrap();
        slugs.dedup();
        assert_eq!(slugs.len(), 2, "distinct slugs survive the round trip");
        invariant::check_graph(&app2).unwrap();
        drop(td);
    }

    #[test]
    fn repeated_save_load_cycles_keep_slugs_and_values_stable() {
        let (td, root, mut app) = rich_project_app();
        harness::save(&mut app).unwrap();
        let slugs0 = graph::slugs(&app).unwrap();

        let mut cur = app;
        for _ in 0..5 {
            let mut next = harness::app().unwrap();
            next.project = Some(Project::open(&root).unwrap());
            harness::load(&mut next).unwrap();
            harness::save(&mut next).unwrap();
            assert_eq!(graph::slugs(&next).unwrap(), slugs0, "slug set stays put across cycles");
            let doe = graph::node_by_slug(&next, "j_doe").unwrap().unwrap();
            assert_eq!(graph::value_of(&next, doe).unwrap(), "John Doe");
            invariant::check_graph(&next).unwrap();
            cur = next;
        }
        assert_eq!(graph::node_count(&cur).unwrap(), 2);
        drop(td);
    }
}

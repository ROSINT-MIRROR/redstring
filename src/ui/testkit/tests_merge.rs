#[cfg(test)]
mod tests {
    use crate::model::{EntityKind, Origin, RelationKind, SourceKind, Verification};
    use crate::ui::RedstringApp;
    use crate::ui::testkit::{graph, harness, invariant};

    #[test]
    fn merge_unions_observations_and_repoints_every_edge_and_marks_dirty() {
        let (td, project) = harness::temp_project().unwrap();
        let mut app = harness::app().unwrap();
        app.project = Some(project);

        let survivor = graph::add(&mut app, EntityKind::Person, "John Doe").unwrap();
        let absorbed = graph::add(&mut app, EntityKind::Person, "J Doe").unwrap();
        let org = graph::add(&mut app, EntityKind::Organization, "Shell Co").unwrap();
        let dom = graph::add(&mut app, EntityKind::Domain, "evil.example").unwrap();

        graph::observe(
            &mut app,
            survivor,
            "email",
            "a@x.com",
            Origin::Manual,
            Verification::AnalystAsserted,
        )
        .unwrap();
        graph::observe(
            &mut app,
            absorbed,
            "email",
            "a@x.com",
            Origin::Manual,
            Verification::AnalystAsserted,
        )
        .unwrap();
        graph::observe(
            &mut app,
            absorbed,
            "phone",
            "555-0100",
            Origin::Lookup("holehe".to_owned()),
            Verification::MachineSuggested,
        )
        .unwrap();

        graph::link(&mut app, survivor, org, RelationKind::Controls).unwrap();
        graph::link(&mut app, absorbed, dom, RelationKind::OwnsDomain).unwrap();
        graph::link(&mut app, org, absorbed, RelationKind::AssociateOf).unwrap();

        harness::save(&mut app).unwrap();
        assert!(!app.dirty().unwrap(), "a freshly saved case is clean");

        app.merge_entities(survivor, absorbed).unwrap();

        assert_eq!(graph::node_count(&app).unwrap(), 3, "the absorbed node is gone");
        assert!(
            graph::node_by_slug(&app, "j_doe").unwrap().is_none(),
            "the absorbed slug no longer resolves"
        );

        let obs = graph::observations_of(&app, survivor).unwrap();
        let emails = obs.iter().filter(|o| o.name == "email").count();
        assert_eq!(emails, 1, "an exact (name,value) duplicate observation is skipped");
        let phone = obs.iter().find(|o| o.name == "phone").expect("distinct observation adopted");
        assert!(
            matches!(phone.origin, Origin::Lookup(_)),
            "the adopted observation keeps its lookup provenance"
        );
        assert_eq!(
            phone.status,
            Verification::MachineSuggested,
            "the adopted observation keeps its verification status"
        );

        assert!(graph::has_edge(&app, survivor, org, RelationKind::Controls).unwrap());
        assert!(
            graph::has_edge(&app, survivor, dom, RelationKind::OwnsDomain).unwrap(),
            "an outgoing edge of the absorbed node is re-pointed off the survivor"
        );
        assert!(
            graph::has_edge(&app, org, survivor, RelationKind::AssociateOf).unwrap(),
            "an incoming edge of the absorbed node is re-pointed onto the survivor"
        );
        assert_eq!(graph::edge_count(&app).unwrap(), 3, "every relation survived, none duplicated");

        assert!(app.dirty().unwrap(), "the merge marks the case dirty");
        invariant::check_graph(&app).unwrap();
        drop(td);
    }

    #[test]
    fn merge_dedups_a_parallel_edge_and_unions_its_sources() {
        let mut app = harness::app().unwrap();
        let survivor = graph::add(&mut app, EntityKind::Person, "Survivor").unwrap();
        let absorbed = graph::add(&mut app, EntityKind::Person, "Absorbed").unwrap();
        let org = graph::add(&mut app, EntityKind::Organization, "Org").unwrap();

        let s1 = graph::source(SourceKind::Url, "https://a.example", "from survivor").unwrap();
        let s2 = graph::source(SourceKind::Url, "https://b.example", "from absorbed").unwrap();
        graph::link_sourced(&mut app, survivor, org, RelationKind::Controls, s1).unwrap();
        graph::link_sourced(&mut app, absorbed, org, RelationKind::Controls, s2).unwrap();
        assert_eq!(graph::edge_count(&app).unwrap(), 2);

        app.merge_entities(survivor, absorbed).unwrap();

        assert_eq!(
            graph::edge_count(&app).unwrap(),
            1,
            "the parallel duplicate (from,to,kind) edge is not added twice"
        );
        let srcs = graph::edge_sources(&app, survivor, org, RelationKind::Controls).unwrap();
        assert_eq!(srcs.len(), 2, "both edges' sources are unioned onto the surviving edge");
        assert!(srcs.iter().any(|s| s.reference == "https://a.example"));
        assert!(srcs.iter().any(|s| s.reference == "https://b.example"));
        invariant::check_graph(&app).unwrap();
    }

    #[test]
    fn merge_drops_the_edge_between_the_two_nodes_never_a_self_loop() {
        let mut app = harness::app().unwrap();
        let survivor = graph::add(&mut app, EntityKind::Person, "Survivor").unwrap();
        let absorbed = graph::add(&mut app, EntityKind::Person, "Absorbed").unwrap();
        graph::link(&mut app, survivor, absorbed, RelationKind::AssociateOf).unwrap();
        graph::link(&mut app, absorbed, survivor, RelationKind::AssociateOf).unwrap();
        assert_eq!(graph::edge_count(&app).unwrap(), 2);

        app.merge_entities(survivor, absorbed).unwrap();

        assert_eq!(graph::node_count(&app).unwrap(), 1, "the absorbed node is gone");
        assert_eq!(
            graph::edge_count(&app).unwrap(),
            0,
            "an edge between absorbed and survivor would be a self-loop, so it is dropped"
        );
        let g = app.g.g();
        for eidx in g.edge_indices() {
            let (a, b) = g.edge_endpoints(eidx).unwrap();
            assert_ne!(a, b, "the merge never leaves a self-loop");
        }
        invariant::check_graph(&app).unwrap();
    }

    #[test]
    fn multi_inspector_still_renders_after_extraction_without_side_effects() {
        let mut app = harness::app().unwrap();
        let a = graph::add(&mut app, EntityKind::Person, "A").unwrap();
        let b = graph::add(&mut app, EntityKind::Person, "B").unwrap();
        let c = graph::add(&mut app, EntityKind::Person, "C").unwrap();

        let sel3 = vec![a, b, c];
        harness::frame(&mut app, |app, ui| app.multi_inspector(ui, sel3.clone())).unwrap();
        assert!(app.merge_confirm.is_none(), "no merge is armed for a 3-entity selection");
        assert!(app.seal_confirm.is_none(), "rendering alone does not seal");
        assert!(app.delete_confirm.is_none(), "rendering alone does not delete");

        let sel2 = vec![a, b];
        harness::frame(&mut app, |app, ui| app.multi_inspector(ui, sel2.clone())).unwrap();
        assert!(app.merge_confirm.is_none(), "rendering the merge button alone does not trigger it");
        assert_eq!(graph::node_count(&app).unwrap(), 3, "nothing is merged by rendering");
        invariant::check_graph(&app).unwrap();
    }

    fn merge_escape(app: &mut RedstringApp) {
        let mut input = egui::RawInput::default();
        input.events.push(egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        });
        harness::frame_input(app, input, |a, ui| a.merge_confirm_window(ui)).unwrap();
    }

    #[test]
    fn merge_confirm_gate_holds_until_action_and_escape_cancels() {
        let mut app = harness::app().unwrap();
        let a = graph::add(&mut app, EntityKind::Person, "A").unwrap();
        let b = graph::add(&mut app, EntityKind::Person, "B").unwrap();
        app.merge_confirm = Some((a, b));

        harness::frame(&mut app, |app, ui| app.merge_confirm_window(ui)).unwrap();
        assert!(app.merge_confirm.is_some(), "an untouched confirm stays pending");
        assert_eq!(graph::node_count(&app).unwrap(), 2, "nothing merges before the analyst acts");

        merge_escape(&mut app);
        assert!(app.merge_confirm.is_none(), "escape dismisses the merge confirm");
        assert_eq!(graph::node_count(&app).unwrap(), 2, "cancelling keeps both entities");
        invariant::check_graph(&app).unwrap();
    }
}

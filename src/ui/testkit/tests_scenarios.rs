#[cfg(test)]
mod tests {
    use crate::model::{EntityKind, Origin, Relation, RelationKind, Verification};
    use crate::ui::testkit::{graph, harness, invariant};

    fn fold_once(app: &mut crate::ui::RedstringApp, idx: petgraph::graph::NodeIndex) {
        app.toggle_collapse(idx).unwrap();
        app.apply_fold().unwrap();
    }

    #[test]
    fn fold_repeated_toggle_returns_to_identical_state() {
        let mut app = harness::app().unwrap();
        let hz = graph::add(&mut app, EntityKind::Server, "hetzner").unwrap();
        let dash = graph::add(&mut app, EntityKind::Website, "dash.panel").unwrap();
        let ip = graph::add(&mut app, EntityKind::IpAddress, "ip.one").unwrap();
        graph::link(&mut app, dash, ip, RelationKind::ResolvesTo).unwrap();
        graph::link(&mut app, hz, ip, RelationKind::Hosts).unwrap();

        let mut folded_states: Vec<Vec<String>> = Vec::new();
        let mut shown_states: Vec<Vec<String>> = Vec::new();
        for _ in 0..8 {
            fold_once(&mut app, ip);
            folded_states.push(graph::hidden_slugs(&app).unwrap());
            fold_once(&mut app, ip);
            shown_states.push(graph::hidden_slugs(&app).unwrap());
        }
        assert!(
            folded_states.windows(2).all(|w| w[0] == w[1]),
            "collapse must hide the exact same set every cycle: {folded_states:?}"
        );
        assert!(
            shown_states.windows(2).all(|w| w[0] == w[1]),
            "expand must restore the exact same set every cycle: {shown_states:?}"
        );
        assert_eq!(folded_states[0], vec!["dash_panel".to_owned(), "hetzner".to_owned()]);
        assert!(shown_states[0].is_empty(), "expand leaves nothing hidden");
        invariant::check_graph(&app).unwrap();
    }

    #[test]
    fn fold_collapse_then_expand_subtree_is_identity() {
        let mut app = harness::app().unwrap();
        let a = graph::add(&mut app, EntityKind::Person, "a").unwrap();
        let b = graph::add(&mut app, EntityKind::Person, "b").unwrap();
        let c = graph::add(&mut app, EntityKind::Person, "c").unwrap();
        let d = graph::add(&mut app, EntityKind::Person, "d").unwrap();
        graph::link(&mut app, a, b, RelationKind::AssociateOf).unwrap();
        graph::link(&mut app, b, c, RelationKind::AssociateOf).unwrap();
        graph::link(&mut app, c, d, RelationKind::AssociateOf).unwrap();

        for _ in 0..6 {
            app.collapse_subtree(b).unwrap();
            app.apply_fold().unwrap();
            let folded = graph::hidden_slugs(&app).unwrap();
            assert_eq!(folded, vec!["a".to_owned()], "collapsing b folds the smaller side");
            app.expand_subtree(b).unwrap();
            app.apply_fold().unwrap();
            assert!(graph::hidden_slugs(&app).unwrap().is_empty());
            assert!(!graph::collapsed_of(&app, b).unwrap(), "expand clears the flag");
        }
        invariant::check_graph(&app).unwrap();
    }

    #[test]
    fn rename_regenerates_the_slug() {
        let mut app = harness::app().unwrap();
        let p = graph::add(&mut app, EntityKind::Person, "John Smith").unwrap();
        assert_eq!(graph::slug_of(&app, p).unwrap(), "john_smith");
        app.apply_label(p, "Jane Doe").unwrap();
        assert_eq!(graph::slug_of(&app, p).unwrap(), "jane_doe");
        assert_eq!(graph::label_of(&app, p).unwrap(), "Jane Doe");
        invariant::check_graph(&app).unwrap();
    }

    #[test]
    fn rename_into_a_collision_gets_a_unique_suffix() {
        let mut app = harness::app().unwrap();
        let _first = graph::add(&mut app, EntityKind::Person, "Chris Roe").unwrap();
        let second = graph::add(&mut app, EntityKind::Person, "Someone Else").unwrap();
        app.apply_label(second, "Chris Roe").unwrap();
        let s = graph::slug_of(&app, second).unwrap();
        assert_ne!(s, "chris_roe", "the second slug must not collide");
        assert!(s.starts_with("chris_roe_"), "collision resolves with a numeric suffix: {s}");
        invariant::check_graph(&app).unwrap();
    }

    #[test]
    fn observations_carry_full_provenance() {
        let mut app = harness::app().unwrap();
        let p = graph::add(&mut app, EntityKind::Person, "Target").unwrap();
        graph::observe(
            &mut app,
            p,
            "dob",
            "1971-03-04",
            Origin::Manual,
            Verification::AnalystAsserted,
        )
        .unwrap();
        let o = graph::observation(&app, p, "dob").unwrap().unwrap();
        assert_eq!(o.value.text().unwrap(), "1971-03-04");
        assert_eq!(o.status, Verification::AnalystAsserted);
        assert!(!o.id.0.is_empty(), "stamped with an id");
        assert!(o.created_at > 0, "stamped with a time");
        assert!(!o.author.name.is_empty(), "stamped with an author");
        invariant::check_graph(&app).unwrap();
    }

    #[test]
    fn manual_unsourced_edge_is_allowed() {
        let mut app = harness::app().unwrap();
        let a = graph::add(&mut app, EntityKind::Person, "A").unwrap();
        let b = graph::add(&mut app, EntityKind::Organization, "B Corp").unwrap();
        graph::link(&mut app, a, b, RelationKind::Controls).unwrap();
        invariant::check_graph(&app).unwrap();
    }

    #[test]
    fn checker_rejects_an_unsourced_lookup_edge() {
        let mut app = harness::app().unwrap();
        let a = graph::add(&mut app, EntityKind::Domain, "a.example").unwrap();
        let b = graph::add(&mut app, EntityKind::Organization, "Registrar Inc").unwrap();
        let rel = Relation::stamp(
            RelationKind::RegisteredAt,
            Origin::Lookup("whois".to_owned()),
            Verification::MachineSuggested,
            app.author.clone(),
        )
        .unwrap();
        app.g
            .add_edge_with_label(a, b, rel, "registered at".to_owned());
        assert!(
            invariant::check_graph(&app).is_err(),
            "an unsourced lookup edge must trip the invariant checker"
        );
    }

    #[test]
    fn checker_rejects_a_value_collision() {
        let mut app = harness::app().unwrap();
        graph::add_valued(&mut app, EntityKind::Domain, "Front Site", "same.example").unwrap();
        graph::add_valued(&mut app, EntityKind::Domain, "Back Site", "same.example").unwrap();
        assert!(
            invariant::check_graph(&app).is_err(),
            "two domains with one value is a duplicate the checker must catch"
        );
    }

    #[test]
    fn checker_rejects_a_slug_that_stops_tracking_its_label() {
        let mut app = harness::app().unwrap();
        let a = graph::add(&mut app, EntityKind::Person, "Alice Example").unwrap();
        app.g.node_mut(a).unwrap().payload_mut().slug = "totally_unrelated".to_owned();
        assert!(
            invariant::check_graph(&app).is_err(),
            "a slug divorced from its label must trip the checker"
        );
    }

    #[test]
    fn edge_relations_reports_every_stamped_edge() {
        let mut app = harness::app().unwrap();
        let a = graph::add(&mut app, EntityKind::Person, "A").unwrap();
        let b = graph::add(&mut app, EntityKind::Organization, "B Corp").unwrap();
        let c = graph::add(&mut app, EntityKind::Domain, "c.example").unwrap();
        graph::link(&mut app, a, b, RelationKind::Controls).unwrap();
        graph::link(&mut app, b, c, RelationKind::OwnsDomain).unwrap();
        let rels = graph::edge_relations(&app).unwrap();
        assert_eq!(rels.len(), 2);
        assert!(rels.iter().all(|r| r.origin == Origin::Manual));
        assert!(
            rels.iter().all(|r| !r.author.name.is_empty()),
            "every edge carries an author"
        );
        invariant::check_graph(&app).unwrap();
    }

    #[test]
    fn seed_demo_is_internally_consistent() {
        let mut app = harness::app().unwrap();
        app.seed_demo().unwrap();
        assert_eq!(graph::node_count(&app).unwrap(), 4);
        assert_eq!(graph::edge_count(&app).unwrap(), 3);
        invariant::check_graph(&app).unwrap();
    }

    #[test]
    fn preset_entities_and_every_layout_are_drivable() {
        use crate::model::PRESETS;
        use crate::ui::layout::LayoutKind;
        let mut app = harness::app().unwrap();
        let anchor = graph::add(&mut app, EntityKind::Person, "Anchor").unwrap();
        let p = app.add_preset(&PRESETS[0], graph::place(1).unwrap()).unwrap();
        graph::link(&mut app, anchor, p, RelationKind::AssociateOf).unwrap();
        for kind in [
            LayoutKind::Grid,
            LayoutKind::Circle,
            LayoutKind::Tree,
            LayoutKind::Force,
        ] {
            harness::layout(&mut app, kind).unwrap();
        }
        assert_eq!(graph::node_count(&app).unwrap(), 2);
        invariant::check_graph(&app).unwrap();
    }
}

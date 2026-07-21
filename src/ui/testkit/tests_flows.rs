#[cfg(test)]
mod tests {
    use crate::model::{EntityKind, KIND_META, Origin, RELATION_META, RelationKind, Verification};
    use crate::project::{Project, read_case};
    use crate::ui::layout::LayoutKind;
    use crate::ui::testkit::{apply, fixtures, graph, harness, invariant};

    fn layout_kind(seed: usize) -> LayoutKind {
        match seed % 4 {
            0 => LayoutKind::Grid,
            1 => LayoutKind::Circle,
            2 => LayoutKind::Tree,
            _ => LayoutKind::Force,
        }
    }

    #[test]
    fn a_long_derived_session_round_trips_and_validates() {
        for seed in 0..4usize {
            let (td, project) = harness::temp_project().unwrap();
            let root = project.root.clone();
            let mut app = harness::app().unwrap();
            app.project = Some(project);

            let mut nodes = Vec::new();
            let total = 9 + seed;
            for i in 0..total {
                let kind = KIND_META[(i * 5 + seed * 3) % KIND_META.len()].kind;
                let idx = graph::add(&mut app, kind, &format!("s{seed}-node-{i}")).unwrap();
                nodes.push(idx);
                if i >= 1 && i % 3 != 0 {
                    let k = RELATION_META[(i + seed) % RELATION_META.len()].kind;
                    graph::link(&mut app, nodes[i - 1], idx, k).unwrap();
                }
                if i % 2 == 0 {
                    graph::observe(
                        &mut app,
                        idx,
                        "seen",
                        &format!("val-{i}"),
                        Origin::Manual,
                        Verification::Unreviewed,
                    )
                    .unwrap();
                }
            }
            invariant::check_graph(&app).unwrap();

            for i in (0..nodes.len()).step_by(4) {
                app.toggle_collapse(nodes[i]).unwrap();
            }
            app.apply_fold().unwrap();
            invariant::check_graph(&app).unwrap();

            harness::layout(&mut app, layout_kind(seed)).unwrap();

            for i in (1..nodes.len()).step_by(3) {
                app.apply_label(nodes[i], &format!("renamed-{seed}-{}", i % 4)).unwrap();
                invariant::check_graph(&app).unwrap();
            }

            harness::save(&mut app).unwrap();
            invariant::check_case_dir(&Project::open(&root).unwrap()).unwrap();

            let mut app2 = harness::app().unwrap();
            app2.project = Some(Project::open(&root).unwrap());
            harness::load(&mut app2).unwrap();
            invariant::check_graph(&app2).unwrap();
            assert_eq!(
                graph::slugs(&app2).unwrap(),
                graph::slugs(&app).unwrap(),
                "seed {seed}: the reloaded slug set must match what was saved"
            );
            assert_eq!(graph::node_count(&app2).unwrap(), total);
            drop(td);
        }
    }

    #[test]
    fn folding_then_deleting_a_hidden_node_then_saving_stays_consistent() {
        let (td, project) = harness::temp_project().unwrap();
        let root = project.root.clone();
        let mut app = harness::app().unwrap();
        app.project = Some(project);

        let hub = graph::add(&mut app, EntityKind::Person, "hub").unwrap();
        let mut leaves = Vec::new();
        for i in 0..4usize {
            let l = graph::add(&mut app, EntityKind::Person, &format!("leaf {i}")).unwrap();
            graph::link(&mut app, hub, l, RelationKind::AssociateOf).unwrap();
            leaves.push(l);
        }
        app.toggle_collapse(hub).unwrap();
        app.apply_fold().unwrap();
        assert_eq!(graph::hidden_slugs(&app).unwrap().len(), 4, "collapsing the hub folds every leaf");

        let gone = app.g.remove_node(leaves[0]);
        assert!(gone.is_some());
        app.apply_fold().unwrap();
        invariant::check_graph(&app).unwrap();

        harness::save(&mut app).unwrap();
        let reopened = Project::open(&root).unwrap();
        invariant::check_case_dir(&reopened).unwrap();
        let cf = read_case(&reopened).unwrap();
        assert_eq!(cf.data.entities.len(), 4, "one hub plus three surviving leaves");
        assert!(cf.display.collapsed.contains("hub"), "the hub's collapse state persisted");
        drop(td);
    }

    #[test]
    fn a_lookup_then_fold_then_save_cycle_validates_clean() {
        let (td, project) = harness::temp_project().unwrap();
        let root = project.root.clone();
        let mut app = harness::app().unwrap();
        app.project = Some(project);

        let dom = graph::add_valued(&mut app, EntityKind::Domain, "evil-corp.example", "evil-corp.example").unwrap();
        let ron = fixtures::module_ron("dnsx").unwrap();
        let raw = fixtures::fixture("dnsx.jsonl").unwrap();
        apply::apply_to(&mut app, &ron, "dnsx", dom, &raw).unwrap();
        assert_eq!(graph::node_count(&app).unwrap(), 6);

        app.toggle_collapse(dom).unwrap();
        app.apply_fold().unwrap();
        assert_eq!(graph::hidden_slugs(&app).unwrap().len(), 5, "collapsing the domain folds its whole fan-out");

        harness::save(&mut app).unwrap();
        let reopened = Project::open(&root).unwrap();
        invariant::check_case_dir(&reopened).unwrap();

        let mut app2 = harness::app().unwrap();
        app2.project = Some(Project::open(&root).unwrap());
        harness::load(&mut app2).unwrap();
        let dom2 = graph::node_by_slug(&app2, "evil_corp_example").unwrap().unwrap();
        assert!(graph::collapsed_of(&app2, dom2).unwrap(), "fold state persists alongside lookup data");
        app2.apply_fold().unwrap();
        assert_eq!(graph::hidden_slugs(&app2).unwrap().len(), 5);
        invariant::check_graph(&app2).unwrap();
        drop(td);
    }

    #[test]
    fn interleaving_lookups_and_manual_edits_never_violates_invariants() {
        let mut app = harness::app().unwrap();
        let ron = fixtures::module_ron("dnsx").unwrap();
        let raw = fixtures::fixture("dnsx.jsonl").unwrap();
        for seed in 0..3usize {
            let host = format!("host{seed}.example");
            let dom = graph::add_valued(&mut app, EntityKind::Domain, &host, &host).unwrap();
            let person = graph::add(&mut app, EntityKind::Person, &format!("owner {seed}")).unwrap();
            graph::link(&mut app, person, dom, RelationKind::OwnsDomain).unwrap();
            apply::apply_to(&mut app, &ron, "dnsx", dom, &raw).unwrap();
            graph::observe(&mut app, dom, "note", &format!("checked {seed}"), Origin::Manual, Verification::AnalystAsserted).unwrap();
            app.apply_label(person, &format!("Owner Renamed {seed}")).unwrap();
            invariant::check_graph(&app).unwrap();
        }
        assert!(graph::node_count(&app).unwrap() > 6, "three domains plus a shared dnsx fan-out and owners");
        invariant::check_graph(&app).unwrap();
    }
}

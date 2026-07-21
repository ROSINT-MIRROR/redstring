#[cfg(test)]
mod tests {
    use petgraph::graph::NodeIndex;

    use crate::model::{EntityKind, RelationKind};
    use crate::ui::RedstringApp;
    use crate::ui::testkit::{graph, harness, invariant};

    fn fold_once(app: &mut RedstringApp, idx: NodeIndex) {
        app.toggle_collapse(idx).unwrap();
        app.apply_fold().unwrap();
    }

    fn set_collapsed(app: &mut RedstringApp, idx: NodeIndex, want: bool) {
        if graph::collapsed_of(app, idx).unwrap() != want {
            app.toggle_collapse(idx).unwrap();
        }
    }

    fn collapsed_slugs(app: &RedstringApp, nodes: &[NodeIndex]) -> Vec<String> {
        let mut out = Vec::new();
        for idx in nodes {
            if graph::collapsed_of(app, *idx).unwrap() {
                out.push(graph::slug_of(app, *idx).unwrap());
            }
        }
        out.sort();
        out
    }

    fn star(app: &mut RedstringApp, leaves: usize) -> (NodeIndex, Vec<NodeIndex>) {
        let center = graph::add(app, EntityKind::Person, "hub").unwrap();
        let mut ls = Vec::new();
        for i in 0..leaves {
            let l = graph::add(app, EntityKind::Person, &format!("leaf {i}")).unwrap();
            graph::link(app, center, l, RelationKind::AssociateOf).unwrap();
            ls.push(l);
        }
        (center, ls)
    }

    fn asymmetric_cluster(app: &mut RedstringApp) -> Vec<NodeIndex> {
        let r = graph::add(app, EntityKind::Person, "root").unwrap();
        let a = graph::add(app, EntityKind::Person, "branch a").unwrap();
        let b = graph::add(app, EntityKind::Person, "branch b").unwrap();
        let a1 = graph::add(app, EntityKind::Person, "a one").unwrap();
        let a2 = graph::add(app, EntityKind::Person, "a two").unwrap();
        let a3 = graph::add(app, EntityKind::Person, "a three").unwrap();
        let b1 = graph::add(app, EntityKind::Person, "b one").unwrap();
        graph::link(app, r, a, RelationKind::Controls).unwrap();
        graph::link(app, r, b, RelationKind::Controls).unwrap();
        graph::link(app, a, a1, RelationKind::Controls).unwrap();
        graph::link(app, a, a2, RelationKind::Controls).unwrap();
        graph::link(app, a, a3, RelationKind::Controls).unwrap();
        graph::link(app, b, b1, RelationKind::Controls).unwrap();
        vec![r, a, b, a1, a2, a3, b1]
    }

    #[test]
    fn collapsing_a_hub_folds_every_leaf_and_expand_restores_over_many_cycles() {
        let mut app = harness::app().unwrap();
        let (center, _leaves) = star(&mut app, 5);
        let mut folded: Vec<Vec<String>> = Vec::new();
        let mut shown: Vec<Vec<String>> = Vec::new();
        for _ in 0..12 {
            fold_once(&mut app, center);
            folded.push(graph::hidden_slugs(&app).unwrap());
            invariant::check_graph(&app).unwrap();
            fold_once(&mut app, center);
            shown.push(graph::hidden_slugs(&app).unwrap());
            invariant::check_graph(&app).unwrap();
        }
        assert!(folded.windows(2).all(|w| w[0] == w[1]), "fold set drifted: {folded:?}");
        assert!(shown.windows(2).all(|w| w[0] == w[1]), "expand set drifted: {shown:?}");
        assert_eq!(folded[0].len(), 5, "hub collapse hides all five leaves");
        assert!(shown[0].is_empty(), "expand leaves nothing hidden");
        invariant::check_graph(&app).unwrap();
    }

    #[test]
    fn nested_cluster_folds_only_the_smaller_branch_and_is_stable() {
        let mut app = harness::app().unwrap();
        let nodes = asymmetric_cluster(&mut app);
        let mut folds: Vec<Vec<String>> = Vec::new();
        for _ in 0..10 {
            fold_once(&mut app, nodes[0]);
            folds.push(graph::hidden_slugs(&app).unwrap());
            assert!(graph::collapsed_of(&app, nodes[0]).unwrap());
            fold_once(&mut app, nodes[0]);
            assert!(graph::hidden_slugs(&app).unwrap().is_empty());
            assert!(!graph::collapsed_of(&app, nodes[0]).unwrap());
        }
        assert!(folds.windows(2).all(|w| w[0] == w[1]));
        assert_eq!(folds[0], vec!["b_one".to_owned(), "branch_b".to_owned()]);
        invariant::check_graph(&app).unwrap();
    }

    #[test]
    fn collapse_all_expand_all_menu_path_never_leaves_stale_flags() {
        let mut app = harness::app().unwrap();
        let nodes = asymmetric_cluster(&mut app);
        for _ in 0..10 {
            app.collapse_subtree(nodes[0]).unwrap();
            app.apply_fold().unwrap();
            assert_eq!(
                graph::hidden_slugs(&app).unwrap(),
                vec!["b_one".to_owned(), "branch_b".to_owned()]
            );
            app.expand_subtree(nodes[0]).unwrap();
            app.apply_fold().unwrap();
            assert!(graph::hidden_slugs(&app).unwrap().is_empty());
            assert!(collapsed_slugs(&app, &nodes).is_empty(), "expand must clear every collapsed flag");
        }
        invariant::check_graph(&app).unwrap();
    }

    #[test]
    fn two_roots_collapsed_reach_the_same_final_state_regardless_of_order() {
        let mut app = harness::app().unwrap();
        let nodes = asymmetric_cluster(&mut app);
        let (r, a) = (nodes[0], nodes[1]);

        set_collapsed(&mut app, r, true);
        set_collapsed(&mut app, a, true);
        app.apply_fold().unwrap();
        let forward_hidden = graph::hidden_slugs(&app).unwrap();
        let forward_collapsed = collapsed_slugs(&app, &nodes);

        set_collapsed(&mut app, r, false);
        set_collapsed(&mut app, a, false);
        app.apply_fold().unwrap();
        assert!(graph::hidden_slugs(&app).unwrap().is_empty());

        set_collapsed(&mut app, a, true);
        set_collapsed(&mut app, r, true);
        app.apply_fold().unwrap();
        let reverse_hidden = graph::hidden_slugs(&app).unwrap();
        let reverse_collapsed = collapsed_slugs(&app, &nodes);

        assert_eq!(forward_hidden, reverse_hidden, "final hidden set must not depend on collapse order");
        assert_eq!(forward_collapsed, reverse_collapsed);
        assert_eq!(forward_hidden, vec!["a_one".to_owned(), "a_three".to_owned(), "a_two".to_owned()]);
        invariant::check_graph(&app).unwrap();
    }

    #[test]
    fn two_disjoint_components_fold_independently() {
        let mut app = harness::app().unwrap();
        let (hub1, _l1) = star(&mut app, 3);
        let n = graph::node_count(&app).unwrap();
        let x = graph::add(&mut app, EntityKind::Organization, "other x").unwrap();
        let y = graph::add(&mut app, EntityKind::Organization, "other y").unwrap();
        let z = graph::add(&mut app, EntityKind::Organization, "other z").unwrap();
        graph::link(&mut app, x, y, RelationKind::Controls).unwrap();
        graph::link(&mut app, y, z, RelationKind::Controls).unwrap();
        assert!(n > 0);
        for _ in 0..6 {
            fold_once(&mut app, hub1);
            let hidden = graph::hidden_slugs(&app).unwrap();
            assert!(!hidden.contains(&"other_x".to_owned()));
            assert!(!hidden.contains(&"other_y".to_owned()));
            assert!(!hidden.contains(&"other_z".to_owned()));
            fold_once(&mut app, hub1);
        }
        invariant::check_graph(&app).unwrap();
    }

    #[test]
    fn apply_fold_is_a_pure_function_of_the_collapsed_set() {
        let mut app = harness::app().unwrap();
        let nodes = asymmetric_cluster(&mut app);
        set_collapsed(&mut app, nodes[1], true);
        app.apply_fold().unwrap();
        let first = graph::hidden_slugs(&app).unwrap();
        app.apply_fold().unwrap();
        let second = graph::hidden_slugs(&app).unwrap();
        assert_eq!(first, second, "re-running apply_fold with the same flags must not change hidden");

        set_collapsed(&mut app, nodes[1], false);
        app.apply_fold().unwrap();
        assert!(graph::hidden_slugs(&app).unwrap().is_empty());
        set_collapsed(&mut app, nodes[1], true);
        app.apply_fold().unwrap();
        let third = graph::hidden_slugs(&app).unwrap();
        assert_eq!(first, third, "same collapsed set always derives the same hidden set");
        invariant::check_graph(&app).unwrap();
    }

    #[test]
    fn folding_then_mutating_then_expanding_stays_consistent() {
        let mut app = harness::app().unwrap();
        let nodes = asymmetric_cluster(&mut app);
        fold_once(&mut app, nodes[0]);
        let extra = graph::add(&mut app, EntityKind::Domain, "late.example").unwrap();
        graph::link(&mut app, nodes[2], extra, RelationKind::OwnsDomain).unwrap();
        app.apply_fold().unwrap();
        invariant::check_graph(&app).unwrap();
        set_collapsed(&mut app, nodes[0], false);
        app.apply_fold().unwrap();
        assert!(graph::hidden_slugs(&app).unwrap().is_empty(), "expanding after a mutation hides nothing");
        invariant::check_graph(&app).unwrap();
    }

    #[test]
    fn index_derived_fold_toggle_sequence_returns_to_the_baseline_graph() {
        let mut app = harness::app().unwrap();
        let mut nodes = Vec::new();
        for i in 0..12usize {
            nodes.push(graph::add(&mut app, EntityKind::Person, &format!("member {i}")).unwrap());
        }
        for i in 1..12usize {
            graph::link(&mut app, nodes[i - 1], nodes[i], RelationKind::AssociateOf).unwrap();
        }
        let baseline_slugs = graph::slugs(&app).unwrap();
        let baseline_edges = graph::edge_count(&app).unwrap();

        for i in 0..48usize {
            let idx = nodes[i % nodes.len()];
            fold_once(&mut app, idx);
            invariant::check_graph(&app).unwrap();
        }
        for idx in &nodes {
            set_collapsed(&mut app, *idx, false);
        }
        app.apply_fold().unwrap();

        assert!(graph::hidden_slugs(&app).unwrap().is_empty(), "all flags cleared leaves nothing hidden");
        assert_eq!(graph::slugs(&app).unwrap(), baseline_slugs, "the node set is untouched by folding");
        assert_eq!(graph::edge_count(&app).unwrap(), baseline_edges, "the edge set is untouched by folding");
        invariant::check_graph(&app).unwrap();
    }

    #[test]
    fn fold_state_survives_save_and_load() {
        let (td, project) = harness::temp_project().unwrap();
        let root = project.root.clone();
        let mut app = harness::app().unwrap();
        app.project = Some(project);
        let (center, _leaves) = star(&mut app, 4);
        fold_once(&mut app, center);
        let before = graph::hidden_slugs(&app).unwrap();
        assert_eq!(before.len(), 4);
        harness::save(&mut app).unwrap();

        let mut app2 = harness::app().unwrap();
        app2.project = Some(crate::project::Project::open(&root).unwrap());
        harness::load(&mut app2).unwrap();
        let center2 = graph::node_by_slug(&app2, "hub").unwrap().unwrap();
        assert!(graph::collapsed_of(&app2, center2).unwrap(), "collapse flag persisted through display.collapsed");
        app2.apply_fold().unwrap();
        assert_eq!(graph::hidden_slugs(&app2).unwrap(), before, "reloaded fold derives the identical hidden set");
        invariant::check_graph(&app2).unwrap();
        drop(td);
    }
}

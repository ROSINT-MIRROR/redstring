#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use petgraph::graph::NodeIndex;

    use crate::error::Error;
    use crate::model::{EntityKind, RelationKind};
    use crate::ui::RedstringApp;
    use crate::ui::layout::LayoutKind;
    use crate::ui::testkit::{graph, harness, invariant};

    fn run_at_time(
        app: &mut RedstringApp,
        time: f64,
        body: impl FnMut(&mut RedstringApp, &mut egui::Ui) -> Result<(), Error>,
    ) {
        let mut input = egui::RawInput::default();
        input.time = Some(time);
        harness::frame_input(app, input, body).unwrap();
    }

    fn settle(app: &mut RedstringApp, kind: LayoutKind) {
        run_at_time(app, 0.0, |a, ui| a.apply_layout(ui, kind));
        run_at_time(app, 1000.0, |a, ui| a.layout_anim_tick(ui));
    }

    fn all_locs(app: &RedstringApp) -> Vec<(usize, f32, f32)> {
        let mut out = Vec::new();
        for idx in app.g.g().node_indices() {
            let n = app.g.node(idx).unwrap();
            let p = n.location();
            out.push((idx.index(), p.x, p.y));
        }
        out.sort_by_key(|(i, _, _)| *i);
        out
    }

    fn loc_of(app: &RedstringApp, idx: NodeIndex) -> (f32, f32) {
        let p = app.g.node(idx).unwrap().location();
        (p.x, p.y)
    }

    fn step_targets(app: &RedstringApp) -> Vec<usize> {
        let anim = app.layout_anim.as_ref().unwrap();
        let mut out: Vec<usize> = anim.steps.iter().map(|(i, _, _)| i.index()).collect();
        out.sort();
        out
    }

    fn build_web(app: &mut RedstringApp) -> Vec<NodeIndex> {
        let kinds = [
            EntityKind::Person,
            EntityKind::Organization,
            EntityKind::Domain,
            EntityKind::IpAddress,
            EntityKind::Server,
            EntityKind::Website,
            EntityKind::Location,
            EntityKind::WebPage,
        ];
        let mut nodes = Vec::new();
        for (i, k) in kinds.iter().enumerate() {
            nodes.push(graph::add(app, *k, &format!("node {i}")).unwrap());
        }
        let e = [
            (0, 1, RelationKind::AssociateOf),
            (0, 2, RelationKind::Controls),
            (1, 3, RelationKind::AssociateOf),
            (1, 4, RelationKind::AssociateOf),
            (2, 5, RelationKind::Controls),
            (5, 6, RelationKind::AssociateOf),
            (2, 7, RelationKind::Controls),
        ];
        for (a, b, k) in e {
            graph::link(app, nodes[a], nodes[b], k).unwrap();
        }
        nodes
    }

    fn built() -> RedstringApp {
        let mut app = harness::app().unwrap();
        build_web(&mut app);
        app
    }

    fn assert_deterministic(kind: LayoutKind) {
        let mut a1 = built();
        settle(&mut a1, kind);
        let mut a2 = built();
        settle(&mut a2, kind);
        assert_eq!(all_locs(&a1), all_locs(&a2), "layout must reproduce identical positions");
        invariant::check_graph(&a1).unwrap();
    }

    #[test]
    fn grid_layout_is_deterministic_across_identical_builds() {
        assert_deterministic(LayoutKind::Grid);
    }

    #[test]
    fn circle_layout_is_deterministic_across_identical_builds() {
        assert_deterministic(LayoutKind::Circle);
    }

    #[test]
    fn tree_layout_is_deterministic_across_identical_builds() {
        assert_deterministic(LayoutKind::Tree);
    }

    #[test]
    fn force_layout_is_deterministic_across_identical_builds() {
        assert_deterministic(LayoutKind::Force);
    }

    #[test]
    fn re_running_a_layout_targets_the_same_node_set() {
        let mut app = built();
        settle(&mut app, LayoutKind::Grid);
        let n = graph::node_count(&app).unwrap();
        settle(&mut app, LayoutKind::Grid);
        assert_eq!(graph::node_count(&app).unwrap(), n, "re-running never drops or duplicates a node");
        invariant::check_graph(&app).unwrap();
    }

    #[test]
    fn every_layout_kind_settles_and_preserves_the_node_set() {
        for kind in [LayoutKind::Grid, LayoutKind::Circle, LayoutKind::Tree, LayoutKind::Force] {
            let mut app = built();
            let n = graph::node_count(&app).unwrap();
            settle(&mut app, kind);
            assert_eq!(graph::node_count(&app).unwrap(), n, "layout never adds or drops nodes");
            for (_, x, y) in all_locs(&app) {
                assert!(x.is_finite() && y.is_finite(), "layout produced a non-finite position");
            }
            assert!(app.layout_anim.is_none(), "a settled animation clears itself");
            invariant::check_graph(&app).unwrap();
        }
    }

    #[test]
    fn grid_positions_are_all_distinct() {
        let mut app = built();
        settle(&mut app, LayoutKind::Grid);
        let mut seen: HashSet<(i32, i32)> = HashSet::new();
        for (_, x, y) in all_locs(&app) {
            let key = ((x * 100.0) as i32, (y * 100.0) as i32);
            assert!(seen.insert(key), "grid layout overlapped two nodes at {key:?}");
        }
    }

    #[test]
    fn layout_targets_only_the_selection_and_leaves_the_rest_pinned() {
        let mut app = built();
        let nodes = build_web(&mut app);
        let selection = vec![nodes[3], nodes[4]];
        let before = all_locs(&app);
        app.g.set_selected_nodes(selection.clone());

        run_at_time(&mut app, 0.0, |a, ui| a.apply_layout(ui, LayoutKind::Grid));
        let mut want: Vec<usize> = selection.iter().map(|i| i.index()).collect();
        want.sort();
        assert_eq!(step_targets(&app), want, "only selected nodes get layout steps");
        assert!(app.status.contains("(selection)"), "status flags the selection scope");

        run_at_time(&mut app, 1000.0, |a, ui| a.layout_anim_tick(ui));
        let sel: HashSet<usize> = want.into_iter().collect();
        for (i, (idx, x, y)) in all_locs(&app).into_iter().enumerate() {
            let (bidx, bx, by) = before[i];
            assert_eq!(idx, bidx);
            if !sel.contains(&idx) {
                assert_eq!((x, y), (bx, by), "unselected node {idx} must not move");
            }
        }
        invariant::check_graph(&app).unwrap();
    }

    #[test]
    fn force_on_a_selection_holds_unselected_nodes_as_fixed_anchors() {
        let mut app = harness::app().unwrap();
        let anchor = graph::add(&mut app, EntityKind::Organization, "anchor").unwrap();
        let s0 = graph::add(&mut app, EntityKind::Person, "sel zero").unwrap();
        let s1 = graph::add(&mut app, EntityKind::Person, "sel one").unwrap();
        let far = graph::add(&mut app, EntityKind::Person, "far").unwrap();
        graph::link(&mut app, s0, s1, RelationKind::AssociateOf).unwrap();
        graph::link(&mut app, s1, anchor, RelationKind::AssociateOf).unwrap();
        graph::link(&mut app, far, anchor, RelationKind::AssociateOf).unwrap();
        let anchor_before = loc_of(&app, anchor);
        let far_before = loc_of(&app, far);

        app.g.set_selected_nodes(vec![s0, s1]);
        run_at_time(&mut app, 0.0, |a, ui| a.apply_layout(ui, LayoutKind::Force));
        assert_eq!(step_targets(&app), {
            let mut v = vec![s0.index(), s1.index()];
            v.sort();
            v
        });
        run_at_time(&mut app, 1000.0, |a, ui| a.layout_anim_tick(ui));

        assert_eq!(loc_of(&app, anchor), anchor_before, "the anchor node is fixed");
        assert_eq!(loc_of(&app, far), far_before, "an unselected node stays put");
        invariant::check_graph(&app).unwrap();
    }

    #[test]
    fn force_on_a_selection_is_reproducible_with_a_fixed_anchor() {
        let build = || {
            let mut app = harness::app().unwrap();
            let anchor = graph::add(&mut app, EntityKind::Organization, "anchor").unwrap();
            let s0 = graph::add(&mut app, EntityKind::Person, "sel zero").unwrap();
            let s1 = graph::add(&mut app, EntityKind::Person, "sel one").unwrap();
            graph::link(&mut app, s0, s1, RelationKind::AssociateOf).unwrap();
            graph::link(&mut app, s1, anchor, RelationKind::AssociateOf).unwrap();
            app.g.set_selected_nodes(vec![s0, s1]);
            (app, s0, s1)
        };
        let (mut a1, a1s0, a1s1) = build();
        settle(&mut a1, LayoutKind::Force);
        let (mut a2, a2s0, a2s1) = build();
        settle(&mut a2, LayoutKind::Force);
        assert_eq!(loc_of(&a1, a1s0), loc_of(&a2, a2s0), "anchored force is reproducible");
        assert_eq!(loc_of(&a1, a1s1), loc_of(&a2, a2s1));
    }

    #[test]
    fn dragging_a_node_by_hand_switches_the_layout_to_free() {
        for kind in [
            LayoutKind::Grid,
            LayoutKind::Circle,
            LayoutKind::Tree,
            LayoutKind::Force,
            LayoutKind::Radial,
        ] {
            let mut app = built();
            app.current_layout = kind;
            let idx = app.g.g().node_indices().next().unwrap();
            app.begin_node_drag(idx, egui::Pos2::new(10.0, 10.0)).unwrap();
            assert_eq!(
                app.current_layout,
                LayoutKind::Free,
                "a hand drag over the {kind:?} layout must switch the board to free"
            );
            assert!(
                app.status.contains("free"),
                "and must say so: {}",
                app.status
            );
        }
    }

    #[test]
    fn a_drag_that_grabs_nothing_leaves_the_layout_alone() {
        let mut app = built();
        app.current_layout = LayoutKind::Grid;
        app.begin_node_drag(NodeIndex::new(9_999), egui::Pos2::ZERO).unwrap();
        assert!(app.node_drag.is_none(), "no node was grabbed");
        assert_eq!(
            app.current_layout,
            LayoutKind::Grid,
            "a drag that grabbed nothing is not a hand placement"
        );
    }

    #[test]
    fn programmatic_movement_never_switches_the_layout_to_free() {
        let mut app = built();
        app.current_layout = LayoutKind::Radial;
        settle(&mut app, LayoutKind::Radial);
        assert_eq!(
            app.current_layout,
            LayoutKind::Radial,
            "running a layout and tweening it home is not a hand placement"
        );

        run_at_time(&mut app, 0.0, |a, ui| a.apply_spread(ui, crate::ui::layout::Spread::Loosen));
        run_at_time(&mut app, 1000.0, |a, ui| a.layout_anim_tick(ui));
        assert_eq!(
            app.current_layout,
            LayoutKind::Radial,
            "loosen/tighten re-space the board without claiming it was hand-placed"
        );

        let idx = app.g.g().node_indices().next().unwrap();
        app.g.node_mut(idx).unwrap().set_location(egui::Pos2::new(7.0, 9.0));
        assert_eq!(
            app.current_layout,
            LayoutKind::Radial,
            "moving a node from code is not a hand placement"
        );

        app.center_graph().unwrap();
        assert_eq!(app.current_layout, LayoutKind::Radial, "framing the board moves no node");
    }

    #[test]
    fn arriving_nodes_relayout_everything_unless_the_board_is_free() {
        for kind in [
            LayoutKind::Grid,
            LayoutKind::Circle,
            LayoutKind::Tree,
            LayoutKind::Force,
            LayoutKind::Radial,
        ] {
            let mut app = built();
            app.current_layout = kind;
            app.layout_anim = None;
            run_at_time(&mut app, 0.0, |a, ui| a.nodes_arrived(ui, 1, Vec::new()));
            let anim = app
                .layout_anim
                .as_ref()
                .unwrap_or_else(|| panic!("{kind:?} must relayout when nodes arrive"));
            assert_eq!(
                anim.steps.len(),
                graph::node_count(&app).unwrap(),
                "{kind:?} re-arranges the whole board, not just the newcomers"
            );
        }
    }

    #[test]
    fn a_free_board_only_spawns_the_newcomer_and_leaves_every_placed_node_put() {
        let mut app = built();
        app.current_layout = LayoutKind::Free;
        app.layout_anim = None;
        let before = all_locs(&app);
        let newcomer = graph::add(&mut app, EntityKind::Domain, "arrived.example").unwrap();
        let steps = vec![(newcomer, egui::Pos2::ZERO, egui::Pos2::new(240.0, 120.0))];

        run_at_time(&mut app, 0.0, |a, ui| a.nodes_arrived(ui, 1, steps.clone()));

        let anim = app.layout_anim.as_ref().expect("the newcomer still pops out");
        assert_eq!(anim.steps.len(), 1, "only the arriving node is animated");
        assert_eq!(anim.steps[0].0, newcomer, "and it is the one that arrived");

        run_at_time(&mut app, 1000.0, |a, ui| a.layout_anim_tick(ui));
        for (idx, x, y) in before {
            let now = all_locs(&app)
                .into_iter()
                .find(|(i, _, _)| *i == idx)
                .unwrap();
            assert_eq!(
                (now.1, now.2),
                (x, y),
                "a free board never moves a node that was already placed"
            );
        }
        assert_eq!(loc_of(&app, newcomer), (240.0, 120.0), "the newcomer reached its slot");
        invariant::check_graph(&app).unwrap();
    }

    #[test]
    fn running_the_free_layout_moves_nothing() {
        let mut app = built();
        let before = all_locs(&app);
        run_at_time(&mut app, 0.0, |a, ui| a.apply_layout(ui, LayoutKind::Free));
        assert!(app.layout_anim.is_none(), "free queues no animation");
        assert_eq!(all_locs(&app), before, "free leaves every node exactly where it is");
        assert!(app.status.contains("free layout"), "status: {}", app.status);
    }

    #[test]
    fn a_single_node_is_never_laid_out() {
        let mut app = harness::app().unwrap();
        graph::add(&mut app, EntityKind::Person, "solo").unwrap();
        run_at_time(&mut app, 0.0, |a, ui| a.apply_layout(ui, LayoutKind::Grid));
        assert!(app.layout_anim.is_none(), "no animation is queued for a single node");
        assert!(app.status.contains("2 nodes"), "status explains the minimum: {}", app.status);
    }
}

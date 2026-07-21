#[cfg(test)]
mod tests {
    use petgraph::graph::NodeIndex;

    use crate::model::{EntityKind, RelationKind};
    use crate::project::Project;
    use crate::ui::RedstringApp;
    use crate::ui::testkit::{apply, fixtures, graph, harness, invariant};

    fn remove(app: &mut RedstringApp, idx: NodeIndex) {
        let gone = app.g.remove_node(idx);
        assert!(gone.is_some(), "remove_node should return the removed node");
    }

    fn remove_all(app: &mut RedstringApp, list: &[NodeIndex]) {
        for idx in list {
            let _gone = app.g.remove_node(*idx);
        }
    }

    fn cites(app: &RedstringApp, idx: NodeIndex) -> bool {
        let g = app.g.g();
        for e in g.edge_indices() {
            let Some((a, b)) = g.edge_endpoints(e) else {
                continue;
            };
            if a == idx || b == idx {
                return true;
            }
        }
        false
    }

    #[test]
    fn deleting_a_middle_node_drops_both_of_its_edges() {
        let mut app = harness::app().unwrap();
        let a = graph::add(&mut app, EntityKind::Person, "A").unwrap();
        let b = graph::add(&mut app, EntityKind::Person, "B").unwrap();
        let c = graph::add(&mut app, EntityKind::Person, "C").unwrap();
        graph::link(&mut app, a, b, RelationKind::AssociateOf).unwrap();
        graph::link(&mut app, b, c, RelationKind::AssociateOf).unwrap();
        remove(&mut app, b);
        assert_eq!(graph::node_count(&app).unwrap(), 2);
        assert_eq!(graph::edge_count(&app).unwrap(), 0, "both edges through B are gone");
        assert!(!cites(&app, b), "no surviving edge references the removed node");
        invariant::check_graph(&app).unwrap();
    }

    #[test]
    fn deleting_a_leaf_leaves_the_rest_of_the_chain_intact() {
        let mut app = harness::app().unwrap();
        let a = graph::add(&mut app, EntityKind::Person, "A").unwrap();
        let b = graph::add(&mut app, EntityKind::Person, "B").unwrap();
        let c = graph::add(&mut app, EntityKind::Person, "C").unwrap();
        graph::link(&mut app, a, b, RelationKind::AssociateOf).unwrap();
        graph::link(&mut app, b, c, RelationKind::AssociateOf).unwrap();
        remove(&mut app, c);
        assert_eq!(graph::edge_count(&app).unwrap(), 1, "only C's edge is dropped");
        assert!(graph::has_edge(&app, a, b, RelationKind::AssociateOf).unwrap());
        invariant::check_graph(&app).unwrap();
    }

    #[test]
    fn band_deleting_a_selection_leaves_no_dangling_edges() {
        let mut app = harness::app().unwrap();
        let mut nodes = Vec::new();
        for i in 0..8usize {
            nodes.push(graph::add(&mut app, EntityKind::Person, &format!("n{i}")).unwrap());
        }
        for i in 1..8usize {
            graph::link(&mut app, nodes[i - 1], nodes[i], RelationKind::AssociateOf).unwrap();
        }
        let selection = vec![nodes[2], nodes[4], nodes[6]];
        app.g.set_selected_nodes(selection.clone());
        remove_all(&mut app, &selection);

        assert_eq!(graph::node_count(&app).unwrap(), 5);
        for idx in &selection {
            assert!(!cites(&app, *idx), "a removed node must have no surviving edges");
        }
        invariant::check_graph(&app).unwrap();
    }

    #[test]
    fn deleting_an_entity_prunes_its_directory_on_save() {
        let (td, project) = harness::temp_project().unwrap();
        let root = project.root.clone();
        let mut app = harness::app().unwrap();
        app.project = Some(project);
        let keep = graph::add(&mut app, EntityKind::Person, "Keeper").unwrap();
        let drop_it = graph::add(&mut app, EntityKind::Person, "Goner").unwrap();
        graph::add(&mut app, EntityKind::Organization, "Third").unwrap();
        assert!(keep.index() != drop_it.index());
        harness::save(&mut app).unwrap();
        assert!(Project::open(&root).unwrap().entity_dir("goner").unwrap().exists());

        remove(&mut app, drop_it);
        harness::save(&mut app).unwrap();
        let reopened = Project::open(&root).unwrap();
        assert!(!reopened.entity_dir("goner").unwrap().exists(), "the removed entity's dir is pruned");
        assert!(reopened.entity_dir("keeper").unwrap().exists(), "surviving entities keep their dirs");
        invariant::check_case_dir(&reopened).unwrap();
        drop(td);
    }

    #[test]
    fn deleting_every_node_saves_a_valid_empty_case() {
        let (td, project) = harness::temp_project().unwrap();
        let root = project.root.clone();
        let mut app = harness::app().unwrap();
        app.project = Some(project);
        let mut nodes = Vec::new();
        for i in 0..5usize {
            nodes.push(graph::add(&mut app, EntityKind::Person, &format!("p{i}")).unwrap());
        }
        for i in 1..5usize {
            graph::link(&mut app, nodes[i - 1], nodes[i], RelationKind::AssociateOf).unwrap();
        }
        harness::save(&mut app).unwrap();
        remove_all(&mut app, &nodes);
        assert_eq!(graph::node_count(&app).unwrap(), 0);
        assert_eq!(graph::edge_count(&app).unwrap(), 0);
        harness::save(&mut app).unwrap();

        let reopened = Project::open(&root).unwrap();
        let cf = crate::project::read_case(&reopened).unwrap();
        assert!(cf.data.entities.is_empty(), "an emptied case has no entity records");
        assert!(cf.data.relations.is_empty());
        invariant::check_case_dir(&reopened).unwrap();
        drop(td);
    }

    #[test]
    fn deleting_a_lookup_source_node_removes_its_created_edges() {
        let mut app = harness::app().unwrap();
        let dom = graph::add_valued(&mut app, EntityKind::Domain, "evil-corp.example", "evil-corp.example").unwrap();
        let ron = fixtures::module_ron("dnsx").unwrap();
        let raw = fixtures::fixture("dnsx.jsonl").unwrap();
        apply::apply_to(&mut app, &ron, "dnsx", dom, &raw).unwrap();
        assert_eq!(graph::edge_count(&app).unwrap(), 5);

        remove(&mut app, dom);
        assert_eq!(graph::edge_count(&app).unwrap(), 0, "every edge fanned out from the source is dropped");
        assert_eq!(graph::node_count(&app).unwrap(), 5, "the spawned nodes remain, now orphaned");
        assert!(!cites(&app, dom));
        invariant::check_graph(&app).unwrap();
    }

    fn run_with_escape(app: &mut RedstringApp) {
        let mut input = egui::RawInput::default();
        input.events.push(egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        });
        harness::frame_input(app, input, |a, ui| a.delete_confirm_window(ui)).unwrap();
    }

    #[test]
    fn the_confirm_gate_does_not_delete_without_action_and_escape_cancels() {
        let mut app = harness::app().unwrap();
        let a = graph::add(&mut app, EntityKind::Person, "Target").unwrap();
        app.delete_confirm = Some(vec![a]);

        harness::frame(&mut app, |app, ui| app.delete_confirm_window(ui)).unwrap();
        assert!(app.delete_confirm.is_some(), "an untouched confirm stays pending");
        assert_eq!(graph::node_count(&app).unwrap(), 1, "nothing is removed before the analyst acts");

        run_with_escape(&mut app);
        assert!(app.delete_confirm.is_none(), "escape dismisses the confirm");
        assert_eq!(graph::node_count(&app).unwrap(), 1, "cancelling keeps the entity");
        invariant::check_graph(&app).unwrap();
    }

    fn sealed(app: &RedstringApp, idx: NodeIndex) -> bool {
        app.g.g().node_weight(idx).unwrap().payload().sealed
    }

    #[test]
    fn bulk_seal_seals_every_selected_entity_then_unseal_clears_them() {
        let (td, project) = harness::temp_project().unwrap();
        let mut app = harness::app().unwrap();
        app.project = Some(project);
        let mut nodes = Vec::new();
        for i in 0..4usize {
            nodes.push(graph::add(&mut app, EntityKind::Person, &format!("victim{i}")).unwrap());
        }
        harness::save(&mut app).unwrap();
        assert!(!app.dirty().unwrap(), "a freshly saved case is clean");

        app.g.set_selected_nodes(nodes.clone());
        let n = app.apply_seal(&nodes, true).unwrap();
        assert_eq!(n, 4, "all four flipped from unsealed to sealed");
        for idx in &nodes {
            assert!(sealed(&app, *idx), "every selected entity is sealed");
        }
        assert!(app.dirty().unwrap(), "bulk seal marks the case dirty");

        harness::save(&mut app).unwrap();
        let again = app.apply_seal(&nodes, true).unwrap();
        assert_eq!(again, 0, "re-sealing an already-sealed selection changes nothing");
        assert!(!app.dirty().unwrap(), "an idempotent re-seal leaves the case clean");

        let cleared = app.apply_seal(&nodes, false).unwrap();
        assert_eq!(cleared, 4, "bulk unseal clears every flag");
        for idx in &nodes {
            assert!(!sealed(&app, *idx), "every selected entity is unsealed");
        }
        assert!(app.dirty().unwrap(), "bulk unseal marks the case dirty");
        invariant::check_graph(&app).unwrap();
        drop(td);
    }

    fn seal_escape(app: &mut RedstringApp) {
        let mut input = egui::RawInput::default();
        input.events.push(egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        });
        harness::frame_input(app, input, |a, ui| a.seal_confirm_window(ui)).unwrap();
    }

    #[test]
    fn bulk_seal_confirm_gate_holds_until_action_and_escape_cancels() {
        let mut app = harness::app().unwrap();
        let a = graph::add(&mut app, EntityKind::Person, "Target").unwrap();
        app.seal_confirm = Some((vec![a], true));

        harness::frame(&mut app, |app, ui| app.seal_confirm_window(ui)).unwrap();
        assert!(app.seal_confirm.is_some(), "an untouched confirm stays pending");
        assert!(!sealed(&app, a), "nothing is sealed before the analyst confirms");

        seal_escape(&mut app);
        assert!(app.seal_confirm.is_none(), "escape dismisses the seal confirm");
        assert!(!sealed(&app, a), "cancelling leaves the entity unsealed");
        invariant::check_graph(&app).unwrap();
    }
}

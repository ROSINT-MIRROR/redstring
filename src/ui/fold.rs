use std::collections::HashSet;

use egui::Rect;
use petgraph::Direction::Outgoing;
use petgraph::graph::{EdgeIndex, NodeIndex};

use crate::error::Error;

use super::app::{RedstringApp, RsGraph};

fn derive_hidden(g: &RsGraph) -> HashSet<NodeIndex> {
    let inner = g.g();
    let mut hidden = HashSet::new();
    for idx in inner.node_indices() {
        let Some(n) = inner.node_weight(idx) else {
            continue;
        };
        if !n.payload().collapsed {
            continue;
        }
        for child in inner.neighbors_directed(idx, Outgoing) {
            if child != idx {
                hidden.insert(child);
            }
        }
    }
    hidden
}

fn collect_subtree(g: &RsGraph, root: NodeIndex) -> Vec<NodeIndex> {
    let inner = g.g();
    let mut seen = HashSet::new();
    let mut stack = vec![root];
    let mut out = Vec::new();
    loop {
        let Some(idx) = stack.pop() else {
            break;
        };
        if !seen.insert(idx) {
            continue;
        }
        out.push(idx);
        for child in inner.neighbors_directed(idx, Outgoing) {
            if !seen.contains(&child) {
                stack.push(child);
            }
        }
    }
    out
}

impl RedstringApp {
    pub fn apply_fold(&mut self) -> Result<(), Error> {
        let hidden = derive_hidden(&self.g);
        let nodes: Vec<NodeIndex> = self.g.g().node_indices().collect();
        for idx in nodes {
            let h = hidden.contains(&idx);
            let Some(n) = self.g.node_mut(idx) else {
                continue;
            };
            n.payload_mut().hidden = h;
            if h && n.selected() {
                n.set_selected(false);
            }
        }
        let edges: Vec<EdgeIndex> = self.g.g().edge_indices().collect();
        for idx in edges {
            let Some((a, b)) = self.g.g().edge_endpoints(idx) else {
                continue;
            };
            let h = hidden.contains(&a) || hidden.contains(&b);
            let Some(ed) = self.g.edge_mut(idx) else {
                continue;
            };
            ed.payload_mut().hidden = h;
        }
        Ok(())
    }

    pub fn toggle_collapse(&mut self, idx: NodeIndex) -> Result<(), Error> {
        let children = self.g.g().neighbors_directed(idx, Outgoing).count();
        let Some(n) = self.g.node_mut(idx) else {
            return Ok(());
        };
        let e = n.payload_mut();
        e.collapsed = !e.collapsed;
        let now = e.collapsed;
        let label = e.label.clone();
        self.status = if now {
            format!("collapsed {label} — {children} direct child(ren) hidden")
        } else {
            format!("expanded {label} — {children} direct child(ren) shown")
        };
        Ok(())
    }

    pub fn collapse_subtree(&mut self, root: NodeIndex) -> Result<(), Error> {
        let subtree = collect_subtree(&self.g, root);
        let count = subtree.len();
        for idx in &subtree {
            let Some(n) = self.g.node_mut(*idx) else {
                continue;
            };
            n.payload_mut().collapsed = true;
        }
        self.status = format!("collapsed all — {count} node(s) in subtree folded");
        Ok(())
    }

    pub fn expand_subtree(&mut self, root: NodeIndex) -> Result<(), Error> {
        let subtree = collect_subtree(&self.g, root);
        let count = subtree.len();
        for idx in &subtree {
            let Some(n) = self.g.node_mut(*idx) else {
                continue;
            };
            n.payload_mut().collapsed = false;
        }
        self.status = format!("expanded all — {count} node(s) in subtree opened");
        Ok(())
    }

    pub fn fold_input(&mut self, ui: &egui::Ui, rect: Rect) -> Result<(), Error> {
        if self.label_edit.is_some() || self.node_menu.is_some() {
            return Ok(());
        }
        let (dbl, pointer) = ui.input(|i| {
            (
                i.pointer.button_double_clicked(egui::PointerButton::Primary),
                i.pointer.interact_pos(),
            )
        });
        if !dbl {
            return Ok(());
        }
        let Some(p) = pointer.filter(|p| rect.contains(*p)) else {
            return Ok(());
        };
        let Some(idx) = self.node_at_screen(ui, rect, p)? else {
            return Ok(());
        };
        self.toggle_collapse(idx)?;
        ui.ctx().request_repaint();
        Ok(())
    }

    pub fn fold_menu(&mut self, ui: &mut egui::Ui) -> Result<(), Error> {
        let Some((idx, at)) = self.node_menu else {
            return Ok(());
        };
        if self.g.node(idx).is_none() {
            self.node_menu = None;
            return Ok(());
        }
        let label = self.node_label(idx)?;
        let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));
        let mut collapse_all = false;
        let mut expand_all = false;

        let area = egui::Area::new(egui::Id::new("rs_node_menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(at)
            .constrain(true)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_max_width(200.0);
                    ui.label(egui::RichText::new(label).small().weak());
                    ui.separator();
                    if ui.button("Expand all").clicked() {
                        expand_all = true;
                    }
                    if ui.button("Collapse all").clicked() {
                        collapse_all = true;
                    }
                });
            });

        if collapse_all {
            self.collapse_subtree(idx)?;
        }
        if expand_all {
            self.expand_subtree(idx)?;
        }
        if escape || area.response.clicked_elsewhere() || collapse_all || expand_all {
            self.node_menu = None;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use egui::Pos2;
    use egui_graphs::Graph;
    use petgraph::graph::NodeIndex;
    use petgraph::stable_graph::StableGraph;

    use super::super::app::RsGraph;
    use super::{collect_subtree, derive_hidden};
    use crate::model::{
        Author, Entity, EntityKind, DEFAULT_NODE_SIZE, Origin, Relation, RelationKind, Verification,
    };

    fn node(slug: &str) -> Entity {
        Entity {
            slug: slug.into(),
            kind: EntityKind::Person,
            label: slug.into(),
            value: String::new(),
            observations: Vec::new(),
            note: String::new(),
            image: None,
            size: DEFAULT_NODE_SIZE,
            boost: 1.0,
            collapsed: false,
            hidden: false,
        }
    }

    fn link(g: &mut RsGraph, a: NodeIndex, b: NodeIndex) {
        g.add_edge_with_label(
            a,
            b,
            Relation::stamp(
                RelationKind::Controls,
                Origin::Manual,
                Verification::Unreviewed,
                Author::default(),
            )
            .unwrap(),
            "controls".into(),
        );
    }

    fn chain() -> (RsGraph, Vec<NodeIndex>) {
        let mut g: RsGraph = Graph::from(&StableGraph::default());
        let mut ids = Vec::new();
        for name in ["a", "b", "c", "d"] {
            ids.push(g.add_node_with_label_and_location(node(name), name.into(), Pos2::ZERO));
        }
        for w in ids.windows(2) {
            link(&mut g, w[0], w[1]);
        }
        (g, ids)
    }

    fn set_collapsed(g: &mut RsGraph, idx: NodeIndex, on: bool) {
        if let Some(n) = g.node_mut(idx) {
            n.payload_mut().collapsed = on;
        }
    }

    fn hidden_slugs(g: &RsGraph) -> Vec<String> {
        let hidden = derive_hidden(g);
        let mut out: Vec<String> = hidden
            .iter()
            .filter_map(|idx| g.node(*idx).map(|n| n.payload().slug.clone()))
            .collect();
        out.sort();
        out
    }

    #[test]
    fn double_click_hides_one_level_only() {
        let (mut g, ids) = chain();
        set_collapsed(&mut g, ids[0], true);
        assert_eq!(hidden_slugs(&g), vec!["b".to_string()]);
        set_collapsed(&mut g, ids[0], false);
        assert!(hidden_slugs(&g).is_empty());
    }

    #[test]
    fn nested_collapse_stays_folded_on_parent_expand() {
        let (mut g, ids) = chain();
        set_collapsed(&mut g, ids[0], true);
        set_collapsed(&mut g, ids[2], true);
        assert_eq!(hidden_slugs(&g), vec!["b".to_string(), "d".to_string()]);
        set_collapsed(&mut g, ids[0], false);
        assert_eq!(hidden_slugs(&g), vec!["d".to_string()]);
    }

    #[test]
    fn collapse_all_hides_whole_subtree() {
        let (mut g, ids) = chain();
        let subtree = collect_subtree(&g, ids[0]);
        assert_eq!(subtree.len(), 4);
        for idx in &subtree {
            set_collapsed(&mut g, *idx, true);
        }
        assert_eq!(
            hidden_slugs(&g),
            vec!["b".to_string(), "c".to_string(), "d".to_string()]
        );
        for idx in &subtree {
            set_collapsed(&mut g, *idx, false);
        }
        assert!(hidden_slugs(&g).is_empty());
    }

    #[test]
    fn subtree_from_leaf_is_single_node_and_cycles_terminate() {
        let mut g: RsGraph = Graph::from(&StableGraph::default());
        let a = g.add_node_with_label_and_location(node("a"), "a".into(), Pos2::ZERO);
        let b = g.add_node_with_label_and_location(node("b"), "b".into(), Pos2::ZERO);
        link(&mut g, a, b);
        link(&mut g, b, a);
        assert_eq!(collect_subtree(&g, b).len(), 2);
        set_collapsed(&mut g, a, true);
        assert_eq!(hidden_slugs(&g), vec!["b".to_string()]);
    }
}

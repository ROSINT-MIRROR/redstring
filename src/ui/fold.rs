use std::collections::HashSet;

use egui::Rect;
use petgraph::graph::{EdgeIndex, NodeIndex};

use crate::error::Error;

use super::app::{RedstringApp, RsGraph};

fn collapsed_set(g: &RsGraph) -> HashSet<NodeIndex> {
    let inner = g.g();
    let mut out = HashSet::new();
    for idx in inner.node_indices() {
        let Some(n) = inner.node_weight(idx) else {
            continue;
        };
        if n.payload().collapsed {
            out.insert(idx);
        }
    }
    out
}

fn undirected_components(g: &RsGraph) -> Vec<Vec<NodeIndex>> {
    let inner = g.g();
    let mut seen: HashSet<NodeIndex> = HashSet::new();
    let mut comps: Vec<Vec<NodeIndex>> = Vec::new();
    for start in inner.node_indices() {
        if seen.contains(&start) {
            continue;
        }
        let mut members = Vec::new();
        let mut stack = vec![start];
        loop {
            let Some(idx) = stack.pop() else {
                break;
            };
            if !seen.insert(idx) {
                continue;
            }
            members.push(idx);
            for nb in inner.neighbors_undirected(idx) {
                if !seen.contains(&nb) {
                    stack.push(nb);
                }
            }
        }
        comps.push(members);
    }
    comps
}

fn subcomponents(
    g: &RsGraph,
    members: &[NodeIndex],
    collapsed: &HashSet<NodeIndex>,
) -> Vec<Vec<NodeIndex>> {
    let inner = g.g();
    let mut seen: HashSet<NodeIndex> = HashSet::new();
    let mut subs: Vec<Vec<NodeIndex>> = Vec::new();
    for &start in members {
        if collapsed.contains(&start) || seen.contains(&start) {
            continue;
        }
        let mut sub_members = Vec::new();
        let mut stack = vec![start];
        loop {
            let Some(idx) = stack.pop() else {
                break;
            };
            if !seen.insert(idx) {
                continue;
            }
            sub_members.push(idx);
            for nb in inner.neighbors_undirected(idx) {
                if !collapsed.contains(&nb) && !seen.contains(&nb) {
                    stack.push(nb);
                }
            }
        }
        subs.push(sub_members);
    }
    subs
}

fn pick_board(subs: &[Vec<NodeIndex>]) -> Option<usize> {
    let mut board = None;
    let mut max_size = 0usize;
    let mut ties = 0usize;
    for (sid, sub_members) in subs.iter().enumerate() {
        let size = sub_members.len();
        if size > max_size {
            max_size = size;
            board = Some(sid);
            ties = 1;
        } else if size == max_size {
            ties += 1;
        }
    }
    if max_size >= 2 && ties == 1 {
        board
    } else {
        None
    }
}

fn hidden_for(g: &RsGraph, collapsed: &HashSet<NodeIndex>) -> HashSet<NodeIndex> {
    let mut hidden = HashSet::new();
    if collapsed.is_empty() {
        return hidden;
    }
    for members in undirected_components(g) {
        let has_k = members.iter().any(|idx| collapsed.contains(idx));
        if !has_k {
            continue;
        }
        let subs = subcomponents(g, &members, collapsed);
        let board = pick_board(&subs);
        for (sid, sub_members) in subs.iter().enumerate() {
            if Some(sid) == board {
                continue;
            }
            for &idx in sub_members {
                hidden.insert(idx);
            }
        }
    }
    hidden
}

fn derive_hidden(g: &RsGraph) -> HashSet<NodeIndex> {
    let collapsed = collapsed_set(g);
    hidden_for(g, &collapsed)
}

fn set_collapsed_flag(g: &mut RsGraph, idx: NodeIndex, value: bool) {
    let Some(n) = g.node_mut(idx) else {
        return;
    };
    n.payload_mut().collapsed = value;
}

fn collapse_root(g: &mut RsGraph, root: NodeIndex) -> usize {
    let before = hidden_for(g, &collapsed_set(g));
    set_collapsed_flag(g, root, true);
    let after = hidden_for(g, &collapsed_set(g));
    after.difference(&before).count()
}

fn expand_root(g: &mut RsGraph, root: NodeIndex) -> usize {
    let before = hidden_for(g, &collapsed_set(g));
    set_collapsed_flag(g, root, false);
    let after = hidden_for(g, &collapsed_set(g));
    before.difference(&after).count()
}

fn fold_target(double: bool, over: Option<NodeIndex>) -> Option<NodeIndex> {
    if !double {
        return None;
    }
    over
}

fn menu_should_close(escape: bool, clicked_elsewhere: bool, acted: bool, just_opened: bool) -> bool {
    escape || acted || (clicked_elsewhere && !just_opened)
}

fn node_slug_str(g: &RsGraph, idx: NodeIndex) -> String {
    let Some(n) = g.node(idx) else {
        return String::new();
    };
    n.payload().slug.clone()
}

fn slug_list(g: &RsGraph, set: &HashSet<NodeIndex>) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for idx in set {
        let Some(n) = g.node(*idx) else {
            continue;
        };
        v.push(n.payload().slug.clone());
    }
    v.sort();
    v
}

fn currently_hidden(g: &RsGraph) -> HashSet<NodeIndex> {
    let inner = g.g();
    let mut out = HashSet::new();
    for idx in inner.node_indices() {
        let Some(n) = inner.node_weight(idx) else {
            continue;
        };
        if n.payload().hidden {
            out.insert(idx);
        }
    }
    out
}

fn collapsed_with_root(g: &RsGraph, root: NodeIndex) -> HashSet<NodeIndex> {
    let mut c = collapsed_set(g);
    c.insert(root);
    c
}

fn stale_collapsed(
    before: &HashSet<NodeIndex>,
    after: &HashSet<NodeIndex>,
    root: NodeIndex,
) -> HashSet<NodeIndex> {
    let mut expected = before.clone();
    expected.remove(&root);
    after.difference(&expected).copied().collect()
}

fn log_components(g: &RsGraph, collapsed: &HashSet<NodeIndex>, op: &str, root: &str) {
    if !crate::log::ENABLED.load(std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    for members in undirected_components(g) {
        let has_k = members.iter().any(|idx| collapsed.contains(idx));
        if !has_k {
            crate::dlog_debug!(
                "fold",
                "{op} root={root} component sz={} no-collapsed-member skipped",
                members.len()
            );
            continue;
        }
        let subs = subcomponents(g, &members, collapsed);
        let board = pick_board(&subs);
        let mut would_hide = 0usize;
        for (sid, m) in subs.iter().enumerate() {
            if Some(sid) != board {
                would_hide += m.len();
            }
        }
        crate::dlog_info!(
            "fold",
            "{op} root={root} component sz={} subs={} board_sz={:?} would_hide={would_hide}",
            members.len(),
            subs.len(),
            board.map(|b| subs[b].len())
        );
    }
}

impl RedstringApp {
    fn node_caption(&self, idx: NodeIndex) -> String {
        let Some(n) = self.g.node(idx) else {
            return String::new();
        };
        n.payload().label.clone()
    }

    pub fn apply_fold(&mut self) -> Result<(), Error> {
        let hidden = derive_hidden(&self.g);
        let prior = currently_hidden(&self.g);
        if hidden != prior {
            crate::dlog_debug!(
                "fold",
                "apply_fold hidden {}->{} now={:?}",
                prior.len(),
                hidden.len(),
                slug_list(&self.g, &hidden)
            );
        }
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
        let before = collapsed_set(&self.g);
        let slug = node_slug_str(&self.g, idx);
        let Some(n) = self.g.node_mut(idx) else {
            return Ok(());
        };
        let e = n.payload_mut();
        e.collapsed = !e.collapsed;
        let now = e.collapsed;
        let label = e.label.clone();
        let after = collapsed_set(&self.g);
        let hidden = hidden_for(&self.g, &after);
        crate::dlog_info!(
            "fold",
            "toggle slug={slug} collapsed={now} before={:?} after={:?} hidden={:?}",
            slug_list(&self.g, &before),
            slug_list(&self.g, &after),
            slug_list(&self.g, &hidden)
        );
        log_components(&self.g, &after, "toggle", &slug);
        self.status = if now {
            format!("collapsed {label}")
        } else {
            format!("expanded {label}")
        };
        Ok(())
    }

    pub fn collapse_subtree(&mut self, root: NodeIndex) -> Result<(), Error> {
        let before = collapsed_set(&self.g);
        let root_slug = node_slug_str(&self.g, root);
        let label = self.node_caption(root);
        crate::dlog_info!(
            "fold",
            "collapse_all root={root_slug} before={:?}",
            slug_list(&self.g, &before)
        );
        log_components(&self.g, &collapsed_with_root(&self.g, root), "collapse_all", &root_slug);
        let count = collapse_root(&mut self.g, root);
        let after = collapsed_set(&self.g);
        crate::dlog_info!(
            "fold",
            "collapse_all root={root_slug} after={:?} folded={count}",
            slug_list(&self.g, &after)
        );
        self.status = format!("collapsed {label} — {count} node(s) folded");
        Ok(())
    }

    pub fn expand_subtree(&mut self, root: NodeIndex) -> Result<(), Error> {
        let before = collapsed_set(&self.g);
        let root_slug = node_slug_str(&self.g, root);
        let label = self.node_caption(root);
        crate::dlog_info!(
            "fold",
            "expand_all root={root_slug} before={:?}",
            slug_list(&self.g, &before)
        );
        log_components(&self.g, &before, "expand_all", &root_slug);
        let count = expand_root(&mut self.g, root);
        let after = collapsed_set(&self.g);
        let stale = stale_collapsed(&before, &after, root);
        if stale.is_empty() {
            crate::dlog_info!(
                "fold",
                "expand_all root={root_slug} after={:?} shown={count}",
                slug_list(&self.g, &after)
            );
        } else {
            crate::dlog_warn!(
                "fold",
                "expand_all root={root_slug} left {} STALE collapsed flags {:?} — expand must clear only the root from the explicit-collapse set",
                stale.len(),
                slug_list(&self.g, &stale)
            );
        }
        self.status = format!("expanded {label} — {count} node(s) shown");
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
        let Some(p) = pointer.filter(|p| rect.contains(*p)) else {
            return Ok(());
        };
        let over = self.node_at_screen(ui, rect, p)?;
        let Some(idx) = fold_target(dbl, over) else {
            return Ok(());
        };
        crate::dlog_debug!("fold", "double-click toggle on {}", node_slug_str(&self.g, idx));
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
            self.node_menu_just_opened = false;
            return Ok(());
        }
        let just_opened = std::mem::take(&mut self.node_menu_just_opened);
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
            crate::dlog_info!("fold", "menu collapse-all on {}", node_slug_str(&self.g, idx));
            self.collapse_subtree(idx)?;
        }
        if expand_all {
            crate::dlog_info!("fold", "menu expand-all on {}", node_slug_str(&self.g, idx));
            self.expand_subtree(idx)?;
        }
        let acted = collapse_all || expand_all;
        if acted {
            ui.ctx().request_repaint();
        }
        let elsewhere = area.response.clicked_elsewhere();
        if menu_should_close(escape, elsewhere, acted, just_opened) {
            self.node_menu = None;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use egui::Pos2;
    use egui_graphs::Graph;
    use petgraph::graph::NodeIndex;
    use petgraph::stable_graph::StableGraph;

    use super::super::app::RsGraph;
    use super::{
        collapse_root, collapsed_set, derive_hidden, expand_root, fold_target, menu_should_close,
        stale_collapsed,
    };
    use crate::model::{
        Author, Entity, EntityKind, DEFAULT_NODE_SIZE, Origin, Relation, RelationKind, Role,
        Verification,
    };

    fn node(slug: &str) -> Entity {
        Entity {
            slug: slug.into(),
            kind: EntityKind::Person,
            label: slug.into(),
            value: String::new(),
            sealed: false,
            observations: Vec::new(),
            role: Role::Unset,
            occupation: None,
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

    fn spawn(g: &mut RsGraph, name: &str) -> NodeIndex {
        g.add_node_with_label_and_location(node(name), name.into(), Pos2::ZERO)
    }

    fn chain() -> (RsGraph, Vec<NodeIndex>) {
        let mut g: RsGraph = Graph::from(&StableGraph::default());
        let mut ids = Vec::new();
        for name in ["a", "b", "c", "d"] {
            ids.push(spawn(&mut g, name));
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
    fn fold_target_needs_double_click_over_a_node() {
        let (_g, ids) = chain();
        assert_eq!(fold_target(true, Some(ids[0])), Some(ids[0]));
        assert_eq!(fold_target(true, None), None);
        assert_eq!(fold_target(false, Some(ids[0])), None);
    }

    #[test]
    fn menu_stays_on_the_frame_it_opens_then_closes_on_outside_click() {
        assert!(!menu_should_close(false, true, false, true));
        assert!(menu_should_close(false, true, false, false));
        assert!(menu_should_close(true, false, false, false));
        assert!(menu_should_close(false, false, true, false));
        assert!(!menu_should_close(false, false, false, false));
    }

    #[test]
    fn collapsing_a_pure_sink_hides_both_incoming_neighbours() {
        let mut g: RsGraph = Graph::from(&StableGraph::default());
        let hetzner = spawn(&mut g, "hetzner");
        let dash = spawn(&mut g, "dash");
        let ip = spawn(&mut g, "ip");
        link(&mut g, dash, ip);
        link(&mut g, hetzner, ip);
        set_collapsed(&mut g, ip, true);
        assert_eq!(
            hidden_slugs(&g),
            vec!["dash".to_string(), "hetzner".to_string()]
        );
        set_collapsed(&mut g, ip, false);
        assert!(hidden_slugs(&g).is_empty());
    }

    #[test]
    fn multi_parent_board_survives_and_a_dangling_leaf_folds() {
        let mut g: RsGraph = Graph::from(&StableGraph::default());
        let a = spawn(&mut g, "a");
        let b = spawn(&mut g, "b");
        let c = spawn(&mut g, "c");
        let d = spawn(&mut g, "d");
        link(&mut g, a, b);
        link(&mut g, b, c);
        link(&mut g, d, c);
        set_collapsed(&mut g, a, true);
        assert!(hidden_slugs(&g).is_empty());
        set_collapsed(&mut g, a, false);
        set_collapsed(&mut g, b, true);
        assert_eq!(hidden_slugs(&g), vec!["a".to_string()]);
    }

    #[test]
    fn direction_agnostic_leaf_folds_nothing_middle_folds_both_ends() {
        let mut g: RsGraph = Graph::from(&StableGraph::default());
        let a = spawn(&mut g, "a");
        let b = spawn(&mut g, "b");
        let c = spawn(&mut g, "c");
        link(&mut g, a, b);
        link(&mut g, b, c);
        assert!(hidden_slugs(&g).is_empty());
        set_collapsed(&mut g, a, true);
        assert!(hidden_slugs(&g).is_empty());
        set_collapsed(&mut g, a, false);
        set_collapsed(&mut g, b, true);
        assert_eq!(hidden_slugs(&g), vec!["a".to_string(), "c".to_string()]);
    }

    #[test]
    fn unrelated_component_stays_visible_when_collapsing_elsewhere() {
        let mut g: RsGraph = Graph::from(&StableGraph::default());
        let x = spawn(&mut g, "x");
        let y = spawn(&mut g, "y");
        let z = spawn(&mut g, "z");
        let p = spawn(&mut g, "p");
        let q = spawn(&mut g, "q");
        link(&mut g, x, y);
        link(&mut g, y, z);
        link(&mut g, p, q);
        set_collapsed(&mut g, x, true);
        let hidden = hidden_slugs(&g);
        assert!(!hidden.contains(&"p".to_string()));
        assert!(!hidden.contains(&"q".to_string()));
        assert!(hidden.is_empty());
    }

    #[test]
    fn isolated_two_cycle_collapse_folds_partner_and_terminates() {
        let mut g: RsGraph = Graph::from(&StableGraph::default());
        let a = spawn(&mut g, "a");
        let b = spawn(&mut g, "b");
        link(&mut g, a, b);
        link(&mut g, b, a);
        set_collapsed(&mut g, a, true);
        assert_eq!(hidden_slugs(&g), vec!["b".to_string()]);
    }

    fn collapsed_slugs(g: &RsGraph) -> Vec<String> {
        let mut v: Vec<String> = collapsed_set(g)
            .iter()
            .filter_map(|idx| g.node(*idx).map(|n| n.payload().slug.clone()))
            .collect();
        v.sort();
        v
    }

    #[test]
    fn menu_collapse_then_expand_is_identity_from_a_clean_board() {
        let (mut g, ids) = chain();
        let root = ids[1];
        let c0 = collapsed_set(&g);
        let h0 = derive_hidden(&g);
        let folded = collapse_root(&mut g, root);
        assert_eq!(folded, 1);
        assert_eq!(collapsed_slugs(&g), vec!["b".to_string()]);
        let shown = expand_root(&mut g, root);
        assert_eq!(shown, 1);
        assert_eq!(collapsed_set(&g), c0);
        assert_eq!(derive_hidden(&g), h0);
    }

    #[test]
    fn menu_collapse_then_expand_leaves_unrelated_collapses_untouched() {
        let mut g: RsGraph = Graph::from(&StableGraph::default());
        let a = spawn(&mut g, "a");
        let b = spawn(&mut g, "b");
        let c = spawn(&mut g, "c");
        let x = spawn(&mut g, "x");
        let y = spawn(&mut g, "y");
        let z = spawn(&mut g, "z");
        link(&mut g, a, b);
        link(&mut g, b, c);
        link(&mut g, x, y);
        link(&mut g, y, z);
        set_collapsed(&mut g, y, true);
        let c0 = collapsed_set(&g);
        let h0 = derive_hidden(&g);
        let folded = collapse_root(&mut g, b);
        assert_eq!(folded, 2);
        let shown = expand_root(&mut g, b);
        assert_eq!(shown, 2);
        assert_eq!(collapsed_set(&g), c0);
        assert_eq!(derive_hidden(&g), h0);
        assert!(collapsed_set(&g).contains(&y));
    }

    #[test]
    fn repeated_collapse_expand_never_accumulates_stale_flags() {
        let mut g: RsGraph = Graph::from(&StableGraph::default());
        let hetzner = spawn(&mut g, "hetzner");
        let dash = spawn(&mut g, "dash");
        let ip = spawn(&mut g, "ip");
        link(&mut g, dash, ip);
        link(&mut g, hetzner, ip);
        let mut folds: Vec<Vec<String>> = Vec::new();
        let mut shows: Vec<Vec<String>> = Vec::new();
        for _ in 0..5 {
            let folded = collapse_root(&mut g, ip);
            assert_eq!(folded, 2);
            assert_eq!(collapsed_slugs(&g), vec!["ip".to_string()]);
            folds.push(hidden_slugs(&g));
            let before_expand = collapsed_set(&g);
            let shown = expand_root(&mut g, ip);
            assert_eq!(shown, 2);
            let after_expand = collapsed_set(&g);
            assert!(stale_collapsed(&before_expand, &after_expand, ip).is_empty());
            assert!(after_expand.is_empty());
            shows.push(hidden_slugs(&g));
        }
        assert!(folds.windows(2).all(|w| w[0] == w[1]));
        assert!(shows.windows(2).all(|w| w[0] == w[1]));
        assert!(folds.iter().all(|s| *s == vec!["dash".to_string(), "hetzner".to_string()]));
        assert!(shows.iter().all(|s| s.is_empty()));
    }

    #[test]
    fn collapse_root_is_idempotent_and_expand_inverts_it() {
        let (mut g, ids) = chain();
        let root = ids[2];
        let f1 = collapse_root(&mut g, root);
        assert_eq!(f1, 1);
        let s1 = hidden_slugs(&g);
        let f2 = collapse_root(&mut g, root);
        assert_eq!(f2, 0);
        let s2 = hidden_slugs(&g);
        assert_eq!(s1, s2);
        assert_eq!(collapsed_slugs(&g), vec!["c".to_string()]);
        let back = expand_root(&mut g, root);
        assert_eq!(back, 1);
        assert!(hidden_slugs(&g).is_empty());
        assert!(collapsed_set(&g).is_empty());
    }

    #[test]
    fn stale_collapsed_detects_a_leftover_derived_member() {
        let (_g, ids) = chain();
        let before: HashSet<NodeIndex> = [ids[1]].into_iter().collect();
        let after: HashSet<NodeIndex> = [ids[0]].into_iter().collect();
        let stale = stale_collapsed(&before, &after, ids[1]);
        let expected: HashSet<NodeIndex> = [ids[0]].into_iter().collect();
        assert_eq!(stale, expected);
    }
}

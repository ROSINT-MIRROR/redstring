use egui::{Align2, RichText, Vec2};
use petgraph::graph::NodeIndex;

use crate::error::Error;
use crate::model::{EntityKind, KIND_META};

use super::app::{RedstringApp, RsGraph};
use super::style;

const RESULT_CAP: usize = 12;
const NODE_FOCUS_MIN_ZOOM: f32 = 0.9;

struct Match {
    idx: NodeIndex,
    label: String,
    kind: EntityKind,
    score: i32,
}

impl RedstringApp {
    pub fn find_window(&mut self, ui: &mut egui::Ui) -> Result<(), Error> {
        if !self.find_open {
            return Ok(());
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.find_open = false;
            return Ok(());
        }
        let mut open = true;
        let shown = egui::Window::new(format!(
            "{} find entity",
            egui_phosphor::regular::MAGNIFYING_GLASS
        ))
        .anchor(Align2::CENTER_TOP, Vec2::new(0.0, 64.0))
        .collapsible(false)
        .resizable(false)
        .default_width(360.0)
        .open(&mut open)
        .show(ui.ctx(), |w| self.find_body(w));
        self.find_open = self.find_open && open;
        let Some(inner) = shown.and_then(|ir| ir.inner) else {
            return Ok(());
        };
        inner
    }

    fn find_body(&mut self, ui: &mut egui::Ui) -> Result<(), Error> {
        let resp = ui.add(
            egui::TextEdit::singleline(&mut self.find_query)
                .hint_text("name, email, IP, domain…")
                .desired_width(f32::INFINITY),
        );
        if std::mem::take(&mut self.find_focus) {
            resp.request_focus();
        }

        let matches = ranked_matches(&self.g, &self.find_query, RESULT_CAP);
        let (up, down) = ui.input(|i| {
            (
                i.key_pressed(egui::Key::ArrowUp),
                i.key_pressed(egui::Key::ArrowDown),
            )
        });
        if matches.is_empty() {
            self.find_selected = 0;
        } else {
            if self.find_selected >= matches.len() {
                self.find_selected = 0;
            }
            if down {
                self.find_selected = (self.find_selected + 1) % matches.len();
            }
            if up {
                self.find_selected = (self.find_selected + matches.len() - 1) % matches.len();
            }
        }
        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

        ui.add_space(style::GAP_SM);
        let mut chosen: Option<NodeIndex> = None;
        if self.find_query.trim().is_empty() {
            ui.weak("type to locate an entity by its name or value");
        } else if matches.is_empty() {
            ui.weak("no matching entity on the board");
        }
        for (i, m) in matches.iter().enumerate() {
            if find_row(ui, m, i == self.find_selected) {
                chosen = Some(m.idx);
            }
        }
        if enter && chosen.is_none() {
            chosen = matches.get(self.find_selected).map(|m| m.idx);
        }
        let Some(idx) = chosen else {
            return Ok(());
        };
        self.jump_to_node(ui, idx)
    }

    fn jump_to_node(&mut self, ui: &egui::Ui, idx: NodeIndex) -> Result<(), Error> {
        let Some(node) = self.g.node(idx) else {
            return Ok(());
        };
        let loc = node.location();
        let zoom = super::board::camera_read(ui)?.zoom.max(NODE_FOCUS_MIN_ZOOM);
        let cam = super::center::center_on_node(loc, self.board_size, zoom)?;
        select_single(&mut self.g, idx)?;
        self.pending_camera = Some(cam);
        self.find_open = false;
        self.status = format!("found {}", self.node_label(idx)?);
        Ok(())
    }
}

fn find_row(ui: &mut egui::Ui, m: &Match, selected: bool) -> bool {
    let meta = &KIND_META[m.kind as usize];
    let resp = ui.add(
        egui::Button::selectable(
            selected,
            (
                RichText::new(format!("{}  {}", meta.icon, m.label)),
                egui::Atom::grow(),
                RichText::new(meta.label).weak().small(),
            ),
        )
        .min_size(Vec2::new(ui.available_width(), style::ROW_H)),
    );
    resp.clicked()
}

fn ranked_matches(g: &RsGraph, query: &str, cap: usize) -> Vec<Match> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return Vec::new();
    }
    let mut hits: Vec<Match> = Vec::new();
    for idx in g.g().node_indices() {
        let Some(n) = g.g().node_weight(idx) else {
            continue;
        };
        let e = n.payload();
        let best = [score(&needle, &e.label), score(&needle, &e.value)]
            .into_iter()
            .flatten()
            .max();
        let Some(sc) = best else {
            continue;
        };
        hits.push(Match {
            idx,
            label: e.label.clone(),
            kind: e.kind,
            score: sc,
        });
    }
    hits.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.label.chars().count().cmp(&b.label.chars().count()))
            .then_with(|| a.label.cmp(&b.label))
    });
    hits.truncate(cap);
    hits
}

fn score(needle: &str, hay: &str) -> Option<i32> {
    let needle = needle.to_lowercase();
    if needle.is_empty() || hay.is_empty() {
        return None;
    }
    let hay_l = hay.to_lowercase();
    let len = hay_l.chars().count() as i32;
    if hay_l == needle {
        return Some(10_000);
    }
    if hay_l.starts_with(&needle) {
        return Some(5_000 - len);
    }
    if hay_l.contains(&needle) {
        return Some(2_000 - len);
    }
    if fuzzy(&needle, hay) {
        return Some(1_000 - len);
    }
    None
}

fn fuzzy(needle: &str, hay: &str) -> bool {
    let hay: Vec<char> = hay.to_lowercase().chars().collect();
    let mut i = 0usize;
    for n in needle.chars() {
        let mut found = false;
        while i < hay.len() {
            let h = hay[i];
            i += 1;
            if h == n {
                found = true;
                break;
            }
        }
        if !found {
            return false;
        }
    }
    true
}

fn select_single(g: &mut RsGraph, idx: NodeIndex) -> Result<(), Error> {
    let nodes: Vec<NodeIndex> = g.g().node_indices().collect();
    for i in nodes {
        let Some(n) = g.node_mut(i) else {
            continue;
        };
        n.set_selected(i == idx);
    }
    let edges: Vec<_> = g.g().edge_indices().collect();
    for e in edges {
        let Some(ed) = g.edge_mut(e) else {
            continue;
        };
        ed.set_selected(false);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use egui::Pos2;
    use egui_graphs::Graph;
    use petgraph::graph::NodeIndex;
    use petgraph::stable_graph::StableGraph;

    use super::super::app::RsGraph;
    use super::{fuzzy, ranked_matches, score, select_single};
    use crate::model::{Entity, EntityKind};

    fn ent(kind: EntityKind, label: &str, value: &str) -> Entity {
        Entity {
            slug: label.to_lowercase(),
            kind,
            label: label.to_owned(),
            value: value.to_owned(),
            ..Default::default()
        }
    }

    fn spawn(g: &mut RsGraph, kind: EntityKind, label: &str, value: &str) -> NodeIndex {
        g.add_node_with_label_and_location(ent(kind, label, value), label.into(), Pos2::ZERO)
    }

    #[test]
    fn fuzzy_is_subsequence_match() {
        assert!(fuzzy("john", "John Doe"));
        assert!(fuzzy("jd", "John Doe"));
        assert!(!fuzzy("xyz", "John Doe"));
        assert!(fuzzy("", "anything"));
    }

    #[test]
    fn score_ranks_exact_over_prefix_over_substring_over_subsequence() {
        let exact = score("john doe", "John Doe").expect("exact matches");
        let prefix = score("john", "John Doe").expect("prefix matches");
        let substr = score("doe", "John Doe").expect("substring matches");
        let subseq = score("jhd", "John Doe").expect("subsequence matches");
        assert!(
            exact > prefix && prefix > substr && substr > subseq,
            "tiers must rank exact>prefix>substring>subseq: {exact} {prefix} {substr} {subseq}"
        );
        assert!(score("zzz", "John Doe").is_none(), "non-match scores None");
        assert!(score("john", "").is_none(), "empty value never matches");
    }

    #[test]
    fn ranked_matches_finds_by_label_and_value() {
        let mut g: RsGraph = Graph::from(&StableGraph::default());
        spawn(&mut g, EntityKind::Person, "John Doe", "");
        spawn(&mut g, EntityKind::IpAddress, "home server", "8.8.8.8");
        spawn(&mut g, EntityKind::Email, "work mail", "bob@acme.com");

        let by_label = ranked_matches(&g, "john", 12);
        assert_eq!(by_label.len(), 1);
        assert_eq!(by_label[0].label, "John Doe");

        let by_ip = ranked_matches(&g, "8.8", 12);
        assert_eq!(by_ip.len(), 1);
        assert_eq!(by_ip[0].label, "home server", "an IP value match is found");

        let by_email = ranked_matches(&g, "acme", 12);
        assert_eq!(by_email.len(), 1);
        assert_eq!(by_email[0].label, "work mail", "an email value match is found");

        assert!(ranked_matches(&g, "zzzzz", 12).is_empty());
        assert!(ranked_matches(&g, "   ", 12).is_empty(), "blank query yields nothing");
    }

    #[test]
    fn ranked_matches_orders_best_first_and_caps() {
        let mut g: RsGraph = Graph::from(&StableGraph::default());
        spawn(&mut g, EntityKind::Person, "Alice Anderson", "");
        spawn(&mut g, EntityKind::Person, "Al", "");
        spawn(&mut g, EntityKind::Person, "Marshal Vidal", "");
        let hits = ranked_matches(&g, "al", 2);
        assert_eq!(hits.len(), 2, "cap truncates the result list");
        assert_eq!(hits[0].label, "Al", "the exact/shortest match ranks first");
    }

    #[test]
    fn select_single_selects_exactly_the_target() {
        let mut g: RsGraph = Graph::from(&StableGraph::default());
        let a = spawn(&mut g, EntityKind::Person, "a", "");
        let b = spawn(&mut g, EntityKind::Person, "b", "");
        let c = spawn(&mut g, EntityKind::Person, "c", "");
        for i in [a, c] {
            g.node_mut(i).expect("node").set_selected(true);
        }
        select_single(&mut g, b).expect("select");
        assert!(g.node(b).expect("b").selected(), "the found node is selected");
        assert!(!g.node(a).expect("a").selected(), "a prior selection is cleared");
        assert!(!g.node(c).expect("c").selected(), "a prior selection is cleared");
    }
}

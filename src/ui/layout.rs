use std::collections::{HashMap, HashSet};

use egui::{Pos2, Vec2};
use petgraph::Direction::{Incoming, Outgoing};
use petgraph::graph::NodeIndex;

use crate::error::Error;
use crate::shapes::{IMG_RADIUS, LABEL_GAP, LABEL_SIZE, LABEL_WRAP, NODE_RADIUS};

use super::app::RedstringApp;

const STEP: f32 = 140.0;
const STEP_MARGIN: f32 = 30.0;
const FORCE_ITERS: u32 = 300;
const GRAVITY: f32 = 0.04;
const ANIM_SECS: f64 = 0.35;
const SPREAD_FACTOR: f32 = 1.2;

#[derive(Clone, Copy)]
pub enum LayoutKind {
    Grid,
    Circle,
    Tree,
    Force,
}

#[derive(Clone, Copy)]
pub enum Spread {
    Loosen,
    Tighten,
}

pub struct LayoutAnim {
    pub steps: Vec<(NodeIndex, Pos2, Pos2)>,
    pub start: f64,
}

impl RedstringApp {
    pub fn apply_layout(&mut self, ui: &egui::Ui, kind: LayoutKind) -> Result<(), Error> {
        let targets = self.layout_targets();
        if targets.len() < 2 {
            self.status = if self.g.g().node_count() >= 2 {
                "select 2+ nodes to lay out, or clear the selection to arrange all".into()
            } else {
                "need at least 2 nodes to lay out".into()
            };
            return Ok(());
        }
        let partial = targets.len() < self.g.g().node_count();
        let step = self.layout_step(ui, &targets);
        let center = self.layout_centroid(&targets);
        let placed = match kind {
            LayoutKind::Grid => self.grid_positions(&targets, center, step),
            LayoutKind::Circle => self.circle_positions(&targets, center, step),
            LayoutKind::Tree => self.tree_positions(&targets, center, step),
            LayoutKind::Force => self.force_positions(&targets, center, step),
        };
        let mut steps = Vec::new();
        for (idx, to) in placed {
            let Some(from) = self.node_loc(idx) else {
                continue;
            };
            steps.push((idx, from, to));
        }
        let moved = steps.len();
        self.layout_anim = Some(LayoutAnim {
            steps,
            start: ui.input(|i| i.time),
        });
        ui.ctx().request_repaint();
        let name = match kind {
            LayoutKind::Grid => "grid",
            LayoutKind::Circle => "circle",
            LayoutKind::Tree => "tree",
            LayoutKind::Force => "force",
        };
        let scope = if partial { " (selection)" } else { "" };
        self.status = format!("{name} layout: {moved} nodes{scope}");
        Ok(())
    }

    pub fn apply_spread(&mut self, ui: &egui::Ui, dir: Spread) -> Result<(), Error> {
        let targets = self.layout_targets();
        if targets.len() < 2 {
            self.status = if self.g.g().node_count() >= 2 {
                "select 2+ nodes to space, or clear the selection to arrange all".into()
            } else {
                "need at least 2 nodes to space".into()
            };
            return Ok(());
        }
        let partial = targets.len() < self.g.g().node_count();
        let factor = match dir {
            Spread::Loosen => SPREAD_FACTOR,
            Spread::Tighten => 1.0 / SPREAD_FACTOR,
        };
        let mut points: Vec<(NodeIndex, Pos2)> = Vec::new();
        for idx in &targets {
            let Some(loc) = self.node_loc(*idx) else {
                continue;
            };
            points.push((*idx, loc));
        }
        let steps = scale_about_centroid(&points, factor);
        let moved = steps.len();
        self.layout_anim = Some(LayoutAnim {
            steps,
            start: ui.input(|i| i.time),
        });
        ui.ctx().request_repaint();
        let verb = match dir {
            Spread::Loosen => "loosened",
            Spread::Tighten => "tightened",
        };
        let scope = if partial { " (selection)" } else { "" };
        self.status = format!("{verb} spacing: {moved} nodes{scope}");
        Ok(())
    }

    pub fn layout_anim_tick(&mut self, ui: &egui::Ui) -> Result<(), Error> {
        let Some(anim) = &self.layout_anim else {
            return Ok(());
        };
        let t = ((ui.input(|i| i.time) - anim.start) / ANIM_SECS).clamp(0.0, 1.0) as f32;
        let eased = t * t * (3.0 - 2.0 * t);
        let steps = anim.steps.clone();
        for (idx, from, to) in steps {
            let Some(n) = self.g.node_mut(idx) else {
                continue;
            };
            n.set_location(from + (to - from) * eased);
        }
        if t >= 1.0 {
            self.layout_anim = None;
        } else {
            ui.ctx().request_repaint();
        }
        Ok(())
    }

    fn layout_targets(&self) -> Vec<NodeIndex> {
        let sel: Vec<NodeIndex> = self
            .g
            .selected_nodes()
            .iter()
            .copied()
            .filter(|idx| !self.node_hidden(*idx))
            .collect();
        if !sel.is_empty() {
            return sel;
        }
        self.g
            .g()
            .node_indices()
            .filter(|idx| !self.node_hidden(*idx))
            .collect()
    }

    fn node_hidden(&self, idx: NodeIndex) -> bool {
        self.g
            .node(idx)
            .is_some_and(|n| n.payload().hidden)
    }

    fn layout_step(&self, ui: &egui::Ui, targets: &[NodeIndex]) -> f32 {
        let mut max_footprint = 0.0f32;
        for idx in targets {
            let Some(n) = self.g.node(*idx) else {
                continue;
            };
            let e = n.payload();
            let base = if e.image.is_some() {
                IMG_RADIUS
            } else {
                NODE_RADIUS
            };
            let vsize = e.size * e.boost;
            let half = base * vsize;
            let label = ui.ctx().fonts_mut(|f| {
                f.layout(
                    e.label.clone(),
                    egui::FontId::monospace(LABEL_SIZE * vsize),
                    egui::Color32::WHITE,
                    LABEL_WRAP * vsize,
                )
            });
            let w = (2.0 * half).max(label.size().x);
            let h = 2.0 * half + LABEL_GAP * vsize + label.size().y;
            max_footprint = max_footprint.max(w.max(h));
        }
        STEP.max(max_footprint + STEP_MARGIN)
    }

    fn layout_centroid(&self, targets: &[NodeIndex]) -> Pos2 {
        let mut sum = Vec2::ZERO;
        let mut count = 0.0f32;
        for idx in targets {
            let Some(n) = self.g.node(*idx) else {
                continue;
            };
            sum += n.location().to_vec2();
            count += 1.0;
        }
        if count == 0.0 {
            return Pos2::ZERO;
        }
        (sum / count).to_pos2()
    }

    fn sorted_by_kind(&self, targets: &[NodeIndex]) -> Vec<NodeIndex> {
        let mut keyed: Vec<(usize, usize, NodeIndex)> = Vec::new();
        for idx in targets {
            let Some(n) = self.g.node(*idx) else {
                continue;
            };
            keyed.push((n.payload().kind as usize, idx.index(), *idx));
        }
        keyed.sort_by_key(|(kind, ord, _)| (*kind, *ord));
        keyed.into_iter().map(|(_, _, idx)| idx).collect()
    }

    fn grid_positions(
        &self,
        targets: &[NodeIndex],
        center: Pos2,
        step: f32,
    ) -> HashMap<NodeIndex, Pos2> {
        let order = self.sorted_by_kind(targets);
        let cols = ((order.len() as f32).sqrt().ceil() as usize).max(1);
        let rows = order.len().div_ceil(cols);
        let mut out = HashMap::new();
        for (i, idx) in order.iter().enumerate() {
            let (c, r) = ((i % cols) as f32, (i / cols) as f32);
            out.insert(
                *idx,
                Pos2::new(
                    center.x + (c - (cols as f32 - 1.0) / 2.0) * step,
                    center.y + (r - (rows as f32 - 1.0) / 2.0) * step,
                ),
            );
        }
        out
    }

    fn circle_positions(
        &self,
        targets: &[NodeIndex],
        center: Pos2,
        step: f32,
    ) -> HashMap<NodeIndex, Pos2> {
        let order = self.sorted_by_kind(targets);
        let n = order.len() as f32;
        let radius = (step * n / std::f32::consts::TAU).max(step);
        let mut out = HashMap::new();
        for (i, idx) in order.iter().enumerate() {
            let angle = i as f32 / n * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
            out.insert(
                *idx,
                center + Vec2::new(angle.cos(), angle.sin()) * radius,
            );
        }
        out
    }

    fn tree_positions(
        &self,
        targets: &[NodeIndex],
        center: Pos2,
        step: f32,
    ) -> HashMap<NodeIndex, Pos2> {
        let set: HashSet<NodeIndex> = targets.iter().copied().collect();
        let mut children: HashMap<NodeIndex, Vec<NodeIndex>> = HashMap::new();
        for idx in targets {
            let mut kids: Vec<NodeIndex> = self
                .g
                .g()
                .neighbors_directed(*idx, Outgoing)
                .filter(|k| set.contains(k) && k != idx)
                .collect();
            kids.sort_by_key(|k| k.index());
            kids.dedup();
            children.insert(*idx, kids);
        }
        let mut roots: Vec<NodeIndex> = targets
            .iter()
            .copied()
            .filter(|idx| {
                !self
                    .g
                    .g()
                    .neighbors_directed(*idx, Incoming)
                    .any(|p| set.contains(&p) && p != *idx)
            })
            .collect();
        roots.sort_by_key(|idx| idx.index());

        let mut seen: HashSet<NodeIndex> = HashSet::new();
        let mut cells: HashMap<NodeIndex, (f32, usize)> = HashMap::new();
        let mut next_col = 0.0f32;
        for root in roots {
            if !seen.contains(&root) {
                place(root, 0, &children, &mut seen, &mut next_col, &mut cells);
            }
        }
        let mut leftovers: Vec<NodeIndex> = targets
            .iter()
            .copied()
            .filter(|idx| !seen.contains(idx))
            .collect();
        leftovers.sort_by_key(|idx| idx.index());
        for idx in leftovers {
            if !seen.contains(&idx) {
                place(idx, 0, &children, &mut seen, &mut next_col, &mut cells);
            }
        }

        let max_col = (next_col - 1.0).max(0.0);
        let mut max_depth = 0usize;
        for (_, d) in cells.values() {
            max_depth = max_depth.max(*d);
        }
        let mut out = HashMap::new();
        for (idx, (col, depth)) in cells {
            out.insert(
                idx,
                Pos2::new(
                    center.x + (col - max_col / 2.0) * step,
                    center.y + (depth as f32 - max_depth as f32 / 2.0) * step,
                ),
            );
        }
        out
    }

    fn node_loc(&self, idx: NodeIndex) -> Option<Pos2> {
        let n = self.g.node(idx)?;
        Some(n.location())
    }

    fn force_positions(
        &self,
        targets: &[NodeIndex],
        center: Pos2,
        step: f32,
    ) -> HashMap<NodeIndex, Pos2> {
        let mut order: Vec<NodeIndex> = Vec::new();
        let mut p: Vec<Pos2> = Vec::new();
        for (i, idx) in targets.iter().enumerate() {
            let Some(loc) = self.node_loc(*idx) else {
                continue;
            };
            let jitter = Vec2::new(
                (i % 13) as f32 * 0.31 - 2.0,
                (i % 7) as f32 * 0.53 - 1.8,
            );
            order.push(*idx);
            p.push(loc + jitter);
        }
        let lookup: HashMap<NodeIndex, usize> =
            order.iter().enumerate().map(|(i, idx)| (*idx, i)).collect();

        let mut edges_m: Vec<(usize, usize)> = Vec::new();
        let mut edges_f: Vec<(usize, Pos2)> = Vec::new();
        for e in self.g.g().edge_indices() {
            let Some((a, b)) = self.g.g().edge_endpoints(e) else {
                continue;
            };
            if a == b {
                continue;
            }
            if self.node_hidden(a) || self.node_hidden(b) {
                continue;
            }
            match (lookup.get(&a).copied(), lookup.get(&b).copied()) {
                (Some(i), Some(j)) => edges_m.push((i, j)),
                (Some(i), None) => edges_f.extend(self.node_loc(b).map(|fp| (i, fp))),
                (None, Some(j)) => edges_f.extend(self.node_loc(a).map(|fp| (j, fp))),
                (None, None) => {}
            }
        }

        let n = p.len();
        for it in 0..FORCE_ITERS {
            let frac = 1.0 - it as f32 / FORCE_ITERS as f32;
            let temp = step * 3.0 * frac + 1.0;
            let mut d = vec![Vec2::ZERO; n];
            for i in 0..n {
                for j in (i + 1)..n {
                    let delta = p[i] - p[j];
                    let dist = delta.length().max(0.01);
                    let push = delta * (step * step / (dist * dist));
                    d[i] += push;
                    d[j] -= push;
                }
            }
            for (i, j) in &edges_m {
                let delta = p[*i] - p[*j];
                let dist = delta.length().max(0.01);
                let pull = delta * (dist / step);
                d[*i] -= pull;
                d[*j] += pull;
            }
            for (i, fp) in &edges_f {
                let delta = p[*i] - *fp;
                let dist = delta.length().max(0.01);
                d[*i] -= delta * (dist / step);
            }
            for i in 0..n {
                d[i] += (center - p[i]) * GRAVITY;
                let len = d[i].length();
                if len > 0.0 {
                    p[i] += d[i] / len * len.min(temp);
                }
            }
        }

        order.into_iter().zip(p).collect()
    }
}

fn place(
    idx: NodeIndex,
    depth: usize,
    children: &HashMap<NodeIndex, Vec<NodeIndex>>,
    seen: &mut HashSet<NodeIndex>,
    next_col: &mut f32,
    cells: &mut HashMap<NodeIndex, (f32, usize)>,
) -> f32 {
    seen.insert(idx);
    let Some(kids) = children.get(&idx) else {
        let col = *next_col;
        *next_col += 1.0;
        cells.insert(idx, (col, depth));
        return col;
    };
    let kids = kids.clone();
    let mut sum = 0.0f32;
    let mut count = 0.0f32;
    for kid in kids {
        if seen.contains(&kid) {
            continue;
        }
        sum += place(kid, depth + 1, children, seen, next_col, cells);
        count += 1.0;
    }
    let col = if count == 0.0 {
        let c = *next_col;
        *next_col += 1.0;
        c
    } else {
        sum / count
    };
    cells.insert(idx, (col, depth));
    col
}

fn scale_about_centroid(
    points: &[(NodeIndex, Pos2)],
    factor: f32,
) -> Vec<(NodeIndex, Pos2, Pos2)> {
    if points.len() < 2 {
        return Vec::new();
    }
    let mut sum = Vec2::ZERO;
    for (_, p) in points {
        sum += p.to_vec2();
    }
    let center = (sum / points.len() as f32).to_pos2();
    points
        .iter()
        .map(|(idx, p)| (*idx, *p, center + (*p - center) * factor))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nix(i: usize) -> NodeIndex {
        NodeIndex::new(i)
    }

    fn centroid_of(ps: impl Iterator<Item = Pos2>) -> Pos2 {
        let mut sum = Vec2::ZERO;
        let mut n = 0.0f32;
        for p in ps {
            sum += p.to_vec2();
            n += 1.0;
        }
        (sum / n).to_pos2()
    }

    #[test]
    fn place_centers_parent_over_children_and_survives_diamond() {
        let mut children: HashMap<NodeIndex, Vec<NodeIndex>> = HashMap::new();
        children.insert(nix(0), vec![nix(1), nix(2)]);
        children.insert(nix(1), vec![nix(3)]);
        children.insert(nix(2), vec![nix(3)]);
        children.insert(nix(3), Vec::new());

        let mut seen = HashSet::new();
        let mut cells = HashMap::new();
        let mut next_col = 0.0;
        place(nix(0), 0, &children, &mut seen, &mut next_col, &mut cells);

        assert_eq!(seen.len(), 4);
        let (c1, d1) = cells[&nix(1)];
        let (c2, d2) = cells[&nix(2)];
        let (c0, d0) = cells[&nix(0)];
        let (_, d3) = cells[&nix(3)];
        assert_eq!((d0, d1, d2, d3), (0, 1, 1, 2));
        assert!((c0 - (c1 + c2) / 2.0).abs() < 1e-4);
        assert_ne!(c1, c2);
    }

    #[test]
    fn place_packs_two_roots_without_overlap() {
        let mut children: HashMap<NodeIndex, Vec<NodeIndex>> = HashMap::new();
        children.insert(nix(0), vec![nix(1)]);
        children.insert(nix(1), Vec::new());
        children.insert(nix(2), Vec::new());

        let mut seen = HashSet::new();
        let mut cells = HashMap::new();
        let mut next_col = 0.0;
        place(nix(0), 0, &children, &mut seen, &mut next_col, &mut cells);
        place(nix(2), 0, &children, &mut seen, &mut next_col, &mut cells);

        assert_ne!(cells[&nix(0)].0, cells[&nix(2)].0);
        assert_eq!(cells[&nix(2)].1, 0);
    }

    fn sample() -> Vec<(NodeIndex, Pos2)> {
        vec![
            (nix(0), Pos2::new(-30.0, 10.0)),
            (nix(1), Pos2::new(20.0, 10.0)),
            (nix(2), Pos2::new(10.0, -40.0)),
        ]
    }

    #[test]
    fn loosen_grows_distances_and_holds_centroid() {
        let pts = sample();
        let f = 1.2;
        let steps = scale_about_centroid(&pts, f);
        assert_eq!(steps.len(), pts.len());

        let before = centroid_of(pts.iter().map(|(_, p)| *p));
        let after = centroid_of(steps.iter().map(|(_, _, to)| *to));
        assert!((before - after).length() < 1e-3);

        for i in 0..steps.len() {
            for j in (i + 1)..steps.len() {
                let d_before = (steps[i].1 - steps[j].1).length();
                let d_after = (steps[i].2 - steps[j].2).length();
                assert!(d_after > d_before);
                assert!((d_after - d_before * f).abs() < 1e-2);
            }
        }
    }

    #[test]
    fn tighten_after_loosen_returns_to_origin() {
        let pts = sample();
        let f = 1.2;
        let loosened: Vec<(NodeIndex, Pos2)> = scale_about_centroid(&pts, f)
            .into_iter()
            .map(|(idx, _, to)| (idx, to))
            .collect();
        let restored = scale_about_centroid(&loosened, 1.0 / f);
        assert_eq!(restored.len(), pts.len());
        for (i, (_, _, to)) in restored.iter().enumerate() {
            assert!((*to - pts[i].1).length() < 1e-2);
        }
    }

    #[test]
    fn fewer_than_two_nodes_is_noop() {
        assert!(scale_about_centroid(&[], 1.2).is_empty());
        let one = vec![(nix(0), Pos2::new(3.0, 4.0))];
        assert!(scale_about_centroid(&one, 1.2).is_empty());
    }
}

use std::collections::HashSet;

use egui::{Pos2, Rect, Vec2};
use petgraph::graph::NodeIndex;

use crate::error::Error;
use crate::model::Camera;

use super::app::RedstringApp;
use super::board::{camera_read, camera_write, clamp_zoom};
use super::layout::{LayoutAnim, LayoutKind};
use super::pack::pack_radius;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct CameraEase {
    pub from: Camera,
    pub to: Camera,
    pub start: f64,
}

impl CameraEase {
    pub const ID: &'static str = "redstring_camera_ease";
    pub const SECS: f64 = 0.45;
    pub const MARGIN: f32 = 0.85;
    const GAP: f32 = 56.0;
    const LABEL_PAD: f32 = 40.0;
    const RINGS: usize = 10;
    const SLOTS: usize = 12;
}

impl RedstringApp {
    pub fn settle_arrivals(
        &mut self,
        ui: &egui::Ui,
        added: &[String],
        removed: &[String],
    ) -> Result<(), Error> {
        if added.is_empty() && removed.is_empty() {
            return Ok(());
        }
        let notice = self.status.clone();
        let fresh = self.indices_of(added);
        if self.current_layout == LayoutKind::Free {
            self.grow_free(ui, &fresh)?;
        } else {
            self.grow_in_layout(ui, &fresh)?;
        }
        let settled = self.final_positions();
        self.ease_to_fit(ui, &settled)?;
        self.status = format!(
            "{} · {} added, {} removed · {:?} layout kept",
            notice,
            added.len(),
            removed.len(),
            self.current_layout
        );
        Ok(())
    }

    pub fn camera_ease_tick(&mut self, ui: &mut egui::Ui) -> Result<(), Error> {
        let id = egui::Id::new(CameraEase::ID);
        let Some(ease) = ui.ctx().data(|d| d.get_temp::<CameraEase>(id)) else {
            return Ok(());
        };
        let gesture = ui.input(|i| {
            i.pointer.any_down() || i.smooth_scroll_delta != Vec2::ZERO || i.zoom_delta() != 1.0
        });
        if gesture || self.panning || self.pending_camera.is_some() {
            ui.ctx().data_mut(|d| d.remove::<CameraEase>(id));
            return Ok(());
        }
        let elapsed = ui.input(|i| i.time) - ease.start;
        let t = (elapsed / CameraEase::SECS).clamp(0.0, 1.0) as f32;
        let k = t * t * (3.0 - 2.0 * t);
        camera_write(ui, blend(ease.from, ease.to, k, self.board_size)?)?;
        if t >= 1.0 {
            ui.ctx().data_mut(|d| d.remove::<CameraEase>(id));
        } else {
            ui.ctx().request_repaint();
        }
        Ok(())
    }

    fn indices_of(&self, slugs: &[String]) -> Vec<NodeIndex> {
        let want: HashSet<&str> = slugs.iter().map(String::as_str).collect();
        let mut out = Vec::new();
        for idx in self.g.g().node_indices() {
            let Some(n) = self.g.node(idx) else {
                continue;
            };
            if want.contains(n.payload().slug.as_str()) {
                out.push(idx);
            }
        }
        out
    }

    fn settled_positions(&self, skip: &HashSet<NodeIndex>) -> Vec<(NodeIndex, Pos2)> {
        let mut out = Vec::new();
        for idx in self.g.g().node_indices() {
            let Some(n) = self.g.node(idx) else {
                continue;
            };
            if n.payload().hidden || skip.contains(&idx) {
                continue;
            }
            out.push((idx, n.location()));
        }
        out
    }

    fn final_positions(&self) -> Vec<(NodeIndex, Pos2)> {
        let mut out = self.settled_positions(&HashSet::new());
        let Some(anim) = &self.layout_anim else {
            return out;
        };
        for (idx, pos) in out.iter_mut() {
            for (step_idx, _from, to) in &anim.steps {
                if step_idx == idx {
                    *pos = *to;
                }
            }
        }
        out
    }

    fn spacing(&self, idx: NodeIndex) -> Result<f32, Error> {
        let Some(n) = self.g.node(idx) else {
            return Ok(CameraEase::GAP);
        };
        Ok(pack_radius(n.payload())? * 2.0 + CameraEase::GAP)
    }

    fn anchor_points(&self, idx: NodeIndex, placed: &[(NodeIndex, Pos2)]) -> Vec<Pos2> {
        let mut out = Vec::new();
        for nb in self.g.g().neighbors_undirected(idx) {
            for (p_idx, pos) in placed {
                if *p_idx == nb {
                    out.push(*pos);
                }
            }
        }
        out
    }

    fn view_centre(&self, ui: &egui::Ui) -> Result<Pos2, Error> {
        let cam = camera_read(ui)?;
        let zoom = clamp_zoom(cam.zoom)?;
        let half = self.board_size / 2.0;
        Ok(((half - Vec2::new(cam.x, cam.y)) / zoom).to_pos2())
    }

    fn move_node(&mut self, idx: NodeIndex, to: Pos2) {
        let Some(n) = self.g.node_mut(idx) else {
            return;
        };
        n.set_location(to);
    }

    fn grow_free(&mut self, ui: &egui::Ui, fresh: &[NodeIndex]) -> Result<(), Error> {
        let skip: HashSet<NodeIndex> = fresh.iter().copied().collect();
        let mut placed = self.settled_positions(&skip);
        let hub = centroid(&placed);
        let view = self.view_centre(ui)?;
        let mut steps = Vec::new();
        for idx in fresh {
            let gap = self.spacing(*idx)?;
            let anchors = self.anchor_points(*idx, &placed);
            let origin = seed_for(&anchors, &placed, view, gap);
            let away = away_from(hub, origin);
            let target = free_spot(origin, away, &placed, gap);
            self.move_node(*idx, origin);
            steps.push((*idx, origin, target));
            placed.push((*idx, target));
        }
        self.start_anim(ui, steps)
    }

    fn grow_in_layout(&mut self, ui: &egui::Ui, fresh: &[NodeIndex]) -> Result<(), Error> {
        let skip: HashSet<NodeIndex> = fresh.iter().copied().collect();
        let placed = self.settled_positions(&skip);
        let view = self.view_centre(ui)?;
        for idx in fresh {
            let gap = self.spacing(*idx)?;
            let anchors = self.anchor_points(*idx, &placed);
            let start = seed_for(&anchors, &placed, view, gap);
            self.move_node(*idx, start);
        }
        self.apply_fold()?;
        self.relayout_all(ui, self.current_layout)
    }

    fn start_anim(&mut self, ui: &egui::Ui, steps: Vec<(NodeIndex, Pos2, Pos2)>) -> Result<(), Error> {
        if steps.is_empty() {
            return Ok(());
        }
        let start = ui.input(|i| i.time);
        ui.ctx().request_repaint();
        let Some(anim) = self.layout_anim.as_mut() else {
            self.layout_anim = Some(LayoutAnim { steps, start });
            return Ok(());
        };
        for (idx, from, _to) in anim.steps.iter_mut() {
            let Some(n) = self.g.node(*idx) else {
                continue;
            };
            *from = n.location();
        }
        anim.steps.extend(steps);
        anim.start = start;
        Ok(())
    }

    fn ease_to_fit(&mut self, ui: &egui::Ui, points: &[(NodeIndex, Pos2)]) -> Result<(), Error> {
        let board = self.board_size;
        if board.x < 1.0 || board.y < 1.0 {
            return Ok(());
        }
        let Some(bbox) = self.padded_bbox(points)? else {
            return Ok(());
        };
        let cam = camera_read(ui)?;
        let to = frame_fit(cam, bbox, board)?;
        if to == cam {
            return Ok(());
        }
        let start = ui.input(|i| i.time);
        let ease = CameraEase { from: cam, to, start };
        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(CameraEase::ID), ease));
        ui.ctx().request_repaint();
        Ok(())
    }

    fn padded_bbox(&self, points: &[(NodeIndex, Pos2)]) -> Result<Option<Rect>, Error> {
        let mut bbox = Rect::NOTHING;
        let mut any = false;
        for (idx, pos) in points {
            let Some(n) = self.g.node(*idx) else {
                continue;
            };
            let r = pack_radius(n.payload())?;
            let rect = Rect::from_min_max(
                *pos - Vec2::splat(r),
                *pos + Vec2::new(r, r + CameraEase::LABEL_PAD),
            );
            bbox = bbox.union(rect);
            any = true;
        }
        if !any {
            return Ok(None);
        }
        Ok(Some(bbox))
    }
}

fn away_from(hub: Option<Pos2>, p: Pos2) -> Vec2 {
    let Some(h) = hub else {
        return Vec2::ZERO;
    };
    p - h
}

fn centroid(points: &[(NodeIndex, Pos2)]) -> Option<Pos2> {
    if points.is_empty() {
        return None;
    }
    let mut sum = Vec2::ZERO;
    for (_idx, p) in points {
        sum += p.to_vec2();
    }
    Some((sum / points.len() as f32).to_pos2())
}

fn seed_for(anchors: &[Pos2], placed: &[(NodeIndex, Pos2)], view: Pos2, gap: f32) -> Pos2 {
    if !anchors.is_empty() {
        let mut sum = Vec2::ZERO;
        for a in anchors {
            sum += a.to_vec2();
        }
        return (sum / anchors.len() as f32).to_pos2();
    }
    edge_spot(placed, view, gap)
}

fn edge_spot(placed: &[(NodeIndex, Pos2)], view: Pos2, gap: f32) -> Pos2 {
    let Some((first, rest)) = placed.split_first() else {
        return view;
    };
    let mut bbox = Rect::from_min_max(first.1, first.1);
    for (_idx, p) in rest {
        bbox.extend_with(*p);
    }
    let edge = bbox.expand(gap);
    if !edge.contains(view) {
        return edge.clamp(view);
    }
    let to_left = view.x - edge.left();
    let to_right = edge.right() - view.x;
    let to_top = view.y - edge.top();
    let to_bottom = edge.bottom() - view.y;
    let nearest = to_left.min(to_right).min(to_top).min(to_bottom);
    if nearest == to_left {
        return Pos2::new(edge.left(), view.y);
    }
    if nearest == to_right {
        return Pos2::new(edge.right(), view.y);
    }
    if nearest == to_top {
        return Pos2::new(view.x, edge.top());
    }
    Pos2::new(view.x, edge.bottom())
}

fn clear(p: Pos2, placed: &[(NodeIndex, Pos2)], gap: f32) -> bool {
    placed.iter().all(|(_idx, q)| p.distance(*q) >= gap)
}

fn free_spot(seed: Pos2, away: Vec2, placed: &[(NodeIndex, Pos2)], gap: f32) -> Pos2 {
    if clear(seed, placed, gap) {
        return seed;
    }
    let base = if away.length() > 1e-3 { away.angle() } else { 0.0 };
    let half_step = std::f32::consts::TAU / (2.0 * CameraEase::SLOTS as f32);
    let mut radius = gap;
    for _ring in 0..CameraEase::RINGS {
        let mut offset = 0.0f32;
        for _slot in 0..CameraEase::SLOTS {
            for sign in [1.0f32, -1.0] {
                let p = seed + Vec2::angled(base + sign * offset) * radius;
                if clear(p, placed, gap) {
                    return p;
                }
            }
            offset += half_step;
        }
        radius += gap;
    }
    seed + Vec2::angled(base) * radius
}

fn frame_fit(cam: Camera, bbox: Rect, board: Vec2) -> Result<Camera, Error> {
    let m = CameraEase::MARGIN;
    let zoom = clamp_zoom(cam.zoom)?;
    let fit = (board.x * m / bbox.width().max(1.0)).min(board.y * m / bbox.height().max(1.0));
    let z = clamp_zoom(fit.min(zoom))?;
    let pan = board / 2.0 - bbox.center().to_vec2() * z;
    Ok(Camera { x: pan.x, y: pan.y, zoom: z })
}

fn blend(from: Camera, to: Camera, k: f32, board: Vec2) -> Result<Camera, Error> {
    let zf = clamp_zoom(from.zoom)?;
    let zt = clamp_zoom(to.zoom)?;
    let zoom = (zf.ln() + (zt.ln() - zf.ln()) * k).exp();
    let half = board / 2.0;
    let cf = (half - Vec2::new(from.x, from.y)) / zf;
    let ct = (half - Vec2::new(to.x, to.y)) / zt;
    let centre = cf + (ct - cf) * k;
    let pan = half - centre * zoom;
    Ok(Camera { x: pan.x, y: pan.y, zoom })
}

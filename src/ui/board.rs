use std::sync::{Mutex, OnceLock};

use egui::{Pos2, Rect, Vec2};
use egui_graphs::{
    GraphView, MetadataFrame, SettingsInteraction, SettingsNavigation, SettingsStyle,
};
use petgraph::graph::NodeIndex;

use crate::error::Error;
use crate::model::{Camera, EntityKind, KIND_META, Preset};
use crate::shapes::{IMG_RADIUS, NODE_RADIUS, SourcedEdgeShape};

use super::app::{BOARD_META_ID, RedstringApp, RsGraph};

const SCROLL_ZOOM_SPEED: f32 = 1.0 / 200.0;

pub const MIN_ZOOM: f32 = 0.01;
pub const MAX_ZOOM: f32 = 30.0;

static LAST_SEL: OnceLock<Mutex<Vec<usize>>> = OnceLock::new();

fn log_selection(g: &RsGraph) {
    if !crate::log::ENABLED.load(std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    let mut cur: Vec<usize> = g.selected_nodes().iter().map(|i| i.index()).collect();
    cur.sort();
    let cell = LAST_SEL.get_or_init(|| Mutex::new(Vec::new()));
    let Ok(mut prev) = cell.lock() else {
        return;
    };
    if *prev == cur {
        return;
    }
    crate::dlog_debug!("board", "selection {}->{} nodes={:?}", prev.len(), cur.len(), cur);
    *prev = cur;
}

pub fn meta(ui: &egui::Ui) -> Result<MetadataFrame, Error> {
    Ok(MetadataFrame::new(Some(BOARD_META_ID.to_owned())).load(ui))
}

pub fn camera_read(ui: &egui::Ui) -> Result<Camera, Error> {
    let mf = meta(ui)?;
    Ok(Camera {
        x: mf.pan.x,
        y: mf.pan.y,
        zoom: mf.zoom,
    })
}

pub fn clamp_zoom(z: f32) -> Result<f32, Error> {
    if !z.is_finite() {
        return Ok(1.0);
    }
    Ok(z.clamp(MIN_ZOOM, MAX_ZOOM))
}

pub fn fit_box_to_board(bbox: Rect, board: Vec2, margin: f32, max_zoom: f32) -> Result<Camera, Error> {
    let span = bbox.size();
    let w = span.x.max(1.0);
    let h = span.y.max(1.0);
    let fit = (board.x * margin / w).min(board.y * margin / h);
    let zoom = clamp_zoom(fit.min(max_zoom))?;
    let pan = board / 2.0 - bbox.center().to_vec2() * zoom;
    Ok(Camera {
        x: pan.x,
        y: pan.y,
        zoom,
    })
}

pub fn camera_write(ui: &mut egui::Ui, cam: Camera) -> Result<(), Error> {
    let mut mf = meta(ui)?;
    mf.pan = Vec2::new(cam.x, cam.y);
    mf.zoom = clamp_zoom(cam.zoom)?;
    mf.save(ui);
    Ok(())
}

fn scroll_zoom(ui: &mut egui::Ui) -> Result<(), Error> {
    let board = ui.available_rect_before_wrap();
    let (dy, pinch, hover) = ui.input(|i| {
        (
            i.smooth_scroll_delta.y,
            i.zoom_delta(),
            i.pointer.hover_pos(),
        )
    });
    let factor = (dy * SCROLL_ZOOM_SPEED).exp() * pinch;
    if factor == 1.0 {
        return Ok(());
    }
    let Some(pointer) = hover else {
        return Ok(());
    };
    if !board.contains(pointer) {
        return Ok(());
    }

    let mut mf = meta(ui)?;
    let new_zoom = clamp_zoom(mf.zoom * factor)?;
    crate::dlog_debug!("board", "zoom {:.3}->{:.3}", mf.zoom, new_zoom);

    let local = pointer - board.left_top();
    mf.pan = local - (local - mf.pan) * (new_zoom / mf.zoom);
    mf.zoom = new_zoom;
    mf.save(ui);
    Ok(())
}

impl RedstringApp {
    pub fn zoom_bar(&mut self, ui: &mut egui::Ui) -> Result<(), Error> {
        let mut mf = meta(ui)?;
        let mut z = mf.zoom;
        ui.spacing_mut().slider_width = 110.0;
        let resp = ui.add(
            egui::Slider::new(&mut z, MIN_ZOOM..=MAX_ZOOM)
                .logarithmic(true)
                .show_value(false),
        );
        ui.weak(format!("{:.0}%", z * 100.0));
        if !resp.changed() {
            return Ok(());
        }
        let local = self.board_size / 2.0;
        mf.pan = local - (local - mf.pan) * (z / mf.zoom);
        mf.zoom = z;
        mf.save(ui);
        Ok(())
    }

    pub fn center_graph(&mut self) -> Result<(), Error> {
        let Some((cam, n)) = super::center::center_camera(&self.g, self.board_size)? else {
            self.status = "center — no visible nodes to frame".to_owned();
            return Ok(());
        };
        crate::dlog_info!("board", "center biggest visible cluster n={n} zoom={:.3}", cam.zoom);
        self.pending_camera = Some(cam);
        self.status = format!("centered on biggest visible cluster — {n} node(s)");
        Ok(())
    }

    pub fn board(&mut self, ui: &mut egui::Ui) -> Result<(), Error> {
        let rect = ui.available_rect_before_wrap();
        self.board_size = rect.size();

        self.apply_fold()?;
        self.edge_drag_input(ui, rect)?;
        self.pan_input(ui, rect)?;
        self.node_drag_input(ui, rect)?;
        self.fold_input(ui, rect)?;
        self.layout_anim_tick(ui)?;
        scroll_zoom(ui)?;

        ui.add(
            &mut GraphView::<_, _, _, _, _, SourcedEdgeShape>::new(&mut self.g)
                .with_id(Some(BOARD_META_ID.to_owned()))
                .with_interactions(
                    &SettingsInteraction::default()
                        .with_dragging_enabled(false)
                        .with_node_selection_enabled(true)
                        .with_edge_selection_enabled(true),
                )
                .with_navigations(
                    &SettingsNavigation::default()
                        .with_fit_to_screen_enabled(false)
                        .with_zoom_and_pan_enabled(false),
                )
                .with_styles(&SettingsStyle::default().with_labels_always(true)),
        );

        self.band_select(ui, rect)?;
        self.edge_drag_overlay(ui, rect)?;
        self.resize_handle(ui, rect)?;
        self.kind_drop(ui, rect)?;
        self.preset_drop(ui, rect)?;
        self.verb_picker(ui)?;
        self.fold_menu(ui)?;
        self.label_editor(ui, rect)?;
        log_selection(&self.g);
        Ok(())
    }

    pub fn node_at_screen(
        &self,
        ui: &egui::Ui,
        rect: Rect,
        screen: Pos2,
    ) -> Result<Option<NodeIndex>, Error> {
        let mf = meta(ui)?;
        Ok(self
            .g
            .node_by_screen_pos(&mf, (screen - rect.left_top()).to_pos2()))
    }

    fn resize_target(&self) -> Option<NodeIndex> {
        if self.resize.is_some() {
            return self.resize;
        }
        let sel = self.g.selected_nodes();
        (sel.len() == 1).then(|| sel[0])
    }

    fn resize_handle_rect(&self, ui: &egui::Ui, rect: Rect) -> Result<Option<(NodeIndex, Rect)>, Error> {
        let Some(idx) = self.resize_target() else {
            return Ok(None);
        };
        let Some(node) = self.g.node(idx) else {
            return Ok(None);
        };
        let e = node.payload();
        let base = if e.image.is_some() {
            IMG_RADIUS
        } else {
            NODE_RADIUS
        };
        let mf = meta(ui)?;
        let center = mf.canvas_to_screen_pos(node.location()) + rect.left_top().to_vec2();
        let corner = center + Vec2::splat(base * e.size * e.boost * mf.zoom);
        Ok(Some((idx, Rect::from_center_size(corner, Vec2::splat(14.0)))))
    }

    fn node_drag_input(&mut self, ui: &egui::Ui, rect: Rect) -> Result<(), Error> {
        let (pressed, down, pointer) = ui.input(|i| {
            (
                i.pointer.primary_pressed(),
                i.pointer.primary_down(),
                i.pointer.interact_pos(),
            )
        });

        if pressed
            && self.node_drag.is_none()
            && self.edge_drag.is_none()
            && self.resize.is_none()
            && let Some(p) = pointer.filter(|p| rect.contains(*p))
            && !self
                .resize_handle_rect(ui, rect)?
                .is_some_and(|(_, h)| h.contains(p))
            && let Some(idx) = self.node_at_screen(ui, rect, p)?
        {
            let canvas_p = meta(ui)?.screen_to_canvas_pos((p - rect.left_top()).to_pos2());
            self.begin_node_drag(idx, canvas_p);
        }

        if self.node_drag.is_none() {
            return Ok(());
        }
        if !down {
            self.end_node_drag();
            return Ok(());
        }
        let Some(p) = pointer else {
            return Ok(());
        };
        let canvas_p = meta(ui)?.screen_to_canvas_pos((p - rect.left_top()).to_pos2());
        let Some(group) = self.node_drag.clone() else {
            return Ok(());
        };
        for (idx, off) in group {
            let Some(n) = self.g.node_mut(idx) else {
                continue;
            };
            n.set_location(canvas_p - off);
        }
        Ok(())
    }

    fn begin_node_drag(&mut self, idx: NodeIndex, canvas_p: Pos2) {
        self.layout_anim = None;
        let selected = self.g.selected_nodes().to_vec();
        let group = if selected.len() > 1 && selected.contains(&idx) {
            selected
        } else {
            vec![idx]
        };
        let mut drags = Vec::new();
        for i in group {
            let Some(n) = self.g.node_mut(i) else {
                continue;
            };
            n.set_dragged(true);
            drags.push((i, canvas_p - n.location()));
        }
        if drags.is_empty() {
            return;
        }
        crate::dlog_debug!("board", "node drag start idx={} group={}", idx.index(), drags.len());
        self.node_drag = Some(drags);
    }

    fn end_node_drag(&mut self) {
        let Some(group) = self.node_drag.take() else {
            return;
        };
        crate::dlog_debug!("board", "node drag end moved={}", group.len());
        for (idx, _) in group {
            let Some(n) = self.g.node_mut(idx) else {
                continue;
            };
            n.set_dragged(false);
        }
    }

    fn pan_input(&mut self, ui: &mut egui::Ui, rect: Rect) -> Result<(), Error> {
        let (mid_pressed, sec_pressed, mid_down, sec_down, pointer, delta) = ui.input(|i| {
            (
                i.pointer.button_pressed(egui::PointerButton::Middle),
                i.pointer.secondary_pressed(),
                i.pointer.button_down(egui::PointerButton::Middle),
                i.pointer.secondary_down(),
                i.pointer.interact_pos(),
                i.pointer.delta(),
            )
        });

        let sec_on_empty = matches!(self.right_press, Some((None, _)));
        if !self.panning
            && pointer.is_some_and(|p| rect.contains(p))
            && (mid_pressed || (sec_pressed && sec_on_empty))
        {
            self.panning = true;
            crate::dlog_debug!("board", "pan start");
        }
        if !self.panning {
            return Ok(());
        }
        if !mid_down && !sec_down {
            self.panning = false;
            return Ok(());
        }
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        if delta == Vec2::ZERO {
            return Ok(());
        }
        let mut mf = meta(ui)?;
        mf.pan += delta;
        mf.save(ui);
        Ok(())
    }

    fn band_select(&mut self, ui: &mut egui::Ui, rect: Rect) -> Result<(), Error> {
        let (pressed, down, pointer) = ui.input(|i| {
            (
                i.pointer.primary_pressed(),
                i.pointer.primary_down(),
                i.pointer.interact_pos(),
            )
        });

        if pressed
            && self.band.is_none()
            && self.node_drag.is_none()
            && self.edge_drag.is_none()
            && self.resize.is_none()
            && self.label_edit.is_none()
            && let Some(p) = pointer.filter(|p| rect.contains(*p))
            && !self
                .resize_handle_rect(ui, rect)?
                .is_some_and(|(_, h)| h.contains(p))
            && self.node_at_screen(ui, rect, p)?.is_none()
        {
            self.band = Some(p);
            crate::dlog_debug!("board", "rubber-band start");
        }

        let Some(anchor) = self.band else {
            return Ok(());
        };
        if !down || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            crate::dlog_debug!("board", "rubber-band end selected={}", self.g.selected_nodes().len());
            self.band = None;
            return Ok(());
        }
        let Some(p) = pointer else {
            return Ok(());
        };
        let band = Rect::from_two_pos(anchor, p);
        if band.size().max_elem() < 4.0 {
            return Ok(());
        }
        self.apply_band(ui, rect, band)?;
        let stroke = ui.visuals().selection.stroke;
        let painter = ui.painter().with_clip_rect(rect);
        painter.rect_filled(band, 0.0, stroke.color.gamma_multiply(0.15));
        painter.rect_stroke(band, 0.0, stroke, egui::StrokeKind::Inside);
        Ok(())
    }

    fn apply_band(&mut self, ui: &egui::Ui, rect: Rect, band: Rect) -> Result<(), Error> {
        let mf = meta(ui)?;
        let nodes: Vec<NodeIndex> = self.g.g().node_indices().collect();
        for idx in nodes {
            let Some(node) = self.g.node(idx) else {
                continue;
            };
            let e = node.payload();
            if e.hidden {
                let Some(n) = self.g.node_mut(idx) else {
                    continue;
                };
                n.set_selected(false);
                continue;
            }
            let base = if e.image.is_some() {
                IMG_RADIUS
            } else {
                NODE_RADIUS
            };
            let center = mf.canvas_to_screen_pos(node.location()) + rect.left_top().to_vec2();
            let half = base * e.size * e.boost * mf.zoom;
            let hit = band.intersects(Rect::from_center_size(center, Vec2::splat(half * 2.0)));
            let Some(n) = self.g.node_mut(idx) else {
                continue;
            };
            n.set_selected(hit);
        }
        Ok(())
    }

    fn resize_handle(&mut self, ui: &mut egui::Ui, rect: Rect) -> Result<(), Error> {
        let Some((idx, hrect)) = self.resize_handle_rect(ui, rect)? else {
            self.resize = None;
            return Ok(());
        };
        let Some(node) = self.g.node(idx) else {
            self.resize = None;
            return Ok(());
        };
        let e = node.payload();
        let base = if e.image.is_some() {
            IMG_RADIUS
        } else {
            NODE_RADIUS
        };
        let boost = e.boost;
        let loc = node.location();

        let mf = meta(ui)?;
        let center = mf.canvas_to_screen_pos(loc) + rect.left_top().to_vec2();
        let corner = hrect.center();
        let resp = ui.interact(
            hrect,
            egui::Id::new("rs_resize_handle"),
            egui::Sense::drag(),
        );
        if resp.hovered() || resp.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeNwSe);
        }
        if resp.drag_started() {
            self.resize = Some(idx);
        }
        if resp.dragged()
            && self.resize == Some(idx)
            && let Some(p) = resp.interact_pointer_pos()
        {
            let d = p - center;

            let new_half = d.x.max(d.y).max(4.0);
            self.set_node_size(idx, (new_half / mf.zoom / base / boost).clamp(0.5, 12.0))?;
        }
        if resp.drag_stopped() {
            self.resize = None;
        }

        let style = if resp.hovered() || resp.dragged() {
            ui.style().visuals.widgets.active
        } else {
            ui.style().visuals.widgets.inactive
        };
        let painter = ui.painter().with_clip_rect(rect);
        painter.rect_filled(
            Rect::from_center_size(corner, Vec2::splat(8.0)),
            1.0,
            style.fg_stroke.color,
        );
        Ok(())
    }

    fn kind_drop(&mut self, ui: &mut egui::Ui, rect: Rect) -> Result<(), Error> {
        if !egui::DragAndDrop::has_payload_of_type::<EntityKind>(ui.ctx()) {
            return Ok(());
        }
        let pointer = ui.input(|i| i.pointer.latest_pos());
        let over = pointer.is_some_and(|p| rect.contains(p));
        if over {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Copy);
        }
        if !ui.input(|i| i.pointer.any_released()) {
            return Ok(());
        }

        let Some(kind) = egui::DragAndDrop::take_payload::<EntityKind>(ui.ctx()) else {
            return Ok(());
        };
        let (Some(p), true) = (pointer, over) else {
            return Ok(());
        };
        let kind = *kind;
        let canvas = meta(ui)?.screen_to_canvas_pos((p - rect.left_top()).to_pos2());
        let idx = self.add_entity(kind, KIND_META[kind as usize].label.to_owned(), canvas)?;
        crate::dlog_info!("board", "drop-spawn kind={:?} idx={}", kind, idx.index());
        self.ne_kind = kind;
        self.ne_preset = None;
        self.start_label_edit(idx)
    }

    fn preset_drop(&mut self, ui: &mut egui::Ui, rect: Rect) -> Result<(), Error> {
        if !egui::DragAndDrop::has_payload_of_type::<Preset>(ui.ctx()) {
            return Ok(());
        }
        let pointer = ui.input(|i| i.pointer.latest_pos());
        let over = pointer.is_some_and(|p| rect.contains(p));
        if over {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Copy);
        }
        if !ui.input(|i| i.pointer.any_released()) {
            return Ok(());
        }

        let Some(preset) = egui::DragAndDrop::take_payload::<Preset>(ui.ctx()) else {
            return Ok(());
        };
        let (Some(p), true) = (pointer, over) else {
            return Ok(());
        };
        let preset = *preset;
        let canvas = meta(ui)?.screen_to_canvas_pos((p - rect.left_top()).to_pos2());
        let idx = self.add_preset(&preset, canvas)?;
        self.ne_preset = Some(preset);
        self.start_label_edit(idx)
    }
}

use std::sync::{Mutex, OnceLock};

use egui::{Color32, Pos2, Rect, Vec2};
use egui_graphs::{
    GraphView, MetadataFrame, SettingsInteraction, SettingsNavigation, SettingsStyle,
};
use petgraph::graph::NodeIndex;

use crate::error::Error;
use crate::model::{Camera, EntityKind, KIND_META, Preset};
use crate::shapes::{SourcedEdgeShape, base_radius, texture_for};

use super::app::{BOARD_META_ID, RedstringApp, RsGraph};

const SCROLL_ZOOM_SPEED: f32 = 1.0 / 200.0;

pub const MIN_ZOOM: f32 = 0.01;
pub const MAX_ZOOM: f32 = 30.0;

const LEGIBLE_RADIUS_PX: f32 = 5.5;

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

pub fn fit_box_to_board(
    bbox: Rect,
    board: Vec2,
    margin: f32,
    max_zoom: f32,
    median_radius: f32,
) -> Result<Camera, Error> {
    let span = bbox.size();
    let w = span.x.max(1.0);
    let h = span.y.max(1.0);
    let fit = (board.x * margin / w).min(board.y * margin / h);
    let floor = if median_radius > 0.0 {
        LEGIBLE_RADIUS_PX / median_radius
    } else {
        0.0
    };
    let zoom = clamp_zoom(fit.max(floor).min(max_zoom))?;
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

fn cover_rect(board: Rect, img: Vec2) -> Rect {
    if img.x <= 0.0 || img.y <= 0.0 {
        return board;
    }
    let scale = (board.width() / img.x).max(board.height() / img.y);
    Rect::from_center_size(board.center(), img * scale)
}

fn pan_should_start(primary_pressed: bool, middle_pressed: bool, on_empty: bool) -> bool {
    middle_pressed || (primary_pressed && on_empty)
}

fn band_should_start(secondary_pressed: bool, on_empty: bool) -> bool {
    secondary_pressed && on_empty
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
        let Some((cam, n, off)) = super::center::center_camera(&self.g, self.board_size)? else {
            self.status = "center — no visible nodes to frame".to_owned();
            return Ok(());
        };
        crate::dlog_info!(
            "board",
            "center biggest visible cluster n={n} zoom={:.3} off={off}",
            cam.zoom
        );
        self.pending_camera = Some(cam);
        self.status = if off > 0 {
            format!("centered on biggest cluster — {n} node(s) · {off} off-screen")
        } else {
            format!("centered on biggest cluster — {n} node(s)")
        };
        Ok(())
    }

    fn draw_background(&self, ui: &mut egui::Ui, rect: Rect) -> Result<(), Error> {
        let Some(path) = self.prefs.background.as_deref() else {
            return Ok(());
        };
        let Some(tex) = texture_for(ui.ctx(), path)? else {
            return Ok(());
        };
        let tint =
            Color32::from_white_alpha((self.prefs.background_opacity.clamp(0.0, 1.0) * 255.0) as u8);
        let painter = ui.painter().with_clip_rect(rect);
        painter.image(
            tex.id(),
            cover_rect(rect, tex.size_vec2()),
            Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
            tint,
        );
        Ok(())
    }

    pub fn board(&mut self, ui: &mut egui::Ui) -> Result<(), Error> {
        let rect = ui.available_rect_before_wrap();
        self.board_size = rect.size();
        self.draw_background(ui, rect)?;

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
        let base = base_radius(e)?;
        let mf = meta(ui)?;
        let center = mf.canvas_to_screen_pos(node.location()) + rect.left_top().to_vec2();
        let corner = center + Vec2::splat(base * e.size * e.boost * mf.zoom);
        Ok(Some((idx, Rect::from_center_size(corner, Vec2::splat(14.0)))))
    }

    fn press_on_empty(&self, ui: &egui::Ui, rect: Rect, p: Pos2) -> Result<bool, Error> {
        if !rect.contains(p) {
            return Ok(false);
        }
        if self
            .resize_handle_rect(ui, rect)?
            .is_some_and(|(_, h)| h.contains(p))
        {
            return Ok(false);
        }
        Ok(self.node_at_screen(ui, rect, p)?.is_none())
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
        let (mid_pressed, primary_pressed, mid_down, primary_down, pointer, delta) = ui.input(|i| {
            (
                i.pointer.button_pressed(egui::PointerButton::Middle),
                i.pointer.primary_pressed(),
                i.pointer.button_down(egui::PointerButton::Middle),
                i.pointer.primary_down(),
                i.pointer.interact_pos(),
                i.pointer.delta(),
            )
        });

        if !self.panning
            && (mid_pressed || primary_pressed)
            && self.node_drag.is_none()
            && self.edge_drag.is_none()
            && self.resize.is_none()
            && self.band.is_none()
            && self.label_edit.is_none()
            && let Some(p) = pointer.filter(|p| rect.contains(*p))
            && pan_should_start(primary_pressed, mid_pressed, self.press_on_empty(ui, rect, p)?)
        {
            self.panning = true;
            crate::dlog_debug!("board", "pan start");
        }
        if !self.panning {
            return Ok(());
        }
        if !mid_down && !primary_down {
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
                i.pointer.secondary_pressed(),
                i.pointer.secondary_down(),
                i.pointer.interact_pos(),
            )
        });

        if pressed
            && self.band.is_none()
            && self.node_drag.is_none()
            && self.edge_drag.is_none()
            && self.resize.is_none()
            && self.label_edit.is_none()
            && let Some(p) = pointer
            && band_should_start(pressed, self.press_on_empty(ui, rect, p)?)
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
            let base = base_radius(e)?;
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
        let base = base_radius(e)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cover_rect_covers_centered_and_keeps_aspect() {
        let board = Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::new(800.0, 600.0));

        let wide = cover_rect(board, Vec2::new(1600.0, 400.0));
        assert!(wide.contains_rect(board));
        assert!((wide.center() - board.center()).length() < 1e-3);
        assert!((wide.width() / wide.height() - 1600.0 / 400.0).abs() < 1e-3);

        let tall = cover_rect(board, Vec2::new(400.0, 1600.0));
        assert!(tall.contains_rect(board));
        assert!((tall.center() - board.center()).length() < 1e-3);
        assert!((tall.width() / tall.height() - 400.0 / 1600.0).abs() < 1e-3);
    }

    use crate::ui::testkit::{graph, harness};

    fn board_rect() -> Rect {
        Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))
    }

    fn raw(events: Vec<egui::Event>) -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(board_rect()),
            events,
            ..Default::default()
        }
    }

    fn moved(to: Pos2) -> egui::RawInput {
        raw(vec![egui::Event::PointerMoved(to)])
    }

    fn button(pos: Pos2, which: egui::PointerButton, pressed: bool) -> egui::RawInput {
        raw(vec![egui::Event::PointerButton {
            pos,
            button: which,
            pressed,
            modifiers: egui::Modifiers::default(),
        }])
    }

    fn drive(app: &mut RedstringApp, ctx: &egui::Context, input: egui::RawInput) -> Camera {
        let mut cam = Camera::default();
        let _full = ctx.run_ui(input, |ui| {
            let rect = board_rect();
            app.board_size = rect.size();
            app.edge_drag_input(ui, rect).unwrap();
            app.pan_input(ui, rect).unwrap();
            app.node_drag_input(ui, rect).unwrap();
            app.band_select(ui, rect).unwrap();
            cam = camera_read(ui).unwrap();
        });
        cam
    }

    fn set_loc(app: &mut RedstringApp, idx: NodeIndex, p: Pos2) {
        app.g.node_mut(idx).unwrap().set_location(p);
    }

    #[test]
    fn primary_on_empty_or_middle_starts_a_pan() {
        assert!(pan_should_start(true, false, true), "left press on empty pans");
        assert!(
            !pan_should_start(true, false, false),
            "left press on a node falls through to node drag, not pan"
        );
        assert!(pan_should_start(false, true, false), "middle press pans over a node");
        assert!(pan_should_start(false, true, true), "middle press pans on empty too");
        assert!(!pan_should_start(false, false, true), "no button, no pan");
    }

    #[test]
    fn secondary_on_empty_starts_a_band() {
        assert!(band_should_start(true, true), "right press on empty rubber-bands");
        assert!(
            !band_should_start(true, false),
            "right press on a node falls through to edge drag, not band"
        );
        assert!(!band_should_start(false, true), "no right press, no band");
    }

    #[test]
    fn left_drag_on_empty_pans_and_selects_nothing() {
        let mut app = harness::app().unwrap();
        let ctx = egui::Context::default();
        let start = Pos2::new(400.0, 300.0);

        drive(&mut app, &ctx, moved(start));
        let cam0 = drive(&mut app, &ctx, button(start, egui::PointerButton::Primary, true));
        assert!(app.panning, "a left press on empty space starts a pan");
        assert!(app.band.is_none(), "left-drag pans, it does not rubber-band");

        let cam1 = drive(&mut app, &ctx, moved(Pos2::new(460.0, 340.0)));
        assert!((cam1.x - cam0.x - 60.0).abs() < 0.5, "camera panned x by ~60, got {}", cam1.x - cam0.x);
        assert!((cam1.y - cam0.y - 40.0).abs() < 0.5, "camera panned y by ~40, got {}", cam1.y - cam0.y);
        assert_eq!(app.g.selected_nodes().len(), 0, "panning selects nothing");

        drive(&mut app, &ctx, button(Pos2::new(460.0, 340.0), egui::PointerButton::Primary, false));
        assert!(!app.panning, "releasing the button ends the pan");
    }

    #[test]
    fn middle_drag_still_pans() {
        let mut app = harness::app().unwrap();
        let ctx = egui::Context::default();
        let start = Pos2::new(200.0, 200.0);

        drive(&mut app, &ctx, moved(start));
        drive(&mut app, &ctx, button(start, egui::PointerButton::Middle, true));
        assert!(app.panning, "a middle press starts a pan");
        let cam0 = drive(&mut app, &ctx, moved(start));
        let cam1 = drive(&mut app, &ctx, moved(Pos2::new(230.0, 250.0)));
        assert!(
            (cam1.x - cam0.x - 30.0).abs() < 0.5 && (cam1.y - cam0.y - 50.0).abs() < 0.5,
            "middle-drag pans by the delta"
        );
        drive(&mut app, &ctx, button(Pos2::new(230.0, 250.0), egui::PointerButton::Middle, false));
        assert!(!app.panning);
    }

    #[test]
    fn right_drag_on_empty_bands_and_does_not_pan() {
        let mut app = harness::app().unwrap();
        let ctx = egui::Context::default();
        let start = Pos2::new(100.0, 100.0);

        drive(&mut app, &ctx, moved(start));
        drive(&mut app, &ctx, button(start, egui::PointerButton::Secondary, true));
        assert!(app.band.is_some(), "a right press on empty space starts a rubber-band");
        assert!(!app.panning, "right-drag selects, it does not pan");

        drive(&mut app, &ctx, moved(Pos2::new(300.0, 260.0)));
        assert!(app.band.is_some(), "the band stays active while the button is held");

        drive(&mut app, &ctx, button(Pos2::new(300.0, 260.0), egui::PointerButton::Secondary, false));
        assert!(app.band.is_none(), "releasing the button ends the band");
    }

    #[test]
    fn rubber_band_selects_the_nodes_it_covers() {
        let mut app = harness::app().unwrap();
        let inside = graph::add(&mut app, EntityKind::Person, "Inside").unwrap();
        let outside = graph::add(&mut app, EntityKind::Person, "Outside").unwrap();
        set_loc(&mut app, inside, Pos2::new(120.0, 120.0));
        set_loc(&mut app, outside, Pos2::new(600.0, 500.0));

        let ctx = egui::Context::default();
        let _full = ctx.run_ui(raw(vec![]), |ui| {
            camera_write(ui, Camera { x: 0.0, y: 0.0, zoom: 1.0 }).unwrap();
            let band = Rect::from_min_max(Pos2::new(80.0, 80.0), Pos2::new(220.0, 220.0));
            app.apply_band(ui, board_rect(), band).unwrap();
        });
        assert!(app.g.node(inside).unwrap().selected(), "a node inside the band is selected");
        assert!(!app.g.node(outside).unwrap().selected(), "a node outside the band is left alone");
    }

    #[test]
    fn left_drag_on_a_node_moves_it() {
        let mut app = harness::app().unwrap();
        let idx = graph::add(&mut app, EntityKind::Person, "Movable").unwrap();
        set_loc(&mut app, idx, Pos2::new(100.0, 100.0));
        let ctx = egui::Context::default();

        drive(&mut app, &ctx, moved(Pos2::new(100.0, 100.0)));
        let _full = ctx.run_ui(
            button(Pos2::new(100.0, 100.0), egui::PointerButton::Primary, true),
            |ui| {
                camera_write(ui, Camera { x: 0.0, y: 0.0, zoom: 1.0 }).unwrap();
                app.begin_node_drag(idx, Pos2::new(100.0, 100.0));
                app.node_drag_input(ui, board_rect()).unwrap();
            },
        );
        assert!(app.node_drag.is_some(), "the node drag is armed");
        assert!(!app.panning, "dragging a node does not pan the board");

        drive(&mut app, &ctx, moved(Pos2::new(180.0, 150.0)));
        let at = app.g.node(idx).unwrap().location();
        assert!(
            (at.x - 180.0).abs() < 0.5 && (at.y - 150.0).abs() < 0.5,
            "the node followed the pointer, at ({}, {})",
            at.x,
            at.y
        );

        drive(&mut app, &ctx, button(Pos2::new(180.0, 150.0), egui::PointerButton::Primary, false));
        assert!(app.node_drag.is_none(), "releasing ends the node drag");
    }
}

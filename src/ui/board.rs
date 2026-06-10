use egui::{Pos2, Rect, Vec2};
use egui_graphs::{
    GraphView, MetadataFrame, SettingsInteraction, SettingsNavigation, SettingsStyle,
};
use petgraph::graph::NodeIndex;

use crate::error::RsResult;
use crate::model::{Camera, EntityKind, KIND_META};
use crate::shapes::{IMG_RADIUS, NODE_RADIUS, SourcedEdgeShape};

use super::app::{BOARD_META_ID, RedstringApp};

const SCROLL_ZOOM_SPEED: f32 = 1.0 / 200.0;

const MIN_ZOOM: f32 = 0.01;
const MAX_ZOOM: f32 = 30.0;

pub fn meta(ui: &egui::Ui) -> RsResult<MetadataFrame> {
    Ok(MetadataFrame::new(Some(BOARD_META_ID.to_owned())).load(ui))
}

pub fn camera_read(ui: &egui::Ui) -> RsResult<Camera> {
    let mf = meta(ui)?;
    Ok(Camera {
        x: mf.pan.x,
        y: mf.pan.y,
        zoom: mf.zoom,
    })
}

fn sane_zoom(z: f32) -> f32 {
    if !z.is_finite() {
        return 1.0;
    }
    z.clamp(MIN_ZOOM, MAX_ZOOM)
}

pub fn camera_write(ui: &mut egui::Ui, cam: Camera) -> RsResult<()> {
    let mut mf = meta(ui)?;
    mf.pan = Vec2::new(cam.x, cam.y);
    mf.zoom = sane_zoom(cam.zoom);
    mf.save(ui);
    Ok(())
}

fn scroll_zoom(ui: &mut egui::Ui) -> RsResult<()> {
    let board = ui.available_rect_before_wrap();
    let (dy, hover) = ui.input(|i| (i.smooth_scroll_delta.y, i.pointer.hover_pos()));
    if dy == 0.0 {
        return Ok(());
    }
    let Some(pointer) = hover else {
        return Ok(());
    };
    if !board.contains(pointer) {
        return Ok(());
    }

    let mut mf = meta(ui)?;
    let new_zoom = sane_zoom(mf.zoom * (dy * SCROLL_ZOOM_SPEED).exp());

    let local = pointer - board.left_top();
    mf.pan = local - (local - mf.pan) * (new_zoom / mf.zoom);
    mf.zoom = new_zoom;
    mf.save(ui);
    Ok(())
}

impl RedstringApp {
    pub fn zoom_bar(&mut self, ui: &mut egui::Ui) -> RsResult<()> {
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

    pub fn board(&mut self, ui: &mut egui::Ui) -> RsResult<()> {
        let rect = ui.available_rect_before_wrap();
        self.board_size = rect.size();

        self.edge_drag_input(ui, rect)?;
        self.node_drag_input(ui, rect)?;
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
                        .with_zoom_and_pan_enabled(self.node_drag.is_none()),
                )
                .with_styles(&SettingsStyle::default().with_labels_always(true)),
        );

        self.edge_drag_overlay(ui, rect)?;
        self.resize_handle(ui, rect)?;
        self.kind_drop(ui, rect)?;
        self.verb_picker(ui)?;
        self.label_editor(ui)?;
        Ok(())
    }

    pub fn node_at_screen(
        &self,
        ui: &egui::Ui,
        rect: Rect,
        screen: Pos2,
    ) -> RsResult<Option<NodeIndex>> {
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

    fn resize_handle_rect(&self, ui: &egui::Ui, rect: Rect) -> RsResult<Option<(NodeIndex, Rect)>> {
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
        let corner = center + Vec2::splat(base * e.size * mf.zoom);
        Ok(Some((idx, Rect::from_center_size(corner, Vec2::splat(14.0)))))
    }

    fn node_drag_input(&mut self, ui: &egui::Ui, rect: Rect) -> RsResult<()> {
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

        let Some((idx, off)) = self.node_drag else {
            return Ok(());
        };
        if !down {
            self.end_node_drag(idx);
            return Ok(());
        }
        let Some(p) = pointer else {
            return Ok(());
        };
        let canvas_p = meta(ui)?.screen_to_canvas_pos((p - rect.left_top()).to_pos2());
        let Some(n) = self.g.node_mut(idx) else {
            self.node_drag = None;
            return Ok(());
        };
        n.set_location(canvas_p - off);
        Ok(())
    }

    fn begin_node_drag(&mut self, idx: NodeIndex, canvas_p: Pos2) {
        let Some(n) = self.g.node_mut(idx) else {
            return;
        };
        let off = canvas_p - n.location();
        n.set_dragged(true);
        self.node_drag = Some((idx, off));
    }

    fn end_node_drag(&mut self, idx: NodeIndex) {
        self.node_drag = None;
        let Some(n) = self.g.node_mut(idx) else {
            return;
        };
        n.set_dragged(false);
    }

    fn resize_handle(&mut self, ui: &mut egui::Ui, rect: Rect) -> RsResult<()> {
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
            self.set_node_size(idx, (new_half / mf.zoom / base).clamp(0.5, 12.0))?;
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

    fn kind_drop(&mut self, ui: &mut egui::Ui, rect: Rect) -> RsResult<()> {
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
        self.ne_kind = kind;
        self.label_edit = Some((idx, p));
        self.label_edit_buf.clear();
        self.label_edit_focus = true;
        Ok(())
    }
}

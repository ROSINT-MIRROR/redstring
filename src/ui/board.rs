//! The corkboard: the GraphView itself plus camera plumbing, plain-wheel
//! zoom, custom node dragging, the resize handle and the palette drop target.

use egui::{Pos2, Rect, Vec2};
use egui_graphs::{
    GraphView, MetadataFrame, SettingsInteraction, SettingsNavigation, SettingsStyle,
};
use petgraph::graph::NodeIndex;

use crate::model::{Camera, EntityKind};
use crate::shapes::{IMG_RADIUS, NODE_RADIUS, SourcedEdgeShape};

use super::app::{BOARD_META_ID, RedstringApp};

/// Plain-wheel zoom feel: factor = exp of scroll points times this. Mirrors
/// egui's own ctrl+scroll mapping of 1/200, so an unmodified wheel notch of
/// 40 points zooms about 1.22x and trackpad scroll stays smooth.
const SCROLL_ZOOM_SPEED: f32 = 1.0 / 200.0;

/// The board's egui_graphs metadata for this frame.
pub(super) fn meta(ui: &egui::Ui) -> MetadataFrame {
    MetadataFrame::new(Some(BOARD_META_ID.to_owned())).load(ui)
}

pub(super) fn camera_read(ui: &egui::Ui) -> Camera {
    let mf = meta(ui);
    Camera {
        x: mf.pan.x,
        y: mf.pan.y,
        zoom: mf.zoom,
    }
}

/// Humanly useful zoom range. Restored cameras and wheel zoom both clamp to
/// it — a poisoned camera in a case file must not detonate glyph
/// rasterization on load.
const MIN_ZOOM: f32 = 0.01;
const MAX_ZOOM: f32 = 30.0;

/// Clamp a zoom into the useful range; junk values land on 1.
fn sane_zoom(z: f32) -> f32 {
    if !z.is_finite() {
        return 1.0;
    }
    z.clamp(MIN_ZOOM, MAX_ZOOM)
}

pub(super) fn camera_write(ui: &mut egui::Ui, cam: Camera) {
    let mut mf = meta(ui);
    mf.pan = Vec2::new(cam.x, cam.y);
    mf.zoom = sane_zoom(cam.zoom);
    mf.save(ui);
}

/// Plain mouse-wheel zoom over the board, centred on the pointer.
///
/// egui_graphs 0.30 only zooms on zoom_delta, which egui feeds exclusively
/// from ctrl/cmd+scroll and pinch gestures; an unmodified wheel lands in
/// smooth_scroll_delta instead — the input state routes a wheel event to one
/// or the other, never both. So driving zoom from smooth_scroll_delta here
/// can't double-apply with the lib, ctrl+scroll and trackpad pinch keep
/// working through the lib's own path, and the lib's pointer-button handling
/// is untouched.
///
/// Must run inside the central panel BEFORE the GraphView is added: the
/// widget loads the metadata at the start of its pass, so a write here is
/// what it draws with this same frame.
fn scroll_zoom(ui: &mut egui::Ui) {
    // the GraphView allocates exactly the remaining rect after us
    let board = ui.available_rect_before_wrap();
    let (dy, hover) = ui.input(|i| (i.smooth_scroll_delta.y, i.pointer.hover_pos()));
    if dy == 0.0 {
        return;
    }
    let Some(pointer) = hover else { return };
    if !board.contains(pointer) {
        return; // scrolling the side panels must not zoom the board
    }

    let mut mf = meta(ui);
    let new_zoom = sane_zoom(mf.zoom * (dy * SCROLL_ZOOM_SPEED).exp());
    // pan is widget-local — the lib adds the rect corner at draw time — so
    // the pointer must be too. Same zoom-about-pointer invariant as the
    // lib's own zoom handler.
    let local = pointer - board.left_top();
    mf.pan = local - (local - mf.pan) * (new_zoom / mf.zoom);
    mf.zoom = new_zoom;
    mf.save(ui);
}

impl RedstringApp {
    /// The corkboard: the GraphView plus every overlay gesture around it.
    pub(super) fn board(&mut self, ui: &mut egui::Ui) {
        // the rect the GraphView is about to allocate — it takes all available space
        let rect = ui.available_rect_before_wrap();

        // Edge-drag input runs BEFORE the GraphView so that on the press
        // frame the view is already built with node-dragging gated off; an
        // ungated right-press over a node would start a lib node drag.
        self.edge_drag_input(ui, rect);
        self.node_drag_input(ui, rect);
        scroll_zoom(ui);

        ui.add(
            &mut GraphView::<_, _, _, _, _, SourcedEdgeShape>::new(&mut self.g)
                .with_id(Some(BOARD_META_ID.to_owned()))
                .with_interactions(
                    &SettingsInteraction::default()
                        // node moving is ours: the lib's drag snap-centers
                        // the node on the cursor
                        .with_dragging_enabled(false)
                        .with_node_selection_enabled(true)
                        .with_edge_selection_enabled(true),
                )
                .with_navigations(
                    &SettingsNavigation::default()
                        .with_fit_to_screen_enabled(false)
                        // gate panning off while a node is being moved — with
                        // lib dragging disabled the view would otherwise pan
                        .with_zoom_and_pan_enabled(self.node_drag.is_none()),
                )
                .with_styles(&SettingsStyle::default().with_labels_always(true)),
        );

        self.edge_drag_overlay(ui, rect);
        self.resize_handle(ui, rect);
        self.kind_drop(ui, rect);
        self.verb_picker(ui);
        self.label_editor(ui);
    }

    /// Which entity is under an absolute screen position? Camera transform
    /// via the board metadata, then each node's drawn-geometry hit test.
    pub(super) fn node_at_screen(&self, ui: &egui::Ui, rect: Rect, screen: Pos2) -> Option<NodeIndex> {
        let mf = meta(ui);
        self.g
            .node_by_screen_pos(&mf, (screen - rect.left_top()).to_pos2())
    }

    /// The node the resize handle targets: the in-flight resize, else the
    /// single selected node.
    fn resize_target(&self) -> Option<NodeIndex> {
        if self.resize.is_some() {
            return self.resize;
        }
        let sel = self.g.selected_nodes();
        (sel.len() == 1).then(|| sel[0])
    }

    /// Where the resize handle sits on screen. Shared by `resize_handle` and
    /// `node_drag_input`, which must not steal its press.
    fn resize_handle_rect(&self, ui: &egui::Ui, rect: Rect) -> Option<(NodeIndex, Rect)> {
        let idx = self.resize_target()?;
        let node = self.g.node(idx)?;
        let e = node.payload();
        let base = if e.image.is_some() { IMG_RADIUS } else { NODE_RADIUS };
        let mf = meta(ui);
        let center = mf.canvas_to_screen_pos(node.location()) + rect.left_top().to_vec2();
        let corner = center + Vec2::splat(base * e.size * mf.zoom);
        Some((idx, Rect::from_center_size(corner, Vec2::splat(14.0))))
    }

    /// Left-press on an entity starts a move; the node follows the pointer
    /// keeping the grab offset — no snap-to-center. Runs before the GraphView
    /// is built so panning is gated off on the press frame.
    pub(super) fn node_drag_input(&mut self, ui: &egui::Ui, rect: Rect) {
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
                .resize_handle_rect(ui, rect)
                .is_some_and(|(_, h)| h.contains(p))
            && let Some(idx) = self.node_at_screen(ui, rect, p)
        {
            let canvas_p = meta(ui).screen_to_canvas_pos((p - rect.left_top()).to_pos2());
            self.begin_node_drag(idx, canvas_p);
        }

        let Some((idx, off)) = self.node_drag else { return };
        if !down {
            self.end_node_drag(idx);
            return;
        }
        let Some(p) = pointer else { return };
        let canvas_p = meta(ui).screen_to_canvas_pos((p - rect.left_top()).to_pos2());
        let Some(n) = self.g.node_mut(idx) else {
            self.node_drag = None; // node vanished mid-drag — delete / hot-reload
            return;
        };
        n.set_location(canvas_p - off);
    }

    fn begin_node_drag(&mut self, idx: NodeIndex, canvas_p: Pos2) {
        let Some(n) = self.g.node_mut(idx) else { return };
        let off = canvas_p - n.location();
        n.set_dragged(true);
        self.node_drag = Some((idx, off));
    }

    fn end_node_drag(&mut self, idx: NodeIndex) {
        self.node_drag = None;
        let Some(n) = self.g.node_mut(idx) else { return };
        n.set_dragged(false);
    }

    /// Click-select a single entity and a drag handle appears at its
    /// bottom-right corner. Dragging it scales the node about its center —
    /// geometry, icon and text all ride the entity size.
    pub(super) fn resize_handle(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let Some((idx, hrect)) = self.resize_handle_rect(ui, rect) else {
            self.resize = None; // no target: selection gone or node vanished
            return;
        };
        let Some(node) = self.g.node(idx) else {
            self.resize = None;
            return;
        };
        let e = node.payload();
        let base = if e.image.is_some() { IMG_RADIUS } else { NODE_RADIUS };
        let loc = node.location();

        let mf = meta(ui);
        let center = mf.canvas_to_screen_pos(loc) + rect.left_top().to_vec2();
        let corner = hrect.center();
        let resp = ui.interact(hrect, egui::Id::new("rs_resize_handle"), egui::Sense::drag());
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
            // corner tracks the pointer along the dominant axis
            let new_half = d.x.max(d.y).max(4.0);
            self.set_node_size(idx, (new_half / mf.zoom / base).clamp(0.5, 12.0));
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
    }

    /// Accept a kind dragged out of the palette: spawn the entity at the drop
    /// point and open the inline name editor there, input pre-focused.
    pub(super) fn kind_drop(&mut self, ui: &mut egui::Ui, rect: Rect) {
        if !egui::DragAndDrop::has_payload_of_type::<EntityKind>(ui.ctx()) {
            return;
        }
        let pointer = ui.input(|i| i.pointer.latest_pos());
        let over = pointer.is_some_and(|p| rect.contains(p));
        if over {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Copy);
        }
        if !ui.input(|i| i.pointer.any_released()) {
            return;
        }
        // take, not peek, so a drop outside the board clears the payload too
        let Some(kind) = egui::DragAndDrop::take_payload::<EntityKind>(ui.ctx()) else {
            return;
        };
        let (Some(p), true) = (pointer, over) else { return };
        let kind = *kind;
        let canvas = meta(ui).screen_to_canvas_pos((p - rect.left_top()).to_pos2());
        let idx = self.add_entity(kind, kind.label().to_owned(), canvas);
        self.ne_kind = kind;
        self.label_edit = Some((idx, p));
        self.label_edit_buf.clear();
        self.label_edit_focus = true;
    }
}

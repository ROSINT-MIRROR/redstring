use egui::text::{CCursor, CCursorRange};
use egui::{Rect, RichText, Shape, Stroke};
use petgraph::graph::NodeIndex;

use crate::error::Error;
use crate::model::{Origin, RELATION_META, Relation, RelationKind, Verification};
use crate::shapes::{IMG_RADIUS, LABEL_GAP, LABEL_SIZE, LABEL_WRAP, NODE_RADIUS};

use super::app::{ALARM_RED, RedstringApp};
use super::board::meta;

const MAX_LABEL_PX: f32 = 256.0;
const DRAG_THRESHOLD: f32 = 4.0;

#[derive(Clone, Copy, PartialEq, Debug)]
enum RightRelease {
    Idle,
    Menu(NodeIndex),
    Link(NodeIndex, NodeIndex),
    CancelLink,
}

fn right_release_outcome(
    edge_drag: Option<NodeIndex>,
    press_node: Option<NodeIndex>,
    release_node: Option<NodeIndex>,
) -> RightRelease {
    let Some(from) = edge_drag else {
        let Some(node) = press_node else {
            return RightRelease::Idle;
        };
        if release_node == Some(node) {
            return RightRelease::Menu(node);
        }
        return RightRelease::Idle;
    };
    let Some(to) = release_node else {
        return RightRelease::CancelLink;
    };
    if to == from {
        return RightRelease::CancelLink;
    }
    RightRelease::Link(from, to)
}

fn edge_start_node(
    edge_drag: Option<NodeIndex>,
    down: bool,
    press_node: Option<NodeIndex>,
    moved: bool,
) -> Option<NodeIndex> {
    if edge_drag.is_some() || !down || !moved {
        return None;
    }
    press_node
}

impl RedstringApp {
    pub fn edge_drag_input(&mut self, ui: &egui::Ui, rect: Rect) -> Result<(), Error> {
        let (pressed, released, down, escape, pointer) = ui.input(|i| {
            (
                i.pointer.secondary_pressed(),
                i.pointer.secondary_released(),
                i.pointer.secondary_down(),
                i.key_pressed(egui::Key::Escape),
                i.pointer.interact_pos(),
            )
        });

        if pressed
            && self.pending_edge.is_none()
            && self.node_drag.is_none()
            && let Some(p) = pointer.filter(|p| rect.contains(*p))
        {
            let node = self.node_at_screen(ui, rect, p)?;
            self.right_press = Some((node, p));
        }

        if escape {
            if self.edge_drag.is_some() {
                self.status = "link cancelled".into();
            }
            self.edge_drag = None;
            self.right_press = None;
            return Ok(());
        }

        let press_node = self.right_press.and_then(|(n, _)| n);
        let moved = self.right_moved(pointer);
        if self.edge_drag.is_none()
            && let Some(node) = edge_start_node(self.edge_drag, down, press_node, moved)
        {
            self.edge_drag = Some(node);
            self.status = "release on a target entity to link".into();
        }

        if released {
            return self.right_release(ui, rect, pointer);
        }
        if !down {
            self.edge_drag = None;
            self.right_press = None;
        }
        Ok(())
    }

    fn right_moved(&self, pointer: Option<egui::Pos2>) -> bool {
        let Some((_, origin)) = self.right_press else {
            return false;
        };
        let Some(p) = pointer else {
            return false;
        };
        (p - origin).length() > DRAG_THRESHOLD
    }

    fn right_release(
        &mut self,
        ui: &egui::Ui,
        rect: Rect,
        pointer: Option<egui::Pos2>,
    ) -> Result<(), Error> {
        let press = self.right_press.take();
        let edge_from = self.edge_drag.take();
        let press_node = press.and_then(|(n, _)| n);
        let release_node = self.released_target(ui, rect, pointer)?;
        match right_release_outcome(edge_from, press_node, release_node) {
            RightRelease::Idle => {}
            RightRelease::CancelLink => {
                self.status = "link cancelled".into();
            }
            RightRelease::Menu(node) => {
                let Some(p) = pointer else {
                    return Ok(());
                };
                self.node_menu = Some((node, p));
                self.node_menu_just_opened = true;
                self.status = "expand or collapse this node's subtree".into();
            }
            RightRelease::Link(from, to) => {
                let Some(p) = pointer else {
                    return Ok(());
                };
                self.verb_filter.clear();
                self.verb_filter_focus = true;
                self.pending_edge = Some((from, to, p));
            }
        }
        Ok(())
    }

    fn released_target(
        &self,
        ui: &egui::Ui,
        rect: Rect,
        pointer: Option<egui::Pos2>,
    ) -> Result<Option<NodeIndex>, Error> {
        let Some(p) = pointer.filter(|p| rect.contains(*p)) else {
            return Ok(None);
        };
        self.node_at_screen(ui, rect, p)
    }

    pub fn edge_drag_overlay(&mut self, ui: &egui::Ui, rect: Rect) -> Result<(), Error> {
        let Some(from) = self.edge_drag else {
            return Ok(());
        };
        let Some(node) = self.g.node(from) else {
            self.edge_drag = None;
            return Ok(());
        };
        let Some(pointer) = ui.input(|i| i.pointer.latest_pos()) else {
            return Ok(());
        };

        let a = meta(ui)?.canvas_to_screen_pos(node.location()) + rect.left_top().to_vec2();
        let stroke = Stroke::new(2.0_f32, ALARM_RED);
        let painter = ui.painter().with_clip_rect(rect);
        painter.extend(Shape::dashed_line(&[a, pointer], stroke, 9.0, 6.0));
        painter.circle_filled(pointer, 3.5, ALARM_RED);
        Ok(())
    }

    pub fn verb_picker(&mut self, ui: &mut egui::Ui) -> Result<(), Error> {
        let Some((from, to, at)) = self.pending_edge else {
            return Ok(());
        };

        if self.g.node(from).is_none() || self.g.node(to).is_none() {
            self.pending_edge = None;
            return Ok(());
        }

        let (from_l, to_l) = (self.node_label(from)?, self.node_label(to)?);
        let mut picked: Option<RelationKind> = None;
        let mut close = ui.input(|i| i.key_pressed(egui::Key::Escape));

        let area = egui::Area::new(egui::Id::new("rs_verb_picker"))
            .order(egui::Order::Foreground)
            .fixed_pos(at)
            .constrain(true)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_max_width(230.0);
                    ui.label(RichText::new(format!("{from_l}  —?→  {to_l}")).small().weak());

                    let filter = ui.text_edit_singleline(&mut self.verb_filter);
                    if std::mem::take(&mut self.verb_filter_focus) {
                        filter.request_focus();
                    }

                    let needle = self.verb_filter.trim().to_lowercase();
                    let verbs: Vec<&RelationKind> = RELATION_META
                        .iter()
                        .filter(|m| needle.is_empty() || m.label.to_lowercase().contains(&needle))
                        .map(|m| &m.kind)
                        .collect();

                    if verbs.len() == 1
                        && filter.lost_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::Enter))
                    {
                        picked = Some(*verbs[0]);
                    }

                    ui.separator();
                    egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                        for k in verbs {
                            if ui.button(RELATION_META[*k as usize].label).clicked() {
                                picked = Some(*k);
                            }
                        }
                    });
                });
            });

        if area.response.clicked_elsewhere() {
            close = true;
        }
        close |= self.commit_verb(from, to, &from_l, &to_l, picked)?;
        if close {
            self.pending_edge = None;
        }
        Ok(())
    }

    fn commit_verb(
        &mut self,
        from: NodeIndex,
        to: NodeIndex,
        from_l: &str,
        to_l: &str,
        picked: Option<RelationKind>,
    ) -> Result<bool, Error> {
        let Some(kind) = picked else { return Ok(false) };
        let label = RELATION_META[kind as usize].label;
        let relation =
            Relation::stamp(kind, Origin::Manual, Verification::Unreviewed, self.author.clone())?;
        self.g.add_edge_with_label(from, to, relation, label.into());
        self.status = format!("linked: {from_l} —{label}→ {to_l} (unsourced — attach a source!)");
        Ok(true)
    }

    pub fn label_editor(&mut self, ui: &mut egui::Ui, rect: Rect) -> Result<(), Error> {
        self.label_double_click(ui, rect)?;
        let Some(idx) = self.label_edit else {
            return Ok(());
        };
        let Some((zone, font_px)) = self.label_zone(ui, rect, idx)? else {
            self.label_edit = None;
            return Ok(());
        };

        let mut commit = false;
        let mut close = ui.input(|i| i.key_pressed(egui::Key::Escape));
        let just_opened = self.label_edit_focus;

        let edit_font = egui::FontId::monospace(font_px);
        let text_size = ui
            .ctx()
            .fonts_mut(|f| {
                f.layout_no_wrap(
                    self.label_edit_buf.clone(),
                    edit_font.clone(),
                    egui::Color32::WHITE,
                )
            })
            .size();
        let width = text_size.x + 2.0 * font_px;
        let pos = zone.shrink(4.0).left_top();
        let edit_rect = Rect::from_min_size(pos, egui::Vec2::new(width, text_size.y));

        ui.painter()
            .with_clip_rect(rect)
            .rect_filled(zone, 2.0, ui.visuals().panel_fill);

        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(edit_rect)
                .layout(egui::Layout::left_to_right(egui::Align::TOP)),
        );
        let out = egui::TextEdit::singleline(&mut self.label_edit_buf)
            .font(edit_font)
            .desired_width(width)
            .frame(egui::Frame::NONE)
            .margin(egui::Margin::ZERO)
            .show(&mut child);
        if std::mem::take(&mut self.label_edit_focus) {
            out.response.request_focus();
            let mut state = out.state;
            state.cursor.set_char_range(Some(CCursorRange::two(
                CCursor::new(0),
                CCursor::new(self.label_edit_buf.chars().count()),
            )));
            state.store(ui.ctx(), out.response.id);
        }
        if out.response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            commit = true;
        }

        if out.response.clicked_elsewhere() && !just_opened {
            commit = true;
        }
        if commit {
            let label = self.label_edit_buf.trim().to_owned();
            if !label.is_empty() && label != self.node_payload_label(idx) {
                self.apply_label(idx, &label)?;
                self.sync_visuals()?;
                self.status = format!("named {label}");
            }
            close = true;
        }
        if close {
            self.label_edit = None;
        }
        Ok(())
    }

    fn label_double_click(&mut self, ui: &egui::Ui, rect: Rect) -> Result<(), Error> {
        if self.label_edit.is_some() {
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
        let nodes: Vec<NodeIndex> = self.g.g().node_indices().collect();
        for idx in nodes {
            let Some((zone, _)) = self.label_zone(ui, rect, idx)? else {
                continue;
            };
            if zone.contains(p) {
                self.start_label_edit(idx)?;
                return Ok(());
            }
        }
        Ok(())
    }

    pub fn start_label_edit(&mut self, idx: NodeIndex) -> Result<(), Error> {
        self.label_edit_buf = self.node_payload_label(idx);
        self.label_edit = Some(idx);
        self.label_edit_focus = true;
        Ok(())
    }

    fn node_payload_label(&self, idx: NodeIndex) -> String {
        let Some(n) = self.g.node(idx) else {
            return String::new();
        };
        n.payload().label.clone()
    }

    fn label_zone(
        &self,
        ui: &egui::Ui,
        rect: Rect,
        idx: NodeIndex,
    ) -> Result<Option<(Rect, f32)>, Error> {
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
        let vsize = e.size * e.boost;
        let half = base * vsize * mf.zoom;
        let font_px = (LABEL_SIZE * vsize * mf.zoom).min(MAX_LABEL_PX);
        let mut job = egui::text::LayoutJob::simple(
            e.label.clone(),
            egui::FontId::new(font_px, egui::FontFamily::Monospace),
            egui::Color32::WHITE,
            LABEL_WRAP * vsize * mf.zoom,
        );
        job.halign = egui::Align::Center;
        let galley = ui.ctx().fonts_mut(|f| f.layout_job(job));
        let gap = LABEL_GAP * vsize * mf.zoom;
        let top_left = egui::Pos2::new(center.x - galley.size().x / 2.0, center.y + half + gap);
        Ok(Some((
            Rect::from_min_size(top_left, galley.size()).expand(4.0),
            font_px,
        )))
    }
}

#[cfg(test)]
mod tests {
    use petgraph::graph::NodeIndex;

    use super::{RightRelease, edge_start_node, right_release_outcome};

    fn n(i: usize) -> NodeIndex {
        NodeIndex::new(i)
    }

    #[test]
    fn simple_right_click_on_node_opens_menu() {
        let a = n(0);
        assert_eq!(
            right_release_outcome(None, Some(a), Some(a)),
            RightRelease::Menu(a)
        );
    }

    #[test]
    fn right_click_released_off_node_does_not_open_menu() {
        let a = n(0);
        assert_eq!(right_release_outcome(None, Some(a), None), RightRelease::Idle);
        assert_eq!(right_release_outcome(None, None, None), RightRelease::Idle);
    }

    #[test]
    fn right_drag_between_nodes_links_and_never_opens_menu() {
        let a = n(0);
        let b = n(1);
        assert_eq!(edge_start_node(None, true, Some(a), true), Some(a));
        assert_eq!(edge_start_node(None, true, Some(a), false), None);
        assert_eq!(edge_start_node(None, false, Some(a), true), None);
        assert_eq!(edge_start_node(Some(a), true, Some(a), true), None);
        assert_eq!(edge_start_node(None, true, None, true), None);
        assert_eq!(
            right_release_outcome(Some(a), Some(a), Some(b)),
            RightRelease::Link(a, b)
        );
        assert_eq!(
            right_release_outcome(Some(a), Some(a), Some(a)),
            RightRelease::CancelLink
        );
        assert_eq!(
            right_release_outcome(Some(a), Some(a), None),
            RightRelease::CancelLink
        );
    }

    fn editor_pass(ctx: &egui::Context, buf: &mut String) -> (f32, f32, usize) {
        let mut got = (0.0, 0.0, 0usize);
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let font = egui::FontId::monospace(14.0);
                let text_size = ctx
                    .fonts_mut(|f| {
                        f.layout_no_wrap(buf.clone(), font.clone(), egui::Color32::WHITE)
                    })
                    .size();
                let width = text_size.x + 28.0;
                let edit_rect = egui::Rect::from_min_size(
                    egui::Pos2::new(40.0, 90.0),
                    egui::Vec2::new(width, text_size.y),
                );
                let mut child = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(edit_rect)
                        .layout(egui::Layout::left_to_right(egui::Align::TOP)),
                );
                let out = egui::TextEdit::singleline(buf)
                    .font(font)
                    .desired_width(width)
                    .frame(egui::Frame::NONE)
                    .margin(egui::Margin::ZERO)
                    .show(&mut child);
                got = (
                    out.response.rect.width(),
                    out.galley.size().x,
                    out.galley.rows.len(),
                );
            });
        });
        got
    }

    #[test]
    fn editor_child_rect_never_clips_or_wraps() {
        let ctx = egui::Context::default();
        let mut short = "x".to_owned();
        for _ in 0..2 {
            editor_pass(&ctx, &mut short);
        }
        let mut buf = "Government body".to_owned();
        for _ in 0..3 {
            let (rect_w, galley_w, rows) = editor_pass(&ctx, &mut buf);
            assert_eq!(rows, 1);
            assert!(
                rect_w >= galley_w + 27.0,
                "edit rect {rect_w} must fit galley {galley_w} plus cursor slack"
            );
        }
    }
}

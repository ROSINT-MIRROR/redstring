//! Right-drag link creation: rubber-band from entity to entity, then the
//! verb picker. Also the inline name editor for freshly dropped entities.

use egui::{Rect, RichText, Shape, Stroke};
use petgraph::graph::NodeIndex;

use crate::model::{Relation, RelationKind};

use super::app::{ALARM_RED, RedstringApp};
use super::board::meta;

impl RedstringApp {
    /// Right-button press on an entity starts an edge drag; release on
    /// another entity opens the verb picker; release anywhere else cancels.
    /// Everything here is scoped to the secondary button — scroll-zoom and
    /// primary/middle-drag pan stay with the board.
    pub(super) fn edge_drag_input(&mut self, ui: &egui::Ui, rect: Rect) {
        let (pressed, released, down, pointer) = ui.input(|i| {
            (
                i.pointer.secondary_pressed(),
                i.pointer.secondary_released(),
                i.pointer.secondary_down(),
                i.pointer.interact_pos(),
            )
        });

        if pressed
            && self.pending_edge.is_none()
            && self.node_drag.is_none()
            && let Some(p) = pointer.filter(|p| rect.contains(*p))
        {
            self.edge_drag = self.node_at_screen(ui, rect, p);
            if self.edge_drag.is_some() {
                self.status = "release on a target entity to link".into();
            }
        }

        let Some(from) = self.edge_drag else { return };

        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.edge_drag = None;
            self.status = "link cancelled".into();
            return;
        }

        if released {
            self.edge_drag = None;
            let to = pointer
                .filter(|p| rect.contains(*p))
                .and_then(|p| self.node_at_screen(ui, rect, p));
            let (Some(to), Some(p)) = (to, pointer) else {
                self.status = "link cancelled".into();
                return;
            };
            if to == from {
                self.status = "link cancelled".into();
                return;
            }
            self.verb_filter.clear();
            self.verb_filter_focus = true;
            self.pending_edge = Some((from, to, p));
            return;
        }
        if !down {
            self.edge_drag = None; // release never arrived — pointer left the window
        }
    }

    /// Rubber-band string from the drag source to the pointer, drawn over the
    /// GraphView in the unsourced style — that's what the new link will be.
    pub(super) fn edge_drag_overlay(&mut self, ui: &egui::Ui, rect: Rect) {
        let Some(from) = self.edge_drag else { return };
        let Some(node) = self.g.node(from) else {
            self.edge_drag = None; // entity vanished mid-drag — delete / hot-reload
            return;
        };
        let Some(pointer) = ui.input(|i| i.pointer.latest_pos()) else {
            return;
        };

        let a = meta(ui).canvas_to_screen_pos(node.location()) + rect.left_top().to_vec2();
        let stroke = Stroke::new(2.0_f32, ALARM_RED);
        let painter = ui.painter().with_clip_rect(rect);
        painter.extend(Shape::dashed_line(&[a, pointer], stroke, 9.0, 6.0));
        painter.circle_filled(pointer, 3.5, ALARM_RED);
    }

    /// Verb picker for a just-dropped edge: filter box on top, kinds below.
    /// Esc / click-away cancels.
    pub(super) fn verb_picker(&mut self, ui: &mut egui::Ui) {
        let Some((from, to, at)) = self.pending_edge else {
            return;
        };

        // endpoints can vanish while the popup is open — delete / hot-reload
        if self.g.node(from).is_none() || self.g.node(to).is_none() {
            self.pending_edge = None;
            return;
        }

        let (from_l, to_l) = (self.node_label(from), self.node_label(to));
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

                    // iterate ALL generically — newly added verbs show up
                    // here without touching this picker
                    let needle = self.verb_filter.trim().to_lowercase();
                    let verbs: Vec<RelationKind> = RelationKind::ALL
                        .into_iter()
                        .filter(|k| {
                            needle.is_empty() || k.label().to_lowercase().contains(&needle)
                        })
                        .collect();

                    // typing narrowed it to one verb? Enter picks it
                    if verbs.len() == 1
                        && filter.lost_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::Enter))
                    {
                        picked = Some(verbs[0]);
                    }

                    ui.separator();
                    egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                        for k in verbs {
                            if ui.button(k.label()).clicked() {
                                picked = Some(k);
                            }
                        }
                    });
                });
            });

        if area.response.clicked_elsewhere() {
            close = true;
        }
        close |= self.commit_verb(from, to, &from_l, &to_l, picked);
        if close {
            self.pending_edge = None;
        }
    }

    /// Commit a picked verb as a new edge; true when one was picked.
    fn commit_verb(
        &mut self,
        from: NodeIndex,
        to: NodeIndex,
        from_l: &str,
        to_l: &str,
        picked: Option<RelationKind>,
    ) -> bool {
        let Some(kind) = picked else { return false };
        self.g
            .add_edge_with_label(from, to, Relation::new(kind), kind.label().into());
        self.status = format!(
            "linked: {from_l} —{}→ {to_l} (unsourced — attach a source!)",
            kind.label()
        );
        true
    }

    /// Inline name editor for a just-dropped entity. Enter commits — label
    /// plus regenerated slug; Esc / click-away keeps the kind-label placeholder.
    pub(super) fn label_editor(&mut self, ui: &mut egui::Ui) {
        let Some((idx, at)) = self.label_edit else {
            return;
        };
        if self.g.node(idx).is_none() {
            self.label_edit = None; // vanished — delete / hot-reload
            return;
        }

        let mut commit = false;
        let mut close = ui.input(|i| i.key_pressed(egui::Key::Escape));

        let area = egui::Area::new(egui::Id::new("rs_label_edit"))
            .order(egui::Order::Foreground)
            .fixed_pos(at)
            .constrain(true)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.label(RichText::new("name the entity").small().weak());
                    let edit = ui.text_edit_singleline(&mut self.label_edit_buf);
                    if std::mem::take(&mut self.label_edit_focus) {
                        edit.request_focus();
                    }
                    if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        commit = true;
                    }
                });
            });

        if area.response.clicked_elsewhere() {
            close = true;
        }
        if commit {
            let label = self.label_edit_buf.trim().to_owned();
            if !label.is_empty() {
                self.apply_label(idx, &label);
                self.status = format!("added {label}");
            }
            close = true;
        }
        if close {
            self.label_edit = None;
        }
    }
}

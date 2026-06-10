//! Left panel: the entity-kind palette. One kind per row — big icon plus
//! name — filtered by the search bar. Rows arm the kind on click and are
//! drag sources; drop one on the board to spawn an entity there.

use egui::{RichText, Vec2};

use crate::model::EntityKind;

use super::app::RedstringApp;

impl RedstringApp {
    /// Collapsed sidebar: a thin strip whose only job is the expand toggle.
    pub(super) fn sidebar_strip(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            if ui.button("⏵").on_hover_text("expand sidebar").clicked() {
                self.prefs.sidebar_collapsed = false;
                self.persist_prefs();
            }
        });
    }

    pub(super) fn left_panel(&mut self, ui: &mut egui::Ui) {
        // search bar header — no title — with the collapse toggle on the right
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("⏴").on_hover_text("collapse sidebar").clicked() {
                    self.prefs.sidebar_collapsed = true;
                    self.persist_prefs();
                }
                ui.add_sized(
                    Vec2::new(ui.available_width(), ui.spacing().interact_size.y),
                    egui::TextEdit::singleline(&mut self.kind_search).hint_text("🔍 search"),
                );
            });
        });
        ui.separator();
        self.kind_picker(ui);
    }

    /// One kind per row, outlined in its category's anchor colour with the
    /// kind's own colour on icon and text.
    fn kind_picker(&mut self, ui: &mut egui::Ui) {
        let needle = self.kind_search.trim().to_lowercase();
        egui::ScrollArea::vertical()
            .id_salt("ne_kind_grid")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for k in EntityKind::ALL {
                    if !needle.is_empty()
                        && !k.label().to_lowercase().contains(&needle)
                        && !k.category().label().to_lowercase().contains(&needle)
                    {
                        continue;
                    }
                    let cat_color = k.category().color();
                    // icon and name as separate atoms so the icon renders 2x
                    // the text; the trailing grow left-aligns the pair. The
                    // min_size is state-invariant — selected allocates +2px
                    // vs hovered, headless-verified
                    let resp = ui.add(
                        egui::Button::selectable(
                            self.ne_kind == k,
                            (
                                RichText::new(k.icon()).color(k.color()).size(32.0f32),
                                RichText::new(k.label()).color(k.color()).size(16.0f32),
                                egui::Atom::grow(),
                            ),
                        )
                        .stroke(egui::Stroke::new(1.0f32, cat_color.gamma_multiply(0.5f32)))
                        .sense(egui::Sense::click_and_drag())
                        .min_size(Vec2::new(ui.available_width(), 44.0)),
                    );
                    if resp.clicked() {
                        self.ne_kind = k;
                    }
                    resp.dnd_set_drag_payload(k);
                }
            });
    }
}

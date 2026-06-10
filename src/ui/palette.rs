use egui::{RichText, Vec2};

use crate::error::Error;
use crate::model::{CATEGORY_META, KIND_META};

use super::app::RedstringApp;

impl RedstringApp {
    pub fn sidebar_strip(&mut self, ui: &mut egui::Ui) -> Result<(), Error> {
        ui.vertical_centered(|ui| -> Result<(), Error> {
            if ui.button("⏵").on_hover_text("expand sidebar").clicked() {
                self.prefs.sidebar_collapsed = false;
                self.prefs.save()?;
            }
            Ok(())
        })
        .inner
    }

    pub fn left_panel(&mut self, ui: &mut egui::Ui) -> Result<(), Error> {
        ui.horizontal(|ui| -> Result<(), Error> {
            ui.with_layout(
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| -> Result<(), Error> {
                    if ui.button("⏴").on_hover_text("collapse sidebar").clicked() {
                        self.prefs.sidebar_collapsed = true;
                        self.prefs.save()?;
                    }
                    ui.add_sized(
                        Vec2::new(ui.available_width(), ui.spacing().interact_size.y),
                        egui::TextEdit::singleline(&mut self.kind_search).hint_text("🔍 search"),
                    );
                    Ok(())
                },
            )
            .inner
        })
        .inner?;
        ui.separator();
        self.kind_picker(ui);
        Ok(())
    }

    fn kind_picker(&mut self, ui: &mut egui::Ui) {
        let needle = self.kind_search.trim().to_lowercase();
        egui::ScrollArea::vertical()
            .id_salt("ne_kind_grid")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for m in &KIND_META {
                    let cat = &CATEGORY_META[m.category as usize];
                    if !needle.is_empty()
                        && !m.label.to_lowercase().contains(&needle)
                        && !cat.label.to_lowercase().contains(&needle)
                    {
                        continue;
                    }
                    let resp = ui.add(
                        egui::Button::selectable(
                            self.ne_kind == m.kind,
                            (
                                RichText::new(m.icon).color(m.color).size(32.0f32),
                                RichText::new(m.label).color(m.color).size(16.0f32),
                                egui::Atom::grow(),
                            ),
                        )
                        .stroke(egui::Stroke::new(1.0f32, cat.color.gamma_multiply(0.5f32)))
                        .sense(egui::Sense::click_and_drag())
                        .min_size(Vec2::new(ui.available_width(), 44.0)),
                    );
                    if resp.clicked() {
                        self.ne_kind = m.kind;
                    }
                    resp.dnd_set_drag_payload(m.kind);
                }
            });
    }
}

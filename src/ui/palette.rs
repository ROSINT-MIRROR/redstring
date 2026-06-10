use egui::{RichText, Vec2};

use crate::error::Error;
use crate::model::{CATEGORY_META, KIND_META, KindMeta};

use super::app::RedstringApp;
use super::style;

const SEARCH_TEXT: f32 = 19.0;

impl RedstringApp {
    pub fn sidebar_strip(&mut self, ui: &mut egui::Ui) -> Result<(), Error> {
        ui.vertical_centered(|ui| -> Result<(), Error> {
            if style::square_btn(ui, egui_phosphor::regular::CARET_RIGHT, "expand palette")? {
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
                    if style::square_btn(ui, egui_phosphor::regular::CARET_LEFT, "collapse palette")? {
                        self.prefs.sidebar_collapsed = true;
                        self.prefs.save()?;
                    }
                    if style::square_btn(ui, egui_phosphor::regular::X, "clear search")? {
                        self.kind_search.clear();
                    }
                    ui.add_sized(
                        Vec2::new(ui.available_width(), style::ROW_H),
                        egui::TextEdit::singleline(&mut self.kind_search)
                            .font(egui::FontId::proportional(SEARCH_TEXT))
                            .vertical_align(egui::Align::Center)
                            .hint_text(
                                RichText::new(format!(
                                    "{} search",
                                    egui_phosphor::regular::MAGNIFYING_GLASS
                                ))
                                .size(SEARCH_TEXT),
                            ),
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
                    if !kind_matches(m, cat.label, &needle) {
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


fn kind_matches(m: &KindMeta, cat_label: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    fuzzy(needle, m.label)
        || fuzzy(needle, cat_label)
        || m.aliases.iter().any(|a| fuzzy(needle, a))
}



fn fuzzy(needle: &str, hay: &str) -> bool {
    let hay: Vec<char> = hay.to_lowercase().chars().collect();
    let mut i = 0usize;
    for n in needle.chars() {
        let mut found = false;
        while i < hay.len() {
            let h = hay[i];
            i += 1;
            if h == n {
                found = true;
                break;
            }
        }
        if !found {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::EntityKind;

    #[test]
    fn fuzzy_is_subsequence_match() {
        assert!(fuzzy("prsn", "Person"));
        assert!(fuzzy("govbod", "Government body"));
        assert!(!fuzzy("xyz", "Person"));
        assert!(fuzzy("", "anything"));
    }

    #[test]
    fn aliases_surface_kinds() {
        let weapon = &KIND_META[EntityKind::Weapon as usize];
        assert!(kind_matches(weapon, "Physical", "gun"));
        let ip = &KIND_META[EntityKind::IpAddress as usize];
        assert!(kind_matches(ip, "Digital", "ipv4"));
        assert!(!kind_matches(weapon, "Physical", "newspaper"));
    }
}

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
                                RichText::new(m.icon).color(m.color).size(24.0f32),
                                RichText::new(m.label).color(m.color).size(12.0f32),
                                egui::Atom::grow(),
                            ),
                        )
                        .stroke(egui::Stroke::new(1.0f32, cat.color.gamma_multiply(0.5f32)))
                        .sense(egui::Sense::click_and_drag())
                        .min_size(Vec2::new(ui.available_width(), 40.0)),
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

    fn phosphor() -> egui::Context {
        let ctx = egui::Context::default();
        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        ctx.set_fonts(fonts);
        ctx
    }

    // mirror kind_picker exactly: phosphor icon + category stroke + app padding +
    // min_size 40. Fresh ctx, settle animations with a STABLE pointer, read row-2.
    fn settled_row2(sel: usize, hover_row2: bool) -> (f32, egui::Pos2) {
        let ctx = phosphor();
        ctx.all_styles_mut(|s| {
            s.spacing.button_padding = Vec2::new(8.0, 5.0);
            s.spacing.item_spacing = Vec2::new(8.0, 6.0);
        });
        let icon = KIND_META[0].icon;
        let mut out = (0.0f32, egui::Pos2::ZERO);
        let mut pointer = None;
        for _ in 0..80 {
            let mut input = egui::RawInput::default();
            if let Some(p) = pointer {
                input.events.push(egui::Event::PointerMoved(p));
            }
            ctx.run(input, |ctx| {
                egui::SidePanel::left("p").exact_width(200.0).show(ctx, |ui| {
                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                        for i in 0..12usize {
                            let resp = ui.add(
                                egui::Button::selectable(
                                    sel == i,
                                    (
                                        RichText::new(icon).size(24.0f32),
                                        RichText::new("Person").size(12.0f32),
                                        egui::Atom::grow(),
                                    ),
                                )
                                .stroke(egui::Stroke::new(1.0f32, egui::Color32::GRAY))
                                .sense(egui::Sense::click_and_drag())
                                .min_size(Vec2::new(ui.available_width(), 40.0)),
                            );
                            if i == 2 {
                                out = (resp.rect.height(), resp.rect.center());
                            }
                        }
                    });
                });
            });
            pointer = hover_row2.then_some(out.1);
        }
        out
    }

    // the stroke makes the selected-idle row 2px taller than min_size 36 allowed,
    // so hovering it (which drops the stroke's extra) shrank it. min_size 40
    // dominates the tallest state, pinning all states equal. (headless-verified)
    #[test]
    fn palette_row_height_constant_across_states() {
        let unsel = settled_row2(99, false).0;
        let sel_idle = settled_row2(2, false).0;
        let (sel_hover, _) = settled_row2(2, true);
        assert_eq!(
            (unsel, sel_idle, sel_hover),
            (unsel, unsel, unsel),
            "palette row height drifts: unsel {unsel}, sel_idle {sel_idle}, sel_hover {sel_hover}"
        );
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

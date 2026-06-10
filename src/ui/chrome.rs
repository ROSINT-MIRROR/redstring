use egui::{Color32, FontId, Pos2, RichText, Sense, StrokeKind, Vec2};

use crate::error::Error;
use crate::prefs::Theme;

use super::app::RedstringApp;

const TB_ICON: f32 = 22.0;
const TB_TEXT: f32 = 13.0;
const TB_GAP: f32 = 2.0;
const TB_NAME: f32 = 16.0;
const TB_PATH: f32 = 12.0;

fn tb_button(ui: &mut egui::Ui, icon: &str, label: &str) -> egui::Response {
    let icon_galley = ui.painter().layout_no_wrap(
        icon.to_owned(),
        FontId::proportional(TB_ICON),
        Color32::PLACEHOLDER,
    );
    let text_galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        FontId::proportional(TB_TEXT),
        Color32::PLACEHOLDER,
    );
    let pad = ui.spacing().button_padding;
    let w = icon_galley.size().x.max(text_galley.size().x) + 2.0 * pad.x;
    let h = icon_galley.size().y + text_galley.size().y + 2.0 * pad.y + TB_GAP;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
    let visuals = ui.style().interact(&resp);
    ui.painter().rect(
        rect,
        visuals.corner_radius,
        visuals.weak_bg_fill,
        visuals.bg_stroke,
        StrokeKind::Inside,
    );
    let cx = rect.center().x;
    let top = rect.top() + pad.y;
    ui.painter().galley(
        Pos2::new(cx - icon_galley.size().x / 2.0, top),
        icon_galley.clone(),
        visuals.text_color(),
    );
    ui.painter().galley(
        Pos2::new(
            cx - text_galley.size().x / 2.0,
            top + icon_galley.size().y + TB_GAP,
        ),
        text_galley,
        visuals.text_color(),
    );
    resp
}

impl RedstringApp {
    pub fn toolbar(&mut self, ui: &mut egui::Ui) -> Result<(), Error> {
        ui.horizontal(|ui| -> Result<(), Error> {
            if tb_button(ui, "🏠", "Home")
                .on_hover_text("Welcome — new case, open, recents")
                .clicked()
            {
                self.show_welcome = true;
            }
            if tb_button(ui, "💾", "Save")
                .on_hover_text("Save (Ctrl+S)")
                .clicked()
            {
                self.save(ui)?;
            }
            if tb_button(ui, "💾", "Save as…")
                .on_hover_text("Save as new case directory… (Ctrl+Shift+S)")
                .clicked()
            {
                self.save_as_dialog(ui)?;
            }
            if tb_button(ui, "📂", "Open")
                .on_hover_text("Open case or bundle… (Ctrl+O)")
                .clicked()
            {
                self.open_dialog(ui)?;
            }
            if tb_button(ui, "📦", "Export")
                .on_hover_text("Export bundle — the case directory, zipped")
                .clicked()
            {
                self.export_bundle(ui)?;
            }
            ui.separator();
            ui.label(RichText::new(&self.case_name).strong().size(TB_NAME));
            ui.label(RichText::new(self.project_label()?).weak().size(TB_PATH));

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if tb_button(ui, "⛭", "Prefs")
                    .on_hover_text("Preferences")
                    .clicked()
                {
                    self.prefs_open = !self.prefs_open;
                }
                ui.separator();
                for name in ["Grid", "Circle", "Tree", "Force"] {
                    ui.add_enabled(false, egui::Button::new(name))
                        .on_disabled_hover_text("layout — coming soon");
                }
                ui.label(RichText::new("layout:").weak().small());
            });
            Ok(())
        })
        .inner
    }

    pub fn prefs_window(&mut self, ui: &egui::Ui) -> Result<(), Error> {
        let mut open = self.prefs_open;
        let mut changed = false;
        egui::Window::new("preferences")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ui.ctx(), |ui| {
                ui.horizontal(|ui| {
                    ui.label("UI scale");
                    let slider = ui.add(
                        egui::Slider::new(&mut self.prefs.ui_scale, 0.75..=3.0)
                            .step_by(0.05)
                            .suffix("×"),
                    );
                    if slider.drag_stopped() || slider.lost_focus() {
                        changed = true;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("theme");
                    changed |= ui
                        .selectable_value(&mut self.prefs.theme, Theme::Dark, "dark")
                        .clicked();
                    changed |= ui
                        .selectable_value(&mut self.prefs.theme, Theme::Light, "light")
                        .clicked();
                });
            });
        self.prefs_open = open;
        if changed {
            self.apply_prefs(ui.ctx())?;
            self.prefs.save()?;
        }
        Ok(())
    }
}

use egui::RichText;

use crate::error::Error;
use crate::prefs::Theme;

use super::app::RedstringApp;

impl RedstringApp {
    pub fn toolbar(&mut self, ui: &mut egui::Ui) -> Result<(), Error> {
        ui.horizontal(|ui| -> Result<(), Error> {
            if ui
                .button("🏠")
                .on_hover_text("Welcome — new case, open, recents")
                .clicked()
            {
                self.show_welcome = true;
            }
            if ui.button("💾").on_hover_text("Save (Ctrl+S)").clicked() {
                self.save(ui)?;
            }
            if ui
                .button("💾…")
                .on_hover_text("Save as new case directory… (Ctrl+Shift+S)")
                .clicked()
            {
                self.save_as_dialog(ui)?;
            }
            if ui
                .button("📂")
                .on_hover_text("Open case or bundle… (Ctrl+O)")
                .clicked()
            {
                self.open_dialog(ui)?;
            }
            if ui
                .button("📦")
                .on_hover_text("Export bundle — the case directory, zipped")
                .clicked()
            {
                self.export_bundle(ui)?;
            }
            ui.separator();
            ui.label(RichText::new(&self.case_name).strong());
            ui.label(RichText::new(self.project_label()?).weak().small());

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("⛭").on_hover_text("Preferences").clicked() {
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

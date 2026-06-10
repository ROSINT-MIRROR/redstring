use egui::{Color32, FontId, Pos2, RichText, Sense, StrokeKind, Vec2};

use crate::error::Error;
use crate::prefs::Theme;

use super::app::RedstringApp;
use super::layout::LayoutKind;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    General,
    ApiKeys,
}

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

            ui.with_layout(
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| -> Result<(), Error> {
                    if tb_button(ui, egui_phosphor::regular::SIGN_OUT, "Quit")
                        .on_hover_text("Quit — asks to save if there are unsaved changes")
                        .clicked()
                    {
                        self.request_quit(ui)?;
                    }
                    if tb_button(ui, egui_phosphor::regular::GEAR, "Settings")
                        .on_hover_text("Settings")
                        .clicked()
                    {
                        self.prefs_open = !self.prefs_open;
                    }
                    ui.separator();
                    let layouts = [
                        ("Force", "organic — force-directed", LayoutKind::Force),
                        ("Tree", "hierarchy — roots on top", LayoutKind::Tree),
                        ("Circle", "everything on a ring", LayoutKind::Circle),
                        ("Grid", "block grid, grouped by kind", LayoutKind::Grid),
                    ];
                    for (name, hover, kind) in layouts {
                        if ui
                            .button(name)
                            .on_hover_text(format!("{hover} — selection if any, else all"))
                            .clicked()
                        {
                            self.apply_layout(ui, kind)?;
                        }
                    }
                    ui.label(RichText::new("layout:").weak().small());
                    Ok(())
                },
            )
            .inner?;
            Ok(())
        })
        .inner
    }

    pub fn settings_window(&mut self, ui: &egui::Ui) -> Result<(), Error> {
        let mut open = self.prefs_open;
        let mut changed = false;
        egui::Window::new("settings")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ui.ctx().content_rect().center())
            .min_size(Vec2::new(440.0, 220.0))
            .show(ui.ctx(), |ui| {
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_min_width(110.0);
                        ui.selectable_value(&mut self.settings_tab, SettingsTab::General, "General");
                        ui.selectable_value(&mut self.settings_tab, SettingsTab::ApiKeys, "API keys");
                    });
                    ui.separator();
                    ui.vertical(|ui| {
                        ui.set_min_width(300.0);
                        match self.settings_tab {
                            SettingsTab::General => changed |= general_tab(ui, &mut self.prefs),
                            SettingsTab::ApiKeys => changed |= api_keys_tab(ui, &mut self.prefs),
                        }
                    });
                });
            });
        self.prefs_open = open;
        if changed {
            self.apply_prefs(ui.ctx())?;
            self.prefs.save()?;
        }
        Ok(())
    }

    pub fn request_quit(&mut self, ui: &egui::Ui) -> Result<(), Error> {
        if self.dirty()? {
            self.quit_confirm = true;
            return Ok(());
        }
        self.allow_close = true;
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        Ok(())
    }

    pub fn guard_close(&mut self, ui: &egui::Ui) -> Result<(), Error> {
        if self.allow_close || !ui.input(|i| i.viewport().close_requested()) {
            return Ok(());
        }
        if !self.dirty()? {
            return Ok(());
        }
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::CancelClose);
        self.quit_confirm = true;
        Ok(())
    }

    pub fn quit_window(&mut self, ui: &egui::Ui) -> Result<(), Error> {
        if !self.quit_confirm {
            return Ok(());
        }
        let mut save_quit = false;
        let mut discard = false;
        let mut cancel = false;
        egui::Window::new("unsaved changes")
            .collapsible(false)
            .resizable(false)
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ui.ctx().content_rect().center())
            .show(ui.ctx(), |ui| {
                ui.label("the case has unsaved changes");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    save_quit = ui.button(RichText::new("💾 save & quit").strong()).clicked();
                    discard = ui.button("quit without saving").clicked();
                    cancel = ui.button("cancel").clicked();
                });
            });
        if save_quit {
            self.save(ui)?;
            if !self.dirty()? {
                self.quit_confirm = false;
                self.allow_close = true;
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        if discard {
            self.quit_confirm = false;
            self.allow_close = true;
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if cancel {
            self.quit_confirm = false;
        }
        Ok(())
    }
}

fn general_tab(ui: &mut egui::Ui, prefs: &mut crate::prefs::Prefs) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label("UI scale");
        let slider = ui.add(
            egui::Slider::new(&mut prefs.ui_scale, 0.75..=3.0)
                .step_by(0.05)
                .suffix("×"),
        );
        if slider.drag_stopped() || slider.lost_focus() {
            changed = true;
        }
    });
    ui.horizontal(|ui| {
        ui.label("theme");
        changed |= ui.selectable_value(&mut prefs.theme, Theme::Dark, "dark").clicked();
        changed |= ui.selectable_value(&mut prefs.theme, Theme::Light, "light").clicked();
    });
    changed
}

fn api_keys_tab(ui: &mut egui::Ui, prefs: &mut crate::prefs::Prefs) -> bool {
    let mut changed = false;
    ui.label(RichText::new("keys for lookup services — stored in prefs.ron").weak());
    ui.add_space(4.0);
    let mut remove: Option<usize> = None;
    for (i, (name, key)) in prefs.api_keys.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            changed |= ui
                .add(egui::TextEdit::singleline(name).desired_width(110.0).hint_text("service"))
                .lost_focus();
            changed |= ui
                .add(
                    egui::TextEdit::singleline(key)
                        .desired_width(180.0)
                        .password(true)
                        .hint_text("key"),
                )
                .lost_focus();
            if ui
                .button(egui_phosphor::regular::TRASH)
                .on_hover_text("remove key")
                .clicked()
            {
                remove = Some(i);
            }
        });
    }
    changed |= remove.is_some();
    drop_key(&mut prefs.api_keys, remove);
    if ui.button("+ key").clicked() {
        prefs.api_keys.push((String::new(), String::new()));
    }
    changed
}

fn drop_key(keys: &mut Vec<(String, String)>, marked: Option<usize>) {
    let Some(i) = marked else { return };
    keys.remove(i);
}

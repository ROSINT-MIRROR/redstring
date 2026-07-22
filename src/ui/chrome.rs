use egui::{Color32, FontId, Pos2, RichText, Sense, StrokeKind, Vec2};

use crate::error::Error;
use crate::prefs::Theme;

use super::app::RedstringApp;
use super::layout::{LayoutKind, Spread};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    General,
    ApiKeys,
    Modules,
}

const TB_ICON: f32 = 18.0;
const TB_TEXT: f32 = 12.0;
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

fn tb_toggle(ui: &mut egui::Ui, icon: &str, label: &str, on: bool) -> egui::Response {
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
    let visuals = ui.style().interact_selectable(&resp, on);
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
            ui.spacing_mut().item_spacing.x = 6.0;
            if tb_button(ui, egui_phosphor::regular::HOUSE, "Home")
                .on_hover_text("Welcome — new case, open, recents")
                .clicked()
            {
                self.show_welcome = true;
            }
            if tb_button(ui, egui_phosphor::regular::FLOPPY_DISK, "Save")
                .on_hover_text("Save (Ctrl+S)")
                .clicked()
            {
                self.save(ui)?;
            }
            if tb_button(ui, egui_phosphor::regular::FLOPPY_DISK_BACK, "Save as…")
                .on_hover_text("Save as new case directory… (Ctrl+Shift+S)")
                .clicked()
            {
                self.save_as_dialog(ui)?;
            }
            if tb_button(ui, egui_phosphor::regular::FOLDER_OPEN, "Open")
                .on_hover_text("Open case or bundle… (Ctrl+O)")
                .clicked()
            {
                self.open_dialog(ui)?;
            }
            if tb_button(ui, egui_phosphor::regular::PACKAGE, "Export")
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
                    if crate::log::ENABLED.load(std::sync::atomic::Ordering::Relaxed)
                        && tb_button(ui, egui_phosphor::regular::BUG, "Log")
                            .on_hover_text("toggle the debug log window (--debug)")
                            .clicked()
                    {
                        let open = crate::log::is_open()?;
                        crate::log::set_open(!open)?;
                    }
                    ui.separator();
                    let layouts = [
                        (egui_phosphor::regular::SQUARES_FOUR, "Grid", "block grid, grouped by kind", LayoutKind::Grid),
                        (egui_phosphor::regular::CIRCLE_DASHED, "Circle", "everything on a ring", LayoutKind::Circle),
                        (egui_phosphor::regular::TREE_STRUCTURE, "Tree", "hierarchy — roots on top", LayoutKind::Tree),
                        (egui_phosphor::regular::GRAPH, "Force", "organic — force-directed", LayoutKind::Force),
                    ];
                    for (icon, name, hover, kind) in layouts {
                        if tb_button(ui, icon, name)
                            .on_hover_text(format!("{hover} — selection if any, else all"))
                            .clicked()
                        {
                            self.choose_layout(ui, kind)?;
                        }
                    }
                    if tb_toggle(
                        ui,
                        egui_phosphor::regular::MAGIC_WAND,
                        "Relayout",
                        self.relayout_on_add,
                    )
                    .on_hover_text(
                        "relayout on add — re-run the selected layout whenever a lookup adds nodes",
                    )
                    .clicked()
                    {
                        self.relayout_on_add = !self.relayout_on_add;
                    }
                    if tb_button(ui, egui_phosphor::regular::ARROWS_IN, "Tighten")
                        .on_hover_text("pull nodes closer — proportional to current spacing; selection if any, else all")
                        .clicked()
                    {
                        self.apply_spread(ui, Spread::Tighten)?;
                    }
                    if tb_button(ui, egui_phosphor::regular::ARROWS_OUT, "Loosen")
                        .on_hover_text("spread nodes apart — proportional to current spacing; selection if any, else all")
                        .clicked()
                    {
                        self.apply_spread(ui, Spread::Loosen)?;
                    }
                    ui.separator();
                    if tb_button(ui, egui_phosphor::regular::FRAME_CORNERS, "Center")
                        .on_hover_text("frame the biggest cluster of visible nodes in view")
                        .clicked()
                    {
                        self.center_graph()?;
                    }
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
        let api_key_ids = self.registry.key_ids()?;
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
                        ui.set_min_width(120.0);
                        ui.selectable_value(
                            &mut self.settings_tab,
                            SettingsTab::General,
                            format!("{} General", egui_phosphor::regular::SLIDERS_HORIZONTAL),
                        );
                        ui.selectable_value(
                            &mut self.settings_tab,
                            SettingsTab::ApiKeys,
                            format!("{} API keys", egui_phosphor::regular::KEY),
                        );
                        ui.selectable_value(
                            &mut self.settings_tab,
                            SettingsTab::Modules,
                            format!("{} Modules", egui_phosphor::regular::PLUGS),
                        );
                    });
                    ui.separator();
                    ui.vertical(|ui| {
                        ui.set_min_width(320.0);
                        match self.settings_tab {
                            SettingsTab::General => changed |= general_tab(ui, &mut self.prefs),
                            SettingsTab::ApiKeys => {
                                changed |= api_keys_tab(ui, &mut self.prefs, &api_key_ids)
                            }
                            SettingsTab::Modules => modules_tab(ui, &self.registry),
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
            crate::dlog_info!("quit", "quit requested — dirty, showing confirm");
            self.quit_confirm = true;
            return Ok(());
        }
        crate::dlog_info!("quit", "quit requested — clean, closing");
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
        crate::dlog_warn!("quit", "window close intercepted — unsaved changes, cancelling");
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::CancelClose);
        self.quit_confirm = true;
        Ok(())
    }

    pub fn delete_confirm_window(&mut self, ui: &egui::Ui) -> Result<(), Error> {
        let Some(list) = self.delete_confirm.clone() else {
            return Ok(());
        };
        let n = list.len();
        let mut yes = false;
        let mut cancel = ui.input(|i| i.key_pressed(egui::Key::Escape));
        egui::Window::new("remove entities")
            .collapsible(false)
            .resizable(false)
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ui.ctx().content_rect().center())
            .show(ui.ctx(), |ui| {
                let noun = if n == 1 { "entity" } else { "entities" };
                ui.label(format!("Remove {n} {noun} and their relations?"));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let yes_btn = ui.button(
                        RichText::new(format!("{} yes, remove", egui_phosphor::regular::TRASH))
                            .color(super::style::DANGER)
                            .strong(),
                    );
                    if ui.memory(|m| m.focused().is_none()) {
                        yes_btn.request_focus();
                    }
                    yes = yes_btn.clicked();
                    cancel |= ui.button("cancel").clicked();
                });
            });
        if yes {
            let mut removed = 0usize;
            for idx in &list {
                if self.g.remove_node(*idx).is_some() {
                    removed += 1;
                }
            }
            crate::dlog_info!("ops", "delete removed={removed} requested={}", list.len());
            self.delete_confirm = None;
            self.status = format!("removed {removed} entities and their relations");
        }
        if cancel {
            self.delete_confirm = None;
        }
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
                    let sq = format!("{} save & quit", egui_phosphor::regular::FLOPPY_DISK);
                    save_quit = ui.button(RichText::new(sq).strong()).clicked();
                    discard = ui.button("quit without saving").clicked();
                    cancel = ui.button("cancel").clicked();
                });
            });
        if save_quit {
            crate::dlog_info!("quit", "save & quit chosen");
            self.save(ui)?;
            if !self.dirty()? {
                self.quit_confirm = false;
                self.allow_close = true;
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        if discard {
            crate::dlog_warn!("quit", "quit without saving chosen");
            self.quit_confirm = false;
            self.allow_close = true;
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if cancel {
            crate::dlog_info!("quit", "quit cancelled");
            self.quit_confirm = false;
        }
        Ok(())
    }
}

fn general_tab(ui: &mut egui::Ui, prefs: &mut crate::prefs::Prefs) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(format!("{} UI scale", egui_phosphor::regular::MAGNIFYING_GLASS));
        let slider = ui.add(
            egui::Slider::new(&mut prefs.ui_scale, 0.75..=3.0)
                .step_by(0.05)
                .suffix("×"),
        );
        if slider.drag_stopped() || slider.lost_focus() {
            changed = true;
        }
    });
    ui.add_space(super::style::GAP_SM);
    ui.horizontal(|ui| {
        ui.label(format!("{} theme", egui_phosphor::regular::PALETTE));
        changed |= ui.selectable_value(&mut prefs.theme, Theme::Dark, "dark").clicked();
        changed |= ui.selectable_value(&mut prefs.theme, Theme::Light, "light").clicked();
    });
    changed
}



fn modules_tab(ui: &mut egui::Ui, registry: &crate::modules::Registry) {
    ui.label(
        RichText::new("lookup modules — run from an entity's inspector; specs load from ~/.config/redstring/modules/")
            .weak(),
    );
    ui.add_space(super::style::GAP_SM);
    for entry in &registry.entries {
        ui.horizontal(|ui| {
            match &entry.status {
                crate::modules::ModuleStatus::Active(path) => {
                    ui.colored_label(super::app::OK_GREEN, egui_phosphor::regular::CHECK);
                    ui.label(RichText::new(&entry.spec.label).strong());
                    ui.label(RichText::new(path.display().to_string()).weak().small());
                }
                crate::modules::ModuleStatus::Missing(missing) => {
                    ui.colored_label(super::style::DANGER, egui_phosphor::regular::X);
                    ui.label(RichText::new(&entry.spec.label).strong());
                    ui.label(
                        RichText::new(format!("{} not on PATH — hidden", missing.join(", ")))
                            .weak()
                            .small(),
                    );
                }
                crate::modules::ModuleStatus::Broken(reason) => {
                    ui.colored_label(super::style::DANGER, egui_phosphor::regular::X);
                    ui.label(RichText::new(&entry.spec.label).strong());
                    ui.label(RichText::new(format!("broken spec: {reason}")).weak().small());
                }
            }
        });
        ui.label(RichText::new(format!("    {}", entry.origin)).weak().small());
    }
}


const API_SERVICES: [(&str, &str, &str); 1] = [(
    "web_archive",
    "Web Archive",
    egui_phosphor::regular::GLOBE_SIMPLE,
)];

fn api_keys_tab(
    ui: &mut egui::Ui,
    prefs: &mut crate::prefs::Prefs,
    module_key_ids: &[String],
) -> bool {
    let mut changed = false;
    ui.label(RichText::new("keys for lookup services — stored locally, never in the case").weak());
    ui.add_space(super::style::GAP_SM);
    for id in service_ids(module_key_ids) {
        let (label, icon) = service_meta(&id);
        let key = ensure_key(&mut prefs.api_keys, &id);
        ui.horizontal(|ui| {
            ui.label(format!("{icon} {label}"));
            changed |= ui
                .add(
                    egui::TextEdit::singleline(key)
                        .desired_width(200.0)
                        .password(true)
                        .hint_text("paste key"),
                )
                .lost_focus();
        });
    }
    changed
}

fn service_ids(module_key_ids: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for (id, _, _) in API_SERVICES {
        out.push(id.to_owned());
    }
    for id in module_key_ids {
        if !out.contains(id) {
            out.push(id.clone());
        }
    }
    out
}

fn service_meta(id: &str) -> (&str, &'static str) {
    for (sid, label, icon) in API_SERVICES {
        if sid == id {
            return (label, icon);
        }
    }
    (id, egui_phosphor::regular::KEY)
}


fn ensure_key<'a>(keys: &'a mut Vec<(String, String)>, id: &str) -> &'a mut String {
    let pos = keys.iter().position(|(k, _)| k == id);
    let Some(p) = pos else {
        keys.push((id.to_owned(), String::new()));
        let last = keys.len() - 1;
        return &mut keys[last].1;
    };
    &mut keys[p].1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_archive_is_always_a_settings_key_slot() {
        assert_eq!(service_ids(&[]), vec!["web_archive".to_owned()]);
    }

    #[test]
    fn a_declared_module_key_gets_its_own_slot_after_web_archive() {
        let ids = service_ids(&["dehashed".to_owned()]);
        assert_eq!(ids, vec!["web_archive".to_owned(), "dehashed".to_owned()]);
    }

    #[test]
    fn union_dedups_repeats_and_the_builtin_web_archive_key() {
        let ids = service_ids(&[
            "dehashed".to_owned(),
            "dehashed".to_owned(),
            "web_archive".to_owned(),
            "shodan".to_owned(),
        ]);
        assert_eq!(
            ids,
            vec![
                "web_archive".to_owned(),
                "dehashed".to_owned(),
                "shodan".to_owned(),
            ]
        );
    }

    #[test]
    fn known_service_keeps_its_label_derived_service_falls_back_to_id() {
        let (wa_label, _) = service_meta("web_archive");
        assert_eq!(wa_label, "Web Archive");
        let (dh_label, dh_icon) = service_meta("dehashed");
        assert_eq!(dh_label, "dehashed");
        assert_eq!(dh_icon, egui_phosphor::regular::KEY);
    }

    #[test]
    fn ensure_key_persists_a_value_under_the_service_id() {
        let mut keys: Vec<(String, String)> = Vec::new();
        {
            let slot = ensure_key(&mut keys, "dehashed");
            slot.push_str("secret");
        }
        let stored = ensure_key(&mut keys, "dehashed");
        assert_eq!(stored, "secret");
        assert_eq!(keys.len(), 1);
    }
}

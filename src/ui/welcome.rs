//! The welcome window — centered over an empty board at startup. New case,
//! open by file or by folder or as a bundle, recent cases, the demo seed.

use std::path::Path;

use egui::{Align2, RichText};

use super::app::RedstringApp;

impl RedstringApp {
    /// Centered welcome window, shown while `show_welcome` is up. Any
    /// successful open or create drops the flag; the close button merely
    /// reveals the empty board underneath.
    pub(super) fn welcome_window(&mut self, ui: &mut egui::Ui) {
        if !self.show_welcome {
            return;
        }
        let mut open = true;
        egui::Window::new("welcome")
            .anchor(Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .collapsible(false)
            .resizable(false)
            .default_width(440.0)
            .open(&mut open)
            .show(ui.ctx(), |w| self.welcome_body(w));
        // an action inside the body may have dropped the flag already — keep
        // the stricter of the two verdicts
        self.show_welcome = self.show_welcome && open;
    }

    fn welcome_body(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(2.0);
            ui.heading("redstring");
            ui.label(RichText::new("local-first graph investigation").weak());
            ui.add_space(8.0);
        });
        ui.horizontal(|ui| {
            if ui
                .button("new case…")
                .on_hover_text("Pick a spot — redstring scaffolds the canonical case directory")
                .clicked()
            {
                self.save_as_dialog(ui);
            }
            if ui
                .button("open…")
                .on_hover_text("A case.redstring or a .zip bundle")
                .clicked()
            {
                self.open_dialog(ui);
            }
            if ui
                .button("open folder…")
                .on_hover_text("A case directory")
                .clicked()
            {
                self.open_folder_dialog(ui);
            }
        });
        ui.add_space(4.0);
        ui.label(
            RichText::new("dropping a case directory, case file or bundle here works too")
                .weak()
                .small(),
        );

        ui.add_space(8.0);
        ui.separator();
        ui.label(RichText::new("recent").weak().small());
        if self.prefs.recent.is_empty() {
            ui.weak("nothing yet — cases you open or create land here");
        }
        let mut open_req: Option<String> = None;
        let mut forget_req: Option<String> = None;
        for path in &self.prefs.recent {
            recent_row(ui, path, &mut open_req, &mut forget_req);
        }
        self.forget_recent_request(forget_req);
        self.open_recent_request(ui, open_req);

        ui.add_space(8.0);
        ui.separator();
        ui.horizontal(|ui| {
            ui.weak("just looking?");
            if ui.small_button("load the demo board").clicked() {
                self.seed_demo();
                self.show_welcome = false;
            }
        });
    }

    fn forget_recent_request(&mut self, req: Option<String>) {
        let Some(path) = req else { return };
        self.prefs.forget_recent(&path);
        self.persist_prefs();
    }

    fn open_recent_request(&mut self, ui: &mut egui::Ui, req: Option<String>) {
        let Some(path) = req else { return };
        self.open_path(ui, Path::new(&path));
    }
}

/// One recents line: forget button, clickable name, weak full path. An entry
/// whose directory vanished renders inert and struck through.
fn recent_row(
    ui: &mut egui::Ui,
    path: &str,
    open_req: &mut Option<String>,
    forget_req: &mut Option<String>,
) {
    ui.horizontal(|ui| {
        if ui.small_button("✕").on_hover_text("forget this entry").clicked() {
            *forget_req = Some(path.to_owned());
        }
        if !Path::new(path).is_dir() {
            ui.label(
                RichText::new(format!("{} — gone", dir_label(path)))
                    .weak()
                    .strikethrough(),
            );
            return;
        }
        if ui.link(RichText::new(dir_label(path)).strong()).clicked() {
            *open_req = Some(path.to_owned());
        }
        ui.label(RichText::new(tilde(path)).weak().small());
    });
}

/// The directory name is the human label for a case path.
fn dir_label(path: &str) -> String {
    let Some(name) = Path::new(path).file_name() else {
        return path.to_owned();
    };
    name.to_string_lossy().into_owned()
}

/// Home-relative rendering of a path, purely cosmetic.
fn tilde(path: &str) -> String {
    let Some(home) = std::env::var_os("HOME") else {
        return path.to_owned();
    };
    let home = home.to_string_lossy().into_owned();
    let Some(rest) = path.strip_prefix(&home) else {
        return path.to_owned();
    };
    format!("~{rest}")
}

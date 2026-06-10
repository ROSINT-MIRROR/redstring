use std::path::Path;

use egui::{Align2, RichText};

use crate::error::Error;
use crate::model::MAX_CASE_NAME;

use super::app::RedstringApp;

impl RedstringApp {
    pub fn welcome_window(&mut self, ui: &mut egui::Ui) -> Result<(), Error> {
        if !self.show_welcome {
            return Ok(());
        }
        let mut open = true;
        let shown = egui::Window::new("welcome")
            .anchor(Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .collapsible(false)
            .resizable(false)
            .default_width(440.0)
            .open(&mut open)
            .show(ui.ctx(), |w| self.welcome_body(w));
        self.show_welcome = self.show_welcome && open;
        let Some(inner) = shown.and_then(|ir| ir.inner) else {
            return Ok(());
        };
        inner
    }

    fn welcome_body(&mut self, ui: &mut egui::Ui) -> Result<(), Error> {
        ui.vertical_centered(|ui| {
            ui.add_space(2.0);
            ui.heading("redstring");
            ui.label(RichText::new("local-first graph investigation").weak());
            ui.add_space(8.0);
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("case name").weak());
            ui.add(
                egui::TextEdit::singleline(&mut self.welcome_name)
                    .hint_text("free text, spaces fine — the directory name comes next")
                    .char_limit(MAX_CASE_NAME)
                    .desired_width(f32::INFINITY),
            );
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| -> Result<(), Error> {
            if ui
                .button("new case…")
                .on_hover_text("Pick where the case directory goes — it scaffolds the canonical tree")
                .clicked()
            {
                self.apply_welcome_name();
                self.save_as_dialog(ui)?;
            }
            if ui
                .button("open…")
                .on_hover_text("A case.redstring or a .zip bundle")
                .clicked()
            {
                self.open_dialog(ui)?;
            }
            if ui
                .button("open folder…")
                .on_hover_text("A case directory")
                .clicked()
            {
                self.open_folder_dialog(ui)?;
            }
            Ok(())
        })
        .inner?;
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
        self.forget_recent_request(forget_req)?;
        self.open_recent_request(ui, open_req)?;

        ui.add_space(8.0);
        ui.separator();
        ui.horizontal(|ui| -> Result<(), Error> {
            ui.weak("just looking?");
            if ui.small_button("load the demo board").clicked() {
                self.seed_demo()?;
                self.show_welcome = false;
            }
            Ok(())
        })
        .inner
    }

    fn apply_welcome_name(&mut self) {
        let name = self.welcome_name.trim();
        if name.is_empty() {
            return;
        }
        self.case_name = name.to_owned();
    }

    fn forget_recent_request(&mut self, req: Option<String>) -> Result<(), Error> {
        let Some(path) = req else {
            return Ok(());
        };
        self.prefs.forget_recent(&path)
    }

    fn open_recent_request(&mut self, ui: &mut egui::Ui, req: Option<String>) -> Result<(), Error> {
        let Some(path) = req else {
            return Ok(());
        };
        self.open_path(ui, Path::new(&path))
    }
}

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

fn dir_label(path: &str) -> String {
    let Some(name) = Path::new(path).file_name() else {
        return path.to_owned();
    };
    name.to_string_lossy().into_owned()
}

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

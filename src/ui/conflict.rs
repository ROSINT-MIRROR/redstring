use egui::RichText;
use egui_phosphor::regular::{ARROW_COUNTER_CLOCKWISE, CLOUD_ARROW_DOWN, FLOPPY_DISK, WARNING};

use crate::dlog_warn;
use crate::error::Error;
use crate::project::{Merged, Resolve, Side, describe};

use super::app::RedstringApp;
use super::reconcile::{Push, Synced};
use super::style::DANGER;

#[derive(Clone, PartialEq, Debug, Default)]
pub struct ConflictView {
    pub items: Vec<String>,
    pub open: bool,
}

impl RedstringApp {
    pub fn raise_conflict(&mut self, merged: &Merged) -> Result<(), Error> {
        let mut items = Vec::new();
        for clash in &merged.clashes {
            items.push(describe(clash)?);
        }
        let n = items.len();
        dlog_warn!("watch", "{n} conflicting change(s) between memory and disk — writes held");
        self.autosave.conflict = Some(ConflictView { items, open: true });
        self.status = format!(
            "⚠ {n} change(s) collide with the case on disk — nothing written; pick a side (Ctrl+S reopens)"
        );
        Ok(())
    }

    pub fn reopen_conflict(&mut self) -> Result<bool, Error> {
        let Some(view) = self.autosave.conflict.as_mut() else {
            return Ok(false);
        };
        view.open = true;
        Ok(true)
    }

    pub fn resolve_conflict(&mut self, ui: &egui::Ui, side: Side) -> Result<(), Error> {
        if self.sync_disk(ui, Push::Explicit, Resolve::Keep(side))? != Synced::Settled {
            return Ok(());
        }
        let kept = match side {
            Side::Local => "kept your version",
            Side::Disk => "took the version on disk",
        };
        if !self.live()? {
            self.settle_sandbox(ui)?;
        }
        self.status = format!("conflict resolved — {kept} for the colliding changes, merged the rest");
        Ok(())
    }

    pub fn conflict_window(&mut self, ui: &mut egui::Ui) -> Result<(), Error> {
        let Some(view) = self.autosave.conflict.clone() else {
            return Ok(());
        };
        if !view.open {
            return Ok(());
        }
        let exploring = !self.live()?;
        let mut choice: Option<Side> = None;
        let mut discard = false;
        let mut later = ui.input(|i| i.key_pressed(egui::Key::Escape));
        egui::Window::new(format!("{WARNING} the case changed on disk"))
            .collapsible(false)
            .resizable(false)
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ui.ctx().content_rect().center())
            .show(ui.ctx(), |ui| {
                ui.label(
                    "these changes were made both here and on disk. nothing has been written. \
                     everything else merges either way — pick which side wins for these:",
                );
                ui.add_space(6.0);
                egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                    for item in &view.items {
                        ui.label(RichText::new(format!("• {item}")).small());
                    }
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui
                        .button(RichText::new(format!("{FLOPPY_DISK} keep mine")).color(DANGER).strong())
                        .clicked()
                    {
                        choice = Some(Side::Local);
                    }
                    if ui.button(format!("{CLOUD_ARROW_DOWN} take theirs")).clicked() {
                        choice = Some(Side::Disk);
                    }
                    if exploring
                        && ui
                            .button(format!("{ARROW_COUNTER_CLOCKWISE} discard my sandbox"))
                            .clicked()
                    {
                        discard = true;
                    }
                    later |= ui.button("decide later").clicked();
                });
            });
        if discard {
            return self.explore_discard(ui);
        }
        for side in choice.into_iter() {
            return self.resolve_conflict(ui, side);
        }
        if later {
            for v in self.autosave.conflict.iter_mut() {
                v.open = false;
            }
        }
        Ok(())
    }
}

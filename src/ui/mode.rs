use std::time::Duration;

use egui::{Color32, StrokeKind};

use crate::error::Error;
use crate::model::{Camera, now_unix};
use crate::project::Resolve;

use super::app::RedstringApp;
use super::board::camera_read;
use super::conflict::ConflictView;
use super::fingerprint::is_dirty;
use super::reconcile::{Push, Synced};

pub const DEBOUNCE_SECS: f64 = 1.5;
pub const IDLE_SECS: f64 = 10.0;

pub const EXPLORE_INK: Color32 = Color32::from_rgb(226, 150, 60);

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Mode {
    #[default]
    Live,
    Explore,
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct Autosave {
    pub last_fp: String,
    pub pending_since: Option<f64>,
    pub derive_since: Option<f64>,
    pub camera_since: Option<f64>,
    pub last_camera: Camera,
    pub disk_moved: bool,
    pub conflict: Option<ConflictView>,
}

impl RedstringApp {
    pub fn live(&self) -> Result<bool, Error> {
        Ok(self.mode == Mode::Live && self.project.is_some())
    }

    pub fn autosave_tick(&mut self, ui: &egui::Ui) -> Result<(), Error> {
        let now = ui.input(|i| i.time);
        let fp = self.fingerprint()?;
        let moved = fp != self.autosave.last_fp;
        let dirty = is_dirty(self.project.is_some(), &fp, &self.saved_fp)?;
        self.autosave.last_fp = fp;
        if !dirty {
            self.autosave.pending_since = None;
        } else if moved || self.autosave.pending_since.is_none() {
            self.autosave.pending_since = Some(now);
        }
        if !self.live()? {
            return Ok(());
        }
        self.debounced_flush(ui, now)?;
        self.idle_beat(ui, now)
    }

    fn debounced_flush(&mut self, ui: &egui::Ui, now: f64) -> Result<(), Error> {
        let Some(since) = self.autosave.pending_since else {
            return Ok(());
        };
        let waited = now - since;
        if waited < DEBOUNCE_SECS {
            ui.ctx()
                .request_repaint_after(Duration::from_secs_f64((DEBOUNCE_SECS - waited).max(0.0)));
            return Ok(());
        }
        if !self.autosave_push(ui)? {
            return Ok(());
        }
        self.autosave.pending_since = None;
        self.autosave.derive_since = Some(now);
        self.autosave.camera_since = None;
        ui.ctx()
            .request_repaint_after(Duration::from_secs_f64(IDLE_SECS));
        Ok(())
    }

    fn autosave_push(&mut self, ui: &egui::Ui) -> Result<bool, Error> {
        if self.autosave.conflict.is_some() {
            return Ok(false);
        }
        if !self.disk_in_sync()? {
            ui.ctx().request_repaint_after(Duration::from_millis(500));
            return Ok(false);
        }
        Ok(self.sync_disk(ui, Push::Autosave, Resolve::Ask)? == Synced::Settled)
    }

    fn idle_beat(&mut self, ui: &egui::Ui, now: f64) -> Result<(), Error> {
        if self.autosave.pending_since.is_some() {
            return Ok(());
        }
        let Some(project) = self.project.clone() else {
            return Ok(());
        };
        let cam = camera_read(ui)?;
        if cam != self.autosave.last_camera && self.autosave.camera_since.is_none() {
            self.autosave.camera_since = Some(now);
        }
        let cam_due = past(self.autosave.camera_since, now, IDLE_SECS);
        let derive_due = past(self.autosave.derive_since, now, IDLE_SECS);
        if !cam_due && !derive_due {
            self.beat_repaint(ui, now);
            return Ok(());
        }
        if cam_due && self.autosave_push(ui)? {
            self.autosave.camera_since = None;
        }
        if derive_due && self.autosave.conflict.is_none() && self.disk_in_sync()? {
            let cf = self.to_case_file(ui)?;
            self.self_write(&project.root, |app| app.write_derived(&project, &cf))?;
            crate::dlog_info!("autosave", "idle beat regenerated the derived artifacts");
            self.autosave.derive_since = None;
        }
        Ok(())
    }

    fn beat_repaint(&self, ui: &egui::Ui, now: f64) {
        let mut wait = f64::MAX;
        for since in [self.autosave.camera_since, self.autosave.derive_since] {
            let Some(t) = since else {
                continue;
            };
            wait = wait.min((IDLE_SECS - (now - t)).max(0.0));
        }
        if wait == f64::MAX {
            return;
        }
        ui.ctx()
            .request_repaint_after(Duration::from_secs_f64(wait));
    }

    pub fn enter_explore(&mut self) -> Result<(), Error> {
        self.mode = Mode::Explore;
        self.autosave.disk_moved = false;
        crate::dlog_info!("mode", "entered explore — writes suspended");
        self.status = "explore — sandbox: nothing reaches disk until you save or discard".into();
        Ok(())
    }

    pub fn explore_save(&mut self, ui: &mut egui::Ui) -> Result<(), Error> {
        if self.project.is_none() {
            return self.save_as_dialog(ui);
        }
        if self.reopen_conflict()? {
            return Ok(());
        }
        if self.sync_disk(ui, Push::Explicit, Resolve::Ask)? != Synced::Settled {
            return Ok(());
        }
        self.settle_sandbox(ui)
    }

    pub fn settle_sandbox(&mut self, ui: &egui::Ui) -> Result<(), Error> {
        self.mode = Mode::Live;
        self.autosave.disk_moved = false;
        self.autosave.conflict = None;
        crate::dlog_info!("mode", "explore saved — back to live");
        self.save_full(ui)?;
        self.last_saved_at = Some(now_unix()?);
        Ok(())
    }

    pub fn explore_discard(&mut self, ui: &mut egui::Ui) -> Result<(), Error> {
        let Some(project) = self.project.clone() else {
            self.status = "nothing on disk to fall back to — save the case first".into();
            return Ok(());
        };
        self.mode = Mode::Live;
        self.autosave.disk_moved = false;
        self.autosave.conflict = None;
        self.load(ui)?;
        crate::dlog_warn!("mode", "explore discarded — re-read {}", project.root.display());
        self.status = format!("explore discarded — re-read {}", project.root.display());
        Ok(())
    }

    pub fn mode_border(&self, ui: &egui::Ui) -> Result<(), Error> {
        if self.live()? {
            return Ok(());
        }
        let rect = ui.ctx().content_rect();
        let painter = ui.ctx().layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("rs_mode_border"),
        ));
        painter.rect_stroke(
            rect,
            0.0,
            egui::Stroke::new(4.0_f32, EXPLORE_INK),
            StrokeKind::Inside,
        );
        Ok(())
    }
}

fn past(since: Option<f64>, now: f64, span: f64) -> bool {
    let Some(t) = since else {
        return false;
    };
    now - t >= span
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_deadline_only_passes_once_the_span_has_elapsed() {
        assert!(!past(None, 100.0, IDLE_SECS), "nothing pending, nothing due");
        assert!(!past(Some(0.0), 9.99, IDLE_SECS), "still inside the idle window");
        assert!(past(Some(0.0), 10.0, IDLE_SECS), "exactly at the deadline fires");
        assert!(past(Some(100.0), 111.0, IDLE_SECS), "past the deadline fires");
    }

    #[test]
    fn the_debounce_window_is_shorter_than_the_idle_beat() {
        assert!(
            DEBOUNCE_SECS < IDLE_SECS,
            "the fast path must settle long before the derived artifacts are regenerated"
        );
        assert!(!past(Some(0.0), 1.4, DEBOUNCE_SECS), "a still-moving edit is not flushed");
        assert!(past(Some(0.0), 1.5, DEBOUNCE_SECS), "a settled edit flushes");
    }

    #[test]
    fn live_is_the_default_mode() {
        assert_eq!(Mode::default(), Mode::Live);
    }
}

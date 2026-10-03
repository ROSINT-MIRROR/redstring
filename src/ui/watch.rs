use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::dlog_warn;
use crate::error::Error;
use crate::project::{CaseState, DiskSig, Resolve, disk_sig};

use super::app::RedstringApp;
use super::reconcile::Push;

pub struct Watcher {
    pub base: Option<CaseState>,
    pub synced: Option<(PathBuf, DiskSig)>,
    pub pending: Option<DiskSig>,
    pub last_poll: Instant,
}

impl Default for Watcher {
    fn default() -> Self {
        Self {
            base: None,
            synced: None,
            pending: None,
            last_poll: Instant::now(),
        }
    }
}

impl Watcher {
    pub fn base_or_empty(&self) -> Result<CaseState, Error> {
        let Some(base) = &self.base else {
            return Ok(CaseState::default());
        };
        Ok(base.clone())
    }

    pub fn in_sync(&self, root: &Path, sig: &DiskSig) -> Result<bool, Error> {
        let Some((at, seen)) = &self.synced else {
            return Ok(false);
        };
        Ok(at == root && seen == sig)
    }

    pub fn settle(&mut self, root: &Path, base: CaseState, sig: DiskSig) -> Result<(), Error> {
        self.base = Some(base);
        self.synced = Some((root.to_path_buf(), sig));
        self.pending = None;
        Ok(())
    }

    pub fn forget(&mut self) -> Result<(), Error> {
        self.synced = None;
        self.pending = None;
        Ok(())
    }

    pub fn reset(&mut self) -> Result<(), Error> {
        self.base = None;
        self.forget()
    }
}

impl RedstringApp {
    pub fn self_write<T>(
        &mut self,
        root: &Path,
        f: impl FnOnce(&mut Self) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let Some(project) = self.project.clone() else {
            return f(self);
        };
        if project.root.as_path() != root {
            return f(self);
        }
        let was_synced = self.watcher.in_sync(root, &disk_sig(&project)?)?;
        let out = f(self);
        if was_synced {
            let sig = disk_sig(&project)?;
            self.watcher.synced = Some((root.to_path_buf(), sig));
        }
        out
    }

    pub fn disk_in_sync(&self) -> Result<bool, Error> {
        let Some(project) = &self.project else {
            return Ok(true);
        };
        self.watcher.in_sync(&project.root, &disk_sig(project)?)
    }

    pub fn poll_external_edit(&mut self, ui: &mut egui::Ui) -> Result<(), Error> {
        if self.watcher.last_poll.elapsed().as_millis() < 1000 {
            return Ok(());
        }
        self.watcher.last_poll = Instant::now();
        let Some(project) = self.project.clone() else {
            return Ok(());
        };
        ui.ctx().request_repaint_after(Duration::from_millis(1100));
        let sig = disk_sig(&project)?;
        if self.watcher.in_sync(&project.root, &sig)? {
            self.watcher.pending = None;
            return Ok(());
        }
        let Some(seen) = &self.watcher.pending else {
            self.watcher.pending = Some(sig);
            return Ok(());
        };
        if *seen != sig {
            self.watcher.pending = Some(sig);
            return Ok(());
        }
        if !self.live()? {
            if !self.autosave.disk_moved {
                dlog_warn!("watch", "external edit while exploring — holding {}", project.root.display());
                self.status =
                    "the case changed on disk — explore is holding your sandbox; saving merges".to_owned();
            }
            self.autosave.disk_moved = true;
            return Ok(());
        }
        self.sync_disk(ui, Push::Absorb, Resolve::Ask)?;
        Ok(())
    }
}

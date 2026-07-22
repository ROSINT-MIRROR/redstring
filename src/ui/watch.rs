use std::path::Path;
use std::time::{Instant, SystemTime};

use crate::error::Error;
use crate::project::Project;

use super::app::RedstringApp;

pub struct Watcher {
    pub baseline: Option<(String, SystemTime)>,
    pub last_poll: Instant,
}

impl Default for Watcher {
    fn default() -> Self {
        Self {
            baseline: None,
            last_poll: Instant::now(),
        }
    }
}

impl Watcher {
    fn rebase(&mut self, project: Option<&Project>) -> Result<(), Error> {
        let Some(project) = project else {
            self.baseline = None;
            return Ok(());
        };
        let path = project.case_path()?.display().to_string();
        self.baseline = newest_mtime(&project.root)?.map(|t| (path, t));
        Ok(())
    }
}

impl RedstringApp {
    pub fn self_write<T>(
        &mut self,
        root: &Path,
        f: impl FnOnce(&mut Self) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let out = f(self);
        let project = self.project.clone();
        let rebased = if project.as_ref().is_some_and(|p| p.root.as_path() == root) {
            self.watcher.rebase(project.as_ref())
        } else {
            Ok(())
        };
        let value = out?;
        rebased?;
        Ok(value)
    }

    pub fn watch_project(&mut self) -> Result<(), Error> {
        let project = self.project.clone();
        self.watcher.rebase(project.as_ref())
    }

    pub fn poll_external_edit(&mut self, ui: &mut egui::Ui) -> Result<(), Error> {
        if self.watcher.last_poll.elapsed().as_millis() < 1000 {
            return Ok(());
        }
        self.watcher.last_poll = Instant::now();
        let Some((path, seen)) = self.watcher.baseline.clone() else {
            return Ok(());
        };
        if Some(path.as_str()) != self.current_case_path().as_deref() {
            return Ok(());
        }
        let Some(project) = self.project.clone() else {
            return Ok(());
        };
        let Some(t) = newest_mtime(&project.root)? else {
            return Ok(());
        };
        if t > seen {
            crate::dlog_info!("watch", "hot-reload: external edit on {path}");
            let before = self.g.g().node_count();
            self.load(ui)?;
            let after = self.g.g().node_count();
            self.nodes_arrived(ui, after.saturating_sub(before), Vec::new())?;
            self.status = format!("external edit detected — reloaded {path}");
        }
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(1100));
        Ok(())
    }

    fn current_case_path(&self) -> Option<String> {
        let project = self.project.as_ref()?;
        let Ok(path) = project.case_path() else {
            return None;
        };
        Some(path.display().to_string())
    }
}

fn newest_mtime(root: &Path) -> Result<Option<SystemTime>, Error> {
    let mut times: Vec<SystemTime> = Vec::new();
    walk_mtime(root, &mut times)?;
    Ok(times.into_iter().max())
}

fn walk_mtime(dir: &Path, times: &mut Vec<SystemTime>) -> Result<(), Error> {
    let rd = std::fs::read_dir(dir).map_err(|source| Error::Io { path: dir.to_path_buf(), source })?;
    for entry in rd {
        let entry = entry.map_err(|source| Error::Io { path: dir.to_path_buf(), source })?;
        let ft = entry
            .file_type()
            .map_err(|source| Error::Io { path: entry.path(), source })?;
        if ft.is_dir() {
            if entry.file_name().to_str() == Some(".git") {
                continue;
            }
            walk_mtime(&entry.path(), times)?;
            continue;
        }
        if entry.file_name().to_str().is_some_and(|n| n.ends_with(".zip")) {
            continue;
        }
        let meta = entry
            .metadata()
            .map_err(|source| Error::Io { path: entry.path(), source })?;
        let t = meta
            .modified()
            .map_err(|source| Error::Io { path: entry.path(), source })?;
        times.push(t);
    }
    Ok(())
}

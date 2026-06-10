//! Project persistence: save/load against the case directory — case file,
//! per-entity note files, derived artifacts — plus the dialogs, bundle
//! export, and the mtime watch that hot-reloads external edits.

use std::collections::HashMap;
use std::path::Path;
use std::time::{Instant, SystemTime};

use egui::Pos2;
use egui_graphs::Graph;
use petgraph::graph::NodeIndex;
use petgraph::stable_graph::StableGraph;

use crate::error::RsResult;
use crate::model::{
    CaseDisplay, CaseFile, CaseMeta, DEFAULT_NODE_SIZE, Entity, EntityRec, Relation, RelationRec,
};
use crate::project::{self, Project};

use super::app::{RedstringApp, RsGraph};
use super::board::{camera_read, camera_write};
use super::ops::spiral;

impl RedstringApp {
    pub(super) fn to_case_file(&self, ui: &egui::Ui) -> CaseFile {
        let g = self.g.g();
        let mut cf = CaseFile::default();
        cf.meta = CaseMeta::stamped(&self.case_name);
        for n in g.node_weights() {
            let e = n.payload();
            cf.data.entities.push(EntityRec {
                slug: e.slug.clone(),
                kind: e.kind,
                label: e.label.clone(),
                attrs: e.attrs.clone(),
                image: self.image_rec(&e.image),
            });
            let loc = n.location();
            cf.display.positions.insert(e.slug.clone(), (loc.x, loc.y));
            if e.size != DEFAULT_NODE_SIZE {
                cf.display.sizes.insert(e.slug.clone(), e.size);
            }
        }
        for idx in g.edge_indices() {
            let Some((a, b)) = g.edge_endpoints(idx) else { continue };
            let Some(r) = g.edge_weight(idx) else { continue };
            let Some(from) = g.node_weight(a) else { continue };
            let Some(to) = g.node_weight(b) else { continue };
            cf.data.relations.push(RelationRec {
                from: from.payload().slug.clone(),
                to: to.payload().slug.clone(),
                kind: r.payload().kind,
                sources: r.payload().sources.clone(),
            });
        }
        cf.display.camera = camera_read(ui);
        cf
    }

    /// Runtime image paths are absolute; the case file cites them
    /// project-relative. Anything outside the project stays absolute and
    /// the validator flags it.
    fn image_rec(&self, image: &Option<String>) -> Option<String> {
        let img = image.as_ref()?;
        let Some(project) = &self.project else {
            return Some(img.clone());
        };
        Some(project.rel_or_self(img))
    }

    /// Ctrl+S: save into the open case directory; with no case yet, fall
    /// through to creating one.
    pub(super) fn save(&mut self, ui: &egui::Ui) {
        if self.project.is_none() {
            self.save_as_dialog(ui);
            return;
        }
        self.save_to_project(ui);
    }

    fn save_to_project(&mut self, ui: &egui::Ui) {
        let Some(project) = self.project.clone() else { return };
        let cf = self.to_case_file(ui);
        match self.write_all(&project, &cf) {
            Ok(()) => {
                self.watch_current();
                self.status = format!(
                    "saved {} entities / {} links → {}{}",
                    cf.data.entities.len(),
                    cf.data.relations.len(),
                    project.root().display(),
                    warn_suffix(&project),
                );
            }
            Err(err) => self.status = format!("save failed: {err}"),
        }
    }

    /// One save = the case file, every entity's note file, and the derived
    /// artifacts (case.json for the web viewer, graph.md for GitHub).
    fn write_all(&self, project: &Project, cf: &CaseFile) -> RsResult<()> {
        project::write_case(project, cf)?;
        for n in self.g.g().node_weights() {
            let e = n.payload();
            project::write_note(project, &e.slug, &e.note)?;
        }
        project::write_json(project, cf)?;
        project::write_graph_md(project, cf)?;
        Ok(())
    }

    /// Save, then zip the case directory to <root>/<dirname>.redstring.zip.
    pub(super) fn export_bundle(&mut self, ui: &egui::Ui) {
        if self.project.is_none() {
            self.status = "save the case first — a bundle is the case directory, zipped".into();
            return;
        }
        self.save_to_project(ui);
        let Some(project) = &self.project else { return };
        match project::export(project) {
            Ok((path, n)) => self.status = format!("bundled {n} files → {}", path.display()),
            Err(err) => self.status = format!("export failed: {err}"),
        }
    }

    /// Pick a case file or a bundle and open it.
    pub(super) fn open_dialog(&mut self, ui: &mut egui::Ui) {
        let Some(p) = open_picker(self.project.as_ref()).pick_file() else {
            return;
        };
        self.open_path(ui, &p);
    }

    /// Pick a case directory and open it — the folder twin of open_dialog.
    pub(super) fn open_folder_dialog(&mut self, ui: &mut egui::Ui) {
        let Some(p) = folder_picker(self.project.as_ref()).pick_folder() else {
            return;
        };
        self.open_path(ui, &p);
    }

    /// A case directory, case file or bundle dropped onto the window opens it.
    pub(super) fn handle_drop(&mut self, ui: &mut egui::Ui) {
        let dropped = ui.input(|i| i.raw.dropped_files.clone());
        let Some(path) = dropped.into_iter().find_map(|f| f.path) else {
            return;
        };
        self.open_path(ui, &path);
    }

    /// Open any case form. A directory goes straight to the marker check, a
    /// `.zip` is read as a bundle, anything else as a case file. The dir test
    /// runs first so a directory named like an archive never reaches the zip
    /// reader.
    pub(super) fn open_path(&mut self, ui: &mut egui::Ui, path: &Path) {
        let opened = if path.is_dir() {
            Project::open(path)
        } else if path.extension().is_some_and(|e| e == "zip") {
            project::open_bundle(path)
        } else {
            Project::open(path)
        };
        match opened {
            Ok(project) => {
                self.project = Some(project);
                self.load(ui);
                self.show_welcome = false;
                self.remember_project();
            }
            Err(err) => self.status = format!("open failed: {err}"),
        }
    }

    /// Pick where the new case directory goes — the typed name becomes the
    /// directory — then save into it.
    pub(super) fn save_as_dialog(&mut self, ui: &egui::Ui) {
        let suggest = dir_suggestion(&self.case_name);
        let Some(p) = create_picker(self.project.as_ref(), &suggest).save_file() else {
            return;
        };
        match Project::create(&p) {
            Ok(project) => {
                // a still-default case name takes the directory's — one
                // less thing to type for a fresh case
                if self.case_name.trim().is_empty() || self.case_name == "untitled case" {
                    self.case_name = project.dir_name();
                }
                self.project = Some(project);
                self.save_to_project(ui);
                self.show_welcome = false;
                self.remember_project();
            }
            Err(err) => self.status = format!("create case failed: {err}"),
        }
    }

    /// Front-load the open case in the recents list and persist prefs.
    pub(super) fn remember_project(&mut self) {
        let Some(project) = &self.project else { return };
        let path = project.root().display().to_string();
        self.prefs.remember_recent(path);
        self.persist_prefs();
    }

    pub(super) fn load(&mut self, ui: &mut egui::Ui) {
        let Some(project) = self.project.clone() else {
            self.status = "no case open".into();
            return;
        };
        let cf = match project::read_case(&project) {
            Ok(cf) => cf,
            Err(err) => {
                self.status = format!("load failed: {err}");
                return;
            }
        };

        let mut g: RsGraph = Graph::from(&StableGraph::default());
        let mut by_slug: HashMap<String, NodeIndex> = HashMap::new();
        let mut healed = 0usize;
        let mut note_errs: Vec<String> = Vec::new();
        for (i, er) in cf.data.entities.iter().enumerate() {
            if by_slug.contains_key(&er.slug) {
                self.status = format!("load failed: duplicate slug '{}'", er.slug);
                return;
            }
            let pos = healed_pos(&cf.display, &er.slug, i, &mut healed);
            let note = match project::read_note(&project, &er.slug) {
                Ok(n) => n,
                Err(err) => {
                    note_errs.push(format!("{err}"));
                    String::new()
                }
            };
            let ent = Entity {
                slug: er.slug.clone(),
                kind: er.kind,
                label: er.label.clone(),
                attrs: er.attrs.clone(),
                note,
                // relative in the file, absolute at runtime; joining an
                // absolute legacy path leaves it unchanged
                image: er
                    .image
                    .as_ref()
                    .map(|rel| project.resolve(rel).display().to_string()),
                size: cf.display.size_of(&er.slug),
            };
            let idx = g.add_node_with_label_and_location(
                ent,
                format!("{} {}", er.kind.icon(), er.label),
                pos,
            );
            by_slug.insert(er.slug.clone(), idx);
        }

        let mut skipped: Vec<String> = Vec::new();
        for rr in &cf.data.relations {
            let (Some(&a), Some(&b)) = (by_slug.get(&rr.from), by_slug.get(&rr.to)) else {
                skipped.push(format!("{} -> {}", rr.from, rr.to));
                continue;
            };
            g.add_edge_with_label(
                a,
                b,
                Relation {
                    kind: rr.kind,
                    sources: rr.sources.clone(),
                },
                rr.kind.label().into(),
            );
        }

        self.g = g;
        self.case_name = cf.meta.name.clone();
        // node indices just got reassigned — abort any in-flight gesture
        self.edge_drag = None;
        self.pending_edge = None;
        self.node_drag = None;
        self.resize = None;
        // Pre-write so the GraphView, added later this same frame, draws with
        // the case camera — flash-free for button loads and hot reloads. The
        // widget's very first frame force-fits over it, hence the deferred
        // re-assert in the app shell.
        camera_write(ui, cf.display.camera);
        self.pending_camera = Some(cf.display.camera);
        self.watch_current();

        self.status = format!(
            "loaded {} entities / {} links from {}",
            cf.data.entities.len(),
            cf.data.relations.len(),
            project.root().display()
        );
        if healed > 0 {
            self.status += &format!(" · placed {healed} unpositioned");
        }
        if !note_errs.is_empty() {
            self.status += &format!(" · {} notes unreadable: {}", note_errs.len(), note_errs[0]);
        }
        if !skipped.is_empty() {
            self.status += &format!(" · SKIPPED unknown slugs: {}", skipped.join(", "));
        }
    }

    /// Remember the case file's path + mtime as the hot-reload baseline.
    pub(super) fn watch_current(&mut self) {
        let Some(project) = &self.project else {
            self.watched = None;
            return;
        };
        let path = project.case_path().display().to_string();
        self.watched = mtime(&path).map(|t| (path, t));
    }

    /// Hot-reload: when the watched case file changed under us — external
    /// editor, AI appending — pull it in. Polled at most about once a second.
    pub(super) fn poll_external_edit(&mut self, ui: &mut egui::Ui) {
        if self.last_watch_poll.elapsed().as_millis() < 1000 {
            return;
        }
        self.last_watch_poll = Instant::now();
        let Some((path, seen)) = self.watched.clone() else {
            return;
        };
        if Some(path.as_str()) != self.current_case_path().as_deref() {
            return; // a different case got opened — stale watch, ignore
        }
        let Some(t) = mtime(&path) else { return };
        if t > seen {
            self.load(ui);
            self.status = format!("external edit detected — reloaded {path}");
        }
        // request a wakeup so the watcher ticks even when the user is idle
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(1100));
    }

    fn current_case_path(&self) -> Option<String> {
        let project = self.project.as_ref()?;
        Some(project.case_path().display().to_string())
    }

    /// Toolbar label: where the case lives.
    pub(super) fn project_label(&self) -> String {
        let Some(project) = &self.project else {
            return "unsaved — Ctrl+S creates the case directory".to_owned();
        };
        project.root().display().to_string()
    }
}

/// Post-save consistency summary for the status bar.
fn warn_suffix(project: &Project) -> String {
    match project::validate(project) {
        Ok(findings) if findings.is_empty() => String::new(),
        Ok(findings) => format!(
            " · ⚠ {} finding(s) — run redstring --check",
            findings.len()
        ),
        Err(err) => format!(" · validate failed: {err}"),
    }
}

/// Position from the display block, or the next spiral slot when missing.
fn healed_pos(display: &CaseDisplay, slug: &str, i: usize, healed: &mut usize) -> Pos2 {
    let Some(&(x, y)) = display.positions.get(slug) else {
        *healed += 1;
        return spiral(i);
    };
    Pos2::new(x, y)
}

/// Modification time when the file is reachable. Best-effort by design — an
/// unreachable file simply gives the watcher nothing to compare against.
fn mtime(path: &str) -> Option<SystemTime> {
    let Ok(meta) = std::fs::metadata(path) else {
        return None;
    };
    let Ok(t) = meta.modified() else {
        return None;
    };
    Some(t)
}

/// Open picker: the case file itself or a bundle. The combined filter is the
/// default so neither form hides behind a dropdown switch.
fn open_picker(current: Option<&Project>) -> rfd::FileDialog {
    let d = rfd::FileDialog::new()
        .set_title("open case or bundle")
        .add_filter("redstring case or bundle", &["redstring", "zip"])
        .add_filter("redstring case", &["redstring"])
        .add_filter("redstring bundle", &["zip"]);
    seed_dir(d, current)
}

/// Folder picker for opening a case by its directory.
fn folder_picker(current: Option<&Project>) -> rfd::FileDialog {
    let d = rfd::FileDialog::new().set_title("open case directory");
    seed_dir(d, current)
}

/// Create picker: the typed name becomes the new case directory.
fn create_picker(current: Option<&Project>, suggest: &str) -> rfd::FileDialog {
    let name = if suggest.is_empty() { "mycase" } else { suggest };
    let d = rfd::FileDialog::new()
        .set_title("create case directory")
        .set_file_name(name);
    seed_dir(d, current)
}

/// Directory suggestion for the create picker: the case display name slugged
/// to lowercase alnum runs joined by underscores. The untouched default name
/// suggests nothing, leaving the picker's own placeholder.
fn dir_suggestion(name: &str) -> String {
    if name == "untitled case" {
        return String::new();
    }
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect::<String>()
        .split('_')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

fn seed_dir(d: rfd::FileDialog, current: Option<&Project>) -> rfd::FileDialog {
    let Some(project) = current else { return d };
    let Some(parent) = project.root().parent() else {
        return d;
    };
    d.set_directory(parent)
}

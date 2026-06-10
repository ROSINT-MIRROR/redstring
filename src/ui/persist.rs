//! Case persistence: RON read/write with position healing, the open/save
//! dialogs, and the mtime watch that hot-reloads external edits.

use std::collections::HashMap;
use std::time::{Instant, SystemTime};

use egui::Pos2;
use egui_graphs::Graph;
use petgraph::graph::NodeIndex;
use petgraph::stable_graph::StableGraph;

use crate::error::{Error, RsResult};
use crate::model::{CaseDisplay, CaseFile, DEFAULT_NODE_SIZE, Entity, EntityRec, Relation, RelationRec};

use super::app::{RedstringApp, RsGraph};
use super::board::{camera_read, camera_write};
use super::ops::spiral;

impl RedstringApp {
    pub(super) fn to_case_file(&self, ui: &egui::Ui) -> CaseFile {
        let g = self.g.g();
        let mut cf = CaseFile::default();
        for n in g.node_weights() {
            let e = n.payload();
            cf.data.entities.push(EntityRec {
                slug: e.slug.clone(),
                kind: e.kind,
                label: e.label.clone(),
                attrs: e.attrs.clone(),
                note: e.note.clone(),
                image: e.image.clone(),
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

    fn write_case(&self, cf: &CaseFile) -> RsResult<()> {
        let s = ron::ser::to_string_pretty(cf, ron::ser::PrettyConfig::default())
            .map_err(|source| Error::RonSer { source })?;
        std::fs::write(&self.case_path, s)
            .map_err(|source| Error::io(self.case_path.as_str(), source))?;
        Ok(())
    }

    fn read_case(&self) -> RsResult<CaseFile> {
        let s = std::fs::read_to_string(&self.case_path)
            .map_err(|source| Error::io(self.case_path.as_str(), source))?;
        ron::from_str(&s).map_err(|source| Error::ron_parse(self.case_path.as_str(), source))
    }

    pub(super) fn save(&mut self, ui: &egui::Ui) {
        let cf = self.to_case_file(ui);
        match self.write_case(&cf) {
            Ok(()) => {
                self.watch_current();
                self.status = format!(
                    "saved {} entities / {} links → {}",
                    cf.data.entities.len(),
                    cf.data.relations.len(),
                    self.case_path
                );
            }
            Err(err) => self.status = format!("save failed: {err}"),
        }
    }

    /// Pick a case file and load it.
    pub(super) fn open_dialog(&mut self, ui: &mut egui::Ui) {
        let Some(p) = case_dialog(&self.case_path).pick_file() else {
            return;
        };
        self.case_path = p.display().to_string();
        self.load(ui);
    }

    /// Pick a destination and save the case there — it becomes the open file.
    pub(super) fn save_as_dialog(&mut self, ui: &egui::Ui) {
        let Some(mut p) = case_dialog(&self.case_path).save_file() else {
            return;
        };
        if p.extension().is_none() {
            p.set_extension("ron");
        }
        self.case_path = p.display().to_string();
        self.save(ui);
    }

    pub(super) fn load(&mut self, ui: &mut egui::Ui) {
        let cf = match self.read_case() {
            Ok(cf) => cf,
            Err(err) => {
                self.status = format!("load failed: {err}");
                return;
            }
        };

        let mut g: RsGraph = Graph::from(&StableGraph::default());
        let mut by_slug: HashMap<String, NodeIndex> = HashMap::new();
        let mut healed = 0usize;
        for (i, er) in cf.data.entities.iter().enumerate() {
            if by_slug.contains_key(&er.slug) {
                self.status = format!("load failed: duplicate slug '{}'", er.slug);
                return;
            }
            let pos = healed_pos(&cf.display, &er.slug, i, &mut healed);
            let ent = Entity {
                slug: er.slug.clone(),
                kind: er.kind,
                label: er.label.clone(),
                attrs: er.attrs.clone(),
                note: er.note.clone(),
                image: er.image.clone(),
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
            self.case_path
        );
        if healed > 0 {
            self.status += &format!(" · placed {healed} unpositioned");
        }
        if !skipped.is_empty() {
            self.status += &format!(" · SKIPPED unknown slugs: {}", skipped.join(", "));
        }
    }

    /// Remember the current path + mtime as the hot-reload baseline.
    pub(super) fn watch_current(&mut self) {
        self.watched = mtime(&self.case_path).map(|t| (self.case_path.clone(), t));
    }

    /// Hot-reload: when the watched file changed under us — external editor,
    /// AI appending to the case — pull it in. Polled at most about once a second.
    pub(super) fn poll_external_edit(&mut self, ui: &mut egui::Ui) {
        if self.last_watch_poll.elapsed().as_millis() < 1000 {
            return;
        }
        self.last_watch_poll = Instant::now();
        let Some((path, seen)) = &self.watched else {
            return;
        };
        if *path != self.case_path {
            return; // user retargeted the path — don't auto-load
        }
        let Some(t) = mtime(path) else { return };
        if t > *seen {
            self.load(ui);
            self.status = format!("external edit detected — reloaded {}", self.case_path);
        }
        // request a wakeup so the watcher ticks even when the user is idle
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(1100));
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

/// Native .ron dialog seeded with the current path's directory + file name.
/// Blocking is fine: a frozen frame behind a modal picker is the desktop
/// behaviour we want.
fn case_dialog(current: &str) -> rfd::FileDialog {
    let d = rfd::FileDialog::new().add_filter("RON case", &["ron"]);
    let p = std::path::Path::new(current);
    let d = seed_dir(d, p);
    seed_name(d, p)
}

fn seed_dir(d: rfd::FileDialog, p: &std::path::Path) -> rfd::FileDialog {
    let Some(dir) = p.parent().filter(|d| d.is_dir()) else {
        return d;
    };
    d.set_directory(dir)
}

fn seed_name(d: rfd::FileDialog, p: &std::path::Path) -> rfd::FileDialog {
    let Some(name) = p.file_name().and_then(|n| n.to_str()) else {
        return d;
    };
    d.set_file_name(name)
}

use std::collections::HashMap;
use std::path::Path;
use std::time::{Instant, SystemTime};

use egui::Pos2;
use egui_graphs::Graph;
use petgraph::graph::NodeIndex;
use petgraph::stable_graph::StableGraph;

use crate::error::Error;
use crate::model::{
    CaseDisplay, CaseFile, CaseMeta, DEFAULT_NODE_SIZE, Entity, EntityRec, KIND_META,
    RELATION_META, Relation, RelationRec,
};
use crate::project::{self, Project};

use super::app::{RedstringApp, RsGraph};
use super::board::{camera_read, camera_write};
use super::ops::spiral;

impl RedstringApp {
    fn to_case_file(&self, ui: &egui::Ui) -> Result<CaseFile, Error> {
        let g = self.g.g();
        let mut cf = CaseFile::default();
        cf.meta = CaseMeta::stamped(&self.case_name)?;
        for n in g.node_weights() {
            let e = n.payload();
            cf.data.entities.push(EntityRec {
                slug: e.slug.clone(),
                kind: e.kind,
                label: e.label.clone(),
                attrs: e.attrs.clone(),
                image: self.image_rec(&e.image)?,
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
        cf.display.camera = camera_read(ui)?;
        Ok(cf)
    }

    fn image_rec(&self, image: &Option<String>) -> Result<Option<String>, Error> {
        let Some(img) = image.as_ref() else {
            return Ok(None);
        };
        let Some(project) = &self.project else {
            return Ok(Some(img.clone()));
        };
        Ok(Some(project.rel_or_self(img)?))
    }

    pub fn save(&mut self, ui: &egui::Ui) -> Result<(), Error> {
        if self.project.is_none() {
            return self.save_as_dialog(ui);
        }
        self.save_to_project(ui)
    }



    pub fn fingerprint(&self) -> Result<String, Error> {
        let g = self.g.g();
        let mut parts: Vec<String> = Vec::new();
        for n in g.node_weights() {
            let e = n.payload();
            let loc = n.location();
            parts.push(format!(
                "n|{}|{:?}|{}|{:?}|{:?}|{}|{:.1}|{:.1}|{}",
                e.slug, e.kind, e.label, e.attrs, e.image, e.note, loc.x, loc.y, e.size
            ));
        }
        for idx in g.edge_indices() {
            let Some((a, b)) = g.edge_endpoints(idx) else { continue };
            let Some(r) = g.edge_weight(idx) else { continue };
            let Some(from) = g.node_weight(a) else { continue };
            let Some(to) = g.node_weight(b) else { continue };
            let srcs = ron::to_string(&r.payload().sources)
                .map_err(|source| Error::RonSer { source })?;
            parts.push(format!(
                "e|{}|{}|{:?}|{}",
                from.payload().slug,
                to.payload().slug,
                r.payload().kind,
                srcs
            ));
        }
        parts.sort();
        parts.insert(0, self.case_name.clone());
        Ok(parts.join("\n"))
    }

    pub fn dirty(&self) -> Result<bool, Error> {
        Ok(self.fingerprint()? != self.saved_fp)
    }

    fn save_to_project(&mut self, ui: &egui::Ui) -> Result<(), Error> {
        let Some(project) = self.project.clone() else {
            return Ok(());
        };
        let cf = self.to_case_file(ui)?;
        self.write_all(&project, &cf)?;
        self.watch_current()?;
        self.saved_fp = self.fingerprint()?;
        self.status = format!(
            "saved {} entities / {} links → {}{}",
            cf.data.entities.len(),
            cf.data.relations.len(),
            project.root.display(),
            warn_suffix(&project),
        );
        Ok(())
    }

    fn write_all(&self, project: &Project, cf: &CaseFile) -> Result<(), Error> {
        project::write_case(project, cf)?;
        for n in self.g.g().node_weights() {
            let e = n.payload();
            project::write_note(project, &e.slug, &e.note)?;
        }
        project::write_json(project, cf)?;
        project::write_graph_md(project, cf)?;
        Ok(())
    }

    pub fn export_bundle(&mut self, ui: &egui::Ui) -> Result<(), Error> {
        if self.project.is_none() {
            self.status = "save the case first — a bundle is the case directory, zipped".into();
            return Ok(());
        }
        self.save_to_project(ui)?;
        let Some(project) = &self.project else {
            return Ok(());
        };
        let (path, n) = project::export(project)?;
        self.status = format!("bundled {n} files → {}", path.display());
        Ok(())
    }

    pub fn open_dialog(&mut self, ui: &mut egui::Ui) -> Result<(), Error> {
        let Some(p) = open_picker(self.project.as_ref()).pick_file() else {
            return Ok(());
        };
        self.open_path(ui, &p)
    }

    pub fn open_folder_dialog(&mut self, ui: &mut egui::Ui) -> Result<(), Error> {
        let Some(p) = folder_picker(self.project.as_ref()).pick_folder() else {
            return Ok(());
        };
        self.open_path(ui, &p)
    }

    pub fn handle_drop(&mut self, ui: &mut egui::Ui) -> Result<(), Error> {
        let dropped = ui.input(|i| i.raw.dropped_files.clone());
        let Some(path) = dropped.into_iter().find_map(|f| f.path) else {
            return Ok(());
        };
        self.open_path(ui, &path)
    }

    pub fn open_path(&mut self, ui: &mut egui::Ui, path: &Path) -> Result<(), Error> {
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
                self.load(ui)?;
                self.show_welcome = false;
                self.remember_project()?;
                Ok(())
            }
            Err(err) => {
                self.status = format!("open failed: {err}");
                Ok(())
            }
        }
    }

    pub fn save_as_dialog(&mut self, ui: &egui::Ui) -> Result<(), Error> {
        let suggest = dir_suggestion(&self.case_name);
        let Some(p) = create_picker(self.project.as_ref(), &suggest).save_file() else {
            return Ok(());
        };
        let project = Project::create(&p)?;
        if self.case_name.trim().is_empty() || self.case_name == "untitled case" {
            self.case_name = project.dir_name()?;
        }
        self.project = Some(project);
        self.save_to_project(ui)?;
        self.show_welcome = false;
        self.remember_project()?;
        Ok(())
    }

    pub fn remember_project(&mut self) -> Result<(), Error> {
        let Some(project) = &self.project else {
            return Ok(());
        };
        let path = project.root.display().to_string();
        self.prefs.remember_recent(path)
    }

    pub fn load(&mut self, ui: &mut egui::Ui) -> Result<(), Error> {
        let Some(project) = self.project.clone() else {
            self.status = "no case open".into();
            return Ok(());
        };
        let cf = project::read_case(&project)?;

        let mut g: RsGraph = Graph::from(&StableGraph::default());
        let mut by_slug: HashMap<String, NodeIndex> = HashMap::new();
        let mut healed = 0usize;
        let mut note_errs: Vec<String> = Vec::new();
        for (i, er) in cf.data.entities.iter().enumerate() {
            if by_slug.contains_key(&er.slug) {
                return Err(Error::DuplicateSlug {
                    slug: er.slug.clone(),
                });
            }
            let pos = healed_pos(&cf.display, &er.slug, i, &mut healed)?;
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
                image: image_abs(&project, &er.image)?,
                size: size_of(&cf.display, &er.slug),
            };
            let idx = g.add_node_with_label_and_location(
                ent,
                format!("{} {}", KIND_META[er.kind as usize].icon, er.label),
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
                RELATION_META[rr.kind as usize].label.into(),
            );
        }

        self.g = g;
        self.case_name = cf.meta.name.clone();
        self.edge_drag = None;
        self.pending_edge = None;
        self.node_drag = None;
        self.resize = None;
        self.layout_anim = None;
        camera_write(ui, cf.display.camera)?;
        self.pending_camera = Some(cf.display.camera);
        self.watch_current()?;
        self.saved_fp = self.fingerprint()?;

        self.status = format!(
            "loaded {} entities / {} links from {}",
            cf.data.entities.len(),
            cf.data.relations.len(),
            project.root.display()
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
        Ok(())
    }

    pub fn watch_current(&mut self) -> Result<(), Error> {
        let Some(project) = &self.project else {
            self.watched = None;
            return Ok(());
        };
        let path = project.case_path()?.display().to_string();
        self.watched = mtime(&path).map(|t| (path, t));
        Ok(())
    }

    pub fn poll_external_edit(&mut self, ui: &mut egui::Ui) -> Result<(), Error> {
        if self.last_watch_poll.elapsed().as_millis() < 1000 {
            return Ok(());
        }
        self.last_watch_poll = Instant::now();
        let Some((path, seen)) = self.watched.clone() else {
            return Ok(());
        };
        if Some(path.as_str()) != self.current_case_path().as_deref() {
            return Ok(());
        }
        let Some(t) = mtime(&path) else {
            return Ok(());
        };
        if t > seen {
            self.load(ui)?;
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

    pub fn project_label(&self) -> Result<String, Error> {
        let Some(project) = &self.project else {
            return Ok("unsaved — Ctrl+S creates the case directory".to_owned());
        };
        Ok(project.root.display().to_string())
    }
}

fn image_abs(project: &Project, image: &Option<String>) -> Result<Option<String>, Error> {
    let Some(rel) = image.as_ref() else {
        return Ok(None);
    };
    Ok(Some(project.resolve(rel)?.display().to_string()))
}

fn size_of(display: &CaseDisplay, slug: &str) -> f32 {
    let Some(s) = display.sizes.get(slug) else {
        return DEFAULT_NODE_SIZE;
    };
    *s
}

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

fn healed_pos(display: &CaseDisplay, slug: &str, i: usize, healed: &mut usize) -> Result<Pos2, Error> {
    let Some(&(x, y)) = display.positions.get(slug) else {
        *healed += 1;
        return spiral(i);
    };
    Ok(Pos2::new(x, y))
}

fn mtime(path: &str) -> Option<SystemTime> {
    let Ok(meta) = std::fs::metadata(path) else {
        return None;
    };
    let Ok(t) = meta.modified() else {
        return None;
    };
    Some(t)
}

fn open_picker(current: Option<&Project>) -> rfd::FileDialog {
    let d = rfd::FileDialog::new()
        .set_title("open case or bundle")
        .add_filter("redstring case or bundle", &["redstring", "zip"])
        .add_filter("redstring case", &["redstring"])
        .add_filter("redstring bundle", &["zip"]);
    seed_dir(d, current)
}

fn folder_picker(current: Option<&Project>) -> rfd::FileDialog {
    let d = rfd::FileDialog::new().set_title("open case directory");
    seed_dir(d, current)
}

fn create_picker(current: Option<&Project>, suggest: &str) -> rfd::FileDialog {
    let name = if suggest.is_empty() { "mycase" } else { suggest };
    let d = rfd::FileDialog::new()
        .set_title("create case directory")
        .set_file_name(name);
    seed_dir(d, current)
}

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
    let Some(parent) = project.root.parent() else {
        return d;
    };
    d.set_directory(parent)
}

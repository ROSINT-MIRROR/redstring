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
    RELATION_META, Relation, RelationRec, now_unix,
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
                value: e.value.clone(),
                sealed: e.sealed,
                observations: e.observations.clone(),
                facets: e.facets.clone(),
                image: self.image_rec(&e.image)?,
            });
            let loc = n.location();
            cf.display.positions.insert(e.slug.clone(), (loc.x, loc.y));
            if e.size != DEFAULT_NODE_SIZE {
                cf.display.sizes.insert(e.slug.clone(), e.size);
            }
            if e.collapsed {
                cf.display.collapsed.insert(e.slug.clone());
            }
        }
        for idx in g.edge_indices() {
            let Some((a, b)) = g.edge_endpoints(idx) else { continue };
            let Some(r) = g.edge_weight(idx) else { continue };
            let Some(from) = g.node_weight(a) else { continue };
            let Some(to) = g.node_weight(b) else { continue };
            let rel = r.payload();
            cf.data.relations.push(RelationRec {
                from: from.payload().slug.clone(),
                to: to.payload().slug.clone(),
                kind: rel.kind,
                origin: rel.origin.clone(),
                status: rel.status,
                confidence: rel.confidence,
                author: rel.author.clone(),
                created_at: rel.created_at,
                sources: rel.sources.clone(),
            });
        }
        cf.display.camera = camera_read(ui)?;
        cf.display.size_by_degree = self.size_by_degree;
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
        compute_fingerprint(&self.g, &self.case_name, self.size_by_degree)
    }

    pub fn dirty(&self) -> Result<bool, Error> {
        let fp = self.fingerprint()?;
        Ok(dirty_state(self.project.is_some(), &fp, &self.saved_fp))
    }

    fn save_to_project(&mut self, ui: &egui::Ui) -> Result<(), Error> {
        let Some(project) = self.project.clone() else {
            return Ok(());
        };
        let cf = self.to_case_file(ui)?;
        self.write_all(&project, &cf)?;
        self.watch_current()?;
        self.saved_fp = self.fingerprint()?;
        self.last_saved_at = Some(now_unix()?);
        crate::dlog_info!(
            "persist",
            "save {} entities / {} links -> {}",
            cf.data.entities.len(),
            cf.data.relations.len(),
            project.root.display()
        );
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
        let (path, n, sealed) = project::export(project)?;
        crate::dlog_info!("persist", "export bundle {n} files -> {}", path.display());
        let redacted = if sealed > 0 { format!(" · {sealed} sealed entities redacted from bundle") } else { String::new() };
        self.status = format!("bundled {n} files → {}{redacted}", path.display());
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
                crate::dlog_info!("persist", "open ok {}", project.root.display());
                self.project = Some(project);
                self.load(ui)?;
                self.show_welcome = false;
                self.remember_project()?;
                Ok(())
            }
            Err(err) => {
                crate::dlog_warn!("persist", "open failed: {err}");
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
        crate::dlog_info!("persist", "load begin {}", project.root.display());
        let cf = project::read_case(&project)?;

        let LoadedGraph {
            g,
            healed,
            note_errs,
            skipped,
        } = build_graph(&project, &cf)?;

        self.g = g;
        self.case_name = cf.meta.name.clone();
        self.size_by_degree = cf.display.size_by_degree;
        self.edge_drag = None;
        self.pending_edge = None;
        self.node_drag = None;
        self.resize = None;
        self.layout_anim = None;
        camera_write(ui, cf.display.camera)?;
        self.pending_camera = Some(cf.display.camera);
        self.watch_current()?;
        self.saved_fp = self.fingerprint()?;
        crate::dlog_info!(
            "persist",
            "load done {} entities / {} links / {healed} healed / {} skipped",
            cf.data.entities.len(),
            cf.data.relations.len(),
            skipped.len()
        );

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
        self.watched = newest_mtime(&project.root)?.map(|t| (path, t));
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
        let Some(project) = self.project.clone() else {
            return Ok(());
        };
        let Some(t) = newest_mtime(&project.root)? else {
            return Ok(());
        };
        if t > seen {
            crate::dlog_info!("persist", "hot-reload: external edit on {path}");
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

pub struct LoadedGraph {
    pub g: RsGraph,
    pub healed: usize,
    pub note_errs: Vec<String>,
    pub skipped: Vec<String>,
}

pub fn build_graph(project: &Project, cf: &CaseFile) -> Result<LoadedGraph, Error> {
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
        let note = match project::read_note(project, &er.slug) {
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
            value: er.value.clone(),
            sealed: er.sealed,
            observations: er.observations.clone(),
            facets: er.facets.clone(),
            note,
            image: image_abs(project, &er.image)?,
            size: size_of(&cf.display, &er.slug),
            boost: 1.0,
            collapsed: cf.display.collapsed.contains(&er.slug),
            hidden: false,
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
                origin: rr.origin.clone(),
                status: rr.status,
                confidence: rr.confidence,
                author: rr.author.clone(),
                created_at: rr.created_at,
                sources: rr.sources.clone(),
                hidden: false,
            },
            RELATION_META[rr.kind as usize].label.into(),
        );
    }

    Ok(LoadedGraph {
        g,
        healed,
        note_errs,
        skipped,
    })
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

fn dirty_state(case_open: bool, fp: &str, saved_fp: &str) -> bool {
    case_open && fp != saved_fp
}

fn compute_fingerprint(graph: &RsGraph, case_name: &str, size_by_degree: bool) -> Result<String, Error> {
    let g = graph.g();
    let mut recs: Vec<String> = Vec::new();
    let mut display = CaseDisplay {
        size_by_degree,
        ..CaseDisplay::default()
    };
    for n in g.node_weights() {
        let e = n.payload();
        let loc = n.location();
        recs.push(node_fp_line(e)?);
        display.positions.insert(e.slug.clone(), (loc.x, loc.y));
        if e.size != DEFAULT_NODE_SIZE {
            display.sizes.insert(e.slug.clone(), e.size);
        }
        if e.collapsed {
            display.collapsed.insert(e.slug.clone());
        }
    }
    for idx in g.edge_indices() {
        let Some((a, b)) = g.edge_endpoints(idx) else { continue };
        let Some(r) = g.edge_weight(idx) else { continue };
        let Some(from) = g.node_weight(a) else { continue };
        let Some(to) = g.node_weight(b) else { continue };
        recs.push(edge_fp_line(&from.payload().slug, &to.payload().slug, r.payload())?);
    }
    recs.sort();
    recs.insert(0, display_fp(&display)?);
    recs.insert(0, case_name.to_owned());
    Ok(recs.join("\n"))
}

fn node_fp_line(e: &Entity) -> Result<String, Error> {
    let rec = EntityRec {
        slug: e.slug.clone(),
        kind: e.kind,
        label: e.label.clone(),
        value: e.value.clone(),
        sealed: e.sealed,
        observations: e.observations.clone(),
        facets: e.facets.clone(),
        image: e.image.clone(),
    };
    let body = serde_json::to_string(&rec).map_err(|source| Error::JsonSer { source })?;
    Ok(format!("n|{body}|{:?}", e.note))
}

fn edge_fp_line(from: &str, to: &str, r: &Relation) -> Result<String, Error> {
    let rec = RelationRec {
        from: from.to_owned(),
        to: to.to_owned(),
        kind: r.kind,
        origin: r.origin.clone(),
        status: r.status,
        confidence: r.confidence,
        author: r.author.clone(),
        created_at: r.created_at,
        sources: r.sources.clone(),
    };
    let body = serde_json::to_string(&rec).map_err(|source| Error::JsonSer { source })?;
    Ok(format!("e|{body}"))
}

fn display_fp(d: &CaseDisplay) -> Result<String, Error> {
    let body = serde_json::to_string(&(&d.positions, &d.sizes, &d.collapsed, d.size_by_degree))
        .map_err(|source| Error::JsonSer { source })?;
    Ok(format!("d|{body}"))
}

#[cfg(test)]
mod tests {
    use super::{
        RsGraph, compute_fingerprint, display_fp, dirty_state, edge_fp_line, node_fp_line,
    };
    use crate::model::{
        Author, Camera, CaseDisplay, DEFAULT_NODE_SIZE, Entity, EntityKind, Facet, FacetValue,
        Occupation, OccupationIcon, Origin, Relation, RelationKind, Verification,
    };
    use egui::Pos2;
    use egui_graphs::Graph;
    use petgraph::stable_graph::StableGraph;

    fn bare_person() -> Entity {
        Entity {
            slug: "p".into(),
            kind: EntityKind::Person,
            label: "P".into(),
            value: String::new(),
            sealed: false,
            observations: Vec::new(),
            facets: Default::default(),
            note: String::new(),
            image: None,
            size: DEFAULT_NODE_SIZE,
            boost: 1.0,
            collapsed: false,
            hidden: false,
        }
    }

    fn person(slug: &str) -> Entity {
        Entity {
            slug: slug.into(),
            ..bare_person()
        }
    }

    fn relation(confidence: u8, status: Verification) -> Relation {
        Relation {
            kind: RelationKind::Controls,
            origin: Origin::Manual,
            status,
            confidence,
            author: Author::default(),
            created_at: 0,
            sources: Vec::new(),
            hidden: false,
        }
    }

    fn tiny_graph(confidence: u8, status: Verification) -> RsGraph {
        let mut g: RsGraph = Graph::from(&StableGraph::default());
        let a = g.add_node_with_label_and_location(person("a"), "a".into(), Pos2::new(0.0, 0.0));
        let b = g.add_node_with_label_and_location(person("b"), "b".into(), Pos2::new(20.0, 0.0));
        g.add_edge_with_label(a, b, relation(confidence, status), "rel".into());
        g
    }

    #[test]
    fn occupation_changes_node_fingerprint() {
        let base = bare_person();
        let mut judge = base.clone();
        judge.facets.insert(
            Facet::Occupation,
            FacetValue::Freeform(Occupation {
                label: "Judge".into(),
                icon: OccupationIcon::Scales,
            }),
        );
        assert_ne!(
            node_fp_line(&base).unwrap(),
            node_fp_line(&judge).unwrap(),
            "setting an occupation must change the dirty fingerprint"
        );

        let mut lawyer = base.clone();
        lawyer.facets.insert(
            Facet::Occupation,
            FacetValue::Freeform(Occupation {
                label: "Judge".into(),
                icon: OccupationIcon::Briefcase,
            }),
        );
        assert_ne!(
            node_fp_line(&judge).unwrap(),
            node_fp_line(&lawyer).unwrap(),
            "changing only the occupation icon must change the fingerprint"
        );
    }

    #[test]
    fn edge_confidence_change_flips_dirty() {
        let saved = edge_fp_line("a", "b", &relation(50, Verification::Unreviewed)).unwrap();
        let bumped = edge_fp_line("a", "b", &relation(80, Verification::Unreviewed)).unwrap();
        assert_ne!(saved, bumped, "bumping edge confidence must change the fingerprint");
        assert!(
            dirty_state(true, &bumped, &saved),
            "an edge confidence change must mark the case dirty"
        );
    }

    #[test]
    fn edge_verification_change_flips_dirty() {
        let saved = edge_fp_line("a", "b", &relation(80, Verification::Unreviewed)).unwrap();
        let verified = edge_fp_line("a", "b", &relation(80, Verification::Corroborated)).unwrap();
        assert_ne!(saved, verified, "re-verifying an edge must change the fingerprint");
        assert!(
            dirty_state(true, &verified, &saved),
            "an edge verification change must mark the case dirty"
        );
    }

    #[test]
    fn edge_source_provenance_change_flips_dirty() {
        use crate::model::{Source, SourceKind, SourceTier};
        let mut base = relation(80, Verification::Unreviewed);
        base.sources.push(Source {
            kind: SourceKind::Document,
            reference: "evidence/docs/whois.txt".into(),
            ..Default::default()
        });
        let saved = edge_fp_line("a", "b", &base).unwrap();

        let mut hashed = base.clone();
        hashed.sources[0].sha256 = Some("deadbeef".repeat(8));
        let after_hash = edge_fp_line("a", "b", &hashed).unwrap();
        assert_ne!(saved, after_hash, "attaching a source sha256 must change the fingerprint");
        assert!(
            dirty_state(true, &after_hash, &saved),
            "a source sha256 change must mark the case dirty"
        );

        let mut rated = base.clone();
        rated.sources[0].tier = SourceTier::Primary;
        let after_tier = edge_fp_line("a", "b", &rated).unwrap();
        assert_ne!(saved, after_tier, "grading a source's tier must change the fingerprint");
        assert!(
            dirty_state(true, &after_tier, &saved),
            "a source tier change must mark the case dirty"
        );
    }

    #[test]
    fn camera_change_leaves_fingerprint_stable() {
        let mut d = CaseDisplay::default();
        d.positions.insert("p".into(), (4.0, 9.0));
        let before = display_fp(&d).unwrap();
        d.camera = Camera {
            x: 999.0,
            y: -999.0,
            zoom: 7.5,
        };
        let after = display_fp(&d).unwrap();
        assert_eq!(before, after, "panning/zooming must not enter the fingerprint");
        assert!(
            !dirty_state(true, &after, &before),
            "a camera-only change must never mark the case dirty"
        );

        d.positions.insert("p".into(), (5.0, 9.0));
        let moved = display_fp(&d).unwrap();
        assert_ne!(before, moved, "moving a node must change the fingerprint");
    }

    #[test]
    fn full_fingerprint_round_trip_and_edge_flip() {
        let saved =
            compute_fingerprint(&tiny_graph(50, Verification::Unreviewed), "case", false).unwrap();
        let reloaded =
            compute_fingerprint(&tiny_graph(50, Verification::Unreviewed), "case", false).unwrap();
        assert!(
            !dirty_state(true, &reloaded, &saved),
            "an identical save→load rebuild must not be dirty"
        );

        let rebumped =
            compute_fingerprint(&tiny_graph(80, Verification::Unreviewed), "case", false).unwrap();
        assert!(
            dirty_state(true, &rebumped, &saved),
            "edge confidence must flip dirty through the full fingerprint"
        );

        let reverified =
            compute_fingerprint(&tiny_graph(50, Verification::Corroborated), "case", false).unwrap();
        assert!(
            dirty_state(true, &reverified, &saved),
            "edge verification must flip dirty through the full fingerprint"
        );
    }

    #[test]
    fn blank_start_is_never_dirty() {
        assert!(!dirty_state(false, "untitled case|false", ""));
        assert!(!dirty_state(false, "untitled case|false", "untitled case|false"));
        assert!(!dirty_state(false, "person built but unsaved", "anything else"));
    }

    #[test]
    fn open_case_dirties_only_on_divergence() {
        assert!(dirty_state(true, "edited fingerprint", "saved fingerprint"));
        assert!(!dirty_state(true, "same fingerprint", "same fingerprint"));
    }
}

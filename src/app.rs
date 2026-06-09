//! The redstring board: three panels around an `egui_graphs` graph view.
//! Left = case I/O + add entity/relation + unsourced audit. Right = inspector
//! for the selected entity or relation (this is where sources get attached).
//!
//! Persistence is one RON `CaseFile` with a `data`/`display` split (see
//! `model.rs`). The file is watched: an external edit (e.g. an AI appending
//! entities/relations to `data`) hot-reloads the board.

use std::collections::HashMap;
use std::time::{Instant, SystemTime};

use eframe::App;
use egui::{Color32, Pos2, RichText, Vec2};
use egui_graphs::{
    Graph, GraphView, MetadataFrame, SettingsInteraction, SettingsNavigation, SettingsStyle,
};
use petgraph::graph::{DefaultIx, EdgeIndex, NodeIndex};
use petgraph::stable_graph::StableGraph;
use petgraph::Directed;

use crate::edge_shape::SourcedEdgeShape;
use crate::model::*;
use crate::node_shape::EntityNodeShape;

pub type RsGraph = Graph<Entity, Relation, Directed, DefaultIx, EntityNodeShape, SourcedEdgeShape>;

const OK_GREEN: Color32 = Color32::from_rgb(120, 200, 140);
const ALARM_RED: Color32 = Color32::from_rgb(235, 80, 80);

/// Stable id for the board's egui_graphs metadata (camera zoom/pan lives there).
const BOARD_META_ID: &str = "redstring_board";

pub struct RedstringApp {
    g: RsGraph,
    case_path: String,
    status: String,

    /// (path, mtime) of the file as we last wrote/read it — the hot-reload watch.
    watched: Option<(String, SystemTime)>,
    last_watch_poll: Instant,
    /// Load `case_path` on the first frame (camera restore needs a live `ui`).
    pending_load: bool,

    // add-entity form
    ne_kind: EntityKind,
    ne_label: String,

    // add-relation form
    nr_from: Option<NodeIndex>,
    nr_to: Option<NodeIndex>,
    nr_kind: RelationKind,

    // add-source form (targets the selected relation)
    ns_kind: SourceKind,
    ns_ref: String,
    ns_excerpt: String,
}

impl RedstringApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let mut app = Self {
            g: Graph::from(&StableGraph::default()),
            case_path: "case.ron".to_owned(),
            status: "new case".to_owned(),
            watched: None,
            last_watch_poll: Instant::now(),
            pending_load: false,
            ne_kind: EntityKind::Person,
            ne_label: String::new(),
            nr_from: None,
            nr_to: None,
            nr_kind: RelationKind::RelatedTo,
            ns_kind: SourceKind::Markdown,
            ns_ref: String::new(),
            ns_excerpt: String::new(),
        };
        // a case file sitting in cwd wins over the demo seed
        if std::path::Path::new(&app.case_path).exists() {
            app.pending_load = true;
        } else {
            app.seed_demo();
        }
        app
    }

    /// A tiny starter case so the board demonstrates the sourced/unsourced split
    /// on first launch.
    fn seed_demo(&mut self) {
        let doe = self.add_entity(EntityKind::Person, "J. Doe".into(), spiral(0));
        let shell = self.add_entity(EntityKind::Org, "Shell Co Ltd".into(), spiral(1));
        let domain = self.add_entity(EntityKind::Domain, "evil.example".into(), spiral(2));

        // sourced link → solid green
        let mut controls = Relation::new(RelationKind::Controls);
        controls.sources.push(Source {
            kind: SourceKind::Markdown,
            reference: "dossier.md#ownership".into(),
            excerpt: "Doe is sole signatory per incorporation docs.".into(),
        });
        self.g
            .add_edge_with_label(doe, shell, controls, RelationKind::Controls.label().into());

        // unsourced link → red + dashed (the thing you still have to prove)
        self.g.add_edge_with_label(
            shell,
            domain,
            Relation::new(RelationKind::RegisteredTo),
            RelationKind::RegisteredTo.label().into(),
        );

        self.status = "demo case — edit freely or Load your own".into();
    }

    /// Slug an entity label: lowercase alnum, runs of anything else collapse
    /// to `_`. Deduped against the current graph with numeric suffixes.
    fn unique_slug(&self, label: &str) -> String {
        let mut base: String = label
            .to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect::<String>()
            .split('_')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("_");
        if base.is_empty() {
            base = "entity".to_owned();
        }
        if !self.slug_taken(&base) {
            return base;
        }
        let mut n = 2;
        loop {
            let candidate = format!("{base}_{n}");
            if !self.slug_taken(&candidate) {
                return candidate;
            }
            n += 1;
        }
    }

    fn slug_taken(&self, slug: &str) -> bool {
        self.g
            .g()
            .node_weights()
            .any(|n| n.payload().slug == slug)
    }

    fn add_entity(&mut self, kind: EntityKind, label: String, pos: Pos2) -> NodeIndex {
        let ent = Entity {
            slug: self.unique_slug(&label),
            kind,
            label: label.clone(),
            attrs: Vec::new(),
            note: String::new(),
            image: None,
        };
        self.g
            .add_node_with_label_and_location(ent, format!("{} {}", kind.icon(), label), pos)
    }

    /// Push payload state (colour + label) onto the display nodes/edges each frame.
    fn sync_visuals(&mut self) {
        let nodes: Vec<NodeIndex> = self.g.g().node_indices().collect();
        for idx in nodes {
            let Some(n) = self.g.node(idx) else { continue };
            let e = n.payload();
            let color = e.kind.color();
            let label = format!("{} {}", e.kind.icon(), e.label);
            if let Some(n) = self.g.node_mut(idx) {
                n.set_color(color);
                n.set_label(label);
            }
        }

        let edges: Vec<EdgeIndex> = self.g.g().edge_indices().collect();
        for idx in edges {
            let Some(ed) = self.g.edge(idx) else { continue };
            let r = ed.payload();
            let tag = if r.is_sourced() {
                format!("[{}]", r.sources.len())
            } else {
                "⚠0".to_owned()
            };
            let label = format!("{} {}", r.kind.label(), tag);
            if let Some(ed) = self.g.edge_mut(idx) {
                ed.set_label(label);
            }
        }
    }

    fn stats(&self) -> (usize, usize, usize) {
        let g = self.g.g();
        let unsourced = g
            .edge_indices()
            .filter(|&e| {
                g.edge_weight(e)
                    .map(|w| !w.payload().is_sourced())
                    .unwrap_or(false)
            })
            .count();
        (g.node_count(), g.edge_count(), unsourced)
    }

    fn node_label(&self, idx: NodeIndex) -> String {
        self.g
            .g()
            .node_weight(idx)
            .map(|n| {
                let e = n.payload();
                format!("{} {}", e.kind.icon(), e.label)
            })
            .unwrap_or_else(|| "?".to_owned())
    }

    fn edge_endpoints_labels(&self, idx: EdgeIndex) -> (String, String) {
        match self.g.g().edge_endpoints(idx) {
            Some((a, b)) => (self.node_label(a), self.node_label(b)),
            None => ("?".to_owned(), "?".to_owned()),
        }
    }

    // ---- persistence ----

    fn camera_read(ui: &egui::Ui) -> Camera {
        let mf = MetadataFrame::new(Some(BOARD_META_ID.to_owned())).load(ui);
        Camera {
            x: mf.pan.x,
            y: mf.pan.y,
            zoom: mf.zoom,
        }
    }

    fn camera_write(ui: &mut egui::Ui, cam: Camera) {
        let mut mf = MetadataFrame::new(Some(BOARD_META_ID.to_owned())).load(ui);
        mf.pan = Vec2::new(cam.x, cam.y);
        mf.zoom = cam.zoom;
        mf.save(ui);
    }

    fn to_case_file(&self, ui: &egui::Ui) -> CaseFile {
        let g = self.g.g();
        let mut cf = CaseFile::default();
        for idx in g.node_indices() {
            let n = g.node_weight(idx).unwrap();
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
        }
        for idx in g.edge_indices() {
            let (a, b) = g.edge_endpoints(idx).unwrap();
            let r = g.edge_weight(idx).unwrap().payload();
            cf.data.relations.push(RelationRec {
                from: g.node_weight(a).unwrap().payload().slug.clone(),
                to: g.node_weight(b).unwrap().payload().slug.clone(),
                kind: r.kind,
                sources: r.sources.clone(),
            });
        }
        cf.display.camera = Self::camera_read(ui);
        cf
    }

    fn save(&mut self, ui: &egui::Ui) {
        let cf = self.to_case_file(ui);
        let cfg = ron::ser::PrettyConfig::default();
        self.status = match ron::ser::to_string_pretty(&cf, cfg) {
            Ok(s) => match std::fs::write(&self.case_path, s) {
                Ok(_) => {
                    self.watch_current();
                    format!(
                        "saved {} entities / {} links → {}",
                        cf.data.entities.len(),
                        cf.data.relations.len(),
                        self.case_path
                    )
                }
                Err(e) => format!("save failed: {e}"),
            },
            Err(e) => format!("serialize failed: {e}"),
        };
    }

    fn load(&mut self, ui: &mut egui::Ui) {
        let s = match std::fs::read_to_string(&self.case_path) {
            Ok(s) => s,
            Err(e) => {
                self.status = format!("load failed: {e}");
                return;
            }
        };
        let cf: CaseFile = match ron::from_str(&s) {
            Ok(c) => c,
            Err(e) => {
                self.status = format!("parse failed: {e}");
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
            // display is optional: position from file, or heal with a spiral slot
            let pos = match cf.display.positions.get(&er.slug) {
                Some(&(x, y)) => Pos2::new(x, y),
                None => {
                    healed += 1;
                    spiral(i)
                }
            };
            let ent = Entity {
                slug: er.slug.clone(),
                kind: er.kind,
                label: er.label.clone(),
                attrs: er.attrs.clone(),
                note: er.note.clone(),
                image: er.image.clone(),
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
            match (by_slug.get(&rr.from), by_slug.get(&rr.to)) {
                (Some(&a), Some(&b)) => {
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
                _ => skipped.push(format!("{} -> {}", rr.from, rr.to)),
            }
        }

        self.g = g;
        self.nr_from = None;
        self.nr_to = None;
        Self::camera_write(ui, cf.display.camera);
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

    /// Remember the current path+mtime as the hot-reload baseline.
    fn watch_current(&mut self) {
        self.watched = std::fs::metadata(&self.case_path)
            .and_then(|m| m.modified())
            .ok()
            .map(|t| (self.case_path.clone(), t));
    }

    /// Hot-reload: if the watched file changed under us (external editor, AI
    /// appending to the case), pull it in. Polled at most ~1/s.
    fn poll_external_edit(&mut self, ui: &mut egui::Ui) {
        if self.last_watch_poll.elapsed().as_millis() < 1000 {
            return;
        }
        self.last_watch_poll = Instant::now();
        let Some((path, seen)) = &self.watched else {
            return;
        };
        if *path != self.case_path {
            return; // user retargeted the path field; don't auto-load
        }
        let Ok(mtime) = std::fs::metadata(path).and_then(|m| m.modified()) else {
            return;
        };
        if mtime > *seen {
            self.load(ui);
            self.status = format!("external edit detected — reloaded {}", self.case_path);
        }
        // request a wakeup so the watcher ticks even when the user is idle
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(1100));
    }

    // ---- panels ----

    fn left_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("redstring");
        ui.label(RichText::new("evidentiary investigation board").weak().small());
        ui.separator();

        ui.horizontal(|ui| {
            ui.label("file");
            ui.text_edit_singleline(&mut self.case_path);
        });
        ui.horizontal(|ui| {
            if ui.button("💾 Save").clicked() {
                self.save(ui);
            }
            if ui.button("📂 Load").clicked() {
                self.load(ui);
            }
        });
        ui.separator();

        // add entity
        ui.label(RichText::new("Add entity").strong());
        egui::ComboBox::from_id_salt("ne_kind")
            .selected_text(format!("{} {}", self.ne_kind.icon(), self.ne_kind.label()))
            .show_ui(ui, |ui| {
                for k in EntityKind::ALL {
                    ui.selectable_value(
                        &mut self.ne_kind,
                        k,
                        format!("{} {}", k.icon(), k.label()),
                    );
                }
            });
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut self.ne_label);
            let go = ui.button("+ entity").clicked();
            if go && !self.ne_label.trim().is_empty() {
                let pos = spiral(self.g.g().node_count());
                let (kind, label) = (self.ne_kind, self.ne_label.trim().to_owned());
                self.add_entity(kind, label, pos);
                self.ne_label.clear();
            }
        });
        ui.separator();

        // add relation
        ui.label(RichText::new("Add relation").strong());
        let entities: Vec<(NodeIndex, String)> = self
            .g
            .g()
            .node_indices()
            .map(|i| (i, self.node_label(i)))
            .collect();
        entity_combo(ui, "nr_from", "from", &mut self.nr_from, &entities);
        entity_combo(ui, "nr_to", "to", &mut self.nr_to, &entities);
        egui::ComboBox::from_id_salt("nr_kind")
            .selected_text(self.nr_kind.label())
            .show_ui(ui, |ui| {
                for k in RelationKind::ALL {
                    ui.selectable_value(&mut self.nr_kind, k, k.label());
                }
            });
        if ui.button("+ relation").clicked() {
            match (self.nr_from, self.nr_to) {
                (Some(a), Some(b)) if a != b => {
                    self.g.add_edge_with_label(
                        a,
                        b,
                        Relation::new(self.nr_kind),
                        self.nr_kind.label().into(),
                    );
                    self.status = "added relation (unsourced — attach a source!)".into();
                }
                _ => self.status = "pick two distinct entities".into(),
            }
        }
        ui.separator();

        // stats + audit
        let (ne, nr, unsourced) = self.stats();
        ui.label(format!("entities {ne}  ·  links {nr}"));
        let audit_color = if unsourced > 0 { ALARM_RED } else { OK_GREEN };
        ui.colored_label(audit_color, format!("⚠ unsourced links: {unsourced}"));
        egui::CollapsingHeader::new("unsourced audit")
            .default_open(true)
            .show(ui, |ui| {
                let g = self.g.g();
                let mut any = false;
                for idx in g.edge_indices() {
                    let unproven = g
                        .edge_weight(idx)
                        .map(|w| !w.payload().is_sourced())
                        .unwrap_or(false);
                    if !unproven {
                        continue;
                    }
                    any = true;
                    let kind = g.edge_weight(idx).unwrap().payload().kind.label();
                    let (a, b) = self.edge_endpoints_labels(idx);
                    ui.colored_label(ALARM_RED, format!("{a}  —{kind}→  {b}"));
                }
                if !any {
                    ui.colored_label(OK_GREEN, "every link is sourced ✓");
                }
            });
    }

    fn inspector(&mut self, ui: &mut egui::Ui) {
        ui.heading("inspector");
        ui.separator();

        if let Some(&nidx) = self.g.selected_nodes().first() {
            self.node_inspector(ui, nidx);
        } else if let Some(&eidx) = self.g.selected_edges().first() {
            self.edge_inspector(ui, eidx);
        } else {
            ui.label(RichText::new("select an entity or relation").weak());
            ui.add_space(8.0);
            ui.label("• drag to move · scroll to zoom");
            ui.label("• red dashed link = unsourced");
            ui.label("• green link = has ≥1 source");
        }
    }

    fn node_inspector(&mut self, ui: &mut egui::Ui, nidx: NodeIndex) {
        // slug edit happens against a copy so we can dedupe against the graph
        let mut slug_edit: Option<String> = None;
        let mut delete = false;

        if let Some(node) = self.g.node_mut(nidx) {
            let e = node.payload_mut();
            ui.label(RichText::new(format!("{} {}", e.kind.icon(), e.kind.label())).strong());

            egui::ComboBox::from_id_salt("insp_kind")
                .selected_text(format!("{} {}", e.kind.icon(), e.kind.label()))
                .show_ui(ui, |ui| {
                    for k in EntityKind::ALL {
                        ui.selectable_value(&mut e.kind, k, format!("{} {}", k.icon(), k.label()));
                    }
                });

            ui.horizontal(|ui| {
                ui.label("label");
                ui.text_edit_singleline(&mut e.label);
            });

            let mut slug = e.slug.clone();
            ui.horizontal(|ui| {
                ui.label("slug");
                if ui.text_edit_singleline(&mut slug).changed() {
                    slug_edit = Some(slug.clone());
                }
            });
            ui.label(
                RichText::new("the handle relations & external edits reference")
                    .weak()
                    .small(),
            );

            ui.add_space(6.0);
            ui.label(RichText::new("image").strong());
            ui.horizontal(|ui| {
                if ui.button("🖼 image…").clicked() {
                    // RECONCILE(chrome): chrome adds the same `rfd` dep line
                    // for the casefile picker — identical blocking-dialog use.
                    if let Some(p) = rfd::FileDialog::new()
                        .add_filter("image", &["png", "jpg", "jpeg", "webp", "gif"])
                        .pick_file()
                    {
                        let p = p.to_string_lossy().into_owned();
                        // re-picking a path must reflect the file as it is now
                        crate::node_shape::invalidate_texture(ui.ctx(), &p);
                        e.image = Some(p);
                    }
                }
                if e.image.is_some() && ui.button("✕").clicked() {
                    e.image = None;
                }
            });
            if let Some(p) = &e.image {
                ui.label(RichText::new(p.as_str()).weak().small());
            }

            ui.add_space(6.0);
            ui.label(RichText::new("attributes").strong());
            let mut remove = None;
            for (i, (k, v)) in e.attrs.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(k).desired_width(80.0).hint_text("key"));
                    ui.add(egui::TextEdit::singleline(v).hint_text("value"));
                    if ui.button("✕").clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                e.attrs.remove(i);
            }
            if ui.button("+ attr").clicked() {
                e.attrs.push((String::new(), String::new()));
            }

            ui.add_space(6.0);
            ui.label(RichText::new("note").strong());
            ui.text_edit_multiline(&mut e.note);

            ui.separator();
            if ui.button("🗑 delete entity").clicked() {
                delete = true;
            }
        }

        // apply slug edit with uniqueness check (outside the node_mut borrow)
        if let Some(new_slug) = slug_edit {
            let clean: String = new_slug
                .to_lowercase()
                .chars()
                .map(|c| if c.is_alphanumeric() { c } else { '_' })
                .collect();
            let taken = self.g.g().node_indices().any(|i| {
                i != nidx
                    && self
                        .g
                        .g()
                        .node_weight(i)
                        .map(|n| n.payload().slug == clean)
                        .unwrap_or(false)
            });
            if !clean.is_empty() && !taken {
                if let Some(node) = self.g.node_mut(nidx) {
                    node.payload_mut().slug = clean;
                }
            }
        }

        if delete {
            self.g.remove_node(nidx);
        }
    }

    fn edge_inspector(&mut self, ui: &mut egui::Ui, eidx: EdgeIndex) {
        let (from_l, to_l) = self.edge_endpoints_labels(eidx);
        let mut delete = false;

        // edit kind + existing sources
        if let Some(edge) = self.g.edge_mut(eidx) {
            let r = edge.payload_mut();
            ui.label(
                RichText::new(format!("{from_l}  —{}→  {to_l}", r.kind.label())).strong(),
            );

            egui::ComboBox::from_id_salt("insp_rel_kind")
                .selected_text(r.kind.label())
                .show_ui(ui, |ui| {
                    for k in RelationKind::ALL {
                        ui.selectable_value(&mut r.kind, k, k.label());
                    }
                });

            if r.is_sourced() {
                ui.colored_label(OK_GREEN, format!("✓ {} source(s)", r.sources.len()));
            } else {
                ui.colored_label(ALARM_RED, "⚠ UNSOURCED — unproven claim");
            }

            ui.separator();
            ui.label(RichText::new("sources").strong());
            let mut remove = None;
            for (i, src) in r.sources.iter_mut().enumerate() {
                ui.group(|ui| {
                    egui::ComboBox::from_id_salt(("src_kind", i))
                        .selected_text(src.kind.label())
                        .show_ui(ui, |ui| {
                            for k in SourceKind::ALL {
                                ui.selectable_value(&mut src.kind, k, k.label());
                            }
                        });
                    ui.add(
                        egui::TextEdit::singleline(&mut src.reference)
                            .hint_text(src.kind.hint()),
                    );
                    ui.add(
                        egui::TextEdit::multiline(&mut src.excerpt)
                            .hint_text("excerpt / quote (optional)")
                            .desired_rows(2),
                    );
                    if ui.button("✕ remove source").clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                r.sources.remove(i);
            }
        }

        // add-source form (kept out of the edge_mut borrow above)
        ui.separator();
        ui.label(RichText::new("attach source").strong());
        egui::ComboBox::from_id_salt("ns_kind")
            .selected_text(self.ns_kind.label())
            .show_ui(ui, |ui| {
                for k in SourceKind::ALL {
                    ui.selectable_value(&mut self.ns_kind, k, k.label());
                }
            });
        ui.add(egui::TextEdit::singleline(&mut self.ns_ref).hint_text(self.ns_kind.hint()));
        ui.add(
            egui::TextEdit::multiline(&mut self.ns_excerpt)
                .hint_text("excerpt / quote (optional)")
                .desired_rows(2),
        );
        if ui.button("+ source").clicked() && !self.ns_ref.trim().is_empty() {
            if let Some(edge) = self.g.edge_mut(eidx) {
                edge.payload_mut().sources.push(Source {
                    kind: self.ns_kind,
                    reference: self.ns_ref.trim().to_owned(),
                    excerpt: self.ns_excerpt.trim().to_owned(),
                });
            }
            self.ns_ref.clear();
            self.ns_excerpt.clear();
            self.status = "source attached".into();
        }

        ui.separator();
        if ui.button("🗑 delete relation").clicked() {
            delete = true;
        }
        if delete {
            self.g.remove_edge(eidx);
        }
    }
}

impl App for RedstringApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if self.pending_load {
            self.pending_load = false;
            self.load(ui);
        }
        self.sync_visuals();
        self.poll_external_edit(ui);

        egui::Panel::left("left")
            .resizable(true)
            .default_size(270.0)
            .show_inside(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.left_panel(ui));
            });
        egui::Panel::right("right")
            .resizable(true)
            .default_size(310.0)
            .show_inside(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.inspector(ui));
            });
        egui::Panel::bottom("status").show_inside(ui, |ui| {
            ui.label(&self.status);
        });
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.add(
                &mut GraphView::<_, _, _, _, EntityNodeShape, SourcedEdgeShape>::new(&mut self.g)
                    .with_id(Some(BOARD_META_ID.to_owned()))
                    .with_interactions(
                        &SettingsInteraction::default()
                            .with_dragging_enabled(true)
                            .with_node_selection_enabled(true)
                            .with_edge_selection_enabled(true),
                    )
                    .with_navigations(
                        &SettingsNavigation::default()
                            .with_fit_to_screen_enabled(false)
                            .with_zoom_and_pan_enabled(true),
                    )
                    .with_styles(&SettingsStyle::default().with_labels_always(true)),
            );
        });
    }
}

/// Golden-angle spiral so seeded / freshly-added / healed nodes don't stack.
fn spiral(n: usize) -> Pos2 {
    let a = n as f32 * 2.399_963_2;
    let r = 40.0 + 16.0 * n as f32;
    Pos2::new(r * a.cos(), r * a.sin())
}

fn entity_combo(
    ui: &mut egui::Ui,
    id: &str,
    label: &str,
    target: &mut Option<NodeIndex>,
    entities: &[(NodeIndex, String)],
) {
    let selected = target
        .and_then(|sel| entities.iter().find(|(i, _)| *i == sel))
        .map(|(_, l)| l.clone())
        .unwrap_or_else(|| format!("{label}…"));
    egui::ComboBox::from_id_salt(id)
        .selected_text(selected)
        .show_ui(ui, |ui| {
            for (i, l) in entities {
                ui.selectable_value(target, Some(*i), l);
            }
        });
}

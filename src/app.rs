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
use egui::{Color32, Margin, Pos2, Rect, RichText, Shape, Stroke, Vec2};
use egui_graphs::{
    Graph, GraphView, MetadataFrame, SettingsInteraction, SettingsNavigation, SettingsStyle,
};
use petgraph::graph::{DefaultIx, EdgeIndex, NodeIndex};
use petgraph::stable_graph::StableGraph;
use petgraph::Directed;

use crate::edge_shape::SourcedEdgeShape;
use crate::model::*;
use crate::node_shape::EntityNodeShape;
use crate::prefs::Prefs;

pub type RsGraph = Graph<Entity, Relation, Directed, DefaultIx, EntityNodeShape, SourcedEdgeShape>;

const OK_GREEN: Color32 = Color32::from_rgb(120, 200, 140);
const ALARM_RED: Color32 = Color32::from_rgb(235, 80, 80);

/// Stable id for the board's egui_graphs metadata (camera zoom/pan lives there).
const BOARD_META_ID: &str = "redstring_board";

/// Plain-wheel zoom feel: factor = exp(scroll_points * speed). Mirrors egui's
/// own ctrl+scroll mapping (`InputOptions::scroll_zoom_speed` = 1/200), so an
/// unmodified wheel notch (40 pt) zooms ~1.22x and trackpad scroll stays
/// smooth + proportional.
const SCROLL_ZOOM_SPEED: f32 = 1.0 / 200.0;

pub struct RedstringApp {
    g: RsGraph,
    case_path: String,
    status: String,
    prefs: Prefs,

    /// (path, mtime) of the file as we last wrote/read it — the hot-reload watch.
    watched: Option<(String, SystemTime)>,
    last_watch_poll: Instant,
    /// Load `case_path` on the first frame (camera restore needs a live `ui`).
    pending_load: bool,
    /// Camera from the last `load()`, re-asserted once AFTER the `GraphView`
    /// ran: the widget force-fits on its first frame even with fit-to-screen
    /// disabled, stomping a camera written before it (see `ui()`).
    pending_camera: Option<Camera>,

    // add-entity form
    ne_kind: EntityKind,
    ne_label: String,

    // add-relation form
    nr_from: Option<NodeIndex>,
    nr_to: Option<NodeIndex>,
    nr_kind: RelationKind,

    // right-drag edge creation: press on A → rubber-band → release on B →
    // verb-picker popup. `pending_edge` is (from, to, popup screen pos).
    edge_drag: Option<NodeIndex>,
    pending_edge: Option<(NodeIndex, NodeIndex, Pos2)>,
    verb_filter: String,
    verb_filter_focus: bool,

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
            prefs: Prefs::load(),
            watched: None,
            last_watch_poll: Instant::now(),
            pending_load: false,
            pending_camera: None,
            ne_kind: EntityKind::Person,
            ne_label: String::new(),
            nr_from: None,
            nr_to: None,
            nr_kind: RelationKind::RelatedTo,
            edge_drag: None,
            pending_edge: None,
            verb_filter: String::new(),
            verb_filter_focus: false,
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
    // RECONCILE(relations): seed assumes RelationKind::{Controls, RegisteredTo,
    // Financed} survive the relation-vocabulary explosion; swap labels here if
    // they get renamed.
    fn seed_demo(&mut self) {
        let doe = self.add_entity(EntityKind::Person, "J. Doe".into(), spiral(0));
        let shell = self.add_entity(EntityKind::ShellCompany, "Shell Co Ltd".into(), spiral(1));
        let domain = self.add_entity(EntityKind::Domain, "evil.example".into(), spiral(2));
        let minister = self.add_entity(EntityKind::Official, "Minister X".into(), spiral(3));

        // sourced link → solid green
        let mut controls = Relation::new(RelationKind::Controls);
        controls.sources.push(Source {
            kind: SourceKind::Markdown,
            reference: "dossier.md#ownership".into(),
            excerpt: "Doe is sole signatory per incorporation docs.".into(),
        });
        self.g
            .add_edge_with_label(doe, shell, controls, RelationKind::Controls.label().into());

        // unsourced links → red + dashed (the things you still have to prove)
        self.g.add_edge_with_label(
            shell,
            domain,
            Relation::new(RelationKind::OwnsDomain),
            RelationKind::OwnsDomain.label().into(),
        );
        self.g.add_edge_with_label(
            shell,
            minister,
            Relation::new(RelationKind::Financed),
            RelationKind::Financed.label().into(),
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
            // pre-fill the kind's predictable attribute keys (empty values) so
            // a fresh entity shows its expected fields immediately; "+ attr"
            // still adds free-form pairs on top. Loaded files are never
            // prefilled — the file is the truth.
            attrs: kind
                .default_attrs()
                .iter()
                .map(|k| ((*k).to_owned(), String::new()))
                .collect(),
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

    /// Plain mouse-wheel zoom over the board, centred on the pointer.
    ///
    /// egui_graphs 0.30 only zooms on `zoom_delta()` (graph_view.rs
    /// `handle_zoom`), which egui feeds exclusively from ctrl/cmd+scroll and
    /// pinch gestures; an unmodified wheel lands in `smooth_scroll_delta`
    /// instead (`InputState::begin_pass` routes a wheel event to one OR the
    /// other, never both). So driving zoom from `smooth_scroll_delta` here
    /// can't double-apply with the lib, ctrl+scroll and trackpad pinch keep
    /// working through the lib's own path, and the lib's drag-to-pan /
    /// node-drag handling (pointer buttons) is untouched.
    ///
    /// Must run inside the central panel BEFORE the `GraphView` is added:
    /// the widget loads the `MetadataFrame` at the start of its `ui()`, so a
    /// write here is what it draws with this same frame.
    // RECONCILE(edgedrag): this only READS scroll delta + hover pos; no
    // pointer-button/drag state is consumed or altered.
    fn scroll_zoom(ui: &mut egui::Ui) {
        // the GraphView allocates exactly the remaining rect after us
        let board = ui.available_rect_before_wrap();
        let (dy, hover) = ui.input(|i| (i.smooth_scroll_delta.y, i.pointer.hover_pos()));
        if dy == 0.0 {
            return;
        }
        let Some(pointer) = hover else { return };
        if !board.contains(pointer) {
            return; // scrolling the side panels must not zoom the board
        }

        let mut mf = MetadataFrame::new(Some(BOARD_META_ID.to_owned())).load(ui);
        let new_zoom = mf.zoom * (dy * SCROLL_ZOOM_SPEED).exp();
        // pan is widget-local (the lib adds rect.left_top at draw time), so
        // the pointer must be too. Same zoom-about-pointer invariant as
        // GraphView::zoom: pan' = pointer - (pointer - pan) * (z'/z).
        let local = pointer - board.left_top();
        mf.pan = local - (local - mf.pan) * (new_zoom / mf.zoom);
        mf.zoom = new_zoom;
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
        // node indices just got reassigned — abort any in-flight edge gesture
        self.edge_drag = None;
        self.pending_edge = None;
        // Pre-write: the GraphView (added later this same frame) loads the
        // MetadataFrame at its start, so this is the camera it draws with —
        // flash-free for button loads and hot reloads. The widget's very
        // first frame force-fits over it, hence the deferred re-assert.
        Self::camera_write(ui, cf.display.camera);
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

    /// Collapsed sidebar: a thin strip whose only job is the expand toggle.
    fn sidebar_strip(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            if ui.button("⏵").on_hover_text("expand sidebar").clicked() {
                self.prefs.sidebar_collapsed = false;
                self.prefs.save();
            }
        });
    }

    fn left_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("redstring");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("⏴").on_hover_text("collapse sidebar").clicked() {
                    self.prefs.sidebar_collapsed = true;
                    self.prefs.save();
                }
            });
        });
        ui.label(RichText::new("evidentiary investigation board").weak().small());
        ui.separator();

        ui.horizontal(|ui| {
            ui.label("file");
            ui.text_edit_singleline(&mut self.case_path);
        });
        ui.horizontal_wrapped(|ui| {
            if ui.button("💾 Save").clicked() {
                self.save(ui);
            }
            if ui.button("📂 Load").clicked() {
                self.load(ui);
            }
            if ui.button("Open…").clicked()
                && let Some(p) = case_dialog(&self.case_path).pick_file()
            {
                self.case_path = p.display().to_string();
                self.load(ui);
            }
            if ui.button("Save as…").clicked()
                && let Some(mut p) = case_dialog(&self.case_path).save_file()
            {
                if p.extension().is_none() {
                    p.set_extension("ron");
                }
                self.case_path = p.display().to_string();
                self.save(ui);
            }
        });
        ui.separator();

        // add entity
        ui.label(RichText::new("Add entity").strong());
        self.kind_picker(ui);
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
        relation_kind_combo(ui, "nr_kind", &mut self.nr_kind);
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

    /// Add-entity kind picker: a multi-column selectable grid of every
    /// `EntityKind`, in a scroll area resized by dragging the handle under it
    /// (height persisted in prefs).
    // RECONCILE(entities): iterates EntityKind::ALL, so the variant explosion
    // lands for free; once a `category()` accessor exists, chunk per category
    // with a small heading between groups.
    fn kind_picker(&mut self, ui: &mut egui::Ui) {
        const MIN_CELL_W: f32 = 92.0;
        let gap = ui.spacing().item_spacing.x;
        let avail = ui.available_width();
        let cols = (((avail + gap) / (MIN_CELL_W + gap)) as usize).max(1);
        let cell_w = (avail - gap * (cols as f32 - 1.0)) / cols as f32;
        let cell_h = ui.spacing().interact_size.y;

        egui::ScrollArea::vertical()
            .id_salt("ne_kind_grid")
            .max_height(self.prefs.kind_picker_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for row in EntityKind::ALL.chunks(cols) {
                    ui.horizontal(|ui| {
                        for &k in row {
                            let cell = egui::Button::selectable(
                                self.ne_kind == k,
                                format!("{} {}", k.icon(), k.label()),
                            )
                            .truncate();
                            if ui.add_sized([cell_w, cell_h], cell).clicked() {
                                self.ne_kind = k;
                            }
                        }
                    });
                }
            });

        // drag handle under the grid; persist height once per gesture
        let (rect, resp) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 6.0), egui::Sense::drag());
        if resp.hovered() || resp.dragged() {
            ui.set_cursor_icon(egui::CursorIcon::ResizeVertical);
        }
        if resp.dragged() {
            self.prefs.kind_picker_height =
                (self.prefs.kind_picker_height + resp.drag_delta().y).clamp(40.0, 480.0);
        }
        if resp.drag_stopped() {
            self.prefs.save();
        }
        let stroke = if resp.dragged() {
            ui.style().visuals.widgets.active.fg_stroke
        } else if resp.hovered() {
            ui.style().visuals.widgets.hovered.fg_stroke
        } else {
            ui.style().visuals.widgets.noninteractive.bg_stroke
        };
        ui.painter().hline(rect.x_range(), rect.center().y, stroke);
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
            ui.label("• right-drag entity → entity to link");
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
            ui.label(RichText::new(e.kind.category()).weak().small());

            kind_combo(ui, "insp_kind", &mut e.kind);

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

            relation_kind_combo(ui, "insp_rel_kind", &mut r.kind);

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

    // ---- board (graph view + right-drag edge creation) ----

    /// The corkboard: the `GraphView` plus the right-drag link-creation overlay.
    /// RECONCILE(chrome): the GraphView builder lives here now (moved out of
    /// `App::ui`) — settings/prefs wiring belongs in this fn.
    fn board(&mut self, ui: &mut egui::Ui) {
        // the rect the GraphView is about to allocate (it takes all available space)
        let rect = ui.available_rect_before_wrap();

        // Edge-drag input runs BEFORE the GraphView so that on the press frame
        // the view is already built with node-dragging gated off. egui_graphs'
        // handle_node_drag keys off any-button press/drag (is_pointer_button_down_on,
        // dragged()), so an ungated right-press over a node would start a node
        // drag and snap the node to the cursor.
        self.edge_drag_input(ui, rect);
        // plain-wheel zoom (no ctrl needed) — RECONCILE(camera): scroll-only,
        // coexists with the right-drag gesture above
        Self::scroll_zoom(ui);

        ui.add(
            &mut GraphView::<_, _, _, _, _, SourcedEdgeShape>::new(&mut self.g)
                .with_id(Some(BOARD_META_ID.to_owned()))
                .with_interactions(
                    &SettingsInteraction::default()
                        // left-drag node moving, except while a right-drag edge
                        // gesture is in flight (see edge_drag_input)
                        .with_dragging_enabled(self.edge_drag.is_none())
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

        self.edge_drag_overlay(ui, rect);
        self.verb_picker(ui);
    }

    /// Which entity is under an absolute screen position? Camera transform via
    /// the board's `MetadataFrame`, then each node's `DisplayNode::is_inside`.
    /// RECONCILE(images): hit-testing rides the display shape, so textured /
    /// resized nodes keep working without touching this.
    fn node_at_screen(&self, ui: &egui::Ui, rect: Rect, screen: Pos2) -> Option<NodeIndex> {
        let mf = MetadataFrame::new(Some(BOARD_META_ID.to_owned())).load(ui);
        self.g
            .node_by_screen_pos(&mf, (screen - rect.left_top()).to_pos2())
    }

    /// Right-button press on an entity starts an edge drag; release on another
    /// entity opens the verb picker; release anywhere else cancels.
    ///
    /// RECONCILE(camera): everything here is scoped to the secondary button —
    /// scroll-zoom and primary/middle-drag pan stay with egui_graphs.
    fn edge_drag_input(&mut self, ui: &egui::Ui, rect: Rect) {
        let (pressed, released, down, pointer) = ui.input(|i| {
            (
                i.pointer.secondary_pressed(),
                i.pointer.secondary_released(),
                i.pointer.secondary_down(),
                i.pointer.interact_pos(),
            )
        });

        if pressed && self.pending_edge.is_none() && self.g.dragged_node().is_none() {
            if let Some(p) = pointer.filter(|p| rect.contains(*p)) {
                self.edge_drag = self.node_at_screen(ui, rect, p);
                if self.edge_drag.is_some() {
                    self.status = "release on a target entity to link".into();
                }
            }
        }

        let Some(from) = self.edge_drag else { return };

        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.edge_drag = None;
            self.status = "link cancelled".into();
            return;
        }

        if released {
            self.edge_drag = None;
            let to = pointer
                .filter(|p| rect.contains(*p))
                .and_then(|p| self.node_at_screen(ui, rect, p));
            match (to, pointer) {
                (Some(to), Some(p)) if to != from => {
                    self.verb_filter.clear();
                    self.verb_filter_focus = true;
                    self.pending_edge = Some((from, to, p));
                }
                _ => self.status = "link cancelled".into(),
            }
        } else if !down {
            // release never arrived (pointer left the window) — abort
            self.edge_drag = None;
        }
    }

    /// Rubber-band string from the drag source to the pointer, drawn over the
    /// `GraphView` in the unsourced style — that's what the new link will be.
    fn edge_drag_overlay(&mut self, ui: &egui::Ui, rect: Rect) {
        let Some(from) = self.edge_drag else { return };
        let Some(node) = self.g.node(from) else {
            self.edge_drag = None; // entity vanished mid-drag (delete / hot-reload)
            return;
        };
        let Some(pointer) = ui.input(|i| i.pointer.latest_pos()) else {
            return;
        };

        let mf = MetadataFrame::new(Some(BOARD_META_ID.to_owned())).load(ui);
        let a = mf.canvas_to_screen_pos(node.location()) + rect.left_top().to_vec2();
        let stroke = Stroke::new(2.0_f32, ALARM_RED);
        let painter = ui.painter().with_clip_rect(rect);
        painter.extend(Shape::dashed_line(&[a, pointer], stroke, 9.0, 6.0));
        painter.circle_filled(pointer, 3.5, ALARM_RED);
    }

    /// Verb picker for a just-dropped edge: filter box on top, the kinds below.
    /// Esc / click-away cancels.
    fn verb_picker(&mut self, ui: &mut egui::Ui) {
        let Some((from, to, at)) = self.pending_edge else {
            return;
        };

        // endpoints can vanish while the popup is open (delete / hot-reload)
        if self.g.node(from).is_none() || self.g.node(to).is_none() {
            self.pending_edge = None;
            return;
        }

        let (from_l, to_l) = (self.node_label(from), self.node_label(to));
        let mut picked: Option<RelationKind> = None;
        let mut close = ui.input(|i| i.key_pressed(egui::Key::Escape));

        let area = egui::Area::new(egui::Id::new("rs_verb_picker"))
            .order(egui::Order::Foreground)
            .fixed_pos(at)
            .constrain(true)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_max_width(230.0);
                    ui.label(RichText::new(format!("{from_l}  —?→  {to_l}")).small().weak());

                    let filter = ui.text_edit_singleline(&mut self.verb_filter);
                    if std::mem::take(&mut self.verb_filter_focus) {
                        filter.request_focus();
                    }

                    // RECONCILE(relations): iterate ALL generically — the verb
                    // list is exploding; anything added to the enum shows up here.
                    let needle = self.verb_filter.trim().to_lowercase();
                    let verbs: Vec<RelationKind> = RelationKind::ALL
                        .into_iter()
                        .filter(|k| needle.is_empty() || k.label().to_lowercase().contains(&needle))
                        .collect();

                    // typing narrowed it to one verb? Enter picks it
                    if verbs.len() == 1
                        && filter.lost_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::Enter))
                    {
                        picked = Some(verbs[0]);
                    }

                    ui.separator();
                    egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                        for k in verbs {
                            if ui.button(k.label()).clicked() {
                                picked = Some(k);
                            }
                        }
                    });
                });
            });

        if area.response.clicked_elsewhere() {
            close = true;
        }
        if let Some(kind) = picked {
            self.g
                .add_edge_with_label(from, to, Relation::new(kind), kind.label().into());
            self.status = format!(
                "linked: {from_l} —{}→ {to_l} (unsourced — attach a source!)",
                kind.label()
            );
            close = true;
        }
        if close {
            self.pending_edge = None;
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

        // left sidebar collapses to a thin strip holding only the expand toggle
        let strip = egui::Panel::left("left_strip")
            .resizable(false)
            .exact_size(26.0)
            .frame(egui::Frame::side_top_panel(ui.style()).inner_margin(Margin::symmetric(2, 6)));
        let expanded = egui::Panel::left("left").resizable(true).default_size(270.0);
        egui::Panel::show_animated_between_inside(
            ui,
            !self.prefs.sidebar_collapsed,
            strip,
            expanded,
            |ui, how_expanded| {
                if how_expanded == 0.0 {
                    self.sidebar_strip(ui);
                } else {
                    egui::ScrollArea::vertical().show(ui, |ui| self.left_panel(ui));
                }
            },
        );
        egui::Panel::right("right")
            .resizable(true)
            .default_size(310.0)
            .show_inside(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.inspector(ui));
            });
        egui::Panel::bottom("status").show_inside(ui, |ui| {
            ui.label(&self.status);
        });
        egui::CentralPanel::default().show_inside(ui, |ui| self.board(ui));

        // Camera-restore ordering, verified against egui_graphs 0.30
        // graph_view.rs: the widget loads our MetadataFrame (BOARD_META_ID
        // key) at the start of its ui() and persists it back at the end, so
        // a `camera_write` BEFORE the add() is what it draws with — EXCEPT
        // on the widget's very first frame: `handle_fit_to_screen` force-fits
        // whenever `MetadataInstance::first_frame_pending` is true, even with
        // fit_to_screen disabled, overwriting the injected pan/zoom.
        // `first_frame_pending` flips false (and is persisted) at the end of
        // frame 1, and nothing else mutates pan/zoom without user input, so
        // ONE re-assert here — after the widget ran — makes frame 2 onward
        // render the case's camera. Costs a single fitted frame at startup;
        // later loads/hot-reloads are already covered by the pre-write in
        // load() and this write is a no-op for them.
        if let Some(cam) = self.pending_camera.take() {
            Self::camera_write(ui, cam);
        }
    }
}

/// Golden-angle spiral so seeded / freshly-added / healed nodes don't stack.
fn spiral(n: usize) -> Pos2 {
    let a = n as f32 * 2.399_963_2;
    let r = 40.0 + 16.0 * n as f32;
    Pos2::new(r * a.cos(), r * a.sin())
}

/// Verb picker: iterates `RelationKind::ALL` (category-ordered) and drops a
/// weak section header whenever the category changes, so 78 verbs stay
/// scannable.
// RECONCILE(edgedrag): the right-drag verb popup should reuse this grouped
// iteration pattern — `ALL` is contiguous per `category()`.
fn relation_kind_combo(ui: &mut egui::Ui, id: &str, kind: &mut RelationKind) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(kind.label())
        .show_ui(ui, |ui| {
            let mut last_cat = "";
            for k in RelationKind::ALL {
                if k.category() != last_cat {
                    last_cat = k.category();
                    ui.label(RichText::new(last_cat).weak().small());
                }
                ui.selectable_value(kind, k, k.label());
            }
        });
}

/// Kind picker dropdown sectioned by `EntityKind::category()` — used by the
/// inspector; the add-entity flow uses the grid `kind_picker` instead.
fn kind_combo(ui: &mut egui::Ui, id: &str, kind: &mut EntityKind) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(format!("{} {}", kind.icon(), kind.label()))
        .show_ui(ui, |ui| {
            for cat in EntityKind::CATEGORIES {
                ui.add_space(2.0);
                ui.label(RichText::new(cat).weak().small());
                for k in EntityKind::ALL.into_iter().filter(|k| k.category() == cat) {
                    ui.selectable_value(kind, k, format!("{} {}", k.icon(), k.label()));
                }
            }
        });
}

/// Native .ron dialog seeded with the current path's directory and file name.
/// Blocking is fine: a frozen frame while a modal picker is up is the desktop
/// behaviour we want. RECONCILE(images): rfd also lands there for image picks.
fn case_dialog(current: &str) -> rfd::FileDialog {
    let mut d = rfd::FileDialog::new().add_filter("RON case", &["ron"]);
    let p = std::path::Path::new(current);
    if let Some(dir) = p.parent().filter(|d| d.is_dir()) {
        d = d.set_directory(dir);
    }
    if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
        d = d.set_file_name(name);
    }
    d
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

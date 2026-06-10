//! `RedstringApp`: the eframe shell. Owns the graph plus all chrome state and
//! lays out the panels; behaviour lives in the sibling modules — palette,
//! board, links, inspector, persist, chrome.

use std::path::Path;
use std::time::{Instant, SystemTime};

use eframe::App;
use egui::{Color32, Margin, Pos2, Vec2};
use egui_graphs::Graph;
use petgraph::Directed;
use petgraph::graph::{DefaultIx, NodeIndex};
use petgraph::stable_graph::StableGraph;

use crate::error::Error;
use crate::model::{Camera, Entity, EntityKind, Relation, SourceKind};
use crate::prefs::{Prefs, Theme};
use crate::project::Project;
use crate::shapes::{EntityNodeShape, SourcedEdgeShape};

use super::board;

pub type RsGraph = Graph<Entity, Relation, Directed, DefaultIx, EntityNodeShape, SourcedEdgeShape>;

pub(super) const OK_GREEN: Color32 = Color32::from_rgb(120, 200, 140);
pub(super) const ALARM_RED: Color32 = Color32::from_rgb(235, 80, 80);

/// Stable id for the board's egui_graphs metadata — camera zoom/pan live there.
pub(super) const BOARD_META_ID: &str = "redstring_board";

pub struct RedstringApp {
    pub(super) g: RsGraph,
    /// The open case directory. None until the first save creates one.
    pub(super) project: Option<Project>,
    /// Case display name — persisted in the file's meta block on save.
    pub(super) case_name: String,
    pub(super) status: String,
    pub(super) prefs: Prefs,

    /// Path + mtime of the case file as last written/read — the hot-reload watch.
    pub(super) watched: Option<(String, SystemTime)>,
    pub(super) last_watch_poll: Instant,
    /// Load the project on the first frame — camera restore needs a live `ui`.
    pub(super) pending_load: bool,
    /// Camera from the last load, re-asserted once AFTER the GraphView ran —
    /// the widget force-fits on its very first frame and would stomp it.
    pub(super) pending_camera: Option<Camera>,

    /// Armed kind in the entity palette — also what a board drop spawns.
    pub(super) ne_kind: EntityKind,
    /// Palette search filter.
    pub(super) kind_search: String,

    /// Right-drag edge creation: press on A, rubber-band, release on B, then
    /// the verb picker. `pending_edge` is the picker state: from, to, screen
    /// position of the popup.
    pub(super) edge_drag: Option<NodeIndex>,
    pub(super) pending_edge: Option<(NodeIndex, NodeIndex, Pos2)>,
    pub(super) verb_filter: String,
    pub(super) verb_filter_focus: bool,
    /// Inline name editor for an entity just dropped from the palette —
    /// node plus popup screen position; buf holds the in-progress text.
    pub(super) label_edit: Option<(NodeIndex, Pos2)>,
    pub(super) label_edit_buf: String,
    pub(super) label_edit_focus: bool,
    /// Node being resized via its corner handle — drag in progress.
    pub(super) resize: Option<NodeIndex>,
    /// Preferences window visibility — the prefs themselves live in `prefs`.
    pub(super) prefs_open: bool,
    /// Left-drag node move: node plus canvas-space grab offset. Replaces the
    /// lib's built-in dragging, whose compensation step snaps the node center
    /// onto the cursor.
    pub(super) node_drag: Option<(NodeIndex, Vec2)>,

    /// Add-source form state — targets the selected relation.
    pub(super) ns_kind: SourceKind,
    pub(super) ns_ref: String,
    pub(super) ns_excerpt: String,
}

impl RedstringApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut app = Self {
            g: Graph::from(&StableGraph::default()),
            project: None,
            case_name: "untitled case".to_owned(),
            status: "new case".to_owned(),
            prefs: Prefs::default(),
            watched: None,
            last_watch_poll: Instant::now(),
            pending_load: false,
            pending_camera: None,
            ne_kind: EntityKind::Person,
            kind_search: String::new(),
            edge_drag: None,
            pending_edge: None,
            verb_filter: String::new(),
            verb_filter_focus: false,
            label_edit: None,
            label_edit_buf: String::new(),
            label_edit_focus: false,
            resize: None,
            node_drag: None,
            prefs_open: false,
            ns_kind: SourceKind::Markdown,
            ns_ref: String::new(),
            ns_excerpt: String::new(),
        };
        // a case directory in cwd wins over the demo seed
        match Project::open(Path::new(".")) {
            Ok(project) => {
                app.project = Some(project);
                app.pending_load = true;
            }
            Err(Error::NotAProject { .. }) => app.seed_demo(),
            Err(err) => {
                app.seed_demo();
                app.status = format!("case in cwd unreadable: {err}");
            }
        }
        match Prefs::load() {
            Ok(p) => app.prefs = p,
            Err(err) => app.status = format!("prefs unreadable — using defaults: {err}"),
        }
        app.apply_prefs(&cc.egui_ctx);
        app
    }

    /// Push scale + theme prefs onto the egui context — startup and on change.
    pub(super) fn apply_prefs(&self, ctx: &egui::Context) {
        ctx.set_zoom_factor(self.prefs.ui_scale);
        ctx.set_theme(match self.prefs.theme {
            Theme::Dark => egui::Theme::Dark,
            Theme::Light => egui::Theme::Light,
        });
    }

    /// Write prefs to disk; a failure lands in the status bar, not stderr.
    pub(super) fn persist_prefs(&mut self) {
        if let Err(err) = self.prefs.save() {
            self.status = format!("prefs save failed: {err}");
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

        // Ctrl+S save, Ctrl+Shift+S save-as, Ctrl+O open — also on the toolbar.
        // Shift variant FIRST: consume_key matches logically, extra held
        // modifiers pass, so the plain Ctrl+S check would eat Ctrl+Shift+S.
        if ui.input_mut(|i| {
            i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::S)
        }) {
            self.save_as_dialog(ui);
        }
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S)) {
            self.save(ui);
        }
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::O)) {
            self.open_dialog(ui);
        }

        egui::Panel::top("toolbar")
            .resizable(false)
            .show_inside(ui, |ui| self.toolbar(ui));
        self.prefs_window(ui);

        // left sidebar collapses to a thin strip holding only the expand toggle
        let strip = egui::Panel::left("left_strip")
            .resizable(false)
            .exact_size(26.0)
            .frame(egui::Frame::side_top_panel(ui.style()).inner_margin(Margin::symmetric(2, 6)));
        // size_range caps auto-sizing — the kind list reports its widest row
        // as desired width and would otherwise balloon the panel
        let expanded = egui::Panel::left("left")
            .resizable(true)
            .default_size(270.0)
            .size_range(160.0..=480.0);
        egui::Panel::show_animated_between_inside(
            ui,
            !self.prefs.sidebar_collapsed,
            strip,
            expanded,
            |ui, how_expanded| {
                if how_expanded == 0.0 {
                    self.sidebar_strip(ui);
                } else {
                    self.left_panel(ui);
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

        // Camera-restore ordering, verified against egui_graphs 0.30 sources:
        // the widget loads our metadata at the start of its pass and persists
        // it at the end, so a camera write BEFORE the add is what it draws
        // with — EXCEPT the widget's very first frame, which force-fits even
        // with fit-to-screen disabled, stomping the injected pan/zoom. The
        // first-frame flag flips after frame one and nothing else mutates the
        // camera without user input, so ONE re-assert here — after the widget
        // ran — makes frame two onward render the case's camera. Costs a
        // single fitted frame at startup; later loads are covered by the
        // pre-write in load and this becomes a no-op.
        let Some(cam) = self.pending_camera.take() else {
            return;
        };
        board::camera_write(ui, cam);
    }
}

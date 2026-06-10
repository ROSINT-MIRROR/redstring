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

pub const OK_GREEN: Color32 = Color32::from_rgb(120, 200, 140);
pub const ALARM_RED: Color32 = Color32::from_rgb(235, 80, 80);

pub const BOARD_META_ID: &str = "redstring_board";

pub struct RedstringApp {
    pub g: RsGraph,

    pub project: Option<Project>,

    pub case_name: String,
    pub status: String,
    pub prefs: Prefs,

    pub watched: Option<(String, SystemTime)>,
    pub last_watch_poll: Instant,

    pub pending_load: bool,

    pub pending_camera: Option<Camera>,

    pub ne_kind: EntityKind,

    pub kind_search: String,

    pub edge_drag: Option<NodeIndex>,
    pub pending_edge: Option<(NodeIndex, NodeIndex, Pos2)>,
    pub verb_filter: String,
    pub verb_filter_focus: bool,

    pub label_edit: Option<(NodeIndex, Pos2)>,
    pub label_edit_buf: String,
    pub label_edit_focus: bool,

    pub resize: Option<NodeIndex>,

    pub prefs_open: bool,

    pub show_welcome: bool,

    pub welcome_name: String,

    pub board_size: Vec2,

    pub node_drag: Option<(NodeIndex, Vec2)>,

    pub ns_kind: SourceKind,
    pub ns_ref: String,
    pub ns_excerpt: String,
}

impl RedstringApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Result<Self, Error> {
        let mut app = Self {
            g: Graph::from(&StableGraph::default()),
            project: None,
            case_name: "untitled case".to_owned(),
            status: "no case open".to_owned(),
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
            show_welcome: false,
            welcome_name: String::new(),
            board_size: Vec2::ZERO,
            ns_kind: SourceKind::Markdown,
            ns_ref: String::new(),
            ns_excerpt: String::new(),
        };

        match Prefs::load() {
            Ok(p) => app.prefs = p,
            Err(err) => app.status = format!("prefs unreadable — using defaults: {err}"),
        }

        match Project::open(Path::new(".")) {
            Ok(project) => {
                app.project = Some(project);
                app.pending_load = true;
                app.remember_project()?;
            }
            Err(Error::NotAProject { .. }) => app.start_blank(),
            Err(err) => {
                app.start_blank();
                app.status = format!("case in cwd unreadable: {err}");
            }
        }
        app.apply_prefs(&cc.egui_ctx)?;
        Ok(app)
    }

    fn start_blank(&mut self) {
        self.show_welcome = true;
        self.pending_camera = Some(Camera::default());
    }

    pub fn apply_prefs(&self, ctx: &egui::Context) -> Result<(), Error> {
        ctx.set_zoom_factor(self.prefs.ui_scale);
        ctx.set_theme(match self.prefs.theme {
        Theme::Dark => egui::Theme::Dark,
            Theme::Light => egui::Theme::Light,
        });
        Ok(())
    }

    fn flag(&mut self, outcome: Result<(), Error>) {
        let Err(err) = outcome else { return };
        self.status = format!("⚠ {err}");
    }

    fn frame(&mut self, ui: &mut egui::Ui) -> Result<(), Error> {
        if self.pending_load {
            self.pending_load = false;
            self.load(ui)?;
        }
        self.sync_visuals()?;
        self.poll_external_edit(ui)?;
        self.handle_drop(ui)?;

        if ui.input_mut(|i| {
            i.consume_key(
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::S,
            )
        }) {
            self.save_as_dialog(ui)?;
        }
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S)) {
            self.save(ui)?;
        }
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::O)) {
            self.open_dialog(ui)?;
        }

        egui::Panel::top("toolbar")
            .resizable(false)
            .show_inside(ui, |ui| self.toolbar(ui))
            .inner?;
        self.prefs_window(ui)?;
        self.welcome_window(ui)?;

        let strip = egui::Panel::left("left_strip")
            .resizable(false)
            .exact_size(26.0)
            .frame(egui::Frame::side_top_panel(ui.style()).inner_margin(Margin::symmetric(2, 6)));

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
                    self.sidebar_strip(ui)
                } else {
                    self.left_panel(ui)
                }
            },
        )
        .inner?;
        egui::Panel::right("right")
            .resizable(true)
            .default_size(310.0)
            .show_inside(ui, |ui| {
                egui::ScrollArea::vertical()
                    .show(ui, |ui| self.inspector(ui))
                    .inner
            })
            .inner?;
        egui::Panel::bottom("status")
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(&self.status);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        self.zoom_bar(ui)
                    })
                    .inner
                })
                .inner
            })
            .inner?;
        egui::CentralPanel::default()
            .show_inside(ui, |ui| self.board(ui))
            .inner?;

        let Some(cam) = self.pending_camera.take() else {
            return Ok(());
        };
        board::camera_write(ui, cam)?;
        Ok(())
    }
}

impl App for RedstringApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let outcome = self.frame(ui);
        self.flag(outcome);
    }
}

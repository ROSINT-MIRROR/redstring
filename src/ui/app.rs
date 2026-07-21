use std::path::Path;
use std::time::{Instant, SystemTime};

use eframe::App;
use egui::{Color32, Margin, Pos2, Vec2};
use egui_graphs::Graph;
use petgraph::Directed;
use petgraph::graph::{DefaultIx, NodeIndex};
use petgraph::stable_graph::StableGraph;

use crate::error::Error;
use crate::model::{Author, CATEGORY_META, Camera, Entity, EntityKind, KIND_META, Relation, SourceKind, resolve_identity};
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

    pub author: Author,

    pub watched: Option<(String, SystemTime)>,
    pub last_watch_poll: Instant,

    pub pending_load: bool,

    pub pending_camera: Option<Camera>,

    pub ne_kind: EntityKind,

    pub kind_search: String,

    pub palette_open: [bool; CATEGORY_META.len()],

    pub edge_drag: Option<NodeIndex>,
    pub pending_edge: Option<(NodeIndex, NodeIndex, Pos2)>,
    pub verb_filter: String,
    pub verb_filter_focus: bool,

    pub label_edit: Option<NodeIndex>,
    pub label_edit_buf: String,
    pub label_edit_focus: bool,

    pub new_attr_key: String,

    pub resize: Option<NodeIndex>,

    pub prefs_open: bool,
    pub settings_tab: super::chrome::SettingsTab,

    pub registry: crate::modules::Registry,
    pub module_tx: std::sync::mpsc::Sender<crate::modules::ModuleRun>,
    pub module_rx: std::sync::mpsc::Receiver<crate::modules::ModuleRun>,
    pub jobs: Vec<super::lookups::Job>,
    pub next_run_id: u64,

    pub delete_confirm: Option<Vec<NodeIndex>>,

    pub quit_confirm: bool,
    pub allow_close: bool,

    pub saved_fp: String,


    pub size_by_degree: bool,

    pub show_welcome: bool,

    pub welcome_name: String,

    pub board_size: Vec2,

    pub node_drag: Option<Vec<(NodeIndex, Vec2)>>,

    pub band: Option<Pos2>,
    pub panning: bool,

    pub layout_anim: Option<super::layout::LayoutAnim>,

    pub ns_kind: SourceKind,
    pub ns_ref: String,
    pub ns_excerpt: String,
}

impl RedstringApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Result<Self, Error> {
        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        cc.egui_ctx.set_fonts(fonts);
        let (module_tx, module_rx) = std::sync::mpsc::channel();
        let (registry, registry_err) = match crate::modules::Registry::load() {
            Ok(r) => (r, String::new()),
            Err(err) => (
                crate::modules::Registry { entries: Vec::new() },
                format!("modules unavailable: {err}"),
            ),
        };
        let mut app = Self {
            g: Graph::from(&StableGraph::default()),
            project: None,
            case_name: "untitled case".to_owned(),
            status: "no case open".to_owned(),
            prefs: Prefs::default(),
            author: resolve_identity(None)?,
            watched: None,
            last_watch_poll: Instant::now(),
            pending_load: false,
            pending_camera: None,
            ne_kind: EntityKind::Person,
            kind_search: String::new(),
            palette_open: [true; CATEGORY_META.len()],
            edge_drag: None,
            pending_edge: None,
            verb_filter: String::new(),
            verb_filter_focus: false,
            label_edit: None,
            label_edit_buf: String::new(),
            label_edit_focus: false,
            new_attr_key: String::new(),
            resize: None,
            node_drag: None,
            band: None,
            panning: false,
            layout_anim: None,
            prefs_open: false,
            settings_tab: super::chrome::SettingsTab::General,
            registry,
            module_tx,
            module_rx,
            jobs: Vec::new(),
            next_run_id: 1,
            delete_confirm: None,
            quit_confirm: false,
            allow_close: false,
            saved_fp: String::new(),
            size_by_degree: false,
            show_welcome: false,
            welcome_name: String::new(),
            board_size: Vec2::ZERO,
            ns_kind: SourceKind::Markdown,
            ns_ref: String::new(),
            ns_excerpt: String::new(),
        };

        if !registry_err.is_empty() {
            app.status = registry_err;
        }

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

        ctx.all_styles_mut(|s| {
            s.text_styles
                .insert(egui::TextStyle::Small, egui::FontId::proportional(12.0));

            s.spacing.item_spacing = Vec2::new(8.0, 6.0);
            s.spacing.button_padding = Vec2::new(8.0, 5.0);
            s.spacing.window_margin = Margin::same(12);
            s.spacing.menu_margin = Margin::same(8);
        });
        Ok(())
    }

    pub fn clear_selection(&mut self) -> Result<(), Error> {
        let nodes = self.g.selected_nodes().to_vec();
        for idx in nodes {
            let Some(n) = self.g.node_mut(idx) else {
                continue;
            };
            n.set_selected(false);
        }
        let edges = self.g.selected_edges().to_vec();
        for idx in edges {
            let Some(e) = self.g.edge_mut(idx) else {
                continue;
            };
            e.set_selected(false);
        }
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
        self.guard_close(ui)?;
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
        if !ui.ctx().egui_wants_keyboard_input()
            && ui.input(|i| i.key_pressed(egui::Key::Delete))
            && self.delete_confirm.is_none()
        {
            let sel = self.g.selected_nodes().to_vec();
            if !sel.is_empty() {
                self.delete_confirm = Some(sel);
            }
        }

        egui::Panel::top("toolbar")
            .resizable(false)
            .frame(
                egui::Frame::side_top_panel(ui.style())
                    .inner_margin(Margin::symmetric(8, 6)),
            )
            .show_inside(ui, |ui| self.toolbar(ui))
            .inner?;
        self.settings_window(ui)?;
        self.quit_window(ui)?;
        self.poll_module_runs(ui)?;
        self.delete_confirm_window(ui)?;
        self.welcome_window(ui)?;

        let strip = egui::Panel::left("left_strip")
            .resizable(false)
            .exact_size(super::style::STRIP_W + 8.0)
            .frame(egui::Frame::side_top_panel(ui.style()).inner_margin(Margin::symmetric(2, 6)));

        let painter = ui.painter().clone();
        let icon_f = egui::FontId::proportional(24.0);
        let label_f = egui::FontId::proportional(12.0);
        let row_w = KIND_META
            .iter()
            .map(|m| {
                let iw = painter
                    .layout_no_wrap(m.icon.to_owned(), icon_f.clone(), Color32::WHITE)
                    .size()
                    .x;
                let lw = painter
                    .layout_no_wrap(m.label.to_owned(), label_f.clone(), Color32::WHITE)
                    .size()
                    .x;
                iw + lw
            })
            .fold(0.0f32, f32::max);
        let extras = ui.spacing().button_padding.x * 2.0
            + ui.spacing().item_spacing.x
            + ui.spacing().scroll.bar_width
            + 16.0;
        let palette_w = ((row_w + extras) * 1.1).ceil();
        let expanded = egui::Panel::left("left")
            .resizable(true)
            .default_size(palette_w)
            .size_range(140.0..=480.0);
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
        let has_sel = !self.g.selected_nodes().is_empty() || !self.g.selected_edges().is_empty();
        let show_inspector = !self.prefs.inspector_collapsed || has_sel;
        let insp_strip = egui::Panel::right("right_strip")
            .resizable(false)
            .exact_size(super::style::STRIP_W + 8.0)
            .frame(egui::Frame::side_top_panel(ui.style()).inner_margin(Margin::symmetric(2, 6)));
        let insp_expanded = egui::Panel::right("right")
            .resizable(true)
            .default_size(248.0)
            .size_range(170.0..=520.0);
        egui::Panel::show_animated_between_inside(
            ui,
            show_inspector,
            insp_strip,
            insp_expanded,
            |ui, how_expanded| {
                if how_expanded == 0.0 {
                    self.inspector_strip(ui)
                } else {
                    egui::ScrollArea::vertical()
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(6.0, 3.0);
                            ui.spacing_mut().button_padding = Vec2::new(6.0, 3.0);
                            for st in [egui::TextStyle::Body, egui::TextStyle::Button] {
                                let Some(f) = ui.style_mut().text_styles.get_mut(&st) else {
                                    continue;
                                };
                                f.size = 15.0;
                            }
                            self.inspector(ui)
                        })
                        .inner
                }
            },
        )
        .inner?;
        egui::Panel::bottom("status")
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(&self.status);
                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| -> Result<(), Error> {
                            self.zoom_bar(ui)?;
                            self.jobs_chip(ui)?;
                            Ok(())
                        },
                    )
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

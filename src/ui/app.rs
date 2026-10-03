use std::path::Path;

use eframe::App;
use egui::{Color32, Margin, Pos2, RichText, Vec2};
use egui_graphs::Graph;
use petgraph::Directed;
use petgraph::graph::{DefaultIx, NodeIndex};
use petgraph::stable_graph::StableGraph;

use crate::error::Error;
use crate::model::{
    Author, CATEGORY_META, Camera, Entity, EntityKind, KIND_META, Preset, Relation, SourceKind,
    resolve_identity,
};
use crate::prefs::Prefs;
use crate::project::Project;
use crate::shapes::{EntityNodeShape, SourcedEdgeShape};

use super::archive::AlibiState;
use super::board;
use super::layout::LayoutKind;
use super::timeline::TimelineState;
use super::watch::Watcher;

pub type RsGraph = Graph<Entity, Relation, Directed, DefaultIx, EntityNodeShape, SourcedEdgeShape>;

pub const OK_GREEN: Color32 = Color32::from_rgb(120, 200, 140);
pub const ALARM_RED: Color32 = super::style::DANGER;
const DIRTY_AMBER: Color32 = Color32::from_rgb(235, 150, 70);

pub const BOARD_META_ID: &str = "redstring_board";

pub struct RedstringApp {
    pub g: RsGraph,

    pub project: Option<Project>,

    pub case_name: String,
    pub status: String,
    pub prefs: Prefs,

    pub author: Author,

    pub watcher: Watcher,

    pub pending_load: bool,

    pub pending_camera: Option<Camera>,

    pub ne_kind: EntityKind,

    pub ne_preset: Option<Preset>,

    pub kind_search: String,

    pub palette_open: [bool; CATEGORY_META.len()],

    pub kind_expanded: [bool; KIND_META.len()],

    pub edge_drag: Option<NodeIndex>,
    pub right_press: Option<(Option<NodeIndex>, Pos2)>,
    pub pending_edge: Option<(NodeIndex, NodeIndex, Pos2)>,

    pub node_menu: Option<(NodeIndex, Pos2)>,
    pub node_menu_just_opened: bool,
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

    pub seal_confirm: Option<(Vec<NodeIndex>, bool)>,

    pub merge_confirm: Option<(NodeIndex, NodeIndex)>,

    pub quit_confirm: bool,
    pub allow_close: bool,

    pub saved_fp: String,

    pub last_saved_at: Option<i64>,

    pub mode: super::mode::Mode,
    pub autosave: super::mode::Autosave,

    pub size_by_degree: bool,

    pub show_welcome: bool,

    pub welcome_name: String,

    pub board_size: Vec2,

    pub node_drag: Option<Vec<(NodeIndex, Vec2)>>,

    pub band: Option<Pos2>,
    pub panning: bool,

    pub layout_anim: Option<super::layout::LayoutAnim>,

    pub current_layout: LayoutKind,

    pub active_path: Option<Vec<super::graphalg::PathHop>>,

    pub find_open: bool,
    pub find_query: String,
    pub find_focus: bool,
    pub find_selected: usize,

    pub ns_kind: SourceKind,
    pub ns_ref: String,
    pub ns_excerpt: String,

    pub theme_watcher: crate::theme::ThemeWatcher,
    pub custom_theme: Option<crate::theme::ThemeDef>,

    pub timeline: TimelineState,

    pub alibi: AlibiState,

    pub fit_on_open: bool,
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
                crate::modules::Registry {
                    entries: Vec::new(),
                },
                format!("modules unavailable: {err}"),
            ),
        };
        let theme_path = crate::theme::custom_theme_path()?;
        let custom_theme = match crate::theme::load_theme_file(&theme_path) {
            Ok(def) => Some(def),
            Err(err) => {
                crate::dlog_info!("theme", "custom theme not loaded at startup: {err}");
                None
            }
        };
        let theme_watcher = crate::theme::ThemeWatcher::new(theme_path)?;

        let mut app = Self {
            g: Graph::from(&StableGraph::default()),
            project: None,
            case_name: "untitled case".to_owned(),
            status: "no case open".to_owned(),
            prefs: Prefs::default(),
            author: resolve_identity(None)?,
            watcher: Watcher::default(),
            pending_load: false,
            pending_camera: None,
            ne_kind: EntityKind::Person,
            ne_preset: None,
            kind_search: String::new(),
            palette_open: [true; CATEGORY_META.len()],
            kind_expanded: [false; KIND_META.len()],
            edge_drag: None,
            right_press: None,
            pending_edge: None,
            node_menu: None,
            node_menu_just_opened: false,
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
            current_layout: LayoutKind::Free,
            active_path: None,
            find_open: false,
            find_query: String::new(),
            find_focus: false,
            find_selected: 0,
            prefs_open: false,
            settings_tab: super::chrome::SettingsTab::General,
            registry,
            module_tx,
            module_rx,
            jobs: Vec::new(),
            next_run_id: 1,
            delete_confirm: None,
            seal_confirm: None,
            merge_confirm: None,
            quit_confirm: false,
            allow_close: false,
            saved_fp: String::new(),
            last_saved_at: None,
            mode: super::mode::Mode::Live,
            autosave: super::mode::Autosave::default(),
            size_by_degree: false,
            show_welcome: false,
            welcome_name: String::new(),
            board_size: Vec2::ZERO,
            ns_kind: SourceKind::Markdown,
            ns_ref: String::new(),
            ns_excerpt: String::new(),
            theme_watcher,
            custom_theme,
            timeline: TimelineState::closed()?,
            alibi: AlibiState::new()?,
            fit_on_open: false,
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
                crate::dlog_info!("app", "opened case in cwd {}", project.root.display());
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
        crate::dlog_info!(
            "app",
            "startup case={:?} has_project={}",
            app.case_name,
            app.project.is_some()
        );
        Ok(app)
    }

    fn start_blank(&mut self) {
        crate::dlog_info!("app", "start blank (no case in cwd)");
        self.show_welcome = true;
        self.pending_camera = Some(Camera::default());
    }

    pub fn active_theme_def(&self) -> Result<crate::theme::ThemeDef, Error> {
        if self.prefs.theme == crate::theme::Theme::Custom {
            let Some(def) = &self.custom_theme else {
                return crate::theme::Theme::Dark.def();
            };
            return Ok(def.clone());
        }
        self.prefs.theme.def()
    }

    pub fn apply_prefs(&self, ctx: &egui::Context) -> Result<(), Error> {
        ctx.set_zoom_factor(self.prefs.ui_scale);
        let def = self.active_theme_def()?;
        let v = def.to_visuals()?;
        ctx.set_visuals(v);

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

    pub fn choose_layout(&mut self, ui: &egui::Ui, kind: LayoutKind) -> Result<(), Error> {
        self.current_layout = kind;
        self.apply_layout(ui, kind)
    }

    fn save_chip(&self, ui: &mut egui::Ui) -> Result<(), Error> {
        if !self.live()? {
            ui.separator();
            ui.colored_label(
                super::mode::EXPLORE_INK,
                RichText::new(format!(
                    "{} EXPLORE — sandbox, nothing is written",
                    egui_phosphor::regular::FLASK
                ))
                .small()
                .strong(),
            );
            return Ok(());
        }
        if self.autosave.pending_since.is_some() {
            ui.separator();
            ui.colored_label(
                DIRTY_AMBER,
                RichText::new(format!(
                    "{} autosaving…",
                    egui_phosphor::regular::CLOUD_ARROW_UP
                ))
                .small(),
            );
            return Ok(());
        }
        let Some(ts) = self.last_saved_at else {
            ui.separator();
            ui.colored_label(
                OK_GREEN,
                RichText::new(format!(
                    "{} live — every change autosaves",
                    egui_phosphor::regular::HARD_DRIVE
                ))
                .small(),
            );
            return Ok(());
        };
        ui.separator();
        ui.colored_label(
            OK_GREEN,
            RichText::new(format!(
                "{} live · saved {} UTC",
                egui_phosphor::regular::HARD_DRIVE,
                hms_utc(ts)
            ))
            .small(),
        );
        Ok(())
    }

    fn flag(&mut self, outcome: Result<(), Error>) {
        let Err(err) = outcome else { return };
        self.status = format!("⚠ {err}");
    }

    pub fn poll_theme_reload(&mut self, ui: &mut egui::Ui) -> Result<(), Error> {
        let outcome = self.theme_watcher.poll();
        let loaded = match outcome {
            Ok(Some(def)) => def,
            Ok(None) => return Ok(()),
            Err(err) => {
                self.status = format!("⚠ theme file error: {err}");
                return Ok(());
            }
        };
        self.custom_theme = Some(loaded.clone());
        if self.prefs.theme == crate::theme::Theme::Custom {
            let v = loaded.to_visuals()?;
            ui.ctx().set_visuals(v);
            ui.ctx().request_repaint();
            self.status = format!("theme reloaded from {}", self.theme_watcher.path.display());
            crate::dlog_info!(
                "theme",
                "hot-reload: reloaded theme from {:?}",
                self.theme_watcher.path
            );
        }
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(1000));
        Ok(())
    }

    fn frame(&mut self, ui: &mut egui::Ui) -> Result<(), Error> {
        if self.pending_load {
            self.pending_load = false;
            self.load(ui)?;
        }
        self.guard_close(ui)?;
        self.sync_visuals()?;
        self.poll_external_edit(ui)?;
        self.poll_theme_reload(ui)?;
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
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::F)) {
            self.find_open = true;
            self.find_focus = true;
            self.find_selected = 0;
        }
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::T)) {
            self.timeline.open = !self.timeline.open;
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
            .frame(egui::Frame::side_top_panel(ui.style()).inner_margin(Margin::symmetric(8, 6)))
            .show_inside(ui, |ui| self.toolbar(ui))
            .inner?;
        self.settings_window(ui)?;
        self.quit_window(ui)?;
        self.conflict_window(ui)?;
        self.poll_module_runs(ui)?;
        self.delete_confirm_window(ui)?;
        self.seal_confirm_window(ui)?;
        self.merge_confirm_window(ui)?;
        self.welcome_window(ui)?;
        self.find_window(ui)?;
        super::logview::window(ui.ctx())?;

        let strip = egui::Panel::left("left_strip")
            .resizable(false)
            .exact_size(super::style::STRIP_W + 8.0)
            .frame(egui::Frame::side_top_panel(ui.style()).inner_margin(Margin::symmetric(2, 6)));

        let expanded = egui::Panel::left("left")
            .resizable(true)
            .default_size(self.prefs.palette_width)
            .size_range(140.0..=480.0);
        let mut palette_settled = false;
        let palette_panel = egui::Panel::show_animated_between_inside(
            ui,
            !self.prefs.sidebar_collapsed,
            strip,
            expanded,
            |ui, how_expanded| {
                if how_expanded == 0.0 {
                    self.sidebar_strip(ui)
                } else {
                    if how_expanded >= 1.0 {
                        palette_settled = true;
                    }
                    self.left_panel(ui)
                }
            },
        );
        let palette_w = palette_panel.response.rect.width();
        palette_panel.inner?;
        if palette_settled
            && (palette_w - self.prefs.palette_width).abs() > 0.5
            && ui.input(|i| i.pointer.any_released())
        {
            self.prefs.palette_width = palette_w;
            self.prefs.save()?;
        }
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
                            self.save_chip(ui)?;
                            Ok(())
                        },
                    )
                    .inner
                })
                .inner
            })
            .inner?;
        if self.timeline.open {
            egui::Panel::bottom("timeline")
                .resizable(true)
                .default_size(430.0)
                .size_range(200.0..=820.0)
                .show_inside(ui, |ui| self.timeline_panel(ui))
                .inner?;
        }
        egui::CentralPanel::default()
            .show_inside(ui, |ui| self.board(ui))
            .inner?;

        self.apply_pending_camera(ui)?;
        self.mode_border(ui)?;
        self.poll_alibi()?;
        self.autosave_tick(ui)
    }

    fn apply_pending_camera(&mut self, ui: &mut egui::Ui) -> Result<(), Error> {
        let Some(cam) = self.pending_camera.take() else {
            return Ok(());
        };
        board::camera_write(ui, cam)
    }

    pub fn apply_seal(&mut self, list: &[NodeIndex], target: bool) -> Result<usize, Error> {
        let mut changed = 0usize;
        for idx in list {
            let Some(node) = self.g.node_mut(*idx) else {
                continue;
            };
            let e = node.payload_mut();
            if e.sealed != target {
                e.sealed = target;
                changed += 1;
            }
        }
        Ok(changed)
    }

    pub fn seal_confirm_window(&mut self, ui: &egui::Ui) -> Result<(), Error> {
        let Some((list, target)) = self.seal_confirm.clone() else {
            return Ok(());
        };
        let n = list.len();
        let verb = if target { "seal" } else { "unseal" };
        let mut yes = false;
        let mut cancel = ui.input(|i| i.key_pressed(egui::Key::Escape));
        egui::Window::new(if target { "seal entities" } else { "unseal entities" })
            .collapsible(false)
            .resizable(false)
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ui.ctx().content_rect().center())
            .show(ui.ctx(), |ui| {
                let noun = if n == 1 { "entity" } else { "entities" };
                let body = if target {
                    format!(
                        "Seal {n} {noun}? Redacted from exports & generated files — the local copy keeps the data."
                    )
                } else {
                    format!("Unseal {n} {noun}? They reappear in exports & generated files.")
                };
                ui.label(body);
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let icon = if target {
                        egui_phosphor::regular::EYE_SLASH
                    } else {
                        egui_phosphor::regular::EYE
                    };
                    let yes_btn = ui.button(
                        RichText::new(format!("{icon} yes, {verb}"))
                            .color(super::style::DANGER)
                            .strong(),
                    );
                    if ui.memory(|m| m.focused().is_none()) {
                        yes_btn.request_focus();
                    }
                    yes = yes_btn.clicked();
                    cancel |= ui.button("cancel").clicked();
                });
            });
        if yes {
            let changed = self.apply_seal(&list, target)?;
            crate::dlog_info!(
                "ops",
                "bulk seal target={target} changed={changed} requested={n}"
            );
            self.seal_confirm = None;
            self.status = format!(
                "{} {n} entities",
                if target { "sealed" } else { "unsealed" }
            );
        }
        if cancel {
            self.seal_confirm = None;
        }
        Ok(())
    }

    pub fn merge_confirm_window(&mut self, ui: &egui::Ui) -> Result<(), Error> {
        let Some((a, b)) = self.merge_confirm else {
            return Ok(());
        };
        let la = self.node_label(a)?;
        let lb = self.node_label(b)?;
        let mut choice: Option<(NodeIndex, NodeIndex)> = None;
        let mut cancel = ui.input(|i| i.key_pressed(egui::Key::Escape));
        egui::Window::new("merge entities")
            .collapsible(false)
            .resizable(false)
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ui.ctx().content_rect().center())
            .show(ui.ctx(), |ui| {
                ui.label(
                    "Two entities become one — pick which survives. The other's facts, sources and relations move onto it, then it is removed.",
                );
                ui.add_space(8.0);
                let merge_icon = egui_phosphor::regular::ARROWS_MERGE;
                let keep_a =
                    ui.button(RichText::new(format!("{merge_icon} keep {la}")).strong());
                if ui.memory(|m| m.focused().is_none()) {
                    keep_a.request_focus();
                }
                if keep_a.clicked() {
                    choice = Some((a, b));
                }
                if ui
                    .button(RichText::new(format!("{merge_icon} keep {lb}")).strong())
                    .clicked()
                {
                    choice = Some((b, a));
                }
                ui.add_space(6.0);
                cancel |= ui.button("cancel").clicked();
            });
        let Some((survivor, absorbed)) = choice else {
            if cancel {
                self.merge_confirm = None;
            }
            return Ok(());
        };
        let survivor_slug = self.node_slug(survivor)?;
        let absorbed_slug = self.node_slug(absorbed)?;
        self.merge_entities(survivor, absorbed)?;
        self.g.set_selected_nodes(vec![survivor]);
        self.merge_confirm = None;
        crate::dlog_info!("ops", "merged {absorbed_slug} into {survivor_slug}");
        self.status = format!("merged {absorbed_slug} into {survivor_slug}");
        Ok(())
    }
}

impl App for RedstringApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let outcome = self.frame(ui);
        self.flag(outcome);
    }
}

fn hms_utc(ts: i64) -> String {
    let secs = ts.rem_euclid(86400);
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{h:02}:{m:02}:{s:02}")
}

#[cfg(test)]
mod tests {
    use super::hms_utc;

    #[test]
    fn hms_utc_formats_time_of_day() {
        assert_eq!(hms_utc(0), "00:00:00");
        assert_eq!(hms_utc(3661), "01:01:01");
        assert_eq!(hms_utc(86399), "23:59:59");
        assert_eq!(hms_utc(86400), "00:00:00");
        assert_eq!(hms_utc(1_700_000_000), "22:13:20");
    }

    #[test]
    fn hms_utc_handles_pre_epoch() {
        assert_eq!(hms_utc(-1), "23:59:59");
    }
}

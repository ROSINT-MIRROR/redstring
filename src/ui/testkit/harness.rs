use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::Instant;

use egui::Vec2;
use egui_graphs::Graph;
use petgraph::stable_graph::StableGraph;

use crate::error::Error;
use crate::model::{
    Author, CATEGORY_META, CaseFile, CaseMeta, EntityKind, KIND_META, SourceKind, now_unix,
};
use crate::modules::Registry;
use crate::prefs::Prefs;
use crate::project::{Project, read_case, write_case};
use crate::ui::RedstringApp;
use crate::ui::chrome::SettingsTab;
use crate::ui::layout::LayoutKind;

pub fn test_author() -> Result<Author, Error> {
    Ok(Author {
        name: "Test Analyst".to_owned(),
        email: "analyst@redstring.test".to_owned(),
    })
}

pub fn app() -> Result<RedstringApp, Error> {
    app_authored(test_author()?)
}

pub fn app_authored(author: Author) -> Result<RedstringApp, Error> {
    let (module_tx, module_rx) = mpsc::channel();
    Ok(RedstringApp {
        g: Graph::from(&StableGraph::default()),
        project: None,
        case_name: "test case".to_owned(),
        status: String::new(),
        prefs: Prefs::default(),
        author,
        watched: None,
        last_watch_poll: Instant::now(),
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
        prefs_open: false,
        settings_tab: SettingsTab::General,
        registry: Registry {
            entries: Vec::new(),
        },
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
        size_by_degree: false,
        show_welcome: false,
        welcome_name: String::new(),
        board_size: Vec2::ZERO,
        node_drag: None,
        band: None,
        panning: false,
        layout_anim: None,
        active_path: None,
        find_open: false,
        find_query: String::new(),
        find_focus: false,
        find_selected: 0,
        ns_kind: SourceKind::Markdown,
        ns_ref: String::new(),
        ns_excerpt: String::new(),
    })
}

pub fn frame<R>(
    app: &mut RedstringApp,
    mut body: impl FnMut(&mut RedstringApp, &mut egui::Ui) -> Result<R, Error>,
) -> Result<R, Error> {
    let ctx = egui::Context::default();
    let mut slot: Option<Result<R, Error>> = None;
    let _full = ctx.run_ui(egui::RawInput::default(), |ui| {
        egui::CentralPanel::default().show_inside(ui, |ui| {
            slot = Some(body(app, ui));
        });
    });
    let outcome = slot.ok_or(Error::Testkit {
        what: "headless egui frame did not execute the panel body".to_owned(),
    })?;
    outcome
}

pub fn frame_input<R>(
    app: &mut RedstringApp,
    input: egui::RawInput,
    mut body: impl FnMut(&mut RedstringApp, &mut egui::Ui) -> Result<R, Error>,
) -> Result<R, Error> {
    let ctx = egui::Context::default();
    let mut slot: Option<Result<R, Error>> = None;
    let _full = ctx.run_ui(input, |ui| {
        egui::CentralPanel::default().show_inside(ui, |ui| {
            slot = Some(body(app, ui));
        });
    });
    let outcome = slot.ok_or(Error::Testkit {
        what: "headless egui frame did not execute the panel body".to_owned(),
    })?;
    outcome
}

pub fn save(app: &mut RedstringApp) -> Result<(), Error> {
    frame(app, |a, ui| a.save(ui))
}

pub fn load(app: &mut RedstringApp) -> Result<(), Error> {
    frame(app, |a, ui| a.load(ui))
}

pub fn layout(app: &mut RedstringApp, kind: LayoutKind) -> Result<(), Error> {
    frame(app, |a, ui| a.apply_layout(ui, kind))
}

static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

pub struct TempDir {
    pub path: PathBuf,
}

impl Drop for TempDir {
    fn drop(&mut self) {
        drop(std::fs::remove_dir_all(&self.path));
    }
}

pub fn temp_dir() -> Result<TempDir, Error> {
    let base = std::env::temp_dir();
    let seq = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let stamp = now_unix()?;
    let name = format!("redstring-test-{}-{stamp}-{seq}", std::process::id());
    let path = base.join(name);
    std::fs::create_dir_all(&path).map_err(|source| Error::Io {
        path: path.clone(),
        source,
    })?;
    Ok(TempDir { path })
}

pub fn temp_project() -> Result<(TempDir, Project), Error> {
    let td = temp_dir()?;
    let case_dir = td.path.join("case");
    let project = Project::create(&case_dir)?;
    let mut cf = CaseFile::default();
    cf.meta = CaseMeta::stamped("test case")?;
    write_case(&project, &cf)?;
    Ok((td, project))
}

pub fn round_trip(project: &Project, cf: &CaseFile) -> Result<CaseFile, Error> {
    write_case(project, cf)?;
    read_case(project)
}

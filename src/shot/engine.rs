use std::collections::HashMap;
use std::path::{Path, PathBuf};

use egui::{Align, Color32, FontFamily, FontId, Pos2, Rect, Vec2};
use egui::text::LayoutJob;
use egui_graphs::{
    DisplayEdge, DisplayNode, Graph, GraphView, Layout, LayoutState, MetadataFrame,
    SettingsInteraction, SettingsNavigation, SettingsStyle,
};
use petgraph::EdgeType;
use petgraph::graph::NodeIndex;
use petgraph::stable_graph::IndexType;
use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::model::{Camera, Entity, KIND_META, RELATION_META, Verification};
use crate::project::{Project, read_case};
use crate::shapes::{
    IMG_RADIUS, LABEL_GAP, LABEL_SIZE, LABEL_WRAP, LabelAnchor, NODE_RADIUS, SourcedEdgeShape,
    label_box,
};
use crate::ui::{RsGraph, apply_visuals, board_color, build_graph};

use super::png::render_png;

const META_ID: &str = "redstring_shot_board";
const RENDER_FRAMES: usize = 3;
const DEFAULT_WIDTH: u32 = 1600;
const DEFAULT_HEIGHT: u32 = 1000;
const MAX_DIM: u32 = 8192;
const MARGIN_FRAC: f32 = 0.86;
const MIN_ZOOM: f32 = 0.01;
const MAX_ZOOM: f32 = 30.0;
const MAX_FIT_ZOOM: f32 = 2.2;
const MAX_FONT_PX: f32 = 256.0;
const FORCE_ITERS: u32 = 240;
const FORCE_STEP: f32 = 150.0;
const FORCE_GRAVITY: f32 = 0.04;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct FixedState;

impl LayoutState for FixedState {}

#[derive(Default)]
struct FixedLayout {
    state: FixedState,
}

impl Layout<FixedState> for FixedLayout {
    fn from_state(state: FixedState) -> impl Layout<FixedState> {
        Self { state }
    }

    fn next<N, E, Ty, Ix, Dn, De>(&mut self, _g: &mut Graph<N, E, Ty, Ix, Dn, De>, _ui: &egui::Ui)
    where
        N: Clone,
        E: Clone,
        Ty: EdgeType,
        Ix: IndexType,
        Dn: DisplayNode<N, E, Ty, Ix>,
        De: DisplayEdge<N, E, Ty, Ix, Dn>,
    {
    }

    fn state(&self) -> FixedState {
        self.state.clone()
    }
}

pub struct ShotReport {
    pub out: PathBuf,
    pub nodes: usize,
    pub edges: usize,
    pub mode: &'static str,
    pub backend: String,
}

pub(super) struct Opts {
    case: PathBuf,
    pub(super) out: PathBuf,
    pub(super) width: u32,
    pub(super) height: u32,
    no_layout: bool,
    pub(super) dark: bool,
}

enum OutMode {
    Png,
    Json,
}

pub fn shoot(args: &[String]) -> Result<ShotReport, Error> {
    let opts = parse_opts(args)?;
    let mode = mode_for(&opts.out)?;
    let project = Project::open(&opts.case)?;
    let cf = read_case(&project)?;
    let loaded = build_graph(&project, &cf)?;
    let mut g = loaded.g;
    apply_visuals(&mut g, cf.display.size_by_degree)?;
    if !opts.no_layout {
        layout_pass(&mut g);
    }
    let board = Vec2::new(opts.width as f32, opts.height as f32);
    let cam = fit_camera(&g, board);
    let nodes = g.g().node_count();
    let edges = g.g().edge_count();
    let (mode_str, backend) = match mode {
        OutMode::Png => ("png", render_png(&mut g, cam, &opts)?),
        OutMode::Json => ("json", render_json(&mut g, cam, &opts)?),
    };
    Ok(ShotReport {
        out: opts.out,
        nodes,
        edges,
        mode: mode_str,
        backend,
    })
}

fn parse_opts(args: &[String]) -> Result<Opts, Error> {
    let mut case: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut width = DEFAULT_WIDTH;
    let mut height = DEFAULT_HEIGHT;
    let mut no_layout = false;
    let mut dark = true;
    let mut i = 0usize;
    loop {
        let Some(arg) = args.get(i) else { break };
        if arg == "--shot" {
            case = Some(PathBuf::from(arg_value(args, i)?));
            i += 2;
        } else if arg == "--out" {
            out = Some(PathBuf::from(arg_value(args, i)?));
            i += 2;
        } else if arg == "--width" {
            width = parse_dim(arg_value(args, i)?, "width")?;
            i += 2;
        } else if arg == "--height" {
            height = parse_dim(arg_value(args, i)?, "height")?;
            i += 2;
        } else if arg == "--theme" {
            dark = parse_theme(arg_value(args, i)?)?;
            i += 2;
        } else if arg == "--no-layout" {
            no_layout = true;
            i += 1;
        } else {
            i += 1;
        }
    }
    let Some(case) = case else {
        return Err(Error::ShotArgs {
            what: "missing <case> after --shot".to_owned(),
        });
    };
    let Some(out) = out else {
        return Err(Error::ShotArgs {
            what: "missing --out <path.png|path.json>".to_owned(),
        });
    };
    Ok(Opts {
        case,
        out,
        width,
        height,
        no_layout,
        dark,
    })
}

fn parse_theme(text: &str) -> Result<bool, Error> {
    if text == "dark" {
        return Ok(true);
    }
    if text == "light" {
        return Ok(false);
    }
    Err(Error::ShotArgs {
        what: format!("bad --theme '{text}': use 'light' or 'dark'"),
    })
}

fn arg_value<'a>(args: &'a [String], i: usize) -> Result<&'a String, Error> {
    let Some(value) = args.get(i + 1) else {
        return Err(Error::ShotArgs {
            what: format!("missing value after '{}'", args[i]),
        });
    };
    Ok(value)
}

fn parse_dim(text: &str, name: &str) -> Result<u32, Error> {
    let value = text.parse::<u32>().map_err(|source| Error::ShotArgs {
        what: format!("bad --{name} '{text}': {source}"),
    })?;
    if value == 0 || value > MAX_DIM {
        return Err(Error::ShotArgs {
            what: format!("--{name} must be 1..={MAX_DIM}, got {value}"),
        });
    }
    Ok(value)
}

fn mode_for(out: &Path) -> Result<OutMode, Error> {
    let Some(ext) = out.extension().and_then(|e| e.to_str()) else {
        return Err(Error::ShotArgs {
            what: "output path needs a .png or .json extension".to_owned(),
        });
    };
    let ext = ext.to_ascii_lowercase();
    if ext == "png" {
        return Ok(OutMode::Png);
    }
    if ext == "json" {
        return Ok(OutMode::Json);
    }
    Err(Error::ShotArgs {
        what: format!("output extension '.{ext}' unsupported — use .png or .json"),
    })
}

fn node_base(e: &Entity) -> f32 {
    if e.image.is_some() {
        IMG_RADIUS
    } else {
        NODE_RADIUS
    }
}

fn screen(p: Pos2, cam: Camera) -> Pos2 {
    Pos2::new(p.x * cam.zoom + cam.x, p.y * cam.zoom + cam.y)
}

fn clamp_zoom(z: f32) -> f32 {
    if !z.is_finite() {
        return 1.0;
    }
    z.clamp(MIN_ZOOM, MAX_ZOOM)
}

fn fit_camera(g: &RsGraph, board: Vec2) -> Camera {
    let mut min = Pos2::new(f32::INFINITY, f32::INFINITY);
    let mut max = Pos2::new(f32::NEG_INFINITY, f32::NEG_INFINITY);
    let mut any = false;
    for idx in g.g().node_indices() {
        let Some(n) = g.node(idx) else { continue };
        let e = n.payload();
        if e.hidden {
            continue;
        }
        let half = node_base(e) * e.size * e.boost;
        let loc = n.location();
        min.x = min.x.min(loc.x - half);
        min.y = min.y.min(loc.y - half);
        max.x = max.x.max(loc.x + half);
        max.y = max.y.max(loc.y + half);
        any = true;
    }
    if !any || board.x < 1.0 || board.y < 1.0 {
        return Camera::default();
    }
    let bbox = Rect::from_min_max(min, max);
    let span = bbox.size();
    let w = span.x.max(1.0);
    let h = span.y.max(1.0);
    let fit = (board.x * MARGIN_FRAC / w).min(board.y * MARGIN_FRAC / h);
    let zoom = clamp_zoom(fit.min(MAX_FIT_ZOOM));
    let pan = board / 2.0 - bbox.center().to_vec2() * zoom;
    Camera {
        x: pan.x,
        y: pan.y,
        zoom,
    }
}

fn layout_pass(g: &mut RsGraph) {
    let idxs: Vec<NodeIndex> = g.g().node_indices().collect();
    if idxs.len() < 2 {
        return;
    }
    let mut pos: Vec<Pos2> = Vec::with_capacity(idxs.len());
    for (i, idx) in idxs.iter().enumerate() {
        let Some(n) = g.node(*idx) else {
            pos.push(Pos2::ZERO);
            continue;
        };
        let jitter = Vec2::new((i % 13) as f32 * 0.31 - 2.0, (i % 7) as f32 * 0.53 - 1.8);
        pos.push(n.location() + jitter);
    }
    let mut lookup: HashMap<NodeIndex, usize> = HashMap::new();
    for (i, idx) in idxs.iter().enumerate() {
        lookup.insert(*idx, i);
    }
    let mut links: Vec<(usize, usize)> = Vec::new();
    for e in g.g().edge_indices() {
        let Some((a, b)) = g.g().edge_endpoints(e) else {
            continue;
        };
        if a == b {
            continue;
        }
        let (Some(&i), Some(&j)) = (lookup.get(&a), lookup.get(&b)) else {
            continue;
        };
        links.push((i, j));
    }
    let center = centroid(&pos);
    let n = pos.len();
    for it in 0..FORCE_ITERS {
        let frac = 1.0 - it as f32 / FORCE_ITERS as f32;
        let temp = FORCE_STEP * 3.0 * frac + 1.0;
        let mut disp = vec![Vec2::ZERO; n];
        for i in 0..n {
            for j in (i + 1)..n {
                let delta = pos[i] - pos[j];
                let dist = delta.length().max(0.01);
                let push = delta * (FORCE_STEP * FORCE_STEP / (dist * dist));
                disp[i] += push;
                disp[j] -= push;
            }
        }
        for (i, j) in &links {
            let delta = pos[*i] - pos[*j];
            let dist = delta.length().max(0.01);
            let pull = delta * (dist / FORCE_STEP);
            disp[*i] -= pull;
            disp[*j] += pull;
        }
        for i in 0..n {
            disp[i] += (center - pos[i]) * FORCE_GRAVITY;
            let len = disp[i].length();
            if len > 0.0 {
                pos[i] += disp[i] / len * len.min(temp);
            }
        }
    }
    for (i, idx) in idxs.iter().enumerate() {
        let Some(node) = g.node_mut(*idx) else {
            continue;
        };
        node.set_location(pos[i]);
    }
}

fn centroid(pos: &[Pos2]) -> Pos2 {
    let mut sum = Vec2::ZERO;
    for p in pos {
        sum += p.to_vec2();
    }
    if pos.is_empty() {
        return Pos2::ZERO;
    }
    (sum / pos.len() as f32).to_pos2()
}

pub(super) struct RenderedFrame {
    pub(super) ctx: egui::Context,
    pub(super) shapes: Vec<egui::epaint::ClippedShape>,
    pub(super) pixels_per_point: f32,
    pub(super) tex_set: Vec<(egui::epaint::TextureId, egui::epaint::ImageDelta)>,
    pub(super) tex_free: Vec<egui::epaint::TextureId>,
}

pub(super) fn run_frames(
    width: u32,
    height: u32,
    cam: Camera,
    g: &mut RsGraph,
    dark: bool,
) -> Result<RenderedFrame, Error> {
    let ctx = egui::Context::default();
    let mut fonts = egui::FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    ctx.set_fonts(fonts);
    ctx.set_theme(if dark {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    });

    let raw = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(
            Pos2::ZERO,
            Vec2::new(width as f32, height as f32),
        )),
        ..Default::default()
    };

    let mut shapes: Vec<egui::epaint::ClippedShape> = Vec::new();
    let mut pixels_per_point = 1.0;
    let mut tex_set: Vec<(egui::epaint::TextureId, egui::epaint::ImageDelta)> = Vec::new();
    let mut tex_free: Vec<egui::epaint::TextureId> = Vec::new();
    let mut produced = false;
    let mut cam_err: Result<(), Error> = Ok(());
    for _ in 0..RENDER_FRAMES {
        let full = ctx.run_ui(raw.clone(), |ui| {
            ui.add(
                &mut GraphView::<_, _, _, _, _, SourcedEdgeShape, FixedState, FixedLayout>::new(g)
                    .with_id(Some(META_ID.to_owned()))
                    .with_interactions(
                        &SettingsInteraction::default()
                            .with_dragging_enabled(false)
                            .with_node_selection_enabled(false)
                            .with_edge_selection_enabled(false),
                    )
                    .with_navigations(
                        &SettingsNavigation::default()
                            .with_fit_to_screen_enabled(false)
                            .with_zoom_and_pan_enabled(false),
                    )
                    .with_styles(&SettingsStyle::default().with_labels_always(true)),
            );
            if let Err(e) = write_camera(ui, cam) {
                cam_err = Err(e);
            }
        });
        pixels_per_point = full.pixels_per_point;
        shapes = full.shapes;
        for entry in full.textures_delta.set {
            tex_set.push(entry);
        }
        for id in full.textures_delta.free {
            tex_free.push(id);
        }
        produced = true;
    }
    cam_err?;
    if !produced {
        return Err(Error::Render {
            what: "no egui frame was produced".to_owned(),
        });
    }
    Ok(RenderedFrame {
        ctx,
        shapes,
        pixels_per_point,
        tex_set,
        tex_free,
    })
}

fn write_camera(ui: &mut egui::Ui, cam: Camera) -> Result<(), Error> {
    let mut mf = MetadataFrame::new(Some(META_ID.to_owned())).load(ui);
    mf.pan = Vec2::new(cam.x, cam.y);
    mf.zoom = clamp_zoom(cam.zoom);
    mf.save(ui);
    Ok(())
}

#[derive(Serialize)]
struct Snapshot {
    width: u32,
    height: u32,
    zoom: f32,
    pan: [f32; 2],
    shape_count: usize,
    nodes: Vec<NodeGeom>,
    edges: Vec<EdgeGeom>,
}

#[derive(Serialize)]
struct NodeGeom {
    slug: String,
    kind: String,
    label: String,
    center: [f32; 2],
    radius: f32,
    image: bool,
    color: [u8; 4],
    label_box: [f32; 4],
}

#[derive(Serialize)]
struct EdgeGeom {
    from: String,
    to: String,
    verb: String,
    sourced: bool,
    status: String,
    sources: usize,
    from_pt: [f32; 2],
    to_pt: [f32; 2],
    color: [u8; 4],
}

fn render_json(g: &mut RsGraph, cam: Camera, opts: &Opts) -> Result<String, Error> {
    let frame = run_frames(opts.width, opts.height, cam, g, opts.dark)?;
    let shape_count = frame.shapes.len();
    let nodes = node_geoms(&frame.ctx, g, cam, opts.dark)?;
    let edges = edge_geoms(g, cam);
    let snap = Snapshot {
        width: opts.width,
        height: opts.height,
        zoom: cam.zoom,
        pan: [cam.x, cam.y],
        shape_count,
        nodes,
        edges,
    };
    let text = serde_json::to_string_pretty(&snap).map_err(|source| Error::JsonSer { source })?;
    std::fs::write(&opts.out, text).map_err(|source| Error::Io {
        path: opts.out.clone(),
        source,
    })?;
    Ok(format!("geometry · {shape_count} egui shapes"))
}

fn node_geoms(
    ctx: &egui::Context,
    g: &RsGraph,
    cam: Camera,
    dark: bool,
) -> Result<Vec<NodeGeom>, Error> {
    let mut out = Vec::new();
    for idx in g.g().node_indices() {
        let Some(n) = g.node(idx) else { continue };
        let e = n.payload();
        if e.hidden {
            continue;
        }
        let vsize = e.size * e.boost;
        let center = screen(n.location(), cam);
        let radius = node_base(e) * vsize * cam.zoom;
        let fpx = (LABEL_SIZE * vsize * cam.zoom).min(MAX_FONT_PX);
        let wrap = LABEL_WRAP * vsize * cam.zoom;
        let gap = LABEL_GAP * vsize * cam.zoom;
        let gsz = measure_label(ctx, &e.label, fpx, wrap);
        let anchor = if e.image.is_some() {
            LabelAnchor::Tile
        } else {
            LabelAnchor::Disc
        };
        let half = if e.image.is_some() {
            IMG_RADIUS * vsize * cam.zoom
        } else {
            radius
        };
        let lbox = label_box(anchor, center, half, gsz, gap, fpx)?;
        let color = board_color(KIND_META[e.kind as usize].color, dark)?;
        out.push(NodeGeom {
            slug: e.slug.clone(),
            kind: KIND_META[e.kind as usize].label.to_owned(),
            label: e.label.clone(),
            center: [center.x, center.y],
            radius,
            image: e.image.is_some(),
            color: [color.r(), color.g(), color.b(), color.a()],
            label_box: [lbox.min.x, lbox.min.y, lbox.max.x, lbox.max.y],
        });
    }
    Ok(out)
}

fn edge_geoms(g: &RsGraph, cam: Camera) -> Vec<EdgeGeom> {
    let mut out = Vec::new();
    for e in g.g().edge_indices() {
        let Some((a, b)) = g.g().edge_endpoints(e) else {
            continue;
        };
        let Some(ed) = g.edge(e) else { continue };
        let r = ed.payload();
        if r.hidden {
            continue;
        }
        let Some(na) = g.node(a) else { continue };
        let Some(nb) = g.node(b) else { continue };
        let ea = na.payload();
        let eb = nb.payload();
        if ea.hidden || eb.hidden {
            continue;
        }
        let ca = screen(na.location(), cam);
        let cb = screen(nb.location(), cam);
        let ra = node_base(ea) * ea.size * ea.boost * cam.zoom;
        let rb = node_base(eb) * eb.size * eb.boost * cam.zoom;
        let raw = cb - ca;
        let dir = if raw.length() > 0.0 {
            raw.normalized()
        } else {
            Vec2::new(1.0, 0.0)
        };
        let from_pt = ca + dir * ra;
        let to_pt = cb - dir * rb;
        let color = edge_color(r.status);
        out.push(EdgeGeom {
            from: ea.slug.clone(),
            to: eb.slug.clone(),
            verb: RELATION_META[r.kind as usize].label.to_owned(),
            sourced: !r.sources.is_empty(),
            status: status_name(r.status).to_owned(),
            sources: r.sources.len(),
            from_pt: [from_pt.x, from_pt.y],
            to_pt: [to_pt.x, to_pt.y],
            color,
        });
    }
    out
}

fn measure_label(ctx: &egui::Context, text: &str, fpx: f32, wrap: f32) -> Vec2 {
    ctx.fonts_mut(|f| {
        let mut job = LayoutJob::simple(
            text.to_owned(),
            FontId::new(fpx, FontFamily::Monospace),
            Color32::WHITE,
            wrap,
        );
        job.halign = Align::Center;
        f.layout_job(job).size()
    })
}

fn status_name(v: Verification) -> &'static str {
    match v {
        Verification::Unreviewed => "unreviewed",
        Verification::MachineSuggested => "machine_suggested",
        Verification::AnalystAsserted => "analyst_asserted",
        Verification::Corroborated => "corroborated",
        Verification::Rejected => "rejected",
    }
}

fn edge_color(v: Verification) -> [u8; 4] {
    let (r, g, b) = match v {
        Verification::Rejected => (235, 80, 80),
        Verification::Unreviewed => (235, 150, 70),
        Verification::MachineSuggested => (230, 200, 90),
        Verification::AnalystAsserted => (110, 190, 200),
        Verification::Corroborated => (120, 200, 140),
    };
    [r, g, b, 255]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_case() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/ui/testkit/fixtures/sample_case")
    }

    fn tmp(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("redstring_shot_{}_{name}", std::process::id()))
    }

    fn args_for(out: &Path, w: &str, h: &str) -> Vec<String> {
        vec![
            "--shot".to_owned(),
            fixture_case().display().to_string(),
            "--out".to_owned(),
            out.display().to_string(),
            "--width".to_owned(),
            w.to_owned(),
            "--height".to_owned(),
            h.to_owned(),
        ]
    }

    #[test]
    fn json_shot_writes_geometry_with_expected_counts() {
        let out = tmp("board.json");
        let _ = std::fs::remove_file(&out);
        let report = shoot(&args_for(&out, "1200", "800")).unwrap();
        assert_eq!(report.mode, "json");
        assert_eq!(report.nodes, 3);
        assert_eq!(report.edges, 2);

        let text = std::fs::read_to_string(&out).unwrap();
        assert!(!text.is_empty(), "json output must be non-empty");
        let snap: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(snap["nodes"].as_array().unwrap().len(), 3);
        assert_eq!(snap["edges"].as_array().unwrap().len(), 2);
        assert!(
            snap["shape_count"].as_u64().unwrap() > 0,
            "the board frame must produce egui shapes"
        );

        let edges = snap["edges"].as_array().unwrap();
        let sourced = edges
            .iter()
            .filter(|e| e["sourced"].as_bool().unwrap_or(false))
            .count();
        assert_eq!(sourced, 1, "one edge is sourced, one is not");

        for node in snap["nodes"].as_array().unwrap() {
            let lb = node["label_box"].as_array().unwrap();
            assert!(lb[3].as_f64().unwrap() >= lb[1].as_f64().unwrap());
        }
        let _ = std::fs::remove_file(&out);
    }

    fn args_theme(out: &Path, theme: &str) -> Vec<String> {
        vec![
            "--shot".to_owned(),
            fixture_case().display().to_string(),
            "--out".to_owned(),
            out.display().to_string(),
            "--width".to_owned(),
            "1000".to_owned(),
            "--height".to_owned(),
            "700".to_owned(),
            "--theme".to_owned(),
            theme.to_owned(),
        ]
    }

    fn color_lum(c: &serde_json::Value) -> f64 {
        let a = c.as_array().unwrap();
        0.299 * a[0].as_f64().unwrap()
            + 0.587 * a[1].as_f64().unwrap()
            + 0.114 * a[2].as_f64().unwrap()
    }

    #[test]
    fn light_theme_darkens_node_colors_in_json() {
        let dark_out = tmp("theme_dark.json");
        let light_out = tmp("theme_light.json");
        let _ = std::fs::remove_file(&dark_out);
        let _ = std::fs::remove_file(&light_out);
        shoot(&args_theme(&dark_out, "dark")).unwrap();
        shoot(&args_theme(&light_out, "light")).unwrap();
        let dv: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&dark_out).unwrap()).unwrap();
        let lv: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&light_out).unwrap()).unwrap();
        let dnodes = dv["nodes"].as_array().unwrap();
        let lnodes = lv["nodes"].as_array().unwrap();
        assert_eq!(dnodes.len(), lnodes.len());
        let mut compared = 0usize;
        for (dn, ln) in dnodes.iter().zip(lnodes.iter()) {
            assert_eq!(dn["slug"], ln["slug"]);
            let dl = color_lum(&dn["color"]);
            let ll = color_lum(&ln["color"]);
            assert!(
                ll < dl - 20.0,
                "light theme must darken node {} color for legibility: dark lum {dl} vs light lum {ll}",
                dn["slug"]
            );
            compared += 1;
        }
        assert!(compared >= 3, "sample case should carry >=3 nodes, compared {compared}");
        let _ = std::fs::remove_file(&dark_out);
        let _ = std::fs::remove_file(&light_out);
    }

    #[test]
    fn bad_theme_value_is_rejected() {
        let out = tmp("bad_theme.json");
        let Err(err) = shoot(&args_theme(&out, "solarized")) else {
            panic!("a bad --theme value must be rejected");
        };
        let msg = format!("{err}");
        assert!(msg.contains("theme"), "error should mention the theme flag, got {msg}");
    }

    #[test]
    fn png_shot_renders_when_adapter_available() {
        let out = tmp("board.png");
        let _ = std::fs::remove_file(&out);
        match shoot(&args_for(&out, "800", "600")) {
            Ok(report) => {
                assert_eq!(report.mode, "png");
                assert!(!report.backend.is_empty(), "png report names the backend");
                let bytes = std::fs::read(&out).unwrap();
                assert!(bytes.len() > 1000, "png should be non-trivial, got {}", bytes.len());
                assert_eq!(
                    &bytes[..8],
                    &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
                    "PNG magic bytes"
                );
                let img = image::open(&out).unwrap();
                assert_eq!((img.width(), img.height()), (800, 600));
            }
            Err(err) => {
                eprintln!("png shot skipped — no usable wgpu adapter in this environment: {err}");
            }
        }
        let _ = std::fs::remove_file(&out);
    }
}

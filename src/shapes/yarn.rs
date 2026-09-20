use egui::epaint::{Mesh, Vertex};
use egui::{Color32, ColorImage, Pos2, Shape, Stroke, TextureHandle, TextureOptions, Vec2};
use egui_graphs::{DisplayNode, MetadataFrame, Node};
use petgraph::{EdgeType, stable_graph::IndexType};

use crate::error::Error;
use crate::model::Relation;

pub const MIN_YARN_PX: f32 = 3.5;
pub const YARN_WIDTH_MULT: f32 = 2.0;

pub fn yarn_red(dark: bool) -> Result<Color32, Error> {
    if dark {
        return Ok(Color32::from_rgb(214, 48, 38));
    }
    Ok(Color32::from_rgb(176, 24, 16))
}

const TILE_PNG: &[u8] = include_bytes!("../../assets/yarn_tile.png");
const HAIR_SPAN: f32 = 3.0;
const SCALLOP: f32 = 0.05;
const WOBBLE_FRAC: f32 = 0.15;
const WOBBLE_MAX: f32 = 1.6;
const WOBBLE_WL_A: f32 = 118.0;
const WOBBLE_WL_B: f32 = 261.0;
const SCALLOP_WL_SPANS: f32 = 1.9;
const SAG_FRAC: f32 = 0.085;
const SAG_MAX: f32 = 46.0;
const SEG_LEN: f32 = 11.0;
const SEG_MIN: usize = 8;
const SEG_MAX: usize = 56;
const DENSE_CAP: usize = 1200;
const TEX_ID: &str = "redstring_yarn_tile";
const TIP_ANGLE: f32 = std::f32::consts::TAU / 30.0;
const TIP_SIZE: f32 = 12.0;
const PARALLEL_SPACING: f32 = 12.0;

fn seed_hash(pts: &[Pos2]) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    let ends = [pts[0], pts[pts.len() - 1]];
    for p in ends {
        for v in [p.x, p.y] {
            for b in ((v * 8.0) as i64).to_le_bytes() {
                h ^= b as u64;
                h = h.wrapping_mul(0x100000001b3);
            }
        }
    }
    h
}

fn seed_phase(seed: u64, lane: u64) -> f32 {
    let mixed = seed.wrapping_mul(lane.wrapping_mul(2) + 0x9e3779b97f4a7c15);
    ((mixed >> 40) as f32 / (1u64 << 24) as f32) * std::f32::consts::TAU
}

fn tile_source() -> Result<image::RgbaImage, Error> {
    let img = image::load_from_memory(TILE_PNG).map_err(|source| Error::Image {
        path: "builtin:yarn_tile".into(),
        source,
    })?;
    Ok(img.to_rgba8())
}

fn mip_level(ribbon_px: f32, base_h: u32) -> u32 {
    let mut level = 0u32;
    while level < 3 && (base_h >> (level + 1)) as f32 >= ribbon_px {
        level += 1;
    }
    level
}

fn tile_texture(ctx: &egui::Context, ribbon_px: f32) -> Result<TextureHandle, Error> {
    let src = tile_source()?;
    let level = mip_level(ribbon_px, src.height());
    let id = egui::Id::new((TEX_ID, level));
    let Some(hit) = ctx.data(|d| d.get_temp::<TextureHandle>(id)) else {
        let (w, h) = (
            (src.width() >> level).max(1),
            (src.height() >> level).max(1),
        );
        let scaled = image::imageops::resize(&src, w, h, image::imageops::FilterType::Lanczos3);
        let ci = ColorImage::from_rgba_unmultiplied([w as usize, h as usize], scaled.as_raw());
        let tex = ctx.load_texture(
            format!("{TEX_ID}_{level}"),
            ci,
            TextureOptions::LINEAR_REPEAT,
        );
        ctx.data_mut(|d| d.insert_temp(id, tex.clone()));
        return Ok(tex);
    };
    Ok(hit)
}

pub fn endpoints<N, Ty, Ix, D>(
    start: &Node<N, Relation, Ty, Ix, D>,
    end: &Node<N, Relation, Ty, Ix, D>,
    order: usize,
) -> Result<(Pos2, Pos2), Error>
where
    N: Clone,
    Ty: EdgeType,
    Ix: IndexType,
    D: DisplayNode<N, Relation, Ty, Ix>,
{
    let dir = (end.location() - start.location()).normalized();
    let s = start.display().closest_boundary_point(dir);
    let e = end.display().closest_boundary_point(-dir);
    if order == 0 {
        return Ok((s, e));
    }
    let span = (e - s).normalized();
    let off = Vec2::new(-span.y, span.x) * (order as f32) * PARALLEL_SPACING;
    Ok((s + off, e + off))
}

fn rotate(v: Vec2, angle: f32) -> Vec2 {
    let (c, s) = (angle.cos(), angle.sin());
    Vec2::new(c * v.x - s * v.y, s * v.x + c * v.y)
}

pub fn tip_shape(
    meta: &MetadataFrame,
    path: &[Pos2],
    mult: f32,
    color: Color32,
) -> Result<Shape, Error> {
    let [.., prev, e] = path else {
        return Ok(Shape::Noop);
    };
    let dir = (*e - *prev).normalized();
    let t1 = *e - rotate(dir, TIP_ANGLE) * TIP_SIZE * mult;
    let t2 = *e - rotate(dir, -TIP_ANGLE) * TIP_SIZE * mult;
    let tip = vec![
        meta.canvas_to_screen_pos(*e),
        meta.canvas_to_screen_pos(t1),
        meta.canvas_to_screen_pos(t2),
    ];
    Ok(Shape::convex_polygon(tip, color, Stroke::NONE))
}

pub fn thread_path(s: Pos2, e: Pos2, drape: bool) -> Result<Vec<Pos2>, Error> {
    if !drape {
        return Ok(vec![s, s.lerp(e, 0.5), e]);
    }
    let dist = (e - s).length();
    let sag = (dist * SAG_FRAC).min(SAG_MAX);
    let ctrl = s + (e - s) * 0.5 + Vec2::Y * sag;
    let n = ((dist / SEG_LEN) as usize).clamp(SEG_MIN, SEG_MAX);
    let mut pts = Vec::with_capacity(n + 1);
    for i in 0..=n {
        let t = i as f32 / n as f32;
        pts.push(s.lerp(ctrl, t).lerp(ctrl.lerp(e, t), t));
    }
    Ok(pts)
}

fn seg_dist(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let len2 = ab.length_sq();
    if len2 == 0.0 {
        return (p - a).length();
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    (p - (a + ab * t)).length()
}

pub fn dist_to_path(p: Pos2, pts: &[Pos2]) -> Result<f32, Error> {
    match pts {
        [] => Ok(f32::MAX),
        [only] => Ok((p - *only).length()),
        many => {
            let mut best = f32::MAX;
            for w in many.windows(2) {
                best = best.min(seg_dist(p, w[0], w[1]));
            }
            Ok(best)
        }
    }
}

fn point_dir(pts: &[Pos2], i: usize) -> Vec2 {
    let a = if i == 0 { pts[0] } else { pts[i - 1] };
    let b = if i + 1 == pts.len() {
        pts[i]
    } else {
        pts[i + 1]
    };
    let d = b - a;
    if d.length_sq() <= f32::EPSILON {
        return Vec2::X;
    }
    d.normalized()
}

fn path_length(pts: &[Pos2]) -> f32 {
    let mut total = 0.0f32;
    for w in pts.windows(2) {
        total += (w[1] - w[0]).length();
    }
    total
}

fn resample(pts: &[Pos2], n: usize) -> Vec<Pos2> {
    let total = path_length(pts);
    if total <= f32::EPSILON || n < 2 {
        return pts.to_vec();
    }
    let step = total / (n - 1) as f32;
    let mut out = Vec::with_capacity(n);
    out.push(pts[0]);
    let mut seg = 0usize;
    let mut seg_start = 0.0f32;
    for k in 1..n - 1 {
        let target = step * k as f32;
        while seg + 2 < pts.len() && seg_start + (pts[seg + 1] - pts[seg]).length() < target {
            seg_start += (pts[seg + 1] - pts[seg]).length();
            seg += 1;
        }
        let len = (pts[seg + 1] - pts[seg]).length();
        let t = if len <= f32::EPSILON {
            0.0
        } else {
            ((target - seg_start) / len).clamp(0.0, 1.0)
        };
        out.push(pts[seg].lerp(pts[seg + 1], t));
    }
    out.push(pts[pts.len() - 1]);
    out
}

pub fn yarn_shape(
    ctx: &egui::Context,
    meta: &MetadataFrame,
    pts: &[Pos2],
    width: f32,
    tint: Color32,
) -> Result<Shape, Error> {
    if pts.len() < 2 || width <= 0.0 {
        return Ok(Shape::Noop);
    }
    let ribbon = width * HAIR_SPAN;
    let tex = tile_texture(ctx, meta.canvas_to_screen_size(ribbon))?;
    let tsize = tex.size();
    let tile_len = ribbon * (tsize[0] as f32 / tsize[1] as f32);
    let total = path_length(pts);
    let zoom = meta.zoom.max(0.01);
    let step = (width * 0.45).max(1.0 / zoom);
    let want = ((total / step).ceil() as usize + 1).clamp(pts.len(), DENSE_CAP);
    let dense = resample(pts, want);
    let seed = seed_hash(pts);
    let (p1, p2, p3) = (
        seed_phase(seed, 1),
        seed_phase(seed, 2),
        seed_phase(seed, 3),
    );
    let u0 = seed_phase(seed, 4) / std::f32::consts::TAU * (tsize[0] as f32 / tsize[1] as f32);
    let amp = (width * WOBBLE_FRAC).min(WOBBLE_MAX);
    let tau = std::f32::consts::TAU;
    let hw = ribbon * 0.5;
    let mut mesh = Mesh::with_texture(tex.id());
    let mut run = 0.0f32;
    for (i, p) in dense.iter().enumerate() {
        let dir = point_dir(&dense, i);
        let perp_dir = Vec2::new(-dir.y, dir.x);
        if i > 0 {
            run += (*p - dense[i - 1]).length();
        }
        let envelope = (run.min(total - run) / (width * 4.0)).clamp(0.0, 1.0);
        let sway =
            (tau * run / WOBBLE_WL_A + p1).sin() + 0.6 * (tau * run / WOBBLE_WL_B + p2).sin();
        let center = *p + perp_dir * (amp * envelope * sway);
        let bulge = 1.0 + SCALLOP * (tau * run / (ribbon * SCALLOP_WL_SPANS) + p3).sin();
        let perp = perp_dir * hw * bulge;
        let u = u0 + run / tile_len;
        let base = mesh.vertices.len() as u32;
        mesh.vertices.push(Vertex {
            pos: meta.canvas_to_screen_pos(center + perp),
            uv: Pos2::new(u, 0.0),
            color: tint,
        });
        mesh.vertices.push(Vertex {
            pos: meta.canvas_to_screen_pos(center - perp),
            uv: Pos2::new(u, 1.0),
            color: tint,
        });
        if i > 0 {
            mesh.indices
                .extend_from_slice(&[base - 2, base - 1, base, base - 1, base + 1, base]);
        }
    }
    Ok(Shape::mesh(mesh))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_photo_tile_decodes_into_a_long_strip() {
        let img = tile_source().expect("the embedded yarn photo must decode");
        assert!(
            img.height() >= 64,
            "the master tile must keep fiber detail, got h={}",
            img.height()
        );
        assert!(
            img.width() >= img.height() * 8,
            "the repeat must be long so the eye cannot catch the period: {}x{}",
            img.width(),
            img.height()
        );
    }

    #[test]
    fn the_photo_tile_has_hairy_margins_and_an_opaque_core() {
        let img = tile_source().unwrap();
        let (w, h) = (img.width(), img.height());
        let row_avg = |y: u32| -> f32 {
            let mut sum = 0.0f32;
            for x in 0..w {
                sum += img.get_pixel(x, y).0[3] as f32;
            }
            sum / w as f32
        };
        assert!(
            row_avg(0) < 40.0,
            "top margin must be mostly air, got {}",
            row_avg(0)
        );
        assert!(
            row_avg(h - 1) < 40.0,
            "bottom margin must be mostly air, got {}",
            row_avg(h - 1)
        );
        let mid = row_avg(h / 2);
        assert!(
            mid > 200.0,
            "the core row must be near-opaque thread, got {mid}"
        );
    }

    #[test]
    fn the_photo_tile_seam_wraps_cleanly() {
        let img = tile_source().unwrap();
        let (w, h) = (img.width(), img.height());
        let mut diff = 0.0f32;
        for y in 0..h {
            let a = img.get_pixel(0, y).0;
            let b = img.get_pixel(w - 1, y).0;
            for c in 0..3 {
                let pa = a[c] as f32 * a[3] as f32 / 255.0;
                let pb = b[c] as f32 * b[3] as f32 / 255.0;
                diff += (pa - pb).abs();
            }
        }
        diff /= (h * 3) as f32;
        assert!(
            diff < 14.0,
            "the crossfaded seam must be invisible when the tile repeats, avg premul diff {diff}"
        );
    }

    #[test]
    fn the_mip_ladder_matches_the_ribbon_height() {
        assert_eq!(
            mip_level(100.0, 96),
            0,
            "a fat ribbon takes the full-res tile"
        );
        assert_eq!(mip_level(40.0, 96), 1);
        assert_eq!(mip_level(20.0, 96), 2);
        assert_eq!(
            mip_level(5.0, 96),
            3,
            "the ladder bottoms out instead of vanishing"
        );
    }

    #[test]
    fn the_wobble_is_deterministic_and_pins_the_endpoints() {
        let ctx = egui::Context::default();
        let pts = thread_path(Pos2::new(3.0, 7.0), Pos2::new(240.0, 40.0), true).unwrap();
        let Shape::Mesh(a) =
            yarn_shape(&ctx, &MetadataFrame::default(), &pts, 6.0, Color32::WHITE).unwrap()
        else {
            panic!("expected a mesh");
        };
        let Shape::Mesh(b) =
            yarn_shape(&ctx, &MetadataFrame::default(), &pts, 6.0, Color32::WHITE).unwrap()
        else {
            panic!("expected a mesh");
        };
        assert_eq!(a.vertices.len(), b.vertices.len());
        for (va, vb) in a.vertices.iter().zip(b.vertices.iter()) {
            assert_eq!(
                va.pos, vb.pos,
                "the same thread must wobble identically frame to frame"
            );
        }
        let first = (a.vertices[0].pos.to_vec2() + a.vertices[1].pos.to_vec2()) * 0.5;
        let n = a.vertices.len();
        let last = (a.vertices[n - 2].pos.to_vec2() + a.vertices[n - 1].pos.to_vec2()) * 0.5;
        assert!(
            (first - pts[0].to_vec2()).length() < 1e-3,
            "the thread must stay pinned at its start node"
        );
        assert!(
            (last - pts[pts.len() - 1].to_vec2()).length() < 1e-3,
            "the thread must stay pinned at its end node"
        );
    }

    #[test]
    fn an_undraped_thread_is_a_straight_three_point_chord() {
        let s = Pos2::new(10.0, 10.0);
        let e = Pos2::new(110.0, 50.0);
        let pts = thread_path(s, e, false).unwrap();
        assert_eq!(pts.len(), 3, "normal edges stay straight lines");
        assert_eq!(pts[0], s);
        assert_eq!(pts[2], e);
        assert_eq!(
            pts[1],
            s.lerp(e, 0.5),
            "the middle point anchors the label at the true midpoint"
        );
    }

    #[test]
    fn resampling_spreads_points_evenly_and_keeps_the_pins() {
        let pts = vec![Pos2::ZERO, Pos2::new(30.0, 0.0), Pos2::new(100.0, 0.0)];
        let dense = resample(&pts, 11);
        assert_eq!(dense.len(), 11);
        assert_eq!(dense[0], pts[0]);
        assert_eq!(dense[10], pts[2]);
        for (i, p) in dense.iter().enumerate() {
            assert!(
                (p.x - 10.0 * i as f32).abs() < 1e-3,
                "arc-length resampling must space points uniformly, point {i} at {}",
                p.x
            );
        }
    }

    #[test]
    fn the_thread_sags_between_its_pins_and_never_leaves_them() {
        let s = Pos2::new(0.0, 0.0);
        let e = Pos2::new(300.0, 0.0);
        let pts = thread_path(s, e, true).unwrap();
        assert_eq!(pts[0], s, "the thread must start at the first pin");
        assert_eq!(
            pts[pts.len() - 1],
            e,
            "the thread must end at the second pin"
        );
        let apex = pts[pts.len() / 2];
        assert!(
            apex.y > 8.0,
            "a 300-unit thread must visibly sag below the straight line, apex y was {}",
            apex.y
        );
        assert!(
            apex.y <= SAG_MAX,
            "sag must stay clamped so long threads do not drape off the board"
        );
        let short = thread_path(Pos2::ZERO, Pos2::new(20.0, 0.0), true).unwrap();
        let short_apex = short[short.len() / 2];
        assert!(
            short_apex.y < apex.y,
            "a short thread must sag less than a long one"
        );
    }

    #[test]
    fn segment_count_scales_with_length_inside_bounds() {
        let short = thread_path(Pos2::ZERO, Pos2::new(1.0, 0.0), true).unwrap();
        let long = thread_path(Pos2::ZERO, Pos2::new(100000.0, 0.0), true).unwrap();
        assert_eq!(short.len(), SEG_MIN + 1);
        assert_eq!(long.len(), SEG_MAX + 1);
    }

    #[test]
    fn distance_to_a_straight_path_matches_segment_distance() {
        let pts = vec![Pos2::ZERO, Pos2::new(50.0, 0.0), Pos2::new(100.0, 0.0)];
        let d = dist_to_path(Pos2::new(50.0, 7.0), &pts).unwrap();
        assert!((d - 7.0).abs() < 1e-4, "expected 7.0, got {d}");
        assert_eq!(dist_to_path(Pos2::ZERO, &[]).unwrap(), f32::MAX);
        let solo = dist_to_path(Pos2::new(3.0, 4.0), &[Pos2::ZERO]).unwrap();
        assert!((solo - 5.0).abs() < 1e-4);
    }

    #[test]
    fn the_yarn_mesh_is_a_textured_ribbon_with_advancing_uv() {
        let ctx = egui::Context::default();
        let pts = thread_path(Pos2::ZERO, Pos2::new(200.0, 0.0), true).unwrap();
        let shape = yarn_shape(&ctx, &MetadataFrame::default(), &pts, 5.0, Color32::RED).unwrap();
        let Shape::Mesh(mesh) = shape else {
            panic!("a corroborated thread must tessellate to a textured mesh");
        };
        assert!(
            mesh.vertices.len() >= pts.len() * 2,
            "densification must add silhouette samples, not drop them"
        );
        assert_eq!(mesh.indices.len(), (mesh.vertices.len() / 2 - 1) * 6);
        assert_ne!(
            mesh.texture_id,
            egui::TextureId::default(),
            "the ribbon must bind the yarn tile, not the font atlas"
        );
        let mut last_u = -1.0f32;
        for pair in mesh.vertices.chunks(2) {
            assert!(
                pair[0].uv.x >= last_u,
                "uv must advance monotonically along the thread"
            );
            last_u = pair[0].uv.x;
            assert_eq!(pair[0].uv.y, 0.0);
            assert_eq!(pair[1].uv.y, 1.0);
            assert_eq!(
                pair[0].color,
                Color32::RED,
                "the tint must ride every vertex"
            );
        }
        assert!(
            last_u > 1.0,
            "a 200px thread at width 5 must repeat the tile at least once"
        );
    }

    #[test]
    fn pan_and_zoom_never_re_twist_the_thread() {
        let ctx = egui::Context::default();
        let pts = thread_path(Pos2::new(3.0, 7.0), Pos2::new(240.0, 40.0), true).unwrap();
        let home = MetadataFrame::default();
        let mut moved = MetadataFrame::default();
        moved.pan = Vec2::new(500.0, -230.0);
        let Shape::Mesh(a) = yarn_shape(&ctx, &home, &pts, 6.0, Color32::WHITE).unwrap() else {
            panic!("expected a mesh");
        };
        let Shape::Mesh(b) = yarn_shape(&ctx, &moved, &pts, 6.0, Color32::WHITE).unwrap() else {
            panic!("expected a mesh");
        };
        assert_eq!(a.vertices.len(), b.vertices.len());
        for (va, vb) in a.vertices.iter().zip(b.vertices.iter()) {
            assert_eq!(
                va.uv, vb.uv,
                "panning the camera must never crawl the fiber texture"
            );
            let d = vb.pos - va.pos;
            assert!(
                (d.x - 500.0).abs() < 1e-3 && (d.y + 230.0).abs() < 1e-3,
                "pan must translate the ribbon rigidly, got delta {d:?}"
            );
        }
        let mut zoomed = MetadataFrame::default();
        zoomed.zoom = 2.0;
        let Shape::Mesh(z) = yarn_shape(&ctx, &zoomed, &pts, 6.0, Color32::WHITE).unwrap() else {
            panic!("expected a mesh");
        };
        let mut ua: Vec<f32> = a.vertices.iter().map(|v| v.uv.x).collect();
        let mut uz: Vec<f32> = z.vertices.iter().map(|v| v.uv.x).collect();
        ua.dedup();
        uz.dedup();
        assert_eq!(
            (ua[0], *ua.last().unwrap()),
            (uz[0], *uz.last().unwrap()),
            "zoom must not slide the texture along the thread"
        );
    }

    #[test]
    fn the_silhouette_scallops_like_a_twisted_cord() {
        let ctx = egui::Context::default();
        let pts = thread_path(Pos2::ZERO, Pos2::new(200.0, 0.0), true).unwrap();
        let Shape::Mesh(mesh) =
            yarn_shape(&ctx, &MetadataFrame::default(), &pts, 6.0, Color32::RED).unwrap()
        else {
            panic!("expected a mesh");
        };
        let ribbon = 6.0 * HAIR_SPAN;
        let mut lo = f32::MAX;
        let mut hi = 0.0f32;
        for pair in mesh.vertices.chunks(2) {
            let w = (pair[0].pos - pair[1].pos).length();
            lo = lo.min(w);
            hi = hi.max(w);
        }
        assert!(
            hi > lo + 0.3,
            "the ribbon width must breathe so the edge is not laser-straight: {lo}..{hi}"
        );
        assert!(
            hi <= ribbon * (1.0 + SCALLOP) + 1e-3,
            "the bulge must stay bounded"
        );
        assert!(
            lo >= ribbon * (1.0 - SCALLOP) - 1e-3,
            "the pinch must stay bounded"
        );
    }

    #[test]
    fn degenerate_input_degrades_to_a_noop_shape() {
        let ctx = egui::Context::default();
        let one = yarn_shape(
            &ctx,
            &MetadataFrame::default(),
            &[Pos2::ZERO],
            5.0,
            Color32::RED,
        )
        .unwrap();
        assert!(matches!(one, Shape::Noop));
        let flat = yarn_shape(
            &ctx,
            &MetadataFrame::default(),
            &[Pos2::ZERO, Pos2::new(10.0, 0.0)],
            0.0,
            Color32::RED,
        )
        .unwrap();
        assert!(matches!(flat, Shape::Noop));
    }

    #[test]
    fn lod_constants_keep_the_fallback_meaningful() {
        assert!(
            MIN_YARN_PX > 0.0,
            "there must be a zoom floor under which the stroke fallback runs"
        );
        assert!(
            YARN_WIDTH_MULT > 1.0,
            "yarn must render wider than the plain stroke so fibers resolve"
        );
    }
}

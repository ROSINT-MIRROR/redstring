//! Custom node renderer. An entity with a loadable `image` (mugshot, logo, …)
//! is drawn as a rounded-rect photo tile — the pinned picture on the corkboard.
//! An imageless entity is a big kind-coloured icon glyph on a tinted disc with
//! the entity name underneath; `DefaultNodeShape` survives only as shared
//! state + circle hit-geometry.
//!
//! Geometry contract: `is_inside` / `closest_boundary_point` always describe
//! whichever shape is actually drawn. The photo tile only becomes the active
//! geometry once its texture has resolved (`tex_ok`), so a missing/corrupt
//! file degrades to the default circle — silently, never a panic.
//!
//! RECONCILE(edgedrag): is_inside reflects the drawn geometry (photo rect when
//! the texture resolved, DefaultNodeShape circle otherwise) — right-drag
//! hit-testing can trust it as-is.
//! RECONCILE(camera): all geometry lives in canvas space; screen projection
//! goes through ctx.meta only, so zoom/pan changes need nothing here.

use egui::epaint::{CircleShape, RectShape, TextShape};
use egui::{
    Color32, ColorImage, CornerRadius, FontFamily, FontId, Pos2, Rect, Shape, Stroke, StrokeKind,
    TextureHandle, TextureOptions, Vec2,
};
use egui_graphs::{DefaultNodeShape, DisplayNode, DrawContext, NodeProps};
use petgraph::{stable_graph::IndexType, EdgeType};

use crate::model::Entity;

/// Half-extent of the square photo tile, canvas units — ~2× the
/// `DefaultNodeShape` radius (5.0) so the picture reads at board zoom.
pub const IMG_RADIUS: f32 = 10.0;
/// Corner rounding of the tile, canvas units.
const IMG_CORNER: f32 = 3.0;
/// Border width around the tile, canvas units (thicker when interacted).
const BORDER: f32 = 1.0;
const BORDER_ACTIVE: f32 = 2.0;
/// Hit/boundary radius of an imageless node, canvas units — sized to roughly
/// cover the big icon glyph (stock DefaultNodeShape is 5.0).
pub const NODE_RADIUS: f32 = 8.0;
/// Icon glyph font size, canvas units — 3× the stock label size (5.0).
const ICON_SIZE: f32 = 15.0;
/// Label font size, canvas units — 2× the stock label size (5.0).
const LABEL_SIZE: f32 = 10.0;
const LABEL_GAP: f32 = 2.0;
/// Decoded images are downscaled to fit this square before GPU upload — a
/// node is ~20 canvas units, so 512px survives any sane zoom.
const MAX_TEX_SIDE: u32 = 512;

#[derive(Clone)]
pub struct EntityNodeShape {
    /// The stock shape — fallback rendering *and* shared state (pos, color,
    /// label, interaction flags are all pub on it; we read them from here).
    circle: DefaultNodeShape,
    /// Image path from the entity payload.
    image: Option<String>,
    /// Sticky "the texture actually resolved" flag, set by `shapes()` and
    /// reset when the path changes. Gates the photo-rect geometry so
    /// hit-testing/anchoring never claim a picture that isn't drawn.
    tex_ok: bool,
    /// Kind icon glyph and entity name, drawn separately (the combined
    /// `label_text` only survives on the photo-tile caption).
    icon: String,
    name: String,
    /// Per-node scale multiplier from the payload — every metric (geometry,
    /// glyph, text) multiplies by it, so resize grows the whole node.
    size: f32,
}

impl From<NodeProps<Entity>> for EntityNodeShape {
    fn from(props: NodeProps<Entity>) -> Self {
        let mut circle = DefaultNodeShape::from(props.clone());
        circle.radius = NODE_RADIUS * props.payload.size;
        Self {
            image: props.payload.image.clone(),
            icon: props.payload.kind.icon().to_owned(),
            name: props.payload.label.clone(),
            size: props.payload.size,
            circle,
            tex_ok: false,
        }
    }
}

impl<E: Clone, Ty: EdgeType, Ix: IndexType> DisplayNode<Entity, E, Ty, Ix> for EntityNodeShape {
    fn closest_boundary_point(&self, dir: Vec2) -> Pos2 {
        if self.image_active() {
            rect_boundary_point(self.circle.pos, IMG_RADIUS * self.size, dir)
        } else {
            <DefaultNodeShape as DisplayNode<Entity, E, Ty, Ix>>::closest_boundary_point(
                &self.circle,
                dir,
            )
        }
    }

    fn shapes(&mut self, ctx: &DrawContext) -> Vec<Shape> {
        if let Some(path) = self.image.clone()
            && let Some(tex) = texture_for(ctx.ctx, &path)
        {
            self.tex_ok = true;
            return self.image_shapes(ctx, &tex);
        }
        // no image set, or the file is missing/unreadable → big icon node
        self.tex_ok = false;
        self.icon_shapes(ctx)
    }

    fn update(&mut self, state: &NodeProps<Entity>) {
        if self.image != state.payload.image {
            self.image = state.payload.image.clone();
            self.tex_ok = false; // re-resolve on the next draw
        }
        self.icon = state.payload.kind.icon().to_owned();
        self.name = state.payload.label.clone();
        self.size = state.payload.size;
        <DefaultNodeShape as DisplayNode<Entity, E, Ty, Ix>>::update(&mut self.circle, state);
        // update() rebuilds circle state from props — re-apply the scaled radius
        self.circle.radius = NODE_RADIUS * self.size;
    }

    fn is_inside(&self, pos: Pos2) -> bool {
        if self.image_active() {
            // plain rect test: at IMG_CORNER rounding the corner error is
            // ~1.5 canvas units — well under interaction tolerances
            self.rect().contains(pos)
        } else {
            <DefaultNodeShape as DisplayNode<Entity, E, Ty, Ix>>::is_inside(&self.circle, pos)
        }
    }
}

impl EntityNodeShape {
    fn image_active(&self) -> bool {
        self.image.is_some() && self.tex_ok
    }

    /// The photo tile in canvas coordinates.
    fn rect(&self) -> Rect {
        Rect::from_center_size(self.circle.pos, Vec2::splat(IMG_RADIUS * 2.0 * self.size))
    }

    fn interacted(&self) -> bool {
        self.circle.selected || self.circle.dragged || self.circle.hovered
    }

    /// Border/label colour: the entity kind's colour pushed by
    /// `sync_visuals` each frame, like the fallback circle's fill.
    /// RECONCILE(entities): new kinds inherit automatically via kind.color().
    fn frame_color(&self, ctx: &DrawContext) -> Color32 {
        self.circle.color.unwrap_or_else(|| {
            let style = if self.interacted() {
                ctx.ctx.global_style().visuals.widgets.active
            } else {
                ctx.ctx.global_style().visuals.widgets.inactive
            };
            style.fg_stroke.color
        })
    }

    /// Imageless node: tinted kind-coloured disc (the hit geometry), the kind
    /// icon glyph 3× centered on it, the entity name 2× underneath.
    fn icon_shapes(&self, ctx: &DrawContext) -> Vec<Shape> {
        let center = ctx.meta.canvas_to_screen_pos(self.circle.pos);
        let r = ctx.meta.canvas_to_screen_size(self.circle.radius);
        let color = self.frame_color(ctx);
        let border = if self.interacted() { BORDER_ACTIVE } else { BORDER };

        let mut res: Vec<Shape> = Vec::with_capacity(3);
        res.push(
            CircleShape {
                center,
                radius: r,
                fill: color.gamma_multiply(0.25),
                stroke: Stroke::new(ctx.meta.canvas_to_screen_size(border), color),
            }
            .into(),
        );

        let icon = ctx.ctx.fonts_mut(|f| {
            f.layout_no_wrap(
                self.icon.clone(),
                FontId::new(
                    ctx.meta.canvas_to_screen_size(ICON_SIZE * self.size),
                    FontFamily::Proportional,
                ),
                color,
            )
        });
        let icon_pos = center - icon.size() / 2.0;
        res.push(TextShape::new(icon_pos, icon, color).into());

        let label = ctx.ctx.fonts_mut(|f| {
            f.layout_no_wrap(
                self.name.clone(),
                FontId::new(
                    ctx.meta.canvas_to_screen_size(LABEL_SIZE * self.size),
                    FontFamily::Monospace,
                ),
                color,
            )
        });
        let label_pos = Pos2::new(
            center.x - label.size().x / 2.0,
            center.y + r + ctx.meta.canvas_to_screen_size(LABEL_GAP * self.size),
        );
        res.push(TextShape::new(label_pos, label, color).into());
        res
    }

    fn image_shapes(&self, ctx: &DrawContext, tex: &TextureHandle) -> Vec<Shape> {
        let canvas = self.rect();
        let rect = Rect::from_min_max(
            ctx.meta.canvas_to_screen_pos(canvas.min),
            ctx.meta.canvas_to_screen_pos(canvas.max),
        );
        let cr = CornerRadius::same(
            ctx.meta
                .canvas_to_screen_size(IMG_CORNER * self.size)
                .round()
                .clamp(0.0, 255.0) as u8,
        );
        let color = self.frame_color(ctx);
        let border = if self.interacted() { BORDER_ACTIVE } else { BORDER };
        let stroke = Stroke::new(ctx.meta.canvas_to_screen_size(border), color);

        let mut res: Vec<Shape> = Vec::with_capacity(3);
        // textured fill and border as two shapes: epaint asserts when
        // tessellating a stroke into a mesh that has an assigned texture
        // (epaint mesh.rs "Mesh has an assigned texture")
        res.push(
            RectShape::new(rect, cr, Color32::WHITE, Stroke::NONE, StrokeKind::Inside)
                .with_texture(tex.id(), cover_uv(tex.size()))
                .into(),
        );
        res.push(RectShape::stroke(rect, cr, stroke, StrokeKind::Inside).into());

        // caption drawn unconditionally: SettingsStyle::labels_always is
        // pub(crate) in egui_graphs 0.30 (no getter), and the app always
        // passes with_labels_always(true) anyway. Kind icon at 2× the name,
        // side by side under the photo, vertically centered on each other.
        let (icon, name) = ctx.ctx.fonts_mut(|f| {
            (
                f.layout_no_wrap(
                    self.icon.clone(),
                    FontId::new(
                        ctx.meta.canvas_to_screen_size(LABEL_SIZE * 2.0 * self.size),
                        FontFamily::Proportional,
                    ),
                    color,
                ),
                f.layout_no_wrap(
                    self.name.clone(),
                    FontId::new(
                        ctx.meta.canvas_to_screen_size(LABEL_SIZE * self.size),
                        FontFamily::Monospace,
                    ),
                    color,
                ),
            )
        });
        let gap = ctx.meta.canvas_to_screen_size(LABEL_GAP * self.size);
        let total_w = icon.size().x + gap + name.size().x;
        let row_h = icon.size().y.max(name.size().y);
        let top = rect.bottom() + gap;
        let left = rect.center().x - total_w / 2.0;
        let icon_pos = Pos2::new(left, top + (row_h - icon.size().y) / 2.0);
        let name_pos =
            Pos2::new(left + icon.size().x + gap, top + (row_h - name.size().y) / 2.0);
        res.push(TextShape::new(icon_pos, icon, color).into());
        res.push(TextShape::new(name_pos, name, color).into());
        res
    }
}

/// Ray from the rect center along `dir` to the square boundary (half-extent
/// `half`). Corner rounding is ignored — see note in `is_inside`.
fn rect_boundary_point(center: Pos2, half: f32, dir: Vec2) -> Pos2 {
    let d = dir.normalized();
    if d == Vec2::ZERO {
        return center + Vec2::new(half, 0.0);
    }
    let tx = if d.x != 0.0 { half / d.x.abs() } else { f32::INFINITY };
    let ty = if d.y != 0.0 { half / d.y.abs() } else { f32::INFINITY };
    center + d * tx.min(ty)
}

/// Cover-fit UV: centered square crop of the texture so any aspect ratio
/// fills the tile without distortion.
fn cover_uv(tex_size: [usize; 2]) -> Rect {
    let (w, h) = (tex_size[0].max(1) as f32, tex_size[1].max(1) as f32);
    let (u, v) = if w > h { (h / w, 1.0) } else { (1.0, w / h) };
    Rect::from_min_max(
        Pos2::new((1.0 - u) / 2.0, (1.0 - v) / 2.0),
        Pos2::new((1.0 + u) / 2.0, (1.0 + v) / 2.0),
    )
}

// ---- texture cache ----
//
// Decode happens once per path. The cache lives in egui temp memory (the
// `Context` is reachable from `DisplayNode::shapes` via `DrawContext.ctx`),
// keyed by path, value `Option<TextureHandle>` — `None` caches *failures* so
// a broken path costs one fs hit per run, not one per frame. `TextureHandle`
// is refcounted (clone retains, drop frees), so the cached copy keeps the
// GPU texture alive for the app's lifetime.

fn cache_id(path: &str) -> egui::Id {
    egui::Id::new(("redstring_entity_image", path))
}

fn texture_for(ctx: &egui::Context, path: &str) -> Option<TextureHandle> {
    let id = cache_id(path);
    if let Some(cached) = ctx.data(|d| d.get_temp::<Option<TextureHandle>>(id)) {
        return cached;
    }
    let loaded = decode(path)
        .map(|ci| ctx.load_texture(format!("entity_image:{path}"), ci, TextureOptions::LINEAR));
    // clone outside data_mut: TextureHandle::clone locks the texture manager
    let stored = loaded.clone();
    ctx.data_mut(|d| d.insert_temp(id, stored));
    loaded
}

/// Forget the cached texture (and any cached failure) for `path` so the next
/// frame re-decodes the file. The inspector calls this on every (re)pick.
pub fn invalidate_texture(ctx: &egui::Context, path: &str) {
    ctx.data_mut(|d| d.remove::<Option<TextureHandle>>(cache_id(path)));
}

fn decode(path: &str) -> Option<ColorImage> {
    let bytes = std::fs::read(path).ok()?;
    let img = image::load_from_memory(&bytes).ok()?; // gif → first frame
    // cap oversized photos before GPU upload (`thumbnail` alone would also
    // *up*scale small images to fit the box, so gate it)
    let img = if img.width() > MAX_TEX_SIDE || img.height() > MAX_TEX_SIDE {
        img.thumbnail(MAX_TEX_SIDE, MAX_TEX_SIDE)
    } else {
        img
    };
    let rgba = img.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    Some(ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boundary_point_hits_square() {
        let c = Pos2::new(0.0, 0.0);
        assert_eq!(
            rect_boundary_point(c, 10.0, Vec2::new(1.0, 0.0)),
            Pos2::new(10.0, 0.0)
        );
        assert_eq!(
            rect_boundary_point(c, 10.0, Vec2::new(0.0, -3.0)),
            Pos2::new(0.0, -10.0)
        );
        // diagonal lands on the corner
        let p = rect_boundary_point(c, 10.0, Vec2::new(1.0, 1.0));
        assert!((p.x - 10.0).abs() < 1e-4 && (p.y - 10.0).abs() < 1e-4);
        // zero dir must not NaN
        let p = rect_boundary_point(c, 10.0, Vec2::ZERO);
        assert!(p.x.is_finite() && p.y.is_finite());
    }

    /// The fallback contract: a decodable file yields pixels, a missing or
    /// garbage file yields None (silent circle fallback) — never a panic.
    #[test]
    fn decode_or_silent_none() {
        let dir = std::env::temp_dir();
        let ok = dir.join("redstring_node_shape_test.png");
        image::RgbaImage::from_pixel(3, 2, image::Rgba([255, 0, 0, 255]))
            .save(&ok)
            .unwrap();
        let ci = decode(ok.to_str().unwrap()).expect("png must decode");
        assert_eq!((ci.width(), ci.height()), (3, 2)); // small stays small
        std::fs::remove_file(&ok).ok();

        let big = dir.join("redstring_node_shape_test_big.png");
        image::RgbaImage::from_pixel(MAX_TEX_SIDE + 88, 100, image::Rgba([0, 255, 0, 255]))
            .save(&big)
            .unwrap();
        let ci = decode(big.to_str().unwrap()).expect("png must decode");
        assert!(ci.width() <= MAX_TEX_SIDE as usize); // oversized gets capped
        std::fs::remove_file(&big).ok();

        assert!(decode("/nonexistent/redstring.png").is_none());

        let bad = dir.join("redstring_node_shape_test.garbage");
        std::fs::write(&bad, b"not an image").unwrap();
        assert!(decode(bad.to_str().unwrap()).is_none());
        std::fs::remove_file(&bad).ok();
    }

    #[test]
    fn cover_uv_crops_long_axis() {
        // landscape 200x100 → u spans the middle half
        let uv = cover_uv([200, 100]);
        assert!((uv.min.x - 0.25).abs() < 1e-4 && (uv.max.x - 0.75).abs() < 1e-4);
        assert!((uv.min.y - 0.0).abs() < 1e-4 && (uv.max.y - 1.0).abs() < 1e-4);
        // square → full texture
        let uv = cover_uv([64, 64]);
        assert_eq!(uv, Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)));
    }
}

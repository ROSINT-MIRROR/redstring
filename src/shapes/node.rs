












use egui::epaint::{CircleShape, RectShape, TextShape};
use egui::{
    Color32, ColorImage, CornerRadius, FontFamily, FontId, Pos2, Rect, Shape, Stroke, StrokeKind,
    TextureHandle, TextureOptions, Vec2,
};
use egui_graphs::{DefaultNodeShape, DisplayNode, DrawContext, NodeProps};
use petgraph::{EdgeType, stable_graph::IndexType};

use crate::error::Error;
use crate::model::{Entity, KIND_META, OCC_BADGE, OCCUPATION_ICONS, Occupation, ROLE_META, Role};



pub const IMG_RADIUS: f32 = 10.0;

const IMG_CORNER: f32 = 3.0;

const BORDER: f32 = 1.0;
const BORDER_ACTIVE: f32 = 2.0;


pub const NODE_RADIUS: f32 = 11.0;

const ICON_SIZE: f32 = 15.0;

pub const LABEL_SIZE: f32 = 7.0;
pub const LABEL_GAP: f32 = 2.0;
pub const LABEL_WRAP: f32 = 75.0;


const MAX_TEX_SIDE: u32 = 512;



const MAX_FONT_PX: f32 = 256.0;


const BACKDROP_ALPHA: f32 = 0.8;
const BACKDROP_PAD_FRAC: f32 = 0.12;
const BACKDROP_PAD_MIN_PX: f32 = 1.0;

const BADGE_CENTER_FRAC: f32 = 0.72;
const BADGE_RADIUS_FRAC: f32 = 0.5;
const IMG_BADGE_RADIUS_FRAC: f32 = 0.24;
const BADGE_R_MIN: f32 = 3.0;
const BADGE_R_MAX: f32 = 40.0;

#[derive(Clone)]
pub struct EntityNodeShape {
    
    
    circle: DefaultNodeShape,
    
    image: Option<String>,
    
    
    
    tex_ok: bool,
    
    icon: String,
    name: String,

    role: Role,
    occupation: Option<Occupation>,

    size: f32,
    hidden: bool,
}

impl From<NodeProps<Entity>> for EntityNodeShape {
    fn from(props: NodeProps<Entity>) -> Self {
        let mut circle = DefaultNodeShape::from(props.clone());
        circle.radius = NODE_RADIUS * props.payload.size * props.payload.boost;
        Self {
            image: props.payload.image.clone(),
            icon: KIND_META[props.payload.kind as usize].icon.to_owned(),
            name: props.payload.label.clone(),
            role: props.payload.role,
            occupation: props.payload.occupation.clone(),
            size: props.payload.size * props.payload.boost,
            hidden: props.payload.hidden,
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
        if self.hidden {
            return Vec::new();
        }
        let Some(tex) = self.resolve_texture(ctx) else {
            
            self.tex_ok = false;
            return self.icon_shapes(ctx);
        };
        self.tex_ok = true;
        self.image_shapes(ctx, &tex)
    }

    fn update(&mut self, state: &NodeProps<Entity>) {
        if self.image != state.payload.image {
            self.image = state.payload.image.clone();
            self.tex_ok = false; 
        }
        self.icon = KIND_META[state.payload.kind as usize].icon.to_owned();
        self.name = state.payload.label.clone();
        self.role = state.payload.role;
        self.occupation = state.payload.occupation.clone();
        self.size = state.payload.size * state.payload.boost;
        self.hidden = state.payload.hidden;
        <DefaultNodeShape as DisplayNode<Entity, E, Ty, Ix>>::update(&mut self.circle, state);
        
        self.circle.radius = NODE_RADIUS * self.size;
    }

    fn is_inside(&self, pos: Pos2) -> bool {
        if self.hidden {
            return false;
        }
        if self.image_active() {
            
            
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

    
    fn rect(&self) -> Rect {
        Rect::from_center_size(self.circle.pos, Vec2::splat(IMG_RADIUS * 2.0 * self.size))
    }

    fn interacted(&self) -> bool {
        self.circle.selected || self.circle.dragged || self.circle.hovered
    }

    
    fn resolve_texture(&self, ctx: &DrawContext) -> Option<TextureHandle> {
        let path = self.image.as_deref()?;
        texture_for(ctx.ctx, path)
    }

    
    
    fn frame_color(&self, ctx: &DrawContext) -> Color32 {
        let Some(c) = self.circle.color else {
            let style = if self.interacted() {
                ctx.ctx.global_style().visuals.widgets.active
            } else {
                ctx.ctx.global_style().visuals.widgets.inactive
            };
            return style.fg_stroke.color;
        };
        c
    }

    
    
    fn selection_ring(&self, ctx: &DrawContext) -> Option<Stroke> {
        if !self.circle.selected {
            return None;
        }
        Some(Stroke::new(
            3.0_f32,
            ctx.ctx.global_style().visuals.selection.stroke.color,
        ))
    }

    fn icon_shapes(&self, ctx: &DrawContext) -> Vec<Shape> {
        let center = ctx.meta.canvas_to_screen_pos(self.circle.pos);
        let r = ctx.meta.canvas_to_screen_size(self.circle.radius);
        let color = self.frame_color(ctx);
        let bg = ctx.ctx.global_style().visuals.panel_fill;
        let border = if self.interacted() { BORDER_ACTIVE } else { BORDER };

        let mut res: Vec<Shape> = Vec::with_capacity(4);
        res.extend(self.selection_ring(ctx).map(|ring| {
            Shape::from(CircleShape {
                center,
                radius: r + 3.0,
                fill: bg.lerp_to_gamma(ring.color, 0.15),
                stroke: ring,
            })
        }));
        res.push(
            CircleShape {
                center,
                radius: r,
                fill: bg.lerp_to_gamma(color, 0.25),
                stroke: Stroke::new(ctx.meta.canvas_to_screen_size(border), color),
            }
            .into(),
        );

        let icon = ctx.ctx.fonts_mut(|f| {
            f.layout_no_wrap(
                self.icon.clone(),
                FontId::new(font_px(ctx, ICON_SIZE * self.size), FontFamily::Proportional),
                color,
            )
        });
        let icon_pos = center - icon.size() / 2.0;
        res.push(TextShape::new(icon_pos, icon, color).into());

        let badge_off = r * BADGE_CENTER_FRAC;
        let badge_r = badge_radius(r);
        self.role_badge(ctx, &mut res, center + Vec2::new(badge_off, -badge_off), badge_r);
        self.occupation_badge(ctx, &mut res, center + Vec2::new(-badge_off, badge_off), badge_r);

        let fpx = font_px(ctx, LABEL_SIZE * self.size);
        let mut job = egui::text::LayoutJob::simple(
            self.name.clone(),
            FontId::new(fpx, FontFamily::Monospace),
            color,
            ctx.meta.canvas_to_screen_size(LABEL_WRAP * self.size),
        );
        job.halign = egui::Align::Center;
        let label = ctx.ctx.fonts_mut(|f| f.layout_job(job));
        let gap = ctx.meta.canvas_to_screen_size(LABEL_GAP * self.size);
        let Ok(box_rect) = label_box(LabelAnchor::Disc, center, r, label.size(), gap, fpx) else {
            return res;
        };
        let label_pos = Pos2::new(center.x, box_rect.min.y + label_pad_px(fpx));
        backdrop(&mut res, box_rect, bg);
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

        let mut res: Vec<Shape> = Vec::with_capacity(4);
        res.extend(self.selection_ring(ctx).map(|ring| {
            Shape::from(RectShape::stroke(rect.expand(3.0), cr, ring, StrokeKind::Outside))
        }));


        res.push(
            RectShape::new(rect, cr, Color32::WHITE, Stroke::NONE, StrokeKind::Inside)
                .with_texture(tex.id(), cover_uv(tex.size()))
                .into(),
        );
        res.push(RectShape::stroke(rect, cr, stroke, StrokeKind::Inside).into());

        let img_badge_r = badge_radius_img(rect.width().min(rect.height()));
        self.role_badge(ctx, &mut res, rect.right_top(), img_badge_r);
        self.occupation_badge(ctx, &mut res, rect.left_bottom(), img_badge_r);

        let fpx = font_px(ctx, LABEL_SIZE * self.size);
        let (icon, name) = ctx.ctx.fonts_mut(|f| {
            (
                f.layout_no_wrap(
                    self.icon.clone(),
                    FontId::new(
                        font_px(ctx, LABEL_SIZE * 2.0 * self.size),
                        FontFamily::Proportional,
                    ),
                    color,
                ),
                f.layout(
                    self.name.clone(),
                    FontId::new(fpx, FontFamily::Monospace),
                    color,
                    ctx.meta.canvas_to_screen_size(LABEL_WRAP * self.size),
                ),
            )
        });
        let gap = ctx.meta.canvas_to_screen_size(LABEL_GAP * self.size);
        let total_w = icon.size().x + gap + name.size().x;
        let row_h = icon.size().y.max(name.size().y);
        let half = rect.height() / 2.0;
        let Ok(box_rect) =
            label_box(LabelAnchor::Tile, rect.center(), half, Vec2::new(total_w, row_h), gap, fpx)
        else {
            return res;
        };
        let pad = label_pad_px(fpx);
        let top = box_rect.min.y + pad;
        let left = box_rect.min.x + pad;
        let icon_pos = Pos2::new(left, top + (row_h - icon.size().y) / 2.0);
        let name_pos = Pos2::new(left + icon.size().x + gap, top + (row_h - name.size().y) / 2.0);
        let bg = ctx.ctx.global_style().visuals.panel_fill;
        backdrop(&mut res, box_rect, bg);
        res.push(TextShape::new(icon_pos, icon, color).into());
        res.push(TextShape::new(name_pos, name, color).into());
        res
    }

    fn role_badge(&self, ctx: &DrawContext, res: &mut Vec<Shape>, center: Pos2, radius: f32) {
        if self.role == Role::Unset {
            return;
        }
        let meta = &ROLE_META[self.role as usize];
        let bg = ctx.ctx.global_style().visuals.panel_fill;
        res.push(
            CircleShape {
                center,
                radius,
                fill: meta.color,
                stroke: Stroke::new((radius * 0.18).max(0.6), bg),
            }
            .into(),
        );
        if radius < 4.0 {
            return;
        }
        let fg = badge_fg(meta.color);
        let glyph = ctx.ctx.fonts_mut(|f| {
            f.layout_no_wrap(
                meta.icon.to_owned(),
                FontId::new((radius * 1.35).min(MAX_FONT_PX), FontFamily::Proportional),
                fg,
            )
        });
        res.push(TextShape::new(center - glyph.size() / 2.0, glyph, fg).into());
    }

    fn occupation_badge(&self, ctx: &DrawContext, res: &mut Vec<Shape>, center: Pos2, radius: f32) {
        let Some(occ) = &self.occupation else {
            return;
        };
        let bg = ctx.ctx.global_style().visuals.panel_fill;
        res.push(
            CircleShape {
                center,
                radius,
                fill: OCC_BADGE,
                stroke: Stroke::new((radius * 0.18).max(0.6), bg),
            }
            .into(),
        );
        if radius < 4.0 {
            return;
        }
        let fg = badge_fg(OCC_BADGE);
        let glyph = ctx.ctx.fonts_mut(|f| {
            f.layout_no_wrap(
                OCCUPATION_ICONS[occ.icon as usize].glyph.to_owned(),
                FontId::new((radius * 1.35).min(MAX_FONT_PX), FontFamily::Proportional),
                fg,
            )
        });
        res.push(TextShape::new(center - glyph.size() / 2.0, glyph, fg).into());
    }
}

fn badge_fg(bg: Color32) -> Color32 {
    let lum = 0.299 * bg.r() as f32 + 0.587 * bg.g() as f32 + 0.114 * bg.b() as f32;
    if lum > 150.0 {
        Color32::from_rgb(25, 25, 30)
    } else {
        Color32::from_rgb(245, 245, 245)
    }
}


fn font_px(ctx: &DrawContext, canvas_size: f32) -> f32 {
    ctx.meta.canvas_to_screen_size(canvas_size).min(MAX_FONT_PX)
}


fn label_pad_px(font_px: f32) -> f32 {
    (font_px * BACKDROP_PAD_FRAC).max(BACKDROP_PAD_MIN_PX)
}

fn badge_radius(half: f32) -> f32 {
    (half * BADGE_RADIUS_FRAC).clamp(BADGE_R_MIN, BADGE_R_MAX)
}

fn badge_radius_img(min_side: f32) -> f32 {
    (min_side * IMG_BADGE_RADIUS_FRAC).clamp(BADGE_R_MIN, BADGE_R_MAX)
}

#[derive(Clone, Copy)]
pub enum LabelAnchor {
    Disc,
    Tile,
}

pub fn label_box(
    anchor: LabelAnchor,
    center: Pos2,
    half: f32,
    galley_size: Vec2,
    gap: f32,
    font_px: f32,
) -> Result<Rect, Error> {
    let pad = label_pad_px(font_px);
    let clear_y = match anchor {
        LabelAnchor::Disc => center.y + half * BADGE_CENTER_FRAC + badge_radius(half),
        LabelAnchor::Tile => center.y + half + badge_radius_img(half * 2.0),
    };
    let text_top = clear_y + gap + pad;
    let min = Pos2::new(center.x - galley_size.x / 2.0, text_top);
    Ok(Rect::from_min_size(min, galley_size).expand(pad))
}

fn backdrop(res: &mut Vec<Shape>, rect: Rect, bg: Color32) {
    res.push(
        RectShape::filled(rect, CornerRadius::same(2), bg.gamma_multiply(BACKDROP_ALPHA)).into(),
    );
}



fn rect_boundary_point(center: Pos2, half: f32, dir: Vec2) -> Pos2 {
    let d = dir.normalized();
    if d == Vec2::ZERO {
        return center + Vec2::new(half, 0.0);
    }
    let tx = if d.x != 0.0 { half / d.x.abs() } else { f32::INFINITY };
    let ty = if d.y != 0.0 { half / d.y.abs() } else { f32::INFINITY };
    center + d * tx.min(ty)
}



fn cover_uv(tex_size: [usize; 2]) -> Rect {
    let (w, h) = (tex_size[0].max(1) as f32, tex_size[1].max(1) as f32);
    let (u, v) = if w > h { (h / w, 1.0) } else { (1.0, w / h) };
    Rect::from_min_max(
        Pos2::new((1.0 - u) / 2.0, (1.0 - v) / 2.0),
        Pos2::new((1.0 + u) / 2.0, (1.0 + v) / 2.0),
    )
}








fn cache_id(path: &str) -> egui::Id {
    egui::Id::new(("redstring_entity_image", path))
}

fn texture_for(ctx: &egui::Context, path: &str) -> Option<TextureHandle> {
    let id = cache_id(path);
    let Some(hit) = ctx.data(|d| d.get_temp::<Option<TextureHandle>>(id)) else {
        return load_and_cache(ctx, path, id);
    };
    hit
}




fn load_and_cache(ctx: &egui::Context, path: &str, id: egui::Id) -> Option<TextureHandle> {
    let Ok(ci) = decode(path) else {
        ctx.data_mut(|d| d.insert_temp::<Option<TextureHandle>>(id, None));
        return None;
    };
    let tex = ctx.load_texture(format!("entity_image:{path}"), ci, TextureOptions::LINEAR);
    
    let stored = Some(tex.clone());
    ctx.data_mut(|d| d.insert_temp(id, stored));
    Some(tex)
}



pub fn invalidate_texture(ctx: &egui::Context, path: &str) -> Result<(), Error> {
    ctx.data_mut(|d| d.remove::<Option<TextureHandle>>(cache_id(path)));
    Ok(())
}

fn decode(path: &str) -> Result<ColorImage, Error> {
    let bytes = std::fs::read(path).map_err(|source| Error::Io { path: path.into(), source })?;
    
    let img = image::load_from_memory(&bytes).map_err(|source| Error::Image { path: path.into(), source })?;
    
    
    let img = if img.width() > MAX_TEX_SIDE || img.height() > MAX_TEX_SIDE {
        img.thumbnail(MAX_TEX_SIDE, MAX_TEX_SIDE)
    } else {
        img
    };
    let rgba = img.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    Ok(ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()))
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
        
        let p = rect_boundary_point(c, 10.0, Vec2::new(1.0, 1.0));
        assert!((p.x - 10.0).abs() < 1e-4 && (p.y - 10.0).abs() < 1e-4);
        
        let p = rect_boundary_point(c, 10.0, Vec2::ZERO);
        assert!(p.x.is_finite() && p.y.is_finite());
    }

    
    
    
    #[test]
    fn decode_or_named_error() {
        let dir = std::env::temp_dir();
        let ok = dir.join("redstring_node_shape_test.png");
        image::RgbaImage::from_pixel(3, 2, image::Rgba([255, 0, 0, 255]))
            .save(&ok)
            .unwrap();
        let ci = decode(ok.to_str().unwrap()).expect("png must decode");
        assert_eq!((ci.width(), ci.height()), (3, 2)); 
        std::fs::remove_file(&ok).ok();

        let big = dir.join("redstring_node_shape_test_big.png");
        image::RgbaImage::from_pixel(MAX_TEX_SIDE + 88, 100, image::Rgba([0, 255, 0, 255]))
            .save(&big)
            .unwrap();
        let ci = decode(big.to_str().unwrap()).expect("png must decode");
        assert!(ci.width() <= MAX_TEX_SIDE as usize); 
        std::fs::remove_file(&big).ok();

        assert!(decode("/nonexistent/redstring.png").is_err());

        let bad = dir.join("redstring_node_shape_test.garbage");
        std::fs::write(&bad, b"not an image").unwrap();
        assert!(decode(bad.to_str().unwrap()).is_err());
        std::fs::remove_file(&bad).ok();
    }

    #[test]
    fn cover_uv_crops_long_axis() {
        
        let uv = cover_uv([200, 100]);
        assert!((uv.min.x - 0.25).abs() < 1e-4 && (uv.max.x - 0.75).abs() < 1e-4);
        assert!((uv.min.y - 0.0).abs() < 1e-4 && (uv.max.y - 1.0).abs() < 1e-4);
        
        let uv = cover_uv([64, 64]);
        assert_eq!(uv, Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)));
    }

    fn galley_size_wrapped(text: &str, font_px: f32, wrap: f32) -> Vec2 {
        let ctx = egui::Context::default();
        let mut sz = Vec2::ZERO;
        ctx.run(egui::RawInput::default(), |ctx| {
            let mut job = egui::text::LayoutJob::simple(
                text.to_owned(),
                FontId::new(font_px, FontFamily::Monospace),
                Color32::WHITE,
                wrap,
            );
            job.halign = egui::Align::Center;
            sz = ctx.fonts_mut(|f| f.layout_job(job)).size();
        });
        sz
    }

    #[test]
    fn disc_label_clears_disc_and_occupation_badge() {
        let font_px = LABEL_SIZE;
        let size = galley_size_wrapped("J. Doe", font_px, LABEL_WRAP);
        let center = Pos2::new(100.0, 100.0);
        let half = NODE_RADIUS;
        let gap = LABEL_GAP;
        let boxr =
            label_box(LabelAnchor::Disc, center, half, size, gap, font_px).expect("label box");

        let disc = Rect::from_center_size(center, Vec2::splat(half * 2.0));
        let badge_off = half * BADGE_CENTER_FRAC;
        let br = badge_radius(half);
        let occ =
            Rect::from_center_size(center + Vec2::new(-badge_off, badge_off), Vec2::splat(br * 2.0));

        assert!(
            boxr.top() >= disc.bottom(),
            "label box top {} must be at/below disc bottom {}",
            boxr.top(),
            disc.bottom()
        );
        assert!(
            boxr.top() >= occ.bottom(),
            "label box top {} must be at/below occupation badge bottom {}",
            boxr.top(),
            occ.bottom()
        );
        assert!(
            !boxr.intersects(occ),
            "label box {boxr:?} must not intersect occupation badge {occ:?}"
        );
        let breathing = boxr.top() - occ.bottom();
        assert!(
            (breathing - gap).abs() < 1e-3,
            "breathing room {breathing} below the badge must equal LABEL_GAP {gap}"
        );
    }

    #[test]
    fn label_padding_is_tight() {
        assert!((BACKDROP_PAD_FRAC - 0.12).abs() < 1e-6);
        assert!((BACKDROP_PAD_MIN_PX - 1.0).abs() < 1e-6);
        assert!((label_pad_px(LABEL_SIZE) - 1.0).abs() < 1e-6);
        assert!((label_pad_px(40.0) - 40.0 * BACKDROP_PAD_FRAC).abs() < 1e-6);
    }

    #[test]
    fn tile_label_clears_tile_badge() {
        let font_px = LABEL_SIZE;
        let size = galley_size_wrapped("Doe", font_px, LABEL_WRAP);
        let center = Pos2::new(50.0, 50.0);
        let half = IMG_RADIUS;
        let gap = LABEL_GAP;
        let boxr =
            label_box(LabelAnchor::Tile, center, half, size, gap, font_px).expect("label box");

        let tile = Rect::from_center_size(center, Vec2::splat(half * 2.0));
        let br = badge_radius_img(half * 2.0);
        let occ = Rect::from_center_size(tile.left_bottom(), Vec2::splat(br * 2.0));

        assert!(boxr.top() >= tile.bottom(), "tile label must clear tile bottom");
        assert!(boxr.top() >= occ.bottom(), "tile label must clear image badge bottom");
        assert!(!boxr.intersects(occ), "tile label must not intersect image badge");
    }

    #[test]
    fn label_zone_and_draw_share_geometry() {
        let (size, zoom, boost) = (1.3f32, 1.7f32, 1.0f32);
        let font_px = (LABEL_SIZE * size * zoom).min(MAX_FONT_PX);
        let gsz = galley_size_wrapped("Jane Q. Public", font_px, LABEL_WRAP * size * zoom);
        let center = Pos2::new(200.0, 140.0);

        let node_half = NODE_RADIUS * size * zoom;
        let node_gap = LABEL_GAP * size * zoom;
        let node_fpx = (LABEL_SIZE * size * zoom).min(MAX_FONT_PX);
        let node_box =
            label_box(LabelAnchor::Disc, center, node_half, gsz, node_gap, node_fpx).expect("node");

        let vsize = size * boost;
        let zone_half = NODE_RADIUS * vsize * zoom;
        let zone_gap = LABEL_GAP * vsize * zoom;
        let zone_fpx = (LABEL_SIZE * vsize * zoom).min(MAX_FONT_PX);
        let zone_box =
            label_box(LabelAnchor::Disc, center, zone_half, gsz, zone_gap, zone_fpx).expect("zone");

        assert_eq!(
            node_box, zone_box,
            "node.rs draw and links.rs label_zone must derive an identical label box"
        );
    }
}

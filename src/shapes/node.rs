












use egui::epaint::{CircleShape, RectShape, TextShape};
use egui::{
    Color32, ColorImage, CornerRadius, FontFamily, FontId, Pos2, Rect, Shape, Stroke, StrokeKind,
    TextureHandle, TextureOptions, Vec2,
};
use egui_graphs::{DefaultNodeShape, DisplayNode, DrawContext, NodeProps};
use petgraph::{EdgeType, stable_graph::IndexType};

use crate::error::Error;
use crate::model::{Entity, KIND_META};



pub const IMG_RADIUS: f32 = 10.0;

const IMG_CORNER: f32 = 3.0;

const BORDER: f32 = 1.0;
const BORDER_ACTIVE: f32 = 2.0;


pub const NODE_RADIUS: f32 = 11.0;

const ICON_SIZE: f32 = 20.0;

pub const LABEL_SIZE: f32 = 7.0;
pub const LABEL_GAP: f32 = 2.0;


const MAX_TEX_SIDE: u32 = 512;



const MAX_FONT_PX: f32 = 256.0;

#[derive(Clone)]
pub struct EntityNodeShape {
    
    
    circle: DefaultNodeShape,
    
    image: Option<String>,
    
    
    
    tex_ok: bool,
    
    icon: String,
    name: String,
    
    
    size: f32,
}

impl From<NodeProps<Entity>> for EntityNodeShape {
    fn from(props: NodeProps<Entity>) -> Self {
        let mut circle = DefaultNodeShape::from(props.clone());
        circle.radius = NODE_RADIUS * props.payload.size;
        Self {
            image: props.payload.image.clone(),
            icon: KIND_META[props.payload.kind as usize].icon.to_owned(),
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
        self.size = state.payload.size;
        <DefaultNodeShape as DisplayNode<Entity, E, Ty, Ix>>::update(&mut self.circle, state);
        
        self.circle.radius = NODE_RADIUS * self.size;
    }

    fn is_inside(&self, pos: Pos2) -> bool {
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
                FontId::new(font_px(ctx, ICON_SIZE * self.size), FontFamily::Proportional),
                color,
            )
        });
        let icon_pos = center - icon.size() / 2.0;
        res.push(TextShape::new(icon_pos, icon, color).into());

        let label = ctx.ctx.fonts_mut(|f| {
            f.layout_no_wrap(
                self.name.clone(),
                FontId::new(font_px(ctx, LABEL_SIZE * self.size), FontFamily::Monospace),
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
        
        
        res.push(
            RectShape::new(rect, cr, Color32::WHITE, Stroke::NONE, StrokeKind::Inside)
                .with_texture(tex.id(), cover_uv(tex.size()))
                .into(),
        );
        res.push(RectShape::stroke(rect, cr, stroke, StrokeKind::Inside).into());

        
        
        
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
                f.layout_no_wrap(
                    self.name.clone(),
                    FontId::new(font_px(ctx, LABEL_SIZE * self.size), FontFamily::Monospace),
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


fn font_px(ctx: &DrawContext, canvas_size: f32) -> f32 {
    ctx.meta.canvas_to_screen_size(canvas_size).min(MAX_FONT_PX)
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
}

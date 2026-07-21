use egui::epaint::{CircleShape, RectShape, TextShape};
use egui::{
    Color32, CornerRadius, FontFamily, FontId, Pos2, Rect, Shape, Stroke, StrokeKind, TextureHandle,
    Vec2,
};
use egui_graphs::{DefaultNodeShape, DisplayNode, DrawContext, NodeProps};
use petgraph::{EdgeType, stable_graph::IndexType};

use egui_phosphor::regular::PLUS;

use crate::model::{Entity, KIND_META, OCC_BADGE, OCCUPATION_ICONS, Occupation, ROLE_META, Role};

use super::label::{
    BADGE_CENTER_FRAC, LABEL_GAP, LABEL_SIZE, LABEL_WRAP, LabelAnchor, MAX_FONT_PX, badge_radius,
    badge_radius_img, label_box, label_pad_px,
};
use super::texture::{cover_uv, texture_for};

pub const IMG_RADIUS: f32 = 10.0;

const IMG_CORNER: f32 = 3.0;

const BORDER: f32 = 1.0;
const BORDER_ACTIVE: f32 = 2.0;

pub const NODE_RADIUS: f32 = 11.0;

const ICON_SIZE: f32 = 15.0;

const LABEL_MIN_PX: f32 = 9.0;

const BACKDROP_ALPHA: f32 = 0.8;

const COLLAPSE_BADGE: Color32 = Color32::from_rgb(72, 96, 132);

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
    collapsed: bool,
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
            collapsed: props.payload.collapsed,
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
        self.collapsed = state.payload.collapsed;
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
        let Ok(tex) = texture_for(ctx.ctx, path) else {
            return None;
        };
        tex
    }

    fn frame_color(&self, ctx: &DrawContext) -> Color32 {
        let dark = ctx.ctx.global_style().visuals.dark_mode;
        let Some(c) = self.circle.color else {
            let style = if self.interacted() {
                ctx.ctx.global_style().visuals.widgets.active
            } else {
                ctx.ctx.global_style().visuals.widgets.inactive
            };
            return style.fg_stroke.color;
        };
        let Ok(col) = crate::ui::board_color(c, dark) else {
            return c;
        };
        col
    }

    fn label_ink(&self, ctx: &DrawContext) -> Color32 {
        badge_fg(ctx.ctx.global_style().visuals.panel_fill)
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
        let Ok(badge_r) = badge_radius(r) else {
            return res;
        };
        self.role_badge(ctx, &mut res, center + Vec2::new(badge_off, -badge_off), badge_r);
        self.occupation_badge(ctx, &mut res, center + Vec2::new(-badge_off, badge_off), badge_r);
        self.collapsed_badge(ctx, &mut res, center + Vec2::new(badge_off, badge_off), badge_r);

        let fpx = font_px(ctx, LABEL_SIZE * self.size);
        if fpx < LABEL_MIN_PX {
            return res;
        }
        let ink = self.label_ink(ctx);
        let mut job = egui::text::LayoutJob::simple(
            self.name.clone(),
            FontId::new(fpx, FontFamily::Monospace),
            ink,
            ctx.meta.canvas_to_screen_size(LABEL_WRAP * self.size),
        );
        job.halign = egui::Align::Center;
        let label = ctx.ctx.fonts_mut(|f| f.layout_job(job));
        let gap = ctx.meta.canvas_to_screen_size(LABEL_GAP * self.size);
        let Ok(box_rect) = label_box(LabelAnchor::Disc, center, r, label.size(), gap, fpx) else {
            return res;
        };
        let Ok(lpad) = label_pad_px(fpx) else {
            return res;
        };
        let label_pos = Pos2::new(center.x, box_rect.min.y + lpad);
        backdrop(&mut res, box_rect, bg);
        res.push(TextShape::new(label_pos, label, ink).into());
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

        let Ok(uv) = cover_uv(tex.size()) else {
            return res;
        };
        res.push(
            RectShape::new(rect, cr, Color32::WHITE, Stroke::NONE, StrokeKind::Inside)
                .with_texture(tex.id(), uv)
                .into(),
        );
        res.push(RectShape::stroke(rect, cr, stroke, StrokeKind::Inside).into());

        let Ok(img_badge_r) = badge_radius_img(rect.width().min(rect.height())) else {
            return res;
        };
        self.role_badge(ctx, &mut res, rect.right_top(), img_badge_r);
        self.occupation_badge(ctx, &mut res, rect.left_bottom(), img_badge_r);
        self.collapsed_badge(ctx, &mut res, rect.right_bottom(), img_badge_r);

        let fpx = font_px(ctx, LABEL_SIZE * self.size);
        if fpx < LABEL_MIN_PX {
            return res;
        }
        let ink = self.label_ink(ctx);
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
                    ink,
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
        let Ok(pad) = label_pad_px(fpx) else {
            return res;
        };
        let top = box_rect.min.y + pad;
        let left = box_rect.min.x + pad;
        let icon_pos = Pos2::new(left, top + (row_h - icon.size().y) / 2.0);
        let name_pos = Pos2::new(left + icon.size().x + gap, top + (row_h - name.size().y) / 2.0);
        let bg = ctx.ctx.global_style().visuals.panel_fill;
        backdrop(&mut res, box_rect, bg);
        res.push(TextShape::new(icon_pos, icon, color).into());
        res.push(TextShape::new(name_pos, name, ink).into());
        res
    }

    fn role_badge(&self, ctx: &DrawContext, res: &mut Vec<Shape>, center: Pos2, radius: f32) {
        if self.role == Role::Unset {
            return;
        }
        let meta = &ROLE_META[self.role as usize];
        glyph_badge(ctx, res, center, radius, meta.color, meta.icon);
    }

    fn occupation_badge(&self, ctx: &DrawContext, res: &mut Vec<Shape>, center: Pos2, radius: f32) {
        let Some(occ) = &self.occupation else {
            return;
        };
        glyph_badge(
            ctx,
            res,
            center,
            radius,
            OCC_BADGE,
            OCCUPATION_ICONS[occ.icon as usize].glyph,
        );
    }

    fn collapsed_badge(&self, ctx: &DrawContext, res: &mut Vec<Shape>, center: Pos2, radius: f32) {
        if !self.collapsed {
            return;
        }
        glyph_badge(ctx, res, center, radius, COLLAPSE_BADGE, PLUS);
    }
}

fn glyph_badge(
    ctx: &DrawContext,
    res: &mut Vec<Shape>,
    center: Pos2,
    radius: f32,
    fill: Color32,
    glyph: &str,
) {
    let bg = ctx.ctx.global_style().visuals.panel_fill;
    res.push(
        CircleShape {
            center,
            radius,
            fill,
            stroke: Stroke::new((radius * 0.18).max(0.6), bg),
        }
        .into(),
    );
    if radius < 4.0 {
        return;
    }
    let fg = badge_fg(fill);
    let g = ctx.ctx.fonts_mut(|f| {
        f.layout_no_wrap(
            glyph.to_owned(),
            FontId::new((radius * 1.35).min(MAX_FONT_PX), FontFamily::Proportional),
            fg,
        )
    });
    res.push(TextShape::new(center - g.size() / 2.0, g, fg).into());
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

    fn node_label_texts(size: f32, collapsed: bool) -> Vec<String> {
        let ent = Entity {
            slug: "n".into(),
            kind: EntityKind::Person,
            label: "Node Label".into(),
            value: String::new(),
            sealed: false,
            observations: Vec::new(),
            role: Role::Unset,
            occupation: None,
            note: String::new(),
            image: None,
            size,
            boost: 1.0,
            collapsed,
            hidden: false,
        };
        let mut node = Node::<Entity, Relation, Directed, DefaultIx, EntityNodeShape>::new(ent);
        let mut out = Vec::new();
        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)));
        ctx.run(input, |c| {
            let painter = c.debug_painter();
            let style = SettingsStyle::default();
            let meta = MetadataFrame::default();
            let dctx = DrawContext {
                ctx: c,
                painter: &painter,
                style: &style,
                is_directed: true,
                meta: &meta,
            };
            let shapes =
                <EntityNodeShape as DisplayNode<Entity, Relation, Directed, DefaultIx>>::shapes(
                    node.display_mut(),
                    &dctx,
                );
            for s in &shapes {
                if let Shape::Text(t) = s {
                    out.push(t.galley.text().to_owned());
                }
            }
        });
        out
    }

    #[test]
    fn collapsed_node_draws_marker() {
        let collapsed = node_label_texts(DEFAULT_NODE_SIZE, true);
        assert!(
            collapsed.iter().any(|t| t.as_str() == PLUS),
            "a collapsed node must render the collapse marker glyph, got {collapsed:?}"
        );
        let normal = node_label_texts(DEFAULT_NODE_SIZE, false);
        assert!(
            !normal.iter().any(|t| t.as_str() == PLUS),
            "a non-collapsed node must not render the collapse marker, got {normal:?}"
        );
    }

    #[test]
    fn label_hidden_below_lod() {
        let big = (LABEL_MIN_PX * 2.0) / LABEL_SIZE;
        let shown = node_label_texts(big, false);
        assert!(
            shown.iter().any(|t| t.as_str() == "Node Label"),
            "above the LOD threshold the node label must render, got {shown:?}"
        );
        let tiny = (LABEL_MIN_PX * 0.5) / LABEL_SIZE;
        let hidden = node_label_texts(tiny, false);
        assert!(
            !hidden.iter().any(|t| t.as_str() == "Node Label"),
            "below the LOD threshold the node label must be omitted, got {hidden:?}"
        );
    }

    fn node_label_color(theme: egui::Theme) -> Color32 {
        let size = (LABEL_MIN_PX * 2.0) / LABEL_SIZE;
        let ent = Entity {
            slug: "n".into(),
            kind: EntityKind::Person,
            label: "Node Label".into(),
            value: String::new(),
            sealed: false,
            observations: Vec::new(),
            role: Role::Unset,
            occupation: None,
            note: String::new(),
            image: None,
            size,
            boost: 1.0,
            collapsed: false,
            hidden: false,
        };
        let mut node = Node::<Entity, Relation, Directed, DefaultIx, EntityNodeShape>::new(ent);
        let ctx = egui::Context::default();
        ctx.set_theme(theme);
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)));
        let mut found: Option<Color32> = None;
        ctx.run(input, |c| {
            let painter = c.debug_painter();
            let style = SettingsStyle::default();
            let meta = MetadataFrame::default();
            let dctx = DrawContext {
                ctx: c,
                painter: &painter,
                style: &style,
                is_directed: true,
                meta: &meta,
            };
            let shapes =
                <EntityNodeShape as DisplayNode<Entity, Relation, Directed, DefaultIx>>::shapes(
                    node.display_mut(),
                    &dctx,
                );
            for s in &shapes {
                if let Shape::Text(t) = s {
                    if t.galley.text() == "Node Label" {
                        found = Some(t.galley.job.sections[0].format.color);
                    }
                }
            }
        });
        found.expect("the node label must render at this size")
    }

    fn luminance(c: Color32) -> f32 {
        0.299 * c.r() as f32 + 0.587 * c.g() as f32 + 0.114 * c.b() as f32
    }

    #[test]
    fn label_ink_adapts_to_theme() {
        let dark = node_label_color(egui::Theme::Dark);
        let light = node_label_color(egui::Theme::Light);
        assert!(
            luminance(dark) > 150.0,
            "dark-theme label ink must be light for legibility, got {dark:?} lum {}",
            luminance(dark)
        );
        assert!(
            luminance(light) < 110.0,
            "light-theme label ink must be dark for legibility, got {light:?} lum {}",
            luminance(light)
        );
        assert!(
            luminance(dark) > luminance(light) + 60.0,
            "label ink must invert between themes: dark {} vs light {}",
            luminance(dark),
            luminance(light)
        );
    }

    use crate::model::{DEFAULT_NODE_SIZE, EntityKind, Relation};
    use egui_graphs::{MetadataFrame, Node, SettingsStyle};
    use petgraph::{Directed, stable_graph::DefaultIx};
}

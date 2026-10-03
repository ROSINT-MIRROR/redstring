use std::collections::BTreeMap;

use egui::epaint::{CircleShape, RectShape, TextShape};
use egui::{
    Color32, CornerRadius, FontFamily, FontId, Pos2, Rect, Shape, Stroke, StrokeKind, TextureHandle,
    Vec2,
};
use egui_graphs::{DefaultNodeShape, DisplayNode, DrawContext, NodeProps};
use petgraph::{EdgeType, stable_graph::IndexType};

use crate::error::Error;
use crate::model::{Entity, Facet, FacetValue, KIND_META};

use super::badge::{Corners, backdrop, badge_fg, collapsed_badge, draw_facet_badges, sealed_badge};
use super::label::{
    BADGE_CENTER_FRAC, LABEL_GAP, LABEL_ICON_SCALE, LABEL_MAX_ROWS, LABEL_MIN_PX, LABEL_SIZE,
    LABEL_WRAP, LabelAnchor, MAX_FONT_PX, badge_radius, badge_radius_img, claim_label, label_box,
    label_pad_px,
};
use super::reveal::{display_label, reveal_label};
use super::texture::{cover_uv, texture_for};

pub const IMG_RADIUS: f32 = 14.0;

const IMG_CORNER: f32 = 3.0;

const BORDER: f32 = 1.0;
const BORDER_ACTIVE: f32 = 2.0;

pub const NODE_RADIUS: f32 = 11.0;

pub fn base_radius(e: &Entity) -> Result<f32, Error> {
    if e.image.is_some() {
        return Ok(IMG_RADIUS);
    }
    Ok(NODE_RADIUS)
}

const ICON_SIZE: f32 = 15.0;

const DISC_FILL_DARK: f32 = 0.25;
const DISC_FILL_LIGHT: f32 = 0.5;

const SEALED_DESAT: f32 = 0.55;
const SEALED_IMG_TINT: Color32 = Color32::from_gray(150);

#[derive(Clone)]
pub struct EntityNodeShape {
    circle: DefaultNodeShape,

    image: Option<String>,

    tex_ok: bool,

    icon: String,
    full: String,

    facets: BTreeMap<Facet, FacetValue>,

    size: f32,
    collapsed: bool,
    sealed: bool,
    hidden: bool,
    id: u64,
}

impl From<NodeProps<Entity>> for EntityNodeShape {
    fn from(props: NodeProps<Entity>) -> Self {
        let mut circle = DefaultNodeShape::from(props.clone());
        circle.radius = NODE_RADIUS * props.payload.size * props.payload.boost;
        Self {
            image: props.payload.image.clone(),
            icon: KIND_META[props.payload.kind as usize].icon.to_owned(),
            full: props.payload.label.trim().to_owned(),
            facets: props.payload.facets.clone(),
            size: props.payload.size * props.payload.boost,
            collapsed: props.payload.collapsed,
            sealed: props.payload.sealed,
            hidden: props.payload.hidden,
            id: slug_id(&props.payload.slug),
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
        self.full = state.payload.label.trim().to_owned();
        self.id = slug_id(&state.payload.slug);
        self.facets = state.payload.facets.clone();
        self.size = state.payload.size * state.payload.boost;
        self.collapsed = state.payload.collapsed;
        self.sealed = state.payload.sealed;
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

    fn kind_color(&self, ctx: &DrawContext) -> Color32 {
        let Some(c) = self.circle.color else {
            return self.frame_color(ctx);
        };
        c
    }

    fn label_priority(&self) -> f32 {
        if self.circle.selected || self.circle.hovered {
            return f32::INFINITY;
        }
        self.size
    }

    fn reveals(&self, elided: bool, claimed: bool) -> bool {
        self.circle.hovered && (elided || !claimed)
    }

    fn reveal_hover(
        &self,
        ctx: &DrawContext,
        anchor: LabelAnchor,
        center: Pos2,
        half: f32,
    ) -> Result<(), Error> {
        if !self.circle.hovered {
            return Ok(());
        }
        reveal_label(ctx, anchor, center, half, self.size, &self.full)
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
        let color = if self.sealed { mute_node(color) } else { color };
        let dark = ctx.ctx.global_style().visuals.dark_mode;
        let kind = self.kind_color(ctx);
        let kind = if self.sealed { mute_node(kind) } else { kind };
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
                fill: disc_fill(bg, kind, dark),
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
        let corners = Corners {
            tr: center + Vec2::new(badge_off, -badge_off),
            bl: center + Vec2::new(-badge_off, badge_off),
            tl: center + Vec2::new(-badge_off, -badge_off),
            br: center + Vec2::new(badge_off, badge_off),
        };
        let collapse_center = collapse_slot(corners.tl, self.sealed, badge_r);
        let Ok(()) = draw_facet_badges(&self.facets, ctx, &mut res, badge_r, &corners) else { return res };
        let Ok(()) = sealed_badge(self.sealed, ctx, &mut res, corners.tl, badge_r) else { return res };
        let Ok(()) = collapsed_badge(self.collapsed, ctx, &mut res, collapse_center, badge_r) else { return res };

        let fpx = font_px(ctx, LABEL_SIZE * self.size);
        if fpx < LABEL_MIN_PX {
            let Ok(()) = self.reveal_hover(ctx, LabelAnchor::Disc, center, r) else { return res };
            return res;
        }
        let Ok(ink) = badge_fg(bg) else { return res };
        let Ok(shown) = display_label(&self.full) else { return res };
        let elided = shown != self.full;
        let mut job = egui::text::LayoutJob::simple(
            shown,
            FontId::new(fpx, FontFamily::Monospace),
            ink,
            ctx.meta.canvas_to_screen_size(LABEL_WRAP * self.size),
        );
        job.halign = egui::Align::Center;
        job.wrap.max_rows = LABEL_MAX_ROWS;
        let label = ctx.ctx.fonts_mut(|f| f.layout_job(job));
        let gap = ctx.meta.canvas_to_screen_size(LABEL_GAP * self.size);
        let Ok(box_rect) = label_box(LabelAnchor::Disc, center, r, label.size(), gap, fpx) else {
            return res;
        };
        let Ok(lpad) = label_pad_px(fpx) else {
            return res;
        };
        let Ok(draw) = claim_label(ctx.ctx, self.id, box_rect, self.label_priority()) else {
            return res;
        };
        if self.reveals(elided, draw) {
            let Ok(()) = self.reveal_hover(ctx, LabelAnchor::Disc, center, r) else { return res };
            return res;
        }
        if !draw {
            return res;
        }
        let label_pos = Pos2::new(center.x, box_rect.min.y + lpad);
        let Ok(()) = backdrop(&mut res, box_rect, bg, dark) else { return res };
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
        let color = if self.sealed { mute_node(color) } else { color };
        let dark = ctx.ctx.global_style().visuals.dark_mode;
        let border = if self.interacted() { BORDER_ACTIVE } else { BORDER };
        let stroke = Stroke::new(ctx.meta.canvas_to_screen_size(border), color);
        let img_tint = if self.sealed { SEALED_IMG_TINT } else { Color32::WHITE };

        let mut res: Vec<Shape> = Vec::with_capacity(4);
        res.extend(self.selection_ring(ctx).map(|ring| {
            Shape::from(RectShape::stroke(rect.expand(3.0), cr, ring, StrokeKind::Outside))
        }));

        let Ok(uv) = cover_uv(tex.size()) else {
            return res;
        };
        res.push(
            RectShape::new(rect, cr, img_tint, Stroke::NONE, StrokeKind::Inside)
                .with_texture(tex.id(), uv)
                .into(),
        );
        res.push(RectShape::stroke(rect, cr, stroke, StrokeKind::Inside).into());

        let Ok(img_badge_r) = badge_radius_img(rect.width().min(rect.height())) else {
            return res;
        };
        let corners = Corners {
            tr: rect.right_top(),
            bl: rect.left_bottom(),
            tl: rect.left_top(),
            br: rect.right_bottom(),
        };
        let collapse_center = collapse_slot(corners.tl, self.sealed, img_badge_r);
        let Ok(()) = draw_facet_badges(&self.facets, ctx, &mut res, img_badge_r, &corners) else { return res };
        let Ok(()) = sealed_badge(self.sealed, ctx, &mut res, corners.tl, img_badge_r) else { return res };
        let Ok(()) = collapsed_badge(self.collapsed, ctx, &mut res, collapse_center, img_badge_r) else { return res };

        let half = rect.height() / 2.0;
        let fpx = font_px(ctx, LABEL_SIZE * self.size);
        if fpx < LABEL_MIN_PX {
            let Ok(()) = self.reveal_hover(ctx, LabelAnchor::Tile, rect.center(), half) else {
                return res;
            };
            return res;
        }
        let Ok(ink) = badge_fg(ctx.ctx.global_style().visuals.panel_fill) else { return res };
        let Ok(shown) = display_label(&self.full) else { return res };
        let elided = shown != self.full;
        let name_wrap = ctx.meta.canvas_to_screen_size(LABEL_WRAP * self.size);
        let (icon, name) = ctx.ctx.fonts_mut(|f| {
            let icon = f.layout_no_wrap(
                self.icon.clone(),
                FontId::new(
                    font_px(ctx, LABEL_SIZE * LABEL_ICON_SCALE * self.size),
                    FontFamily::Proportional,
                ),
                color,
            );
            let mut name_job = egui::text::LayoutJob::simple(
                shown,
                FontId::new(fpx, FontFamily::Monospace),
                ink,
                name_wrap,
            );
            name_job.wrap.max_rows = LABEL_MAX_ROWS;
            (icon, f.layout_job(name_job))
        });
        let gap = ctx.meta.canvas_to_screen_size(LABEL_GAP * self.size);
        let total_w = icon.size().x + gap + name.size().x;
        let row_h = icon.size().y.max(name.size().y);
        let Ok(box_rect) =
            label_box(LabelAnchor::Tile, rect.center(), half, Vec2::new(total_w, row_h), gap, fpx)
        else {
            return res;
        };
        let Ok(pad) = label_pad_px(fpx) else {
            return res;
        };
        let Ok(draw) = claim_label(ctx.ctx, self.id, box_rect, self.label_priority()) else {
            return res;
        };
        if self.reveals(elided, draw) {
            let Ok(()) = self.reveal_hover(ctx, LabelAnchor::Tile, rect.center(), half) else {
                return res;
            };
            return res;
        }
        if !draw {
            return res;
        }
        let top = box_rect.min.y + pad;
        let left = box_rect.min.x + pad;
        let icon_pos = Pos2::new(left, top + (row_h - icon.size().y) / 2.0);
        let name_pos = Pos2::new(left + icon.size().x + gap, top + (row_h - name.size().y) / 2.0);
        let bg = ctx.ctx.global_style().visuals.panel_fill;
        let Ok(()) = backdrop(&mut res, box_rect, bg, dark) else { return res };
        res.push(TextShape::new(icon_pos, icon, color).into());
        res.push(TextShape::new(name_pos, name, ink).into());
        res
    }
}

fn font_px(ctx: &DrawContext, canvas_size: f32) -> f32 {
    ctx.meta.canvas_to_screen_size(canvas_size).min(MAX_FONT_PX)
}

fn disc_fill(bg: Color32, kind: Color32, dark: bool) -> Color32 {
    let frac = if dark { DISC_FILL_DARK } else { DISC_FILL_LIGHT };
    bg.lerp_to_gamma(kind, frac)
}

fn mute_node(c: Color32) -> Color32 {
    let lum = 0.299 * c.r() as f32 + 0.587 * c.g() as f32 + 0.114 * c.b() as f32;
    let gray = Color32::from_gray(lum.clamp(0.0, 255.0) as u8);
    c.lerp_to_gamma(gray, SEALED_DESAT)
}

fn slug_id(slug: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in slug.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash & !(1u64 << 63)
}

fn collapse_slot(tl: Pos2, sealed: bool, badge_r: f32) -> Pos2 {
    if sealed {
        tl + Vec2::new(0.0, badge_r * 2.0 + 1.0)
    } else {
        tl
    }
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
    use egui_phosphor::regular::{LOCK, PLUS};

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

    fn entity_shape_texts(ent: Entity) -> Vec<String> {
        let mut node = Node::<Entity, Relation, Directed, DefaultIx, EntityNodeShape>::new(ent);
        let mut out = Vec::new();
        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)));
        let _full = ctx.run_ui(input, |ui| {
            let c = ui.ctx();
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

    fn node_label_texts(size: f32, collapsed: bool) -> Vec<String> {
        entity_shape_texts(Entity {
            slug: "n".into(),
            kind: EntityKind::Person,
            label: "Node Label".into(),
            size,
            collapsed,
            ..Default::default()
        })
    }

    #[test]
    fn sealed_node_draws_lock_badge() {
        let sealed = entity_shape_texts(Entity {
            slug: "n".into(),
            kind: EntityKind::Person,
            label: "Node Label".into(),
            size: DEFAULT_NODE_SIZE,
            sealed: true,
            ..Default::default()
        });
        assert!(
            sealed.iter().any(|t| t.as_str() == LOCK),
            "a sealed node must render the lock badge glyph, got {sealed:?}"
        );
        let unsealed = entity_shape_texts(Entity {
            slug: "n".into(),
            kind: EntityKind::Person,
            label: "Node Label".into(),
            size: DEFAULT_NODE_SIZE,
            sealed: false,
            ..Default::default()
        });
        assert!(
            !unsealed.iter().any(|t| t.as_str() == LOCK),
            "an unsealed node must not render the lock badge, got {unsealed:?}"
        );
    }

    #[test]
    fn mute_node_desaturates_toward_gray() {
        let vivid = Color32::from_rgb(220, 60, 40);
        let muted = mute_node(vivid);
        assert!(
            chroma(muted) < chroma(vivid),
            "a sealed node color must lose saturation so it recedes: {muted:?} vs {vivid:?}"
        );
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
            size,
            ..Default::default()
        };
        let mut node = Node::<Entity, Relation, Directed, DefaultIx, EntityNodeShape>::new(ent);
        let ctx = egui::Context::default();
        ctx.set_theme(theme);
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)));
        let mut found: Option<Color32> = None;
        let _full = ctx.run_ui(input, |ui| {
            let c = ui.ctx();
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

    fn disc_node(slug: &str, label: &str) -> Node<Entity, Relation, Directed, DefaultIx, EntityNodeShape> {
        let ent = Entity {
            slug: slug.into(),
            kind: EntityKind::Person,
            label: label.into(),
            size: (LABEL_MIN_PX * 2.0) / LABEL_SIZE,
            ..Default::default()
        };
        Node::new(ent)
    }

    #[test]
    fn overlapping_labels_declutter() {
        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)));
        let mut drawn = 0usize;
        for _ in 0..2 {
            drawn = 0;
            let _full = ctx.run_ui(input.clone(), |ui| {
                let c = ui.ctx();
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
                for slug in ["n1", "n2"] {
                    let mut node = disc_node(slug, "Overlap Label");
                    let shapes = <EntityNodeShape as DisplayNode<
                        Entity,
                        Relation,
                        Directed,
                        DefaultIx,
                    >>::shapes(node.display_mut(), &dctx);
                    for s in &shapes {
                        if let Shape::Text(t) = s {
                            if t.galley.text() == "Overlap Label" {
                                drawn += 1;
                            }
                        }
                    }
                }
            });
        }
        assert_eq!(
            drawn, 1,
            "two identically-placed labels must declutter to one after a stable frame, got {drawn}"
        );
    }

    fn chroma(c: Color32) -> f32 {
        let r = c.r() as f32;
        let g = c.g() as f32;
        let b = c.b() as f32;
        r.max(g).max(b) - r.min(g).min(b)
    }

    #[test]
    fn light_disc_fill_is_more_saturated_than_before() {
        let bg = Color32::from_rgb(248, 248, 248);
        let kind = Color32::from_rgb(255, 170, 80);
        let new_fill = disc_fill(bg, kind, false);
        let old_base = crate::ui::board_color(kind, false).expect("board color");
        let old_fill = bg.lerp_to_gamma(old_base, 0.25);
        assert!(
            chroma(new_fill) > chroma(old_fill) + 10.0,
            "light disc fill must keep more category hue than the old pale formula: new {new_fill:?} chroma {} vs old {old_fill:?} chroma {}",
            chroma(new_fill),
            chroma(old_fill)
        );
        let dark_fill = disc_fill(Color32::from_rgb(27, 27, 27), kind, true);
        assert_eq!(
            dark_fill,
            Color32::from_rgb(27, 27, 27).lerp_to_gamma(kind, 0.25),
            "dark disc fill must be unchanged"
        );
    }

    use crate::model::{DEFAULT_NODE_SIZE, EntityKind, Relation};
    use egui_graphs::{MetadataFrame, Node, SettingsStyle};
    use petgraph::{Directed, stable_graph::DefaultIx};
}

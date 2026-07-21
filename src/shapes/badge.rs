use std::collections::BTreeMap;

use egui::epaint::{CircleShape, RectShape, TextShape};
use egui::{Color32, CornerRadius, FontFamily, FontId, Pos2, Rect, Shape, Stroke, StrokeKind};
use egui_graphs::DrawContext;

use egui_phosphor::regular::{LOCK, PLUS};

use crate::error::Error;
use crate::model::{BadgeCorner, FACET_META, Facet, FacetValue, facet_badge};

use super::label::{MAX_FONT_PX, theme_shift};

const COLLAPSE_BADGE: Color32 = Color32::from_rgb(72, 96, 132);
const SEALED_BADGE: Color32 = Color32::from_rgb(70, 72, 84);

const BACKDROP_MIX: f32 = 0.14;
const BACKDROP_LINE_MIX: f32 = 0.34;

pub struct Corners {
    pub tr: Pos2,
    pub bl: Pos2,
    pub tl: Pos2,
    pub br: Pos2,
}

impl Corners {
    fn at(&self, corner: BadgeCorner) -> Pos2 {
        match corner {
            BadgeCorner::TopRight => self.tr,
            BadgeCorner::BottomLeft => self.bl,
            BadgeCorner::BottomRight => self.br,
        }
    }
}

pub fn draw_facet_badges(
    facets: &BTreeMap<Facet, FacetValue>,
    ctx: &DrawContext,
    res: &mut Vec<Shape>,
    radius: f32,
    corners: &Corners,
) -> Result<(), Error> {
    for (facet, value) in facets {
        let Ok(Some(badge)) = facet_badge(*facet, value) else {
            continue;
        };
        let Some(meta) = FACET_META.iter().find(|m| m.facet == *facet) else {
            continue;
        };
        glyph_badge(ctx, res, corners.at(meta.corner), radius, badge.color, badge.glyph);
    }
    Ok(())
}

pub fn collapsed_badge(
    collapsed: bool,
    ctx: &DrawContext,
    res: &mut Vec<Shape>,
    center: Pos2,
    radius: f32,
) -> Result<(), Error> {
    if !collapsed {
        return Ok(());
    }
    glyph_badge(ctx, res, center, radius, COLLAPSE_BADGE, PLUS);
    Ok(())
}

pub fn sealed_badge(
    sealed: bool,
    ctx: &DrawContext,
    res: &mut Vec<Shape>,
    center: Pos2,
    radius: f32,
) -> Result<(), Error> {
    if !sealed {
        return Ok(());
    }
    glyph_badge(ctx, res, center, radius, SEALED_BADGE, LOCK);
    Ok(())
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
    let Ok(fg) = badge_fg(fill) else {
        return;
    };
    let g = ctx.ctx.fonts_mut(|f| {
        f.layout_no_wrap(
            glyph.to_owned(),
            FontId::new((radius * 1.35).min(MAX_FONT_PX), FontFamily::Proportional),
            fg,
        )
    });
    res.push(TextShape::new(center - g.size() / 2.0, g, fg).into());
}

pub fn badge_fg(bg: Color32) -> Result<Color32, Error> {
    let lum = 0.299 * bg.r() as f32 + 0.587 * bg.g() as f32 + 0.114 * bg.b() as f32;
    if lum > 150.0 {
        return Ok(Color32::from_rgb(25, 25, 30));
    }
    Ok(Color32::from_rgb(245, 245, 245))
}

pub fn backdrop(res: &mut Vec<Shape>, rect: Rect, bg: Color32, dark: bool) -> Result<(), Error> {
    let fill = theme_shift(bg, dark, BACKDROP_MIX)?;
    let line = theme_shift(bg, dark, BACKDROP_LINE_MIX)?;
    res.push(
        RectShape::new(
            rect,
            CornerRadius::same(2),
            fill,
            Stroke::new(1.0_f32, line),
            StrokeKind::Inside,
        )
        .into(),
    );
    Ok(())
}

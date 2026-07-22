use std::cell::Cell;

use egui::epaint::{RectShape, TextShape};
use egui::text::{LayoutJob, TextFormat};
use egui::{Align, Color32, CornerRadius, FontFamily, FontId, Pos2, Rect, Shape, Stroke, Vec2};
use egui_graphs::{DisplayEdge, DisplayNode, DrawContext, EdgeProps, Node};
use petgraph::{EdgeType, stable_graph::IndexType};

use egui_phosphor::regular::{LOCK, PAPERCLIP, WARNING};

use crate::error::Error;
use crate::model::{
    RELATION_META, Relation, RelationKind, SOURCE_TIER_META, Source, SourceTier, Verification,
    status_color,
};

use super::label::{claim_edge_label, theme_shift};

const TIP_ANGLE: f32 = std::f32::consts::TAU / 30.0;
const TIP_SIZE: f32 = 12.0;
const PARALLEL_SPACING: f32 = 12.0;
const WIDTH: f32 = 3.0;
const HOT_MULT: f32 = 1.9;
const GLOW_MULT: f32 = 3.2;
const HIT_TOLERANCE: f32 = 6.0;
const HOVER_PX: f32 = 9.0;

const EDGE_LABEL_SIZE: f32 = 11.0;
const EDGE_LABEL_MIN_PX: f32 = 8.0;
const EDGE_LABEL_MAX_PX: f32 = 40.0;
const EDGE_LABEL_PAD_FRAC: f32 = 0.3;
const EDGE_LABEL_PAD_MIN: f32 = 2.0;
const EDGE_BACKDROP_ALPHA: f32 = 0.94;
const EDGE_BACKDROP_LIGHTEN: f32 = 0.16;
const EDGE_BACKDROP_DARKEN: f32 = 0.17;
const EDGE_WARN_MULT: f32 = 1.4;

const EDGE_LABEL_WARN_PRIORITY: f32 = 1000.0;
const EDGE_LABEL_SEALED_PRIORITY: f32 = 750.0;
const EDGE_LABEL_HOT_PRIORITY: f32 = 500.0;
const EDGE_LABEL_VERB_PRIORITY: f32 = 0.0;

const SEALED_LINE_DIM: f32 = 0.6;

thread_local! {
    static SHOT_LABELS: Cell<bool> = const { Cell::new(false) };
}

fn shot_labels_flag() -> bool {
    SHOT_LABELS.with(|c| c.get())
}

#[derive(Clone)]
pub struct SourcedEdgeShape {
    kind: RelationKind,
    sourced: bool,
    source_count: usize,
    tier: SourceTier,
    status: Verification,
    sealed: bool,
    selected: bool,
    order: usize,
    hidden: bool,
}

impl From<EdgeProps<Relation>> for SourcedEdgeShape {
    fn from(props: EdgeProps<Relation>) -> Self {
        Self {
            kind: props.payload.kind,
            sourced: !props.payload.sources.is_empty(),
            source_count: props.payload.sources.len(),
            tier: strongest_tier(&props.payload.sources),
            status: props.payload.status,
            sealed: props.payload.sealed,
            selected: props.selected,
            order: props.order,
            hidden: props.payload.hidden,
        }
    }
}

impl<N, Ty, Ix, D> DisplayEdge<N, Relation, Ty, Ix, D> for SourcedEdgeShape
where
    N: Clone,
    Ty: EdgeType,
    Ix: IndexType,
    D: DisplayNode<N, Relation, Ty, Ix>,
{
    fn shapes(
        &mut self,
        start: &Node<N, Relation, Ty, Ix, D>,
        end: &Node<N, Relation, Ty, Ix, D>,
        ctx: &DrawContext,
    ) -> Vec<Shape> {
        if self.hidden {
            return Vec::new();
        }
        let (s, e) = trim_to_boundaries(start, end);
        let (s, e) = offset_ends(s, e, self.order);
        let dark = ctx.ctx.global_style().visuals.dark_mode;
        let Ok(raw) = status_color(self.status) else {
            return Vec::new();
        };
        let Ok(color) = crate::ui::board_color(raw, dark) else {
            return Vec::new();
        };

        let a = ctx.meta.canvas_to_screen_pos(s);
        let b = ctx.meta.canvas_to_screen_pos(e);
        let hovered = ctx
            .ctx
            .input(|i| i.pointer.hover_pos())
            .is_some_and(|p| dist_point_segment(p, a, b) <= HOVER_PX);
        let hot = self.selected || hovered;
        let mult = if hot { HOT_MULT } else { 1.0 };
        let line_color = if self.sealed { mute_edge(color) } else { color };
        let stroke = Stroke::new(ctx.meta.canvas_to_screen_size(WIDTH) * mult, line_color);

        let mut res = Vec::new();
        if hot {
            let halo = Stroke::new(stroke.width * GLOW_MULT, line_color.gamma_multiply(0.25));
            res.push(Shape::line_segment([a, b], halo));
        }
        if self.sourced {
            res.push(Shape::line_segment([a, b], stroke));
        } else {
            res.extend(Shape::dashed_line(
                &[a, b],
                stroke,
                ctx.meta.canvas_to_screen_size(9.0),
                ctx.meta.canvas_to_screen_size(6.0),
            ));
        }

        let dir = (e - s).normalized();
        let t1 = e - rotate(dir, TIP_ANGLE) * TIP_SIZE * mult;
        let t2 = e - rotate(dir, -TIP_ANGLE) * TIP_SIZE * mult;
        let tip = vec![
            ctx.meta.canvas_to_screen_pos(e),
            ctx.meta.canvas_to_screen_pos(t1),
            ctx.meta.canvas_to_screen_pos(t2),
        ];
        res.push(Shape::convex_polygon(tip, line_color, Stroke::NONE));

        let mid = a + (b - a) * 0.5;
        let id = edge_id(start.location(), end.location(), self.kind, self.order);
        res.extend(self.label_shapes(ctx, id, mid, hot, color, dark));

        res
    }

    fn update(&mut self, state: &EdgeProps<Relation>) {
        self.kind = state.payload.kind;
        self.sourced = !state.payload.sources.is_empty();
        self.source_count = state.payload.sources.len();
        self.tier = strongest_tier(&state.payload.sources);
        self.status = state.payload.status;
        self.sealed = state.payload.sealed;
        self.selected = state.selected;
        self.order = state.order;
        self.hidden = state.payload.hidden;
    }

    fn is_inside(
        &self,
        start: &Node<N, Relation, Ty, Ix, D>,
        end: &Node<N, Relation, Ty, Ix, D>,
        pos: Pos2,
    ) -> bool {
        if self.hidden {
            return false;
        }
        let (s, e) = trim_to_boundaries(start, end);
        let (s, e) = offset_ends(s, e, self.order);
        dist_point_segment(pos, s, e) <= HIT_TOLERANCE
    }
}

impl SourcedEdgeShape {
    pub fn set_shot_labels(on: bool) -> Result<(), Error> {
        SHOT_LABELS.with(|c| c.set(on));
        Ok(())
    }

    fn label_shapes(
        &self,
        ctx: &DrawContext,
        id: u64,
        mid: Pos2,
        hot: bool,
        color: Color32,
        dark: bool,
    ) -> Vec<Shape> {
        let Some(parts) = edge_label_parts(
            self.sourced,
            self.source_count,
            self.kind,
            hot,
            shot_labels_flag(),
            self.sealed,
        ) else {
            return Vec::new();
        };
        let fpx = ctx
            .meta
            .canvas_to_screen_size(EDGE_LABEL_SIZE)
            .clamp(EDGE_LABEL_MIN_PX, EDGE_LABEL_MAX_PX);
        let tail_px = if parts.warn { fpx * EDGE_WARN_MULT } else { fpx };
        let tail_color = if parts.warn {
            warn_color(dark)
        } else {
            tier_color(self.tier, dark)
        };
        let mut job = LayoutJob::default();
        job.append(
            &parts.verb,
            0.0,
            TextFormat {
                font_id: FontId::new(fpx, FontFamily::Proportional),
                color,
                valign: Align::Center,
                ..Default::default()
            },
        );
        job.append(
            &parts.tail,
            0.0,
            TextFormat {
                font_id: FontId::new(tail_px, FontFamily::Proportional),
                color: tail_color,
                valign: Align::Center,
                ..Default::default()
            },
        );
        let galley = ctx.ctx.fonts_mut(|f| f.layout_job(job));
        let sz = galley.size();
        let pad = (fpx * EDGE_LABEL_PAD_FRAC).max(EDGE_LABEL_PAD_MIN);
        let box_rect = Rect::from_center_size(mid, sz + Vec2::splat(pad * 2.0));
        let (priority, force) = edge_label_claim(parts.warn, hot, parts.sealed);
        let Ok(draw) = claim_edge_label(ctx.ctx, id, box_rect, priority, force) else {
            return Vec::new();
        };
        if !draw {
            return Vec::new();
        }
        let bg = ctx.ctx.global_style().visuals.panel_fill;
        let fill = label_backdrop_fill(bg, dark).gamma_multiply(EDGE_BACKDROP_ALPHA);
        let mut res = Vec::with_capacity(2);
        res.push(RectShape::filled(box_rect, CornerRadius::same(3), fill).into());
        res.push(TextShape::new(mid - sz / 2.0, galley, color).into());
        res
    }
}

struct LabelParts {
    verb: String,
    tail: String,
    warn: bool,
    sealed: bool,
}

fn edge_label_parts(
    sourced: bool,
    count: usize,
    kind: RelationKind,
    hot: bool,
    always: bool,
    sealed: bool,
) -> Option<LabelParts> {
    let base = RELATION_META[kind as usize].label;
    let verb = if sealed { format!("{base}  {LOCK}") } else { base.to_owned() };
    if !sourced {
        return Some(LabelParts {
            verb,
            tail: format!("  {WARNING} 0 src"),
            warn: true,
            sealed,
        });
    }
    if !hot && !always && !sealed {
        return None;
    }
    Some(LabelParts {
        verb,
        tail: format!("  {PAPERCLIP} {count}"),
        warn: false,
        sealed,
    })
}

fn edge_label_claim(warn: bool, hot: bool, sealed: bool) -> (f32, bool) {
    let priority = if warn {
        EDGE_LABEL_WARN_PRIORITY
    } else if sealed {
        EDGE_LABEL_SEALED_PRIORITY
    } else if hot {
        EDGE_LABEL_HOT_PRIORITY
    } else {
        EDGE_LABEL_VERB_PRIORITY
    };
    (priority, warn || hot || sealed)
}

fn mute_edge(c: Color32) -> Color32 {
    c.gamma_multiply(SEALED_LINE_DIM)
}

fn edge_id(a: Pos2, b: Pos2, kind: RelationKind, order: usize) -> u64 {
    let words = [
        a.x.round() as i64 as u64, a.y.round() as i64 as u64,
        b.x.round() as i64 as u64, b.y.round() as i64 as u64,
        kind as u32 as u64, order as u64,
    ];
    let mut hash: u64 = 0xcbf29ce484222325;
    for w in words {
        for byte in w.to_le_bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    hash | (1u64 << 63)
}

#[cfg(test)]
fn edge_label_text(
    sourced: bool,
    count: usize,
    kind: RelationKind,
    hot: bool,
    always: bool,
    sealed: bool,
) -> Option<String> {
    let parts = edge_label_parts(sourced, count, kind, hot, always, sealed)?;
    Some(format!("{}{}", parts.verb, parts.tail))
}

fn warn_color(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(255, 96, 72)
    } else {
        Color32::from_rgb(198, 30, 18)
    }
}

fn tier_rank(tier: SourceTier) -> u8 {
    match tier {
        SourceTier::Primary => 3,
        SourceTier::Secondary => 2,
        SourceTier::Reported => 1,
        SourceTier::Unrated => 0,
    }
}

fn strongest_tier(sources: &[Source]) -> SourceTier {
    let mut best = SourceTier::Unrated;
    for s in sources {
        if tier_rank(s.tier) > tier_rank(best) {
            best = s.tier;
        }
    }
    best
}

fn tier_color(tier: SourceTier, dark: bool) -> Color32 {
    let raw = SOURCE_TIER_META[tier as usize].color;
    let Ok(c) = crate::ui::board_color(raw, dark) else {
        return raw;
    };
    c
}

fn label_backdrop_fill(bg: Color32, dark: bool) -> Color32 {
    let mix = if dark { EDGE_BACKDROP_LIGHTEN } else { EDGE_BACKDROP_DARKEN };
    let Ok(c) = theme_shift(bg, dark, mix) else {
        return bg;
    };
    c
}

fn trim_to_boundaries<N, Ty, Ix, D>(
    start: &Node<N, Relation, Ty, Ix, D>,
    end: &Node<N, Relation, Ty, Ix, D>,
) -> (Pos2, Pos2)
where
    N: Clone,
    Ty: EdgeType,
    Ix: IndexType,
    D: DisplayNode<N, Relation, Ty, Ix>,
{
    let dir = (end.location() - start.location()).normalized();
    (
        start.display().closest_boundary_point(dir),
        end.display().closest_boundary_point(-dir),
    )
}

fn offset_ends(s: Pos2, e: Pos2, order: usize) -> (Pos2, Pos2) {
    if order == 0 {
        return (s, e);
    }
    let dir = (e - s).normalized();
    let perp = Vec2::new(-dir.y, dir.x);
    let off = perp * (order as f32) * PARALLEL_SPACING;
    (s + off, e + off)
}

fn rotate(v: Vec2, angle: f32) -> Vec2 {
    let (c, s) = (angle.cos(), angle.sin());
    Vec2::new(c * v.x - s * v.y, s * v.x + c * v.y)
}

fn dist_point_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let len2 = ab.length_sq();
    if len2 == 0.0 {
        return (p - a).length();
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    let proj = a + ab * t;
    (p - proj).length()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Author, Entity, EntityKind, Origin, Source, SourceKind};
    use crate::ui::testkit::nodes;
    use crate::shapes::EntityNodeShape;
    use egui_graphs::{MetadataFrame, SettingsStyle};
    use petgraph::{Directed, stable_graph::DefaultIx};

    fn ent() -> Entity {
        nodes::entity(EntityKind::Person, "n", "N", "")
    }

    fn rel(kind: RelationKind, sources: Vec<Source>) -> Relation {
        Relation {
            kind,
            origin: Origin::default(),
            status: Verification::Unreviewed,
            confidence: 100,
            author: Author::default(),
            created_at: 0,
            sources,
            valid_from: None,
            valid_to: None,
            sealed: false,
            hidden: false,
        }
    }

    fn edge_shapes(relation: Relation) -> Vec<Shape> {
        let mut es = SourcedEdgeShape::from(EdgeProps {
            payload: relation,
            order: 0,
            selected: false,
            label: String::new(),
        });
        let n1 = Node::<Entity, Relation, Directed, DefaultIx, EntityNodeShape>::new(ent());
        let mut n2 = Node::<Entity, Relation, Directed, DefaultIx, EntityNodeShape>::new(ent());
        n2.set_location(Pos2::new(200.0, 0.0));
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
            out = es.shapes(&n1, &n2, &dctx);
        });
        out
    }

    fn text_shapes(shapes: &[Shape]) -> Vec<String> {
        let mut out = Vec::new();
        for s in shapes {
            if let Shape::Text(t) = s {
                out.push(t.galley.text().to_owned());
            }
        }
        out
    }

    fn tail_section_color(shapes: &[Shape]) -> Option<Color32> {
        for s in shapes {
            if let Shape::Text(t) = s {
                let secs = &t.galley.job.sections;
                if secs.len() >= 2 {
                    return Some(secs[1].format.color);
                }
            }
        }
        None
    }

    fn src_tier(tier: SourceTier) -> Source {
        Source {
            kind: SourceKind::Note,
            reference: "r".into(),
            tier,
            ..Default::default()
        }
    }

    fn color_dist(a: Color32, b: Color32) -> f32 {
        let dr = a.r() as f32 - b.r() as f32;
        let dg = a.g() as f32 - b.g() as f32;
        let db = a.b() as f32 - b.b() as f32;
        (dr * dr + dg * dg + db * db).sqrt()
    }

    #[test]
    fn edge_label_text_gates_and_warns() {
        let controls = RELATION_META[RelationKind::Controls as usize].label;
        let financed = RELATION_META[RelationKind::Financed as usize].label;

        let un = edge_label_text(false, 0, RelationKind::Controls, false, false, false).unwrap();
        assert!(un.contains(WARNING), "unsourced label must carry the warning glyph");
        assert!(un.contains(controls), "unsourced label must name the verb");

        assert!(
            edge_label_text(true, 7, RelationKind::Financed, false, false, false).is_none(),
            "a sourced edge at rest with labels off must not draw a label"
        );

        let hot = edge_label_text(true, 7, RelationKind::Financed, true, false, false).unwrap();
        assert!(hot.contains('7'), "a hot sourced label must show the source count");
        assert!(hot.contains(financed), "a sourced label must name the verb");

        let always = edge_label_text(true, 7, RelationKind::Financed, false, true, false).unwrap();
        assert!(
            always.contains(financed) && always.contains('7'),
            "always-label mode must render a sourced verb + count even at rest, got {always}"
        );
    }

    #[test]
    fn unsourced_edge_draws_warning_shape() {
        SourcedEdgeShape::set_shot_labels(false).unwrap();
        let controls = RELATION_META[RelationKind::Controls as usize].label;
        let texts = text_shapes(&edge_shapes(rel(RelationKind::Controls, Vec::new())));
        assert!(
            texts.iter().any(|t| t.contains(WARNING)),
            "an unsourced edge must render a warning-text shape, got {texts:?}"
        );
        assert!(
            texts.iter().any(|t| t.contains(controls)),
            "the verb must render on the edge, got {texts:?}"
        );
    }

    #[test]
    fn sourced_edge_at_rest_has_no_label() {
        SourcedEdgeShape::set_shot_labels(false).unwrap();
        let src = vec![Source {
            kind: SourceKind::Note,
            reference: "n".into(),
            excerpt: String::new(),
            ..Default::default()
        }];
        let texts = text_shapes(&edge_shapes(rel(RelationKind::Controls, src)));
        assert!(
            texts.is_empty(),
            "a sourced edge at rest must not render a label, got {texts:?}"
        );
    }

    #[test]
    fn shot_labels_flag_reveals_sourced_verb() {
        let controls = RELATION_META[RelationKind::Controls as usize].label;
        let src = vec![Source {
            kind: SourceKind::Note,
            reference: "n".into(),
            excerpt: String::new(),
            ..Default::default()
        }];
        SourcedEdgeShape::set_shot_labels(true).unwrap();
        let texts = text_shapes(&edge_shapes(rel(RelationKind::Controls, src)));
        SourcedEdgeShape::set_shot_labels(false).unwrap();
        assert!(
            texts.iter().any(|t| t.contains(controls)),
            "shot always-label mode must render a sourced edge's verb, got {texts:?}"
        );
    }

    #[test]
    fn light_theme_darkens_edge_color() {
        let raw = status_color(Verification::MachineSuggested).unwrap();
        let dark = crate::ui::board_color(raw, true).unwrap();
        let light = crate::ui::board_color(raw, false).unwrap();
        let lum = |c: Color32| 0.299 * c.r() as f32 + 0.587 * c.g() as f32 + 0.114 * c.b() as f32;
        assert!(
            lum(light) < lum(dark) - 20.0,
            "light theme must darken the edge status color for legibility: dark {} vs light {}",
            lum(dark),
            lum(light)
        );
    }

    #[test]
    fn edge_label_priority_wiring() {
        let (warn_priority, warn_force) = edge_label_claim(true, false, false);
        assert!(
            warn_force,
            "an unsourced-edge warning must be force-drawn so it can never be hidden in a pile-up"
        );
        assert!(
            warn_priority > 12.0,
            "warning priority must outrank the largest node label (max size 12) so a colliding node label yields"
        );

        let (hot_priority, hot_force) = edge_label_claim(false, true, false);
        assert!(hot_force, "a hovered or selected edge label must be force-drawn");
        assert!(
            hot_priority > 12.0,
            "hot-edge priority must outrank node labels so the focused label is not buried"
        );

        let (verb_priority, verb_force) = edge_label_claim(false, false, false);
        assert!(
            !verb_force,
            "a sourced verb at rest must be gated, not forced, so it can drop out of a dense knot"
        );
        assert!(
            verb_priority < 0.5,
            "sourced-verb priority must sit below every node label (min size 0.5) so it yields first"
        );
    }

    #[test]
    fn strongest_tier_picks_best_credibility() {
        assert_eq!(strongest_tier(&[]), SourceTier::Unrated, "no sources -> unrated");
        assert_eq!(
            strongest_tier(&[src_tier(SourceTier::Unrated), src_tier(SourceTier::Unrated)]),
            SourceTier::Unrated,
            "an all-unrated edge must stay unrated so weak-only sourcing reads weak"
        );
        assert_eq!(
            strongest_tier(&[
                src_tier(SourceTier::Reported),
                src_tier(SourceTier::Primary),
                src_tier(SourceTier::Unrated),
            ]),
            SourceTier::Primary,
            "a single primary source wins the tint because it is what makes the claim defensible"
        );
        assert_eq!(
            strongest_tier(&[src_tier(SourceTier::Reported), src_tier(SourceTier::Secondary)]),
            SourceTier::Secondary,
            "secondary outranks reported when no primary is present"
        );
    }

    #[test]
    fn tier_colors_distinct_and_legible_both_themes() {
        let tiers = [
            SourceTier::Unrated,
            SourceTier::Primary,
            SourceTier::Secondary,
            SourceTier::Reported,
        ];
        for &dark in &[true, false] {
            let cols: Vec<Color32> = tiers.iter().map(|&t| tier_color(t, dark)).collect();
            for i in 0..cols.len() {
                for j in (i + 1)..cols.len() {
                    assert!(
                        color_dist(cols[i], cols[j]) > 35.0,
                        "tiers {:?}/{:?} must stay visually distinct (dark={dark}): {:?} vs {:?}",
                        tiers[i],
                        tiers[j],
                        cols[i],
                        cols[j]
                    );
                }
            }
        }
        let lum = |c: Color32| 0.299 * c.r() as f32 + 0.587 * c.g() as f32 + 0.114 * c.b() as f32;
        for &t in &tiers {
            assert!(
                lum(tier_color(t, false)) < lum(tier_color(t, true)),
                "light theme must darken tier {t:?} so the badge stays legible on a light board"
            );
        }
    }

    #[test]
    fn sourced_badge_tinted_by_strongest_tier() {
        SourcedEdgeShape::set_shot_labels(true).unwrap();
        let prim = tail_section_color(&edge_shapes(rel(
            RelationKind::Controls,
            vec![src_tier(SourceTier::Primary)],
        )));
        let unr = tail_section_color(&edge_shapes(rel(
            RelationKind::Controls,
            vec![src_tier(SourceTier::Unrated), src_tier(SourceTier::Unrated)],
        )));
        SourcedEdgeShape::set_shot_labels(false).unwrap();
        let prim = prim.expect("a sourced edge must render a badge tail section");
        let unr = unr.expect("an all-unrated edge must render a badge tail section");
        assert_eq!(
            prim,
            tier_color(SourceTier::Primary, true),
            "the badge tail must take the strongest present tier's board color"
        );
        assert_ne!(
            prim, unr,
            "a primary-sourced badge must be tinted apart from an all-unrated one"
        );
    }

    fn sealed(mut r: Relation) -> Relation {
        r.sealed = true;
        r
    }

    #[test]
    fn sealed_edge_shows_lock_and_forces_label_at_rest() {
        SourcedEdgeShape::set_shot_labels(false).unwrap();
        let controls = RELATION_META[RelationKind::Controls as usize].label;
        let texts = text_shapes(&edge_shapes(sealed(rel(
            RelationKind::Controls,
            vec![src_tier(SourceTier::Primary)],
        ))));
        assert!(
            texts.iter().any(|t| t.contains(LOCK)),
            "a sealed edge must render the lock glyph, got {texts:?}"
        );
        assert!(
            texts.iter().any(|t| t.contains(controls)),
            "a sealed sourced edge must force its label on even at rest so the lock is visible, got {texts:?}"
        );
    }

    #[test]
    fn unsealed_sourced_edge_has_no_lock() {
        SourcedEdgeShape::set_shot_labels(true).unwrap();
        let texts = text_shapes(&edge_shapes(rel(
            RelationKind::Controls,
            vec![src_tier(SourceTier::Primary)],
        )));
        SourcedEdgeShape::set_shot_labels(false).unwrap();
        assert!(
            !texts.iter().any(|t| t.contains(LOCK)),
            "an unsealed edge must never render the lock glyph, got {texts:?}"
        );
    }

    #[test]
    fn sealed_unsourced_edge_keeps_the_warning_and_adds_lock() {
        SourcedEdgeShape::set_shot_labels(false).unwrap();
        let texts = text_shapes(&edge_shapes(sealed(rel(RelationKind::Controls, Vec::new()))));
        assert!(
            texts.iter().any(|t| t.contains(WARNING)),
            "a sealed UNSOURCED edge must still read as unsourced (warning glyph kept), got {texts:?}"
        );
        assert!(
            texts.iter().any(|t| t.contains(LOCK)),
            "a sealed unsourced edge must also carry the lock, got {texts:?}"
        );
    }

    #[test]
    fn sealed_edge_label_is_forced_and_outranks_node_labels() {
        let (priority, force) = edge_label_claim(false, false, true);
        assert!(
            force,
            "a sealed sourced edge at rest must force its label so the redaction cue is never hidden"
        );
        assert!(
            priority > 12.0,
            "sealed-edge priority must outrank the largest node label so the lock is not buried"
        );
    }

    #[test]
    fn mute_edge_recedes_the_line() {
        let base = Color32::from_rgb(200, 160, 120);
        let muted = mute_edge(base);
        assert!(
            muted.a() < base.a(),
            "a sealed edge line must lose alpha so it visually recedes: {muted:?} vs {base:?}"
        );
    }
}

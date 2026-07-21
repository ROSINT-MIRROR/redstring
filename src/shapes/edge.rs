






use std::cell::Cell;

use egui::epaint::{RectShape, TextShape};
use egui::text::{LayoutJob, TextFormat};
use egui::{Align, Color32, CornerRadius, FontFamily, FontId, Pos2, Rect, Shape, Stroke, Vec2};
use egui_graphs::{DisplayEdge, DisplayNode, DrawContext, EdgeProps, Node};
use petgraph::{EdgeType, stable_graph::IndexType};

use egui_phosphor::regular::{PAPERCLIP, WARNING};

use crate::error::Error;
use crate::model::{RELATION_META, Relation, RelationKind, Verification, status_color};

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
    status: Verification,
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
            status: props.payload.status,
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
        let stroke = Stroke::new(ctx.meta.canvas_to_screen_size(WIDTH) * mult, color);

        let mut res = Vec::new();
        if hot {
            let halo = Stroke::new(stroke.width * GLOW_MULT, color.gamma_multiply(0.25));
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
        res.push(Shape::convex_polygon(tip, color, Stroke::NONE));

        let mid = a + (b - a) * 0.5;
        res.extend(self.label_shapes(ctx, mid, hot, color, dark));

        res
    }

    fn update(&mut self, state: &EdgeProps<Relation>) {
        self.kind = state.payload.kind;
        self.sourced = !state.payload.sources.is_empty();
        self.source_count = state.payload.sources.len();
        self.status = state.payload.status;
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
        ) else {
            return Vec::new();
        };
        let fpx = ctx
            .meta
            .canvas_to_screen_size(EDGE_LABEL_SIZE)
            .clamp(EDGE_LABEL_MIN_PX, EDGE_LABEL_MAX_PX);
        let tail_px = if parts.warn { fpx * EDGE_WARN_MULT } else { fpx };
        let tail_color = if parts.warn { warn_color(dark) } else { color };
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
}

fn edge_label_parts(
    sourced: bool,
    count: usize,
    kind: RelationKind,
    hot: bool,
    always: bool,
) -> Option<LabelParts> {
    let verb = RELATION_META[kind as usize].label.to_owned();
    if !sourced {
        return Some(LabelParts {
            verb,
            tail: format!("  {WARNING} 0 src"),
            warn: true,
        });
    }
    if !hot && !always {
        return None;
    }
    Some(LabelParts {
        verb,
        tail: format!("  {PAPERCLIP} {count}"),
        warn: false,
    })
}

#[cfg(test)]
fn edge_label_text(
    sourced: bool,
    count: usize,
    kind: RelationKind,
    hot: bool,
    always: bool,
) -> Option<String> {
    let parts = edge_label_parts(sourced, count, kind, hot, always)?;
    Some(format!("{}{}", parts.verb, parts.tail))
}

fn warn_color(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(255, 96, 72)
    } else {
        Color32::from_rgb(198, 30, 18)
    }
}

fn label_backdrop_fill(bg: Color32, dark: bool) -> Color32 {
    if dark {
        bg.lerp_to_gamma(Color32::WHITE, EDGE_BACKDROP_LIGHTEN)
    } else {
        bg.lerp_to_gamma(Color32::BLACK, EDGE_BACKDROP_DARKEN)
    }
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
    use crate::model::{Author, DEFAULT_NODE_SIZE, Entity, EntityKind, Origin, Role, Source, SourceKind};
    use crate::shapes::EntityNodeShape;
    use egui_graphs::{MetadataFrame, SettingsStyle};
    use petgraph::{Directed, stable_graph::DefaultIx};

    fn ent() -> Entity {
        Entity {
            slug: "n".into(),
            kind: EntityKind::Person,
            label: "N".into(),
            value: String::new(),
            observations: Vec::new(),
            role: Role::Unset,
            occupation: None,
            note: String::new(),
            image: None,
            size: DEFAULT_NODE_SIZE,
            boost: 1.0,
            collapsed: false,
            hidden: false,
        }
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

    #[test]
    fn edge_label_text_gates_and_warns() {
        let controls = RELATION_META[RelationKind::Controls as usize].label;
        let financed = RELATION_META[RelationKind::Financed as usize].label;

        let un = edge_label_text(false, 0, RelationKind::Controls, false, false).unwrap();
        assert!(un.contains(WARNING), "unsourced label must carry the warning glyph");
        assert!(un.contains(controls), "unsourced label must name the verb");

        assert!(
            edge_label_text(true, 7, RelationKind::Financed, false, false).is_none(),
            "a sourced edge at rest with labels off must not draw a label"
        );

        let hot = edge_label_text(true, 7, RelationKind::Financed, true, false).unwrap();
        assert!(hot.contains('7'), "a hot sourced label must show the source count");
        assert!(hot.contains(financed), "a sourced label must name the verb");

        let always = edge_label_text(true, 7, RelationKind::Financed, false, true).unwrap();
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
}

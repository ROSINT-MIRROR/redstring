






use egui::{Color32, Pos2, Shape, Stroke, Vec2};
use egui_graphs::{DisplayEdge, DisplayNode, DrawContext, EdgeProps, Node};
use petgraph::{EdgeType, stable_graph::IndexType};

use crate::model::Relation;

const TIP_ANGLE: f32 = std::f32::consts::TAU / 30.0;
const TIP_SIZE: f32 = 12.0;
const PARALLEL_SPACING: f32 = 12.0;
const WIDTH: f32 = 3.0;
const HOT_MULT: f32 = 1.9;
const GLOW_MULT: f32 = 3.2;
const HIT_TOLERANCE: f32 = 6.0;
const HOVER_PX: f32 = 9.0;

const SOURCED: Color32 = Color32::from_rgb(120, 200, 140);
const UNSOURCED: Color32 = Color32::from_rgb(235, 80, 80);

#[derive(Clone)]
pub struct SourcedEdgeShape {
    sourced: bool,
    selected: bool,
    order: usize,
}

impl From<EdgeProps<Relation>> for SourcedEdgeShape {
    fn from(props: EdgeProps<Relation>) -> Self {
        Self {
            sourced: !props.payload.sources.is_empty(),
            selected: props.selected,
            order: props.order,
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
        let (s, e) = trim_to_boundaries(start, end);
        let (s, e) = offset_ends(s, e, self.order);
        let color = if self.sourced { SOURCED } else { UNSOURCED };

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

        res
    }

    fn update(&mut self, state: &EdgeProps<Relation>) {
        self.sourced = !state.payload.sources.is_empty();
        self.selected = state.selected;
        self.order = state.order;
    }

    fn is_inside(
        &self,
        start: &Node<N, Relation, Ty, Ix, D>,
        end: &Node<N, Relation, Ty, Ix, D>,
        pos: Pos2,
    ) -> bool {
        let (s, e) = trim_to_boundaries(start, end);
        let (s, e) = offset_ends(s, e, self.order);
        dist_point_segment(pos, s, e) <= HIT_TOLERANCE
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

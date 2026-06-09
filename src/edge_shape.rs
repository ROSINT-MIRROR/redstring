//! Custom edge renderer. The point of redstring: an unsourced link is drawn
//! red + dashed (an accusation you can't back up), a sourced link solid green.
//! Self-contained straight-line + arrowhead drawing; parallel edges between the
//! same pair are fanned out by `order` so they don't overlap.

use egui::{Color32, Pos2, Shape, Stroke, Vec2};
use egui_graphs::{DisplayEdge, DisplayNode, DrawContext, EdgeProps, Node};
use petgraph::{stable_graph::IndexType, EdgeType};

use crate::model::Relation;

const TIP_ANGLE: f32 = std::f32::consts::TAU / 30.0;
const TIP_SIZE: f32 = 12.0;
const PARALLEL_SPACING: f32 = 12.0;
const WIDTH: f32 = 2.0;
const HIT_TOLERANCE: f32 = 4.0;

const SOURCED: Color32 = Color32::from_rgb(120, 200, 140);
const UNSOURCED: Color32 = Color32::from_rgb(235, 80, 80);

#[derive(Clone)]
pub struct SourcedEdgeShape {
    sourced: bool,
    order: usize,
}

impl From<EdgeProps<Relation>> for SourcedEdgeShape {
    fn from(props: EdgeProps<Relation>) -> Self {
        Self {
            sourced: props.payload.is_sourced(),
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
        let (s, e) = offset_ends(start.location(), end.location(), self.order);
        let color = if self.sourced { SOURCED } else { UNSOURCED };

        let a = ctx.meta.canvas_to_screen_pos(s);
        let b = ctx.meta.canvas_to_screen_pos(e);
        let stroke = Stroke::new(ctx.meta.canvas_to_screen_size(WIDTH), color);

        let mut res = Vec::new();
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

        // arrowhead, built in canvas space then projected so it scales with zoom
        let dir = (e - s).normalized();
        let t1 = e - rotate(dir, TIP_ANGLE) * TIP_SIZE;
        let t2 = e - rotate(dir, -TIP_ANGLE) * TIP_SIZE;
        let tip = vec![
            ctx.meta.canvas_to_screen_pos(e),
            ctx.meta.canvas_to_screen_pos(t1),
            ctx.meta.canvas_to_screen_pos(t2),
        ];
        res.push(Shape::convex_polygon(tip, color, Stroke::NONE));

        res
    }

    fn update(&mut self, state: &EdgeProps<Relation>) {
        self.sourced = state.payload.is_sourced();
        self.order = state.order;
    }

    fn is_inside(
        &self,
        start: &Node<N, Relation, Ty, Ix, D>,
        end: &Node<N, Relation, Ty, Ix, D>,
        pos: Pos2,
    ) -> bool {
        let (s, e) = offset_ends(start.location(), end.location(), self.order);
        dist_point_segment(pos, s, e) <= HIT_TOLERANCE
    }
}

/// Fan parallel edges apart perpendicular to their direction.
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

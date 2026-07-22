use std::collections::HashSet;

use egui::{Pos2, Rect, Vec2};
use petgraph::graph::NodeIndex;

use crate::error::Error;
use crate::model::Camera;
use crate::shapes::base_radius;

use super::app::RsGraph;
use super::board::{clamp_zoom, fit_box_to_board};
use super::graphalg;

const CENTER_MARGIN: f32 = 0.85;
const CENTER_MAX_ZOOM: f32 = 1.5;
const MIN_CLUSTER: usize = 8;
const OUTLIER_IQR_K: f32 = 1.5;
const OUTLIER_SLACK: f32 = 1.1;
const QUARTILE_LOW: f32 = 0.25;
const QUARTILE_HIGH: f32 = 0.75;

pub fn center_on_node(loc: Pos2, board: Vec2, zoom: f32) -> Result<Camera, Error> {
    let z = clamp_zoom(zoom)?;
    if board.x < 1.0 || board.y < 1.0 {
        return Ok(Camera { x: 0.0, y: 0.0, zoom: z });
    }
    let pan = board / 2.0 - loc.to_vec2() * z;
    Ok(Camera {
        x: pan.x,
        y: pan.y,
        zoom: z,
    })
}

fn visible_set(g: &RsGraph) -> HashSet<NodeIndex> {
    let mut set = HashSet::new();
    for idx in g.g().node_indices() {
        let Some(n) = g.node(idx) else {
            continue;
        };
        if !n.payload().hidden {
            set.insert(idx);
        }
    }
    set
}

fn member_points(g: &RsGraph, members: &[NodeIndex]) -> Result<Vec<(Pos2, f32)>, Error> {
    let mut pts: Vec<(Pos2, f32)> = Vec::new();
    for &idx in members {
        let Some(n) = g.node(idx) else {
            continue;
        };
        let e = n.payload();
        let half = base_radius(e)? * e.size * e.boost;
        pts.push((n.location(), half));
    }
    Ok(pts)
}

pub fn cluster_bounds(pts: &[(Pos2, f32)]) -> Result<Rect, Error> {
    if pts.is_empty() {
        return Err(Error::Render {
            what: "cluster bounds: no visible nodes".to_owned(),
        });
    }
    if pts.len() < MIN_CLUSTER {
        return Ok(bounds_of(pts));
    }
    let mut xs: Vec<f32> = pts.iter().map(|(p, _)| p.x).collect();
    let mut ys: Vec<f32> = pts.iter().map(|(p, _)| p.y).collect();
    let center = Pos2::new(median(&mut xs), median(&mut ys));
    let mut dists: Vec<f32> = pts.iter().map(|(p, _)| (*p - center).length()).collect();
    let q1 = percentile(&mut dists, QUARTILE_LOW);
    let q3 = percentile(&mut dists, QUARTILE_HIGH);
    let iqr = (q3 - q1).max(0.0);
    let cut = (q3 + OUTLIER_IQR_K * iqr).max(q3 * OUTLIER_SLACK);
    let kept: Vec<(Pos2, f32)> = pts
        .iter()
        .copied()
        .filter(|(p, _)| (*p - center).length() <= cut)
        .collect();
    if kept.is_empty() {
        return Ok(bounds_of(pts));
    }
    Ok(bounds_of(&kept))
}

fn bounds_of(pts: &[(Pos2, f32)]) -> Rect {
    let mut min = Pos2::new(f32::INFINITY, f32::INFINITY);
    let mut max = Pos2::new(f32::NEG_INFINITY, f32::NEG_INFINITY);
    for (p, half) in pts {
        min.x = min.x.min(p.x - half);
        min.y = min.y.min(p.y - half);
        max.x = max.x.max(p.x + half);
        max.y = max.y.max(p.y + half);
    }
    Rect::from_min_max(min, max)
}

fn median(vals: &mut [f32]) -> f32 {
    if vals.is_empty() {
        return 0.0;
    }
    vals.sort_by(f32::total_cmp);
    let mid = vals.len() / 2;
    if vals.len() % 2 == 1 {
        vals[mid]
    } else {
        (vals[mid - 1] + vals[mid]) * 0.5
    }
}

fn percentile(vals: &mut [f32], p: f32) -> f32 {
    if vals.is_empty() {
        return 0.0;
    }
    vals.sort_by(f32::total_cmp);
    let clamped = p.clamp(0.0, 1.0);
    let idx = ((clamped * (vals.len() as f32 - 1.0)).round() as usize).min(vals.len() - 1);
    vals[idx]
}

fn offscreen_count(
    g: &RsGraph,
    visible: &HashSet<NodeIndex>,
    board: Vec2,
    cam: Camera,
) -> Result<usize, Error> {
    let mut off = 0usize;
    for &idx in visible {
        let Some(n) = g.node(idx) else {
            continue;
        };
        let loc = n.location();
        let sx = loc.x * cam.zoom + cam.x;
        let sy = loc.y * cam.zoom + cam.y;
        if sx < 0.0 || sx > board.x || sy < 0.0 || sy > board.y {
            off += 1;
        }
    }
    Ok(off)
}

pub fn center_camera(g: &RsGraph, board: Vec2) -> Result<Option<(Camera, usize, usize)>, Error> {
    if board.x < 1.0 || board.y < 1.0 {
        return Ok(None);
    }
    let visible = visible_set(g);
    if visible.is_empty() {
        return Ok(None);
    }
    let comps = graphalg::components(g, |n| visible.contains(&n))?;
    let Some(pick) = graphalg::largest(&comps)? else {
        return Ok(None);
    };
    let members = &comps[pick];
    let pts = member_points(g, members)?;
    if pts.is_empty() {
        return Ok(None);
    }
    let bbox = cluster_bounds(&pts)?;
    let mut radii: Vec<f32> = pts.iter().map(|(_, r)| *r).collect();
    let medr = median(&mut radii);
    let cam = fit_box_to_board(bbox, board, CENTER_MARGIN, CENTER_MAX_ZOOM, medr)?;
    let off = offscreen_count(g, &visible, board, cam)?;
    Ok(Some((cam, members.len(), off)))
}

#[cfg(test)]
mod tests {
    use egui::{Pos2, Vec2};
    use egui_graphs::Graph;
    use petgraph::graph::NodeIndex;
    use petgraph::stable_graph::StableGraph;

    use super::super::app::RsGraph;
    use super::super::board::{MAX_ZOOM, MIN_ZOOM};
    use super::{CENTER_MAX_ZOOM, bounds_of, center_camera, center_on_node, cluster_bounds};
    use crate::model::{Author, Camera, Origin, Relation, RelationKind, Verification};
    use crate::shapes::base_radius;
    use crate::ui::testkit::nodes::person;

    fn node_radius(g: &RsGraph, idx: NodeIndex) -> f32 {
        let n = g.node(idx).expect("node exists");
        let e = n.payload();
        base_radius(e).expect("radius") * e.size * e.boost
    }

    fn spawn_at(g: &mut RsGraph, name: &str, x: f32, y: f32) -> NodeIndex {
        g.add_node_with_label_and_location(person(name), name.into(), Pos2::new(x, y))
    }

    fn link(g: &mut RsGraph, a: NodeIndex, b: NodeIndex) {
        g.add_edge_with_label(
            a,
            b,
            Relation::stamp(
                RelationKind::Controls,
                Origin::Manual,
                Verification::Unreviewed,
                Author::default(),
            )
            .unwrap(),
            "controls".into(),
        );
    }

    fn set_hidden(g: &mut RsGraph, idx: NodeIndex, on: bool) {
        if let Some(n) = g.node_mut(idx) {
            n.payload_mut().hidden = on;
        }
    }

    fn framed_center(cam: Camera, board: Vec2) -> Pos2 {
        Pos2::new(
            (board.x / 2.0 - cam.x) / cam.zoom,
            (board.y / 2.0 - cam.y) / cam.zoom,
        )
    }

    #[test]
    fn frames_biggest_visible_cluster_excluding_hidden_and_smaller() {
        let mut g: RsGraph = Graph::from(&StableGraph::default());

        let a0 = spawn_at(&mut g, "a0", -1050.0, 0.0);
        let a1 = spawn_at(&mut g, "a1", -950.0, 0.0);
        link(&mut g, a0, a1);

        let b0 = spawn_at(&mut g, "b0", 500.0, 0.0);
        let b1 = spawn_at(&mut g, "b1", 1500.0, 0.0);
        let b2 = spawn_at(&mut g, "b2", 1000.0, -400.0);
        let b3 = spawn_at(&mut g, "b3", 1000.0, 400.0);
        let b4 = spawn_at(&mut g, "b4", 1000.0, 0.0);
        link(&mut g, b0, b4);
        link(&mut g, b1, b4);
        link(&mut g, b2, b4);
        link(&mut g, b3, b4);

        let mut hids = Vec::new();
        for i in 0..8 {
            let idx = spawn_at(&mut g, &format!("h{i}"), 5000.0 + i as f32 * 10.0, 5000.0);
            set_hidden(&mut g, idx, true);
            hids.push(idx);
        }
        for w in hids.windows(2) {
            link(&mut g, w[0], w[1]);
        }

        let board = Vec2::new(800.0, 600.0);
        let Ok(Some((cam, n, _off))) = center_camera(&g, board) else {
            panic!("expected a camera for a board with visible nodes");
        };
        assert_eq!(n, 5);

        let c = framed_center(cam, board);
        assert!((c.x - 1000.0).abs() < 1.0, "framed x={} expected ~1000", c.x);
        assert!(c.y.abs() < 1.0, "framed y={} expected ~0", c.y);
        assert!((c.x + 1000.0).abs() > 100.0, "must not frame small cluster");
        assert!((c.x - 5000.0).abs() > 100.0, "must not frame hidden cluster");

        assert!(cam.zoom > MIN_ZOOM && cam.zoom <= MAX_ZOOM);
        assert!(cam.zoom < CENTER_MAX_ZOOM, "wide cluster should fit below the cap");
        assert!((cam.zoom - 0.604).abs() < 0.01, "fit zoom={}", cam.zoom);
    }

    #[test]
    fn single_visible_node_centers_at_capped_zoom() {
        let mut g: RsGraph = Graph::from(&StableGraph::default());
        let _solo = spawn_at(&mut g, "solo", 300.0, 200.0);
        let board = Vec2::new(800.0, 600.0);

        let Ok(Some((cam, n, _off))) = center_camera(&g, board) else {
            panic!("expected a camera for a single visible node");
        };
        assert_eq!(n, 1);
        assert!(cam.zoom.is_finite() && cam.x.is_finite() && cam.y.is_finite());
        assert!((cam.zoom - CENTER_MAX_ZOOM).abs() < 1e-4, "zoom={}", cam.zoom);

        let c = framed_center(cam, board);
        assert!((c.x - 300.0).abs() < 1.0 && (c.y - 200.0).abs() < 1.0);
    }

    #[test]
    fn empty_all_hidden_and_zero_board_yield_no_camera() {
        let empty: RsGraph = Graph::from(&StableGraph::default());
        assert!(matches!(center_camera(&empty, Vec2::new(800.0, 600.0)), Ok(None)));

        let mut hidden: RsGraph = Graph::from(&StableGraph::default());
        let x = spawn_at(&mut hidden, "x", 0.0, 0.0);
        let y = spawn_at(&mut hidden, "y", 50.0, 0.0);
        link(&mut hidden, x, y);
        set_hidden(&mut hidden, x, true);
        set_hidden(&mut hidden, y, true);
        assert!(matches!(center_camera(&hidden, Vec2::new(800.0, 600.0)), Ok(None)));

        let mut zero: RsGraph = Graph::from(&StableGraph::default());
        let _z = spawn_at(&mut zero, "z", 0.0, 0.0);
        assert!(matches!(center_camera(&zero, Vec2::ZERO), Ok(None)));
    }

    #[test]
    fn center_on_node_frames_the_node_at_the_given_zoom() {
        let board = Vec2::new(800.0, 600.0);
        let loc = Pos2::new(320.0, -140.0);
        for z in [0.5f32, 1.0, 2.5] {
            let cam = center_on_node(loc, board, z).expect("camera");
            assert!((cam.zoom - z).abs() < 1e-4, "zoom kept {z} got {}", cam.zoom);
            let c = framed_center(cam, board);
            assert!(
                (c.x - loc.x).abs() < 1e-3 && (c.y - loc.y).abs() < 1e-3,
                "node must land at board center: framed {c:?} vs {loc:?}"
            );
        }
    }

    #[test]
    fn center_on_node_respects_the_zoom_clamp() {
        let board = Vec2::new(800.0, 600.0);
        let hi = center_on_node(Pos2::ZERO, board, 9_000.0).expect("camera");
        assert!((hi.zoom - MAX_ZOOM).abs() < 1e-4, "over-zoom clamps to MAX");
        let lo = center_on_node(Pos2::ZERO, board, 0.0).expect("camera");
        assert!((lo.zoom - MIN_ZOOM).abs() < 1e-4, "zero zoom clamps to MIN");
    }

    #[test]
    fn equal_clusters_break_ties_by_lowest_min_index() {
        let mut g: RsGraph = Graph::from(&StableGraph::default());
        let c0 = spawn_at(&mut g, "c0", -20.0, 0.0);
        let c1 = spawn_at(&mut g, "c1", 20.0, 0.0);
        link(&mut g, c0, c1);
        let d0 = spawn_at(&mut g, "d0", 1980.0, 0.0);
        let d1 = spawn_at(&mut g, "d1", 2020.0, 0.0);
        link(&mut g, d0, d1);

        let board = Vec2::new(800.0, 600.0);
        let Ok(Some((cam, n, _off))) = center_camera(&g, board) else {
            panic!("expected a camera");
        };
        assert_eq!(n, 2);
        let c = framed_center(cam, board);
        assert!(c.x.abs() < 50.0, "tie must frame lowest-index cluster, x={}", c.x);
        assert!((c.x - 2000.0).abs() > 100.0);
    }

    fn pt(x: f32, y: f32) -> (Pos2, f32) {
        (Pos2::new(x, y), 5.0)
    }

    #[test]
    fn empty_cluster_bounds_is_error() {
        assert!(cluster_bounds(&[]).is_err(), "no nodes must be an error, not a bogus rect");
    }

    #[test]
    fn single_node_bounds_that_node() {
        let r = cluster_bounds(&[pt(100.0, 40.0)]).unwrap();
        assert!(r.contains(Pos2::new(100.0, 40.0)));
        assert!(r.width() >= 10.0 && r.height() >= 10.0, "half-extent must inflate the box");
    }

    #[test]
    fn far_outlier_dropped_from_bounds() {
        let mut pts: Vec<(Pos2, f32)> = Vec::new();
        for i in 0..19 {
            let a = i as f32 * 0.33;
            pts.push(pt(200.0 + a.cos() * 30.0, 200.0 + a.sin() * 30.0));
        }
        pts.push(pt(5000.0, 5000.0));
        let clamped = cluster_bounds(&pts).unwrap();
        let full = bounds_of(&pts);
        assert!(
            clamped.max.x < 400.0 && clamped.max.y < 400.0,
            "the far outlier must be excluded from the fit box, got {clamped:?}"
        );
        assert!(
            full.max.x > 4000.0,
            "sanity: the full box would include the outlier, got {full:?}"
        );
    }

    #[test]
    fn tight_cluster_keeps_everyone() {
        let mut pts: Vec<(Pos2, f32)> = Vec::new();
        for i in 0..12 {
            let a = i as f32 / 12.0 * std::f32::consts::TAU;
            pts.push(pt(a.cos() * 50.0, a.sin() * 50.0));
        }
        let r = cluster_bounds(&pts).unwrap();
        assert!(
            r.width() > 90.0 && r.height() > 90.0,
            "a spread with no outliers must keep its full extent, got {r:?}"
        );
    }

    #[test]
    fn tight_core_survives_many_outliers() {
        let mut pts: Vec<(Pos2, f32)> = Vec::new();
        for ring in 1..=4 {
            let radius = ring as f32 * 25.0;
            for k in 0..10 {
                let a = k as f32 / 10.0 * std::f32::consts::TAU;
                pts.push(pt(300.0 + a.cos() * radius, 300.0 + a.sin() * radius));
            }
        }
        for (ox, oy) in [
            (3300.0, 300.0),
            (-3200.0, 300.0),
            (300.0, 4300.0),
            (300.0, -4500.0),
            (6000.0, 6000.0),
            (-5000.0, -5000.0),
            (300.0, 8000.0),
            (9000.0, 300.0),
        ] {
            pts.push(pt(ox, oy));
        }
        let r = cluster_bounds(&pts).unwrap();
        assert!(
            r.width() < 600.0 && r.height() < 600.0,
            "the box must hug the tight core, not stretch to the flung outliers, got {r:?}"
        );
        assert!(r.contains(Pos2::new(300.0, 300.0)), "the core center stays framed");
        assert!(
            r.max.x < 1000.0 && r.max.y < 1000.0 && r.min.x > -1000.0 && r.min.y > -1000.0,
            "every far outlier must fall outside the fit box, got {r:?}"
        );
    }

    #[test]
    fn small_cluster_frames_fully_with_nothing_offscreen() {
        let mut g: RsGraph = Graph::from(&StableGraph::default());
        let a = spawn_at(&mut g, "a", -40.0, 0.0);
        let b = spawn_at(&mut g, "b", 40.0, 0.0);
        let c = spawn_at(&mut g, "c", 0.0, 40.0);
        link(&mut g, a, b);
        link(&mut g, b, c);

        let board = Vec2::new(800.0, 600.0);
        let Ok(Some((cam, n, off))) = center_camera(&g, board) else {
            panic!("expected a camera");
        };
        assert_eq!(n, 3);
        assert_eq!(off, 0, "a small cluster must fit fully — nothing off-screen");
        assert!(cam.zoom <= CENTER_MAX_ZOOM + 1e-4, "must not over-zoom past the fit cap");
        let r = node_radius(&g, a);
        assert!(r * cam.zoom >= 5.0, "nodes stay legible, on-screen radius={}", r * cam.zoom);
    }

    #[test]
    fn wide_cluster_floors_zoom_and_reports_offscreen() {
        let mut g: RsGraph = Graph::from(&StableGraph::default());
        let mut prev: Option<NodeIndex> = None;
        let mut first: Option<NodeIndex> = None;
        for i in 0..10 {
            let x = -3000.0 + i as f32 * 667.0;
            let idx = spawn_at(&mut g, &format!("w{i}"), x, 0.0);
            if let Some(p) = prev {
                link(&mut g, p, idx);
            }
            if first.is_none() {
                first = Some(idx);
            }
            prev = Some(idx);
        }

        let board = Vec2::new(800.0, 600.0);
        let natural_fit = board.x * 0.85 / 6022.0;
        let Ok(Some((cam, n, off))) = center_camera(&g, board) else {
            panic!("expected a camera");
        };
        assert_eq!(n, 10);
        let idx = first.expect("a node was spawned");
        let r = node_radius(&g, idx);
        assert!(
            r * cam.zoom >= 5.0,
            "a cluster too wide to fit must floor zoom so nodes stay >=5px, on-screen radius={}",
            r * cam.zoom
        );
        assert!(
            cam.zoom > natural_fit * 1.5,
            "the legibility floor must lift zoom above the raw fit ({natural_fit}), got {}",
            cam.zoom
        );
        assert!(cam.zoom <= CENTER_MAX_ZOOM, "the floor must never exceed the fit cap");
        assert!(off > 0, "a cluster wider than the viewport must report off-screen nodes");
    }
}

use std::collections::HashSet;

use egui::{Pos2, Rect, Vec2};
use petgraph::graph::NodeIndex;

use crate::error::Error;
use crate::model::Camera;
use crate::shapes::{IMG_RADIUS, NODE_RADIUS};

use super::app::RsGraph;
use super::board::{MAX_ZOOM, MIN_ZOOM};

const CENTER_MARGIN: f32 = 0.85;
const CENTER_MAX_ZOOM: f32 = 1.5;

fn clamp_zoom(z: f32) -> f32 {
    if !z.is_finite() {
        return 1.0;
    }
    z.clamp(MIN_ZOOM, MAX_ZOOM)
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

fn visible_components(g: &RsGraph, visible: &HashSet<NodeIndex>) -> Vec<Vec<NodeIndex>> {
    let inner = g.g();
    let mut starts: Vec<NodeIndex> = visible.iter().copied().collect();
    starts.sort();
    let mut seen: HashSet<NodeIndex> = HashSet::new();
    let mut comps: Vec<Vec<NodeIndex>> = Vec::new();
    for start in starts {
        if seen.contains(&start) {
            continue;
        }
        let mut members = Vec::new();
        let mut stack = vec![start];
        loop {
            let Some(idx) = stack.pop() else {
                break;
            };
            if !seen.insert(idx) {
                continue;
            }
            members.push(idx);
            for nb in inner.neighbors_undirected(idx) {
                if visible.contains(&nb) && !seen.contains(&nb) {
                    stack.push(nb);
                }
            }
        }
        comps.push(members);
    }
    comps
}

fn pick_biggest(comps: &[Vec<NodeIndex>]) -> Option<usize> {
    let mut best: Option<usize> = None;
    let mut best_len = 0usize;
    for (i, comp) in comps.iter().enumerate() {
        if comp.len() > best_len {
            best_len = comp.len();
            best = Some(i);
        }
    }
    best
}

fn cluster_bbox(g: &RsGraph, members: &[NodeIndex]) -> Option<Rect> {
    let mut min = Pos2::new(f32::INFINITY, f32::INFINITY);
    let mut max = Pos2::new(f32::NEG_INFINITY, f32::NEG_INFINITY);
    let mut any = false;
    for &idx in members {
        let Some(n) = g.node(idx) else {
            continue;
        };
        let e = n.payload();
        let base = if e.image.is_some() {
            IMG_RADIUS
        } else {
            NODE_RADIUS
        };
        let half = base * e.size * e.boost;
        let loc = n.location();
        min.x = min.x.min(loc.x - half);
        min.y = min.y.min(loc.y - half);
        max.x = max.x.max(loc.x + half);
        max.y = max.y.max(loc.y + half);
        any = true;
    }
    if !any {
        return None;
    }
    Some(Rect::from_min_max(min, max))
}

pub fn center_camera(g: &RsGraph, board: Vec2) -> Result<Option<(Camera, usize)>, Error> {
    if board.x < 1.0 || board.y < 1.0 {
        return Ok(None);
    }
    let visible = visible_set(g);
    if visible.is_empty() {
        return Ok(None);
    }
    let comps = visible_components(g, &visible);
    let Some(pick) = pick_biggest(&comps) else {
        return Ok(None);
    };
    let members = &comps[pick];
    let Some(bbox) = cluster_bbox(g, members) else {
        return Ok(None);
    };
    let span = bbox.size();
    let w = span.x.max(1.0);
    let h = span.y.max(1.0);
    let fit = (board.x * CENTER_MARGIN / w).min(board.y * CENTER_MARGIN / h);
    let zoom = clamp_zoom(fit.min(CENTER_MAX_ZOOM));
    let pan = board / 2.0 - bbox.center().to_vec2() * zoom;
    Ok(Some((
        Camera {
            x: pan.x,
            y: pan.y,
            zoom,
        },
        members.len(),
    )))
}

#[cfg(test)]
mod tests {
    use egui::{Pos2, Vec2};
    use egui_graphs::Graph;
    use petgraph::graph::NodeIndex;
    use petgraph::stable_graph::StableGraph;

    use super::super::app::RsGraph;
    use super::super::board::{MAX_ZOOM, MIN_ZOOM};
    use super::{CENTER_MAX_ZOOM, center_camera};
    use crate::model::{
        Author, Camera, DEFAULT_NODE_SIZE, Entity, EntityKind, Origin, Relation, RelationKind,
        Role, Verification,
    };

    fn node(slug: &str) -> Entity {
        Entity {
            slug: slug.into(),
            kind: EntityKind::Person,
            label: slug.into(),
            value: String::new(),
            sealed: false,
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

    fn spawn_at(g: &mut RsGraph, name: &str, x: f32, y: f32) -> NodeIndex {
        g.add_node_with_label_and_location(node(name), name.into(), Pos2::new(x, y))
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
        let Ok(Some((cam, n))) = center_camera(&g, board) else {
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

        let Ok(Some((cam, n))) = center_camera(&g, board) else {
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
    fn equal_clusters_break_ties_by_lowest_min_index() {
        let mut g: RsGraph = Graph::from(&StableGraph::default());
        let c0 = spawn_at(&mut g, "c0", -20.0, 0.0);
        let c1 = spawn_at(&mut g, "c1", 20.0, 0.0);
        link(&mut g, c0, c1);
        let d0 = spawn_at(&mut g, "d0", 1980.0, 0.0);
        let d1 = spawn_at(&mut g, "d1", 2020.0, 0.0);
        link(&mut g, d0, d1);

        let board = Vec2::new(800.0, 600.0);
        let Ok(Some((cam, n))) = center_camera(&g, board) else {
            panic!("expected a camera");
        };
        assert_eq!(n, 2);
        let c = framed_center(cam, board);
        assert!(c.x.abs() < 50.0, "tie must frame lowest-index cluster, x={}", c.x);
        assert!((c.x - 2000.0).abs() > 100.0);
    }
}

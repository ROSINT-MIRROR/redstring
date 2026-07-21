use std::collections::{HashMap, HashSet, VecDeque};

use petgraph::Direction;
use petgraph::graph::{EdgeIndex, NodeIndex};
use petgraph::visit::EdgeRef;

use crate::error::Error;

use super::app::RsGraph;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PathHop {
    pub from: NodeIndex,
    pub to: NodeIndex,
    pub edge: EdgeIndex,
}

pub fn path_between(
    g: &RsGraph,
    from: NodeIndex,
    to: NodeIndex,
) -> Result<Option<Vec<PathHop>>, Error> {
    let inner = g.g();
    if from == to {
        return Ok(Some(Vec::new()));
    }
    let mut pred: HashMap<NodeIndex, (NodeIndex, EdgeIndex)> = HashMap::new();
    let mut seen: HashSet<NodeIndex> = HashSet::new();
    let mut queue: VecDeque<NodeIndex> = VecDeque::new();
    seen.insert(from);
    queue.push_back(from);
    let mut reached = false;
    loop {
        let Some(cur) = queue.pop_front() else {
            break;
        };
        for e in inner
            .edges_directed(cur, Direction::Outgoing)
            .chain(inner.edges_directed(cur, Direction::Incoming))
        {
            let other = if e.source() == cur { e.target() } else { e.source() };
            if other == cur || !seen.insert(other) {
                continue;
            }
            pred.insert(other, (cur, e.id()));
            queue.push_back(other);
            if other == to {
                reached = true;
                break;
            }
        }
        if reached {
            break;
        }
    }
    if !reached {
        return Ok(None);
    }
    let mut hops: Vec<PathHop> = Vec::new();
    let mut cur = to;
    while cur != from {
        let Some(&(prev, edge)) = pred.get(&cur) else {
            return Err(Error::PathTrace { node: cur.index() });
        };
        hops.push(PathHop {
            from: prev,
            to: cur,
            edge,
        });
        cur = prev;
    }
    hops.reverse();
    Ok(Some(hops))
}

pub fn components<F>(g: &RsGraph, allowed: F) -> Result<Vec<Vec<NodeIndex>>, Error>
where
    F: Fn(NodeIndex) -> bool,
{
    let inner = g.g();
    let mut seen: HashSet<NodeIndex> = HashSet::new();
    let mut comps: Vec<Vec<NodeIndex>> = Vec::new();
    for start in inner.node_indices() {
        if !allowed(start) || seen.contains(&start) {
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
                if allowed(nb) && !seen.contains(&nb) {
                    stack.push(nb);
                }
            }
        }
        comps.push(members);
    }
    Ok(comps)
}

pub fn largest(comps: &[Vec<NodeIndex>]) -> Result<Option<usize>, Error> {
    let mut best: Option<usize> = None;
    let mut best_len = 0usize;
    for (i, comp) in comps.iter().enumerate() {
        if comp.len() > best_len {
            best_len = comp.len();
            best = Some(i);
        }
    }
    Ok(best)
}

#[cfg(test)]
mod tests {
    use egui::Pos2;
    use egui_graphs::Graph;
    use petgraph::graph::NodeIndex;
    use petgraph::stable_graph::StableGraph;

    use super::super::app::RsGraph;
    use super::{components, largest, path_between};
    use crate::model::{
        Author, Origin, Relation, RelationKind, Source, Verification,
    };
    use crate::ui::testkit::nodes::person;

    fn spawn(g: &mut RsGraph, name: &str) -> NodeIndex {
        g.add_node_with_label_and_location(person(name), name.into(), Pos2::ZERO)
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

    fn link_sourced(g: &mut RsGraph, a: NodeIndex, b: NodeIndex) {
        let mut r = Relation::stamp(
            RelationKind::Controls,
            Origin::Manual,
            Verification::Unreviewed,
            Author::default(),
        )
        .unwrap();
        r.sources.push(Source {
            reference: "notes/proof.md".to_owned(),
            ..Default::default()
        });
        g.add_edge_with_label(a, b, r, "controls".into());
    }

    #[test]
    fn disjoint_clusters_are_separate_components() {
        let mut g: RsGraph = Graph::from(&StableGraph::default());
        let a = spawn(&mut g, "a");
        let b = spawn(&mut g, "b");
        let c = spawn(&mut g, "c");
        let d = spawn(&mut g, "d");
        let e = spawn(&mut g, "e");
        link(&mut g, a, b);
        link(&mut g, b, c);
        link(&mut g, d, e);
        let comps = components(&g, |_| true).unwrap();
        assert_eq!(comps.len(), 2);
        let mut sizes: Vec<usize> = comps.iter().map(|comp| comp.len()).collect();
        sizes.sort();
        assert_eq!(sizes, vec![2, 3]);
    }

    #[test]
    fn single_node_is_one_component() {
        let mut g: RsGraph = Graph::from(&StableGraph::default());
        let _solo = spawn(&mut g, "solo");
        let comps = components(&g, |_| true).unwrap();
        assert_eq!(comps.len(), 1);
        assert_eq!(comps[0].len(), 1);
        assert_eq!(largest(&comps).unwrap(), Some(0));
    }

    #[test]
    fn empty_graph_has_no_components_and_no_largest() {
        let g: RsGraph = Graph::from(&StableGraph::default());
        let comps = components(&g, |_| true).unwrap();
        assert!(comps.is_empty());
        assert_eq!(largest(&comps).unwrap(), None);
    }

    #[test]
    fn largest_picks_the_biggest_cluster() {
        let mut g: RsGraph = Graph::from(&StableGraph::default());
        let s0 = spawn(&mut g, "s0");
        let s1 = spawn(&mut g, "s1");
        link(&mut g, s0, s1);
        let b0 = spawn(&mut g, "b0");
        let b1 = spawn(&mut g, "b1");
        let b2 = spawn(&mut g, "b2");
        link(&mut g, b0, b1);
        link(&mut g, b1, b2);
        let comps = components(&g, |_| true).unwrap();
        let pick = largest(&comps).unwrap().unwrap();
        assert_eq!(comps[pick].len(), 3);
        assert!(comps[pick].contains(&b0));
    }

    #[test]
    fn equal_clusters_break_ties_by_lowest_min_index() {
        let mut g: RsGraph = Graph::from(&StableGraph::default());
        let a = spawn(&mut g, "a");
        let b = spawn(&mut g, "b");
        let c = spawn(&mut g, "c");
        let d = spawn(&mut g, "d");
        link(&mut g, a, b);
        link(&mut g, c, d);
        let comps = components(&g, |_| true).unwrap();
        assert_eq!(comps.len(), 2);
        assert_eq!(largest(&comps).unwrap(), Some(0));
        assert!(comps[0].contains(&a));
    }

    #[test]
    fn allowed_predicate_removes_nodes_and_splits_paths() {
        let mut g: RsGraph = Graph::from(&StableGraph::default());
        let a = spawn(&mut g, "a");
        let b = spawn(&mut g, "b");
        let c = spawn(&mut g, "c");
        link(&mut g, a, b);
        link(&mut g, b, c);
        let comps = components(&g, |n| n != b).unwrap();
        assert_eq!(comps.len(), 2);
        assert!(comps.iter().all(|comp| comp.len() == 1));
        assert!(!comps.iter().any(|comp| comp.contains(&b)));
    }

    #[test]
    fn path_between_walks_a_chain_end_to_end() {
        let mut g: RsGraph = Graph::from(&StableGraph::default());
        let a = spawn(&mut g, "a");
        let b = spawn(&mut g, "b");
        let c = spawn(&mut g, "c");
        let d = spawn(&mut g, "d");
        link(&mut g, a, b);
        link(&mut g, b, c);
        link(&mut g, c, d);
        let hops = path_between(&g, a, d).unwrap().expect("a reaches d");
        let seq: Vec<(NodeIndex, NodeIndex)> = hops.iter().map(|h| (h.from, h.to)).collect();
        assert_eq!(seq, vec![(a, b), (b, c), (c, d)]);
    }

    #[test]
    fn path_between_ignores_edge_direction() {
        let mut g: RsGraph = Graph::from(&StableGraph::default());
        let a = spawn(&mut g, "a");
        let b = spawn(&mut g, "b");
        let c = spawn(&mut g, "c");
        link(&mut g, a, b);
        link(&mut g, b, c);
        let hops = path_between(&g, c, a).unwrap().expect("undirected reach");
        let seq: Vec<(NodeIndex, NodeIndex)> = hops.iter().map(|h| (h.from, h.to)).collect();
        assert_eq!(seq, vec![(c, b), (b, a)]);
    }

    #[test]
    fn path_between_takes_the_shortest_route_through_a_diamond() {
        let mut g: RsGraph = Graph::from(&StableGraph::default());
        let a = spawn(&mut g, "a");
        let b = spawn(&mut g, "b");
        let c = spawn(&mut g, "c");
        let d = spawn(&mut g, "d");
        link(&mut g, a, b);
        link(&mut g, a, c);
        link(&mut g, b, d);
        link(&mut g, c, d);
        let hops = path_between(&g, a, d).unwrap().expect("a reaches d");
        assert_eq!(hops.len(), 2, "diamond shortest path is two hops");
        assert_eq!(hops[0].from, a);
        assert_eq!(hops[1].to, d);
        assert_eq!(hops[0].to, hops[1].from, "hops chain contiguously");
    }

    #[test]
    fn path_between_returns_none_when_unreachable() {
        let mut g: RsGraph = Graph::from(&StableGraph::default());
        let a = spawn(&mut g, "a");
        let b = spawn(&mut g, "b");
        let c = spawn(&mut g, "c");
        let d = spawn(&mut g, "d");
        link(&mut g, a, b);
        link(&mut g, c, d);
        assert!(path_between(&g, a, d).unwrap().is_none());
    }

    #[test]
    fn path_between_to_self_is_trivial() {
        let mut g: RsGraph = Graph::from(&StableGraph::default());
        let a = spawn(&mut g, "a");
        let hops = path_between(&g, a, a).unwrap().expect("self is reachable");
        assert!(hops.is_empty(), "no hops needed to reach yourself");
    }

    #[test]
    fn path_trail_flags_the_unsourced_hop() {
        let mut g: RsGraph = Graph::from(&StableGraph::default());
        let a = spawn(&mut g, "a");
        let b = spawn(&mut g, "b");
        let c = spawn(&mut g, "c");
        link(&mut g, a, b);
        link_sourced(&mut g, b, c);
        let hops = path_between(&g, a, c).unwrap().expect("a reaches c");
        assert_eq!(hops.len(), 2);
        let unsourced: Vec<bool> = hops
            .iter()
            .map(|h| g.edge(h.edge).expect("edge exists").payload().sources.is_empty())
            .collect();
        assert_eq!(unsourced, vec![true, false], "first hop unsourced, second sourced");
        assert_eq!(
            unsourced.iter().filter(|weak| **weak).count(),
            1,
            "exactly one weak link in the chain"
        );
    }
}

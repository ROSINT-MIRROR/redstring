use std::collections::HashSet;

use petgraph::graph::NodeIndex;

use crate::error::Error;

use super::app::RsGraph;

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
    use super::{components, largest};
    use crate::model::{
        Author, Origin, Relation, RelationKind, Verification,
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
}

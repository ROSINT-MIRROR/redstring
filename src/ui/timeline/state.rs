use petgraph::graph::{EdgeIndex, NodeIndex};

use crate::error::Error;

use super::super::app::RsGraph;
use super::event::{EventKey, Lane, Record};

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct View {
    pub t0: f64,
    pub spp: f64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fit {
    All,
    Focus,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Focus {
    pub nodes: Vec<NodeIndex>,
    pub edges: Vec<EdgeIndex>,
}

#[derive(Clone, PartialEq, Debug)]
pub struct TimelineState {
    pub open: bool,
    pub view: View,
    pub pending_fit: Option<Fit>,
    pub selected: Option<EventKey>,
    pub lanes: [bool; 4],
    pub follow: bool,
    pub seen: Focus,
}

impl TimelineState {
    pub const MIN_SPP: f64 = 20.0;
    pub const MAX_SPP: f64 = 2.0e8;

    pub fn closed() -> Result<TimelineState, Error> {
        Ok(TimelineState {
            open: false,
            view: View {
                t0: 946_684_800.0,
                spp: 1.0e6,
            },
            pending_fit: Some(Fit::All),
            selected: None,
            lanes: [true; 4],
            follow: true,
            seen: Focus {
                nodes: Vec::new(),
                edges: Vec::new(),
            },
        })
    }

    pub fn shows(&self, lane: Lane) -> Result<bool, Error> {
        Ok(self.lanes[lane.slot()?])
    }
}

impl Focus {
    pub fn of(g: &RsGraph) -> Result<Focus, Error> {
        let mut nodes = g.selected_nodes().to_vec();
        let mut edges = g.selected_edges().to_vec();
        nodes.sort();
        edges.sort();
        Ok(Focus { nodes, edges })
    }

    pub fn is_empty(&self) -> Result<bool, Error> {
        Ok(self.nodes.is_empty() && self.edges.is_empty())
    }

    pub fn holds(&self, record: Record) -> Result<bool, Error> {
        Ok(match record {
            Record::Entity(n) | Record::Obs(n, ..) => self.nodes.contains(&n),
            Record::Rel { edge, from, to } => {
                self.edges.contains(&edge) || self.nodes.contains(&from) || self.nodes.contains(&to)
            }
        })
    }
}

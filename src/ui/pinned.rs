use egui_graphs::{DisplayEdge, DisplayNode, Graph, Layout, LayoutState};
use petgraph::EdgeType;
use petgraph::stable_graph::IndexType;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PinnedState;

impl LayoutState for PinnedState {}

#[derive(Default)]
pub struct PinnedLayout {
    state: PinnedState,
}

impl Layout<PinnedState> for PinnedLayout {
    fn from_state(state: PinnedState) -> impl Layout<PinnedState> {
        Self { state }
    }

    fn next<N, E, Ty, Ix, Dn, De>(&mut self, _g: &mut Graph<N, E, Ty, Ix, Dn, De>, _ui: &egui::Ui)
    where
        N: Clone,
        E: Clone,
        Ty: EdgeType,
        Ix: IndexType,
        Dn: DisplayNode<N, E, Ty, Ix>,
        De: DisplayEdge<N, E, Ty, Ix, Dn>,
    {
    }

    fn state(&self) -> PinnedState {
        self.state.clone()
    }
}

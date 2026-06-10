//! Graph-side operations: building entities, slug discipline, the per-frame
//! payload-to-display sync, and the demo seed.

use egui::Pos2;
use petgraph::graph::{EdgeIndex, NodeIndex};

use crate::model::{
    DEFAULT_NODE_SIZE, Entity, EntityKind, Relation, RelationKind, Source, SourceKind,
};

use super::app::RedstringApp;

impl RedstringApp {
    /// A tiny starter case so the board demonstrates the sourced/unsourced
    /// split on first launch.
    pub(super) fn seed_demo(&mut self) {
        let doe = self.add_entity(EntityKind::Person, "J. Doe".into(), spiral(0));
        let shell = self.add_entity(EntityKind::ShellCompany, "Shell Co Ltd".into(), spiral(1));
        let domain = self.add_entity(EntityKind::Domain, "evil.example".into(), spiral(2));
        let minister = self.add_entity(EntityKind::Official, "Minister X".into(), spiral(3));

        // sourced link draws solid green
        let mut controls = Relation::new(RelationKind::Controls);
        controls.sources.push(Source {
            kind: SourceKind::Markdown,
            reference: "dossier.md#ownership".into(),
            excerpt: "Doe is sole signatory per incorporation docs.".into(),
        });
        self.g
            .add_edge_with_label(doe, shell, controls, RelationKind::Controls.label().into());

        // unsourced links draw red + dashed — the things you still must prove
        self.g.add_edge_with_label(
            shell,
            domain,
            Relation::new(RelationKind::OwnsDomain),
            RelationKind::OwnsDomain.label().into(),
        );
        self.g.add_edge_with_label(
            shell,
            minister,
            Relation::new(RelationKind::Financed),
            RelationKind::Financed.label().into(),
        );

        self.status = "demo case — edit freely or Load your own".into();
    }

    /// Slug an entity label: lowercase alnum, runs of anything else collapse
    /// to `_`. Deduped against the current graph with numeric suffixes.
    pub(super) fn unique_slug(&self, label: &str) -> String {
        let mut base: String = label
            .to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect::<String>()
            .split('_')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("_");
        if base.is_empty() {
            base = "entity".to_owned();
        }
        if !self.slug_taken(&base) {
            return base;
        }
        let mut n = 2;
        loop {
            let candidate = format!("{base}_{n}");
            if !self.slug_taken(&candidate) {
                return candidate;
            }
            n += 1;
        }
    }

    fn slug_taken(&self, slug: &str) -> bool {
        self.g.g().node_weights().any(|n| n.payload().slug == slug)
    }

    /// True when any node other than `except` already holds `slug`.
    pub(super) fn slug_taken_by_other(&self, slug: &str, except: NodeIndex) -> bool {
        self.g
            .g()
            .node_indices()
            .any(|i| i != except && self.node_slug_is(i, slug))
    }

    fn node_slug_is(&self, idx: NodeIndex, slug: &str) -> bool {
        let Some(n) = self.g.g().node_weight(idx) else {
            return false;
        };
        n.payload().slug == slug
    }

    pub(super) fn node_slug(&self, idx: NodeIndex) -> String {
        let Some(n) = self.g.g().node_weight(idx) else {
            return String::new();
        };
        n.payload().slug.clone()
    }

    pub(super) fn add_entity(&mut self, kind: EntityKind, label: String, pos: Pos2) -> NodeIndex {
        let ent = Entity {
            slug: self.unique_slug(&label),
            kind,
            label: label.clone(),
            // pre-fill the kind's predictable attribute keys with empty
            // values so a fresh entity shows its expected fields immediately.
            // Loaded files are never prefilled — the file is the truth.
            attrs: kind
                .default_attrs()
                .iter()
                .map(|k| ((*k).to_owned(), String::new()))
                .collect(),
            note: String::new(),
            image: None,
            size: DEFAULT_NODE_SIZE,
        };
        self.g
            .add_node_with_label_and_location(ent, format!("{} {}", kind.icon(), label), pos)
    }

    /// Commit a label — and a freshly derived slug — onto an existing node.
    pub(super) fn apply_label(&mut self, idx: NodeIndex, label: &str) {
        let slug = self.unique_slug(label);
        let Some(n) = self.g.node_mut(idx) else { return };
        let e = n.payload_mut();
        e.label = label.to_owned();
        e.slug = slug;
    }

    /// Scale a node, when it still exists.
    pub(super) fn set_node_size(&mut self, idx: NodeIndex, size: f32) {
        let Some(n) = self.g.node_mut(idx) else { return };
        n.payload_mut().size = size;
    }

    /// Push payload state — colour and label — onto display nodes/edges each frame.
    pub(super) fn sync_visuals(&mut self) {
        let nodes: Vec<NodeIndex> = self.g.g().node_indices().collect();
        for idx in nodes {
            let Some(n) = self.g.node(idx) else { continue };
            let e = n.payload();
            let color = e.kind.color();
            let label = format!("{} {}", e.kind.icon(), e.label);
            let Some(n) = self.g.node_mut(idx) else { continue };
            n.set_color(color);
            n.set_label(label);
        }

        let edges: Vec<EdgeIndex> = self.g.g().edge_indices().collect();
        for idx in edges {
            let Some(ed) = self.g.edge(idx) else { continue };
            let r = ed.payload();
            let tag = if r.is_sourced() {
                format!("[{}]", r.sources.len())
            } else {
                "⚠0".to_owned()
            };
            let label = format!("{} {}", r.kind.label(), tag);
            let Some(ed) = self.g.edge_mut(idx) else { continue };
            ed.set_label(label);
        }
    }

    pub(super) fn node_label(&self, idx: NodeIndex) -> String {
        let Some(n) = self.g.g().node_weight(idx) else {
            return "?".to_owned();
        };
        let e = n.payload();
        format!("{} {}", e.kind.icon(), e.label)
    }

    pub(super) fn edge_endpoints_labels(&self, idx: EdgeIndex) -> (String, String) {
        let Some((a, b)) = self.g.g().edge_endpoints(idx) else {
            return ("?".to_owned(), "?".to_owned());
        };
        (self.node_label(a), self.node_label(b))
    }
}

/// Golden-angle spiral so seeded / freshly-added / healed nodes don't stack.
pub(super) fn spiral(n: usize) -> Pos2 {
    let a = n as f32 * 2.399_963_2;
    let r = 40.0 + 16.0 * n as f32;
    Pos2::new(r * a.cos(), r * a.sin())
}

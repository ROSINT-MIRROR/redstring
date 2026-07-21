use egui::Pos2;
use petgraph::graph::{EdgeIndex, NodeIndex};

use crate::error::Error;
use crate::model::{
    DEFAULT_NODE_SIZE, Entity, EntityKind, KIND_META, Origin, Preset, RELATION_META, Relation,
    RelationKind, Role, Source, SourceKind, Verification, apply_preset,
};

use super::app::RedstringApp;

impl RedstringApp {
    pub fn seed_demo(&mut self) -> Result<(), Error> {
        let doe = self.add_entity(EntityKind::Person, "J. Doe".into(), spiral(0)?)?;
        let shell = self.add_entity(EntityKind::Organization, "Shell Co Ltd".into(), spiral(1)?)?;
        let domain = self.add_entity(EntityKind::Domain, "evil.example".into(), spiral(2)?)?;
        let minister = self.add_entity(EntityKind::Person, "Minister X".into(), spiral(3)?)?;

        let mut controls = Relation::stamp(
            RelationKind::Controls,
            Origin::Manual,
            Verification::AnalystAsserted,
            self.author.clone(),
        )?;
        controls.sources.push(Source {
            kind: SourceKind::Markdown,
            reference: "dossier.md#ownership".into(),
            excerpt: "Doe is sole signatory per incorporation docs.".into(),
        });
        self.g.add_edge_with_label(
            doe,
            shell,
            controls,
            RELATION_META[RelationKind::Controls as usize].label.into(),
        );

        self.g.add_edge_with_label(
            shell,
            domain,
            Relation::stamp(
                RelationKind::OwnsDomain,
                Origin::Manual,
                Verification::Unreviewed,
                self.author.clone(),
            )?,
            RELATION_META[RelationKind::OwnsDomain as usize].label.into(),
        );
        self.g.add_edge_with_label(
            shell,
            minister,
            Relation::stamp(
                RelationKind::Financed,
                Origin::Manual,
                Verification::Unreviewed,
                self.author.clone(),
            )?,
            RELATION_META[RelationKind::Financed as usize].label.into(),
        );

        self.status = "demo case — edit freely or Load your own".into();
        Ok(())
    }

    fn unique_slug(&self, label: &str) -> String {
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

    pub fn node_slug(&self, idx: NodeIndex) -> Result<String, Error> {
        let Some(n) = self.g.g().node_weight(idx) else {
            return Ok(String::new());
        };
        Ok(n.payload().slug.clone())
    }

    pub fn add_entity(
        &mut self,
        kind: EntityKind,
        label: String,
        pos: Pos2,
    ) -> Result<NodeIndex, Error> {
        let meta = &KIND_META[kind as usize];
        let ent = Entity {
            slug: self.unique_slug(&label),
            kind,
            label: label.clone(),
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
        };
        let idx = self
            .g
            .add_node_with_label_and_location(ent, format!("{} {}", meta.icon, label), pos);
        crate::dlog_info!("ops", "add_entity kind={:?} label={label:?} idx={}", kind, idx.index());
        Ok(idx)
    }

    pub fn add_preset(&mut self, preset: &Preset, pos: Pos2) -> Result<NodeIndex, Error> {
        let idx = self.add_entity(preset.base_kind, preset.label.to_owned(), pos)?;
        let Some(n) = self.g.node_mut(idx) else {
            return Ok(idx);
        };
        apply_preset(n.payload_mut(), preset, &self.author)?;
        Ok(idx)
    }

    pub fn apply_label(&mut self, idx: NodeIndex, label: &str) -> Result<(), Error> {
        let slug = self.unique_slug(label);
        let Some(n) = self.g.node_mut(idx) else {
            return Ok(());
        };
        let e = n.payload_mut();
        e.label = label.to_owned();
        e.slug = slug;
        crate::dlog_info!("ops", "rename idx={} slug={} label={label:?}", idx.index(), e.slug);
        Ok(())
    }

    pub fn set_node_size(&mut self, idx: NodeIndex, size: f32) -> Result<(), Error> {
        let Some(n) = self.g.node_mut(idx) else {
            return Ok(());
        };
        n.payload_mut().size = size;
        crate::dlog_debug!("ops", "resize idx={} size={size:.2}", idx.index());
        Ok(())
    }

    pub fn sync_visuals(&mut self) -> Result<(), Error> {
        let nodes: Vec<NodeIndex> = self.g.g().node_indices().collect();
        for idx in nodes {
            let boost = self.degree_boost(idx);
            let Some(n) = self.g.node(idx) else { continue };
            let e = n.payload();
            let meta = &KIND_META[e.kind as usize];
            let color = meta.color;
            let label = format!("{} {}", meta.icon, e.label);
            let Some(n) = self.g.node_mut(idx) else { continue };
            n.set_color(color);
            n.set_label(label);
            n.payload_mut().boost = boost;
        }

        let edges: Vec<EdgeIndex> = self.g.g().edge_indices().collect();
        for idx in edges {
            let Some(ed) = self.g.edge(idx) else { continue };
            let r = ed.payload();
            let tag = if r.sources.is_empty() {
                "⚠0".to_owned()
            } else {
                format!("[{}]", r.sources.len())
            };
            let label = format!("{} {}", RELATION_META[r.kind as usize].label, tag);
            let Some(ed) = self.g.edge_mut(idx) else { continue };
            ed.set_label(label);
        }
        Ok(())
    }



    fn degree_boost(&self, idx: NodeIndex) -> f32 {
        if !self.size_by_degree {
            return 1.0;
        }
        let g = self.g.g();
        let degree = g
            .edges_directed(idx, petgraph::Direction::Incoming)
            .count()
            + g.edges_directed(idx, petgraph::Direction::Outgoing).count();
        (1.0 + 0.18 * degree as f32).sqrt().clamp(1.0, 2.5)
    }

    pub fn node_label(&self, idx: NodeIndex) -> Result<String, Error> {
        let Some(n) = self.g.g().node_weight(idx) else {
            return Ok("?".to_owned());
        };
        let e = n.payload();
        Ok(format!("{} {}", KIND_META[e.kind as usize].icon, e.label))
    }

    pub fn edge_endpoints_labels(&self, idx: EdgeIndex) -> Result<(String, String), Error> {
        let Some((a, b)) = self.g.g().edge_endpoints(idx) else {
            return Ok(("?".to_owned(), "?".to_owned()));
        };
        Ok((self.node_label(a)?, self.node_label(b)?))
    }
}

pub fn spiral(n: usize) -> Result<Pos2, Error> {
    let a = n as f32 * 2.399_963_2;
    let r = 40.0 + 16.0 * n as f32;
    Ok(Pos2::new(r * a.cos(), r * a.sin()))
}

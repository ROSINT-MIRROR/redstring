use egui::{Color32, Pos2};
use petgraph::Direction;
use petgraph::graph::{EdgeIndex, NodeIndex};
use petgraph::visit::EdgeRef;

use crate::error::Error;
use crate::model::{
    Entity, EntityKind, KIND_META, Origin, Preset, RELATION_META, Relation,
    RelationKind, Source, SourceKind, Verification, apply_preset, retain_declared_facets,
};

use super::app::{RedstringApp, RsGraph};

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
            ..Default::default()
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
        let idx = spawn_node(&mut self.g, kind, &label, "", pos)?;
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
        relabel(&mut self.g, idx, label)
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
        apply_visuals(&mut self.g, self.size_by_degree)
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

    pub fn merge_entities(
        &mut self,
        survivor: NodeIndex,
        absorbed: NodeIndex,
    ) -> Result<(), Error> {
        if survivor == absorbed {
            return Ok(());
        }
        let Some(node) = self.g.g().node_weight(absorbed) else {
            return Ok(());
        };
        let abs = node.payload().clone();
        let incident = incident_edges(&self.g, absorbed);
        merge_payload(&mut self.g, survivor, &abs)?;
        repoint_edges(&mut self.g, survivor, absorbed, incident);
        let _removed = self.g.remove_node(absorbed);
        crate::dlog_info!(
            "ops",
            "merge absorbed={} slug={} into survivor={}",
            absorbed.index(),
            abs.slug,
            survivor.index()
        );
        Ok(())
    }
}

fn incident_edges(g: &RsGraph, absorbed: NodeIndex) -> Vec<(NodeIndex, NodeIndex, Relation)> {
    let gg = g.g();
    let mut out = Vec::new();
    for er in gg
        .edges_directed(absorbed, Direction::Outgoing)
        .chain(gg.edges_directed(absorbed, Direction::Incoming))
    {
        out.push((er.source(), er.target(), er.weight().payload().clone()));
    }
    out
}

fn merge_payload(g: &mut RsGraph, survivor: NodeIndex, abs: &Entity) -> Result<(), Error> {
    let Some(node) = g.node_mut(survivor) else {
        return Ok(());
    };
    let s = node.payload_mut();
    for o in &abs.observations {
        if s.observations.iter().any(|x| x.name == o.name && x.value == o.value) {
            continue;
        }
        s.observations.push(o.clone());
    }
    for (facet, val) in &abs.facets {
        s.facets.entry(*facet).or_insert_with(|| val.clone());
    }
    retain_declared_facets(&mut s.facets, s.kind)?;
    if !abs.note.trim().is_empty() {
        if s.note.trim().is_empty() {
            s.note = abs.note.clone();
        } else {
            s.note = format!("{}\n\n---\n\n{}", s.note, abs.note);
        }
    }
    if s.image.is_none() {
        s.image = abs.image.clone();
    }
    Ok(())
}

fn repoint_edges(
    g: &mut RsGraph,
    survivor: NodeIndex,
    absorbed: NodeIndex,
    incident: Vec<(NodeIndex, NodeIndex, Relation)>,
) {
    for (a, b, rel) in incident {
        let na = if a == absorbed { survivor } else { a };
        let nb = if b == absorbed { survivor } else { b };
        if na == nb {
            continue;
        }
        let existing = g
            .g()
            .edges_connecting(na, nb)
            .find(|er| er.weight().payload().kind == rel.kind)
            .map(|er| er.id());
        let Some(eidx) = existing else {
            let label = RELATION_META[rel.kind as usize].label.to_owned();
            g.add_edge_with_label(na, nb, rel, label);
            continue;
        };
        let Some(edge) = g.edge_mut(eidx) else {
            continue;
        };
        union_sources(&mut edge.payload_mut().sources, &rel.sources);
    }
}

fn union_sources(into: &mut Vec<Source>, extra: &[Source]) {
    for s in extra {
        if into.iter().any(|x| x == s) {
            continue;
        }
        into.push(s.clone());
    }
}

pub fn spawn_node(
    g: &mut RsGraph,
    kind: EntityKind,
    label: &str,
    value: &str,
    pos: Pos2,
) -> Result<NodeIndex, Error> {
    let meta = &KIND_META[kind as usize];
    let ent = Entity {
        slug: slug_unique(g, label, None),
        kind,
        label: label.to_owned(),
        value: value.to_owned(),
        ..Default::default()
    };
    let idx = g.add_node_with_label_and_location(ent, format!("{} {}", meta.icon, label), pos);
    Ok(idx)
}

fn relabel(g: &mut RsGraph, idx: NodeIndex, label: &str) -> Result<(), Error> {
    let slug = slug_unique(g, label, Some(idx));
    let Some(n) = g.node_mut(idx) else {
        return Ok(());
    };
    let e = n.payload_mut();
    e.label = label.to_owned();
    e.slug = slug;
    crate::dlog_info!("ops", "rename idx={} slug={} label={label:?}", idx.index(), e.slug);
    Ok(())
}

fn slug_unique(g: &RsGraph, label: &str, exclude: Option<NodeIndex>) -> String {
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
    if !slug_taken(g, &base, exclude) {
        return base;
    }
    let mut n = 2;
    loop {
        let candidate = format!("{base}_{n}");
        if !slug_taken(g, &candidate, exclude) {
            return candidate;
        }
        n += 1;
    }
}

fn slug_taken(g: &RsGraph, slug: &str, exclude: Option<NodeIndex>) -> bool {
    g.g().node_indices().any(|i| {
        if Some(i) == exclude {
            return false;
        }
        let Some(n) = g.g().node_weight(i) else {
            return false;
        };
        n.payload().slug == slug
    })
}

pub fn spiral(n: usize) -> Result<Pos2, Error> {
    let a = n as f32 * 2.399_963_2;
    let r = 40.0 + 16.0 * n as f32;
    Ok(Pos2::new(r * a.cos(), r * a.sin()))
}

pub fn free_spot(taken: &[Pos2], anchor: Pos2) -> Result<Pos2, Error> {
    const GAP: f32 = 64.0;
    const TRIES: usize = 4096;
    for n in 0..TRIES {
        let spot = anchor + spiral(n)?.to_vec2();
        if taken.iter().all(|t| t.distance(spot) >= GAP) {
            return Ok(spot);
        }
    }
    Ok(anchor + spiral(TRIES)?.to_vec2())
}

pub fn apply_visuals(g: &mut RsGraph, size_by_degree: bool) -> Result<(), Error> {
    let nodes: Vec<NodeIndex> = g.g().node_indices().collect();
    for idx in nodes {
        let boost = degree_boost(g, idx, size_by_degree);
        let Some(n) = g.node(idx) else { continue };
        let e = n.payload();
        let meta = &KIND_META[e.kind as usize];
        let color = meta.color;
        let label = format!("{} {}", meta.icon, e.label);
        let Some(n) = g.node_mut(idx) else { continue };
        n.set_color(color);
        n.set_label(label);
        n.payload_mut().boost = boost;
    }

    let edges: Vec<EdgeIndex> = g.g().edge_indices().collect();
    for idx in edges {
        let Some(ed) = g.edge(idx) else { continue };
        let r = ed.payload();
        let tag = if r.sources.is_empty() {
            "⚠0".to_owned()
        } else {
            format!("[{}]", r.sources.len())
        };
        let label = format!("{} {}", RELATION_META[r.kind as usize].label, tag);
        let Some(ed) = g.edge_mut(idx) else { continue };
        ed.set_label(label);
    }
    Ok(())
}

const LIGHT_DARKEN: f32 = 0.55;

pub fn board_color(raw: Color32, dark: bool) -> Result<Color32, Error> {
    if dark {
        return Ok(raw);
    }
    Ok(raw.lerp_to_gamma(Color32::BLACK, LIGHT_DARKEN))
}

fn degree_boost(g: &RsGraph, idx: NodeIndex, size_by_degree: bool) -> f32 {
    if !size_by_degree {
        return 1.0;
    }
    let gg = g.g();
    let degree = gg
        .edges_directed(idx, petgraph::Direction::Incoming)
        .count()
        + gg.edges_directed(idx, petgraph::Direction::Outgoing).count();
    (1.0 + 0.18 * degree as f32).sqrt().clamp(1.0, 2.5)
}

#[cfg(test)]
mod tests {
    use egui::Pos2;
    use egui_graphs::Graph;
    use petgraph::stable_graph::StableGraph;

    use super::super::app::RsGraph;
    use super::{relabel, slug_unique, spawn_node};
    use crate::model::EntityKind;

    #[test]
    fn spawn_slug_tracks_label() {
        let mut g: RsGraph = Graph::from(&StableGraph::default());
        let idx = spawn_node(&mut g, EntityKind::Domain, "evil.com", "evil.com", Pos2::ZERO).unwrap();
        assert_eq!(g.node(idx).unwrap().payload().slug, "evil_com");
    }

    #[test]
    fn inspector_rename_regenerates_slug() {
        let mut g: RsGraph = Graph::from(&StableGraph::default());
        let idx = spawn_node(&mut g, EntityKind::Domain, "Domain", "", Pos2::ZERO).unwrap();
        assert_eq!(g.node(idx).unwrap().payload().slug, "domain");
        relabel(&mut g, idx, "evil.com").unwrap();
        let e = g.node(idx).unwrap().payload();
        assert_eq!(e.label, "evil.com");
        assert_eq!(e.slug, "evil_com");
        assert_eq!(e.slug, slug_unique(&g, "evil.com", Some(idx)));
    }

    #[test]
    fn recommitting_same_label_keeps_a_stable_slug() {
        let mut g: RsGraph = Graph::from(&StableGraph::default());
        let idx = spawn_node(&mut g, EntityKind::Domain, "evil.com", "evil.com", Pos2::ZERO).unwrap();
        relabel(&mut g, idx, "evil.com").unwrap();
        relabel(&mut g, idx, "evil.com").unwrap();
        assert_eq!(g.node(idx).unwrap().payload().slug, "evil_com");
    }

    #[test]
    fn rename_colliding_with_another_node_suffixes() {
        let mut g: RsGraph = Graph::from(&StableGraph::default());
        spawn_node(&mut g, EntityKind::Domain, "evil.com", "evil.com", Pos2::ZERO).unwrap();
        let b = spawn_node(&mut g, EntityKind::Domain, "other", "other", Pos2::ZERO).unwrap();
        relabel(&mut g, b, "evil.com").unwrap();
        assert_eq!(g.node(b).unwrap().payload().slug, "evil_com_2");
    }
}

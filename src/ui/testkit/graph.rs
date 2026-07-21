use egui::Pos2;
use petgraph::graph::{EdgeIndex, NodeIndex};

use crate::error::Error;
use crate::model::{
    EntityKind, EntityRec, ObsValue, Observation, Origin, RELATION_META, Relation, RelationKind,
    RelationRec, Source, SourceKind, Verification, slugify,
};
use crate::ui::RedstringApp;

pub fn entity_rec(kind: EntityKind, label: &str, value: &str) -> Result<EntityRec, Error> {
    Ok(EntityRec {
        slug: slugify(label)?,
        kind,
        label: label.to_owned(),
        value: value.to_owned(),
        sealed: false,
        observations: Vec::new(),
        facets: Default::default(),
        image: None,
    })
}

pub fn relation_rec(
    from: &str,
    to: &str,
    kind: RelationKind,
    author: &crate::model::Author,
) -> Result<RelationRec, Error> {
    Ok(RelationRec {
        from: from.to_owned(),
        to: to.to_owned(),
        kind,
        origin: Origin::Manual,
        status: Verification::Unreviewed,
        confidence: 100,
        author: author.clone(),
        created_at: 0,
        sources: Vec::new(),
        sealed: false,
    })
}

pub fn place(n: usize) -> Result<Pos2, Error> {
    let a = n as f32 * 2.399_963_2;
    let r = 40.0 + 16.0 * n as f32;
    Ok(Pos2::new(r * a.cos(), r * a.sin()))
}

pub fn add(app: &mut RedstringApp, kind: EntityKind, label: &str) -> Result<NodeIndex, Error> {
    let n = app.g.g().node_count();
    app.add_entity(kind, label.to_owned(), place(n)?)
}

pub fn add_valued(
    app: &mut RedstringApp,
    kind: EntityKind,
    label: &str,
    value: &str,
) -> Result<NodeIndex, Error> {
    let idx = add(app, kind, label)?;
    set_value(app, idx, value)?;
    Ok(idx)
}

pub fn set_value(app: &mut RedstringApp, idx: NodeIndex, value: &str) -> Result<(), Error> {
    let Some(n) = app.g.node_mut(idx) else {
        return Err(Error::Testkit {
            what: format!("set_value: node {} not found", idx.index()),
        });
    };
    n.payload_mut().value = value.to_owned();
    Ok(())
}

pub fn observe(
    app: &mut RedstringApp,
    idx: NodeIndex,
    name: &str,
    value: &str,
    origin: Origin,
    status: Verification,
) -> Result<(), Error> {
    let author = app.author.clone();
    let obs = Observation::stamp(
        name.to_owned(),
        ObsValue::Text(value.to_owned()),
        origin,
        status,
        author,
    )?;
    let Some(n) = app.g.node_mut(idx) else {
        return Err(Error::Testkit {
            what: format!("observe: node {} not found", idx.index()),
        });
    };
    n.payload_mut().observations.push(obs);
    Ok(())
}

pub fn source(kind: SourceKind, reference: &str, excerpt: &str) -> Result<Source, Error> {
    Ok(Source {
        kind,
        reference: reference.to_owned(),
        excerpt: excerpt.to_owned(),
        ..Default::default()
    })
}

fn stamped_link(
    app: &mut RedstringApp,
    from: NodeIndex,
    to: NodeIndex,
    kind: RelationKind,
    origin: Origin,
    status: Verification,
    sources: Vec<Source>,
) -> Result<EdgeIndex, Error> {
    let mut rel = Relation::stamp(kind, origin, status, app.author.clone())?;
    rel.sources = sources;
    let label = RELATION_META[kind as usize].label.to_owned();
    Ok(app.g.add_edge_with_label(from, to, rel, label))
}

pub fn link(
    app: &mut RedstringApp,
    from: NodeIndex,
    to: NodeIndex,
    kind: RelationKind,
) -> Result<EdgeIndex, Error> {
    stamped_link(
        app,
        from,
        to,
        kind,
        Origin::Manual,
        Verification::Unreviewed,
        Vec::new(),
    )
}

pub fn link_sourced(
    app: &mut RedstringApp,
    from: NodeIndex,
    to: NodeIndex,
    kind: RelationKind,
    src: Source,
) -> Result<EdgeIndex, Error> {
    stamped_link(
        app,
        from,
        to,
        kind,
        Origin::Manual,
        Verification::AnalystAsserted,
        vec![src],
    )
}

pub fn node_count(app: &RedstringApp) -> Result<usize, Error> {
    Ok(app.g.g().node_count())
}

pub fn edge_count(app: &RedstringApp) -> Result<usize, Error> {
    Ok(app.g.g().edge_count())
}

pub fn node_by_slug(app: &RedstringApp, slug: &str) -> Result<Option<NodeIndex>, Error> {
    let g = app.g.g();
    for idx in g.node_indices() {
        let Some(n) = g.node_weight(idx) else {
            continue;
        };
        if n.payload().slug == slug {
            return Ok(Some(idx));
        }
    }
    Ok(None)
}

pub fn nodes_with_value(
    app: &RedstringApp,
    kind: EntityKind,
    value: &str,
) -> Result<Vec<NodeIndex>, Error> {
    let g = app.g.g();
    let mut out = Vec::new();
    for idx in g.node_indices() {
        let Some(n) = g.node_weight(idx) else {
            continue;
        };
        let e = n.payload();
        if e.kind == kind && e.value == value {
            out.push(idx);
        }
    }
    Ok(out)
}

pub fn slugs(app: &RedstringApp) -> Result<Vec<String>, Error> {
    let g = app.g.g();
    let mut out = Vec::new();
    for idx in g.node_indices() {
        let Some(n) = g.node_weight(idx) else {
            continue;
        };
        out.push(n.payload().slug.clone());
    }
    out.sort();
    Ok(out)
}

pub fn slug_of(app: &RedstringApp, idx: NodeIndex) -> Result<String, Error> {
    let Some(n) = app.g.g().node_weight(idx) else {
        return Err(Error::Testkit {
            what: format!("slug_of: node {} not found", idx.index()),
        });
    };
    Ok(n.payload().slug.clone())
}

pub fn label_of(app: &RedstringApp, idx: NodeIndex) -> Result<String, Error> {
    let Some(n) = app.g.g().node_weight(idx) else {
        return Err(Error::Testkit {
            what: format!("label_of: node {} not found", idx.index()),
        });
    };
    Ok(n.payload().label.clone())
}

pub fn value_of(app: &RedstringApp, idx: NodeIndex) -> Result<String, Error> {
    let Some(n) = app.g.g().node_weight(idx) else {
        return Err(Error::Testkit {
            what: format!("value_of: node {} not found", idx.index()),
        });
    };
    Ok(n.payload().value.clone())
}

pub fn kind_of(app: &RedstringApp, idx: NodeIndex) -> Result<EntityKind, Error> {
    let Some(n) = app.g.g().node_weight(idx) else {
        return Err(Error::Testkit {
            what: format!("kind_of: node {} not found", idx.index()),
        });
    };
    Ok(n.payload().kind)
}

pub fn observations_of(app: &RedstringApp, idx: NodeIndex) -> Result<Vec<Observation>, Error> {
    let Some(n) = app.g.g().node_weight(idx) else {
        return Err(Error::Testkit {
            what: format!("observations_of: node {} not found", idx.index()),
        });
    };
    Ok(n.payload().observations.clone())
}

pub fn observation(
    app: &RedstringApp,
    idx: NodeIndex,
    name: &str,
) -> Result<Option<Observation>, Error> {
    let obs = observations_of(app, idx)?;
    Ok(obs.into_iter().find(|o| o.name == name))
}

pub fn collapsed_of(app: &RedstringApp, idx: NodeIndex) -> Result<bool, Error> {
    let Some(n) = app.g.g().node_weight(idx) else {
        return Err(Error::Testkit {
            what: format!("collapsed_of: node {} not found", idx.index()),
        });
    };
    Ok(n.payload().collapsed)
}

pub fn hidden_slugs(app: &RedstringApp) -> Result<Vec<String>, Error> {
    let g = app.g.g();
    let mut out = Vec::new();
    for idx in g.node_indices() {
        let Some(n) = g.node_weight(idx) else {
            continue;
        };
        if n.payload().hidden {
            out.push(n.payload().slug.clone());
        }
    }
    out.sort();
    Ok(out)
}

pub fn has_edge(
    app: &RedstringApp,
    from: NodeIndex,
    to: NodeIndex,
    kind: RelationKind,
) -> Result<bool, Error> {
    let g = app.g.g();
    for er in g.edges_connecting(from, to) {
        if er.weight().payload().kind == kind {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn edge_relations(app: &RedstringApp) -> Result<Vec<Relation>, Error> {
    let g = app.g.g();
    let mut out = Vec::new();
    for eidx in g.edge_indices() {
        let Some(ed) = g.edge_weight(eidx) else {
            continue;
        };
        out.push(ed.payload().clone());
    }
    Ok(out)
}

pub fn edge_sources(
    app: &RedstringApp,
    from: NodeIndex,
    to: NodeIndex,
    kind: RelationKind,
) -> Result<Vec<Source>, Error> {
    let g = app.g.g();
    for er in g.edges_connecting(from, to) {
        let rel = er.weight().payload();
        if rel.kind == kind {
            return Ok(rel.sources.clone());
        }
    }
    Err(Error::Testkit {
        what: format!(
            "edge_sources: no {:?} edge {}->{}",
            kind,
            from.index(),
            to.index()
        ),
    })
}

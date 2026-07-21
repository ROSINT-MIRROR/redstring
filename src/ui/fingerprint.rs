use crate::error::Error;
use crate::model::{CaseDisplay, DEFAULT_NODE_SIZE, Entity, EntityRec, Relation, RelationRec};

use super::app::RsGraph;

pub fn is_dirty(case_open: bool, fp: &str, saved_fp: &str) -> Result<bool, Error> {
    Ok(dirty_state(case_open, fp, saved_fp))
}

fn dirty_state(case_open: bool, fp: &str, saved_fp: &str) -> bool {
    case_open && fp != saved_fp
}

pub fn compute_fingerprint(graph: &RsGraph, case_name: &str, size_by_degree: bool) -> Result<String, Error> {
    let g = graph.g();
    let mut recs: Vec<String> = Vec::new();
    let mut display = CaseDisplay {
        size_by_degree,
        ..CaseDisplay::default()
    };
    for n in g.node_weights() {
        let e = n.payload();
        let loc = n.location();
        recs.push(node_fp_line(e)?);
        display.positions.insert(e.slug.clone(), (loc.x, loc.y));
        if e.size != DEFAULT_NODE_SIZE {
            display.sizes.insert(e.slug.clone(), e.size);
        }
        if e.collapsed {
            display.collapsed.insert(e.slug.clone());
        }
    }
    for idx in g.edge_indices() {
        let Some((a, b)) = g.edge_endpoints(idx) else { continue };
        let Some(r) = g.edge_weight(idx) else { continue };
        let Some(from) = g.node_weight(a) else { continue };
        let Some(to) = g.node_weight(b) else { continue };
        recs.push(edge_fp_line(&from.payload().slug, &to.payload().slug, r.payload())?);
    }
    recs.sort();
    recs.insert(0, display_fp(&display)?);
    recs.insert(0, case_name.to_owned());
    Ok(recs.join("\n"))
}

fn node_fp_line(e: &Entity) -> Result<String, Error> {
    let rec = EntityRec {
        slug: e.slug.clone(),
        kind: e.kind,
        label: e.label.clone(),
        value: e.value.clone(),
        sealed: e.sealed,
        observations: e.observations.clone(),
        facets: e.facets.clone(),
        image: e.image.clone(),
    };
    let body = serde_json::to_string(&rec).map_err(|source| Error::JsonSer { source })?;
    Ok(format!("n|{body}|{:?}", e.note))
}

fn edge_fp_line(from: &str, to: &str, r: &Relation) -> Result<String, Error> {
    let rec = RelationRec {
        from: from.to_owned(),
        to: to.to_owned(),
        kind: r.kind,
        origin: r.origin.clone(),
        status: r.status,
        confidence: r.confidence,
        author: r.author.clone(),
        created_at: r.created_at,
        sources: r.sources.clone(),
        valid_from: r.valid_from,
        valid_to: r.valid_to,
        sealed: r.sealed,
    };
    let body = serde_json::to_string(&rec).map_err(|source| Error::JsonSer { source })?;
    Ok(format!("e|{body}"))
}

fn display_fp(d: &CaseDisplay) -> Result<String, Error> {
    let body = serde_json::to_string(&(&d.positions, &d.sizes, &d.collapsed, d.size_by_degree))
        .map_err(|source| Error::JsonSer { source })?;
    Ok(format!("d|{body}"))
}

#[cfg(test)]
mod tests {
    use super::{
        RsGraph, compute_fingerprint, display_fp, dirty_state, edge_fp_line, node_fp_line,
    };
    use crate::model::{
        Author, Camera, CaseDisplay, DatePrecision, DateVal, Entity, EntityKind, Facet, FacetValue,
        Occupation, OccupationIcon, Origin, Relation, RelationKind, Verification,
    };
    use crate::ui::testkit::nodes;
    use egui::Pos2;
    use egui_graphs::Graph;
    use petgraph::stable_graph::StableGraph;

    fn bare_person() -> Entity {
        nodes::entity(EntityKind::Person, "p", "P", "")
    }

    fn person(slug: &str) -> Entity {
        Entity {
            slug: slug.into(),
            ..bare_person()
        }
    }

    fn relation(confidence: u8, status: Verification) -> Relation {
        Relation {
            kind: RelationKind::Controls,
            origin: Origin::Manual,
            status,
            confidence,
            author: Author::default(),
            created_at: 0,
            sources: Vec::new(),
            valid_from: None,
            valid_to: None,
            sealed: false,
            hidden: false,
        }
    }

    fn tiny_graph(confidence: u8, status: Verification) -> RsGraph {
        let mut g: RsGraph = Graph::from(&StableGraph::default());
        let a = g.add_node_with_label_and_location(person("a"), "a".into(), Pos2::new(0.0, 0.0));
        let b = g.add_node_with_label_and_location(person("b"), "b".into(), Pos2::new(20.0, 0.0));
        g.add_edge_with_label(a, b, relation(confidence, status), "rel".into());
        g
    }

    #[test]
    fn occupation_changes_node_fingerprint() {
        let base = bare_person();
        let mut judge = base.clone();
        judge.facets.insert(
            Facet::Occupation,
            FacetValue::Freeform(Occupation {
                label: "Judge".into(),
                icon: OccupationIcon::Scales,
            }),
        );
        assert_ne!(
            node_fp_line(&base).unwrap(),
            node_fp_line(&judge).unwrap(),
            "setting an occupation must change the dirty fingerprint"
        );

        let mut lawyer = base.clone();
        lawyer.facets.insert(
            Facet::Occupation,
            FacetValue::Freeform(Occupation {
                label: "Judge".into(),
                icon: OccupationIcon::Briefcase,
            }),
        );
        assert_ne!(
            node_fp_line(&judge).unwrap(),
            node_fp_line(&lawyer).unwrap(),
            "changing only the occupation icon must change the fingerprint"
        );
    }

    #[test]
    fn edge_confidence_change_flips_dirty() {
        let saved = edge_fp_line("a", "b", &relation(50, Verification::Unreviewed)).unwrap();
        let bumped = edge_fp_line("a", "b", &relation(80, Verification::Unreviewed)).unwrap();
        assert_ne!(saved, bumped, "bumping edge confidence must change the fingerprint");
        assert!(
            dirty_state(true, &bumped, &saved),
            "an edge confidence change must mark the case dirty"
        );
    }

    #[test]
    fn edge_verification_change_flips_dirty() {
        let saved = edge_fp_line("a", "b", &relation(80, Verification::Unreviewed)).unwrap();
        let verified = edge_fp_line("a", "b", &relation(80, Verification::Corroborated)).unwrap();
        assert_ne!(saved, verified, "re-verifying an edge must change the fingerprint");
        assert!(
            dirty_state(true, &verified, &saved),
            "an edge verification change must mark the case dirty"
        );
    }

    #[test]
    fn edge_source_provenance_change_flips_dirty() {
        use crate::model::{Source, SourceKind, SourceTier};
        let mut base = relation(80, Verification::Unreviewed);
        base.sources.push(Source {
            kind: SourceKind::Document,
            reference: "evidence/docs/whois.txt".into(),
            ..Default::default()
        });
        let saved = edge_fp_line("a", "b", &base).unwrap();

        let mut hashed = base.clone();
        hashed.sources[0].sha256 = Some("deadbeef".repeat(8));
        let after_hash = edge_fp_line("a", "b", &hashed).unwrap();
        assert_ne!(saved, after_hash, "attaching a source sha256 must change the fingerprint");
        assert!(
            dirty_state(true, &after_hash, &saved),
            "a source sha256 change must mark the case dirty"
        );

        let mut rated = base.clone();
        rated.sources[0].tier = SourceTier::Primary;
        let after_tier = edge_fp_line("a", "b", &rated).unwrap();
        assert_ne!(saved, after_tier, "grading a source's tier must change the fingerprint");
        assert!(
            dirty_state(true, &after_tier, &saved),
            "a source tier change must mark the case dirty"
        );
    }

    #[test]
    fn edge_validity_window_change_flips_dirty() {
        let saved = edge_fp_line("a", "b", &relation(80, Verification::Unreviewed)).unwrap();
        let mut windowed = relation(80, Verification::Unreviewed);
        windowed.valid_from = Some(DateVal {
            secs: 1_325_376_000,
            precision: DatePrecision::Year,
        });
        let after = edge_fp_line("a", "b", &windowed).unwrap();
        assert_ne!(saved, after, "setting a relation's valid_from must change the fingerprint");
        assert!(
            dirty_state(true, &after, &saved),
            "editing an edge's validity window must mark the case dirty"
        );

        let mut moved = windowed.clone();
        moved.valid_to = Some(DateVal {
            secs: 1_356_998_400,
            precision: DatePrecision::Year,
        });
        let after_to = edge_fp_line("a", "b", &moved).unwrap();
        assert_ne!(after, after_to, "adding valid_to must further change the fingerprint");
    }

    #[test]
    fn edge_seal_toggle_flips_dirty() {
        let saved = edge_fp_line("a", "b", &relation(80, Verification::Unreviewed)).unwrap();
        let mut sealed = relation(80, Verification::Unreviewed);
        sealed.sealed = true;
        let after = edge_fp_line("a", "b", &sealed).unwrap();
        assert_ne!(saved, after, "sealing an edge must change the fingerprint");
        assert!(
            dirty_state(true, &after, &saved),
            "sealing a relation must mark the case dirty"
        );
    }

    #[test]
    fn camera_change_leaves_fingerprint_stable() {
        let mut d = CaseDisplay::default();
        d.positions.insert("p".into(), (4.0, 9.0));
        let before = display_fp(&d).unwrap();
        d.camera = Camera {
            x: 999.0,
            y: -999.0,
            zoom: 7.5,
        };
        let after = display_fp(&d).unwrap();
        assert_eq!(before, after, "panning/zooming must not enter the fingerprint");
        assert!(
            !dirty_state(true, &after, &before),
            "a camera-only change must never mark the case dirty"
        );

        d.positions.insert("p".into(), (5.0, 9.0));
        let moved = display_fp(&d).unwrap();
        assert_ne!(before, moved, "moving a node must change the fingerprint");
    }

    #[test]
    fn full_fingerprint_round_trip_and_edge_flip() {
        let saved =
            compute_fingerprint(&tiny_graph(50, Verification::Unreviewed), "case", false).unwrap();
        let reloaded =
            compute_fingerprint(&tiny_graph(50, Verification::Unreviewed), "case", false).unwrap();
        assert!(
            !dirty_state(true, &reloaded, &saved),
            "an identical save→load rebuild must not be dirty"
        );

        let rebumped =
            compute_fingerprint(&tiny_graph(80, Verification::Unreviewed), "case", false).unwrap();
        assert!(
            dirty_state(true, &rebumped, &saved),
            "edge confidence must flip dirty through the full fingerprint"
        );

        let reverified =
            compute_fingerprint(&tiny_graph(50, Verification::Corroborated), "case", false).unwrap();
        assert!(
            dirty_state(true, &reverified, &saved),
            "edge verification must flip dirty through the full fingerprint"
        );
    }

    #[test]
    fn blank_start_is_never_dirty() {
        assert!(!dirty_state(false, "untitled case|false", ""));
        assert!(!dirty_state(false, "untitled case|false", "untitled case|false"));
        assert!(!dirty_state(false, "person built but unsaved", "anything else"));
    }

    #[test]
    fn open_case_dirties_only_on_divergence() {
        assert!(dirty_state(true, "edited fingerprint", "saved fingerprint"));
        assert!(!dirty_state(true, "same fingerprint", "same fingerprint"));
    }
}

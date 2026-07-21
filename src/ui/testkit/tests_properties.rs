#[cfg(test)]
mod tests {
    use crate::model::{CaseFile, CaseMeta, EntityKind, KIND_META, Origin, RELATION_META, Verification};
    use crate::ui::testkit::{graph, harness, invariant};

    #[test]
    fn derived_op_sequence_preserves_all_invariants() {
        let mut app = harness::app().unwrap();
        let mut nodes = Vec::new();
        for i in 0..40usize {
            let kind = KIND_META[i % KIND_META.len()].kind;
            let idx = graph::add(&mut app, kind, &format!("Entity {i}")).unwrap();
            nodes.push(idx);
            if i % 4 == 0 {
                graph::observe(
                    &mut app,
                    idx,
                    "seen",
                    &format!("v{i}"),
                    Origin::Manual,
                    Verification::Unreviewed,
                )
                .unwrap();
            }
            if i > 0 && i % 3 == 0 {
                let rel = RELATION_META[i % RELATION_META.len()].kind;
                graph::link(&mut app, nodes[i], nodes[i - 1], rel).unwrap();
            }
        }
        assert_eq!(graph::node_count(&app).unwrap(), 40);
        invariant::check_graph(&app).unwrap();
    }

    #[test]
    fn derived_case_round_trips_and_stays_consistent() {
        let (td, project) = harness::temp_project().unwrap();
        let author = harness::test_author().unwrap();
        let mut cf = CaseFile::default();
        cf.meta = CaseMeta::stamped("derived").unwrap();
        let mut slugs = Vec::new();
        for i in 0..30usize {
            let kind = KIND_META[i % KIND_META.len()].kind;
            let rec = graph::entity_rec(kind, &format!("Rec {i}"), "").unwrap();
            slugs.push(rec.slug.clone());
            cf.data.entities.push(rec);
        }
        for i in 1..30usize {
            if i % 2 == 0 {
                let kind = RELATION_META[i % RELATION_META.len()].kind;
                cf.data
                    .relations
                    .push(graph::relation_rec(&slugs[i], &slugs[i - 1], kind, &author).unwrap());
            }
        }
        let back = harness::round_trip(&project, &cf).unwrap();
        invariant::assert_round_trip(&cf, &back).unwrap();
        invariant::check_case_file(&back).unwrap();
        drop(td);
    }

    #[test]
    fn many_colliding_labels_get_unique_derivable_slugs() {
        let mut app = harness::app().unwrap();
        for _ in 0..25 {
            graph::add(&mut app, EntityKind::Person, "Duplicate Name").unwrap();
        }
        let mut slugs = graph::slugs(&app).unwrap();
        assert_eq!(slugs.len(), 25);
        slugs.dedup();
        assert_eq!(slugs.len(), 25, "every slug is unique despite one shared label");
        invariant::check_graph(&app).unwrap();
    }

    #[test]
    fn rename_sweep_keeps_slugs_unique_and_consistent() {
        let mut app = harness::app().unwrap();
        let mut nodes = Vec::new();
        for i in 0..15usize {
            nodes.push(graph::add(&mut app, EntityKind::Organization, &format!("Org {i}")).unwrap());
        }
        for (i, idx) in nodes.iter().enumerate() {
            app.apply_label(*idx, &format!("Renamed Entity {}", i % 5)).unwrap();
            invariant::check_graph(&app).unwrap();
        }
        assert_eq!(graph::node_count(&app).unwrap(), 15);
        invariant::check_graph(&app).unwrap();
    }
}

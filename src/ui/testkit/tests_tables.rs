#[cfg(test)]
mod tests {
    use crate::model::{
        CaseFile, CaseMeta, EntityKind, KIND_META, RELATION_META, SOURCE_META, Source, slugify,
    };
    use crate::project::{read_case, write_case};
    use crate::ui::testkit::{graph, harness, invariant};

    #[test]
    fn every_entity_kind_disk_round_trips() {
        let (_td, project) = harness::temp_project().unwrap();
        for m in KIND_META.iter() {
            let mut cf = CaseFile::default();
            cf.meta = CaseMeta::stamped("kinds").unwrap();
            let label = format!("Sample {}", m.label);
            cf.data
                .entities
                .push(graph::entity_rec(m.kind, &label, "").unwrap());
            write_case(&project, &cf).unwrap();
            let back = read_case(&project).unwrap();
            assert_eq!(back.data.entities.len(), 1, "kind {:?}", m.kind);
            assert_eq!(back.data.entities[0].kind, m.kind, "kind {:?}", m.kind);
            invariant::assert_round_trip(&cf, &back).unwrap();
            invariant::check_case_file(&back).unwrap();
        }
    }

    #[test]
    fn every_relation_kind_disk_round_trips() {
        let (_td, project) = harness::temp_project().unwrap();
        let author = harness::test_author().unwrap();
        for m in RELATION_META.iter() {
            let mut cf = CaseFile::default();
            cf.meta = CaseMeta::stamped("rels").unwrap();
            cf.data
                .entities
                .push(graph::entity_rec(EntityKind::Person, "Alice Roe", "").unwrap());
            cf.data
                .entities
                .push(graph::entity_rec(EntityKind::Organization, "Acme Corp", "").unwrap());
            cf.data
                .relations
                .push(graph::relation_rec("alice_roe", "acme_corp", m.kind, &author).unwrap());
            write_case(&project, &cf).unwrap();
            let back = read_case(&project).unwrap();
            assert_eq!(back.data.relations.len(), 1, "rel {:?}", m.kind);
            assert_eq!(back.data.relations[0].kind, m.kind, "rel {:?} kind", m.kind);
            invariant::assert_round_trip(&cf, &back).unwrap();
            invariant::check_case_file(&back).unwrap();
        }
    }

    #[test]
    fn every_source_kind_serde_round_trips() {
        for m in SOURCE_META.iter() {
            let s = Source {
                kind: m.kind,
                reference: format!("ref/for/{}", m.label),
                excerpt: "supporting excerpt".to_owned(),
            };
            let json = serde_json::to_string(&s).unwrap();
            let back: Source = serde_json::from_str(&json).unwrap();
            assert_eq!(s, back, "source kind {:?}", m.kind);
        }
        assert_eq!(SOURCE_META.len(), 6, "source taxonomy size guard");
    }

    #[test]
    fn kind_meta_labels_slugify_nonempty_and_unique() {
        let mut seen: Vec<String> = Vec::new();
        for m in KIND_META.iter() {
            let slug = slugify(m.label).unwrap();
            assert!(!slug.is_empty(), "{:?} slugifies to empty", m.kind);
            assert!(!seen.contains(&slug), "duplicate kind slug '{slug}'");
            seen.push(slug);
        }
        assert_eq!(seen.len(), KIND_META.len());
    }

    #[test]
    fn stamped_ontology_covers_every_table_entry() {
        let meta = CaseMeta::stamped("ontology").unwrap();
        assert_eq!(meta.ontology.entity_kinds.len(), KIND_META.len());
        assert_eq!(meta.ontology.relation_kinds.len(), RELATION_META.len());
        for m in KIND_META.iter() {
            let name = format!("{:?}", m.kind);
            assert!(
                meta.ontology.entity_kinds.contains(&name),
                "ontology missing kind {name}"
            );
        }
        for m in RELATION_META.iter() {
            let name = format!("{:?}", m.kind);
            assert!(
                meta.ontology.relation_kinds.contains(&name),
                "ontology missing relation {name}"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::model::{
        CaseFile, CaseMeta, Entity, EntityKind, Facet, FacetValue, KIND_META, RELATION_META,
        SOURCE_META, Source, slugify, subtype_value,
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
                ..Default::default()
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
    fn folded_org_subtypes_resolve_to_organization_and_round_trip() {
        let by_slug =
            |want: &str| KIND_META.iter().find(|m| slugify(m.label).unwrap() == want).map(|m| m.kind);
        for gone in ["law_firm", "media_outlet", "political_party", "criminal_org"] {
            assert_eq!(by_slug(gone), None, "{gone} is no longer a base kind");
        }
        assert_eq!(by_slug("organization"), Some(EntityKind::Organization));
        assert_eq!(by_slug("court"), Some(EntityKind::Court), "Court stays a distinct kind");
        assert_eq!(by_slug("bank"), Some(EntityKind::Bank), "Bank stays a distinct kind");

        assert_eq!(
            subtype_value(Facet::OrgType, 7).unwrap().expect("law-firm vocab row").label,
            "Law firm",
            "the folded LawFirm kind survives as an OrgType vocab value"
        );

        let mut e = Entity {
            slug: "acme_legal".into(),
            kind: EntityKind::Organization,
            label: "Acme Legal".into(),
            ..Default::default()
        };
        e.facets.insert(Facet::OrgType, FacetValue::Indexed(7));

        let ron_s = ron::to_string(&e).unwrap();
        let from_ron: Entity = ron::from_str(&ron_s).unwrap();
        assert_eq!(from_ron.kind, EntityKind::Organization);
        assert_eq!(
            from_ron.facets.get(&Facet::OrgType),
            Some(&FacetValue::Indexed(7)),
            "an Organization tagged OrgType=Law firm survives RON (entity.ron)"
        );

        let json = serde_json::to_string(&e).unwrap();
        let from_json: Entity = serde_json::from_str(&json).unwrap();
        assert_eq!(from_json.facets.get(&Facet::OrgType), Some(&FacetValue::Indexed(7)));
    }

    #[test]
    fn round3_cohort_present_and_money_payment_folded() {
        let by_slug =
            |want: &str| KIND_META.iter().find(|m| slugify(m.label).unwrap() == want).map(|m| m.kind);
        assert_eq!(by_slug("cohort"), Some(EntityKind::Cohort));
        assert_eq!(by_slug("money"), None, "Money folded into Asset");
        assert_eq!(by_slug("payment"), None, "Payment folded into Transaction");

        let cohort = &KIND_META[EntityKind::Cohort as usize];
        assert_eq!(
            cohort.category,
            KIND_META[EntityKind::Person as usize].category,
            "Cohort sits with Person in the People category"
        );
        assert_eq!(cohort.facets, &[Facet::Role], "a cohort is an actor aggregate — Role only");
        for a in ["members", "description", "age_range"] {
            assert!(cohort.attrs.contains(&a), "Cohort suggests '{a}'");
        }

        let tx = &KIND_META[EntityKind::Transaction as usize];
        for alias in ["payment", "payout", "fee", "bribe", "kickback"] {
            assert!(tx.aliases.contains(&alias), "Transaction absorbs '{alias}'");
        }
        assert!(tx.attrs.contains(&"method"), "Transaction gains Payment's method attr");

        let asset = &KIND_META[EntityKind::Asset as usize];
        for alias in ["cash", "funds", "sum"] {
            assert!(asset.aliases.contains(&alias), "Asset absorbs '{alias}'");
        }

        let person = &KIND_META[EntityKind::Person as usize];
        assert!(person.attrs.contains(&"dod"), "Person suggests date of death");

        let claim = &KIND_META[EntityKind::Claim as usize];
        assert_eq!(
            claim.attrs,
            &["statement"],
            "status/verdict/confidence free-text attrs are replaced by the ClaimStatus facet"
        );
        assert_eq!(KIND_META.len(), 62, "round-3 kind count");
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

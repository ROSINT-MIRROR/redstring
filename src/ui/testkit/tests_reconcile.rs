#[cfg(test)]
mod tests {
    use crate::model::{Camera, EntityKind, ObsValue, Observation, Origin, RelationKind, Verification};
    use crate::project::{CaseState, Resolve, Side, merge};
    use crate::ui::testkit::{graph, harness};

    fn state(slugs: &[&str]) -> CaseState {
        let mut s = CaseState::default();
        s.file.meta.name = "case".to_owned();
        for slug in slugs {
            let e = graph::entity_rec(EntityKind::Person, slug, "").unwrap();
            s.notes.insert(e.slug.clone(), String::new());
            s.file.data.entities.push(e);
        }
        s
    }

    fn value_of(s: &CaseState, slug: &str) -> String {
        s.file.data.entities.iter().find(|e| e.slug == slug).unwrap().value.clone()
    }

    fn set_value(s: &mut CaseState, slug: &str, v: &str) {
        s.file.data.entities.iter_mut().find(|e| e.slug == slug).unwrap().value = v.to_owned();
    }

    fn drop_entity(s: &mut CaseState, slug: &str) {
        s.file.data.entities.retain(|e| e.slug != slug);
        s.file.data.relations.retain(|r| r.from != slug && r.to != slug);
        s.notes.remove(slug);
    }

    fn link(s: &mut CaseState, from: &str, to: &str, created_at: i64) {
        let mut r = graph::relation_rec(from, to, RelationKind::AssociateOf, &harness::test_author().unwrap()).unwrap();
        r.created_at = created_at;
        s.file.data.relations.push(r);
    }

    fn obs(id: &str, name: &str) -> Observation {
        let mut o = Observation::stamp(
            name.to_owned(),
            ObsValue::Text("x".to_owned()),
            Origin::Manual,
            Verification::AnalystAsserted,
            harness::test_author().unwrap(),
        )
        .unwrap();
        o.id.0 = id.to_owned();
        o
    }

    fn slugs(s: &CaseState) -> Vec<String> {
        let mut v: Vec<String> = s.file.data.entities.iter().map(|e| e.slug.clone()).collect();
        v.sort();
        v
    }

    #[test]
    fn independent_additions_on_both_sides_union_without_a_question() {
        let base = state(&["a"]);
        let mut local = base.clone();
        local.file.data.entities.push(graph::entity_rec(EntityKind::Person, "mine", "").unwrap());
        let mut disk = base.clone();
        disk.file.data.entities.push(graph::entity_rec(EntityKind::Person, "theirs", "").unwrap());
        link(&mut disk, "a", "theirs", 7);
        let m = merge(&base, &local, &disk, Resolve::Ask).unwrap();
        assert!(m.clashes.is_empty(), "{:?}", m.clashes);
        assert_eq!(slugs(&m.state), vec!["a", "mine", "theirs"]);
        assert_eq!(m.state.file.data.relations.len(), 1, "the disk-side link survives");
    }

    #[test]
    fn distinct_records_edited_on_each_side_both_survive() {
        let base = state(&["a", "b"]);
        let mut local = base.clone();
        set_value(&mut local, "a", "local");
        let mut disk = base.clone();
        set_value(&mut disk, "b", "disk");
        let m = merge(&base, &local, &disk, Resolve::Ask).unwrap();
        assert!(m.clashes.is_empty());
        assert_eq!(value_of(&m.state, "a"), "local");
        assert_eq!(value_of(&m.state, "b"), "disk");
    }

    #[test]
    fn the_same_record_edited_differently_is_a_clash_and_never_a_guess() {
        let base = state(&["a"]);
        let mut local = base.clone();
        set_value(&mut local, "a", "local");
        let mut disk = base.clone();
        set_value(&mut disk, "a", "disk");
        let asked = merge(&base, &local, &disk, Resolve::Ask).unwrap();
        assert_eq!(asked.clashes.len(), 1, "one clash, reported");
        let mine = merge(&base, &local, &disk, Resolve::Keep(Side::Local)).unwrap();
        assert_eq!(value_of(&mine.state, "a"), "local");
        let theirs = merge(&base, &local, &disk, Resolve::Keep(Side::Disk)).unwrap();
        assert_eq!(value_of(&theirs.state, "a"), "disk");
    }

    #[test]
    fn the_same_edit_on_both_sides_is_agreement_not_conflict() {
        let base = state(&["a"]);
        let mut local = base.clone();
        set_value(&mut local, "a", "same");
        let disk = local.clone();
        let m = merge(&base, &local, &disk, Resolve::Ask).unwrap();
        assert!(m.clashes.is_empty());
        assert_eq!(value_of(&m.state, "a"), "same");
    }

    #[test]
    fn disk_deleting_what_you_edited_asks_and_keep_mine_restores_the_whole_entity() {
        let mut base = state(&["a", "b"]);
        base.file.data.entities[0].observations.push(obs("o1", "dob"));
        let mut local = base.clone();
        set_value(&mut local, "a", "edited here");
        let mut disk = base.clone();
        drop_entity(&mut disk, "a");
        let asked = merge(&base, &local, &disk, Resolve::Ask).unwrap();
        assert_eq!(asked.clashes.len(), 1, "{:?}", asked.clashes);
        let mine = merge(&base, &local, &disk, Resolve::Keep(Side::Local)).unwrap();
        assert_eq!(value_of(&mine.state, "a"), "edited here");
        let a = mine.state.file.data.entities.iter().find(|e| e.slug == "a").unwrap();
        assert_eq!(a.observations.len(), 1, "keep mine keeps the entity whole, facts included");
        let theirs = merge(&base, &local, &disk, Resolve::Keep(Side::Disk)).unwrap();
        assert_eq!(slugs(&theirs.state), vec!["b"]);
    }

    #[test]
    fn disk_deleting_an_untouched_entity_just_deletes_it() {
        let base = state(&["a", "b"]);
        let local = base.clone();
        let mut disk = base.clone();
        drop_entity(&mut disk, "a");
        let m = merge(&base, &local, &disk, Resolve::Ask).unwrap();
        assert!(m.clashes.is_empty());
        assert_eq!(slugs(&m.state), vec!["b"]);
    }

    #[test]
    fn a_link_to_an_entity_the_other_side_deleted_is_a_clash_not_a_dangling_edge() {
        let base = state(&["a", "b"]);
        let mut local = base.clone();
        drop_entity(&mut local, "b");
        let mut disk = base.clone();
        link(&mut disk, "a", "b", 9);
        let asked = merge(&base, &local, &disk, Resolve::Ask).unwrap();
        assert_eq!(asked.clashes.len(), 1, "{:?}", asked.clashes);
        let mine = merge(&base, &local, &disk, Resolve::Keep(Side::Local)).unwrap();
        assert_eq!(slugs(&mine.state), vec!["a"], "your delete stands");
        assert!(mine.state.file.data.relations.is_empty(), "and no edge points at nothing");
        let theirs = merge(&base, &local, &disk, Resolve::Keep(Side::Disk)).unwrap();
        assert_eq!(slugs(&theirs.state), vec!["a", "b"], "their side brings the entity back");
        assert_eq!(theirs.state.file.data.relations.len(), 1, "with its new link");
    }

    #[test]
    fn facts_added_on_each_side_to_the_same_entity_union() {
        let base = state(&["a"]);
        let mut local = base.clone();
        local.file.data.entities[0].observations.push(obs("l1", "phone"));
        let mut disk = base.clone();
        disk.file.data.entities[0].observations.push(obs("d1", "email"));
        let m = merge(&base, &local, &disk, Resolve::Ask).unwrap();
        assert!(m.clashes.is_empty());
        let names: Vec<String> = m.state.file.data.entities[0].observations.iter().map(|o| o.name.clone()).collect();
        assert_eq!(names, vec!["phone", "email"], "local order first, then disk additions");
    }

    #[test]
    fn a_link_edited_differently_on_both_sides_clashes() {
        let mut base = state(&["a", "b"]);
        link(&mut base, "a", "b", 3);
        let mut local = base.clone();
        local.file.data.relations[0].confidence = 10;
        let mut disk = base.clone();
        disk.file.data.relations[0].confidence = 90;
        let m = merge(&base, &local, &disk, Resolve::Ask).unwrap();
        assert_eq!(m.clashes.len(), 1);
        let theirs = merge(&base, &local, &disk, Resolve::Keep(Side::Disk)).unwrap();
        assert_eq!(theirs.state.file.data.relations[0].confidence, 90);
    }

    #[test]
    fn view_state_never_clashes_and_your_camera_wins_an_overlap() {
        let base = state(&["a"]);
        let mut local = base.clone();
        local.file.display.camera = Camera { x: 1.0, y: 1.0, zoom: 2.0 };
        let mut disk = base.clone();
        disk.file.display.camera = Camera { x: 9.0, y: 9.0, zoom: 0.5 };
        disk.file.display.positions.insert("a".to_owned(), (5.0, 5.0));
        let m = merge(&base, &local, &disk, Resolve::Ask).unwrap();
        assert!(m.clashes.is_empty(), "camera and layout are view state, not evidence");
        assert_eq!(m.state.file.display.camera, local.file.display.camera);
        assert_eq!(
            m.state.file.display.positions.get("a").copied(),
            Some((5.0, 5.0)),
            "a disk-only position change is taken"
        );
    }
}

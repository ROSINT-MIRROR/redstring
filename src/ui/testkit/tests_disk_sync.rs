#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    use egui::Pos2;

    use crate::model::{Camera, EntityKind, EntityRec, Origin, RelationKind, Verification};
    use crate::project::{Claim, Project, Side, read_case, try_lock, write_case};
    use crate::ui::RedstringApp;
    use crate::ui::board::camera_write;
    use crate::ui::mode::{DEBOUNCE_SECS, IDLE_SECS};
    use crate::ui::testkit::{graph, harness, invariant};

    const SETTLED: f64 = DEBOUNCE_SECS + 0.1;
    const BEAT: f64 = IDLE_SECS + DEBOUNCE_SECS + 1.0;

    fn live_app() -> (harness::TempDir, Project, RedstringApp) {
        let (td, project) = harness::temp_project().unwrap();
        let mut app = harness::app().unwrap();
        app.project = Some(project.clone());
        harness::load(&mut app).unwrap();
        (td, project, app)
    }

    fn settle(app: &mut RedstringApp, t: f64) {
        harness::tick(app, t).unwrap();
        harness::tick(app, t + SETTLED).unwrap();
    }

    fn tree(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
        let mut out = BTreeMap::new();
        walk(root, root, &mut out);
        out
    }

    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, out);
                continue;
            }
            out.insert(path.strip_prefix(root).unwrap().to_path_buf(), std::fs::read(&path).unwrap());
        }
    }

    fn changed(a: &BTreeMap<PathBuf, Vec<u8>>, b: &BTreeMap<PathBuf, Vec<u8>>) -> Vec<String> {
        let mut keys: Vec<&PathBuf> = a.keys().chain(b.keys()).collect();
        keys.sort();
        keys.dedup();
        keys.into_iter()
            .filter(|k| a.get(*k) != b.get(*k))
            .map(|k| k.display().to_string())
            .collect()
    }

    fn shell_add(project: &Project, label: &str, value: &str) -> String {
        let rec = graph::entity_rec(EntityKind::Organization, label, value).unwrap();
        let dir = project.entity_dir(&rec.slug).unwrap();
        std::fs::create_dir_all(&dir).unwrap();
        let ron = ron::ser::to_string_pretty(&rec, ron::ser::PrettyConfig::default()).unwrap();
        std::fs::write(dir.join("entity.ron"), ron).unwrap();
        rec.slug
    }

    fn shell_set_value(project: &Project, slug: &str, value: &str) {
        let path = project.entity_head_path(slug).unwrap();
        let mut rec: EntityRec = ron::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        rec.value = value.to_owned();
        std::fs::write(&path, ron::ser::to_string_pretty(&rec, ron::ser::PrettyConfig::default()).unwrap())
            .unwrap();
    }

    fn shell_link(project: &Project, from: &str, to: &str) {
        let rec = graph::relation_rec(from, to, RelationKind::AssociateOf, &harness::test_author().unwrap()).unwrap();
        let path = project.relations_path().unwrap();
        let mut text = std::fs::read_to_string(&path).unwrap();
        text.push_str(&serde_json::to_string(&rec).unwrap());
        text.push('\n');
        std::fs::write(&path, text).unwrap();
    }

    fn disk_value(project: &Project, slug: &str) -> String {
        read_case(project).unwrap().data.entities.iter().find(|e| e.slug == slug).unwrap().value.clone()
    }

    fn node(app: &RedstringApp, slug: &str) -> petgraph::graph::NodeIndex {
        graph::node_by_slug(app, slug).unwrap().unwrap()
    }

    fn in_ctx(ctx: &egui::Context, app: &mut RedstringApp, t: f64, cam: Camera, poll: bool) {
        let input = egui::RawInput {
            time: Some(t),
            ..Default::default()
        };
        let _out = ctx.run_ui(input, |ui| {
            camera_write(ui, cam).unwrap();
            if poll {
                app.watcher.last_poll = Instant::now() - Duration::from_millis(1100);
                app.poll_external_edit(ui).unwrap();
            }
            app.autosave_tick(ui).unwrap();
        });
    }

    #[test]
    fn a_camera_save_never_eats_an_entity_added_on_disk_meanwhile() {
        let (td, project, mut app) = live_app();
        graph::add(&mut app, EntityKind::Person, "anchor").unwrap();
        settle(&mut app, 0.0);

        let ctx = egui::Context::default();
        let cam = Camera { x: 300.0, y: -120.0, zoom: 1.75 };
        in_ctx(&ctx, &mut app, 10.0, cam, false);
        let planted = shell_add(&project, "Planted Org", "planted.example");
        in_ctx(&ctx, &mut app, 10.0 + BEAT, cam, false);
        assert!(
            project.entity_head_path(&planted).unwrap().exists(),
            "the camera beat fired with an unabsorbed disk change and must not prune or rewrite over it"
        );

        in_ctx(&ctx, &mut app, 30.0, cam, true);
        in_ctx(&ctx, &mut app, 30.5, cam, true);
        assert!(graph::node_by_slug(&app, &planted).unwrap().is_some(), "the board absorbed the addition");

        let before = tree(&project.root);
        in_ctx(&ctx, &mut app, 31.0, cam, false);
        in_ctx(&ctx, &mut app, 31.0 + BEAT, cam, false);
        let after = tree(&project.root);
        assert_eq!(
            changed(&before, &after),
            vec!["display.ron"],
            "a view-only save rewrites display.ron and nothing else"
        );
        assert_eq!(read_case(&project).unwrap().display.camera, cam, "the camera reached disk");
        assert!(project.entity_head_path(&planted).unwrap().exists());
        invariant::check_graph(&app).unwrap();
        drop(td);
    }

    #[test]
    fn independent_edits_on_both_sides_merge_and_both_reach_disk() {
        let (td, project, mut app) = live_app();
        let a = graph::add(&mut app, EntityKind::Person, "alice").unwrap();
        graph::add(&mut app, EntityKind::Person, "bob").unwrap();
        settle(&mut app, 0.0);

        graph::set_value(&mut app, a, "edited in the gui").unwrap();
        shell_set_value(&project, "bob", "edited by an agent");
        shell_link(&project, "alice", "bob");
        let carol = shell_add(&project, "Carol Corp", "");
        harness::watch(&mut app).unwrap();

        assert_eq!(graph::value_of(&app, node(&app, "alice")).unwrap(), "edited in the gui");
        assert_eq!(graph::value_of(&app, node(&app, "bob")).unwrap(), "edited by an agent");
        assert!(graph::has_edge(&app, node(&app, "alice"), node(&app, "bob"), RelationKind::AssociateOf).unwrap());
        assert!(graph::node_by_slug(&app, &carol).unwrap().is_some());
        assert!(app.autosave.conflict.is_none(), "independent changes never ask");

        settle(&mut app, 50.0);
        assert_eq!(disk_value(&project, "alice"), "edited in the gui");
        assert_eq!(disk_value(&project, "bob"), "edited by an agent");
        assert_eq!(read_case(&project).unwrap().data.relations.len(), 1);
        assert!(project.entity_head_path(&carol).unwrap().exists());
        assert!(!app.dirty().unwrap(), "memory and disk converged");
        invariant::check_graph(&app).unwrap();
        drop(td);
    }

    #[test]
    fn an_external_edit_never_wipes_an_unflushed_gui_edit() {
        let (td, project, mut app) = live_app();
        let a = graph::add(&mut app, EntityKind::Person, "alice").unwrap();
        settle(&mut app, 0.0);

        graph::observe(&mut app, a, "dob", "1971-03-04", Origin::Manual, Verification::AnalystAsserted).unwrap();
        assert!(app.dirty().unwrap(), "the fact is only in memory");
        let planted = shell_add(&project, "Planted Org", "");
        harness::watch(&mut app).unwrap();

        let a2 = node(&app, "alice");
        assert!(graph::observation(&app, a2, "dob").unwrap().is_some(), "the unflushed fact survived the reload");
        assert!(graph::node_by_slug(&app, &planted).unwrap().is_some(), "and the external entity arrived");
        assert!(app.dirty().unwrap(), "the gui edit is still owed to disk");

        settle(&mut app, 50.0);
        let cf = read_case(&project).unwrap();
        let alice = cf.data.entities.iter().find(|e| e.slug == "alice").unwrap();
        assert_eq!(alice.observations.len(), 1, "the fact reached disk");
        assert!(cf.data.entities.iter().any(|e| e.slug == planted), "next to the external entity");
        drop(td);
    }

    #[test]
    fn the_same_field_edited_on_both_sides_raises_a_conflict_and_writes_nothing() {
        let (td, project, mut app) = live_app();
        let a = graph::add(&mut app, EntityKind::Person, "alice").unwrap();
        settle(&mut app, 0.0);

        graph::set_value(&mut app, a, "mine").unwrap();
        shell_set_value(&project, "alice", "theirs");
        harness::watch(&mut app).unwrap();
        assert!(app.autosave.conflict.is_some(), "a real collision is shown, not guessed");
        let frozen = tree(&project.root);
        settle(&mut app, 50.0);
        settle(&mut app, 50.0 + BEAT);
        assert_eq!(changed(&frozen, &tree(&project.root)), Vec::<String>::new(), "no autosave while the conflict is open");
        assert_eq!(graph::value_of(&app, node(&app, "alice")).unwrap(), "mine", "your side is still on screen");
        assert_eq!(disk_value(&project, "alice"), "theirs", "their side is still on disk");

        harness::frame(&mut app, |a, ui| a.resolve_conflict(ui, Side::Local)).unwrap();
        assert!(app.autosave.conflict.is_none());
        assert_eq!(disk_value(&project, "alice"), "mine", "keep mine writes your side");
        drop(td);
    }

    #[test]
    fn taking_their_side_of_a_conflict_replaces_only_the_colliding_record() {
        let (td, project, mut app) = live_app();
        let a = graph::add(&mut app, EntityKind::Person, "alice").unwrap();
        let b = graph::add(&mut app, EntityKind::Person, "bob").unwrap();
        settle(&mut app, 0.0);

        graph::set_value(&mut app, a, "mine").unwrap();
        graph::set_value(&mut app, b, "bob local only").unwrap();
        shell_set_value(&project, "alice", "theirs");
        harness::watch(&mut app).unwrap();
        harness::frame(&mut app, |a, ui| a.resolve_conflict(ui, Side::Disk)).unwrap();

        assert_eq!(graph::value_of(&app, node(&app, "alice")).unwrap(), "theirs");
        assert_eq!(graph::value_of(&app, node(&app, "bob")).unwrap(), "bob local only", "the rest still merges");
        assert_eq!(disk_value(&project, "alice"), "theirs");
        assert_eq!(disk_value(&project, "bob"), "bob local only");
        drop(td);
    }

    #[test]
    fn deleting_on_disk_what_you_are_editing_asks_and_keep_mine_restores_it() {
        let (td, project, mut app) = live_app();
        let a = graph::add(&mut app, EntityKind::Person, "alice").unwrap();
        settle(&mut app, 0.0);

        graph::set_value(&mut app, a, "still working on her").unwrap();
        std::fs::remove_dir_all(project.entity_dir("alice").unwrap()).unwrap();
        harness::watch(&mut app).unwrap();
        assert!(app.autosave.conflict.is_some(), "edit vs delete is a conflict");
        assert!(graph::node_by_slug(&app, "alice").unwrap().is_some(), "nothing vanished from the board");

        harness::frame(&mut app, |a, ui| a.resolve_conflict(ui, Side::Local)).unwrap();
        assert_eq!(disk_value(&project, "alice"), "still working on her");
        drop(td);
    }

    #[test]
    fn an_external_delete_is_detected_and_never_resurrected() {
        let (td, project, mut app) = live_app();
        let a = graph::add(&mut app, EntityKind::Person, "alice").unwrap();
        let b = graph::add(&mut app, EntityKind::Person, "bob").unwrap();
        graph::link(&mut app, a, b, RelationKind::AssociateOf).unwrap();
        settle(&mut app, 0.0);

        let mut cf = read_case(&project).unwrap();
        cf.data.entities.retain(|e| e.slug != "bob");
        cf.data.relations.clear();
        write_case(&project, &cf).unwrap();
        harness::watch(&mut app).unwrap();

        assert!(graph::node_by_slug(&app, "bob").unwrap().is_none(), "the delete reached the board");
        assert_eq!(graph::edge_count(&app).unwrap(), 0, "with its edge");
        graph::add(&mut app, EntityKind::Person, "carol").unwrap();
        settle(&mut app, 50.0);
        assert!(!project.entity_dir("bob").unwrap().exists(), "a later save never brings it back");
        assert!(project.entity_head_path("carol").unwrap().exists());
        invariant::check_graph(&app).unwrap();
        drop(td);
    }

    #[test]
    fn a_gui_delete_removes_only_what_it_deleted() {
        let (td, project, mut app) = live_app();
        graph::add(&mut app, EntityKind::Person, "alice").unwrap();
        let b = graph::add(&mut app, EntityKind::Person, "bob").unwrap();
        settle(&mut app, 0.0);

        app.g.remove_node(b);
        let planted = shell_add(&project, "Planted Org", "");
        settle(&mut app, 50.0);
        assert!(project.entity_dir("bob").unwrap().exists(), "the flush waits for the watcher first");
        harness::watch(&mut app).unwrap();
        settle(&mut app, 60.0);

        assert!(!project.entity_dir("bob").unwrap().exists(), "the gui delete landed");
        assert!(project.entity_head_path(&planted).unwrap().exists(), "the entity it never loaded was not pruned");
        let leftovers: Vec<String> = std::fs::read_dir(project.root.join("entities"))
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .filter(|n| n.starts_with(".rsgone-") || n.ends_with(".rstmp"))
            .collect();
        assert!(leftovers.is_empty(), "no hidden staging left behind: {leftovers:?}");
        drop(td);
    }

    #[test]
    fn a_half_written_file_is_never_absorbed() {
        let (td, project, mut app) = live_app();
        let a = graph::add(&mut app, EntityKind::Person, "alice").unwrap();
        let b = graph::add(&mut app, EntityKind::Person, "bob").unwrap();
        graph::link(&mut app, a, b, RelationKind::AssociateOf).unwrap();
        settle(&mut app, 0.0);
        let full = std::fs::read_to_string(project.relations_path().unwrap()).unwrap();

        std::fs::write(project.relations_path().unwrap(), "").unwrap();
        harness::poll(&mut app).unwrap();
        assert_eq!(graph::edge_count(&app).unwrap(), 1, "a change seen once is not trusted yet");
        std::fs::write(project.relations_path().unwrap(), &full).unwrap();
        harness::poll(&mut app).unwrap();
        harness::poll(&mut app).unwrap();
        assert_eq!(graph::edge_count(&app).unwrap(), 1, "the transient truncation never reached the board");

        std::fs::write(project.relations_path().unwrap(), &full[..full.len() / 2]).unwrap();
        harness::watch(&mut app).unwrap();
        assert!(app.status.contains("unreadable"), "a broken file is reported: {}", app.status);
        assert_eq!(graph::edge_count(&app).unwrap(), 1, "and nothing is dropped from the board");
        graph::add(&mut app, EntityKind::Person, "carol").unwrap();
        settle(&mut app, 50.0);
        assert_eq!(
            std::fs::read_to_string(project.relations_path().unwrap()).unwrap(),
            &full[..full.len() / 2],
            "no save lands on top of a file the app could not read"
        );
        std::fs::write(project.relations_path().unwrap(), &full).unwrap();
        harness::watch(&mut app).unwrap();
        settle(&mut app, 60.0);
        assert!(project.entity_head_path("carol").unwrap().exists(), "once readable, the held edit lands");
        assert_eq!(read_case(&project).unwrap().data.relations.len(), 1);
        drop(td);
    }

    #[test]
    fn a_held_case_lock_defers_writes_and_reads_until_released() {
        let (td, project, mut app) = live_app();
        graph::add(&mut app, EntityKind::Person, "alice").unwrap();
        let Claim::Held(lock) = try_lock(&project).unwrap() else {
            panic!("an idle case must lock");
        };
        settle(&mut app, 0.0);
        assert!(!project.entity_dir("alice").unwrap().exists(), "another writer holds the case");
        assert!(app.status.contains("busy"), "and the app says so: {}", app.status);
        drop(lock);
        settle(&mut app, 10.0);
        assert!(project.entity_head_path("alice").unwrap().exists(), "released, the write lands");
        drop(td);
    }

    #[test]
    fn a_failed_multi_file_save_leaves_a_valid_case_and_retries() {
        let (td, project, mut app) = live_app();
        let a = graph::add(&mut app, EntityKind::Person, "alice").unwrap();
        settle(&mut app, 0.0);
        let c = graph::add(&mut app, EntityKind::Person, "carol").unwrap();
        graph::link(&mut app, a, c, RelationKind::AssociateOf).unwrap();

        let root = project.root.clone();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o555)).unwrap();
        if std::fs::write(root.join("probe"), "").is_ok() {
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o755)).unwrap();
            return;
        }
        harness::tick(&mut app, 50.0).unwrap();
        let failed = harness::tick(&mut app, 50.0 + SETTLED);
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o755)).unwrap();

        assert!(failed.is_err(), "a write that cannot land is reported, never swallowed");
        assert!(project.entity_head_path("carol").unwrap().exists(), "files are atomic one by one: carol landed");
        assert!(
            read_case(&project).unwrap().data.relations.is_empty(),
            "the relations file did not — a multi-file save is not one transaction"
        );
        invariant::check_case_file(&read_case(&project).unwrap()).unwrap();
        let temps: Vec<String> = std::fs::read_dir(&root)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .filter(|n| n.ends_with(".rstmp"))
            .collect();
        assert!(temps.is_empty(), "no temp survives the failure: {temps:?}");
        assert!(app.dirty().unwrap(), "the unsaved link is still owed");

        harness::watch(&mut app).unwrap();
        settle(&mut app, 70.0);
        assert_eq!(read_case(&project).unwrap().data.relations.len(), 1, "the retry completes the save");
        drop(td);
    }

    #[test]
    fn an_unpositioned_arrival_lands_in_free_space_not_on_another_node() {
        let (td, project, mut app) = live_app();
        let tp = graph::add(&mut app, EntityKind::Organization, "turning point").unwrap();
        app.g.node_mut(tp).unwrap().set_location(Pos2::new(40.0, 0.0));
        settle(&mut app, 0.0);

        let erika = shell_add(&project, "Erika", "");
        harness::watch(&mut app).unwrap();
        let p = app.g.node(node(&app, &erika)).unwrap().location();
        assert!(
            p.distance(Pos2::new(40.0, 0.0)) >= 64.0,
            "the newcomer must not stack on an existing node: {p:?}"
        );
        assert_eq!(
            app.g.node(node(&app, "turning_point")).unwrap().location(),
            Pos2::new(40.0, 0.0),
            "existing positions never move in free layout"
        );

        harness::load(&mut app).unwrap();
        let reloaded = app.g.node(node(&app, &erika)).unwrap().location();
        assert!(reloaded.distance(Pos2::new(40.0, 0.0)) >= 64.0, "a cold load heals into free space too");
        drop(td);
    }

    #[test]
    fn the_first_board_frame_keeps_saved_positions() {
        let (td, _project, mut app) = live_app();
        let a = graph::add(&mut app, EntityKind::Person, "alice").unwrap();
        app.g.node_mut(a).unwrap().set_location(Pos2::new(-420.0, 137.5));
        settle(&mut app, 0.0);
        harness::load(&mut app).unwrap();
        harness::frame(&mut app, |a, ui| a.board(ui)).unwrap();
        assert_eq!(
            app.g.node(node(&app, "alice")).unwrap().location(),
            Pos2::new(-420.0, 137.5),
            "the board's graph view must never re-layout saved positions on a fresh frame"
        );
        drop(td);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant, SystemTime};

    use egui::Pos2;
    use petgraph::graph::NodeIndex;

    use crate::model::{Camera, EntityKind, Origin, RelationKind, Verification};
    use crate::project::{Project, read_case, write_case};
    use crate::ui::RedstringApp;
    use crate::ui::board::{camera_read, camera_write};
    use crate::ui::layout::LayoutKind;
    use crate::ui::mode::{DEBOUNCE_SECS, IDLE_SECS, Mode};
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

    fn tree(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
        let mut out = BTreeMap::new();
        walk(root, root, &mut out);
        out
    }

    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if entry.file_type().unwrap().is_dir() {
                walk(root, &path, out);
                continue;
            }
            let rel = path.strip_prefix(root).unwrap().to_path_buf();
            out.insert(rel, std::fs::read(&path).unwrap());
        }
    }

    fn loc(app: &RedstringApp, idx: NodeIndex) -> (f32, f32) {
        let p = app.g.node(idx).unwrap().location();
        (p.x, p.y)
    }

    fn place(app: &mut RedstringApp, idx: NodeIndex, x: f32, y: f32) {
        app.g.node_mut(idx).unwrap().set_location(Pos2::new(x, y));
    }

    fn stale_watcher(app: &mut RedstringApp) {
        let (path, _) = app.watcher.baseline.clone().unwrap();
        app.watcher.baseline = Some((path, SystemTime::now() - Duration::from_secs(5)));
        app.watcher.last_poll = Instant::now() - Duration::from_millis(1100);
    }

    fn external_add(project: &Project, label: &str) {
        let mut cf = read_case(project).unwrap();
        cf.data
            .entities
            .push(graph::entity_rec(EntityKind::Domain, label, label).unwrap());
        write_case(project, &cf).unwrap();
    }

    #[test]
    fn an_external_edit_can_no_longer_eat_the_boards_hand_arrangement() {
        let (td, project, mut app) = live_app();
        let a = graph::add(&mut app, EntityKind::Person, "alice").unwrap();
        let b = graph::add(&mut app, EntityKind::Person, "bob").unwrap();
        graph::link(&mut app, a, b, RelationKind::AssociateOf).unwrap();
        graph::observe(
            &mut app,
            a,
            "dob",
            "1971-03-04",
            Origin::Manual,
            Verification::AnalystAsserted,
        )
        .unwrap();
        place(&mut app, a, -420.0, 137.5);
        place(&mut app, b, 613.0, -88.0);
        app.size_by_degree = true;
        assert_eq!(app.current_layout, LayoutKind::Free, "hand-placed board is free");

        harness::tick(&mut app, 0.0).unwrap();
        harness::tick(&mut app, SETTLED).unwrap();
        assert!(!app.dirty().unwrap(), "the autosave settled the case onto disk");

        external_add(&project, "planted.example");
        stale_watcher(&mut app);
        harness::frame(&mut app, |app, ui| app.poll_external_edit(ui)).unwrap();

        assert_eq!(
            graph::node_count(&app).unwrap(),
            3,
            "the external edit reloaded and brought its node in"
        );
        let a2 = graph::node_by_slug(&app, "alice").unwrap().unwrap();
        let b2 = graph::node_by_slug(&app, "bob").unwrap().unwrap();
        assert_eq!(
            loc(&app, a2),
            (-420.0, 137.5),
            "the analyst's hand placement survived the reload — this is the bug the design fixes"
        );
        assert_eq!(loc(&app, b2), (613.0, -88.0), "and so did the second node's");
        assert_eq!(app.current_layout, LayoutKind::Free, "the layout choice survived");
        assert!(app.size_by_degree, "the board's display policy survived");
        let planted = graph::node_by_slug(&app, "planted_example").unwrap().unwrap();
        for (idx, _, _) in app.layout_anim.iter().flat_map(|anim| anim.steps.iter()) {
            assert_eq!(
                *idx, planted,
                "a free board never re-arranges itself when nodes arrive: only the newcomer animates in"
            );
        }
        let dob = graph::observation(&app, a2, "dob").unwrap().unwrap();
        assert_eq!(dob.value.text().unwrap(), "1971-03-04", "in-memory facts survived");
        assert!(
            graph::has_edge(&app, a2, b2, RelationKind::AssociateOf).unwrap(),
            "the red string survived"
        );
        invariant::check_graph(&app).unwrap();
        drop(td);
    }

    #[test]
    fn a_live_mutation_lands_on_disk_after_the_debounce_and_not_before() {
        let (td, project, mut app) = live_app();
        graph::add(&mut app, EntityKind::Person, "late arrival").unwrap();
        let dir = project.entity_dir("late_arrival").unwrap();

        harness::tick(&mut app, 100.0).unwrap();
        assert!(!dir.exists(), "the first dirty frame only arms the debounce, it never writes");
        harness::tick(&mut app, 100.0 + DEBOUNCE_SECS - 0.2).unwrap();
        assert!(!dir.exists(), "still inside the debounce window — nothing is written");
        assert!(app.autosave.pending_since.is_some(), "the write is still pending");

        harness::tick(&mut app, 100.0 + SETTLED).unwrap();
        assert!(dir.exists(), "the settled edit is flushed to disk");
        assert!(app.autosave.pending_since.is_none(), "the pending write is discharged");
        assert!(!app.dirty().unwrap(), "memory and disk have converged");

        let cf = read_case(&Project::open(&project.root).unwrap()).unwrap();
        assert_eq!(cf.data.entities.len(), 1, "the entity is really on disk");
        drop(td);
    }

    #[test]
    fn a_still_moving_drag_keeps_resetting_the_debounce_instead_of_writing_every_frame() {
        let (td, project, mut app) = live_app();
        let idx = graph::add(&mut app, EntityKind::Person, "dragged").unwrap();
        let dir = project.entity_dir("dragged").unwrap();

        let mut t = 0.0;
        for step in 0..10 {
            place(&mut app, idx, step as f32 * 10.0, 0.0);
            t += 1.0;
            harness::tick(&mut app, t).unwrap();
            assert!(
                !dir.exists(),
                "a drag mutating every frame must never write mid-gesture (frame {step})"
            );
        }
        harness::tick(&mut app, t + SETTLED).unwrap();
        assert!(dir.exists(), "the settled state is what reaches disk");
        let cf = read_case(&Project::open(&project.root).unwrap()).unwrap();
        assert_eq!(
            cf.display.positions.get("dragged").copied(),
            Some((90.0, 0.0)),
            "only the final resting position was written"
        );
        drop(td);
    }

    #[test]
    fn a_camera_only_change_never_triggers_an_autosave() {
        let (td, project, mut app) = live_app();
        graph::add(&mut app, EntityKind::Person, "anchor").unwrap();
        harness::tick(&mut app, 0.0).unwrap();
        harness::tick(&mut app, SETTLED).unwrap();
        let settled = tree(&project.root);

        let ctx = egui::Context::default();
        for step in 0..4 {
            let input = egui::RawInput {
                time: Some(SETTLED + 1.0 + step as f64),
                ..Default::default()
            };
            let _out = ctx.run_ui(input, |ui| {
                camera_write(
                    ui,
                    Camera {
                        x: 500.0 + step as f32,
                        y: -300.0,
                        zoom: 2.5,
                    },
                )
                .unwrap();
                app.autosave_tick(ui).unwrap();
            });
        }

        assert!(
            app.autosave.pending_since.is_none(),
            "panning and zooming never arms the fast write path"
        );
        assert_eq!(
            tree(&project.root),
            settled,
            "a camera-only change must not rewrite a single byte on the fast path"
        );
        drop(td);
    }

    #[test]
    fn the_idle_beat_eventually_writes_a_moved_camera() {
        let (td, project, mut app) = live_app();
        graph::add(&mut app, EntityKind::Person, "anchor").unwrap();
        harness::tick(&mut app, 0.0).unwrap();
        harness::tick(&mut app, SETTLED).unwrap();

        let ctx = egui::Context::default();
        for t in [SETTLED + 1.0, SETTLED + 1.0 + BEAT] {
            let input = egui::RawInput {
                time: Some(t),
                ..Default::default()
            };
            let _out = ctx.run_ui(input, |ui| {
                camera_write(
                    ui,
                    Camera {
                        x: 500.0,
                        y: -300.0,
                        zoom: 2.5,
                    },
                )
                .unwrap();
                app.autosave_tick(ui).unwrap();
            });
        }

        let cf = read_case(&Project::open(&project.root).unwrap()).unwrap();
        assert_eq!(
            cf.display.camera,
            Camera {
                x: 500.0,
                y: -300.0,
                zoom: 2.5,
            },
            "the slow idle beat is where pan/zoom finally reaches display.ron"
        );
        drop(td);
    }

    #[test]
    fn a_fast_autosave_leaves_the_derived_artifacts_alone_until_the_idle_beat() {
        let (td, project, mut app) = live_app();
        graph::add(&mut app, EntityKind::Person, "first").unwrap();
        harness::save(&mut app).unwrap();
        let json = project.root.join("case.json");
        let md = project.root.join("graph.md");
        assert!(json.exists() && md.exists(), "an explicit save derives the public artifacts");
        let json_before = std::fs::read(&json).unwrap();
        let md_before = std::fs::read(&md).unwrap();

        graph::add(&mut app, EntityKind::Person, "second").unwrap();
        harness::tick(&mut app, 0.0).unwrap();
        harness::tick(&mut app, SETTLED).unwrap();

        assert!(
            project.entity_dir("second").unwrap().join("entity.ron").exists(),
            "the fast path did write the source of truth"
        );
        assert_eq!(
            std::fs::read(&json).unwrap(),
            json_before,
            "case.json is not on the fast path"
        );
        assert_eq!(
            std::fs::read(&md).unwrap(),
            md_before,
            "graph.md is not on the fast path"
        );
        assert!(
            !project.entity_dir("second").unwrap().join("dossier.md").exists(),
            "115 dossiers must not be rewritten for one node nudge"
        );

        harness::tick(&mut app, SETTLED + BEAT).unwrap();
        assert_ne!(
            std::fs::read(&json).unwrap(),
            json_before,
            "the idle beat is where the derived artifacts catch up"
        );
        assert!(
            project.entity_dir("second").unwrap().join("dossier.md").exists(),
            "the idle beat regenerates the dossiers"
        );
        drop(td);
    }

    #[test]
    fn explore_writes_absolutely_nothing_no_matter_how_hard_you_edit() {
        let (td, project, mut app) = live_app();
        let keep = graph::add(&mut app, EntityKind::Person, "on the record").unwrap();
        harness::tick(&mut app, 0.0).unwrap();
        harness::tick(&mut app, SETTLED).unwrap();
        let sealed_disk = tree(&project.root);

        app.enter_explore().unwrap();
        assert!(!app.live().unwrap(), "explore suspends every write");

        let theory = graph::add(&mut app, EntityKind::Organization, "wild theory").unwrap();
        let other = graph::add(&mut app, EntityKind::Domain, "hunch.example").unwrap();
        graph::link(&mut app, keep, theory, RelationKind::MemberOf).unwrap();
        graph::observe(
            &mut app,
            theory,
            "registrar",
            "guesswork",
            Origin::Manual,
            Verification::Unreviewed,
        )
        .unwrap();
        place(&mut app, theory, 999.0, -999.0);
        app.current_layout = LayoutKind::Radial;
        app.case_name = "renamed inside the sandbox".to_owned();
        app.g.remove_node(other);

        let mut t = 0.0;
        for _ in 0..30 {
            t += BEAT;
            harness::tick(&mut app, t).unwrap();
        }

        assert_eq!(
            tree(&project.root),
            sealed_disk,
            "explore is a pure sandbox — not one byte of the case directory may change"
        );
        assert_eq!(graph::node_count(&app).unwrap(), 2, "the sandbox itself kept the edits");
        invariant::check_graph(&app).unwrap();
        drop(td);
    }

    #[test]
    fn explore_discard_restores_exactly_what_is_on_disk() {
        let (td, project, mut app) = live_app();
        graph::add(&mut app, EntityKind::Person, "on the record").unwrap();
        harness::tick(&mut app, 0.0).unwrap();
        harness::tick(&mut app, SETTLED).unwrap();
        let disk = tree(&project.root);

        app.enter_explore().unwrap();
        graph::add(&mut app, EntityKind::Organization, "wild theory").unwrap();
        harness::tick(&mut app, BEAT).unwrap();
        assert_eq!(graph::node_count(&app).unwrap(), 2);

        harness::frame(&mut app, |app, ui| app.explore_discard(ui)).unwrap();

        assert_eq!(app.mode, Mode::Live, "discarding returns to live");
        assert!(app.live().unwrap(), "and autosaving resumes");
        assert_eq!(graph::node_count(&app).unwrap(), 1, "the sandbox is gone");
        assert!(
            graph::node_by_slug(&app, "wild_theory").unwrap().is_none(),
            "the hypothetical entity never existed"
        );
        assert_eq!(tree(&project.root), disk, "discarding wrote nothing either");
        assert!(!app.dirty().unwrap(), "memory matches disk again");
        invariant::check_graph(&app).unwrap();
        drop(td);
    }

    #[test]
    fn explore_save_writes_the_sandbox_and_returns_to_live() {
        let (td, project, mut app) = live_app();
        graph::add(&mut app, EntityKind::Person, "on the record").unwrap();
        harness::tick(&mut app, 0.0).unwrap();
        harness::tick(&mut app, SETTLED).unwrap();

        app.enter_explore().unwrap();
        graph::add(&mut app, EntityKind::Organization, "proven theory").unwrap();
        harness::tick(&mut app, BEAT).unwrap();
        assert!(
            !project.entity_dir("proven_theory").unwrap().exists(),
            "still nothing on disk while exploring"
        );

        harness::frame(&mut app, |app, ui| app.explore_save(ui)).unwrap();

        assert_eq!(app.mode, Mode::Live, "saving returns to live");
        assert!(
            project.entity_dir("proven_theory").unwrap().join("entity.ron").exists(),
            "the sandbox is written on the way out"
        );
        assert!(
            project.root.join("case.json").exists() && project.root.join("graph.md").exists(),
            "leaving explore regenerates the derived artifacts"
        );
        assert!(!app.dirty().unwrap(), "the case is settled");
        let cf = read_case(&Project::open(&project.root).unwrap()).unwrap();
        assert_eq!(cf.data.entities.len(), 2);
        drop(td);
    }

    #[test]
    fn explore_suspends_the_watcher_and_flags_that_disk_moved() {
        let (td, project, mut app) = live_app();
        graph::add(&mut app, EntityKind::Person, "on the record").unwrap();
        harness::tick(&mut app, 0.0).unwrap();
        harness::tick(&mut app, SETTLED).unwrap();

        app.enter_explore().unwrap();
        let theory = graph::add(&mut app, EntityKind::Organization, "wild theory").unwrap();
        place(&mut app, theory, 314.0, 271.0);

        external_add(&project, "planted.example");
        stale_watcher(&mut app);
        harness::frame(&mut app, |app, ui| app.poll_external_edit(ui)).unwrap();

        assert_eq!(
            graph::node_count(&app).unwrap(),
            2,
            "the watcher must not reload over a sandbox"
        );
        assert_eq!(loc(&app, theory), (314.0, 271.0), "the sandbox is untouched");
        assert!(app.autosave.disk_moved, "but the app knows disk moved under it");

        harness::frame(&mut app, |app, ui| app.explore_save(ui)).unwrap();
        assert!(
            app.autosave.conflict,
            "saving over someone else's edit must ask first, never silently overwrite"
        );
        assert_eq!(app.mode, Mode::Explore, "and it stays in explore until answered");
        assert!(
            !project.entity_dir("wild_theory").unwrap().exists(),
            "nothing is written while the conflict is open"
        );
        invariant::check_graph(&app).unwrap();
        drop(td);
    }

    #[test]
    fn a_conflicted_explore_can_still_take_the_other_side() {
        let (td, project, mut app) = live_app();
        graph::add(&mut app, EntityKind::Person, "on the record").unwrap();
        harness::tick(&mut app, 0.0).unwrap();
        harness::tick(&mut app, SETTLED).unwrap();

        app.enter_explore().unwrap();
        graph::add(&mut app, EntityKind::Organization, "wild theory").unwrap();
        external_add(&project, "planted.example");
        stale_watcher(&mut app);
        harness::frame(&mut app, |app, ui| app.poll_external_edit(ui)).unwrap();
        harness::frame(&mut app, |app, ui| app.explore_save(ui)).unwrap();
        assert!(app.autosave.conflict);

        harness::frame(&mut app, |app, ui| app.explore_discard(ui)).unwrap();
        assert!(!app.autosave.conflict, "the conflict is resolved");
        assert_eq!(app.mode, Mode::Live);
        assert!(
            graph::node_by_slug(&app, "planted_example").unwrap().is_some(),
            "taking their side loads the external edit"
        );
        assert!(
            graph::node_by_slug(&app, "wild_theory").unwrap().is_none(),
            "and drops the sandbox"
        );
        drop(td);
    }

    #[test]
    fn no_project_means_explore_no_matter_what_the_mode_field_says() {
        let mut app = harness::app().unwrap();
        assert_eq!(app.mode, Mode::Live, "the stored intent is live");
        assert!(
            !app.live().unwrap(),
            "with nowhere to autosave the app is really exploring"
        );
        graph::add(&mut app, EntityKind::Person, "scratch").unwrap();
        harness::tick(&mut app, 0.0).unwrap();
        harness::tick(&mut app, BEAT).unwrap();
        assert!(!app.dirty().unwrap(), "a case with no directory is never dirty");
        invariant::check_graph(&app).unwrap();
    }

    #[test]
    fn the_quit_guard_holds_a_sandbox_and_waves_a_live_case_through() {
        let (td, project, mut app) = live_app();
        graph::add(&mut app, EntityKind::Person, "on the record").unwrap();
        harness::tick(&mut app, 0.0).unwrap();
        harness::tick(&mut app, SETTLED).unwrap();

        graph::add(&mut app, EntityKind::Person, "unflushed").unwrap();
        assert!(app.dirty().unwrap(), "a pending fast write is still dirty");
        harness::frame(&mut app, |app, ui| app.request_quit(ui)).unwrap();
        assert!(app.allow_close, "in live the pending write is flushed and quit proceeds");
        assert!(!app.quit_confirm, "live never nags about unsaved changes");
        assert!(
            project.entity_dir("unflushed").unwrap().exists(),
            "quitting flushes what the debounce had not written yet"
        );

        app.allow_close = false;
        app.enter_explore().unwrap();
        graph::add(&mut app, EntityKind::Person, "sandbox only").unwrap();
        harness::frame(&mut app, |app, ui| app.request_quit(ui)).unwrap();
        assert!(app.quit_confirm, "an unsaved sandbox stops the quit");
        assert!(!app.allow_close, "and the window stays open");
        drop(td);
    }

    #[test]
    fn autosave_rebaselines_the_watcher_so_it_never_reloads_its_own_write() {
        let (td, _project, mut app) = live_app();
        graph::add(&mut app, EntityKind::Person, "self written").unwrap();
        let (_, before) = app.watcher.baseline.clone().unwrap();

        harness::tick(&mut app, 0.0).unwrap();
        harness::tick(&mut app, SETTLED).unwrap();
        let (_, after) = app.watcher.baseline.clone().unwrap();
        assert!(after >= before, "the autosave re-baselined the watcher over its own write");

        app.watcher.last_poll = Instant::now() - Duration::from_millis(1100);
        harness::frame(&mut app, |app, ui| app.poll_external_edit(ui)).unwrap();
        assert_eq!(
            graph::node_count(&app).unwrap(),
            1,
            "the app must never read its own autosave back as an external edit"
        );
        assert!(
            !app.status.contains("external edit"),
            "and must not claim one happened: {}",
            app.status
        );
        drop(td);
    }

    #[test]
    fn the_camera_baseline_starts_where_the_case_left_it() {
        let (td, _project, mut app) = live_app();
        let read = harness::frame(&mut app, |_app, ui| camera_read(ui)).unwrap();
        assert_eq!(
            app.autosave.last_camera, read,
            "a fresh load has nothing to write — the camera baseline matches what the board holds"
        );
        drop(td);
    }
}

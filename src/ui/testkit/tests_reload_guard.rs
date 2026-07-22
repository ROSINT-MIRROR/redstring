#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{Duration, Instant, SystemTime};

    use egui::Pos2;

    use crate::model::{EntityKind, RelationKind};
    use crate::modules::{Fact, ModuleRun};
    use crate::ui::RedstringApp;
    use crate::ui::graphalg;
    use crate::ui::testkit::{apply, fixtures, graph, harness, invariant};

    fn module_run(
        run_id: u64,
        target_slug: &str,
        project_root: Option<PathBuf>,
        evidence_rel: &str,
        shape_id: String,
        facts: Vec<Fact>,
        raw: String,
    ) -> ModuleRun {
        ModuleRun {
            run_id,
            module_id: "dnsx".to_owned(),
            module_label: "dnsx".to_owned(),
            target_slug: target_slug.to_owned(),
            target_label: target_slug.to_owned(),
            project_root,
            shape_id,
            facts,
            raw,
            stderr_tail: String::new(),
            ok: true,
            cancelled: false,
            evidence_rel: evidence_rel.to_owned(),
            last: true,
        }
    }

    fn deliver(app: &mut RedstringApp, run: ModuleRun) {
        app.next_run_id += 1;
        app.module_tx.send(run).unwrap();
        harness::frame(app, |a, ui| a.poll_module_runs(ui)).unwrap();
    }

    #[test]
    fn linked_only_lookup_never_reshuffles_the_board() {
        let mut app = harness::app().unwrap();
        app.relayout_on_add = true;
        let dom = graph::add_valued(
            &mut app,
            EntityKind::Domain,
            "evil-corp.example",
            "evil-corp.example",
        )
        .unwrap();
        let ip = graph::add_valued(&mut app, EntityKind::IpAddress, "203.0.113.10", "203.0.113.10")
            .unwrap();
        let facts = vec![Fact::Entity {
            kind: EntityKind::IpAddress,
            label: "203.0.113.10".to_owned(),
            value: "203.0.113.10".to_owned(),
            rel: RelationKind::ResolvesTo,
            outbound: true,
            attrs: Vec::new(),
        }];
        let slug = graph::slug_of(&app, dom).unwrap();
        let run = module_run(
            app.next_run_id,
            &slug,
            None,
            "",
            "dnsx".to_owned(),
            facts,
            "203.0.113.10 A\n".to_owned(),
        );
        deliver(&mut app, run);

        assert_eq!(
            graph::node_count(&app).unwrap(),
            2,
            "the emitted IP converges onto the existing node — zero nodes spawn"
        );
        assert!(
            graph::has_edge(&app, dom, ip, RelationKind::ResolvesTo).unwrap(),
            "the red string between the converged nodes still lands"
        );
        assert!(
            app.layout_anim.is_none(),
            "a linked-only lookup adds no nodes — relayout-on-add must leave the hand-placed board alone"
        );
        invariant::check_graph(&app).unwrap();
    }

    #[test]
    fn cross_case_lookup_archives_into_its_launch_case_and_leaves_the_open_case_alone() {
        let (td_a, project_a) = harness::temp_project().unwrap();
        let (td_b, project_b) = harness::temp_project().unwrap();
        let root_a = project_a.root.clone();
        let root_b = project_b.root.clone();
        let mut app = harness::app().unwrap();
        app.project = Some(project_b);
        harness::load(&mut app).unwrap();
        let dom = graph::add_valued(
            &mut app,
            EntityKind::Domain,
            "evil-corp.example",
            "evil-corp.example",
        )
        .unwrap();
        harness::save(&mut app).unwrap();
        let (watched_path, _) = app.watched.clone().unwrap();
        let sentinel = (watched_path, SystemTime::now() - Duration::from_secs(5));
        app.watched = Some(sentinel.clone());

        let raw = fixtures::fixture("dnsx.jsonl").unwrap();
        let ron = fixtures::module_ron("dnsx").unwrap();
        let (shape_id, facts) = apply::parse(&ron, &raw).unwrap();
        let slug = graph::slug_of(&app, dom).unwrap();
        let run = module_run(
            app.next_run_id,
            &slug,
            Some(root_a.clone()),
            "evidence/docs/dnsx-crosscase-test.txt",
            shape_id,
            facts,
            raw,
        );
        deliver(&mut app, run);

        assert_eq!(
            graph::node_count(&app).unwrap(),
            1,
            "foreign facts never land on the open case, even when a slug collides"
        );
        assert_eq!(graph::edge_count(&app).unwrap(), 0, "no foreign edge with a dangling citation");
        assert!(
            root_a.join("evidence/docs/dnsx-crosscase-test.txt").exists(),
            "raw stdout is archived into the launch-pinned case"
        );
        assert!(
            !root_b.join("evidence/docs/dnsx-crosscase-test.txt").exists(),
            "nothing is written into the case that did not launch the run"
        );
        assert_eq!(
            app.watched,
            Some(sentinel),
            "the open case's watch baseline is untouched — a pending external edit is not swallowed"
        );
        assert!(app.status.contains("another case"), "status: {}", app.status);
        invariant::check_graph(&app).unwrap();
        drop(td_a);
        drop(td_b);
    }

    #[test]
    fn slug_missed_lookup_still_archives_its_raw_output() {
        let (td, project) = harness::temp_project().unwrap();
        let root = project.root.clone();
        let mut app = harness::app().unwrap();
        app.project = Some(project);
        harness::load(&mut app).unwrap();
        graph::add_valued(&mut app, EntityKind::Domain, "kept.example", "kept.example").unwrap();
        harness::save(&mut app).unwrap();
        let (watched_path, _) = app.watched.clone().unwrap();
        let stale = SystemTime::now() - Duration::from_secs(5);
        app.watched = Some((watched_path, stale));

        let raw = fixtures::fixture("dnsx.jsonl").unwrap();
        let ron = fixtures::module_ron("dnsx").unwrap();
        let (shape_id, facts) = apply::parse(&ron, &raw).unwrap();
        let run = module_run(
            app.next_run_id,
            "deleted_meanwhile",
            Some(root.clone()),
            "evidence/docs/dnsx-slugmiss-test.txt",
            shape_id,
            facts,
            raw,
        );
        deliver(&mut app, run);

        assert!(
            root.join("evidence/docs/dnsx-slugmiss-test.txt").exists(),
            "evidence survives even when the target left the board"
        );
        assert_eq!(graph::node_count(&app).unwrap(), 1, "nothing is applied without its target");
        assert!(app.status.contains("no longer on the board"), "status: {}", app.status);
        let (_, seen) = app.watched.clone().unwrap();
        assert!(
            seen > stale,
            "the watcher re-baselines over its own evidence write so the archive does not read back as an external edit"
        );
        invariant::check_graph(&app).unwrap();
        drop(td);
    }

    #[test]
    fn reload_drops_every_stale_node_index_held_by_ui_state() {
        let (td, project) = harness::temp_project().unwrap();
        let mut app = harness::app().unwrap();
        app.project = Some(project);
        harness::load(&mut app).unwrap();
        let a = graph::add(&mut app, EntityKind::Person, "alice").unwrap();
        let b = graph::add(&mut app, EntityKind::Person, "bob").unwrap();
        graph::link(&mut app, a, b, RelationKind::AssociateOf).unwrap();
        harness::save(&mut app).unwrap();

        app.delete_confirm = Some(vec![a, b]);
        app.seal_confirm = Some((vec![a], true));
        app.merge_confirm = Some((a, b));
        app.label_edit = Some(a);
        app.node_menu = Some((a, Pos2::ZERO));
        app.node_menu_just_opened = true;
        app.right_press = Some((Some(a), Pos2::ZERO));
        app.band = Some(Pos2::ZERO);
        app.active_path = graphalg::path_between(&app.g, a, b).unwrap();
        assert!(app.active_path.is_some(), "path trail armed before the reload");

        harness::load(&mut app).unwrap();

        assert!(
            app.delete_confirm.is_none(),
            "a reload remaps node indices — confirming a stale delete would remove the wrong nodes"
        );
        assert!(app.seal_confirm.is_none(), "stale seal confirm dropped");
        assert!(app.merge_confirm.is_none(), "stale merge confirm dropped");
        assert!(app.label_edit.is_none(), "stale label editor dropped");
        assert!(app.node_menu.is_none(), "stale fold menu dropped");
        assert!(!app.node_menu_just_opened, "menu bookkeeping reset");
        assert!(app.right_press.is_none(), "stale right-press anchor dropped");
        assert!(app.band.is_none(), "stale rubber-band dropped");
        assert!(
            app.active_path.is_none(),
            "the path trail retires with the graph it indexed"
        );
        invariant::check_graph(&app).unwrap();
        drop(td);
    }

    #[test]
    fn hot_reload_relayout_respects_fold_state_and_keeps_the_reload_notice() {
        let (td, project) = harness::temp_project().unwrap();
        let mut app = harness::app().unwrap();
        app.project = Some(project.clone());
        harness::load(&mut app).unwrap();
        let hetzner = graph::add(&mut app, EntityKind::Organization, "hetzner").unwrap();
        let dash = graph::add(&mut app, EntityKind::Person, "dash").unwrap();
        let ip = graph::add_valued(&mut app, EntityKind::IpAddress, "203.0.113.9", "203.0.113.9")
            .unwrap();
        graph::link(&mut app, dash, ip, RelationKind::ResolvesTo).unwrap();
        graph::link(&mut app, hetzner, ip, RelationKind::ResolvesTo).unwrap();
        app.toggle_collapse(ip).unwrap();
        harness::save(&mut app).unwrap();
        app.relayout_on_add = true;
        app.layout_anim = None;

        let mut cf = crate::project::read_case(&project).unwrap();
        cf.data
            .entities
            .push(graph::entity_rec(EntityKind::Domain, "new.example", "new.example").unwrap());
        crate::project::write_case(&project, &cf).unwrap();

        let (path, _) = app.watched.clone().unwrap();
        app.watched = Some((path, SystemTime::now() - Duration::from_secs(5)));
        app.last_watch_poll = Instant::now() - Duration::from_millis(1100);
        harness::frame(&mut app, |a, ui| a.poll_external_edit(ui)).unwrap();

        assert_eq!(graph::node_count(&app).unwrap(), 4, "the external edit reloaded");
        assert!(
            app.status.contains("external edit detected"),
            "the auto-relayout must not clobber the reload notice — status: {}",
            app.status
        );
        assert_eq!(
            graph::hidden_slugs(&app).unwrap(),
            vec!["dash".to_string(), "hetzner".to_string()],
            "fold state is re-derived on the fresh graph before the relayout"
        );
        let steps = app.layout_anim.as_ref().expect("growth relayout armed").steps.clone();
        let mut moved: Vec<String> = Vec::new();
        for (idx, _, _) in steps {
            moved.push(graph::slug_of(&app, idx).unwrap());
        }
        moved.sort();
        assert_eq!(
            moved,
            vec!["203_0_113_9".to_string(), "new_example".to_string()],
            "only visible nodes ride the relayout — folded subtrees stay put"
        );
        invariant::check_graph(&app).unwrap();
        drop(td);
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use egui::{Pos2, Rect, Vec2};
    use petgraph::graph::NodeIndex;

    use crate::error::Error;
    use crate::model::{Camera, EntityKind, RelationKind};
    use crate::project::{read_case, write_case};
    use crate::ui::RedstringApp;
    use crate::ui::arrivals::CameraEase;
    use crate::ui::board::{camera_read, camera_write};
    use crate::ui::layout::LayoutKind;
    use crate::ui::testkit::{graph, harness};

    const BOARD: Vec2 = Vec2::new(1000.0, 700.0);

    struct Rig {
        ctx: egui::Context,
        app: RedstringApp,
    }

    impl Rig {
        fn new(layout: LayoutKind) -> Rig {
            let mut app = harness::app().unwrap();
            app.board_size = BOARD;
            app.current_layout = layout;
            Rig { ctx: egui::Context::default(), app }
        }

        fn frame<R>(
            &mut self,
            t: f64,
            mut body: impl FnMut(&mut RedstringApp, &mut egui::Ui) -> Result<R, Error>,
        ) -> R {
            let mut input = egui::RawInput { time: Some(t), ..Default::default() };
            input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, BOARD));
            let app = &mut self.app;
            let mut slot = None;
            let _full = self.ctx.run_ui(input, |ui| slot = Some(body(app, ui)));
            slot.expect("the frame ran").expect("the frame body succeeded")
        }

        fn node_at(&mut self, label: &str, at: Pos2) -> NodeIndex {
            let idx = graph::add(&mut self.app, EntityKind::Person, label).unwrap();
            self.app.g.node_mut(idx).unwrap().set_location(at);
            idx
        }

        fn camera(&mut self, t: f64) -> Camera {
            self.frame(t, |_a, ui| camera_read(ui))
        }

        fn look(&mut self, t: f64, centre: Pos2, zoom: f32) -> Camera {
            let pan = BOARD / 2.0 - centre.to_vec2() * zoom;
            let cam = Camera { x: pan.x, y: pan.y, zoom };
            self.frame(t, |_a, ui| camera_write(ui, cam));
            cam
        }

        fn run_out(&mut self, from: f64) -> f64 {
            let mut t = from;
            for _step in 0..12 {
                t += 0.1;
                self.frame(t, |a, ui| {
                    a.layout_anim_tick(ui)?;
                    a.camera_ease_tick(ui)
                });
            }
            t
        }

        fn easing(&self) -> bool {
            let id = egui::Id::new(CameraEase::ID);
            self.ctx.data(|d| d.get_temp::<CameraEase>(id)).is_some()
        }

        fn loc(&self, idx: NodeIndex) -> Pos2 {
            self.app.g.node(idx).unwrap().location()
        }

        fn slug(&self, idx: NodeIndex) -> String {
            self.app.g.node(idx).unwrap().payload().slug.clone()
        }
    }

    #[test]
    fn a_small_arrival_that_fits_leaves_the_camera_exactly_still() {
        let mut rig = Rig::new(LayoutKind::Free);
        let a = rig.node_at("Anchor A", Pos2::new(0.0, 0.0));
        let b = rig.node_at("Anchor B", Pos2::new(150.0, 0.0));
        let before = rig.look(0.0, Pos2::new(75.0, 0.0), 1.0);
        let c = graph::add(&mut rig.app, EntityKind::Person, "Newcomer").unwrap();
        graph::link(&mut rig.app, c, a, RelationKind::AssociateOf).unwrap();
        let added = vec![rig.slug(c)];
        rig.frame(0.1, |app, ui| app.settle_arrivals(ui, &added, &[]));
        assert!(rig.app.layout_anim.is_some(), "the newcomer animates in");
        assert!(!rig.easing(), "everything fits: no camera ease is even scheduled");
        let t = rig.run_out(0.1);
        assert_eq!(rig.camera(t), before, "the camera must not move when the graph still fits");
        let (pa, pb, pc) = (rig.loc(a), rig.loc(b), rig.loc(c));
        assert_eq!((pa, pb), (Pos2::new(0.0, 0.0), Pos2::new(150.0, 0.0)), "existing nodes stay put");
        assert!(pc.distance(pa) <= 220.0, "the newcomer lands beside its neighbour, got {pc:?}");
        assert!(pc.distance(pa) >= 90.0 && pc.distance(pb) >= 90.0, "and never on top of anyone, got {pc:?}");
    }

    #[test]
    fn a_batch_outside_the_frame_zooms_out_just_enough_and_never_in() {
        let mut rig = Rig::new(LayoutKind::Free);
        let a = rig.node_at("Home", Pos2::new(0.0, 0.0));
        let far = rig.node_at("Far hub", Pos2::new(1600.0, 900.0));
        let before = rig.look(0.0, Pos2::new(0.0, 0.0), 2.0);
        let mut added = Vec::new();
        for i in 0..5 {
            let n = graph::add(&mut rig.app, EntityKind::Organization, &format!("Shell {i}")).unwrap();
            graph::link(&mut rig.app, far, n, RelationKind::Controls).unwrap();
            added.push(rig.slug(n));
        }
        rig.frame(0.1, |app, ui| app.settle_arrivals(ui, &added, &[]));
        assert!(rig.easing(), "the graph no longer fits: a camera ease starts");
        let t = rig.run_out(0.1);
        let cam = rig.camera(t);
        assert!(!rig.easing(), "the ease finishes");
        assert!(cam.zoom < before.zoom, "it zooms out, got {} from {}", cam.zoom, before.zoom);
        let lo = BOARD * (1.0 - CameraEase::MARGIN) / 2.0;
        let hi = BOARD * (1.0 + CameraEase::MARGIN) / 2.0;
        let mut span = Rect::NOTHING;
        for idx in rig.app.g.g().node_indices().collect::<Vec<_>>() {
            let s = rig.loc(idx).to_vec2() * cam.zoom + Vec2::new(cam.x, cam.y);
            span.extend_with(s.to_pos2());
            assert!(
                s.x >= lo.x - 1.0 && s.x <= hi.x + 1.0 && s.y >= lo.y - 1.0 && s.y <= hi.y + 1.0,
                "node {idx:?} at screen {s:?} must sit inside the margin box {lo:?}..{hi:?}"
            );
        }
        let used = (span.width() / (BOARD.x * CameraEase::MARGIN)).max(span.height() / (BOARD.y * CameraEase::MARGIN));
        assert!(used > 0.75, "the zoom-out is the minimum needed, not a blind reset (fill {used})");
        assert_eq!(rig.loc(a), Pos2::new(0.0, 0.0), "existing nodes never move in free layout");
    }

    #[test]
    fn a_note_only_reload_does_nothing_at_all() {
        let mut rig = Rig::new(LayoutKind::Grid);
        let a = rig.node_at("Quiet", Pos2::new(10.0, 20.0));
        let before = rig.look(0.0, Pos2::new(5000.0, 5000.0), 0.4);
        rig.app.status = "untouched".to_owned();
        rig.frame(0.1, |app, ui| app.settle_arrivals(ui, &[], &[]));
        assert!(rig.app.layout_anim.is_none(), "no animation");
        assert!(!rig.easing(), "no camera ease");
        assert_eq!(rig.camera(0.2), before, "even an off-screen graph does not pull the camera");
        assert_eq!(rig.loc(a), Pos2::new(10.0, 20.0));
        assert_eq!(rig.app.status, "untouched");
    }

    #[test]
    fn grid_keeps_its_layout_and_new_nodes_grow_out_of_their_neighbour() {
        let mut rig = Rig::new(LayoutKind::Grid);
        let a = rig.node_at("Grid A", Pos2::new(0.0, 0.0));
        rig.node_at("Grid B", Pos2::new(160.0, 0.0));
        rig.node_at("Grid C", Pos2::new(320.0, 0.0));
        rig.look(0.0, Pos2::new(160.0, 0.0), 1.0);
        let d = graph::add(&mut rig.app, EntityKind::Person, "Grid D").unwrap();
        graph::link(&mut rig.app, a, d, RelationKind::AssociateOf).unwrap();
        let added = vec![rig.slug(d)];
        rig.frame(0.1, |app, ui| app.settle_arrivals(ui, &added, &[]));
        assert_eq!(rig.app.current_layout, LayoutKind::Grid, "the selected layout is kept");
        let anim = rig.app.layout_anim.as_ref().expect("the grid re-flows with animation");
        let step = anim.steps.iter().find(|s| s.0 == d).expect("the newcomer is part of the re-flow");
        assert_eq!(step.1, Pos2::new(0.0, 0.0), "it starts at its neighbour and grows out from there");
        rig.run_out(0.1);
        let mut seen: Vec<Pos2> = rig.app.g.g().node_indices().map(|i| rig.loc(i)).collect();
        seen.sort_by(|p, q| p.x.total_cmp(&q.x).then(p.y.total_cmp(&q.y)));
        seen.dedup();
        assert_eq!(seen.len(), 4, "every node gets its own grid cell");
    }

    #[test]
    fn hot_reload_keeps_the_live_camera_and_layout_on_master() {
        let (_td, project) = harness::temp_project().unwrap();
        let mut cf = read_case(&project).unwrap();
        let author = harness::test_author().unwrap();
        cf.data.entities.push(graph::entity_rec(EntityKind::Person, "Root Person", "").unwrap());
        cf.data.entities.push(graph::entity_rec(EntityKind::Person, "Second Person", "").unwrap());
        cf.display.positions.insert("root_person".into(), (0.0, 0.0));
        cf.display.positions.insert("second_person".into(), (150.0, 0.0));
        cf.display.layout = LayoutKind::Free;
        write_case(&project, &cf).unwrap();

        let mut rig = Rig::new(LayoutKind::Free);
        rig.app.project = Some(project.clone());
        rig.frame(0.0, |app, ui| app.load(ui));
        let live = rig.look(0.1, Pos2::new(75.0, 0.0), 1.3);

        let mut cf = read_case(&project).unwrap();
        cf.data.entities.push(graph::entity_rec(EntityKind::Organization, "Arrived Org", "").unwrap());
        cf.data.relations.push(
            graph::relation_rec("arrived_org", "root_person", RelationKind::Controls, &author).unwrap(),
        );
        write_case(&project, &cf).unwrap();
        rig.app.pending_camera = None;
        for t in [0.2, 0.25] {
            rig.app.watcher.last_poll = Instant::now() - Duration::from_secs(5);
            rig.frame(t, |app, ui| app.poll_external_edit(ui));
        }

        assert_eq!(rig.app.g.g().node_count(), 3, "the external entity was absorbed");
        assert_eq!(rig.app.current_layout, LayoutKind::Free);
        assert!(rig.app.pending_camera.is_none(), "load()'s disk camera must not be re-applied");
        assert!(!rig.app.fit_on_open, "a hot reload never runs the open-time fit");
        let t = rig.run_out(0.2);
        assert_eq!(rig.camera(t), live, "everything fits, so the live camera survives the reload untouched");
        assert!(rig.app.status.contains("1 added"), "status names the arrival, got {:?}", rig.app.status);
    }
}

#[cfg(test)]
mod tests {
    use egui::Pos2;

    use crate::model::{Camera, EntityKind};
    use crate::project::{Project, read_case, write_case};
    use crate::ui::RedstringApp;
    use crate::ui::board::camera_write;
    use crate::ui::center::center_camera;
    use crate::ui::testkit::{graph, harness};

    const ROW: [(f32, f32); 3] = [(0.0, 0.0), (170.0, 0.0), (340.0, 0.0)];

    fn saved_row(camera: Camera) -> (harness::TempDir, Project) {
        let (td, project) = harness::temp_project().unwrap();
        let mut app = harness::app().unwrap();
        app.project = Some(project.clone());
        harness::load(&mut app).unwrap();
        for (i, (x, y)) in ROW.iter().enumerate() {
            let idx = graph::add(&mut app, EntityKind::Person, &format!("row {i}")).unwrap();
            app.g.node_mut(idx).unwrap().set_location(Pos2::new(*x, *y));
        }
        harness::save(&mut app).unwrap();
        let mut cf = read_case(&project).unwrap();
        cf.display.camera = camera;
        write_case(&project, &cf).unwrap();
        (td, project)
    }

    fn reopen(project: &Project) -> RedstringApp {
        let mut app = harness::app().unwrap();
        app.project = Some(Project::open(&project.root).unwrap());
        harness::load(&mut app).unwrap();
        app
    }

    fn row_positions(app: &RedstringApp) -> Vec<(f32, f32)> {
        let mut out: Vec<(f32, f32)> = app
            .g
            .nodes_iter()
            .map(|(_, n)| (n.location().x, n.location().y))
            .collect();
        out.sort_by(|a, b| a.0.total_cmp(&b.0));
        out
    }

    #[test]
    fn the_first_board_frame_keeps_saved_positions() {
        let saved = Camera {
            x: 120.0,
            y: -40.0,
            zoom: 2.0,
        };
        let (td, project) = saved_row(saved);
        let mut app = reopen(&project);
        harness::frame(&mut app, |a, ui| a.board(ui)).unwrap();
        assert_eq!(
            row_positions(&app),
            ROW.to_vec(),
            "a fresh egui memory must not let a layout scramble display.ron positions"
        );
        drop(td);
    }

    #[test]
    fn a_saved_camera_is_restored_and_not_refit() {
        let saved = Camera {
            x: 120.0,
            y: -40.0,
            zoom: 2.0,
        };
        let (td, project) = saved_row(saved);
        let mut app = reopen(&project);
        assert!(
            !app.fit_on_open,
            "a case with a saved camera opens without a fit"
        );
        harness::frame(&mut app, |a, ui| a.board(ui)).unwrap();
        assert_eq!(
            app.pending_camera,
            Some(saved),
            "the saved zoom and centre win"
        );
        drop(td);
    }

    #[test]
    fn a_case_without_a_camera_fits_its_graph_once() {
        let (td, project) = saved_row(Camera::default());
        let mut app = reopen(&project);
        assert!(app.fit_on_open, "no saved camera falls back to a fit");
        harness::frame(&mut app, |a, ui| a.board(ui)).unwrap();
        let (fit, _, _) = center_camera(&app.g, app.board_size).unwrap().unwrap();
        assert_eq!(
            app.pending_camera,
            Some(fit),
            "the fallback frames the graph"
        );
        assert!(!app.fit_on_open, "the fit runs once, never again");
        drop(td);
    }

    #[test]
    fn a_saved_case_carries_the_live_camera() {
        let (td, project) = saved_row(Camera::default());
        let mut app = reopen(&project);
        let cam = Camera {
            x: -75.0,
            y: 30.0,
            zoom: 0.6,
        };
        harness::frame(&mut app, |a, ui| {
            camera_write(ui, cam)?;
            a.save_full(ui)
        })
        .unwrap();
        let cf = read_case(&Project::open(&project.root).unwrap()).unwrap();
        assert_eq!(
            cf.display.camera, cam,
            "close-time save writes zoom and centre"
        );
        assert_eq!(
            reopen(&project).pending_camera,
            Some(cam),
            "and reopen restores them"
        );
        drop(td);
    }
}

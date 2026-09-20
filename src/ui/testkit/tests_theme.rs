#[cfg(test)]
mod tests {
    use std::time::Instant;

    use egui::Color32;

    use crate::theme::{
        Theme, ThemeWatcher, ensure_template_file, save_theme_file, write_template_force,
    };
    use crate::ui::testkit::harness;

    #[test]
    fn every_predefined_theme_applies_distinct_visuals() {
        let mut app = harness::app().unwrap();
        harness::frame(&mut app, |app, ui| {
            for t in Theme::ALL {
                if *t == Theme::Custom {
                    continue;
                }
                app.prefs.theme = *t;
                app.apply_prefs(ui.ctx()).unwrap();
                let visuals = ui.ctx().global_style().visuals.clone();
                let def = t.def().unwrap();
                assert_eq!(
                    visuals.dark_mode,
                    def.dark_mode,
                    "dark_mode must match definition for {t}"
                );
                assert_eq!(
                    visuals.panel_fill, def.panel_fill.0,
                    "panel_fill must match definition for {t}"
                );
            }
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn live_reload_absorbs_external_theme_edit_without_restart() {
        let tmp = std::env::temp_dir().join(format!(
            "redstring_test_live_reload_{}.ron",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&tmp);

        let mut app = harness::app().unwrap();
        app.theme_watcher = ThemeWatcher::new(tmp.clone()).unwrap();
        app.prefs.theme = Theme::Custom;

        let mut def = Theme::Nord.def().unwrap();
        def.panel_fill = Color32::from_rgb(16, 32, 48).into();
        save_theme_file(&tmp, &def).unwrap();

        harness::frame(&mut app, |app, ui| {
            app.theme_watcher.seen_mtime = None;
            app.theme_watcher.last_poll = Instant::now() - std::time::Duration::from_secs(2);
            app.poll_theme_reload(ui).unwrap();

            let visuals = ui.ctx().global_style().visuals.clone();
            assert_eq!(
                visuals.panel_fill,
                Color32::from_rgb(16, 32, 48),
                "custom theme colors must be applied on initial load"
            );

            def.panel_fill = Color32::from_rgb(80, 96, 112).into();
            save_theme_file(&tmp, &def).unwrap();

            app.theme_watcher.seen_mtime = None;
            app.theme_watcher.last_poll = Instant::now() - std::time::Duration::from_secs(2);
            app.poll_theme_reload(ui).unwrap();

            let visuals2 = ui.ctx().global_style().visuals.clone();
            assert_eq!(
                visuals2.panel_fill,
                Color32::from_rgb(80, 96, 112),
                "live reload must update panel_fill without app restart"
            );
            assert!(
                app.status.contains("theme reloaded"),
                "status must report hot-reload, got {}",
                app.status
            );

            Ok(())
        })
        .unwrap();

        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn malformed_theme_file_surfaces_warning_and_preserves_active_theme() {
        let tmp = std::env::temp_dir().join(format!(
            "redstring_test_malformed_{}.ron",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&tmp);

        let mut app = harness::app().unwrap();
        app.theme_watcher = ThemeWatcher::new(tmp.clone()).unwrap();
        app.prefs.theme = Theme::Custom;

        ensure_template_file(&tmp).unwrap();

        harness::frame(&mut app, |app, ui| {
            app.theme_watcher.seen_mtime = None;
            app.theme_watcher.last_poll = Instant::now() - std::time::Duration::from_secs(2);
            app.poll_theme_reload(ui).unwrap();

            let before_fill = ui.ctx().global_style().visuals.panel_fill;

            std::fs::write(&tmp, "((not valid ron").unwrap();

            app.theme_watcher.seen_mtime = None;
            app.theme_watcher.last_poll = Instant::now() - std::time::Duration::from_secs(2);
            app.poll_theme_reload(ui).unwrap();

            assert!(
                app.status.contains("theme file error"),
                "status should flag parse error, got {}",
                app.status
            );

            let after_fill = ui.ctx().global_style().visuals.panel_fill;
            assert_eq!(
                before_fill, after_fill,
                "broken theme file must preserve previous valid colors"
            );

            write_template_force(&tmp).unwrap();
            app.theme_watcher.seen_mtime = None;
            app.theme_watcher.last_poll = Instant::now() - std::time::Duration::from_secs(2);
            app.poll_theme_reload(ui).unwrap();

            assert!(
                app.status.contains("theme reloaded"),
                "fixing theme file must clear error and reload"
            );

            Ok(())
        })
        .unwrap();

        let _ = std::fs::remove_file(&tmp);
    }
}

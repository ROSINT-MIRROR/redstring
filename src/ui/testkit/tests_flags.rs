#[cfg(test)]
mod tests {
    use petgraph::graph::NodeIndex;

    use crate::model::{EntityKind, EntityRec, FlagMeta};
    use crate::project::{Project, read_case, validate, write_json};
    use crate::ui::RedstringApp;
    use crate::ui::testkit::{graph, harness, invariant};

    fn entity_ron(project: &Project, slug: &str) -> String {
        std::fs::read_to_string(project.root.join("entities").join(slug).join("entity.ron")).unwrap()
    }

    fn flagged_case(label: &str, code: &str) -> (harness::TempDir, RedstringApp, NodeIndex) {
        let (td, project) = harness::temp_project().unwrap();
        let mut app = harness::app().unwrap();
        app.project = Some(project);
        let n = graph::add(&mut app, EntityKind::Organization, label).unwrap();
        app.g.node_mut(n).unwrap().payload_mut().flag = Some(code.to_owned());
        harness::save(&mut app).unwrap();
        (td, app, n)
    }

    #[test]
    fn the_bundled_set_is_iso_alpha2_and_keeps_each_flags_own_proportions() {
        assert_eq!(FlagMeta::ALL.len(), 249, "every ISO 3166-1 alpha-2 code, nothing else");
        for pair in FlagMeta::ALL.windows(2) {
            assert!(pair[0].code < pair[1].code, "sorted and unique: {}", pair[1].code);
        }
        for m in FlagMeta::ALL.iter() {
            assert!(m.code.len() == 2 && m.code.chars().all(|c| c.is_ascii_uppercase()), "{}", m.code);
            assert!(!m.name.trim().is_empty(), "{} has an ISO short name", m.code);
            let img = image::load_from_memory(m.png).unwrap();
            assert_eq!(img.width().max(img.height()), 256, "{} is rasterised to a 256px long side", m.code);
        }
        let dims = |code: &str| {
            let img = image::load_from_memory(FlagMeta::of(code).unwrap().unwrap().png).unwrap();
            (img.width() as f32, img.height() as f32)
        };
        let (w, h) = dims("NP");
        assert!(h > w, "Nepal stays taller than wide");
        let (w, h) = dims("QA");
        assert!(w / h > 2.4, "Qatar keeps its 28:11 ratio, not a 4:3 crop");
        let (w, h) = dims("CH");
        assert_eq!(w, h, "Switzerland stays square");
        for not_iso in ["XK", "EU", "UN", "AC", "UK"] {
            assert!(FlagMeta::of(not_iso).unwrap().is_none(), "{not_iso} is not an ISO 3166-1 alpha-2 code");
        }
    }

    #[test]
    fn an_entity_line_from_before_flags_loads_and_saves_without_a_flag_key() {
        let old = r#"(slug: "doe", kind: Person, label: "Doe", value: "", image: Some("entities/doe/p.png"))"#;
        let rec: EntityRec = ron::from_str(old).unwrap();
        assert_eq!(rec.flag, None);
        assert_eq!(rec.image.as_deref(), Some("entities/doe/p.png"));
        let again = ron::ser::to_string(&rec).unwrap();
        assert!(!again.contains("flag"), "an unset flag adds nothing to the line: {again}");

        let (_td, project) = harness::temp_project().unwrap();
        let mut app = harness::app().unwrap();
        app.project = Some(project.clone());
        graph::add(&mut app, EntityKind::Person, "Jane Roe").unwrap();
        harness::save(&mut app).unwrap();
        assert!(!entity_ron(&project, "jane_roe").contains("flag"));
        harness::load(&mut app).unwrap();
        invariant::check_case_dir(&project).unwrap();
    }

    #[test]
    fn setting_and_clearing_a_flag_round_trips_and_never_touches_the_image() {
        let (_td, mut app, n) = flagged_case("Example Ministry", "RO");
        let project = app.project.clone().unwrap();
        let logo = project.root.join("entities/example_ministry/logo.png");
        std::fs::create_dir_all(logo.parent().unwrap()).unwrap();
        image::RgbaImage::from_pixel(4, 4, image::Rgba([0, 0, 0, 255])).save(&logo).unwrap();
        let abs = logo.display().to_string();
        app.g.node_mut(n).unwrap().payload_mut().image = Some(abs.clone());
        harness::save(&mut app).unwrap();

        let ron = entity_ron(&project, "example_ministry");
        assert!(ron.contains("flag: Some(\"RO\")"), "{ron}");
        assert!(ron.contains("entities/example_ministry/logo.png"), "{ron}");

        harness::load(&mut app).unwrap();
        let back = read_case(&project).unwrap();
        assert_eq!(back.data.entities[0].flag.as_deref(), Some("RO"));
        let n = graph::node_by_slug(&app, "example_ministry").unwrap().unwrap();
        let e = app.g.node(n).unwrap().payload().clone();
        assert_eq!(e.flag.as_deref(), Some("RO"));
        assert_eq!(e.image.as_deref(), Some(abs.as_str()), "the image is kept under the flag");

        app.g.node_mut(n).unwrap().payload_mut().flag = None;
        harness::save(&mut app).unwrap();
        let ron = entity_ron(&project, "example_ministry");
        assert!(!ron.contains("flag"), "clearing the flag removes the key: {ron}");
        assert!(ron.contains("entities/example_ministry/logo.png"), "the image is back in charge: {ron}");
        invariant::check_case_dir(&project).unwrap();
    }

    #[test]
    fn exports_drop_a_sealed_entitys_flag_and_check_flags_unknown_codes() {
        let (_td, mut app, n) = flagged_case("Example Ministry", "FR");
        let project = app.project.clone().unwrap();
        write_json(&project, &read_case(&project).unwrap()).unwrap();
        let json = std::fs::read_to_string(project.root.join("case.json")).unwrap();
        assert!(json.contains("\"flag\": \"FR\""), "{json}");

        app.g.node_mut(n).unwrap().payload_mut().sealed = true;
        harness::save(&mut app).unwrap();
        write_json(&project, &read_case(&project).unwrap()).unwrap();
        let json = std::fs::read_to_string(project.root.join("case.json")).unwrap();
        assert!(!json.contains("\"flag\""), "a sealed entity's flag is redacted like its image: {json}");

        app.g.node_mut(n).unwrap().payload_mut().flag = Some("ZZ".to_owned());
        harness::save(&mut app).unwrap();
        let findings = validate(&project).unwrap().join("\n");
        assert!(findings.contains("flag 'ZZ' is not an ISO 3166-1 alpha-2 code"), "{findings}");
    }

    #[test]
    fn a_flagged_node_renders_headless_as_an_image_tile() {
        let (_td, app, _n) = flagged_case("Romanian Example", "RO");
        let root = app.project.clone().unwrap().root;
        let args = |out: &std::path::Path| {
            vec![
                "--shot".to_owned(),
                root.display().to_string(),
                "--out".to_owned(),
                out.display().to_string(),
                "--width".to_owned(),
                "400".to_owned(),
                "--height".to_owned(),
                "300".to_owned(),
            ]
        };
        let json_out = root.join("shot.json");
        crate::shot::shoot(&args(&json_out)).unwrap();
        let snap: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&json_out).unwrap()).unwrap();
        let node = &snap["nodes"][0];
        assert_eq!(node["image"], serde_json::Value::Bool(true), "a flag draws as an image tile: {node}");

        let png_out = root.join("shot.png");
        match crate::shot::shoot(&args(&png_out)) {
            Ok(_) => {
                let img = image::open(&png_out).unwrap().to_rgba8();
                let x = node["center"][0].as_f64().unwrap().round() as u32;
                let y = node["center"][1].as_f64().unwrap().round() as u32;
                let px = img.get_pixel(x, y).0;
                assert!(
                    px[0] > 200 && px[1] > 150 && px[2] < 110,
                    "the centre of a Romanian flag tile is its yellow band, got {px:?}"
                );
            }
            Err(err) => eprintln!("png flag shot skipped — no usable wgpu adapter: {err}"),
        }
    }
}

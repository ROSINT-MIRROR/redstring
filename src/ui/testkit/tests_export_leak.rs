#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::model::EntityKind;
    use crate::project::export;
    use crate::ui::testkit::{apply, graph, harness};

    fn whois_spec() -> &'static str {
        r#"(
            id: "whois",
            label: "whois",
            applies_to: ["domain"],
            bin: "whois",
            args: ["{value}"],
            shapes: [(
                id: "icann",
                extract: (id: "keyvalue", config: (sep: ":")),
                min_hits: 1,
                emit: [Attr(from: "registrant", key: "registrant")],
            )],
        )"#
    }

    fn zip_entry_names(zip_path: &Path) -> Vec<String> {
        let f = std::fs::File::open(zip_path).unwrap();
        let mut archive = zip::ZipArchive::new(f).unwrap();
        let mut names = Vec::new();
        for i in 0..archive.len() {
            names.push(archive.by_index(i).unwrap().name().to_owned());
        }
        names
    }

    fn zip_raw_bytes(zip_path: &Path) -> Vec<u8> {
        let f = std::fs::File::open(zip_path).unwrap();
        let mut archive = zip::ZipArchive::new(f).unwrap();
        let mut all = Vec::new();
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).unwrap();
            all.extend_from_slice(entry.name().as_bytes());
            std::io::Read::read_to_end(&mut entry, &mut all).unwrap();
        }
        all
    }

    #[test]
    fn lookup_evidence_on_a_sealed_entity_never_reaches_the_bundle() {
        let (_td, project) = harness::temp_project().unwrap();
        let mut app = harness::app().unwrap();
        app.project = Some(project.clone());
        app.case_name = "leakcheck".to_owned();

        let target = graph::add_valued(
            &mut app,
            EntityKind::Domain,
            "sealeddomainsentinel.example",
            "sealeddomainsentinel.example",
        )
        .unwrap();

        let raw = "Registrant: REGISTRANTSECRETSENTINEL\nEmail: victim@leak.example\n";
        let slug = graph::slug_of(&app, target).unwrap();
        let archived = format!("evidence/docs/whois-{slug}-1700000000.txt");
        apply::apply_archiving(&mut app, whois_spec(), "whois", &slug, raw, &archived).unwrap();

        let on_disk = project.resolve(&archived).unwrap();
        assert!(
            on_disk.is_file(),
            "the lookup must actually archive raw stdout, otherwise this test proves nothing"
        );
        assert!(
            std::fs::read_to_string(&on_disk)
                .unwrap()
                .contains("REGISTRANTSECRETSENTINEL"),
            "the archived evidence really holds the secret before sealing"
        );

        graph::set_sealed(&mut app, target, true).unwrap();
        harness::save(&mut app).unwrap();

        let (zip_path, _n, sealed) = export(&project).unwrap();
        assert_eq!(sealed, 1, "the sealed entity is reported redacted");

        let names = zip_entry_names(&zip_path);
        let blob = String::from_utf8_lossy(&zip_raw_bytes(&zip_path)).into_owned();

        for name in &names {
            assert!(
                !name.contains(slug.as_str()),
                "a bundle FILENAME carries the sealed entity's real slug: {name}"
            );
            assert!(
                !name.contains("sealeddomainsentinel"),
                "a bundle FILENAME carries the sealed entity's real value: {name}"
            );
        }

        for leak in [
            "REGISTRANTSECRETSENTINEL",
            "victim@leak.example",
            "sealeddomainsentinel",
            slug.as_str(),
        ] {
            assert!(
                !blob.contains(leak),
                "the exported bundle leaked sealed material '{leak}'"
            );
        }

        let local = std::fs::read_to_string(&on_disk).unwrap();
        assert!(
            local.contains("REGISTRANTSECRETSENTINEL"),
            "the working case keeps the full evidence — only the bundle is redacted"
        );
    }

    #[test]
    fn unsealed_lookup_evidence_still_ships_in_the_bundle() {
        let (_td, project) = harness::temp_project().unwrap();
        let mut app = harness::app().unwrap();
        app.project = Some(project.clone());
        app.case_name = "keepcheck".to_owned();

        let target = graph::add_valued(
            &mut app,
            EntityKind::Domain,
            "openexample.test",
            "openexample.test",
        )
        .unwrap();
        let raw = "Registrant: PUBLICREGISTRANTSENTINEL\n";
        let slug = graph::slug_of(&app, target).unwrap();
        let archived = format!("evidence/docs/whois-{slug}-1700000001.txt");
        apply::apply_archiving(&mut app, whois_spec(), "whois", &slug, raw, &archived).unwrap();
        harness::save(&mut app).unwrap();

        let (zip_path, _n, sealed) = export(&project).unwrap();
        assert_eq!(sealed, 0, "nothing is sealed in this case");
        let blob = String::from_utf8_lossy(&zip_raw_bytes(&zip_path)).into_owned();
        assert!(
            blob.contains("PUBLICREGISTRANTSENTINEL"),
            "an unsealed case must still publish its evidence — redaction must not over-reach"
        );
    }

    #[test]
    fn sealed_note_and_image_never_reach_the_bundle() {
        let (_td, project) = harness::temp_project().unwrap();
        let mut app = harness::app().unwrap();
        app.project = Some(project.clone());
        app.case_name = "notecheck".to_owned();

        let target = graph::add_valued(
            &mut app,
            EntityKind::Person,
            "Sealed Person",
            "SEALEDPERSONVALUE",
        )
        .unwrap();
        let slug = graph::slug_of(&app, target).unwrap();
        graph::set_sealed(&mut app, target, true).unwrap();
        harness::save(&mut app).unwrap();

        crate::project::write_note(&project, &slug, "SEALEDNOTEBODYSENTINEL\n").unwrap();
        let img_rel = format!("entities/{slug}/portrait.png");
        let img = project.resolve(&img_rel).unwrap();
        std::fs::create_dir_all(img.parent().unwrap()).unwrap();
        std::fs::write(&img, b"PORTRAITBYTESENTINEL").unwrap();

        let (zip_path, _n, sealed) = export(&project).unwrap();
        assert_eq!(sealed, 1);
        let names = zip_entry_names(&zip_path);
        let blob = String::from_utf8_lossy(&zip_raw_bytes(&zip_path)).into_owned();

        for name in &names {
            assert!(
                !name.contains(slug.as_str()),
                "a sealed entity's directory must not appear in the bundle: {name}"
            );
        }
        for leak in [
            "SEALEDNOTEBODYSENTINEL",
            "PORTRAITBYTESENTINEL",
            "SEALEDPERSONVALUE",
            "Sealed Person",
            slug.as_str(),
        ] {
            assert!(
                !blob.contains(leak),
                "the bundle leaked sealed material '{leak}'"
            );
        }
    }

    #[test]
    fn two_exports_of_the_same_case_are_not_correlatable() {
        let (_td, project) = harness::temp_project().unwrap();
        let mut app = harness::app().unwrap();
        app.project = Some(project.clone());
        app.case_name = "correlate".to_owned();

        let target = graph::add_valued(
            &mut app,
            EntityKind::Person,
            "Protected One",
            "PROTECTEDVALUE",
        )
        .unwrap();
        graph::set_sealed(&mut app, target, true).unwrap();
        harness::save(&mut app).unwrap();

        let (zip_a, _, _) = export(&project).unwrap();
        let blob_a = String::from_utf8_lossy(&zip_raw_bytes(&zip_a)).into_owned();
        let alias_a = first_alias(&blob_a);
        std::fs::remove_file(&zip_a).unwrap();

        let (zip_b, _, _) = export(&project).unwrap();
        let blob_b = String::from_utf8_lossy(&zip_raw_bytes(&zip_b)).into_owned();
        let alias_b = first_alias(&blob_b);

        assert!(
            !alias_a.is_empty() && !alias_b.is_empty(),
            "each bundle names the sealed node"
        );
        assert_ne!(
            alias_a, alias_b,
            "two bundles of the same case must not share a pseudonym — that is the correlation handle"
        );
    }

    fn first_alias(blob: &str) -> String {
        for token in blob.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')) {
            if token.starts_with("sealed_") && token.len() > "sealed_".len() {
                return token.to_owned();
            }
        }
        String::new()
    }
}

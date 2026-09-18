





use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use zip::write::SimpleFileOptions;

use crate::error::Error;
use crate::model::{CaseFile, CaseMeta, FORMAT_VERSION};

use super::derive::{AliasBook, redact_with, sealed_slugs, write_graph_md, write_json};
use super::dossier::write_dossiers;
use super::ignore::RsIgnore;
use super::io::{read_case, read_note, write_case, write_note};
use super::layout::{CASE_FILE, Project};
use super::leakscan::{self, sealed_tokens};
use super::validate::cited_paths;

pub const BUNDLE_SUFFIX: &str = ".redstring.zip";
static EXPORT_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);




pub fn export(project: &Project) -> Result<(PathBuf, usize, usize), Error> {
    let ignore = RsIgnore::load(&project.root)?;
    let out = project
        .root
        .join(format!("{}{BUNDLE_SUFFIX}", project.dir_name()?));

    let cf = read_case(project)?;
    let sealed = cf.data.entities.iter().filter(|e| e.sealed).count();
    if !needs_redaction(&cf) {
        let n = zip_tree(&project.root, &out, &ignore)?;
        return Ok((out, n, 0));
    }

    let staging = staging_dir(project)?;
    let _stale = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging)
        .map_err(|source| Error::Io { path: staging.to_path_buf(), source })?;
    let temp = Project::create(&staging)?;
    let stage_root = temp.root.clone();

    let mut book = AliasBook::new()?;
    let public = redact_with(&cf, &mut book)?;
    let hidden = sealed_slugs(&cf)?;

    write_case(&temp, &public)?;
    write_json(&temp, &public)?;
    write_graph_md(&temp, &public)?;
    write_dossiers(&temp, &public)?;
    copy_allowed(project, &temp, &cf, &public, &hidden, &ignore)?;

    let tokens = sealed_tokens(&cf, &hidden)?;
    match assert_no_sealed_tokens(&stage_root, &tokens, &ignore) {
        Ok(()) => {}
        Err(leak) => {
            let _cleanup = std::fs::remove_dir_all(&stage_root);
            return Err(leak);
        }
    }

    let n = zip_tree(&stage_root, &out, &ignore)?;
    let _cleanup = std::fs::remove_dir_all(&stage_root);
    Ok((out, n, sealed))
}

fn copy_allowed(
    project: &Project,
    temp: &Project,
    cf: &CaseFile,
    public: &CaseFile,
    hidden: &BTreeSet<String>,
    ignore: &RsIgnore,
) -> Result<(), Error> {
    for (_, rel) in cited_paths(cf)? {
        copy_one(project, temp, &rel, ignore)?;
    }
    for e in &public.data.entities {
        let note = read_note(project, &e.slug)?;
        if note.trim().is_empty() {
            continue;
        }
        write_note(temp, &e.slug, &note)?;
    }
    for e in &cf.data.entities {
        if !hidden.contains(&e.slug) {
            continue;
        }
        let dir = temp.entity_dir(&e.slug)?;
        let _gone = std::fs::remove_dir_all(&dir);
    }
    Ok(())
}

fn copy_one(
    project: &Project,
    temp: &Project,
    rel: &str,
    ignore: &RsIgnore,
) -> Result<(), Error> {
    if ignore.matches(rel)? {
        return Ok(());
    }
    let from = project.resolve(rel)?;
    if !from.is_file() {
        return Ok(());
    }
    let to = temp.resolve(rel)?;
    let Some(parent) = to.parent() else {
        return Ok(());
    };
    std::fs::create_dir_all(parent)
        .map_err(|source| Error::Io { path: parent.to_path_buf(), source })?;
    std::fs::copy(&from, &to).map_err(|source| Error::Io { path: from.to_path_buf(), source })?;
    Ok(())
}

fn needs_redaction(cf: &CaseFile) -> bool {
    let entity = cf.data.entities.iter().any(|e| {
        e.sealed || e.observations.iter().any(|o| o.sealed || o.sources.iter().any(|s| s.sealed))
    });
    let relation = cf
        .data
        .relations
        .iter()
        .any(|r| r.sealed || r.sources.iter().any(|s| s.sealed));
    entity || relation
}

fn staging_dir(project: &Project) -> Result<PathBuf, Error> {
    let stem = project.dir_name()?;
    let seq = EXPORT_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    Ok(std::env::temp_dir().join(format!(
        "redstring_export_{stem}_{}_{seq}",
        std::process::id()
    )))
}

fn assert_no_sealed_tokens(
    staging: &Path,
    tokens: &[String],
    ignore: &RsIgnore,
) -> Result<(), Error> {
    let mut files: Vec<String> = Vec::new();
    collect_files(staging, "", ignore, &mut files)?;
    leakscan::assert_clean(staging, tokens, &files)
}

fn zip_tree(src_root: &Path, out: &Path, ignore: &RsIgnore) -> Result<usize, Error> {
    let mut files: Vec<String> = Vec::new();
    collect_files(src_root, "", ignore, &mut files)?;
    files.sort();

    let f = std::fs::File::create(out).map_err(|source| Error::Io { path: out.to_path_buf(), source })?;
    let mut zip = zip::ZipWriter::new(f);
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for rel in &files {
        zip.start_file(rel, opts)
            .map_err(|source| Error::Zip { path: out.to_path_buf(), source })?;
        let abs = src_root.join(rel);
        let mut src = std::fs::File::open(&abs).map_err(|source| Error::Io { path: abs.to_path_buf(), source })?;
        std::io::copy(&mut src, &mut zip).map_err(|source| Error::Io { path: abs.to_path_buf(), source })?;
    }
    zip.finish().map_err(|source| Error::Zip { path: out.to_path_buf(), source })?;
    Ok(files.len())
}





fn collect_files(root: &Path, rel_dir: &str, ignore: &RsIgnore, out: &mut Vec<String>) -> Result<(), Error> {
    let abs = root.join(rel_dir);
    let entries = std::fs::read_dir(&abs).map_err(|source| Error::Io { path: abs.to_path_buf(), source })?;
    for entry in entries {
        let entry = entry.map_err(|source| Error::Io { path: abs.to_path_buf(), source })?;
        let name_os = entry.file_name();
        let Some(name) = name_os.to_str() else { continue };
        let rel = if rel_dir.is_empty() {
            name.to_owned()
        } else {
            format!("{rel_dir}/{name}")
        };
        if ignore.matches(&rel)? {
            continue;
        }
        let ft = entry.file_type().map_err(|source| Error::Io { path: abs.to_path_buf(), source })?;
        if ft.is_dir() {
            if name == ".git" {
                continue;
            }
            collect_files(root, &rel, ignore, out)?;
        } else if ft.is_file() {
            if name.ends_with(BUNDLE_SUFFIX) {
                continue;
            }
            out.push(rel);
        }
    }
    Ok(())
}





pub fn open_bundle(zip_path: &Path) -> Result<Project, Error> {
    let Some(parent) = zip_path.parent().filter(|p| !p.as_os_str().is_empty()) else {
        return Err(Error::NotAProject {
            path: zip_path.to_path_buf(),
        });
    };
    
    
    if parent.join(CASE_FILE).is_file() {
        return Err(Error::BundleInsideProject {
            path: parent.to_path_buf(),
        });
    }

    let f = std::fs::File::open(zip_path).map_err(|source| Error::Io { path: zip_path.to_path_buf(), source })?;
    let mut archive =
        zip::ZipArchive::new(f).map_err(|source| Error::Zip { path: zip_path.to_path_buf(), source })?;

    let meta_text = read_case_entry(&mut archive, zip_path)?;
    let meta: CaseMeta =
        ron::from_str(&meta_text).map_err(|source| Error::RonParse { path: zip_path.to_path_buf(), source })?;
    if meta.format > FORMAT_VERSION {
        return Err(Error::FormatTooNew {
            found: meta.format,
            supported: FORMAT_VERSION,
        });
    }

    let target = parent.join(bundle_stem(zip_path));
    if target.exists() {
        return Err(Error::TargetExists { path: target });
    }
    extract_all(&mut archive, &target, zip_path)?;

    let project = Project::open(&target)?;
    let cf = read_case(&project)?;
    for (_, rel) in cited_paths(&cf)? {
        if !project.root.join(&rel).exists() {
            let _cleanup = std::fs::remove_dir_all(&project.root);
            return Err(Error::BundleMissingRef { reference: rel });
        }
    }
    Ok(project)
}

fn read_case_entry(
    archive: &mut zip::ZipArchive<std::fs::File>,
    zip_path: &Path,
) -> Result<String, Error> {
    let Ok(mut entry) = archive.by_name(CASE_FILE) else {
        return Err(Error::BundleMissingCase {
            path: zip_path.to_path_buf(),
        });
    };
    let mut text = String::new();
    std::io::Read::read_to_string(&mut entry, &mut text)
        .map_err(|source| Error::Io { path: zip_path.to_path_buf(), source })?;
    Ok(text)
}


fn bundle_stem(p: &Path) -> String {
    let name = p.file_name().map(|n| n.to_string_lossy().into_owned());
    let Some(name) = name else {
        return "case".to_owned();
    };
    let Some(stem) = name.strip_suffix(BUNDLE_SUFFIX) else {
        let Some(stem) = name.strip_suffix(".zip") else {
            return name;
        };
        return stem.to_owned();
    };
    stem.to_owned()
}

fn extract_all(
    archive: &mut zip::ZipArchive<std::fs::File>,
    target: &Path,
    zip_path: &Path,
) -> Result<(), Error> {
    std::fs::create_dir_all(target).map_err(|source| Error::Io { path: target.to_path_buf(), source })?;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|source| Error::Zip { path: zip_path.to_path_buf(), source })?;
        
        let Some(rel) = entry.enclosed_name() else {
            let bad = std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "archive entry escapes the extraction directory",
            );
            return Err(Error::Io { path: zip_path.to_path_buf(), source: bad });
        };
        let dst = target.join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&dst).map_err(|source| Error::Io { path: dst.to_path_buf(), source })?;
            continue;
        }
        let Some(dir) = dst.parent() else { continue };
        std::fs::create_dir_all(dir).map_err(|source| Error::Io { path: dir.to_path_buf(), source })?;
        let mut out = std::fs::File::create(&dst).map_err(|source| Error::Io { path: dst.to_path_buf(), source })?;
        
        std::io::copy(&mut entry, &mut out).map_err(|source| Error::Io { path: dst.to_path_buf(), source })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        Author, CaseFile, CaseMeta, EntityKind, EntityRec, ObsValue, Observation, Origin,
        RelationKind, RelationRec, Source, SourceKind, Verification,
    };
    use crate::project::io;

    fn fresh_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("redstring_bundle_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn seeded_project(name: &str) -> Project {
        let p = Project::create(&fresh_dir(name)).unwrap();
        let img = p.resolve("entities/doe/portrait.png").unwrap();
        std::fs::create_dir_all(img.parent().unwrap()).unwrap();
        std::fs::write(&img, b"fakepng").unwrap();
        std::fs::create_dir_all(p.resolve("extra").unwrap()).unwrap();
        std::fs::write(p.resolve("extra/raw-big.mp4").unwrap(), vec![0u8; 4096]).unwrap();
        std::fs::write(p.root.join(".rsignore"), "extra/raw-*.mp4\n").unwrap();

        let mut cf = CaseFile::default();
        cf.meta = CaseMeta::stamped(name).unwrap();
        cf.data.entities.push(EntityRec {
            slug: "doe".into(),
            kind: EntityKind::Person,
            label: "J. Doe".into(),
            value: String::new(),
            sealed: false,
            observations: vec![],
            facets: Default::default(),
            image: Some("entities/doe/portrait.png".into()),
        });
        io::write_case(&p, &cf).unwrap();
        io::write_note(&p, "doe", "the guy\n").unwrap();
        p
    }

    #[test]
    fn export_then_open_round_trip() {
        let p = seeded_project("trip");
        let (zip_path, n, sealed) = export(&p).unwrap();
        assert!(zip_path.ends_with(format!("{}{BUNDLE_SUFFIX}", p.dir_name().unwrap())));
        assert_eq!(sealed, 0, "an unsealed case reports zero redactions");


        assert_eq!(n, 16);

        
        assert!(matches!(
            open_bundle(&zip_path),
            Err(Error::BundleInsideProject { .. })
        ));

        
        let inbox = fresh_dir("trip_inbox");
        std::fs::create_dir_all(&inbox).unwrap();
        let received = inbox.join(zip_path.file_name().unwrap());
        std::fs::copy(&zip_path, &received).unwrap();

        let opened = open_bundle(&received).unwrap();
        assert_eq!(io::read_case(&opened).unwrap().meta.name, "trip");
        assert_eq!(io::read_note(&opened, "doe").unwrap(), "the guy\n");
        assert!(opened.resolve("entities/doe/portrait.png").unwrap().exists());
        assert!(!opened.resolve("extra/raw-big.mp4").unwrap().exists()); 

        
        assert!(matches!(
            open_bundle(&received),
            Err(Error::TargetExists { .. })
        ));

        std::fs::remove_dir_all(p.root).unwrap();
        std::fs::remove_dir_all(&inbox).unwrap();
    }

    #[test]
    fn integrity_catches_missing_citations() {
        let p = seeded_project("integrity");
        
        
        std::fs::remove_file(p.resolve("entities/doe/portrait.png").unwrap()).unwrap();
        let (zip_path, _, _) = export(&p).unwrap();

        let inbox = fresh_dir("integrity_inbox");
        std::fs::create_dir_all(&inbox).unwrap();
        let received = inbox.join(zip_path.file_name().unwrap());
        std::fs::copy(&zip_path, &received).unwrap();

        assert!(matches!(
            open_bundle(&received),
            Err(Error::BundleMissingRef { .. })
        ));

        std::fs::remove_dir_all(p.root).unwrap();
        std::fs::remove_dir_all(&inbox).unwrap();
    }

    fn obs(name: &str, value: &str, sealed: bool) -> Observation {
        let mut o = Observation::stamp(
            name.to_owned(),
            ObsValue::Text(value.to_owned()),
            Origin::Manual,
            Verification::AnalystAsserted,
            Author::default(),
        )
        .unwrap();
        o.sealed = sealed;
        o
    }

    fn sealed_project(name: &str) -> Project {
        let p = Project::create(&fresh_dir(name)).unwrap();
        let img = p.resolve("entities/jane_victim/face.png").unwrap();
        std::fs::create_dir_all(img.parent().unwrap()).unwrap();
        std::fs::write(&img, b"fakepng").unwrap();

        let victim = EntityRec {
            slug: "jane_victim".into(),
            kind: EntityKind::Person,
            label: "Jane Victim".into(),
            value: "jane.victim@example.com".into(),
            sealed: true,
            observations: vec![obs("dob", "2009-05-01", false)],
            facets: Default::default(),
            image: Some("entities/jane_victim/face.png".into()),
        };
        let acme = EntityRec {
            slug: "acme".into(),
            kind: EntityKind::Organization,
            label: "Acme Care Home".into(),
            value: String::new(),
            sealed: false,
            observations: vec![
                obs("public_reg", "RO12345", false),
                obs("informant_tip", "Deep Throat rang", true),
            ],
            facets: Default::default(),
            image: None,
        };
        let soldier = EntityRec {
            slug: "soldier".into(),
            kind: EntityKind::Person,
            label: "Sgt Marku".into(),
            value: String::new(),
            sealed: false,
            observations: vec![],
            facets: Default::default(),
            image: None,
        };
        let assault = RelationRec {
            from: "jane_victim".into(),
            to: "soldier".into(),
            kind: RelationKind::SexuallyAssaulted,
            origin: Origin::Manual,
            status: Verification::AnalystAsserted,
            confidence: 100,
            author: Author::default(),
            created_at: 0,
            sources: vec![Source {
                kind: SourceKind::Note,
                reference: "confidential-wire".into(),
                excerpt: "secret wiretap line".into(),
                sealed: true,
                ..Default::default()
            }],
            valid_from: None,
            valid_to: None,
            sealed: false,
        };
        let employ = RelationRec {
            from: "soldier".into(),
            to: "acme".into(),
            kind: RelationKind::EmployeeOf,
            origin: Origin::Manual,
            status: Verification::AnalystAsserted,
            confidence: 100,
            author: Author::default(),
            created_at: 0,
            sources: vec![Source {
                kind: SourceKind::Url,
                reference: "https://public.example/filing".into(),
                ..Default::default()
            }],
            valid_from: None,
            valid_to: None,
            sealed: false,
        };
        let hush = RelationRec {
            from: "acme".into(),
            to: "soldier".into(),
            kind: RelationKind::Paid,
            origin: Origin::Manual,
            status: Verification::AnalystAsserted,
            confidence: 100,
            author: Author::default(),
            created_at: 0,
            sources: vec![Source {
                kind: SourceKind::Note,
                reference: "hush-money-ledger".into(),
                excerpt: "off-books settlement".into(),
                ..Default::default()
            }],
            valid_from: None,
            valid_to: None,
            sealed: true,
        };

        let mut cf = CaseFile::default();
        cf.meta = CaseMeta::stamped(name).unwrap();
        cf.data.entities.push(victim);
        cf.data.entities.push(acme);
        cf.data.entities.push(soldier);
        cf.data.relations.push(assault);
        cf.data.relations.push(employ);
        cf.data.relations.push(hush);
        cf.display.positions.insert("jane_victim".into(), (10.0, 20.0));
        io::write_case(&p, &cf).unwrap();
        io::write_note(&p, "jane_victim", "minor, 14yo, do not publish her name\n").unwrap();
        io::write_note(&p, "acme", "public company\n").unwrap();
        p
    }

    fn alias_in_blob(blob: &str) -> String {
        for token in blob.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')) {
            if token.starts_with("sealed_") && token.len() > "sealed_".len() {
                return token.to_owned();
            }
        }
        String::new()
    }

    fn zip_blob(zip_path: &Path) -> String {
        let f = std::fs::File::open(zip_path).unwrap();
        let mut archive = zip::ZipArchive::new(f).unwrap();
        let mut blob = String::new();
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).unwrap();
            blob.push_str(entry.name());
            blob.push('\n');
            let mut bytes = Vec::new();
            std::io::Read::read_to_end(&mut entry, &mut bytes).unwrap();
            blob.push_str(&String::from_utf8_lossy(&bytes));
            blob.push('\n');
        }
        blob
    }

    #[test]
    fn sealed_entity_never_leaks_into_bundle() {
        let p = sealed_project("sealed");
        let (zip_path, _n, sealed) = export(&p).unwrap();
        assert_eq!(sealed, 1, "one sealed entity reported redacted");

        let blob = zip_blob(&zip_path);
        let alias = alias_in_blob(&blob);
        for leak in [
            "jane_victim",
            "Jane Victim",
            "jane.victim@example.com",
            "2009-05-01",
            "minor, 14yo",
            "Deep Throat rang",
            "informant_tip",
            "confidential-wire",
            "secret wiretap",
            "sexually assaulted",
            "hush-money-ledger",
            "off-books settlement",
        ] {
            assert!(!blob.contains(leak), "bundle leaked sealed data '{leak}'");
        }
        assert!(blob.contains(&alias), "the anonymized id must be present in the bundle");
        assert!(blob.contains("[sealed]"), "the placeholder label must be present");
        assert!(blob.contains("Acme Care Home"), "an unsealed entity survives");
        assert!(blob.contains("Sgt Marku"), "the named perpetrator still ships — only the victim is protected");
        assert!(blob.contains("RO12345"), "an unsealed observation survives");
        assert!(blob.contains("public.example/filing"), "an unsealed source on a surviving edge survives");

        let inbox = fresh_dir("sealed_inbox");
        std::fs::create_dir_all(&inbox).unwrap();
        let received = inbox.join(zip_path.file_name().unwrap());
        std::fs::copy(&zip_path, &received).unwrap();
        let opened = open_bundle(&received).unwrap();
        let back = io::read_case(&opened).unwrap();

        let ghost = back
            .data
            .entities
            .iter()
            .find(|e| e.slug == alias)
            .expect("the sealed entity keeps a stable anonymized slug in the bundle");
        assert_eq!(ghost.label, "[sealed]");
        assert!(ghost.value.is_empty());
        assert!(ghost.observations.is_empty(), "no real observations ship");
        assert!(ghost.image.is_none(), "no image path ships");
        assert!(
            io::read_note(&opened, &alias).unwrap().is_empty(),
            "no note ships for the sealed entity"
        );

        assert_eq!(
            back.data.relations.len(),
            1,
            "only the unsealed edge between two unsealed nodes survives — the incident and explicitly-sealed edges are withheld"
        );
        let survivor = &back.data.relations[0];
        assert_eq!(
            survivor.kind,
            RelationKind::EmployeeOf,
            "the sensitive incident verb and the explicitly-sealed verb are both gone"
        );
        assert_eq!(survivor.from, "soldier");
        assert_eq!(survivor.to, "acme");
        assert_eq!(survivor.sources.len(), 1, "the surviving edge keeps its unsealed source");
        assert!(
            back.data.relations.iter().all(|r| r.from != alias && r.to != alias),
            "no surviving edge may touch the anonymized victim — that pairing is the triangulation vector"
        );
        assert!(
            back.data.relations.iter().all(|r| r.kind != RelationKind::SexuallyAssaulted),
            "the sensitive verb must not survive on any relation record"
        );

        assert_eq!(
            io::read_note(&p, "jane_victim").unwrap(),
            "minor, 14yo, do not publish her name\n",
            "the local on-disk case keeps the full note as the working copy"
        );
        let local = io::read_case(&p).unwrap();
        assert!(
            local.data.entities.iter().any(|e| e.label == "Jane Victim"),
            "the local on-disk case keeps the real entity data"
        );

        std::fs::remove_dir_all(p.root).unwrap();
        std::fs::remove_dir_all(&inbox).unwrap();
    }

    #[test]
    fn sealed_relation_alone_triggers_redaction_and_is_dropped() {
        let p = Project::create(&fresh_dir("relseal")).unwrap();
        let mut cf = CaseFile::default();
        cf.meta = CaseMeta::stamped("relseal").unwrap();
        for (slug, kind, label) in [
            ("ana", EntityKind::Person, "Ana"),
            ("beta", EntityKind::Organization, "Beta SRL"),
        ] {
            cf.data.entities.push(EntityRec {
                slug: slug.into(),
                kind,
                label: label.into(),
                value: String::new(),
                sealed: false,
                observations: vec![],
                facets: Default::default(),
                image: None,
            });
        }
        cf.data.relations.push(RelationRec {
            from: "ana".into(),
            to: "beta".into(),
            kind: RelationKind::Bribed,
            origin: Origin::Manual,
            status: Verification::AnalystAsserted,
            confidence: 100,
            author: Author::default(),
            created_at: 0,
            sources: vec![Source {
                kind: SourceKind::Note,
                reference: "sealed-tip-xyz".into(),
                excerpt: "kept strictly off record".into(),
                ..Default::default()
            }],
            valid_from: None,
            valid_to: None,
            sealed: true,
        });
        io::write_case(&p, &cf).unwrap();

        let (zip_path, _n, _sealed) = export(&p).unwrap();
        let blob = zip_blob(&zip_path);
        assert!(
            !blob.contains("sealed-tip-xyz"),
            "a case whose only seal is a relation must still redact — its source ref leaked"
        );
        assert!(!blob.contains("kept strictly off record"), "the sealed edge excerpt leaked");
        assert!(!blob.contains("|bribed"), "graph.md must not render the sealed edge");
        assert!(blob.contains("Ana") && blob.contains("Beta SRL"), "the unsealed named nodes still ship");

        let inbox = fresh_dir("relseal_inbox");
        std::fs::create_dir_all(&inbox).unwrap();
        let received = inbox.join(zip_path.file_name().unwrap());
        std::fs::copy(&zip_path, &received).unwrap();
        let opened = open_bundle(&received).unwrap();
        assert!(
            io::read_case(&opened).unwrap().data.relations.is_empty(),
            "the sealed relation must be dropped from the bundle"
        );

        std::fs::remove_dir_all(p.root).unwrap();
        std::fs::remove_dir_all(&inbox).unwrap();
    }

    #[test]
    fn unsealed_bundle_is_byte_identical_to_source_dir() {
        let p = seeded_project("regress");
        let (zip_path, n, sealed) = export(&p).unwrap();
        assert_eq!(sealed, 0, "no redaction happens for an unsealed case");
        assert_eq!(n, 16, "the unsealed file count is unchanged");

        let f = std::fs::File::open(&zip_path).unwrap();
        let mut archive = zip::ZipArchive::new(f).unwrap();
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).unwrap();
            let entry_name = entry.name().to_owned();
            let mut bytes = Vec::new();
            std::io::Read::read_to_end(&mut entry, &mut bytes).unwrap();
            let on_disk = std::fs::read(p.root.join(&entry_name)).unwrap();
            assert_eq!(
                bytes, on_disk,
                "unsealed bundle rewrote '{entry_name}' — it must be a pristine copy of the dir"
            );
        }

        std::fs::remove_dir_all(p.root).unwrap();
    }
}

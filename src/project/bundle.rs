//! Bundling: a bundle is the case directory, zipped — nothing more clever.
//! Anyone without redstring can unzip it and read the evidence with stock
//! tools; redstring itself opens it directly: verify integrity, extract
//! beside the zip, open the extracted case. Export lands in the project
//! root by default, so bundles never bundle bundles.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use zip::write::SimpleFileOptions;

use crate::error::{Error, RsResult};
use crate::model::{CaseFile, FORMAT_VERSION};

use super::ignore::RsIgnore;
use super::layout::{CASE_FILE, Project};
use super::validate::cited_paths;

pub const BUNDLE_SUFFIX: &str = ".redstring.zip";

/// Zip the case directory to `<root>/<dirname>.redstring.zip`. Excluded:
/// .git, anything .rsignore matches, and other bundles. Returns the bundle
/// path and how many files went in.
pub fn export(project: &Project) -> RsResult<(PathBuf, usize)> {
    let ignore = RsIgnore::load(project.root())?;
    let out = project
        .root()
        .join(format!("{}{BUNDLE_SUFFIX}", project.dir_name()));

    let mut files: Vec<String> = Vec::new();
    collect_files(project.root(), "", &ignore, &mut files)?;
    files.sort();

    let f = std::fs::File::create(&out).map_err(|source| Error::io(&out, source))?;
    let mut zip = zip::ZipWriter::new(f);
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for rel in &files {
        zip.start_file(rel, opts)
            .map_err(|source| Error::zip(&out, source))?;
        let abs = project.resolve(rel);
        let mut src = std::fs::File::open(&abs).map_err(|source| Error::io(&abs, source))?;
        std::io::copy(&mut src, &mut zip).map_err(|source| Error::io(&abs, source))?;
    }
    zip.finish().map_err(|source| Error::zip(&out, source))?;
    Ok((out, files.len()))
}

/// Walk the tree depth-first collecting project-relative file paths.
/// Skipped: .git directories, bundle files, .rsignore matches, and entries
/// with non-UTF-8 names or of exotic types — symlinks don't belong in a
/// portable evidence bundle.
fn collect_files(root: &Path, rel_dir: &str, ignore: &RsIgnore, out: &mut Vec<String>) -> RsResult<()> {
    let abs = root.join(rel_dir);
    let entries = std::fs::read_dir(&abs).map_err(|source| Error::io(&abs, source))?;
    for entry in entries {
        let entry = entry.map_err(|source| Error::io(&abs, source))?;
        let name_os = entry.file_name();
        let Some(name) = name_os.to_str() else { continue };
        let rel = if rel_dir.is_empty() {
            name.to_owned()
        } else {
            format!("{rel_dir}/{name}")
        };
        if ignore.matches(&rel) {
            continue;
        }
        let ft = entry.file_type().map_err(|source| Error::io(&abs, source))?;
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

/// Open a bundle: check integrity — readable archive, a case file that
/// parses at a supported format, every cited file present — then extract to
/// a directory named after the bundle, beside it, and open that case.
/// Per-entry CRCs are verified by the zip reader as each file streams out.
pub fn open_bundle(zip_path: &Path) -> RsResult<Project> {
    let Some(parent) = zip_path.parent().filter(|p| !p.as_os_str().is_empty()) else {
        return Err(Error::NotAProject {
            path: zip_path.to_path_buf(),
        });
    };
    // the default export location is inside the case dir — opening that
    // bundle should open the surrounding case, not nest a copy into it
    if parent.join(CASE_FILE).is_file() {
        return Err(Error::BundleInsideProject {
            path: parent.to_path_buf(),
        });
    }

    let f = std::fs::File::open(zip_path).map_err(|source| Error::io(zip_path, source))?;
    let mut archive =
        zip::ZipArchive::new(f).map_err(|source| Error::zip(zip_path, source))?;

    let case_text = read_case_entry(&mut archive, zip_path)?;
    let cf: CaseFile =
        ron::from_str(&case_text).map_err(|source| Error::ron_parse(zip_path, source))?;
    if cf.meta.format > FORMAT_VERSION {
        return Err(Error::FormatTooNew {
            found: cf.meta.format,
            supported: FORMAT_VERSION,
        });
    }
    let names: HashSet<String> = archive.file_names().map(str::to_owned).collect();
    for (_, rel) in cited_paths(&cf) {
        if !names.contains(&rel) {
            return Err(Error::BundleMissingRef { reference: rel });
        }
    }

    let target = parent.join(bundle_stem(zip_path));
    if target.exists() {
        return Err(Error::TargetExists { path: target });
    }
    extract_all(&mut archive, &target, zip_path)?;
    Project::open(&target)
}

fn read_case_entry(
    archive: &mut zip::ZipArchive<std::fs::File>,
    zip_path: &Path,
) -> RsResult<String> {
    let Ok(mut entry) = archive.by_name(CASE_FILE) else {
        return Err(Error::BundleMissingCase {
            path: zip_path.to_path_buf(),
        });
    };
    let mut text = String::new();
    std::io::Read::read_to_string(&mut entry, &mut text)
        .map_err(|source| Error::io(zip_path, source))?;
    Ok(text)
}

/// "asfalt.redstring.zip" extracts into "asfalt"; a plain "asfalt.zip" too.
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
) -> RsResult<()> {
    std::fs::create_dir_all(target).map_err(|source| Error::io(target, source))?;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|source| Error::zip(zip_path, source))?;
        // enclosed_name refuses absolute or parent-escaping entry paths
        let Some(rel) = entry.enclosed_name() else {
            let bad = std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "archive entry escapes the extraction directory",
            );
            return Err(Error::io(zip_path, bad));
        };
        let dst = target.join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&dst).map_err(|source| Error::io(&dst, source))?;
            continue;
        }
        let Some(dir) = dst.parent() else { continue };
        std::fs::create_dir_all(dir).map_err(|source| Error::io(dir, source))?;
        let mut out = std::fs::File::create(&dst).map_err(|source| Error::io(&dst, source))?;
        // CRC mismatches surface here as read errors
        std::io::copy(&mut entry, &mut out).map_err(|source| Error::io(&dst, source))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CaseMeta, EntityKind, EntityRec};
    use crate::project::io;

    fn fresh_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("redstring_bundle_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn seeded_project(name: &str) -> Project {
        let p = Project::create(&fresh_dir(name)).unwrap();
        let img = p.resolve("entities/doe/portrait.png");
        std::fs::create_dir_all(img.parent().unwrap()).unwrap();
        std::fs::write(&img, b"fakepng").unwrap();
        std::fs::create_dir_all(p.resolve("extra")).unwrap();
        std::fs::write(p.resolve("extra/raw-big.mp4"), vec![0u8; 4096]).unwrap();
        std::fs::write(p.root().join(".rsignore"), "extra/raw-*.mp4\n").unwrap();

        let mut cf = CaseFile::default();
        cf.meta = CaseMeta::stamped(name);
        cf.data.entities.push(EntityRec {
            slug: "doe".into(),
            kind: EntityKind::Person,
            label: "J. Doe".into(),
            attrs: vec![],
            image: Some("entities/doe/portrait.png".into()),
        });
        io::write_case(&p, &cf).unwrap();
        io::write_note(&p, "doe", "the guy\n").unwrap();
        p
    }

    #[test]
    fn export_then_open_round_trip() {
        let p = seeded_project("trip");
        let (zip_path, n) = export(&p).unwrap();
        assert!(zip_path.ends_with(format!("{}{BUNDLE_SUFFIX}", p.dir_name())));
        // case + readme + rsignore + portrait + note are in; the ignored mp4 is out
        assert_eq!(n, 5);

        // opening it where it sits — inside its own case dir — is refused
        assert!(matches!(
            open_bundle(&zip_path),
            Err(Error::BundleInsideProject { .. })
        ));

        // received-bundle scenario: the zip sits in some download dir
        let inbox = fresh_dir("trip_inbox");
        std::fs::create_dir_all(&inbox).unwrap();
        let received = inbox.join(zip_path.file_name().unwrap());
        std::fs::copy(&zip_path, &received).unwrap();

        let opened = open_bundle(&received).unwrap();
        assert_eq!(io::read_case(&opened).unwrap().meta.name, "trip");
        assert_eq!(io::read_note(&opened, "doe").unwrap(), "the guy\n");
        assert!(opened.resolve("entities/doe/portrait.png").exists());
        assert!(!opened.resolve("extra/raw-big.mp4").exists()); // ignored stayed home

        // a second open refuses to clobber the extracted dir
        assert!(matches!(
            open_bundle(&received),
            Err(Error::TargetExists { .. })
        ));

        std::fs::remove_dir_all(p.root()).unwrap();
        std::fs::remove_dir_all(&inbox).unwrap();
    }

    #[test]
    fn integrity_catches_missing_citations() {
        let p = seeded_project("integrity");
        // cite a file, then remove it from disk before export — the case
        // lies, the bundle must not pretend otherwise on open
        std::fs::remove_file(p.resolve("entities/doe/portrait.png")).unwrap();
        let (zip_path, _) = export(&p).unwrap();

        let inbox = fresh_dir("integrity_inbox");
        std::fs::create_dir_all(&inbox).unwrap();
        let received = inbox.join(zip_path.file_name().unwrap());
        std::fs::copy(&zip_path, &received).unwrap();

        assert!(matches!(
            open_bundle(&received),
            Err(Error::BundleMissingRef { .. })
        ));

        std::fs::remove_dir_all(p.root()).unwrap();
        std::fs::remove_dir_all(&inbox).unwrap();
    }
}

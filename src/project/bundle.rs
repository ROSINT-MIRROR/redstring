





use std::path::{Path, PathBuf};

use zip::write::SimpleFileOptions;

use crate::error::Error;
use crate::model::{CaseMeta, FORMAT_VERSION};

use super::ignore::RsIgnore;
use super::io::read_case;
use super::layout::{CASE_FILE, Project};
use super::validate::cited_paths;

pub const BUNDLE_SUFFIX: &str = ".redstring.zip";




pub fn export(project: &Project) -> Result<(PathBuf, usize), Error> {
    let ignore = RsIgnore::load(&project.root)?;
    let out = project
        .root
        .join(format!("{}{BUNDLE_SUFFIX}", project.dir_name()?));

    let mut files: Vec<String> = Vec::new();
    collect_files(&project.root, "", &ignore, &mut files)?;
    files.sort();

    let f = std::fs::File::create(&out).map_err(|source| Error::Io { path: out.to_path_buf(), source })?;
    let mut zip = zip::ZipWriter::new(f);
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for rel in &files {
        zip.start_file(rel, opts)
            .map_err(|source| Error::Zip { path: out.to_path_buf(), source })?;
        let abs = project.resolve(rel)?;
        let mut src = std::fs::File::open(&abs).map_err(|source| Error::Io { path: abs.to_path_buf(), source })?;
        std::io::copy(&mut src, &mut zip).map_err(|source| Error::Io { path: abs.to_path_buf(), source })?;
    }
    zip.finish().map_err(|source| Error::Zip { path: out.to_path_buf(), source })?;
    Ok((out, files.len()))
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
    use crate::model::{CaseFile, CaseMeta, EntityKind, EntityRec, Role};
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
            observations: vec![],
            role: Role::Unset,
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
        assert!(zip_path.ends_with(format!("{}{BUNDLE_SUFFIX}", p.dir_name().unwrap())));


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
        let (zip_path, _) = export(&p).unwrap();

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
}

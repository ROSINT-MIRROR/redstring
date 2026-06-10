





use std::path::{Path, PathBuf};

use crate::error::Error;
use crate::model::SourceKind;

use super::layout::Project;



pub fn import_entity_image(project: &Project, slug: &str, src: &Path) -> Result<PathBuf, Error> {
    copy_unique(&project.entity_dir(slug)?, src)
}

pub fn import_source_file(project: &Project, kind: SourceKind, src: &Path) -> Result<String, Error> {
    let dir = project.resolve(dest_subdir(kind, src))?;
    let abs = copy_unique(&dir, src)?;
    project.rel_or_self(&abs.to_string_lossy())
}


fn dest_subdir(kind: SourceKind, src: &Path) -> &'static str {
    match kind {
        SourceKind::Document => doc_or_media(src),
        SourceKind::Archive => "evidence/archives",
        SourceKind::Transcript => "evidence/transcripts",
        SourceKind::Markdown => "notes",
        
        SourceKind::Url | SourceKind::Note => "evidence/docs",
    }
}

fn doc_or_media(src: &Path) -> &'static str {
    const MEDIA: &[&str] = &[
        "png", "jpg", "jpeg", "webp", "gif", "bmp", "svg", "mp4", "mkv", "mov", "webm", "avi",
        "mp3", "wav", "ogg", "flac", "m4a",
    ];
    let ext = src
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase());
    let is_media = ext.is_some_and(|e| MEDIA.contains(&e.as_str()));
    if is_media { "evidence/media" } else { "evidence/docs" }
}




fn copy_unique(dst_dir: &Path, src: &Path) -> Result<PathBuf, Error> {
    std::fs::create_dir_all(dst_dir).map_err(|source| Error::Io { path: dst_dir.to_path_buf(), source })?;
    let canon_src = std::fs::canonicalize(src).map_err(|source| Error::Io { path: src.to_path_buf(), source })?;
    let canon_dst = std::fs::canonicalize(dst_dir).map_err(|source| Error::Io { path: dst_dir.to_path_buf(), source })?;
    if canon_src.parent() == Some(canon_dst.as_path()) {
        return Ok(canon_src);
    }

    let Some(name) = canon_src.file_name().and_then(|n| n.to_str()) else {
        let bad = std::io::Error::new(std::io::ErrorKind::InvalidInput, "unusable file name");
        return Err(Error::Io { path: src.to_path_buf(), source: bad });
    };
    let (stem, dot_ext) = split_name(name);
    let mut n = 1usize;
    loop {
        let candidate = if n == 1 {
            name.to_owned()
        } else {
            format!("{stem}-{n}{dot_ext}")
        };
        let dst = canon_dst.join(&candidate);
        if !dst.exists() {
            std::fs::copy(&canon_src, &dst).map_err(|source| Error::Io { path: dst.to_path_buf(), source })?;
            return Ok(dst);
        }
        n += 1;
    }
}


fn split_name(name: &str) -> (&str, &str) {
    let Some(dot) = name.rfind('.').filter(|&i| i > 0) else {
        return (name, "");
    };
    (&name[..dot], &name[dot..])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(name: &str) -> Project {
        let d = std::env::temp_dir().join(format!("redstring_attach_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        Project::create(&d).unwrap()
    }

    #[test]
    fn copy_in_with_collision_suffix() {
        let p = project("copy");
        let outside = std::env::temp_dir().join(format!("rs_attach_src_{}.png", std::process::id()));
        std::fs::write(&outside, b"img1").unwrap();

        let first = import_entity_image(&p, "doe", &outside).unwrap();
        assert_eq!(first.strip_prefix(&p.root).unwrap().to_string_lossy(), "entities/doe/".to_owned() + outside.file_name().unwrap().to_str().unwrap());

        
        std::fs::write(&outside, b"img2").unwrap();
        let second = import_entity_image(&p, "doe", &outside).unwrap();
        assert_ne!(first, second);
        assert!(second.to_string_lossy().contains("-2"));

        
        let again = import_entity_image(&p, "doe", &first).unwrap();
        assert_eq!(again, std::fs::canonicalize(&first).unwrap());

        std::fs::remove_file(&outside).unwrap();
        std::fs::remove_dir_all(p.root).unwrap();
    }

    #[test]
    fn source_routing_by_kind_and_extension() {
        let p = project("route");
        let doc = std::env::temp_dir().join(format!("rs_route_{}.pdf", std::process::id()));
        let img = std::env::temp_dir().join(format!("rs_route_{}.jpg", std::process::id()));
        std::fs::write(&doc, b"pdf").unwrap();
        std::fs::write(&img, b"jpg").unwrap();

        let r = import_source_file(&p, SourceKind::Document, &doc).unwrap();
        assert!(r.starts_with("evidence/docs/"), "{r}");
        let r = import_source_file(&p, SourceKind::Document, &img).unwrap();
        assert!(r.starts_with("evidence/media/"), "{r}");
        let r = import_source_file(&p, SourceKind::Transcript, &doc).unwrap();
        assert!(r.starts_with("evidence/transcripts/"), "{r}");
        let r = import_source_file(&p, SourceKind::Markdown, &doc).unwrap();
        assert!(r.starts_with("notes/"), "{r}");

        std::fs::remove_file(&doc).unwrap();
        std::fs::remove_file(&img).unwrap();
        std::fs::remove_dir_all(p.root).unwrap();
    }
}

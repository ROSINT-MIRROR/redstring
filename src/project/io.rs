//! Reading and writing the case file and the per-entity note files. The
//! format gate lives here: anything stamped newer than this build refuses
//! to load instead of silently dropping fields.

use crate::error::{Error, RsResult};
use crate::model::{CaseFile, FORMAT_VERSION};

use super::layout::Project;

pub fn read_case(project: &Project) -> RsResult<CaseFile> {
    let path = project.case_path();
    let text = std::fs::read_to_string(&path).map_err(|source| Error::io(&path, source))?;
    let cf: CaseFile = ron::from_str(&text).map_err(|source| Error::ron_parse(&path, source))?;
    if cf.meta.format > FORMAT_VERSION {
        return Err(Error::FormatTooNew {
            found: cf.meta.format,
            supported: FORMAT_VERSION,
        });
    }
    Ok(cf)
}

pub fn write_case(project: &Project, cf: &CaseFile) -> RsResult<()> {
    let path = project.case_path();
    let text = ron::ser::to_string_pretty(cf, ron::ser::PrettyConfig::default())
        .map_err(|source| Error::RonSer { source })?;
    std::fs::write(&path, text).map_err(|source| Error::io(&path, source))?;
    Ok(())
}

/// The entity's markdown note. A missing file is simply an empty note.
pub fn read_note(project: &Project, slug: &str) -> RsResult<String> {
    let path = project.note_path(slug);
    match std::fs::read_to_string(&path) {
        Ok(s) => Ok(s),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            // no file means no note yet — empty is the truthful value here
            Ok(String::new())
        }
        Err(source) => Err(Error::io(&path, source)),
    }
}

/// Write the entity's note file. An empty note truncates an existing file —
/// clearing a note is a real edit — but never creates one.
pub fn write_note(project: &Project, slug: &str, note: &str) -> RsResult<()> {
    let path = project.note_path(slug);
    if note.trim().is_empty() {
        if !path.exists() {
            return Ok(());
        }
        return std::fs::write(&path, "").map_err(|source| Error::io(&path, source));
    }
    let Some(dir) = path.parent() else {
        return Ok(());
    };
    std::fs::create_dir_all(dir).map_err(|source| Error::io(dir, source))?;
    std::fs::write(&path, note).map_err(|source| Error::io(&path, source))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::CaseMeta;

    fn project(name: &str) -> Project {
        let d = std::env::temp_dir().join(format!("redstring_io_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        Project::create(&d).unwrap()
    }

    #[test]
    fn case_round_trip_and_format_gate() {
        let p = project("gate");
        let mut cf = CaseFile::default();
        cf.meta = CaseMeta::stamped("gate test");
        write_case(&p, &cf).unwrap();
        let back = read_case(&p).unwrap();
        assert_eq!(back.meta.name, "gate test");

        // a file from the future refuses to load
        cf.meta.format = FORMAT_VERSION + 1;
        write_case(&p, &cf).unwrap();
        assert!(matches!(
            read_case(&p),
            Err(Error::FormatTooNew { .. })
        ));
        std::fs::remove_dir_all(p.root()).unwrap();
    }

    #[test]
    fn notes_lifecycle() {
        let p = project("notes");
        // missing file reads as empty
        assert_eq!(read_note(&p, "doe").unwrap(), "");
        // empty note never creates a file
        write_note(&p, "doe", "").unwrap();
        assert!(!p.note_path("doe").exists());
        // content creates entities/doe/note.md
        write_note(&p, "doe", "shady guy\n").unwrap();
        assert_eq!(read_note(&p, "doe").unwrap(), "shady guy\n");
        // clearing truncates but keeps the file
        write_note(&p, "doe", "").unwrap();
        assert!(p.note_path("doe").exists());
        assert_eq!(read_note(&p, "doe").unwrap(), "");
        std::fs::remove_dir_all(p.root()).unwrap();
    }
}

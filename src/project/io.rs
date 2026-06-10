



use crate::error::{Error, RsResult};
use crate::model::{CaseFile, FORMAT_VERSION};

use super::layout::Project;

pub fn read_case(project: &Project) -> RsResult<CaseFile> {
    let path = project.case_path()?;
    let text = std::fs::read_to_string(&path).map_err(|source| Error::Io { path: path.to_path_buf(), source })?;
    let cf: CaseFile = ron::from_str(&text).map_err(|source| Error::RonParse { path: path.to_path_buf(), source })?;
    if cf.meta.format > FORMAT_VERSION {
        return Err(Error::FormatTooNew {
            found: cf.meta.format,
            supported: FORMAT_VERSION,
        });
    }
    Ok(cf)
}

pub fn write_case(project: &Project, cf: &CaseFile) -> RsResult<()> {
    let path = project.case_path()?;
    let text = ron::ser::to_string_pretty(cf, ron::ser::PrettyConfig::default())
        .map_err(|source| Error::RonSer { source })?;
    std::fs::write(&path, text).map_err(|source| Error::Io { path: path.to_path_buf(), source })?;
    Ok(())
}


pub fn read_note(project: &Project, slug: &str) -> RsResult<String> {
    let path = project.note_path(slug)?;
    match std::fs::read_to_string(&path) {
        Ok(s) => Ok(s),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            
            Ok(String::new())
        }
        Err(source) => Err(Error::Io { path: path.to_path_buf(), source }),
    }
}



pub fn write_note(project: &Project, slug: &str, note: &str) -> RsResult<()> {
    let path = project.note_path(slug)?;
    if note.trim().is_empty() {
        if !path.exists() {
            return Ok(());
        }
        return std::fs::write(&path, "").map_err(|source| Error::Io { path: path.to_path_buf(), source });
    }
    let Some(dir) = path.parent() else {
        return Ok(());
    };
    std::fs::create_dir_all(dir).map_err(|source| Error::Io { path: dir.to_path_buf(), source })?;
    std::fs::write(&path, note).map_err(|source| Error::Io { path: path.to_path_buf(), source })?;
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
        cf.meta = CaseMeta::stamped("gate test").unwrap();
        write_case(&p, &cf).unwrap();
        let back = read_case(&p).unwrap();
        assert_eq!(back.meta.name, "gate test");

        
        cf.meta.format = FORMAT_VERSION + 1;
        write_case(&p, &cf).unwrap();
        assert!(matches!(
            read_case(&p),
            Err(Error::FormatTooNew { .. })
        ));
        std::fs::remove_dir_all(p.root).unwrap();
    }

    #[test]
    fn notes_lifecycle() {
        let p = project("notes");
        
        assert_eq!(read_note(&p, "doe").unwrap(), "");
        
        write_note(&p, "doe", "").unwrap();
        assert!(!p.note_path("doe").unwrap().exists());
        
        write_note(&p, "doe", "shady guy\n").unwrap();
        assert_eq!(read_note(&p, "doe").unwrap(), "shady guy\n");
        
        write_note(&p, "doe", "").unwrap();
        assert!(p.note_path("doe").unwrap().exists());
        assert_eq!(read_note(&p, "doe").unwrap(), "");
        std::fs::remove_dir_all(p.root).unwrap();
    }
}

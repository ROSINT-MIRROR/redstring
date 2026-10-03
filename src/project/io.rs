use std::collections::HashSet;
use std::io::ErrorKind;

use ron::ser::PrettyConfig;

use crate::error::Error;
use crate::model::{
    CaseDisplay, CaseFile, CaseMeta, EntityRec, FORMAT_VERSION, Observation, RelationRec,
};

use super::atomic::{is_hidden, remove_dir_atomic, write_atomic};
use super::layout::Project;

pub fn read_case(project: &Project) -> Result<CaseFile, Error> {
    let meta = read_meta(project)?;
    if meta.format > FORMAT_VERSION {
        return Err(Error::FormatTooNew {
            found: meta.format,
            supported: FORMAT_VERSION,
        });
    }
    let mut cf = CaseFile::default();
    cf.meta = meta;
    cf.display = read_display(project)?;
    cf.data.entities = read_entities(project)?;
    cf.data.relations = read_relations(project)?;
    Ok(cf)
}

fn read_meta(project: &Project) -> Result<CaseMeta, Error> {
    let path = project.case_path()?;
    let text = std::fs::read_to_string(&path)
        .map_err(|source| Error::Io { path: path.to_path_buf(), source })?;
    ron::from_str(&text).map_err(|source| Error::RonParse { path, source })
}

fn read_display(project: &Project) -> Result<CaseDisplay, Error> {
    let path = project.display_path()?;
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(source) if source.kind() == ErrorKind::NotFound => {
            return Ok(CaseDisplay::default());
        }
        Err(source) => return Err(Error::Io { path, source }),
    };
    ron::from_str(&text).map_err(|source| Error::RonParse { path, source })
}

fn read_relations(project: &Project) -> Result<Vec<RelationRec>, Error> {
    let path = project.relations_path()?;
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(source) if source.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => return Err(Error::Io { path, source }),
    };
    let mut out = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let rr = serde_json::from_str(line)
            .map_err(|source| Error::JsonParse { path: path.to_path_buf(), source })?;
        out.push(rr);
    }
    Ok(out)
}

pub fn entity_slugs(project: &Project) -> Result<Vec<String>, Error> {
    let dir = project.root.join("entities");
    let rd = match std::fs::read_dir(&dir) {
        Ok(r) => r,
        Err(source) if source.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => return Err(Error::Io { path: dir, source }),
    };
    let mut slugs = Vec::new();
    for entry in rd {
        let entry = entry.map_err(|source| Error::Io { path: dir.to_path_buf(), source })?;
        let ft = entry
            .file_type()
            .map_err(|source| Error::Io { path: entry.path(), source })?;
        if !ft.is_dir() {
            continue;
        }
        let Some(slug) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if is_hidden(&slug)? {
            continue;
        }
        slugs.push(slug);
    }
    slugs.sort();
    Ok(slugs)
}

fn read_entities(project: &Project) -> Result<Vec<EntityRec>, Error> {
    let mut out = Vec::new();
    for slug in entity_slugs(project)? {
        let head_path = project.entity_head_path(&slug)?;
        let text = match std::fs::read_to_string(&head_path) {
            Ok(t) => t,
            Err(source) if source.kind() == ErrorKind::NotFound => continue,
            Err(source) => return Err(Error::Io { path: head_path, source }),
        };
        let mut rec: EntityRec = ron::from_str(&text)
            .map_err(|source| Error::RonParse { path: head_path.to_path_buf(), source })?;
        rec.observations = read_obs(project, &slug)?;
        out.push(rec);
    }
    Ok(out)
}

fn read_obs(project: &Project, slug: &str) -> Result<Vec<Observation>, Error> {
    let path = project.obs_path(slug)?;
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(source) if source.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => return Err(Error::Io { path, source }),
    };
    let mut out = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        out.push(
            serde_json::from_str(line)
                .map_err(|source| Error::JsonParse { path: path.to_path_buf(), source })?,
        );
    }
    Ok(out)
}

pub fn write_case(project: &Project, cf: &CaseFile) -> Result<(), Error> {
    let mut keep = HashSet::new();
    for er in &cf.data.entities {
        keep.insert(er.slug.clone());
        write_entity(project, er)?;
    }
    write_atomic(&project.relations_path()?, relations_text(&cf.data.relations)?.as_bytes())?;
    write_atomic(&project.display_path()?, display_text(&cf.display)?.as_bytes())?;
    write_atomic(&project.case_path()?, meta_text(&cf.meta)?.as_bytes())?;
    prune_entities(project, &keep)
}

pub fn meta_text(meta: &CaseMeta) -> Result<String, Error> {
    ron::ser::to_string_pretty(meta, PrettyConfig::default()).map_err(|source| Error::RonSer { source })
}

pub fn display_text(display: &CaseDisplay) -> Result<String, Error> {
    ron::ser::to_string_pretty(display, PrettyConfig::default())
        .map_err(|source| Error::RonSer { source })
}

pub fn relations_text(relations: &[RelationRec]) -> Result<String, Error> {
    let mut text = String::new();
    for rr in relations {
        text.push_str(&serde_json::to_string(rr).map_err(|source| Error::JsonSer { source })?);
        text.push('\n');
    }
    Ok(text)
}

pub fn head_text(er: &EntityRec) -> Result<String, Error> {
    let mut head = er.clone();
    head.observations = Vec::new();
    ron::ser::to_string_pretty(&head, PrettyConfig::default()).map_err(|source| Error::RonSer { source })
}

pub fn obs_text(observations: &[Observation]) -> Result<String, Error> {
    let mut text = String::new();
    for o in observations {
        text.push_str(&serde_json::to_string(o).map_err(|source| Error::JsonSer { source })?);
        text.push('\n');
    }
    Ok(text)
}

pub fn write_entity(project: &Project, er: &EntityRec) -> Result<(), Error> {
    let dir = project.entity_dir(&er.slug)?;
    std::fs::create_dir_all(&dir).map_err(|source| Error::Io { path: dir.to_path_buf(), source })?;
    write_atomic(&project.obs_path(&er.slug)?, obs_text(&er.observations)?.as_bytes())?;
    write_atomic(&project.entity_head_path(&er.slug)?, head_text(er)?.as_bytes())
}

pub fn remove_entity(project: &Project, slug: &str) -> Result<(), Error> {
    remove_dir_atomic(&project.entity_dir(slug)?)
}

fn prune_entities(project: &Project, keep: &HashSet<String>) -> Result<(), Error> {
    for slug in entity_slugs(project)? {
        if keep.contains(&slug) {
            continue;
        }
        remove_entity(project, &slug)?;
    }
    Ok(())
}

pub fn read_note(project: &Project, slug: &str) -> Result<String, Error> {
    let path = project.note_path(slug)?;
    match std::fs::read_to_string(&path) {
        Ok(s) => Ok(s),
        Err(source) if source.kind() == ErrorKind::NotFound => {
            Ok(String::new())
        }
        Err(source) => Err(Error::Io { path: path.to_path_buf(), source }),
    }
}

pub fn write_note(project: &Project, slug: &str, note: &str) -> Result<(), Error> {
    let path = project.note_path(slug)?;
    if note.trim().is_empty() {
        if !path.exists() {
            return Ok(());
        }
        return write_atomic(&path, b"");
    }
    let Some(dir) = path.parent() else {
        return Ok(());
    };
    std::fs::create_dir_all(dir).map_err(|source| Error::Io { path: dir.to_path_buf(), source })?;
    write_atomic(&path, note.as_bytes())
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

    #[test]
    fn hidden_staging_and_deleted_dirs_never_load_as_entities() {
        let p = project("hidden");
        let mut cf = CaseFile::default();
        cf.meta = CaseMeta::stamped("hidden").unwrap();
        write_case(&p, &cf).unwrap();
        let ghost = p.root.join("entities").join(".rsgone-doe.1.2");
        std::fs::create_dir_all(&ghost).unwrap();
        std::fs::write(ghost.join("entity.ron"), "(slug: \"doe\", kind: Person, label: \"Doe\")")
            .unwrap();
        assert!(read_case(&p).unwrap().data.entities.is_empty(), "a hidden dir is not an entity");
        std::fs::remove_dir_all(p.root).unwrap();
    }
}

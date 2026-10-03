use std::collections::{BTreeMap, BTreeSet};
use std::fs::Metadata;
use std::time::UNIX_EPOCH;

use crate::error::Error;
use crate::model::{CaseFile, EntityRec};

use super::atomic::write_atomic;
use super::io::{
    display_text, entity_slugs, head_text, meta_text, obs_text, read_case, read_note,
    relations_text, remove_entity, write_note,
};
use super::layout::{CASE_FILE, DISPLAY_FILE, Project, RELATIONS_FILE};

#[derive(Clone, PartialEq, Debug, Default)]
pub struct CaseState {
    pub file: CaseFile,
    pub notes: BTreeMap<String, String>,
}

impl CaseState {
    pub fn note(&self, slug: &str) -> Result<String, Error> {
        let Some(n) = self.notes.get(slug) else {
            return Ok(String::new());
        };
        Ok(n.clone())
    }
}

fn find_entity<'a>(state: &'a CaseState, slug: &str) -> Option<&'a EntityRec> {
    state.file.data.entities.iter().find(|e| e.slug == slug)
}

pub fn read_state(project: &Project) -> Result<CaseState, Error> {
    if !project.case_path()?.is_file() {
        return Ok(CaseState::default());
    }
    let file = read_case(project)?;
    let mut notes = BTreeMap::new();
    for e in &file.data.entities {
        notes.insert(e.slug.clone(), read_note(project, &e.slug)?);
    }
    Ok(CaseState { file, notes })
}

pub fn note_on_disk(note: &str) -> Result<String, Error> {
    if note.trim().is_empty() {
        return Ok(String::new());
    }
    Ok(note.to_owned())
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Written {
    pub files: usize,
    pub removed: usize,
}

pub fn write_changes(project: &Project, before: &CaseState, after: &CaseState) -> Result<Written, Error> {
    let mut out = Written::default();
    let mut kept: BTreeSet<&str> = BTreeSet::new();
    for e in &after.file.data.entities {
        kept.insert(e.slug.as_str());
        out.files += write_entity_delta(project, before, after, e)?;
    }
    let rel_after = relations_text(&after.file.data.relations)?;
    if relations_text(&before.file.data.relations)? != rel_after {
        write_atomic(&project.relations_path()?, rel_after.as_bytes())?;
        out.files += 1;
    }
    let display_after = display_text(&after.file.display)?;
    if display_text(&before.file.display)? != display_after {
        write_atomic(&project.display_path()?, display_after.as_bytes())?;
        out.files += 1;
    }
    let meta_after = meta_text(&after.file.meta)?;
    if meta_text(&before.file.meta)? != meta_after || !project.case_path()?.is_file() {
        write_atomic(&project.case_path()?, meta_after.as_bytes())?;
        out.files += 1;
    }
    for e in &before.file.data.entities {
        if kept.contains(e.slug.as_str()) {
            continue;
        }
        remove_entity(project, &e.slug)?;
        out.removed += 1;
    }
    Ok(out)
}

fn write_entity_delta(
    project: &Project,
    before: &CaseState,
    after: &CaseState,
    e: &EntityRec,
) -> Result<usize, Error> {
    let Some(prior) = find_entity(before, &e.slug) else {
        return write_new_entity(project, after, e);
    };
    let mut files = 0usize;
    let obs_after = obs_text(&e.observations)?;
    if obs_text(&prior.observations)? != obs_after {
        write_atomic(&project.obs_path(&e.slug)?, obs_after.as_bytes())?;
        files += 1;
    }
    let head_after = head_text(e)?;
    if head_text(prior)? != head_after {
        write_atomic(&project.entity_head_path(&e.slug)?, head_after.as_bytes())?;
        files += 1;
    }
    let note_after = note_on_disk(&after.note(&e.slug)?)?;
    if note_on_disk(&before.note(&e.slug)?)? != note_after {
        write_note(project, &e.slug, &note_after)?;
        files += 1;
    }
    Ok(files)
}

fn write_new_entity(project: &Project, after: &CaseState, e: &EntityRec) -> Result<usize, Error> {
    let dir = project.entity_dir(&e.slug)?;
    std::fs::create_dir_all(&dir).map_err(|source| Error::Io { path: dir, source })?;
    write_atomic(&project.obs_path(&e.slug)?, obs_text(&e.observations)?.as_bytes())?;
    write_note(project, &e.slug, &note_on_disk(&after.note(&e.slug)?)?)?;
    write_atomic(&project.entity_head_path(&e.slug)?, head_text(e)?.as_bytes())?;
    Ok(3)
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct DiskSig {
    pub entries: Vec<(String, u64, u128, u64)>,
}

pub fn disk_sig(project: &Project) -> Result<DiskSig, Error> {
    let mut entries = Vec::new();
    for name in [CASE_FILE, DISPLAY_FILE, RELATIONS_FILE] {
        stamp(&project.root.join(name), name, &mut entries)?;
    }
    for slug in entity_slugs(project)? {
        entries.push((format!("entities/{slug}/"), 0, 0, 0));
        for (file, path) in [
            ("entity.ron", project.entity_head_path(&slug)?),
            ("observations.ndjson", project.obs_path(&slug)?),
            ("note.md", project.note_path(&slug)?),
        ] {
            stamp(&path, &format!("entities/{slug}/{file}"), &mut entries)?;
        }
    }
    entries.sort();
    Ok(DiskSig { entries })
}

fn stamp(
    path: &std::path::Path,
    rel: &str,
    entries: &mut Vec<(String, u64, u128, u64)>,
) -> Result<(), Error> {
    if !path.exists() {
        return Ok(());
    }
    let meta = std::fs::metadata(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let modified = meta.modified().map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let nanos = modified
        .duration_since(UNIX_EPOCH)
        .map_err(|source| Error::Clock { source })?
        .as_nanos();
    entries.push((rel.to_owned(), meta.len(), nanos, inode(&meta)?));
    Ok(())
}

#[cfg(unix)]
fn inode(meta: &Metadata) -> Result<u64, Error> {
    use std::os::unix::fs::MetadataExt;
    Ok(meta.ino())
}

#[cfg(not(unix))]
fn inode(meta: &Metadata) -> Result<u64, Error> {
    Ok(meta.len())
}

pub fn same_content(a: &CaseState, b: &CaseState) -> Result<bool, Error> {
    Ok(content_key(a)? == content_key(b)?)
}

fn content_key(s: &CaseState) -> Result<String, Error> {
    let mut lines: Vec<String> = Vec::new();
    lines.push(format!("name|{}", s.file.meta.name));
    for e in &s.file.data.entities {
        lines.push(format!(
            "e|{}|{}|{}|{:?}",
            e.slug,
            head_text(e)?,
            obs_text(&e.observations)?,
            note_on_disk(&s.note(&e.slug)?)?
        ));
    }
    lines.sort();
    lines.push(relations_text(&s.file.data.relations)?);
    let d = &s.file.display;
    let view = serde_json::to_string(&(&d.positions, &d.sizes, &d.collapsed, d.size_by_degree, d.layout))
        .map_err(|source| Error::JsonSer { source })?;
    lines.push(view);
    Ok(lines.join("\n"))
}

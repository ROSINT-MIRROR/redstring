//! A case is a directory. `case.redstring` at its root is the marker and the
//! single source of truth — the only file redstring parses. Everything the
//! graph references lives inside the directory at canonical places, cited by
//! project-relative path. That single rule makes a case relocatable,
//! git-friendly, bundleable, and servable as a static site.
//!
//! Canonical layout: entities/<slug>/ holds each entity's note.md, portrait
//! and anything else about it. The evidence/ tree splits into docs, media,
//! archives and transcripts and holds the proof; notes/ holds case-level
//! analysis; extra/ is the user's dumping ground that redstring never
//! parses. Directories appear on first use — git can't track empty ones.

use std::path::{Path, PathBuf};

use crate::error::{Error, RsResult};

/// Marker + main file name at the project root.
pub const CASE_FILE: &str = "case.redstring";

#[derive(Clone, Debug)]
pub struct Project {
    root: PathBuf,
}

impl Project {
    /// Open an existing case: accepts the case directory or the case file
    /// itself.
    pub fn open(path: &Path) -> RsResult<Self> {
        let dir = if path.is_dir() {
            path.to_path_buf()
        } else {
            case_file_parent(path)?
        };
        let root = std::fs::canonicalize(&dir).map_err(|source| Error::io(&dir, source))?;
        if !root.join(CASE_FILE).is_file() {
            return Err(Error::NotAProject { path: root });
        }
        Ok(Self { root })
    }

    /// Create a fresh case directory: mkdir plus a README scaffold. The case
    /// file itself lands on the first save.
    pub fn create(path: &Path) -> RsResult<Self> {
        std::fs::create_dir_all(path).map_err(|source| Error::io(path, source))?;
        let root = std::fs::canonicalize(path).map_err(|source| Error::io(path, source))?;
        let project = Self { root };
        let readme = project.root.join("README.md");
        if !readme.exists() {
            std::fs::write(&readme, scaffold_readme(&project.dir_name()))
                .map_err(|source| Error::io(&readme, source))?;
        }
        Ok(project)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn case_path(&self) -> PathBuf {
        self.root.join(CASE_FILE)
    }

    /// The directory name — doubles as the default bundle stem.
    pub fn dir_name(&self) -> String {
        let Some(name) = self.root.file_name() else {
            return "case".to_owned();
        };
        name.to_string_lossy().into_owned()
    }

    pub fn entity_dir(&self, slug: &str) -> PathBuf {
        self.root.join("entities").join(slug)
    }

    pub fn note_path(&self, slug: &str) -> PathBuf {
        self.entity_dir(slug).join("note.md")
    }

    /// Resolve a project-relative path to an absolute one. Joining an
    /// already-absolute path yields it unchanged, which is exactly the
    /// degraded behaviour we want for legacy absolute references.
    pub fn resolve(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    /// Make an absolute path project-relative when it lives under the root.
    pub fn relativize(&self, path: &Path) -> Option<String> {
        let Ok(rel) = path.strip_prefix(&self.root) else {
            return None;
        };
        Some(rel.to_string_lossy().into_owned())
    }

    /// Project-relative form when possible, the input untouched otherwise —
    /// the validator flags whatever stays absolute.
    pub fn rel_or_self(&self, path: &str) -> String {
        let Some(rel) = self.relativize(Path::new(path)) else {
            return path.to_owned();
        };
        rel
    }
}

/// The directory a case file sits in — only `case.redstring` counts.
fn case_file_parent(path: &Path) -> RsResult<PathBuf> {
    let named_case = path.file_name().is_some_and(|n| n == CASE_FILE);
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty());
    let (true, Some(dir)) = (named_case, parent) else {
        return Err(Error::NotAProject {
            path: path.to_path_buf(),
        });
    };
    Ok(dir.to_path_buf())
}

/// Written once at case creation, then user-owned forever — GitHub renders
/// it as the repo front page.
fn scaffold_readme(name: &str) -> String {
    format!(
        "# {name}\n\n\
         A [redstring] case. The graph lives in `case.redstring`; browse the rest:\n\n\
         - `graph.md` — the board, rendered right here by GitHub\n\
         - `entities/` — one dossier directory per entity\n\
         - `evidence/` — docs, media, archives, transcripts backing the edges\n\
         - `notes/` — case-level analysis\n\
         - `extra/` — unsorted material, never parsed\n\n\
         redstring scaffolds this file once and never touches it again.\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("redstring_layout_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn create_scaffolds_once_and_open_needs_marker() {
        let dir = tmp("create");
        // a dir without the marker is not a case
        std::fs::create_dir_all(&dir).unwrap();
        assert!(matches!(
            Project::open(&dir),
            Err(Error::NotAProject { .. })
        ));

        let p = Project::create(&dir).unwrap();
        let readme = p.root().join("README.md");
        let scaffold = std::fs::read_to_string(&readme).unwrap();
        assert!(scaffold.contains("case.redstring"));

        // user owns the README after creation — recreate must not clobber
        std::fs::write(&readme, "mine now").unwrap();
        let p = Project::create(&dir).unwrap();
        assert_eq!(std::fs::read_to_string(&readme).unwrap(), "mine now");

        // with the marker present, both dir and file forms open
        std::fs::write(p.case_path(), "(data: ())").unwrap();
        assert!(Project::open(&dir).is_ok());
        assert!(Project::open(&p.case_path()).is_ok());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn relativize_and_resolve() {
        let dir = tmp("rel");
        let p = Project::create(&dir).unwrap();
        let abs = p.resolve("entities/doe/portrait.png");
        assert_eq!(p.relativize(&abs).unwrap(), "entities/doe/portrait.png");
        assert_eq!(p.rel_or_self("/elsewhere/x.png"), "/elsewhere/x.png");
        assert!(p.relativize(Path::new("/elsewhere/x.png")).is_none());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

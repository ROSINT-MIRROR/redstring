use std::path::{Path, PathBuf};

use crate::error::{Error, RsResult};

pub const CASE_FILE: &str = "case.redstring";

const CANONICAL_DIRS: [&str; 7] = [
    "entities",
    "evidence/docs",
    "evidence/media",
    "evidence/archives",
    "evidence/transcripts",
    "notes",
    "extra",
];

#[derive(Clone, Debug)]
pub struct Project {
    pub root: PathBuf,
}

impl Project {
    pub fn open(path: &Path) -> RsResult<Self> {
        let dir = if path.is_dir() {
            path.to_path_buf()
        } else {
            case_file_parent(path)?
        };
        let root = std::fs::canonicalize(&dir).map_err(|source| Error::Io {
            path: dir.to_path_buf(),
            source,
        })?;
        if !root.join(CASE_FILE).is_file() {
            return Err(Error::NotAProject { path: root });
        }
        Ok(Self { root })
    }

    pub fn create(path: &Path) -> RsResult<Self> {
        std::fs::create_dir_all(path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let root = std::fs::canonicalize(path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let project = Self { root };
        project.scaffold_tree()?;
        let readme = project.root.join("README.md");
        if !readme.exists() {
            std::fs::write(&readme, scaffold_readme(&project.dir_name()?)).map_err(|source| {
                Error::Io {
                    path: readme.to_path_buf(),
                    source,
                }
            })?;
        }
        Ok(project)
    }

    fn scaffold_tree(&self) -> RsResult<()> {
        for dir in CANONICAL_DIRS {
            let d = self.root.join(dir);
            std::fs::create_dir_all(&d).map_err(|source| Error::Io {
                path: d.to_path_buf(),
                source,
            })?;
            let occupied = std::fs::read_dir(&d)
                .map_err(|source| Error::Io {
                    path: d.to_path_buf(),
                    source,
                })?
                .next()
                .is_some();
            if occupied {
                continue;
            }
            let keep = d.join(".gitkeep");
            std::fs::write(&keep, "").map_err(|source| Error::Io {
                path: keep.to_path_buf(),
                source,
            })?;
        }
        Ok(())
    }

    pub fn case_path(&self) -> RsResult<PathBuf> {
        Ok(self.root.join(CASE_FILE))
    }

    pub fn dir_name(&self) -> RsResult<String> {
        let Some(name) = self.root.file_name() else {
            return Ok("case".to_owned());
        };
        Ok(name.to_string_lossy().into_owned())
    }

    pub fn entity_dir(&self, slug: &str) -> RsResult<PathBuf> {
        Ok(self.root.join("entities").join(slug))
    }

    pub fn note_path(&self, slug: &str) -> RsResult<PathBuf> {
        Ok(self.entity_dir(slug)?.join("note.md"))
    }

    pub fn resolve(&self, rel: &str) -> RsResult<PathBuf> {
        Ok(self.root.join(rel))
    }

    pub fn rel_or_self(&self, path: &str) -> RsResult<String> {
        let Some(rel) = relativize(&self.root, Path::new(path)) else {
            return Ok(path.to_owned());
        };
        Ok(rel)
    }
}

fn relativize(root: &Path, path: &Path) -> Option<String> {
    let Ok(rel) = path.strip_prefix(root) else {
        return None;
    };
    Some(rel.to_string_lossy().into_owned())
}

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

        std::fs::create_dir_all(&dir).unwrap();
        assert!(matches!(
            Project::open(&dir),
            Err(Error::NotAProject { .. })
        ));

        let p = Project::create(&dir).unwrap();
        let readme = p.root.join("README.md");
        let scaffold = std::fs::read_to_string(&readme).unwrap();
        assert!(scaffold.contains("case.redstring"));

        for d in CANONICAL_DIRS {
            assert!(p.root.join(d).is_dir(), "{d} scaffolded");
            assert!(p.root.join(d).join(".gitkeep").is_file(), "{d}/.gitkeep");
        }

        std::fs::write(&readme, "mine now").unwrap();
        let p = Project::create(&dir).unwrap();
        assert_eq!(std::fs::read_to_string(&readme).unwrap(), "mine now");

        std::fs::write(p.case_path().unwrap(), "(data: ())").unwrap();
        assert!(Project::open(&dir).is_ok());
        assert!(Project::open(&p.case_path().unwrap()).is_ok());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn relativize_and_resolve() {
        let dir = tmp("rel");
        let p = Project::create(&dir).unwrap();
        let abs = p.resolve("entities/doe/portrait.png").unwrap();
        assert_eq!(
            relativize(&p.root, &abs).unwrap(),
            "entities/doe/portrait.png"
        );
        assert_eq!(p.rel_or_self("/elsewhere/x.png").unwrap(), "/elsewhere/x.png");
        assert!(relativize(&p.root, Path::new("/elsewhere/x.png")).is_none());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

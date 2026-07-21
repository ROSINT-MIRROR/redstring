use std::path::{Path, PathBuf};

use crate::error::Error;
use crate::model::EntityKind;
use crate::prefs::config_base;

use super::spec::ModuleSpec;
use super::builtin::BUILTIN;


#[derive(Clone, Debug)]
pub enum ModuleStatus {

    Active(PathBuf),

    Missing(Vec<String>),

    Broken(String),
}


pub struct ModuleEntry {
    pub spec: ModuleSpec,
    pub origin: String,
    pub status: ModuleStatus,
}


pub struct Registry {
    pub entries: Vec<ModuleEntry>,
}

impl Registry {

    pub fn load() -> Result<Self, Error> {
        let mut entries = Vec::new();
        for (origin, text) in BUILTIN {
            entries.push(entry_from_str(origin, text)?);
        }
        for (origin, text) in user_specs()? {
            entries.push(entry_from_str(&origin, &text)?);
        }
        Ok(Self { entries })
    }


    pub fn for_kind(&self, kind: EntityKind) -> Result<Vec<usize>, Error> {
        let mut out = Vec::new();
        for (i, e) in self.entries.iter().enumerate() {
            let ModuleStatus::Active(_) = &e.status else {
                continue;
            };
            if e.spec.kinds()?.contains(&kind) {
                out.push(i);
            }
        }
        Ok(out)
    }
}

fn entry_from_str(origin: &str, text: &str) -> Result<ModuleEntry, Error> {
    let spec: ModuleSpec = ron::from_str(text).map_err(|source| Error::RonParse {
        path: PathBuf::from(origin),
        source,
    })?;
    let problems = spec.validate()?;
    let status = module_status(&spec, problems);
    Ok(ModuleEntry {
        spec,
        origin: origin.to_owned(),
        status,
    })
}

fn module_status(spec: &ModuleSpec, problems: Vec<String>) -> ModuleStatus {
    if !problems.is_empty() {
        return ModuleStatus::Broken(problems.join("; "));
    }
    let bin_path = which(&spec.bin);
    let mut missing = Vec::new();
    if bin_path.is_none() {
        missing.push(spec.bin.clone());
    }
    for req in &spec.requires {
        if which(req).is_none() {
            missing.push(req.clone());
        }
    }
    let Some(path) = bin_path else {
        return ModuleStatus::Missing(missing);
    };
    if !missing.is_empty() {
        return ModuleStatus::Missing(missing);
    }
    ModuleStatus::Active(path)
}

fn user_specs() -> Result<Vec<(String, String)>, Error> {
    let dir = config_base()?.join("redstring").join("modules");
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => return Err(Error::Io { path: dir, source }),
    };
    let mut out = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| Error::Io {
            path: dir.clone(),
            source,
        })?;
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "ron") {
            continue;
        }
        let text = std::fs::read_to_string(&path).map_err(|source| Error::Io {
            path: path.clone(),
            source,
        })?;
        out.push((path.display().to_string(), text));
    }
    Ok(out)
}

fn which(bin: &str) -> Option<PathBuf> {
    if bin.contains('/') {
        let p = PathBuf::from(bin);
        if is_exec(&p) {
            return Some(p);
        }
        return None;
    }
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let cand = dir.join(bin);
        if is_exec(&cand) {
            return Some(cand);
        }
    }
    None
}

fn is_exec(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    let Ok(md) = std::fs::metadata(p) else {
        return false;
    };
    md.is_file() && md.permissions().mode() & 0o111 != 0
}

#[cfg(test)]
include!("registry_tests.rs");
#[cfg(test)]
include!("transforms_tests.rs");

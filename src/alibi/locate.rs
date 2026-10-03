use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::error::Error;

use super::config::{ALIBI_BIN, HOME_BINS, SearchEnv};

pub enum Located {
    Found(PathBuf),
    Missing(Vec<PathBuf>),
}

pub fn candidates(env: &SearchEnv) -> Result<Vec<PathBuf>, Error> {
    for pinned in env.pinned.iter() {
        return Ok(vec![pinned.clone()]);
    }
    let mut out: Vec<PathBuf> = Vec::new();
    for dir in &env.path {
        push_new(&mut out, dir.join(ALIBI_BIN));
    }
    for cargo_home in env.cargo_home.iter() {
        push_new(&mut out, cargo_home.join("bin").join(ALIBI_BIN));
    }
    for home in env.home.iter() {
        for rel in HOME_BINS {
            push_new(&mut out, home.join(rel).join(ALIBI_BIN));
        }
    }
    Ok(out)
}

pub fn locate(env: &SearchEnv) -> Result<Located, Error> {
    let tried = candidates(env)?;
    for cand in &tried {
        if executable(cand)? {
            return Ok(Located::Found(cand.clone()));
        }
    }
    Ok(Located::Missing(tried))
}

fn executable(path: &Path) -> Result<bool, Error> {
    if !path.is_file() {
        return Ok(false);
    }
    let meta = std::fs::metadata(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(meta.permissions().mode() & 0o111 != 0)
}

fn push_new(out: &mut Vec<PathBuf>, cand: PathBuf) {
    if cand.is_absolute() && !out.contains(&cand) {
        out.push(cand);
    }
}

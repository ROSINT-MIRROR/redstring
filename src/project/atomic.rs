use std::fs::File;
use std::io::{Error as IoError, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::dlog_warn;
use crate::error::Error;

pub const TMP_SUFFIX: &str = ".rstmp";
pub const GONE_PREFIX: &str = ".rsgone-";

static SEQ: AtomicU64 = AtomicU64::new(0);

pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    let tmp = staging_path(path)?;
    let landed = stage(&tmp, bytes).and_then(|()| {
        std::fs::rename(&tmp, path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })
    });
    match landed {
        Ok(()) => Ok(()),
        Err(err) => {
            discard(&tmp);
            Err(err)
        }
    }
}

pub fn remove_dir_atomic(dir: &Path) -> Result<(), Error> {
    if !dir.exists() {
        return Ok(());
    }
    let gone = gone_path(dir)?;
    std::fs::rename(dir, &gone).map_err(|source| Error::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    std::fs::remove_dir_all(&gone).map_err(|source| Error::Io { path: gone, source })
}

pub fn is_hidden(name: &str) -> Result<bool, Error> {
    Ok(name.starts_with('.'))
}

pub fn sweep_stale(root: &Path) -> Result<usize, Error> {
    let mut swept = sweep_temps(root)?;
    let entities = root.join("entities");
    if !entities.is_dir() {
        return Ok(swept);
    }
    for entry in read_dir(&entities)? {
        let entry = entry.map_err(|source| Error::Io {
            path: entities.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if name_of(&path)?.starts_with(GONE_PREFIX) {
            std::fs::remove_dir_all(&path).map_err(|source| Error::Io { path, source })?;
            swept += 1;
            continue;
        }
        swept += sweep_temps(&path)?;
    }
    Ok(swept)
}

fn sweep_temps(dir: &Path) -> Result<usize, Error> {
    let mut swept = 0usize;
    for entry in read_dir(dir)? {
        let entry = entry.map_err(|source| Error::Io {
            path: dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let name = name_of(&path)?;
        if !(name.starts_with('.') && name.ends_with(TMP_SUFFIX)) {
            continue;
        }
        std::fs::remove_file(&path).map_err(|source| Error::Io { path, source })?;
        swept += 1;
    }
    Ok(swept)
}

fn read_dir(dir: &Path) -> Result<std::fs::ReadDir, Error> {
    std::fs::read_dir(dir).map_err(|source| Error::Io {
        path: dir.to_path_buf(),
        source,
    })
}

fn stage(tmp: &Path, bytes: &[u8]) -> Result<(), Error> {
    let io = |source| Error::Io {
        path: tmp.to_path_buf(),
        source,
    };
    let mut f = File::create_new(tmp).map_err(io)?;
    f.write_all(bytes).map_err(io)?;
    f.sync_all().map_err(io)
}

fn discard(tmp: &Path) {
    if !tmp.exists() {
        return;
    }
    match std::fs::remove_file(tmp) {
        Ok(()) => {}
        Err(source) => {
            dlog_warn!("atomic", "temp {} survived a failed write: {source}", tmp.display());
        }
    }
}

fn name_of(path: &Path) -> Result<String, Error> {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return Err(Error::Io {
            path: path.to_path_buf(),
            source: IoError::new(ErrorKind::InvalidData, "file name is not utf-8"),
        });
    };
    Ok(name.to_owned())
}

fn staging_path(path: &Path) -> Result<PathBuf, Error> {
    let Some(dir) = path.parent() else {
        return Err(Error::Io {
            path: path.to_path_buf(),
            source: IoError::new(ErrorKind::InvalidInput, "no parent directory"),
        });
    };
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    Ok(dir.join(format!(".{}.{pid}.{seq}{TMP_SUFFIX}", name_of(path)?)))
}

fn gone_path(dir: &Path) -> Result<PathBuf, Error> {
    let Some(parent) = dir.parent() else {
        return Err(Error::Io {
            path: dir.to_path_buf(),
            source: IoError::new(ErrorKind::InvalidInput, "no parent directory"),
        });
    };
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    Ok(parent.join(format!("{GONE_PREFIX}{}.{pid}.{seq}", name_of(dir)?)))
}

#[cfg(test)]
mod tests {
    use std::io::Read;
    use std::os::unix::fs::MetadataExt;

    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("redstring_atomic_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn listing(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        v.sort();
        v
    }

    #[test]
    fn a_reader_holding_the_old_file_never_sees_a_truncated_one() {
        let d = tmp("torn");
        let p = d.join("relations.ndjson");
        std::fs::write(&p, "line one\nline two\n").unwrap();
        let ino_before = std::fs::metadata(&p).unwrap().ino();
        let mut reader = std::fs::File::open(&p).unwrap();

        write_atomic(&p, b"replacement\n").unwrap();

        let mut old = String::new();
        reader.read_to_string(&mut old).unwrap();
        assert_eq!(
            old, "line one\nline two\n",
            "an in-place truncate would hand this reader an empty or partial file"
        );
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "replacement\n");
        assert_ne!(
            std::fs::metadata(&p).unwrap().ino(),
            ino_before,
            "the new content arrives as a new inode, swapped in by rename"
        );
        assert_eq!(listing(&d), vec!["relations.ndjson"], "no temp file is left behind");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn a_failed_write_leaves_no_temp_and_reports_the_failure() {
        let d = tmp("fail");
        let target = d.join("display.ron");
        std::fs::create_dir_all(target.join("in_the_way")).unwrap();
        assert!(
            write_atomic(&target, b"new").is_err(),
            "renaming a file over a non-empty directory must fail loudly"
        );
        assert_eq!(listing(&d), vec!["display.ron"], "the staged temp was cleaned up");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn a_removed_dir_vanishes_in_one_step_and_a_missing_one_is_fine() {
        let d = tmp("rmdir");
        let e = d.join("entities").join("doe");
        std::fs::create_dir_all(&e).unwrap();
        std::fs::write(e.join("entity.ron"), "()").unwrap();
        remove_dir_atomic(&e).unwrap();
        assert!(listing(&d.join("entities")).is_empty(), "nothing hidden is left over");
        remove_dir_atomic(&e).unwrap();
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn sweep_clears_interrupted_temps_and_deletes_but_nothing_else() {
        let d = tmp("sweep");
        let e = d.join("entities").join("doe");
        std::fs::create_dir_all(&e).unwrap();
        std::fs::write(e.join("entity.ron"), "()").unwrap();
        std::fs::write(e.join(".entity.ron.1.2.rstmp"), "half").unwrap();
        std::fs::write(d.join(".relations.ndjson.1.3.rstmp"), "half").unwrap();
        std::fs::write(d.join(".redstring.lock"), "").unwrap();
        std::fs::create_dir_all(d.join("entities").join(".rsgone-old.1.4")).unwrap();
        assert_eq!(sweep_stale(&d).unwrap(), 3);
        assert_eq!(listing(&e), vec!["entity.ron"]);
        assert_eq!(listing(&d.join("entities")), vec!["doe"]);
        assert_eq!(
            listing(&d),
            vec![".redstring.lock", "entities"],
            "the lock file is not a leftover"
        );
        std::fs::remove_dir_all(&d).unwrap();
    }
}

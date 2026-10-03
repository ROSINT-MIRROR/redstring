use std::fs::{File, OpenOptions, TryLockError};
use std::io::{Error as IoError, ErrorKind};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::error::Error;

use super::layout::Project;

pub const LOCK_FILE: &str = ".redstring.lock";

pub struct CaseLock {
    _held: File,
}

pub enum Claim {
    Held(CaseLock),
    Busy,
}

pub fn lock_path(project: &Project) -> Result<PathBuf, Error> {
    Ok(project.root.join(LOCK_FILE))
}

pub fn try_lock(project: &Project) -> Result<Claim, Error> {
    let path = lock_path(project)?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
    match file.try_lock() {
        Ok(()) => Ok(Claim::Held(CaseLock { _held: file })),
        Err(TryLockError::WouldBlock) => Ok(Claim::Busy),
        Err(TryLockError::Error(source)) => Err(Error::Io { path, source }),
    }
}

pub fn wait_lock(project: &Project, patience: Duration) -> Result<CaseLock, Error> {
    let start = Instant::now();
    loop {
        if let Claim::Held(lock) = try_lock(project)? {
            return Ok(lock);
        }
        if start.elapsed() >= patience {
            return Err(Error::Io {
                path: lock_path(project)?,
                source: IoError::new(
                    ErrorKind::WouldBlock,
                    "another redstring process is writing this case",
                ),
            });
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(name: &str) -> Project {
        let d = std::env::temp_dir().join(format!("redstring_lock_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        Project::create(&d).unwrap()
    }

    #[test]
    fn one_holder_at_a_time_and_release_on_drop() {
        let p = project("hold");
        let first = wait_lock(&p, Duration::from_millis(60)).expect("an idle case locks");
        assert!(
            matches!(try_lock(&p).unwrap(), Claim::Busy),
            "a second holder is refused while the first lives"
        );
        assert!(
            wait_lock(&p, Duration::from_millis(60)).is_err(),
            "a bounded wait gives up instead of hanging the caller"
        );
        drop(first);
        assert!(
            matches!(try_lock(&p).unwrap(), Claim::Held(_)),
            "dropping the guard releases the case"
        );
        std::fs::remove_dir_all(&p.root).unwrap();
    }
}

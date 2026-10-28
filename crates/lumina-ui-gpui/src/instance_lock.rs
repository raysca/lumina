//! Single-instance guard.
//!
//! Two Lumina processes sharing one catalog can interleave writes and leave the
//! UI state inconsistent, so only one instance may run at a time. An advisory
//! OS file lock is used: it is released automatically when the process exits.

use anyhow::{Context, Result};
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};

pub struct InstanceLock {
    _file: File,
    path: PathBuf,
}

impl InstanceLock {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

pub fn lock_path() -> Result<PathBuf> {
    let base = dirs::data_dir().unwrap_or_else(std::env::temp_dir);
    let dir = base.join("lumina");
    fs::create_dir_all(&dir).with_context(|| format!("could not create {dir:?}"))?;
    Ok(dir.join("lumina.lock"))
}

pub fn acquire() -> Result<InstanceLock> {
    acquire_at(&lock_path()?)
}

pub fn acquire_at(path: &Path) -> Result<InstanceLock> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)
        .with_context(|| format!("could not open instance lock at {path:?}"))?;
    file.try_lock().map_err(|err| match err {
        std::fs::TryLockError::WouldBlock => anyhow::anyhow!(
            "Another Lumina instance is already running. Close it before opening a second copy."
        ),
        std::fs::TryLockError::Error(io_err) => {
            anyhow::Error::from(io_err).context("could not lock the Lumina instance file")
        }
    })?;
    Ok(InstanceLock {
        _file: file,
        path: path.to_path_buf(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_instance_is_rejected_and_lock_releases_on_drop() {
        let dir = std::env::temp_dir().join(format!(
            "lumina_instance_lock_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("lumina.lock");

        let first = acquire_at(&path).expect("first instance should acquire the lock");
        assert!(
            acquire_at(&path).is_err(),
            "second instance must be rejected"
        );
        drop(first);
        let _second = acquire_at(&path).expect("lock must be released when the first drops");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

//! Atomic file writes and rolling snapshots in Application Support.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config;
use crate::error::AppError;

const BACKUPS_DIR_NAME: &str = "backups";
const MAX_BACKUP_SNAPSHOTS: usize = 25;

/// Resolves the rolling backups directory (`~/Library/Application Support/AgentDock/backups`).
///
/// # Errors
/// Returns [`AppError::DirectoryUnavailable`] if Application Support cannot be located.
pub fn backups_dir() -> Result<PathBuf, AppError> {
    Ok(config::app_support_dir()?.join(BACKUPS_DIR_NAME))
}

/// Copies `target` into the default rolling backups directory before modification, keeping the newest 25 snapshots.
///
/// # Errors
/// Returns [`AppError`] if reading, copying, or pruning backups fails.
pub fn backup_file(target: &Path) -> Result<Option<PathBuf>, AppError> {
    let dir = backups_dir()?;
    backup_file_in_dir(target, &dir)
}

/// Copies `target` into `backups_root` before modification, retaining the latest 25 snapshots.
///
/// # Errors
/// Returns [`AppError`] on filesystem errors.
pub fn backup_file_in_dir(target: &Path, backups_root: &Path) -> Result<Option<PathBuf>, AppError> {
    let Ok(meta) = fs::symlink_metadata(target) else {
        return Ok(None);
    };
    if !meta.file_type().is_file() {
        return Ok(None);
    }

    fs::create_dir_all(backups_root).map_err(|e| AppError::io(backups_root, e))?;

    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let raw_name = target
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file");
    let sanitized: String = raw_name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();

    let mut dest = backups_root.join(format!("{ms:016}_{sanitized}"));
    let mut seq = 1usize;
    while dest.exists() {
        dest = backups_root.join(format!("{ms:016}_{seq:02}_{sanitized}"));
        seq += 1;
    }

    fs::copy(target, &dest).map_err(|e| AppError::io(&dest, e))?;
    prune_old_backups(backups_root, MAX_BACKUP_SNAPSHOTS)?;

    Ok(Some(dest))
}

fn prune_old_backups(backups_root: &Path, retain: usize) -> Result<(), AppError> {
    let entries = fs::read_dir(backups_root).map_err(|e| AppError::io(backups_root, e))?;
    let mut files: Vec<(String, PathBuf)> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Some(name) = entry.file_name().to_str() {
                files.push((name.to_owned(), path));
            }
        }
    }
    if files.len() <= retain {
        return Ok(());
    }

    files.sort_by(|a, b| a.0.cmp(&b.0));
    let to_remove = files.len().saturating_sub(retain);
    for (_, path) in files.into_iter().take(to_remove) {
        fs::remove_file(&path).map_err(|e| AppError::io(&path, e))?;
    }
    Ok(())
}

/// Atomically writes `contents` to `target` via a sibling temporary file and `fs::rename`.
///
/// # Errors
/// Returns [`AppError`] if directory creation, temp write, or rename fails.
pub fn atomic_write_string(target: &Path, contents: &str) -> Result<(), AppError> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| AppError::io(parent, e))?;
    }

    let file_name = target
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file");
    let pid = std::process::id();
    let tmp = target.with_file_name(format!(".{file_name}.agentdock.{pid}.tmp"));

    fs::write(&tmp, contents).map_err(|e| AppError::io(&tmp, e))?;
    if let Err(err) = fs::rename(&tmp, target) {
        let _ = fs::remove_file(&tmp);
        return Err(AppError::io(target, err));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn backs_up_and_prunes_beyond_25_snapshots() {
        let tmp = TempDir::new().expect("temp dir");
        let target = tmp.path().join("AGENTS.md");
        let backups = tmp.path().join("backups");

        for i in 0..30 {
            atomic_write_string(&target, &format!("version {i}")).expect("atomic write");
            backup_file_in_dir(&target, &backups).expect("backup");
        }

        let count = fs::read_dir(&backups)
            .expect("read backups")
            .flatten()
            .filter(|e| e.path().is_file())
            .count();
        assert_eq!(count, MAX_BACKUP_SNAPSHOTS);
        assert_eq!(
            fs::read_to_string(&target).expect("read final"),
            "version 29"
        );
    }

    #[test]
    fn missing_target_creates_no_backup() {
        let tmp = TempDir::new().expect("temp dir");
        let missing = tmp.path().join("does-not-exist.md");
        let backups = tmp.path().join("backups");
        let res = backup_file_in_dir(&missing, &backups).expect("backup missing");
        assert!(res.is_none());
        assert!(!backups.exists());
    }
}

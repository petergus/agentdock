//! Local-only `.git/info/exclude` management (never modifying `.gitignore`).

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::backup;
use crate::error::AppError;

const BEGIN_MARKER: &str = "# BEGIN AGENTDOCK";
const END_MARKER: &str = "# END AGENTDOCK";

/// Resolves `<project>/.git/info/exclude` if `project_path` is inside a Git working tree.
#[must_use]
pub fn resolve_git_exclude_path(project_path: &Path) -> Option<PathBuf> {
    let dot_git = project_path.join(".git");
    if dot_git.is_dir() {
        return Some(dot_git.join("info").join("exclude"));
    }
    if dot_git.is_file() {
        let content = fs::read_to_string(&dot_git).ok()?;
        let gitdir = content
            .lines()
            .find_map(|line| line.trim().strip_prefix("gitdir:"))?
            .trim();
        let resolved = if Path::new(gitdir).is_absolute() {
            PathBuf::from(gitdir)
        } else {
            project_path.join(gitdir)
        };
        if resolved.is_dir() {
            return Some(resolved.join("info").join("exclude"));
        }
    }
    None
}

/// Updates the isolated `# BEGIN AGENTDOCK` / `# END AGENTDOCK` block inside `.git/info/exclude`.
///
/// Silently returns `Ok(())` when `project_path` is not a Git repository.
///
/// # Errors
/// Returns [`AppError`] if reading, backing up, or writing `.git/info/exclude` fails.
pub fn sync_git_exclude(
    project_path: &Path,
    add_entries: &[String],
    remove_entries: &[String],
    custom_backups_root: Option<&Path>,
) -> Result<(), AppError> {
    let Some(exclude_path) = resolve_git_exclude_path(project_path) else {
        return Ok(());
    };

    let existing = if exclude_path.is_file() {
        fs::read_to_string(&exclude_path).map_err(|e| AppError::io(&exclude_path, e))?
    } else {
        String::new()
    };

    let (outside_lines, mut managed_set) = parse_exclude_sections(&existing);

    for rem in remove_entries {
        let norm = normalize_exclude_entry(rem);
        managed_set.remove(&norm);
    }
    for add in add_entries {
        let norm = normalize_exclude_entry(add);
        if !norm.is_empty() {
            managed_set.insert(norm);
        }
    }

    let next_content = render_exclude_sections(&outside_lines, &managed_set);
    if next_content == existing {
        return Ok(());
    }

    if exclude_path.is_file() {
        match custom_backups_root {
            Some(dir) => {
                backup::backup_file_in_dir(&exclude_path, dir)?;
            }
            None => {
                backup::backup_file(&exclude_path)?;
            }
        }
    }

    backup::atomic_write_string(&exclude_path, &next_content)
}

fn normalize_exclude_entry(raw: &str) -> String {
    raw.trim().trim_start_matches("./").to_owned()
}

fn parse_exclude_sections(content: &str) -> (Vec<String>, BTreeSet<String>) {
    let mut outside = Vec::new();
    let mut managed = BTreeSet::new();
    let mut inside_block = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == BEGIN_MARKER {
            inside_block = true;
            continue;
        }
        if trimmed == END_MARKER {
            inside_block = false;
            continue;
        }

        if inside_block {
            let norm = normalize_exclude_entry(trimmed);
            if !norm.is_empty() && !norm.starts_with('#') {
                managed.insert(norm);
            }
        } else {
            outside.push(line.to_owned());
        }
    }

    // Trim trailing blank lines outside the block so toggling on/off stays tidy.
    while outside.last().is_some_and(|l| l.trim().is_empty()) {
        outside.pop();
    }

    (outside, managed)
}

fn render_exclude_sections(outside_lines: &[String], managed: &BTreeSet<String>) -> String {
    let mut parts: Vec<String> = outside_lines.to_vec();

    if !managed.is_empty() {
        if !parts.is_empty() {
            parts.push(String::new());
        }
        parts.push(BEGIN_MARKER.to_owned());
        for entry in managed {
            parts.push(entry.clone());
        }
        parts.push(END_MARKER.to_owned());
    }

    if parts.is_empty() {
        String::new()
    } else {
        let mut joined = parts.join("\n");
        joined.push('\n');
        joined
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn manages_isolated_block_without_touching_user_lines_or_gitignore() {
        let tmp = TempDir::new().expect("temp dir");
        let project = tmp.path();
        let backups = tmp.path().join("backups");
        fs::create_dir_all(project.join(".git/info")).expect("create .git/info");
        let exclude = project.join(".git/info/exclude");
        fs::write(&exclude, "# user comment\n*.local\n").expect("seed exclude");
        fs::write(project.join(".gitignore"), "node_modules/\n").expect("seed gitignore");

        sync_git_exclude(
            project,
            &[
                ".agents/skills/my-skill".to_owned(),
                "_support-docs/".to_owned(),
            ],
            &[],
            Some(&backups),
        )
        .expect("add excludes");

        let after_add = fs::read_to_string(&exclude).expect("read exclude");
        assert!(after_add.contains("# user comment\n*.local\n"));
        assert!(after_add.contains(
            "# BEGIN AGENTDOCK\n.agents/skills/my-skill\n_support-docs/\n# END AGENTDOCK"
        ));
        assert_eq!(
            fs::read_to_string(project.join(".gitignore")).expect("gitignore"),
            "node_modules/\n"
        );

        // Remove one, keep one
        sync_git_exclude(
            project,
            &[],
            &[".agents/skills/my-skill".to_owned()],
            Some(&backups),
        )
        .expect("remove one");
        let after_partial = fs::read_to_string(&exclude).expect("read partial");
        assert!(!after_partial.contains("my-skill"));
        assert!(after_partial.contains("_support-docs/"));

        // Remove remaining -> block disappears cleanly
        sync_git_exclude(project, &[], &["_support-docs/".to_owned()], Some(&backups))
            .expect("remove last");
        let after_clear = fs::read_to_string(&exclude).expect("read clear");
        assert_eq!(after_clear, "# user comment\n*.local\n");
    }

    #[test]
    fn ignores_non_git_directories() {
        let tmp = TempDir::new().expect("temp dir");
        sync_git_exclude(tmp.path(), &[".agents/skills/test".to_owned()], &[], None)
            .expect("non-git should succeed as no-op");
        assert!(!tmp.path().join(".git").exists());
    }
}

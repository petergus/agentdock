//! Safe migration ("Move to Library & Unlink") for unmanaged real skill directories.

use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use crate::adapters::enabled_adapters;
use crate::error::AppError;
use crate::frontmatter::parse_markdown_frontmatter;
use crate::git_hygiene;
use crate::models::{
    AfterMoveAction, AppConfig, MoveExecutionResult, MovePreviewItem, ToggleScope,
};
use crate::symlinks::validate_entry_name;

/// Generates a dry-run preview for moving unmanaged skill folders into `target_library_path`.
///
/// # Errors
/// Returns [`AppError`] if `target_library_path` is not a valid directory or a skill ID is invalid.
pub fn preview_move_skills_to_library(
    cfg: &AppConfig,
    home: &Path,
    skill_ids: &[String],
    target_library_path: &Path,
    scope: &ToggleScope,
) -> Result<Vec<MovePreviewItem>, AppError> {
    let dest_parent = resolve_skills_destination_parent(target_library_path)?;
    let mut previews = Vec::new();

    for skill_id in skill_ids {
        validate_entry_name(skill_id)?;
        let Some(discovered) = locate_real_skill_and_symlinks(cfg, home, scope, skill_id) else {
            continue;
        };

        let destination_dir = dest_parent.join(skill_id);
        let conflict_exists = fs::symlink_metadata(&destination_dir).is_ok();
        let warning = conflict_exists.then(|| {
            format!(
                "Destination already exists in library: {}",
                destination_dir.display()
            )
        });

        let skill_name = read_skill_display_name(&discovered.real_dir, skill_id);
        previews.push(MovePreviewItem {
            skill_id: skill_id.clone(),
            skill_name,
            source_real_dir: discovered.real_dir.to_string_lossy().into_owned(),
            destination_dir: destination_dir.to_string_lossy().into_owned(),
            symlinks_to_update: discovered
                .related_symlinks
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect(),
            conflict_exists,
            warning,
        });
    }

    Ok(previews)
}

/// Safely moves unmanaged real skill folders into `target_library_path` and unlinks or relinks them.
///
/// # Errors
/// Returns [`AppError`] if `target_library_path` is invalid or any requested move encounters an I/O failure.
pub fn execute_move_skills_to_library(
    cfg: &AppConfig,
    home: &Path,
    skill_ids: &[String],
    target_library_path: &Path,
    scope: &ToggleScope,
    after_move: AfterMoveAction,
) -> Result<MoveExecutionResult, AppError> {
    let dest_parent = resolve_skills_destination_parent(target_library_path)?;
    let mut moved = Vec::new();
    let mut skipped = Vec::new();
    let mut warnings = Vec::new();
    let mut git_removals = Vec::new();

    for skill_id in skill_ids {
        validate_entry_name(skill_id)?;
        let Some(discovered) = locate_real_skill_and_symlinks(cfg, home, scope, skill_id) else {
            skipped.push(skill_id.clone());
            warnings.push(format!("No unmanaged real folder found for '{skill_id}'"));
            continue;
        };

        let destination_dir = dest_parent.join(skill_id);
        if fs::symlink_metadata(&destination_dir).is_ok() {
            skipped.push(skill_id.clone());
            warnings.push(format!(
                "Skipped '{skill_id}': destination already exists at {}",
                destination_dir.display()
            ));
            continue;
        }

        move_directory_safely(&discovered.real_dir, &destination_dir)?;
        let canonical_dest =
            fs::canonicalize(&destination_dir).unwrap_or_else(|_| destination_dir.clone());

        match after_move {
            AfterMoveAction::Unlink => {
                for link in &discovered.related_symlinks {
                    if let Ok(meta) = fs::symlink_metadata(link) {
                        if meta.file_type().is_symlink() {
                            let _ = fs::remove_file(link);
                        }
                    }
                }
                if let ToggleScope::Project { project_path } = scope {
                    let project = Path::new(project_path);
                    for p in std::iter::once(&discovered.real_dir)
                        .chain(discovered.related_symlinks.iter())
                    {
                        if let Ok(rel) = p.strip_prefix(project) {
                            git_removals.push(rel.to_string_lossy().into_owned());
                        }
                    }
                }
            }
            AfterMoveAction::Relink => {
                // Re-point secondary symlinks + restore primary symlink at original real_dir location
                for link in &discovered.related_symlinks {
                    if let Ok(meta) = fs::symlink_metadata(link) {
                        if meta.file_type().is_symlink() {
                            let _ = fs::remove_file(link);
                            symlink(&canonical_dest, link).map_err(|e| AppError::io(link, e))?;
                        }
                    }
                }
                symlink(&canonical_dest, &discovered.real_dir)
                    .map_err(|e| AppError::io(&discovered.real_dir, e))?;
            }
        }

        moved.push(skill_id.clone());
    }

    if let ToggleScope::Project { project_path } = scope {
        if cfg.git_hygiene && !git_removals.is_empty() {
            git_hygiene::sync_git_exclude(Path::new(project_path), &[], &git_removals, None)?;
        }
    }

    Ok(MoveExecutionResult {
        moved,
        skipped,
        warnings,
    })
}

struct DiscoveredRealSkill {
    real_dir: PathBuf,
    related_symlinks: Vec<PathBuf>,
}

fn resolve_skills_destination_parent(target_library_path: &Path) -> Result<PathBuf, AppError> {
    if !target_library_path.is_dir() {
        return Err(AppError::InvalidPath(format!(
            "Target library folder does not exist: {}",
            target_library_path.display()
        )));
    }
    let is_skills_dir = target_library_path
        .file_name()
        .is_some_and(|n| n.eq_ignore_ascii_case("skills"));
    let nested_skills = target_library_path.join("skills");
    if !is_skills_dir && nested_skills.is_dir() {
        Ok(nested_skills)
    } else {
        Ok(target_library_path.to_path_buf())
    }
}

fn locate_real_skill_and_symlinks(
    cfg: &AppConfig,
    home: &Path,
    scope: &ToggleScope,
    skill_id: &str,
) -> Option<DiscoveredRealSkill> {
    let candidates = scope_skill_candidate_paths(cfg, home, scope, skill_id);
    let mut real_dir: Option<PathBuf> = None;
    let mut symlinks = Vec::new();

    for path in candidates {
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.file_type().is_symlink() {
            symlinks.push(path);
        } else if meta.is_dir() && real_dir.is_none() {
            real_dir = Some(path);
        }
    }

    let real_dir = real_dir?;
    let canonical_real = fs::canonicalize(&real_dir).unwrap_or_else(|_| real_dir.clone());
    let related_symlinks: Vec<PathBuf> = symlinks
        .into_iter()
        .filter(|link| {
            fs::canonicalize(link)
                .map(|c| c == canonical_real)
                .unwrap_or(false)
        })
        .collect();

    Some(DiscoveredRealSkill {
        real_dir,
        related_symlinks,
    })
}

fn scope_skill_candidate_paths(
    cfg: &AppConfig,
    home: &Path,
    scope: &ToggleScope,
    skill_id: &str,
) -> Vec<PathBuf> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();

    match scope {
        ToggleScope::Global => {
            for adapter in enabled_adapters(cfg.active_agents) {
                for spec in adapter.global_skill_dirs(home) {
                    let candidate = spec.path.join(skill_id);
                    if seen.insert(candidate.clone()) {
                        out.push(candidate);
                    }
                }
            }
        }
        ToggleScope::Project { project_path } => {
            let project = Path::new(project_path);
            for rel in [".agents/skills", ".agent/skills", ".claude/skills"] {
                let candidate = project.join(rel).join(skill_id);
                if seen.insert(candidate.clone()) {
                    out.push(candidate);
                }
            }
        }
    }
    out
}

fn read_skill_display_name(dir: &Path, fallback: &str) -> String {
    let md_path = dir.join("SKILL.md");
    let Ok(raw) = fs::read_to_string(md_path) else {
        return fallback.to_owned();
    };
    parse_markdown_frontmatter(&raw)
        .name
        .unwrap_or_else(|| fallback.to_owned())
}

fn move_directory_safely(src: &Path, dst: &Path) -> Result<(), AppError> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent).map_err(|e| AppError::io(parent, e))?;
    }

    // Fast path: atomic rename when source and destination are on the same filesystem volume.
    if fs::rename(src, dst).is_ok() {
        return Ok(());
    }

    // Cross-volume fallback: recursive copy into sibling temp dir, rename into dst, then delete src.
    let name = dst.file_name().and_then(|n| n.to_str()).unwrap_or("skill");
    let tmp_dst = dst.with_file_name(format!(".{name}.agentdock-move.tmp"));
    if tmp_dst.exists() {
        fs::remove_dir_all(&tmp_dst).map_err(|e| AppError::io(&tmp_dst, e))?;
    }

    copy_dir_recursive(src, &tmp_dst)?;
    fs::rename(&tmp_dst, dst).map_err(|e| {
        let _ = fs::remove_dir_all(&tmp_dst);
        AppError::io(dst, e)
    })?;
    fs::remove_dir_all(src).map_err(|e| AppError::io(src, e))?;
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), AppError> {
    fs::create_dir_all(dst).map_err(|e| AppError::io(dst, e))?;
    let entries = fs::read_dir(src).map_err(|e| AppError::io(src, e))?;

    for entry in entries {
        let entry = entry.map_err(|e| AppError::io(src, e))?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        let meta = fs::symlink_metadata(&src_path).map_err(|e| AppError::io(&src_path, e))?;

        if meta.file_type().is_symlink() {
            let link_target = fs::read_link(&src_path).map_err(|e| AppError::io(&src_path, e))?;
            symlink(&link_target, &dst_path).map_err(|e| AppError::io(&dst_path, e))?;
        } else if meta.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path).map_err(|e| AppError::io(&dst_path, e))?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ActiveAgents;
    use tempfile::TempDir;

    #[test]
    fn previews_and_moves_unmanaged_global_skill_with_secondary_symlink_cleanup() {
        let tmp = TempDir::new().expect("temp dir");
        let root = tmp.path();
        let home = root.join("home");
        let vault = root.join("ObsidianSkills");
        let real_skill = home.join(".agents/skills/rogue-tool");
        let secondary_link = home.join(".gemini/config/skills/rogue-tool");

        fs::create_dir_all(&vault).expect("vault");
        fs::create_dir_all(&real_skill).expect("real skill");
        fs::create_dir_all(secondary_link.parent().expect("parent")).expect("gemini");
        fs::write(
            real_skill.join("SKILL.md"),
            "---\nname: rogue-tool\ndescription: Unmanaged skill\n---\nInstructions\n",
        )
        .expect("SKILL.md");
        symlink(&real_skill, &secondary_link).expect("secondary symlink");

        let cfg = AppConfig {
            projects_root: None,
            library_sources: Vec::new(),
            support_docs_root: None,
            pinned_projects: Vec::new(),
            ignore_folders: Vec::new(),
            active_agents: ActiveAgents {
                antigravity: true,
                codex: true,
                claude: false,
            },
            git_hygiene: true,
        };

        let ids = vec!["rogue-tool".to_owned()];
        let preview =
            preview_move_skills_to_library(&cfg, &home, &ids, &vault, &ToggleScope::Global)
                .expect("preview");
        assert_eq!(preview.len(), 1);
        assert!(!preview[0].conflict_exists);
        assert_eq!(preview[0].symlinks_to_update.len(), 1);

        let res = execute_move_skills_to_library(
            &cfg,
            &home,
            &ids,
            &vault,
            &ToggleScope::Global,
            AfterMoveAction::Unlink,
        )
        .expect("execute move");
        assert_eq!(res.moved, vec!["rogue-tool"]);
        assert!(vault.join("rogue-tool/SKILL.md").is_file());
        assert!(!real_skill.exists());
        assert!(fs::symlink_metadata(&secondary_link).is_err());
    }
}

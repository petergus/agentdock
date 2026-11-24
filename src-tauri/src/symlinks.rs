//! Safe symlink creation and guarded unlinking for skills and library rules.

use std::collections::BTreeSet;
use std::fs;
use std::io::ErrorKind;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use crate::adapters::enabled_adapters;
use crate::error::AppError;
use crate::git_hygiene;
use crate::models::{AppConfig, DesiredState, ToggleOutcome, ToggleScope};

/// Validates a single path segment (skill slug or rule filename) to prevent directory traversal.
///
/// # Errors
/// Returns [`AppError::InvalidPath`] if the segment is empty or contains separators or `.`/`..`.
pub fn validate_entry_name(name: &str) -> Result<(), AppError> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains('/')
        || name.contains('\\')
        || name.contains('\0')
    {
        return Err(AppError::InvalidPath(format!("Unsafe entry name: {name}")));
    }
    Ok(())
}

/// Toggles a skill ON (creates symlinks) or OFF (strictly unlinks verified symlinks only).
///
/// When disabling, if any installed target is a real directory/file rather than a symlink,
/// this aborts before unlinking anything and returns [`ToggleOutcome::RequiresMoveToLibrary`].
///
/// # Errors
/// Returns [`AppError`] on invalid paths, missing canonical sources, or filesystem write errors.
pub fn toggle_skill(
    cfg: &AppConfig,
    home: &Path,
    scope: &ToggleScope,
    skill_id: &str,
    canonical_skill_dir: &Path,
    desired: DesiredState,
) -> Result<ToggleOutcome, AppError> {
    validate_entry_name(skill_id)?;

    match desired {
        DesiredState::Enable => enable_skill(cfg, home, scope, skill_id, canonical_skill_dir),
        DesiredState::Disable => disable_skill(cfg, home, scope, skill_id),
    }
}

/// Toggles a reusable Library rule `.md` file ON or OFF via symlinks.
///
/// # Errors
/// Returns [`AppError`] on invalid paths, missing canonical files, or filesystem write errors.
pub fn toggle_library_rule(
    cfg: &AppConfig,
    home: &Path,
    scope: &ToggleScope,
    rule_filename: &str,
    canonical_rule_file: &Path,
    desired: DesiredState,
) -> Result<ToggleOutcome, AppError> {
    validate_entry_name(rule_filename)?;
    if !rule_filename.ends_with(".md") {
        return Err(AppError::InvalidPath(format!(
            "Rule filename must end with .md: {rule_filename}"
        )));
    }

    match desired {
        DesiredState::Enable => {
            if !canonical_rule_file.is_file() {
                return Err(AppError::InvalidPath(format!(
                    "Canonical rule file does not exist: {}",
                    canonical_rule_file.display()
                )));
            }
            let targets = primary_rule_targets(cfg, home, scope, rule_filename);
            let mut changed = Vec::new();
            let mut exclude_entries = Vec::new();

            for target in targets {
                ensure_symlink(&target.path, canonical_rule_file)?;
                changed.push(target.path.to_string_lossy().into_owned());
                if let Some(rel) = target.git_relative {
                    exclude_entries.push(rel);
                }
            }

            if let ToggleScope::Project { project_path } = scope {
                if cfg.git_hygiene && !exclude_entries.is_empty() {
                    git_hygiene::sync_git_exclude(
                        Path::new(project_path),
                        &exclude_entries,
                        &[],
                        None,
                    )?;
                }
            }

            Ok(ToggleOutcome::Updated {
                changed_paths: changed,
            })
        }
        DesiredState::Disable => {
            let targets = all_rule_targets(cfg, home, scope, rule_filename);
            disable_verified_symlinks(cfg, scope, rule_filename, &targets)
        }
    }
}

/// Unlinks a broken symlink after verifying with `symlink_metadata` that it is indeed a broken symlink.
///
/// # Errors
/// Returns [`AppError`] if the path is not a symlink, resolves to an existing target, or removal fails.
pub fn unlink_broken_symlink(
    cfg: &AppConfig,
    target_path: &Path,
    project_path: Option<&Path>,
) -> Result<(), AppError> {
    let meta = fs::symlink_metadata(target_path).map_err(|e| AppError::io(target_path, e))?;
    if !meta.file_type().is_symlink() {
        return Err(AppError::InvalidPath(format!(
            "Refusing to remove non-symlink path: {}",
            target_path.display()
        )));
    }
    if target_path.exists() {
        return Err(AppError::InvalidPath(format!(
            "Target is not broken: {}",
            target_path.display()
        )));
    }

    fs::remove_file(target_path).map_err(|e| AppError::io(target_path, e))?;

    if let Some(project) = project_path {
        if cfg.git_hygiene {
            if let Ok(rel) = target_path.strip_prefix(project) {
                let rel_str = rel.to_string_lossy().into_owned();
                git_hygiene::sync_git_exclude(project, &[], &[rel_str], None)?;
            }
        }
    }

    Ok(())
}

struct TargetEntry {
    path: PathBuf,
    git_relative: Option<String>,
}

fn enable_skill(
    cfg: &AppConfig,
    home: &Path,
    scope: &ToggleScope,
    skill_id: &str,
    canonical_skill_dir: &Path,
) -> Result<ToggleOutcome, AppError> {
    if !canonical_skill_dir.is_dir() {
        return Err(AppError::InvalidPath(format!(
            "Canonical skill directory does not exist: {}",
            canonical_skill_dir.display()
        )));
    }

    let targets = primary_skill_targets(cfg, home, scope, skill_id);
    let mut changed = Vec::new();
    let mut exclude_entries = Vec::new();

    for target in targets {
        if let Ok(meta) = fs::symlink_metadata(&target.path) {
            if !meta.file_type().is_symlink() {
                return Ok(ToggleOutcome::RequiresMoveToLibrary {
                    skill_id: skill_id.to_owned(),
                    real_paths: vec![target.path.to_string_lossy().into_owned()],
                });
            }
        }
        ensure_symlink(&target.path, canonical_skill_dir)?;
        changed.push(target.path.to_string_lossy().into_owned());
        if let Some(rel) = target.git_relative {
            exclude_entries.push(rel);
        }
    }

    if let ToggleScope::Project { project_path } = scope {
        if cfg.git_hygiene && !exclude_entries.is_empty() {
            git_hygiene::sync_git_exclude(Path::new(project_path), &exclude_entries, &[], None)?;
        }
    }

    Ok(ToggleOutcome::Updated {
        changed_paths: changed,
    })
}

fn disable_skill(
    cfg: &AppConfig,
    home: &Path,
    scope: &ToggleScope,
    skill_id: &str,
) -> Result<ToggleOutcome, AppError> {
    let targets = all_skill_targets(cfg, home, scope, skill_id);
    disable_verified_symlinks(cfg, scope, skill_id, &targets)
}

fn disable_verified_symlinks(
    cfg: &AppConfig,
    scope: &ToggleScope,
    entry_id: &str,
    targets: &[TargetEntry],
) -> Result<ToggleOutcome, AppError> {
    let mut real_paths = Vec::new();
    let mut symlinks_to_remove: Vec<&TargetEntry> = Vec::new();
    let mut remove_git_entries = Vec::new();

    // Pass 1: Strict symlink_metadata check across every candidate before removing anything.
    for target in targets {
        match fs::symlink_metadata(&target.path) {
            Ok(meta) => {
                if meta.file_type().is_symlink() {
                    symlinks_to_remove.push(target);
                } else {
                    real_paths.push(target.path.to_string_lossy().into_owned());
                }
            }
            Err(err) if err.kind() == ErrorKind::NotFound => {
                // Still clean up stale .git/info/exclude entry if present
                if let Some(rel) = &target.git_relative {
                    remove_git_entries.push(rel.clone());
                }
            }
            Err(err) => return Err(AppError::io(&target.path, err)),
        }
    }

    if !real_paths.is_empty() {
        return Ok(ToggleOutcome::RequiresMoveToLibrary {
            skill_id: entry_id.to_owned(),
            real_paths,
        });
    }

    // Pass 2: All existing paths are verified symlinks — safe to unlink.
    let mut changed = Vec::new();
    for target in symlinks_to_remove {
        match fs::remove_file(&target.path) {
            Ok(()) => {
                changed.push(target.path.to_string_lossy().into_owned());
            }
            // If parent directory itself was a symlink to another scanned primary directory
            // (e.g. ~/.gemini/antigravity/skills -> ~/.gemini/config/skills), the second
            // remove_file sees NotFound after the primary entry is unlinked.
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(err) => return Err(AppError::io(&target.path, err)),
        }
        if let Some(rel) = &target.git_relative {
            remove_git_entries.push(rel.clone());
        }
    }

    if let ToggleScope::Project { project_path } = scope {
        if cfg.git_hygiene && !remove_git_entries.is_empty() {
            git_hygiene::sync_git_exclude(Path::new(project_path), &[], &remove_git_entries, None)?;
        }
    }

    Ok(ToggleOutcome::Updated {
        changed_paths: changed,
    })
}

fn ensure_symlink(link_path: &Path, canonical_target: &Path) -> Result<(), AppError> {
    if let Some(parent) = link_path.parent() {
        fs::create_dir_all(parent).map_err(|e| AppError::io(parent, e))?;
    }

    match fs::symlink_metadata(link_path) {
        Ok(meta) => {
            if !meta.file_type().is_symlink() {
                return Err(AppError::InvalidPath(format!(
                    "Real file or directory already exists at {}",
                    link_path.display()
                )));
            }
            // Replace if pointing elsewhere or broken
            let existing_canon = fs::canonicalize(link_path).ok();
            let desired_canon = fs::canonicalize(canonical_target)
                .unwrap_or_else(|_| canonical_target.to_path_buf());
            if existing_canon.as_deref() == Some(desired_canon.as_path()) {
                return Ok(());
            }
            fs::remove_file(link_path).map_err(|e| AppError::io(link_path, e))?;
        }
        Err(err) if err.kind() == ErrorKind::NotFound => {}
        Err(err) => return Err(AppError::io(link_path, err)),
    }

    symlink(canonical_target, link_path).map_err(|e| AppError::io(link_path, e))
}

fn primary_skill_targets(
    cfg: &AppConfig,
    home: &Path,
    scope: &ToggleScope,
    skill_id: &str,
) -> Vec<TargetEntry> {
    match scope {
        ToggleScope::Global => {
            let mut seen_parents = BTreeSet::new();
            let mut out = Vec::new();
            for adapter in enabled_adapters(cfg.active_agents) {
                for spec in adapter.global_skill_dirs(home) {
                    if spec.is_legacy {
                        continue;
                    }
                    let key = fs::canonicalize(&spec.path).unwrap_or_else(|_| spec.path.clone());
                    if seen_parents.insert(key) {
                        out.push(TargetEntry {
                            path: spec.path.join(skill_id),
                            git_relative: None,
                        });
                    }
                }
            }
            out
        }
        ToggleScope::Project { project_path } => {
            let project = Path::new(project_path);
            let mut out = vec![TargetEntry {
                path: project.join(".agents/skills").join(skill_id),
                git_relative: Some(format!(".agents/skills/{skill_id}")),
            }];
            if cfg.active_agents.claude {
                out.push(TargetEntry {
                    path: project.join(".claude/skills").join(skill_id),
                    git_relative: Some(format!(".claude/skills/{skill_id}")),
                });
            }
            out
        }
    }
}

fn all_skill_targets(
    cfg: &AppConfig,
    home: &Path,
    scope: &ToggleScope,
    skill_id: &str,
) -> Vec<TargetEntry> {
    match scope {
        ToggleScope::Global => {
            let mut seen_paths = BTreeSet::new();
            let mut out = Vec::new();
            for adapter in enabled_adapters(cfg.active_agents) {
                for spec in adapter.global_skill_dirs(home) {
                    let candidate = spec.path.join(skill_id);
                    if seen_paths.insert(candidate.clone()) {
                        out.push(TargetEntry {
                            path: candidate,
                            git_relative: None,
                        });
                    }
                }
            }
            out
        }
        ToggleScope::Project { project_path } => {
            let project = Path::new(project_path);
            let mut seen = BTreeSet::new();
            let mut out = Vec::new();

            let candidates = [
                (
                    project.join(".agents/skills").join(skill_id),
                    format!(".agents/skills/{skill_id}"),
                ),
                (
                    project.join(".agent/skills").join(skill_id),
                    format!(".agent/skills/{skill_id}"),
                ),
                (
                    project.join(".claude/skills").join(skill_id),
                    format!(".claude/skills/{skill_id}"),
                ),
            ];

            for (path, rel) in candidates {
                if seen.insert(path.clone()) {
                    out.push(TargetEntry {
                        path,
                        git_relative: Some(rel),
                    });
                }
            }
            out
        }
    }
}

fn primary_rule_targets(
    cfg: &AppConfig,
    home: &Path,
    scope: &ToggleScope,
    rule_filename: &str,
) -> Vec<TargetEntry> {
    match scope {
        ToggleScope::Global => {
            let mut out = Vec::new();
            if cfg.active_agents.antigravity {
                out.push(TargetEntry {
                    path: home.join(".gemini/config/rules").join(rule_filename),
                    git_relative: None,
                });
            }
            out
        }
        ToggleScope::Project { project_path } => {
            let project = Path::new(project_path);
            let mut out = vec![TargetEntry {
                path: project.join(".agents/rules").join(rule_filename),
                git_relative: Some(format!(".agents/rules/{rule_filename}")),
            }];
            if cfg.active_agents.claude {
                out.push(TargetEntry {
                    path: project.join(".claude/rules").join(rule_filename),
                    git_relative: Some(format!(".claude/rules/{rule_filename}")),
                });
            }
            out
        }
    }
}

fn all_rule_targets(
    _cfg: &AppConfig,
    home: &Path,
    scope: &ToggleScope,
    rule_filename: &str,
) -> Vec<TargetEntry> {
    match scope {
        ToggleScope::Global => vec![TargetEntry {
            path: home.join(".gemini/config/rules").join(rule_filename),
            git_relative: None,
        }],
        ToggleScope::Project { project_path } => {
            let project = Path::new(project_path);
            vec![
                TargetEntry {
                    path: project.join(".agents/rules").join(rule_filename),
                    git_relative: Some(format!(".agents/rules/{rule_filename}")),
                },
                TargetEntry {
                    path: project.join(".agent/rules").join(rule_filename),
                    git_relative: Some(format!(".agent/rules/{rule_filename}")),
                },
                TargetEntry {
                    path: project.join(".claude/rules").join(rule_filename),
                    git_relative: Some(format!(".claude/rules/{rule_filename}")),
                },
            ]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ActiveAgents, LibrarySource};
    use crate::scanner;
    use tempfile::TempDir;

    fn make_skill_dir(dir: &Path, name: &str, desc: &str) {
        fs::create_dir_all(dir).expect("create skill dir");
        fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: {desc}\n---\nBody instructions\n"),
        )
        .expect("write SKILL.md");
    }

    #[test]
    fn toggling_project_skill_on_and_off_updates_symlink_git_exclude_and_tokens() {
        let tmp = TempDir::new().expect("temp dir");
        let root = tmp.path();
        let home = root.join("home");
        let vault_skills = root.join("BRAIN-LOCAL/09-ai-os/skills");
        let project = root.join("APP-PROJECTS/Clients/Acme");

        fs::create_dir_all(&home).expect("home");
        fs::create_dir_all(project.join(".git/info")).expect("git info");
        let obsidian_skill = vault_skills.join("peter-writing");
        make_skill_dir(&obsidian_skill, "peter-writing", "Writes crisp prose.");

        let cfg = AppConfig {
            projects_root: Some(root.join("APP-PROJECTS").to_string_lossy().into_owned()),
            library_sources: vec![LibrarySource {
                id: "obsidian".to_owned(),
                label: "Obsidian".to_owned(),
                path: vault_skills.to_string_lossy().into_owned(),
            }],
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

        let scope = ToggleScope::Project {
            project_path: project.to_string_lossy().into_owned(),
        };

        // Baseline scan (OFF)
        let before =
            scanner::scan_project_for_home(&cfg, &home, &project, "Clients").expect("scan before");
        assert_eq!(before.budget.skills_project, 0);

        // Toggle ON -> creates symlink + .git/info/exclude + increases tokens
        let on_res = toggle_skill(
            &cfg,
            &home,
            &scope,
            "peter-writing",
            &obsidian_skill,
            DesiredState::Enable,
        )
        .expect("toggle on");
        assert!(matches!(on_res, ToggleOutcome::Updated { .. }));
        let link_path = project.join(".agents/skills/peter-writing");
        assert!(fs::symlink_metadata(&link_path)
            .expect("meta")
            .file_type()
            .is_symlink());
        let exclude = fs::read_to_string(project.join(".git/info/exclude")).expect("exclude");
        assert!(exclude.contains(".agents/skills/peter-writing"));

        let active_snap =
            scanner::scan_project_for_home(&cfg, &home, &project, "Clients").expect("scan on");
        assert!(active_snap.budget.skills_project > 0);

        // Toggle OFF -> removes only symlink, cleans exclude, preserves Obsidian file, tokens back to 0
        let off_res = toggle_skill(
            &cfg,
            &home,
            &scope,
            "peter-writing",
            &obsidian_skill,
            DesiredState::Disable,
        )
        .expect("toggle off");
        assert!(matches!(off_res, ToggleOutcome::Updated { .. }));
        assert!(!link_path.exists());
        assert!(obsidian_skill.join("SKILL.md").is_file());
        let exclude_after =
            fs::read_to_string(project.join(".git/info/exclude")).expect("exclude after");
        assert!(!exclude_after.contains("peter-writing"));

        let after_snap =
            scanner::scan_project_for_home(&cfg, &home, &project, "Clients").expect("scan off");
        assert_eq!(after_snap.budget.skills_project, 0);
    }

    #[test]
    fn refuses_to_delete_real_directory_on_toggle_off() {
        let tmp = TempDir::new().expect("temp dir");
        let root = tmp.path();
        let home = root.join("home");
        let real_skill = home.join(".agents/skills/rogue-pack");
        make_skill_dir(&real_skill, "rogue-pack", "Real unmanaged folder.");

        let cfg = AppConfig {
            projects_root: None,
            library_sources: Vec::new(),
            support_docs_root: None,
            pinned_projects: Vec::new(),
            ignore_folders: Vec::new(),
            active_agents: ActiveAgents::default(),
            git_hygiene: true,
        };

        let outcome = toggle_skill(
            &cfg,
            &home,
            &ToggleScope::Global,
            "rogue-pack",
            &real_skill,
            DesiredState::Disable,
        )
        .expect("toggle off real dir");

        match outcome {
            ToggleOutcome::RequiresMoveToLibrary {
                skill_id,
                real_paths,
            } => {
                assert_eq!(skill_id, "rogue-pack");
                assert_eq!(real_paths.len(), 1);
            }
            ToggleOutcome::Updated { .. } => panic!("must never delete a real directory"),
        }
        assert!(real_skill.join("SKILL.md").is_file());
    }
}

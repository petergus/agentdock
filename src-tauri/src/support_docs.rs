//! Project Support Docs attachment (Zero-Token pointer blocks & `_support-docs/` symlinks).

use std::fs;
use std::io::ErrorKind;
use std::os::unix::fs::symlink;
use std::path::Path;

use crate::backup;
use crate::error::AppError;
use crate::git_hygiene;
use crate::models::{AppConfig, LinkSupportDocRequest, SupportDocMode, SupportDocTargetFile};
use crate::symlinks::validate_entry_name;

const START_MARKER: &str = "<!-- AGENTDOCK:SUPPORT_DOCS_START -->";
const END_MARKER: &str = "<!-- AGENTDOCK:SUPPORT_DOCS_END -->";
const SUPPORT_DOCS_RULE_REL: &str = ".agents/rules/support-docs.md";
const SUPPORT_DOCS_DIR_REL: &str = "_support-docs";

/// Attaches an external file or directory to a project via pointer block or local symlink.
///
/// # Errors
/// Returns [`AppError`] if the project or source path does not exist or filesystem writes fail.
pub fn link_support_doc(cfg: &AppConfig, req: &LinkSupportDocRequest) -> Result<(), AppError> {
    link_support_doc_with_backups(cfg, req, None)
}

/// Variant of [`link_support_doc`] accepting a custom backups directory for tests.
///
/// # Errors
/// Returns [`AppError`] on validation or I/O failure.
pub fn link_support_doc_with_backups(
    cfg: &AppConfig,
    req: &LinkSupportDocRequest,
    custom_backups_root: Option<&Path>,
) -> Result<(), AppError> {
    let project = Path::new(&req.project_path);
    if !project.is_dir() {
        return Err(AppError::InvalidPath(format!(
            "Project directory does not exist: {}",
            project.display()
        )));
    }

    let source = Path::new(&req.source_path);
    if !source.exists() {
        return Err(AppError::InvalidPath(format!(
            "Support doc path does not exist: {}",
            source.display()
        )));
    }

    let canonical_source = fs::canonicalize(source).unwrap_or_else(|_| source.to_path_buf());
    let doc_name = canonical_source
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("support-doc")
        .to_owned();

    match req.mode {
        SupportDocMode::Pointer => {
            let target_rel = match req.target_file.unwrap_or(SupportDocTargetFile::AgentsMd) {
                SupportDocTargetFile::AgentsMd => "AGENTS.md",
                SupportDocTargetFile::SupportDocsRule => SUPPORT_DOCS_RULE_REL,
            };
            let target_path = project.join(target_rel);
            let canonical_str = canonical_source.to_string_lossy().into_owned();
            let desc = req
                .description
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned);

            upsert_pointer_entry(
                &target_path,
                target_rel == SUPPORT_DOCS_RULE_REL,
                &doc_name,
                &canonical_str,
                desc.as_deref(),
                custom_backups_root,
            )?;

            if target_rel == SUPPORT_DOCS_RULE_REL && cfg.git_hygiene {
                git_hygiene::sync_git_exclude(
                    project,
                    &[SUPPORT_DOCS_RULE_REL.to_owned()],
                    &[],
                    custom_backups_root,
                )?;
            }
            Ok(())
        }
        SupportDocMode::Symlink => {
            validate_entry_name(&doc_name)?;
            let support_dir = project.join(SUPPORT_DOCS_DIR_REL);
            fs::create_dir_all(&support_dir).map_err(|e| AppError::io(&support_dir, e))?;

            let link_path = support_dir.join(&doc_name);
            match fs::symlink_metadata(&link_path) {
                Ok(meta) => {
                    if !meta.file_type().is_symlink() {
                        return Err(AppError::InvalidPath(format!(
                            "A real file or folder already exists at {}",
                            link_path.display()
                        )));
                    }
                    fs::remove_file(&link_path).map_err(|e| AppError::io(&link_path, e))?;
                }
                Err(err) if err.kind() == ErrorKind::NotFound => {}
                Err(err) => return Err(AppError::io(&link_path, e_to_io(err))),
            }

            symlink(&canonical_source, &link_path).map_err(|e| AppError::io(&link_path, e))?;

            if cfg.git_hygiene {
                git_hygiene::sync_git_exclude(
                    project,
                    &[format!("{SUPPORT_DOCS_DIR_REL}/")],
                    &[],
                    custom_backups_root,
                )?;
            }
            Ok(())
        }
    }
}

fn e_to_io(err: std::io::Error) -> std::io::Error {
    err
}

/// Unlinks an attached support doc from a project (removing pointer entry or symlink cleanly).
///
/// # Errors
/// Returns [`AppError`] if unlinking or updating target files fails.
pub fn unlink_support_doc(
    cfg: &AppConfig,
    project_path: &Path,
    doc_path: &str,
    mode: SupportDocMode,
) -> Result<(), AppError> {
    unlink_support_doc_with_backups(cfg, project_path, doc_path, mode, None)
}

/// Variant of [`unlink_support_doc`] accepting a custom backups directory for tests.
///
/// # Errors
/// Returns [`AppError`] on validation or I/O failure.
pub fn unlink_support_doc_with_backups(
    cfg: &AppConfig,
    project_path: &Path,
    doc_path: &str,
    mode: SupportDocMode,
    custom_backups_root: Option<&Path>,
) -> Result<(), AppError> {
    match mode {
        SupportDocMode::Pointer => {
            for rel in ["AGENTS.md", SUPPORT_DOCS_RULE_REL] {
                let file_path = project_path.join(rel);
                if file_path.is_file() {
                    remove_pointer_entry(&file_path, doc_path, custom_backups_root)?;
                }
            }
            Ok(())
        }
        SupportDocMode::Symlink => {
            let support_dir = project_path.join(SUPPORT_DOCS_DIR_REL);
            if !support_dir.is_dir() {
                return Ok(());
            }

            let target_canon = fs::canonicalize(doc_path).ok();
            let entries = fs::read_dir(&support_dir).map_err(|e| AppError::io(&support_dir, e))?;
            for entry in entries.flatten() {
                let entry_path = entry.path();
                let Ok(meta) = fs::symlink_metadata(&entry_path) else {
                    continue;
                };
                if !meta.file_type().is_symlink() {
                    continue;
                }

                let matches = fs::read_link(&entry_path)
                    .map(|raw| {
                        raw.to_string_lossy() == doc_path
                            || fs::canonicalize(&entry_path).ok().is_some_and(|c| {
                                target_canon.as_ref() == Some(&c) || c.to_string_lossy() == doc_path
                            })
                    })
                    .unwrap_or(false);

                if matches {
                    fs::remove_file(&entry_path).map_err(|e| AppError::io(&entry_path, e))?;
                }
            }

            let remaining = fs::read_dir(&support_dir)
                .map_err(|e| AppError::io(&support_dir, e))?
                .flatten()
                .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
                .count();
            if remaining == 0 {
                let _ = fs::remove_dir(&support_dir);
                if cfg.git_hygiene {
                    git_hygiene::sync_git_exclude(
                        project_path,
                        &[],
                        &[format!("{SUPPORT_DOCS_DIR_REL}/")],
                        custom_backups_root,
                    )?;
                }
            }

            Ok(())
        }
    }
}

fn upsert_pointer_entry(
    target_path: &Path,
    is_rule_file: bool,
    name: &str,
    path: &str,
    description: Option<&str>,
    custom_backups_root: Option<&Path>,
) -> Result<(), AppError> {
    let existing = if target_path.is_file() {
        fs::read_to_string(target_path).map_err(|e| AppError::io(target_path, e))?
    } else if is_rule_file {
        "---\ntrigger: model_decision\ndescription: External project support documentation pointers (read on demand).\n---\n\n# Support Docs\n".to_owned()
    } else {
        String::new()
    };

    let mut entries = extract_entries(&existing);
    entries.retain(|(existing_path, _)| existing_path != path);
    entries.push((path.to_owned(), format_entry_line(name, path, description)));

    let new_block = render_block(&entries);
    let updated = replace_or_append_block(&existing, &new_block);

    if target_path.is_file() {
        backup_target(target_path, custom_backups_root)?;
    }
    backup::atomic_write_string(target_path, &updated)
}

fn remove_pointer_entry(
    target_path: &Path,
    doc_path: &str,
    custom_backups_root: Option<&Path>,
) -> Result<(), AppError> {
    let existing = fs::read_to_string(target_path).map_err(|e| AppError::io(target_path, e))?;
    if !existing.contains(START_MARKER) {
        return Ok(());
    }

    let mut entries = extract_entries(&existing);
    let before_len = entries.len();
    entries.retain(|(p, _)| p != doc_path);
    if entries.len() == before_len {
        return Ok(());
    }

    let updated = if entries.is_empty() {
        strip_block(&existing)
    } else {
        let new_block = render_block(&entries);
        replace_or_append_block(&existing, &new_block)
    };

    backup_target(target_path, custom_backups_root)?;
    backup::atomic_write_string(target_path, &updated)
}

fn backup_target(target: &Path, custom_backups_root: Option<&Path>) -> Result<(), AppError> {
    match custom_backups_root {
        Some(dir) => {
            backup::backup_file_in_dir(target, dir)?;
        }
        None => {
            backup::backup_file(target)?;
        }
    }
    Ok(())
}

fn format_entry_line(name: &str, path: &str, description: Option<&str>) -> String {
    match description {
        Some(desc) => format!("- [{name}]({path}) — {desc}"),
        None => format!("- [{name}]({path})"),
    }
}

fn extract_entries(content: &str) -> Vec<(String, String)> {
    let Some(start_idx) = content.find(START_MARKER) else {
        return Vec::new();
    };
    let rest = &content[start_idx + START_MARKER.len()..];
    let Some(end_idx) = rest.find(END_MARKER) else {
        return Vec::new();
    };
    let block = &rest[..end_idx];

    let mut out = Vec::new();
    for line in block.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(path) = parse_entry_path(trimmed) {
            out.push((path, trimmed.to_owned()));
        }
    }
    out
}

fn parse_entry_path(line: &str) -> Option<String> {
    let stripped = line.trim().trim_start_matches('-').trim();
    if let Some(after_bracket) = stripped.strip_prefix('[') {
        if let Some((_, tail)) = after_bracket.split_once("](") {
            if let Some((raw_path, _)) = tail.split_once(')') {
                return Some(
                    raw_path
                        .strip_prefix("file://")
                        .unwrap_or(raw_path)
                        .trim()
                        .to_owned(),
                );
            }
        }
    }
    if let Some(after_tick) = stripped.strip_prefix('`') {
        if let Some((raw_path, _)) = after_tick.split_once('`') {
            return Some(raw_path.trim().to_owned());
        }
    }
    None
}

fn render_block(entries: &[(String, String)]) -> String {
    let mut lines = Vec::with_capacity(entries.len() + 2);
    lines.push(START_MARKER.to_owned());
    for (_, line) in entries {
        lines.push(line.clone());
    }
    lines.push(END_MARKER.to_owned());
    lines.join("\n")
}

fn replace_or_append_block(content: &str, new_block: &str) -> String {
    if let Some(start_idx) = content.find(START_MARKER) {
        let before = &content[..start_idx];
        let after_start = &content[start_idx + START_MARKER.len()..];
        if let Some(end_rel) = after_start.find(END_MARKER) {
            let after = &after_start[end_rel + END_MARKER.len()..];
            let mut joined = format!("{before}{new_block}{after}");
            if !joined.ends_with('\n') {
                joined.push('\n');
            }
            return joined;
        }
    }

    let trimmed = content.trim_end();
    if trimmed.is_empty() {
        format!("## Support Docs\n\n{new_block}\n")
    } else if trimmed.ends_with("# Support Docs") {
        format!("{trimmed}\n\n{new_block}\n")
    } else {
        format!("{trimmed}\n\n## Support Docs\n\n{new_block}\n")
    }
}

fn strip_block(content: &str) -> String {
    let Some(start_idx) = content.find(START_MARKER) else {
        return content.to_owned();
    };
    let before = &content[..start_idx];
    let after_start = &content[start_idx + START_MARKER.len()..];
    let Some(end_rel) = after_start.find(END_MARKER) else {
        return content.to_owned();
    };
    let after = &after_start[end_rel + END_MARKER.len()..];

    let before_cleaned = before
        .trim_end()
        .strip_suffix("## Support Docs")
        .unwrap_or_else(|| before.trim_end())
        .trim_end();
    let after_cleaned = after.trim_start();

    match (before_cleaned.is_empty(), after_cleaned.is_empty()) {
        (true, true) => String::new(),
        (true, false) => format!("{after_cleaned}\n"),
        (false, true) => format!("{before_cleaned}\n"),
        (false, false) => format!("{before_cleaned}\n\n{after_cleaned}\n"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ActiveAgents;
    use crate::scanner;
    use tempfile::TempDir;

    fn test_config() -> AppConfig {
        AppConfig {
            projects_root: None,
            library_sources: Vec::new(),
            support_docs_root: None,
            pinned_projects: Vec::new(),
            ignore_folders: Vec::new(),
            active_agents: ActiveAgents::default(),
            git_hygiene: true,
        }
    }

    #[test]
    fn links_and_unlinks_pointer_block_preserving_agents_md() {
        let tmp = TempDir::new().expect("temp dir");
        let root = tmp.path();
        let home = root.join("home");
        let project = root.join("proj");
        let docs_folder = root.join("ClientNotes");
        let backups = root.join("backups");

        fs::create_dir_all(&home).expect("home");
        fs::create_dir_all(&project).expect("project");
        fs::create_dir_all(&docs_folder).expect("docs");
        fs::write(
            project.join("AGENTS.md"),
            "# Project Rules\nKeep code clean.\n",
        )
        .expect("write AGENTS.md");

        let cfg = test_config();
        let canonical_docs = fs::canonicalize(&docs_folder).expect("canon");

        link_support_doc_with_backups(
            &cfg,
            &LinkSupportDocRequest {
                project_path: project.to_string_lossy().into_owned(),
                source_path: docs_folder.to_string_lossy().into_owned(),
                mode: SupportDocMode::Pointer,
                description: Some("Client specs and architecture notes".to_owned()),
                target_file: Some(SupportDocTargetFile::AgentsMd),
            },
            Some(&backups),
        )
        .expect("link pointer");

        let agents_after = fs::read_to_string(project.join("AGENTS.md")).expect("read AGENTS");
        assert!(agents_after.contains("# Project Rules\nKeep code clean."));
        assert!(agents_after.contains(START_MARKER));
        assert!(agents_after.contains(&canonical_docs.to_string_lossy().into_owned()));

        let snap =
            scanner::scan_project_for_home(&cfg, &home, &project, "Test").expect("scan project");
        assert_eq!(snap.support_docs.len(), 1);
        assert_eq!(snap.support_docs[0].mode, SupportDocMode::Pointer);

        unlink_support_doc_with_backups(
            &cfg,
            &project,
            &canonical_docs.to_string_lossy(),
            SupportDocMode::Pointer,
            Some(&backups),
        )
        .expect("unlink pointer");

        let agents_restored = fs::read_to_string(project.join("AGENTS.md")).expect("read restored");
        assert_eq!(agents_restored, "# Project Rules\nKeep code clean.\n");
    }

    #[test]
    fn links_and_unlinks_symlink_support_docs_with_git_exclude() {
        let tmp = TempDir::new().expect("temp dir");
        let root = tmp.path();
        let home = root.join("home");
        let project = root.join("proj");
        let docs_folder = root.join("SpecPack");
        let backups = root.join("backups");

        fs::create_dir_all(&home).expect("home");
        fs::create_dir_all(project.join(".git/info")).expect("git");
        fs::create_dir_all(&docs_folder).expect("docs");
        fs::write(docs_folder.join("brief.md"), "Brief").expect("write brief");

        let cfg = test_config();
        let canonical_docs = fs::canonicalize(&docs_folder).expect("canon");

        link_support_doc_with_backups(
            &cfg,
            &LinkSupportDocRequest {
                project_path: project.to_string_lossy().into_owned(),
                source_path: docs_folder.to_string_lossy().into_owned(),
                mode: SupportDocMode::Symlink,
                description: None,
                target_file: None,
            },
            Some(&backups),
        )
        .expect("link symlink");

        let exclude = fs::read_to_string(project.join(".git/info/exclude")).expect("exclude");
        assert!(exclude.contains("_support-docs/"));
        assert!(project.join("_support-docs/SpecPack/brief.md").is_file());

        unlink_support_doc_with_backups(
            &cfg,
            &project,
            &canonical_docs.to_string_lossy(),
            SupportDocMode::Symlink,
            Some(&backups),
        )
        .expect("unlink symlink");

        assert!(!project.join("_support-docs").exists());
        assert!(docs_folder.join("brief.md").is_file());
        let exclude_after = fs::read_to_string(project.join(".git/info/exclude")).expect("exclude");
        assert!(!exclude_after.contains("_support-docs/"));
    }
}

//! Persistent configuration management stored in macOS Application Support.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::AppError;
use crate::models::{
    AgentPaths, AppConfig, ContextSet, LibrarySource, ProjectRoot, ProjectScanMode, TagDefinition,
    DEFAULT_LIBRARY_SCAN_DEPTH, DEFAULT_PROJECT_SCAN_DEPTH, DEFAULT_REFERENCE_SCAN_DEPTH,
};

const APP_DIR_NAME: &str = "AgentDock";
const CONFIG_FILE_NAME: &str = "config.json";
const CATALOG_FILE_NAME: &str = "mcp-catalog.json";

/// Resolved global directory roots for each supported agent adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRoots {
    pub antigravity_dir: PathBuf,
    pub codex_dir: PathBuf,
    pub claude_dir: PathBuf,
    pub claude_json: PathBuf,
    pub shared_agents_dir: PathBuf,
}

/// Resolves global agent directories from custom `AppConfig.agent_paths` or home defaults.
#[must_use]
pub fn resolve_agent_roots(cfg: &AppConfig, home: &Path) -> AgentRoots {
    let antigravity_dir = cfg
        .agent_paths
        .antigravity_dir
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| expand_user_path(home, s))
        .unwrap_or_else(|| home.join(".gemini"));

    let codex_dir = cfg
        .agent_paths
        .codex_dir
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| expand_user_path(home, s))
        .unwrap_or_else(|| home.join(".codex"));

    let custom_claude = cfg
        .agent_paths
        .claude_dir
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| expand_user_path(home, s));

    let claude_json = match &custom_claude {
        Some(dir) => dir.join(".claude.json"),
        None => home.join(".claude.json"),
    };
    let claude_dir = custom_claude.unwrap_or_else(|| home.join(".claude"));

    AgentRoots {
        antigravity_dir,
        codex_dir,
        claude_dir,
        claude_json,
        shared_agents_dir: home.join(".agents"),
    }
}

/// Expands a leading `~` or `~/` against `home`, returning a clean [`PathBuf`].
#[must_use]
pub fn expand_user_path(home: &Path, raw: &str) -> PathBuf {
    let trimmed = raw.trim();
    if trimmed == "~" {
        return home.to_path_buf();
    }
    if let Some(rest) = trimmed.strip_prefix("~/") {
        return home.join(rest);
    }
    PathBuf::from(trimmed)
}

/// Resolves a normalized canonical path key string for project-indexed maps.
#[must_use]
pub fn canonical_path_key(home: &Path, raw: &str) -> String {
    let expanded = expand_user_path(home, raw);
    let canon = fs::canonicalize(&expanded).unwrap_or(expanded);
    canon.to_string_lossy().into_owned()
}

/// Resolves effective project roots, transparently promoting legacy `projects_root` if needed.
#[must_use]
pub fn effective_project_roots(cfg: &AppConfig) -> Vec<ProjectRoot> {
    if !cfg.project_roots.is_empty() {
        return cfg.project_roots.clone();
    }
    if let Some(legacy) = cfg
        .projects_root
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return vec![ProjectRoot {
            id: "default-projects".to_owned(),
            label: folder_display_name(Path::new(legacy), "Projects"),
            path: legacy.to_owned(),
            depth: DEFAULT_PROJECT_SCAN_DEPTH,
            mode: ProjectScanMode::ExactDepth,
        }];
    }
    Vec::new()
}

/// Normalizes persisted configuration (clamps depth limits and migrates legacy fields).
#[must_use]
pub fn normalize_config(mut cfg: AppConfig) -> AppConfig {
    if cfg.project_roots.is_empty() {
        if let Some(legacy) = cfg
            .projects_root
            .take()
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
        {
            let label = folder_display_name(Path::new(&legacy), "Projects");
            cfg.project_roots.push(ProjectRoot {
                id: "default-projects".to_owned(),
                label,
                path: legacy,
                depth: DEFAULT_PROJECT_SCAN_DEPTH,
                mode: ProjectScanMode::ExactDepth,
            });
        }
    } else {
        cfg.projects_root = None;
    }

    cfg.project_roots = cfg
        .project_roots
        .into_iter()
        .filter_map(|mut root| {
            let trimmed = root.path.trim().to_owned();
            if trimmed.is_empty() {
                return None;
            }
            root.path = trimmed;
            if root.label.trim().is_empty() {
                root.label = folder_display_name(Path::new(&root.path), "Projects");
            } else {
                root.label = root.label.trim().to_owned();
            }
            root.depth = root.depth.clamp(1, 6);
            Some(root)
        })
        .collect();

    cfg.library_sources = cfg
        .library_sources
        .into_iter()
        .filter_map(|mut src| {
            let trimmed = src.path.trim().to_owned();
            if trimmed.is_empty() {
                return None;
            }
            src.path = trimmed;
            if src.label.trim().is_empty() {
                src.label = folder_display_name(Path::new(&src.path), "Library");
            } else {
                src.label = src.label.trim().to_owned();
            }
            src.depth = if src.depth == 0 {
                DEFAULT_LIBRARY_SCAN_DEPTH
            } else {
                src.depth.clamp(1, 5)
            };
            Some(src)
        })
        .collect();

    cfg.support_docs_root = cfg
        .support_docs_root
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty());

    cfg.pinned_projects = cfg
        .pinned_projects
        .into_iter()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect();

    cfg.reference_scan_depth = if cfg.reference_scan_depth == 0 {
        DEFAULT_REFERENCE_SCAN_DEPTH
    } else {
        cfg.reference_scan_depth.clamp(1, 10)
    };

    cfg.agent_paths = AgentPaths {
        antigravity_dir: clean_opt(cfg.agent_paths.antigravity_dir),
        codex_dir: clean_opt(cfg.agent_paths.codex_dir),
        claude_dir: clean_opt(cfg.agent_paths.claude_dir),
    };

    cfg.hidden_projects = dedup_non_empty(cfg.hidden_projects);

    let mut seen_tag_ids = BTreeSet::new();
    cfg.tag_definitions = cfg
        .tag_definitions
        .into_iter()
        .filter_map(|mut tag| {
            tag.id = tag.id.trim().to_owned();
            tag.label = tag.label.trim().to_owned();
            tag.auto_folder = clean_opt(tag.auto_folder);
            if tag.id.is_empty() || tag.label.is_empty() || !seen_tag_ids.insert(tag.id.clone()) {
                return None;
            }
            Some(tag)
        })
        .collect();

    let mut seen_set_ids = BTreeSet::new();
    cfg.sets = cfg
        .sets
        .into_iter()
        .filter_map(|mut set| {
            set.id = set.id.trim().to_owned();
            set.name = set.name.trim().to_owned();
            set.description = set.description.trim().to_owned();
            if set.id.is_empty() || set.name.is_empty() || !seen_set_ids.insert(set.id.clone()) {
                return None;
            }
            set.skill_ids = dedup_non_empty(set.skill_ids);
            set.mcp_names = dedup_non_empty(set.mcp_names);
            set.rule_filenames = dedup_non_empty(set.rule_filenames);
            Some(set)
        })
        .collect();

    cfg
}

fn dedup_non_empty(items: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for item in items {
        let trimmed = item.trim().to_owned();
        if !trimmed.is_empty() && seen.insert(trimmed.clone()) {
            out.push(trimmed);
        }
    }
    out
}

fn clean_opt(opt: Option<String>) -> Option<String> {
    opt.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty())
}

fn folder_display_name(path: &Path, fallback: &str) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(fallback)
        .to_owned()
}

/// Resolves the Application Support directory for AgentDock (`~/Library/Application Support/AgentDock`).
///
/// # Errors
/// Returns [`AppError::DirectoryUnavailable`] if the OS data directory cannot be located.
pub fn app_support_dir() -> Result<PathBuf, AppError> {
    let base = dirs::data_dir().ok_or(AppError::DirectoryUnavailable("Application Support"))?;
    Ok(base.join(APP_DIR_NAME))
}

/// Resolves the path to `config.json`.
///
/// # Errors
/// Returns [`AppError::DirectoryUnavailable`] if Application Support cannot be resolved.
pub fn config_file_path() -> Result<PathBuf, AppError> {
    Ok(app_support_dir()?.join(CONFIG_FILE_NAME))
}

/// Resolves the path to `mcp-catalog.json` for cached MCP token probes.
///
/// # Errors
/// Returns [`AppError::DirectoryUnavailable`] if Application Support cannot be resolved.
pub fn mcp_catalog_path() -> Result<PathBuf, AppError> {
    Ok(app_support_dir()?.join(CATALOG_FILE_NAME))
}

/// Constructs portable first-run defaults by inspecting common existing directories under `home`.
#[must_use]
pub fn default_config_for_home(home: &Path) -> AppConfig {
    let mut project_roots = Vec::new();
    let mut seen_project_roots = BTreeSet::new();
    let project_candidates = [
        "Projects",
        "Developer",
        "Code",
        "Workspace",
        "workspace",
        "Repos",
        "src",
        "APP-PROJECTS",
    ];

    for rel in project_candidates {
        let candidate = home.join(rel);
        if !candidate.is_dir() {
            continue;
        }
        let canonical = fs::canonicalize(&candidate).unwrap_or_else(|_| candidate.clone());
        if !seen_project_roots.insert(canonical) {
            continue;
        }
        let (depth, mode) = infer_project_root_layout(&candidate);
        project_roots.push(ProjectRoot {
            id: format!("root-{}", project_roots.len() + 1),
            label: folder_display_name(&candidate, "Projects"),
            path: candidate.to_string_lossy().into_owned(),
            depth,
            mode,
        });
    }

    let mut library_sources = Vec::new();
    let mut seen_libraries = BTreeSet::new();
    let library_candidates = [
        "Skills",
        "skills",
        "AgentLibrary",
        ".config/agentdock/library",
        ".agents/library",
        "BRAIN-LOCAL/09-ai-os/skills",
        "BRAIN-LOCAL/09-ai-os/rules",
    ];

    for rel in library_candidates {
        let candidate = home.join(rel);
        if !candidate.is_dir() {
            continue;
        }
        let canonical = fs::canonicalize(&candidate).unwrap_or_else(|_| candidate.clone());
        if !seen_libraries.insert(canonical) {
            continue;
        }
        let label = infer_library_label(&candidate, home);
        library_sources.push(LibrarySource {
            id: format!("lib-{}", library_sources.len() + 1),
            label,
            path: candidate.to_string_lossy().into_owned(),
            depth: DEFAULT_LIBRARY_SCAN_DEPTH,
        });
    }

    let support_docs_root = ["Notes", "BRAIN-LOCAL"]
        .into_iter()
        .map(|rel| home.join(rel))
        .find(|p| p.is_dir())
        .map(|p| p.to_string_lossy().into_owned());

    AppConfig {
        project_roots,
        library_sources,
        support_docs_root,
        ..AppConfig::default()
    }
}

fn infer_project_root_layout(root: &Path) -> (u8, ProjectScanMode) {
    let Ok(entries) = fs::read_dir(root) else {
        return (DEFAULT_PROJECT_SCAN_DEPTH, ProjectScanMode::ExactDepth);
    };

    let mut direct_markers = 0usize;
    let mut nested_markers = 0usize;

    for entry in entries.flatten().take(30) {
        let p = entry.path();
        let Some(name) = p.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !p.is_dir() || name.starts_with('.') {
            continue;
        }
        if directory_looks_like_project(&p) {
            direct_markers += 1;
        } else if let Ok(sub_entries) = fs::read_dir(&p) {
            for sub in sub_entries.flatten().take(15) {
                let sp = sub.path();
                let Some(sub_name) = sp.file_name().and_then(|n| n.to_str()) else {
                    continue;
                };
                if sp.is_dir() && !sub_name.starts_with('.') && directory_looks_like_project(&sp) {
                    nested_markers += 1;
                    break;
                }
            }
        }
    }

    if direct_markers > 0 && nested_markers == 0 {
        (1, ProjectScanMode::ExactDepth)
    } else if direct_markers > 0 && nested_markers > 0 {
        (3, ProjectScanMode::AutoDetect)
    } else {
        (DEFAULT_PROJECT_SCAN_DEPTH, ProjectScanMode::ExactDepth)
    }
}

/// Checks common version control, agent config, or package manifest markers for auto-detection.
#[must_use]
pub fn directory_looks_like_project(dir: &Path) -> bool {
    const MARKERS: &[&str] = &[
        ".git",
        ".agents",
        ".agent",
        ".claude",
        ".codex",
        ".gemini",
        "AGENTS.md",
        "CLAUDE.md",
        "GEMINI.md",
        ".mcp.json",
        "package.json",
        "Cargo.toml",
        "pyproject.toml",
        "go.mod",
        "Package.swift",
    ];
    MARKERS.iter().any(|marker| dir.join(marker).exists())
}

fn infer_library_label(dir: &Path, home: &Path) -> String {
    let mut cur = Some(dir);
    let mut is_obsidian = false;
    while let Some(path) = cur {
        if path.join(".obsidian").is_dir() {
            is_obsidian = true;
            break;
        }
        if path == home {
            break;
        }
        cur = path.parent();
    }

    let folder = folder_display_name(dir, "Library");
    if is_obsidian {
        if folder.eq_ignore_ascii_case("rules") {
            "Obsidian Rules".to_owned()
        } else {
            "Obsidian".to_owned()
        }
    } else if folder.eq_ignore_ascii_case("skills") {
        "Skills Library".to_owned()
    } else if folder.eq_ignore_ascii_case("rules") {
        "Rules Library".to_owned()
    } else {
        folder
    }
}

/// Loads `config.json` from Application Support, or returns defaults if not yet saved.
///
/// # Errors
/// Returns [`AppError`] if the home/data directory cannot be resolved or an existing config file is unreadable/corrupt.
pub fn load_config() -> Result<AppConfig, AppError> {
    let path = config_file_path()?;
    load_config_from(&path, &home_dir()?)
}

/// Loads configuration from an explicit path, falling back to defaults relative to `home` when absent.
///
/// # Errors
/// Returns [`AppError`] on I/O or JSON parse failure when the file exists.
pub fn load_config_from(path: &Path, home: &Path) -> Result<AppConfig, AppError> {
    if !path.exists() {
        return Ok(default_config_for_home(home));
    }

    let raw = fs::read_to_string(path).map_err(|e| AppError::io(path, e))?;
    let parsed: AppConfig = serde_json::from_str(&raw)?;
    Ok(normalize_config(parsed))
}

/// Atomically writes `config` to Application Support (`config.json.tmp` -> `config.json`).
///
/// # Errors
/// Returns [`AppError`] if the target directory cannot be created or writing/renaming fails.
pub fn save_config(config: &AppConfig) -> Result<AppConfig, AppError> {
    let normalized = normalize_config(config.clone());
    let path = config_file_path()?;
    save_config_to(&normalized, &path)?;
    Ok(normalized)
}

/// Atomically writes `config` to an explicit path.
///
/// # Errors
/// Returns [`AppError`] on directory creation, serialization, write, or rename failure.
pub fn save_config_to(config: &AppConfig, path: &Path) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| AppError::io(parent, e))?;
    }

    let json = serde_json::to_string_pretty(config)?;
    let tmp_path = path.with_extension("json.tmp");
    fs::write(&tmp_path, json).map_err(|e| AppError::io(&tmp_path, e))?;
    fs::rename(&tmp_path, path).map_err(|e| AppError::io(path, e))?;
    Ok(())
}

/// Resolves the current user's home directory.
///
/// # Errors
/// Returns [`AppError::DirectoryUnavailable`] if `dirs::home_dir` returns `None`.
pub fn home_dir() -> Result<PathBuf, AppError> {
    dirs::home_dir().ok_or(AppError::DirectoryUnavailable("Home"))
}

/// Toggles `project_path` inside `cfg.pinned_projects` using canonical path matching.
///
/// Returns `true` if the project is now pinned, `false` if unpinned.
///
/// # Errors
/// Returns [`AppError::InvalidPath`] when pinning a path that does not exist as a directory.
pub fn toggle_pinned_project_in_config(
    cfg: &mut AppConfig,
    home: &Path,
    project_path: &str,
) -> Result<bool, AppError> {
    let target = expand_user_path(home, project_path);
    let target_canon = fs::canonicalize(&target).unwrap_or_else(|_| target.clone());

    let before_len = cfg.pinned_projects.len();
    cfg.pinned_projects.retain(|existing| {
        let ex_path = expand_user_path(home, existing);
        let ex_canon = fs::canonicalize(&ex_path).unwrap_or(ex_path);
        ex_canon != target_canon
    });

    if cfg.pinned_projects.len() < before_len {
        return Ok(false);
    }

    if !target.is_dir() {
        return Err(AppError::InvalidPath(format!(
            "Project directory does not exist: {}",
            target.display()
        )));
    }

    cfg.pinned_projects
        .push(target.to_string_lossy().into_owned());
    Ok(true)
}

/// Toggles `project_path` inside `cfg.hidden_projects` using canonical path matching.
///
/// Returns `true` if the project is now hidden, `false` if restored.
///
/// # Errors
/// Returns [`AppError::InvalidPath`] when hiding an empty path.
pub fn toggle_hidden_project_in_config(
    cfg: &mut AppConfig,
    home: &Path,
    project_path: &str,
) -> Result<bool, AppError> {
    let trimmed = project_path.trim();
    if trimmed.is_empty() {
        return Err(AppError::InvalidPath(
            "Project path cannot be empty".to_owned(),
        ));
    }
    let target = expand_user_path(home, trimmed);
    let target_canon = fs::canonicalize(&target).unwrap_or_else(|_| target.clone());

    let before_len = cfg.hidden_projects.len();
    cfg.hidden_projects.retain(|existing| {
        let ex_path = expand_user_path(home, existing);
        let ex_canon = fs::canonicalize(&ex_path).unwrap_or(ex_path);
        ex_canon != target_canon
    });

    if cfg.hidden_projects.len() < before_len {
        return Ok(false);
    }

    cfg.hidden_projects
        .push(target_canon.to_string_lossy().into_owned());
    Ok(true)
}

/// Sets manual tags and muted auto-tags for a project keyed by canonical path.
///
/// # Errors
/// Returns [`AppError::InvalidPath`] if `project_path` is empty.
pub fn set_project_tags_in_config(
    cfg: &mut AppConfig,
    home: &Path,
    project_path: &str,
    manual_tag_ids: Vec<String>,
    muted_auto_tag_ids: Vec<String>,
) -> Result<(), AppError> {
    if project_path.trim().is_empty() {
        return Err(AppError::InvalidPath(
            "Project path cannot be empty".to_owned(),
        ));
    }
    let key = canonical_path_key(home, project_path);

    let manual = dedup_non_empty(manual_tag_ids);
    if manual.is_empty() {
        cfg.project_manual_tags.remove(&key);
    } else {
        cfg.project_manual_tags.insert(key.clone(), manual);
    }

    let muted = dedup_non_empty(muted_auto_tag_ids);
    if muted.is_empty() {
        cfg.project_muted_auto_tags.remove(&key);
    } else {
        cfg.project_muted_auto_tags.insert(key, muted);
    }

    Ok(())
}

/// Inserts or updates a tag definition (matching by ID, or case-insensitive `auto_folder` when set).
///
/// # Errors
/// Returns [`AppError::InvalidPath`] if `id` or `label` is empty.
pub fn upsert_tag_definition_in_config(
    cfg: &mut AppConfig,
    mut tag: TagDefinition,
) -> Result<(), AppError> {
    tag.id = tag.id.trim().to_owned();
    tag.label = tag.label.trim().to_owned();
    tag.auto_folder = clean_opt(tag.auto_folder);

    if tag.id.is_empty() || tag.label.is_empty() {
        return Err(AppError::InvalidPath(
            "Tag ID and label cannot be empty".to_owned(),
        ));
    }

    if let Some(existing) = cfg.tag_definitions.iter_mut().find(|existing| {
        existing.id == tag.id
            || (tag.auto_folder.is_some()
                && existing.auto_folder.as_deref().is_some_and(|af| {
                    af.eq_ignore_ascii_case(tag.auto_folder.as_deref().unwrap_or(""))
                }))
    }) {
        existing.label = tag.label;
        existing.color = tag.color;
        existing.auto_folder = tag.auto_folder;
        existing.hidden = tag.hidden;
    } else {
        cfg.tag_definitions.push(tag);
    }

    Ok(())
}

/// Deletes a tag definition and cleans up references across all projects.
pub fn delete_tag_definition_in_config(cfg: &mut AppConfig, tag_id: &str) {
    cfg.tag_definitions.retain(|t| t.id != tag_id);
    cfg.project_manual_tags.retain(|_, list| {
        list.retain(|id| id != tag_id);
        !list.is_empty()
    });
    cfg.project_muted_auto_tags.retain(|_, list| {
        list.retain(|id| id != tag_id);
        !list.is_empty()
    });
}

/// Inserts or updates a context set definition.
///
/// # Errors
/// Returns [`AppError::InvalidPath`] if `id` or `name` is empty.
pub fn upsert_set_in_config(cfg: &mut AppConfig, mut set: ContextSet) -> Result<(), AppError> {
    set.id = set.id.trim().to_owned();
    set.name = set.name.trim().to_owned();
    set.description = set.description.trim().to_owned();
    set.skill_ids = dedup_non_empty(set.skill_ids);
    set.mcp_names = dedup_non_empty(set.mcp_names);
    set.rule_filenames = dedup_non_empty(set.rule_filenames);

    if set.id.is_empty() || set.name.is_empty() {
        return Err(AppError::InvalidPath(
            "Set ID and name cannot be empty".to_owned(),
        ));
    }

    if let Some(existing) = cfg.sets.iter_mut().find(|s| s.id == set.id) {
        *existing = set;
    } else {
        cfg.sets.push(set);
    }

    Ok(())
}

/// Deletes a context set and strips it from all project active-set lists.
pub fn delete_set_in_config(cfg: &mut AppConfig, set_id: &str) {
    cfg.sets.retain(|s| s.id != set_id);
    cfg.project_active_sets.retain(|_, list| {
        list.retain(|id| id != set_id);
        !list.is_empty()
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn defaults_populate_detected_directories_across_layouts() {
        let tmp = TempDir::new().expect("temp dir");
        let home = tmp.path();
        let flat_code = home.join("Code");
        fs::create_dir_all(flat_code.join("demo-app/.git")).expect("create flat project");

        let vault_skills = home.join("MyVault/skills");
        fs::create_dir_all(home.join("MyVault/.obsidian")).expect("obsidian marker");
        fs::create_dir_all(&vault_skills).expect("create vault skills");

        let cfg = default_config_for_home(home);
        assert_eq!(cfg.project_roots.len(), 1);
        assert_eq!(cfg.project_roots[0].path, flat_code.to_string_lossy());
        assert_eq!(cfg.project_roots[0].depth, 1);
        assert_eq!(cfg.project_roots[0].mode, ProjectScanMode::ExactDepth);
        assert!(cfg.active_agents.antigravity);
        assert!(cfg.active_agents.codex);
        assert!(!cfg.active_agents.claude);
        assert!(cfg.git_hygiene);

        let generic_skills = home.join("Skills");
        fs::create_dir_all(&generic_skills).expect("generic skills");
        let cfg2 = default_config_for_home(home);
        assert_eq!(cfg2.library_sources.len(), 1);
        assert_eq!(cfg2.library_sources[0].label, "Skills Library");
    }

    #[test]
    fn migrates_legacy_projects_root_and_expands_tilde() {
        let tmp = TempDir::new().expect("temp dir");
        let cfg_path = tmp.path().join("AgentDock/config.json");
        fs::create_dir_all(cfg_path.parent().expect("parent")).expect("mkdir");
        fs::write(
            &cfg_path,
            r#"{"projectsRoot":"~/Projects","librarySources":[{"id":"l1","label":"Custom","path":"~/Skills"}],"ignoreFolders":[],"gitHygiene":true}"#,
        )
        .expect("write legacy");

        let loaded = load_config_from(&cfg_path, tmp.path()).expect("load legacy");
        assert!(loaded.projects_root.is_none());
        assert_eq!(loaded.project_roots.len(), 1);
        assert_eq!(loaded.project_roots[0].path, "~/Projects");
        assert_eq!(loaded.project_roots[0].depth, 2);
        assert_eq!(loaded.library_sources[0].depth, 2);
        assert_eq!(loaded.reference_scan_depth, DEFAULT_REFERENCE_SCAN_DEPTH);
        assert_eq!(
            expand_user_path(tmp.path(), "~/Projects"),
            tmp.path().join("Projects")
        );
    }

    #[test]
    fn round_trips_config_atomically() {
        let tmp = TempDir::new().expect("temp dir");
        let cfg_path = tmp.path().join("AgentDock/config.json");
        let mut cfg = default_config_for_home(tmp.path());
        cfg.pinned_projects.push("/tmp/custom-proj".to_owned());
        cfg.reference_scan_depth = 5;

        save_config_to(&cfg, &cfg_path).expect("save config");
        let loaded = load_config_from(&cfg_path, tmp.path()).expect("load config");
        assert_eq!(loaded, normalize_config(cfg));
    }

    #[test]
    fn toggles_pinned_projects_with_tilde_and_canonical_matching() {
        let tmp = TempDir::new().expect("temp dir");
        let home = tmp.path();
        let proj = home.join("Code/demo");
        fs::create_dir_all(&proj).expect("create proj");

        let mut cfg = AppConfig::default();
        assert!(toggle_pinned_project_in_config(&mut cfg, home, "~/Code/demo").expect("pin"));
        assert_eq!(cfg.pinned_projects.len(), 1);

        // Toggling via absolute path unpins the tilde-expanded canonical match
        assert!(
            !toggle_pinned_project_in_config(&mut cfg, home, &proj.to_string_lossy())
                .expect("unpin")
        );
        assert!(cfg.pinned_projects.is_empty());
    }

    #[test]
    fn manages_hidden_projects_tags_and_sets_cleanly() {
        let tmp = TempDir::new().expect("temp dir");
        let home = tmp.path();
        let proj = home.join("Code/demo");
        fs::create_dir_all(&proj).expect("create proj");

        let mut cfg = AppConfig::default();
        assert!(toggle_hidden_project_in_config(&mut cfg, home, "~/Code/demo").expect("hide"));
        assert_eq!(cfg.hidden_projects.len(), 1);
        assert!(
            !toggle_hidden_project_in_config(&mut cfg, home, &proj.to_string_lossy())
                .expect("unhide")
        );
        assert!(cfg.hidden_projects.is_empty());

        let tag = TagDefinition {
            id: "auto:work".to_owned(),
            label: "Client Work".to_owned(),
            color: crate::models::TagColor::Blue,
            auto_folder: Some("work".to_owned()),
            hidden: false,
        };
        upsert_tag_definition_in_config(&mut cfg, tag).expect("upsert tag");
        set_project_tags_in_config(
            &mut cfg,
            home,
            "~/Code/demo",
            vec!["auto:work".to_owned()],
            vec![],
        )
        .expect("set tags");
        assert_eq!(cfg.project_manual_tags.len(), 1);

        delete_tag_definition_in_config(&mut cfg, "auto:work");
        assert!(cfg.tag_definitions.is_empty());
        assert!(cfg.project_manual_tags.is_empty());

        let set = ContextSet {
            id: "set-1".to_owned(),
            name: "Web Stack".to_owned(),
            description: "Frontend tools".to_owned(),
            skill_ids: vec!["skill-a".to_owned(), "skill-a".to_owned()],
            mcp_names: vec!["mcp-1".to_owned()],
            rule_filenames: vec![],
        };
        upsert_set_in_config(&mut cfg, set).expect("upsert set");
        assert_eq!(cfg.sets.len(), 1);
        assert_eq!(cfg.sets[0].skill_ids, vec!["skill-a"]);

        let key = canonical_path_key(home, "~/Code/demo");
        cfg.project_active_sets
            .insert(key, vec!["set-1".to_owned()]);
        delete_set_in_config(&mut cfg, "set-1");
        assert!(cfg.sets.is_empty());
        assert!(cfg.project_active_sets.is_empty());
    }
}

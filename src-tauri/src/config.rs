//! Persistent configuration management stored in macOS Application Support.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::AppError;
use crate::models::{ActiveAgents, AppConfig, LibrarySource};

const APP_DIR_NAME: &str = "AgentDock";
const CONFIG_FILE_NAME: &str = "config.json";
const CATALOG_FILE_NAME: &str = "mcp-catalog.json";

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

/// Constructs sensible first-run defaults relative to the given home directory.
#[must_use]
pub fn default_config_for_home(home: &Path) -> AppConfig {
    let projects_candidate = home.join("APP-PROJECTS");
    let projects_root = projects_candidate
        .is_dir()
        .then(|| projects_candidate.to_string_lossy().into_owned());

    let mut library_sources = Vec::new();
    let obsidian_skills = home.join("BRAIN-LOCAL").join("09-ai-os").join("skills");
    if obsidian_skills.is_dir() {
        library_sources.push(LibrarySource {
            id: "obsidian-skills".to_owned(),
            label: "Obsidian".to_owned(),
            path: obsidian_skills.to_string_lossy().into_owned(),
        });
    }

    let obsidian_rules = home.join("BRAIN-LOCAL").join("09-ai-os").join("rules");
    if obsidian_rules.is_dir() {
        library_sources.push(LibrarySource {
            id: "obsidian-rules".to_owned(),
            label: "Obsidian Rules".to_owned(),
            path: obsidian_rules.to_string_lossy().into_owned(),
        });
    }

    let support_docs_candidate = home.join("BRAIN-LOCAL");
    let support_docs_root = support_docs_candidate
        .is_dir()
        .then(|| support_docs_candidate.to_string_lossy().into_owned());

    AppConfig {
        projects_root,
        library_sources,
        support_docs_root,
        pinned_projects: Vec::new(),
        ignore_folders: vec![
            ".git".to_owned(),
            "node_modules".to_owned(),
            "_archive".to_owned(),
            "Archive".to_owned(),
            ".DS_Store".to_owned(),
        ],
        active_agents: ActiveAgents::default(),
        git_hygiene: true,
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
    Ok(parsed)
}

/// Atomically writes `config` to Application Support (`config.json.tmp` -> `config.json`).
///
/// # Errors
/// Returns [`AppError`] if the target directory cannot be created or writing/renaming fails.
pub fn save_config(config: &AppConfig) -> Result<AppConfig, AppError> {
    let path = config_file_path()?;
    save_config_to(config, &path)?;
    Ok(config.clone())
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn defaults_populate_detected_directories() {
        let tmp = TempDir::new().expect("temp dir");
        let home = tmp.path();
        fs::create_dir_all(home.join("APP-PROJECTS")).expect("create projects");
        fs::create_dir_all(home.join("BRAIN-LOCAL/09-ai-os/skills")).expect("create skills");

        let cfg = default_config_for_home(home);
        assert_eq!(
            cfg.projects_root.as_deref(),
            Some(home.join("APP-PROJECTS").to_string_lossy().as_ref())
        );
        assert_eq!(cfg.library_sources.len(), 1);
        assert_eq!(cfg.library_sources[0].label, "Obsidian");
        assert!(cfg.active_agents.antigravity);
        assert!(cfg.active_agents.codex);
        assert!(!cfg.active_agents.claude);
        assert!(cfg.git_hygiene);
    }

    #[test]
    fn round_trips_config_atomically() {
        let tmp = TempDir::new().expect("temp dir");
        let cfg_path = tmp.path().join("AgentDock/config.json");
        let mut cfg = default_config_for_home(tmp.path());
        cfg.pinned_projects.push("/tmp/custom-proj".to_owned());

        save_config_to(&cfg, &cfg_path).expect("save config");
        let loaded = load_config_from(&cfg_path, tmp.path()).expect("load config");
        assert_eq!(loaded, cfg);
    }
}

//! Tauri IPC command handlers exposed to the frontend switchboard.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use tauri::{AppHandle, State};

use crate::config;
use crate::debloat;
use crate::error::AppError;
use crate::mcp_catalog;
use crate::mcp_probe;
use crate::mcp_writer;
use crate::models::{
    AfterMoveAction, AppConfig, DesiredState, LinkSupportDocRequest, McpCatalogEntry,
    McpProbeResult, McpServerDraft, MoveExecutionResult, MovePreviewItem, RuleTriggerUpdate,
    ScopeSnapshot, SupportDocMode, ToggleOutcome, ToggleScope, WorkspaceOverview,
};
use crate::rules_writer;
use crate::scanner;
use crate::support_docs;
use crate::symlinks;
use crate::watcher::WatcherManager;

/// Loads the persisted configuration from Application Support (or first-run defaults).
///
/// # Errors
/// Returns a stringified [`AppError`] if reading or parsing `config.json` fails.
#[tauri::command]
pub fn get_config() -> Result<AppConfig, String> {
    config::load_config().map_err(Into::into)
}

/// Validates and atomically persists updated configuration settings and refreshes file watches.
///
/// # Errors
/// Returns a stringified [`AppError`] if saving `config.json` fails.
#[tauri::command]
pub fn save_config(
    config: AppConfig,
    app: AppHandle,
    watcher_state: State<'_, Mutex<WatcherManager>>,
) -> Result<AppConfig, String> {
    let saved = config::save_config(&config).map_err(String::from)?;
    if let Ok(home) = config::home_dir() {
        if let Ok(mut mgr) = watcher_state.lock() {
            mgr.refresh_watches(&app, &saved, &home, None);
        }
    }
    Ok(saved)
}

/// Performs a full read-only scan of Global scope and all 2-level categorized Projects.
///
/// # Errors
/// Returns a stringified [`AppError`] if config loading or workspace scanning fails.
#[tauri::command]
pub fn scan_workspace() -> Result<WorkspaceOverview, String> {
    let cfg = config::load_config().map_err(String::from)?;
    scanner::scan_workspace(&cfg).map_err(Into::into)
}

/// Performs a read-only scan of a specific project directory against Global baseline.
///
/// # Errors
/// Returns a stringified [`AppError`] if the project directory is missing or scanning fails.
#[tauri::command]
pub fn scan_project(project_path: String, category: String) -> Result<ScopeSnapshot, String> {
    let cfg = config::load_config().map_err(String::from)?;
    scanner::scan_project(&cfg, Path::new(&project_path), &category).map_err(Into::into)
}

/// Updates the FSEvents watcher to monitor the currently selected project directory.
///
/// # Errors
/// Returns a stringified error if the watcher lock is poisoned.
#[tauri::command]
pub fn set_watched_project(
    project_path: Option<String>,
    app: AppHandle,
    watcher_state: State<'_, Mutex<WatcherManager>>,
) -> Result<(), String> {
    let cfg = config::load_config().map_err(String::from)?;
    let home = config::home_dir().map_err(String::from)?;
    let mut mgr = watcher_state
        .lock()
        .map_err(|_| "Watcher lock poisoned".to_owned())?;
    mgr.set_project(&app, &cfg, &home, project_path.map(PathBuf::from));
    Ok(())
}

/// Reveals a file or folder in macOS Finder (`open -R`).
///
/// # Errors
/// Returns a stringified [`AppError`] if the path does not exist or launching `open` fails.
#[tauri::command]
pub fn reveal_in_finder(path: String) -> Result<(), String> {
    let target = Path::new(&path);
    if !target.exists() && std::fs::symlink_metadata(target).is_err() {
        return Err(AppError::InvalidPath(format!("Path does not exist: {path}")).into());
    }

    let status = Command::new("/usr/bin/open")
        .arg("-R")
        .arg(target)
        .status()
        .map_err(|e| AppError::Command(format!("failed to invoke Finder: {e}")))?;

    if !status.success() {
        return Err(AppError::Command(format!("Finder exited with status {status}")).into());
    }
    Ok(())
}

/// Opens a file (such as `SKILL.md` or a rule markdown file) in the user's default macOS app.
///
/// # Errors
/// Returns a stringified [`AppError`] if the path does not exist or launching `open` fails.
#[tauri::command]
pub fn open_in_default_app(path: String) -> Result<(), String> {
    let target = Path::new(&path);
    if !target.exists() {
        return Err(AppError::InvalidPath(format!("File does not exist: {path}")).into());
    }

    let status = Command::new("/usr/bin/open")
        .arg(target)
        .status()
        .map_err(|e| AppError::Command(format!("failed to open file: {e}")))?;

    if !status.success() {
        return Err(AppError::Command(format!("open exited with status {status}")).into());
    }
    Ok(())
}

/// Toggles a skill ON or OFF via symlinks in Global or Project scope.
///
/// # Errors
/// Returns a stringified [`AppError`] on invalid inputs or filesystem failure.
#[tauri::command]
pub fn toggle_skill(
    scope: ToggleScope,
    skill_id: String,
    canonical_skill_dir: String,
    enable: bool,
) -> Result<ToggleOutcome, String> {
    let cfg = config::load_config().map_err(String::from)?;
    let home = config::home_dir().map_err(String::from)?;
    let desired = if enable {
        DesiredState::Enable
    } else {
        DesiredState::Disable
    };
    symlinks::toggle_skill(
        &cfg,
        &home,
        &scope,
        &skill_id,
        Path::new(&canonical_skill_dir),
        desired,
    )
    .map_err(Into::into)
}

/// Unlinks a verified broken symlink safely.
///
/// # Errors
/// Returns a stringified [`AppError`] if the path is not a broken symlink or cannot be unlinked.
#[tauri::command]
pub fn unlink_broken_symlink(
    target_path: String,
    project_path: Option<String>,
) -> Result<(), String> {
    let cfg = config::load_config().map_err(String::from)?;
    symlinks::unlink_broken_symlink(
        &cfg,
        Path::new(&target_path),
        project_path.as_deref().map(Path::new),
    )
    .map_err(Into::into)
}

/// Toggles a reusable Library rule `.md` file ON or OFF via symlinks.
///
/// # Errors
/// Returns a stringified [`AppError`] on invalid inputs or filesystem failure.
#[tauri::command]
pub fn toggle_library_rule(
    scope: ToggleScope,
    rule_filename: String,
    canonical_rule_file: String,
    enable: bool,
) -> Result<ToggleOutcome, String> {
    let cfg = config::load_config().map_err(String::from)?;
    let home = config::home_dir().map_err(String::from)?;
    let desired = if enable {
        DesiredState::Enable
    } else {
        DesiredState::Disable
    };
    symlinks::toggle_library_rule(
        &cfg,
        &home,
        &scope,
        &rule_filename,
        Path::new(&canonical_rule_file),
        desired,
    )
    .map_err(Into::into)
}

/// Previews moving unmanaged real skill directories into a Library source.
///
/// # Errors
/// Returns a stringified [`AppError`] if the library folder or skill IDs are invalid.
#[tauri::command]
pub fn preview_move_skills_to_library(
    skill_ids: Vec<String>,
    target_library_path: String,
    scope: ToggleScope,
) -> Result<Vec<MovePreviewItem>, String> {
    let cfg = config::load_config().map_err(String::from)?;
    let home = config::home_dir().map_err(String::from)?;
    debloat::preview_move_skills_to_library(
        &cfg,
        &home,
        &skill_ids,
        Path::new(&target_library_path),
        &scope,
    )
    .map_err(Into::into)
}

/// Executes moving unmanaged real skill directories into a Library source and unlinks/relinks them.
///
/// # Errors
/// Returns a stringified [`AppError`] on move or symlink failure.
#[tauri::command]
pub fn execute_move_skills_to_library(
    skill_ids: Vec<String>,
    target_library_path: String,
    scope: ToggleScope,
    after_move: AfterMoveAction,
) -> Result<MoveExecutionResult, String> {
    let cfg = config::load_config().map_err(String::from)?;
    let home = config::home_dir().map_err(String::from)?;
    debloat::execute_move_skills_to_library(
        &cfg,
        &home,
        &skill_ids,
        Path::new(&target_library_path),
        &scope,
        after_move,
    )
    .map_err(Into::into)
}

/// Updates only the YAML frontmatter trigger fields on an Antigravity or Library rule file.
///
/// # Errors
/// Returns a stringified [`AppError`] on validation, backup, or write failure.
#[tauri::command]
pub fn update_rule_trigger(rule_path: String, update: RuleTriggerUpdate) -> Result<(), String> {
    rules_writer::update_rule_trigger(Path::new(&rule_path), &update).map_err(Into::into)
}

/// Attaches an external file or directory to a project as a Support Doc pointer or symlink.
///
/// # Errors
/// Returns a stringified [`AppError`] on validation or write failure.
#[tauri::command]
pub fn link_support_doc(request: LinkSupportDocRequest) -> Result<(), String> {
    let cfg = config::load_config().map_err(String::from)?;
    support_docs::link_support_doc(&cfg, &request).map_err(Into::into)
}

/// Removes an attached Support Doc pointer entry or symlink from a project.
///
/// # Errors
/// Returns a stringified [`AppError`] on unlink or write failure.
#[tauri::command]
pub fn unlink_support_doc(
    project_path: String,
    doc_path: String,
    mode: SupportDocMode,
) -> Result<(), String> {
    let cfg = config::load_config().map_err(String::from)?;
    support_docs::unlink_support_doc(&cfg, Path::new(&project_path), &doc_path, mode)
        .map_err(Into::into)
}

/// Enables or disables an MCP server across enabled agents in Global or Project scope.
///
/// # Errors
/// Returns a stringified [`AppError`] if the server is unknown or config mutation fails.
#[tauri::command]
pub fn toggle_mcp_server(
    scope: ToggleScope,
    server_name: String,
    enable: bool,
) -> Result<(), String> {
    let cfg = config::load_config().map_err(String::from)?;
    let home = config::home_dir().map_err(String::from)?;
    let desired = if enable {
        DesiredState::Enable
    } else {
        DesiredState::Disable
    };
    mcp_writer::toggle_mcp_server(&cfg, &home, &scope, &server_name, desired).map_err(Into::into)
}

/// Adds or updates an MCP server in the central catalog and optionally enables it in a scope.
///
/// # Errors
/// Returns a stringified [`AppError`] on validation, catalog save, or config write failure.
#[tauri::command]
pub fn add_mcp_server(
    draft: McpServerDraft,
    enable_in_scope: Option<ToggleScope>,
) -> Result<McpCatalogEntry, String> {
    let entry = mcp_catalog::upsert_server(draft).map_err(String::from)?;
    if let Some(scope) = enable_in_scope {
        let cfg = config::load_config().map_err(String::from)?;
        let home = config::home_dir().map_err(String::from)?;
        mcp_writer::toggle_mcp_server(&cfg, &home, &scope, &entry.name, DesiredState::Enable)
            .map_err(String::from)?;
    }
    Ok(entry)
}

/// Probes a stdio MCP server via JSON-RPC `initialize` + `tools/list` (5s timeout) and caches tokens.
///
/// # Errors
/// Returns a stringified [`AppError`] if spawning, probing, or caching fails.
#[tauri::command]
pub async fn probe_mcp_server(server_name: String) -> Result<McpProbeResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        mcp_probe::probe_and_cache(&server_name).map_err(String::from)
    })
    .await
    .map_err(|e| format!("Probe task panicked: {e}"))?
}

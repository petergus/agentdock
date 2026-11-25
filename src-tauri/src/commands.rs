//! Tauri IPC command handlers exposed to the frontend switchboard.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use tauri::{AppHandle, State};

use crate::cleanup;
use crate::config;
use crate::debloat;
use crate::error::AppError;
use crate::global_switch;
use crate::mcp_catalog;
use crate::mcp_probe;
use crate::mcp_writer;
use crate::models::{
    AfterMoveAction, AgentKind, AppConfig, BatchToggleResult, CleanupActionResult,
    CleanupAuditReport, ContextSet, DesiredState, GlobalSwitchTarget, LinkSupportDocRequest,
    McpCatalogEntry, McpProbeResult, McpServerDraft, MoveExecutionResult, MovePreviewItem,
    RuleTriggerUpdate, ScopeSnapshot, SetToggleResult, SupportDocMode, TagDefinition,
    ToggleOutcome, ToggleScope, WorkspaceOverview,
};
use crate::plugins;
use crate::rules_writer;
use crate::scanner;
use crate::sets;
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
            mgr.refresh_keeping_project(&app, &saved, &home);
        }
    }
    Ok(saved)
}

/// Pins or unpins a project folder in `config.json` without resetting the active watcher.
///
/// # Errors
/// Returns a stringified [`AppError`] if loading, validating, or saving `config.json` fails.
#[tauri::command]
pub fn toggle_pinned_project(project_path: String) -> Result<AppConfig, String> {
    let mut cfg = config::load_config().map_err(String::from)?;
    let home = config::home_dir().map_err(String::from)?;
    config::toggle_pinned_project_in_config(&mut cfg, &home, &project_path)
        .map_err(String::from)?;
    config::save_config(&cfg).map_err(Into::into)
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
    let mut cfg = config::load_config().map_err(String::from)?;
    let home = config::home_dir().map_err(String::from)?;
    let desired = if enable {
        DesiredState::Enable
    } else {
        DesiredState::Disable
    };
    let outcome = symlinks::toggle_skill(
        &cfg,
        &home,
        &scope,
        &skill_id,
        Path::new(&canonical_skill_dir),
        desired,
    )
    .map_err(String::from)?;
    if matches!(scope, ToggleScope::Global)
        && enable
        && matches!(outcome, ToggleOutcome::Updated { .. })
    {
        global_switch::clear_paused_skill(&mut cfg, &skill_id);
        let _ = config::save_config(&cfg);
    }
    Ok(outcome)
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
    let mut cfg = config::load_config().map_err(String::from)?;
    let home = config::home_dir().map_err(String::from)?;
    let desired = if enable {
        DesiredState::Enable
    } else {
        DesiredState::Disable
    };
    let outcome = symlinks::toggle_library_rule(
        &cfg,
        &home,
        &scope,
        &rule_filename,
        Path::new(&canonical_rule_file),
        desired,
    )
    .map_err(String::from)?;
    if matches!(scope, ToggleScope::Global)
        && enable
        && matches!(outcome, ToggleOutcome::Updated { .. })
    {
        global_switch::clear_paused_rule(&mut cfg, &rule_filename);
        let _ = config::save_config(&cfg);
    }
    Ok(outcome)
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
    let lib_path = config::expand_user_path(&home, &target_library_path);
    debloat::preview_move_skills_to_library(&cfg, &home, &skill_ids, &lib_path, &scope)
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
    let lib_path = config::expand_user_path(&home, &target_library_path);
    debloat::execute_move_skills_to_library(&cfg, &home, &skill_ids, &lib_path, &scope, after_move)
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
pub fn link_support_doc(mut request: LinkSupportDocRequest) -> Result<(), String> {
    let cfg = config::load_config().map_err(String::from)?;
    if let Ok(home) = config::home_dir() {
        request.source_path = config::expand_user_path(&home, &request.source_path)
            .to_string_lossy()
            .into_owned();
    }
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
    let mut cfg = config::load_config().map_err(String::from)?;
    let home = config::home_dir().map_err(String::from)?;
    let desired = if enable {
        DesiredState::Enable
    } else {
        DesiredState::Disable
    };
    mcp_writer::toggle_mcp_server(&cfg, &home, &scope, &server_name, desired)
        .map_err(String::from)?;
    if matches!(scope, ToggleScope::Global) && enable {
        global_switch::clear_paused_mcp(&mut cfg, &server_name);
        let _ = config::save_config(&cfg);
    }
    Ok(())
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

/// Pauses or restores Global skills, plugins, MCP servers, and/or library rules.
///
/// # Errors
/// Returns a stringified [`AppError`] on config or filesystem failure.
#[tauri::command]
pub async fn set_global_switch(
    target: GlobalSwitchTarget,
    enable: bool,
) -> Result<BatchToggleResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut cfg = config::load_config().map_err(String::from)?;
        let home = config::home_dir().map_err(String::from)?;
        let result = global_switch::set_global_switch(&mut cfg, &home, target, enable, None, None)
            .map_err(String::from)?;
        config::save_config(&cfg).map_err(String::from)?;
        Ok(result)
    })
    .await
    .map_err(|e| format!("Global switch task panicked: {e}"))?
}

/// Enables or disables an installed Antigravity, Codex, or Claude plugin.
///
/// # Errors
/// Returns a stringified [`AppError`] if the plugin does not exist or toggling fails.
#[tauri::command]
pub async fn toggle_plugin(
    agent: AgentKind,
    plugin_id: String,
    enable: bool,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut cfg = config::load_config().map_err(String::from)?;
        let home = config::home_dir().map_err(String::from)?;
        let desired = if enable {
            DesiredState::Enable
        } else {
            DesiredState::Disable
        };
        plugins::toggle_plugin(&cfg, &home, agent, &plugin_id, desired, None)
            .map_err(String::from)?;
        if enable {
            global_switch::clear_paused_plugin(&mut cfg, agent, &plugin_id);
            let _ = config::save_config(&cfg);
        }
        Ok(())
    })
    .await
    .map_err(|e| format!("Plugin toggle task panicked: {e}"))?
}

/// Inspects Set changes without modifying project files or the MCP catalog.
#[tauri::command]
pub async fn preview_project_set(
    project_path: String,
    set_id: String,
    operation: sets::SetOperation,
) -> Result<sets::SetPreview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cfg = config::load_config()?;
        let home = config::home_dir()?;
        let catalog = config::mcp_catalog_path()?;
        sets::preview_project_set(&cfg, &home, Path::new(&project_path), &set_id, operation, &catalog)
    }).await.map_err(|err| err.to_string())?.map_err(String::from)
}

/// Applies, disables, or repairs a reusable Context Set for a project.
///
/// # Errors
/// Returns a stringified [`AppError`] if the set/project is invalid or config save fails.
#[tauri::command]
pub fn toggle_project_set(
    project_path: String,
    set_id: String,
    operation: sets::SetOperation,
    revision: String,
) -> Result<SetToggleResult, String> {
    let mut cfg = config::load_config().map_err(String::from)?;
    let home = config::home_dir().map_err(String::from)?;
    let result = sets::toggle_project_set(
        &mut cfg,
        &home,
        Path::new(&project_path),
        &set_id,
        operation,
        None,
        None,
        Some(&revision),
    )
    .map_err(String::from)?;
    if operation != sets::SetOperation::Repair {
        config::save_config(&cfg).map_err(String::from)?;
    }
    Ok(result)
}

/// Inserts or updates a Context Set definition in `config.json`.
///
/// # Errors
/// Returns a stringified [`AppError`] if validation or saving fails.
#[tauri::command]
pub fn upsert_set(set: ContextSet) -> Result<AppConfig, String> {
    let mut cfg = config::load_config().map_err(String::from)?;
    config::upsert_set_in_config(&mut cfg, set).map_err(String::from)?;
    config::save_config(&cfg).map_err(Into::into)
}

/// Deletes a Context Set definition from `config.json`.
///
/// # Errors
/// Returns a stringified [`AppError`] if loading or saving fails.
#[tauri::command]
pub fn delete_set(set_id: String) -> Result<AppConfig, String> {
    let mut cfg = config::load_config().map_err(String::from)?;
    config::delete_set_in_config(&mut cfg, &set_id);
    config::save_config(&cfg).map_err(Into::into)
}

/// Hides or unhides a project folder in `config.json`.
///
/// # Errors
/// Returns a stringified [`AppError`] if loading or saving fails.
#[tauri::command]
pub fn toggle_hidden_project(project_path: String) -> Result<AppConfig, String> {
    let mut cfg = config::load_config().map_err(String::from)?;
    let home = config::home_dir().map_err(String::from)?;
    config::toggle_hidden_project_in_config(&mut cfg, &home, &project_path)
        .map_err(String::from)?;
    config::save_config(&cfg).map_err(Into::into)
}

/// Sets manual tags and muted auto-tags for a project.
///
/// # Errors
/// Returns a stringified [`AppError`] if validation or saving fails.
#[tauri::command]
pub fn set_project_tags(
    project_path: String,
    manual_tag_ids: Vec<String>,
    muted_auto_tag_ids: Vec<String>,
) -> Result<AppConfig, String> {
    let mut cfg = config::load_config().map_err(String::from)?;
    let home = config::home_dir().map_err(String::from)?;
    config::set_project_tags_in_config(
        &mut cfg,
        &home,
        &project_path,
        manual_tag_ids,
        muted_auto_tag_ids,
    )
    .map_err(String::from)?;
    config::save_config(&cfg).map_err(Into::into)
}

/// Creates or updates a tag definition (including ancestor auto-tag rules).
///
/// # Errors
/// Returns a stringified [`AppError`] if validation or saving fails.
#[tauri::command]
pub fn upsert_tag_definition(tag: TagDefinition) -> Result<AppConfig, String> {
    let mut cfg = config::load_config().map_err(String::from)?;
    config::upsert_tag_definition_in_config(&mut cfg, tag).map_err(String::from)?;
    config::save_config(&cfg).map_err(Into::into)
}

/// Deletes a tag definition and cleans up project tag references.
///
/// # Errors
/// Returns a stringified [`AppError`] if loading or saving fails.
#[tauri::command]
pub fn delete_tag_definition(tag_id: String) -> Result<AppConfig, String> {
    let mut cfg = config::load_config().map_err(String::from)?;
    config::delete_tag_definition_in_config(&mut cfg, &tag_id);
    config::save_config(&cfg).map_err(Into::into)
}

/// Runs a full read-only workspace health and cleanup audit.
///
/// # Errors
/// Returns a stringified [`AppError`] if loading config or resolving home fails.
#[tauri::command]
pub async fn run_cleanup_audit() -> Result<CleanupAuditReport, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let cfg = config::load_config().map_err(String::from)?;
        cleanup::audit_workspace(&cfg).map_err(Into::into)
    })
    .await
    .map_err(|e| format!("Cleanup audit task panicked: {e}"))?
}

/// Unlinks a batch of verified broken symlinks across Global and Project scopes.
///
/// # Errors
/// Returns a stringified [`AppError`] if loading config fails.
#[tauri::command]
pub async fn unlink_broken_symlinks_batch(
    items: Vec<(String, Option<String>)>,
) -> Result<CleanupActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cfg = config::load_config().map_err(String::from)?;
        cleanup::unlink_broken_symlinks_batch(&cfg, &items).map_err(Into::into)
    })
    .await
    .map_err(|e| format!("Broken symlink cleanup task panicked: {e}"))?
}

/// Safely removes stale Codex plugin cache directories.
///
/// # Errors
/// Returns a stringified [`AppError`] if config or home directory cannot be resolved.
#[tauri::command]
pub async fn prune_codex_plugin_caches(paths: Vec<String>) -> Result<CleanupActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cfg = config::load_config().map_err(String::from)?;
        let home = config::home_dir().map_err(String::from)?;
        cleanup::prune_codex_plugin_caches(&cfg, &home, &paths).map_err(Into::into)
    })
    .await
    .map_err(|e| format!("Plugin cache prune task panicked: {e}"))?
}

/// Removes stale AgentDock-managed `.git/info/exclude` entries across projects.
///
/// # Errors
/// Returns a stringified [`AppError`] if config or home directory cannot be resolved.
#[tauri::command]
pub async fn prune_stale_git_excludes(
    project_paths: Vec<String>,
) -> Result<CleanupActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cfg = config::load_config().map_err(String::from)?;
        let home = config::home_dir().map_err(String::from)?;
        cleanup::prune_stale_git_excludes(&cfg, &home, &project_paths, None).map_err(Into::into)
    })
    .await
    .map_err(|e| format!("Git exclude prune task panicked: {e}"))?
}

/// Safely removes a synced Claude skill bucket directory inside `~/.claude/skills/synced/`.
///
/// # Errors
/// Returns a stringified [`AppError`] if validation or removal fails.
#[tauri::command]
pub async fn remove_claude_synced_bucket(path: String) -> Result<CleanupActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cfg = config::load_config().map_err(String::from)?;
        let home = config::home_dir().map_err(String::from)?;
        cleanup::remove_claude_synced_bucket(&cfg, &home, &path).map_err(Into::into)
    })
    .await
    .map_err(|e| format!("Claude bucket cleanup task panicked: {e}"))?
}

/// Generates a suggested fix/template preview for a flagged global instruction or rule file.
///
/// # Errors
/// Returns a stringified [`AppError`] if config, home, or the target file cannot be read.
#[tauri::command]
pub async fn preview_instruction_fix(
    path: String,
    kind: crate::models::InstructionHygieneKind,
) -> Result<crate::models::InstructionFixPreview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cfg = config::load_config().map_err(String::from)?;
        let home = config::home_dir().map_err(String::from)?;
        cleanup::preview_instruction_fix(&cfg, &home, &path, kind).map_err(Into::into)
    })
    .await
    .map_err(|e| format!("Instruction fix preview task panicked: {e}"))?
}

/// Backs up and atomically writes an approved global instruction or rule fix.
///
/// # Errors
/// Returns a stringified [`AppError`] if the file cannot be backed up or written.
#[tauri::command]
pub async fn apply_instruction_fix(
    path: String,
    content: String,
) -> Result<crate::models::InstructionFixApplyResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        cleanup::apply_instruction_fix(&path, &content, None).map_err(Into::into)
    })
    .await
    .map_err(|e| format!("Instruction fix write task panicked: {e}"))?
}

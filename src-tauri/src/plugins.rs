//! Multi-agent plugin discovery, upfront token calculation, and live enable/disable toggling.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Map, Value as JsonValue};
use toml_edit::{DocumentMut, Item, Table};

use crate::backup;
use crate::config::{self, AgentRoots};
use crate::error::AppError;
use crate::frontmatter::parse_markdown_frontmatter;
use crate::models::{AgentKind, AppConfig, DesiredState, PluginItem, PluginSkillInfo};
use crate::symlinks;
use crate::tokenizer;

const ANTIGRAVITY_MANIFEST_CANDIDATES: &[&str] = &[
    "plugin.json",
    ".antigravity-plugin/plugin.json",
    ".gemini-plugin/plugin.json",
    ".claude-plugin/plugin.json",
];

const CODEX_MANIFEST_CANDIDATES: &[&str] = &[".codex-plugin/plugin.json", "plugin.json"];

const CLAUDE_MANIFEST_CANDIDATES: &[&str] = &[".claude-plugin/plugin.json", "plugin.json"];

#[derive(Debug, Default)]
struct PluginConfigState {
    enabled: Option<bool>,
    marketplace: Option<String>,
    version: Option<String>,
}

/// Scans global plugins and customizations for all enabled agents (Antigravity, Codex, and Claude).
#[must_use]
pub fn scan_global_plugins(
    cfg: &AppConfig,
    roots: &AgentRoots,
    warnings: &mut Vec<String>,
) -> Vec<PluginItem> {
    let mut plugins = Vec::new();
    if cfg.active_agents.antigravity {
        plugins.extend(scan_antigravity_plugins(roots, warnings));
    }
    if cfg.active_agents.codex {
        plugins.extend(scan_codex_plugins(roots, warnings));
    }
    if cfg.active_agents.claude {
        plugins.extend(scan_claude_plugins(roots, warnings));
    }

    plugins.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.agent.cmp(&b.agent))
            .then_with(|| a.id.cmp(&b.id))
    });
    plugins
}

fn scan_antigravity_plugins(roots: &AgentRoots, warnings: &mut Vec<String>) -> Vec<PluginItem> {
    let config_dir = roots.antigravity_dir.join("config");
    let plugins_dir = config_dir.join("plugins");
    if !plugins_dir.is_dir() {
        return Vec::new();
    }

    let config_json_path = config_dir.join("config.json");
    let config_states = read_plugin_config_states(&config_json_path, warnings);

    let Ok(entries) = fs::read_dir(&plugins_dir) else {
        warnings.push(format!(
            "Unable to read Antigravity plugins directory: {}",
            plugins_dir.display()
        ));
        return Vec::new();
    };

    let mut plugins = Vec::new();
    for entry in entries.flatten() {
        let plugin_dir = entry.path();
        if !plugin_dir.is_dir() {
            continue;
        }
        let Some(id) = entry.file_name().to_str().map(ToOwned::to_owned) else {
            continue;
        };
        if id.starts_with('.') {
            continue;
        }

        let manifest = read_plugin_manifest(&plugin_dir, ANTIGRAVITY_MANIFEST_CANDIDATES);
        let has_skills_dir = plugin_dir.join("skills").is_dir();
        if manifest.is_none() && !has_skills_dir {
            continue;
        }

        let manifest_obj = manifest.as_ref().and_then(JsonValue::as_object);
        let cfg_state = config_states.get(&id);

        let manifest_disabled = manifest_obj
            .and_then(|obj| obj.get("disabled"))
            .and_then(JsonValue::as_bool)
            .unwrap_or(false);
        let enabled = cfg_state
            .and_then(|s| s.enabled)
            .unwrap_or(!manifest_disabled);

        let name = manifest_obj
            .and_then(|obj| {
                non_empty_json_str(obj, "displayName").or_else(|| non_empty_json_str(obj, "name"))
            })
            .unwrap_or_else(|| id.clone());
        let description = manifest_obj.and_then(|obj| non_empty_json_str(obj, "description"));
        let version = manifest_obj
            .and_then(|obj| non_empty_json_str(obj, "version"))
            .or_else(|| cfg_state.and_then(|s| s.version.clone()));
        let source = Some(format_antigravity_source(
            cfg_state.and_then(|s| s.marketplace.as_deref()),
        ));

        let (skills, skills_upfront_tokens) = scan_plugin_skills(&plugin_dir, 1);
        let (agent_names, agents_upfront_tokens) = scan_plugin_agents(&plugin_dir);
        let (rule_names, rules_upfront_tokens) = scan_plugin_rules(&plugin_dir);
        let mcp_names = scan_plugin_mcp(&plugin_dir, &["mcp_config.json"]);
        let upfront_tokens = skills_upfront_tokens + agents_upfront_tokens;

        plugins.push(PluginItem {
            agent: AgentKind::Antigravity,
            id,
            name,
            description,
            version,
            source,
            path: plugin_dir.to_string_lossy().into_owned(),
            config_path: Some(config_json_path.to_string_lossy().into_owned()),
            manage_hint: Some("Antigravity Plugins Manager or `agy plugin`".to_owned()),
            enabled,
            can_toggle: true,
            skills,
            agent_names,
            mcp_names,
            rule_names,
            skills_upfront_tokens,
            agents_upfront_tokens,
            rules_upfront_tokens,
            upfront_tokens,
        });
    }

    plugins
}

fn scan_codex_plugins(roots: &AgentRoots, warnings: &mut Vec<String>) -> Vec<PluginItem> {
    let config_toml_path = roots.codex_dir.join("config.toml");
    if !config_toml_path.is_file() {
        return Vec::new();
    }

    let raw = match fs::read_to_string(&config_toml_path) {
        Ok(content) => content,
        Err(err) => {
            warnings.push(format!(
                "Unable to read Codex config.toml ({}): {err}",
                config_toml_path.display()
            ));
            return Vec::new();
        }
    };

    let doc = match raw.parse::<DocumentMut>() {
        Ok(doc) => doc,
        Err(err) => {
            warnings.push(format!(
                "Invalid TOML in Codex config.toml ({}): {err}",
                config_toml_path.display()
            ));
            return Vec::new();
        }
    };

    let Some(plugins_table) = doc.get("plugins").and_then(|item| item.as_table_like()) else {
        return Vec::new();
    };

    let cache_root = roots.codex_dir.join("plugins/cache");
    let mut plugins = Vec::new();

    for (plugin_id, item) in plugins_table.iter() {
        let Some(entry_table) = item.as_table_like() else {
            continue;
        };
        let enabled = entry_table
            .get("enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let (plugin_slug, marketplace) = match plugin_id.rsplit_once('@') {
            Some((slug, market)) => (slug, market),
            None => (plugin_id, "local"),
        };

        let resolved_dir = resolve_codex_cached_plugin_dir(&cache_root, marketplace, plugin_slug);
        let source = Some(format_codex_source(marketplace));

        if let Some((plugin_dir, version_folder)) = resolved_dir {
            let manifest = read_plugin_manifest(&plugin_dir, CODEX_MANIFEST_CANDIDATES);
            let manifest_obj = manifest.as_ref().and_then(JsonValue::as_object);
            let interface_obj = manifest_obj
                .and_then(|obj| obj.get("interface"))
                .and_then(JsonValue::as_object);

            let name = interface_obj
                .and_then(|iface| non_empty_json_str(iface, "displayName"))
                .or_else(|| manifest_obj.and_then(|obj| non_empty_json_str(obj, "displayName")))
                .or_else(|| manifest_obj.and_then(|obj| non_empty_json_str(obj, "name")))
                .unwrap_or_else(|| plugin_slug.to_owned());
            let description = manifest_obj
                .and_then(|obj| non_empty_json_str(obj, "description"))
                .or_else(|| {
                    interface_obj.and_then(|iface| non_empty_json_str(iface, "shortDescription"))
                });
            let version = manifest_obj
                .and_then(|obj| non_empty_json_str(obj, "version"))
                .or(Some(version_folder));

            // Codex's prompt-input loader scans plugin skills up to depth 2 (including `upstream/SKILL.md`).
            let (skills, skills_upfront_tokens) = scan_plugin_skills(&plugin_dir, 2);
            let (rule_names, rules_upfront_tokens) = scan_plugin_rules(&plugin_dir);
            let mcp_names = scan_plugin_mcp(&plugin_dir, &[".mcp.json", "mcp_config.json"]);

            plugins.push(PluginItem {
                agent: AgentKind::Codex,
                id: plugin_id.to_owned(),
                name,
                description,
                version,
                source,
                path: plugin_dir.to_string_lossy().into_owned(),
                config_path: Some(config_toml_path.to_string_lossy().into_owned()),
                manage_hint: Some("Codex App Plugins or `[plugins]` in config.toml".to_owned()),
                enabled,
                can_toggle: true,
                skills,
                agent_names: Vec::new(),
                mcp_names,
                rule_names,
                skills_upfront_tokens,
                agents_upfront_tokens: 0,
                rules_upfront_tokens,
                upfront_tokens: skills_upfront_tokens,
            });
        } else {
            plugins.push(PluginItem {
                agent: AgentKind::Codex,
                id: plugin_id.to_owned(),
                name: plugin_slug.to_owned(),
                description: None,
                version: None,
                source,
                path: config_toml_path.to_string_lossy().into_owned(),
                config_path: Some(config_toml_path.to_string_lossy().into_owned()),
                manage_hint: Some("Codex App Plugins or `[plugins]` in config.toml".to_owned()),
                enabled,
                can_toggle: true,
                skills: Vec::new(),
                agent_names: Vec::new(),
                mcp_names: Vec::new(),
                rule_names: Vec::new(),
                skills_upfront_tokens: 0,
                agents_upfront_tokens: 0,
                rules_upfront_tokens: 0,
                upfront_tokens: 0,
            });
        }
    }

    plugins
}

pub(crate) fn resolve_codex_cached_plugin_dir(
    cache_root: &Path,
    marketplace: &str,
    plugin_slug: &str,
) -> Option<(PathBuf, String)> {
    let mut candidate_roots = vec![cache_root.join(marketplace).join(plugin_slug)];
    if marketplace == "openai-curated" {
        candidate_roots.push(cache_root.join("openai-curated-remote").join(plugin_slug));
    }

    for base in candidate_roots {
        if !base.is_dir() {
            continue;
        }
        let latest = base.join("latest");
        if latest.is_dir() {
            return Some((latest, "latest".to_owned()));
        }

        let Ok(read_dir) = fs::read_dir(&base) else {
            continue;
        };
        let mut versions: Vec<(String, PathBuf)> = read_dir
            .flatten()
            .filter_map(|entry| {
                let p = entry.path();
                let name = entry.file_name().to_str()?.to_owned();
                if !p.is_dir() || name.starts_with('.') {
                    return None;
                }
                Some((name, p))
            })
            .collect();
        versions.sort_by(|a, b| a.0.cmp(&b.0));
        if let Some((ver, dir)) = versions.pop() {
            return Some((dir, ver));
        }
    }

    None
}

/// Returns the canonical paths of Codex plugin cache directories currently referenced in `config.toml`.
#[must_use]
pub(crate) fn active_codex_plugin_cache_paths(roots: &AgentRoots) -> BTreeSet<PathBuf> {
    let mut active = BTreeSet::new();
    let config_toml_path = roots.codex_dir.join("config.toml");
    let Ok(raw) = fs::read_to_string(&config_toml_path) else {
        return active;
    };
    let Ok(doc) = raw.parse::<DocumentMut>() else {
        return active;
    };
    let Some(plugins_table) = doc.get("plugins").and_then(|item| item.as_table_like()) else {
        return active;
    };

    let cache_root = roots.codex_dir.join("plugins/cache");
    for (plugin_id, item) in plugins_table.iter() {
        if item.as_table_like().is_none() {
            continue;
        }
        let (plugin_slug, marketplace) = match plugin_id.rsplit_once('@') {
            Some((slug, market)) => (slug, market),
            None => (plugin_id, "local"),
        };
        if let Some((dir, _)) =
            resolve_codex_cached_plugin_dir(&cache_root, marketplace, plugin_slug)
        {
            let canon = fs::canonicalize(&dir).unwrap_or(dir);
            active.insert(canon);
        }
    }
    active
}

/// Enables or disables an installed Antigravity, Codex, or Claude plugin.
///
/// - **Antigravity:** Creates a rolling backup of `<antigravity_dir>/config/config.json` if
///   present, invokes `agy plugin enable|disable <id>` when targeting the default `~/.gemini`
///   directory and an `agy` CLI binary is available, or otherwise updates
///   `<antigravity_dir>/config/config.json` atomically with `0o600` mode.
/// - **Codex:** Updates `[plugins."<id>"].enabled` in `<codex_dir>/config.toml` via `toml_edit`
///   with rolling backup, atomic write, and preserved comments and file permissions.
/// - **Claude:** Updates `enabledPlugins["<id>"]` in `<claude_dir>/settings.json` (and
///   `<claude_dir>/settings.local.json` when an override key already exists there) with rolling
///   backup, atomic write, and preserved sibling keys and file permissions.
///
/// # Errors
/// Returns [`AppError`] if `plugin_id` is invalid or non-toggleable, the target plugin/config
/// cannot be updated, or `agy` exits with an error.
pub fn toggle_plugin(
    cfg: &AppConfig,
    home: &Path,
    agent: AgentKind,
    plugin_id: &str,
    desired: DesiredState,
    custom_backups_root: Option<&Path>,
) -> Result<(), AppError> {
    match agent {
        AgentKind::Antigravity => {
            toggle_antigravity_plugin(cfg, home, plugin_id, desired, custom_backups_root)
        }
        AgentKind::Codex => toggle_codex_plugin(cfg, home, plugin_id, desired, custom_backups_root),
        AgentKind::Claude => {
            toggle_claude_plugin(cfg, home, plugin_id, desired, custom_backups_root)
        }
    }
}

fn toggle_antigravity_plugin(
    cfg: &AppConfig,
    home: &Path,
    plugin_id: &str,
    desired: DesiredState,
    custom_backups_root: Option<&Path>,
) -> Result<(), AppError> {
    symlinks::validate_entry_name(plugin_id)?;

    let roots = config::resolve_agent_roots(cfg, home);
    let config_dir = roots.antigravity_dir.join("config");
    let plugin_dir = config_dir.join("plugins").join(plugin_id);
    if !plugin_dir.is_dir() {
        return Err(AppError::InvalidPath(format!(
            "Antigravity plugin '{plugin_id}' does not exist at {}",
            plugin_dir.display()
        )));
    }

    let config_json_path = config_dir.join("config.json");
    let enable = matches!(desired, DesiredState::Enable);
    let is_real_default_home = cfg.agent_paths.antigravity_dir.is_none()
        && custom_backups_root.is_none()
        && dirs::home_dir().as_deref() == Some(home);

    if is_real_default_home {
        if let Some(agy_bin) = find_agy_binary(home) {
            backup_if_exists(&config_json_path, custom_backups_root)?;
            let subcmd = if enable { "enable" } else { "disable" };
            let output = Command::new(&agy_bin)
                .args(["plugin", subcmd, plugin_id])
                .output()
                .map_err(|e| {
                    AppError::Command(format!("Failed to run {}: {e}", agy_bin.display()))
                })?;
            if output.status.success() {
                return Ok(());
            }
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            let detail = if !stderr.is_empty() { stderr } else { stdout };
            return Err(AppError::Command(format!(
                "agy plugin {subcmd} '{plugin_id}' failed: {detail}"
            )));
        }
    }

    update_plugin_enabled_in_config_json(&config_json_path, plugin_id, enable, custom_backups_root)
}

fn toggle_codex_plugin(
    cfg: &AppConfig,
    home: &Path,
    plugin_id: &str,
    desired: DesiredState,
    custom_backups_root: Option<&Path>,
) -> Result<(), AppError> {
    // Allow `@` in Codex plugin IDs (`name@marketplace`), while blocking path traversal.
    symlinks::validate_entry_name(plugin_id)?;

    let roots = config::resolve_agent_roots(cfg, home);
    let config_toml_path = roots.codex_dir.join("config.toml");

    let mut doc = if config_toml_path.is_file() {
        let raw = fs::read_to_string(&config_toml_path)
            .map_err(|e| AppError::io(&config_toml_path, e))?;
        raw.parse::<DocumentMut>().map_err(|e| {
            AppError::InvalidPath(format!(
                "Invalid TOML in {}: {e}",
                config_toml_path.display()
            ))
        })?
    } else {
        DocumentMut::new()
    };

    if !doc
        .get("plugins")
        .is_some_and(|item| item.as_table_like().is_some())
    {
        let mut tbl = Table::new();
        tbl.set_implicit(true);
        doc["plugins"] = Item::Table(tbl);
    }

    let enable = matches!(desired, DesiredState::Enable);
    let plugins_table = doc
        .get_mut("plugins")
        .and_then(|item| item.as_table_mut())
        .ok_or_else(|| {
            AppError::InvalidPath(format!(
                "Expected [plugins] table in {}",
                config_toml_path.display()
            ))
        })?;

    if !plugins_table
        .get(plugin_id)
        .is_some_and(|item| item.as_table_like().is_some())
    {
        plugins_table.insert(plugin_id, Item::Table(Table::new()));
    }

    if let Some(entry_table) = plugins_table
        .get_mut(plugin_id)
        .and_then(|item| item.as_table_like_mut())
    {
        entry_table.insert("enabled", toml_edit::value(enable));
    }

    backup_if_exists(&config_toml_path, custom_backups_root)?;

    #[cfg(unix)]
    let existing_mode = existing_unix_mode(&config_toml_path);

    backup::atomic_write_string(&config_toml_path, &doc.to_string())?;

    #[cfg(unix)]
    restore_unix_mode(&config_toml_path, existing_mode)?;

    Ok(())
}

fn toggle_claude_plugin(
    cfg: &AppConfig,
    home: &Path,
    plugin_id: &str,
    desired: DesiredState,
    custom_backups_root: Option<&Path>,
) -> Result<(), AppError> {
    symlinks::validate_entry_name(plugin_id)?;
    if matches!(
        plugin_id,
        "claude-custom-agents" | "claude-custom-commands"
    ) {
        return Err(AppError::InvalidPath(
            "Custom subagents and slash commands are directory-based and cannot be toggled in settings.json"
                .to_owned(),
        ));
    }

    let roots = config::resolve_agent_roots(cfg, home);
    let enable = matches!(desired, DesiredState::Enable);
    let settings_path = roots.claude_dir.join("settings.json");
    let settings_local_path = roots.claude_dir.join("settings.local.json");

    update_claude_enabled_plugin_in_json(&settings_path, plugin_id, enable, custom_backups_root)?;

    if settings_local_path.is_file() {
        let local_root = read_json_object_or_empty(&settings_local_path, true)?;
        let has_override = local_root
            .get("enabledPlugins")
            .and_then(JsonValue::as_object)
            .is_some_and(|map| map.contains_key(plugin_id));
        if has_override {
            update_claude_enabled_plugin_in_json(
                &settings_local_path,
                plugin_id,
                enable,
                custom_backups_root,
            )?;
        }
    }

    Ok(())
}

fn find_agy_binary(home: &Path) -> Option<PathBuf> {
    let candidates = [
        home.join(".gemini/bin/agy"),
        home.join(".local/bin/agy"),
        PathBuf::from("/opt/homebrew/bin/agy"),
        PathBuf::from("/usr/local/bin/agy"),
    ];
    for candidate in candidates {
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let bin = dir.join("agy");
        if bin.is_file() {
            return Some(bin);
        }
    }
    None
}

fn read_json_object_or_empty(
    path: &Path,
    allow_jsonc: bool,
) -> Result<Map<String, JsonValue>, AppError> {
    if !path.is_file() {
        return Ok(Map::new());
    }
    let raw = fs::read_to_string(path).map_err(|e| AppError::io(path, e))?;
    let parsed: JsonValue = if allow_jsonc {
        serde_json::from_str(&strip_jsonc_comments_and_trailing_commas(&raw))?
    } else {
        serde_json::from_str(&raw)?
    };
    parsed.as_object().cloned().ok_or_else(|| {
        AppError::InvalidPath(format!("Expected JSON object in {}", path.display()))
    })
}

fn write_json_object_preserving_mode(
    path: &Path,
    root_obj: &Map<String, JsonValue>,
    custom_backups_root: Option<&Path>,
) -> Result<(), AppError> {
    backup_if_exists(path, custom_backups_root)?;

    #[cfg(unix)]
    let existing_mode = existing_unix_mode(path);

    let formatted = serde_json::to_string_pretty(&JsonValue::Object(root_obj.clone()))?;
    backup::atomic_write_string(path, &format!("{formatted}\n"))?;

    #[cfg(unix)]
    restore_unix_mode(path, existing_mode)?;

    Ok(())
}

fn update_plugin_enabled_in_config_json(
    config_json_path: &Path,
    plugin_id: &str,
    enable: bool,
    custom_backups_root: Option<&Path>,
) -> Result<(), AppError> {
    let mut root_obj = read_json_object_or_empty(config_json_path, false)?;

    let mut plugins_obj = root_obj
        .get("plugins")
        .and_then(JsonValue::as_object)
        .cloned()
        .unwrap_or_default();

    let mut entry_obj = plugins_obj
        .get(plugin_id)
        .and_then(JsonValue::as_object)
        .cloned()
        .unwrap_or_default();
    entry_obj.insert("enabled".to_owned(), JsonValue::Bool(enable));
    plugins_obj.insert(plugin_id.to_owned(), JsonValue::Object(entry_obj));
    root_obj.insert("plugins".to_owned(), JsonValue::Object(plugins_obj));

    write_json_object_preserving_mode(config_json_path, &root_obj, custom_backups_root)
}

fn update_claude_enabled_plugin_in_json(
    settings_path: &Path,
    plugin_id: &str,
    enable: bool,
    custom_backups_root: Option<&Path>,
) -> Result<(), AppError> {
    let mut root_obj = read_json_object_or_empty(settings_path, true)?;

    let mut enabled_plugins = root_obj
        .get("enabledPlugins")
        .and_then(JsonValue::as_object)
        .cloned()
        .unwrap_or_default();
    enabled_plugins.insert(plugin_id.to_owned(), JsonValue::Bool(enable));
    root_obj.insert(
        "enabledPlugins".to_owned(),
        JsonValue::Object(enabled_plugins),
    );

    write_json_object_preserving_mode(settings_path, &root_obj, custom_backups_root)
}

fn backup_if_exists(path: &Path, custom_backups_root: Option<&Path>) -> Result<(), AppError> {
    if !path.is_file() {
        return Ok(());
    }
    match custom_backups_root {
        Some(dir) => {
            backup::backup_file_in_dir(path, dir)?;
        }
        None => {
            backup::backup_file(path)?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn existing_unix_mode(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path)
        .ok()
        .map_or(0o600, |m| m.permissions().mode() & 0o777)
}

#[cfg(unix)]
fn restore_unix_mode(path: &Path, mode: u32) -> Result<(), AppError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).map_err(|e| AppError::io(path, e))
}

fn read_plugin_config_states(
    config_json_path: &Path,
    warnings: &mut Vec<String>,
) -> BTreeMap<String, PluginConfigState> {
    let mut out = BTreeMap::new();
    if !config_json_path.is_file() {
        return out;
    }

    let raw = match fs::read_to_string(config_json_path) {
        Ok(content) => content,
        Err(err) => {
            warnings.push(format!(
                "Unable to read Antigravity config.json ({}): {err}",
                config_json_path.display()
            ));
            return out;
        }
    };

    let parsed: JsonValue = match serde_json::from_str(&raw) {
        Ok(val) => val,
        Err(err) => {
            warnings.push(format!(
                "Invalid JSON in Antigravity config.json ({}): {err}",
                config_json_path.display()
            ));
            return out;
        }
    };

    let Some(plugins_map) = parsed.get("plugins").and_then(JsonValue::as_object) else {
        return out;
    };

    for (dir_name, val) in plugins_map {
        let Some(obj) = val.as_object() else {
            continue;
        };
        let enabled = obj.get("enabled").and_then(JsonValue::as_bool);
        let installed_from = obj.get("installedFrom").and_then(JsonValue::as_object);
        let marketplace = installed_from.and_then(|m| non_empty_json_str(m, "marketplace"));
        let version = installed_from.and_then(|m| non_empty_json_str(m, "version"));
        out.insert(
            dir_name.clone(),
            PluginConfigState {
                enabled,
                marketplace,
                version,
            },
        );
    }

    out
}

fn read_plugin_manifest(plugin_dir: &Path, candidates: &[&str]) -> Option<JsonValue> {
    for rel in candidates {
        let path = plugin_dir.join(rel);
        if !path.is_file() {
            continue;
        }
        let Ok(raw) = fs::read_to_string(&path) else {
            continue;
        };
        if let Ok(val) = serde_json::from_str::<JsonValue>(&raw) {
            return Some(val);
        }
        let sanitized = strip_jsonc_comments_and_trailing_commas(&raw);
        if let Ok(val) = serde_json::from_str::<JsonValue>(&sanitized) {
            return Some(val);
        }
    }
    None
}

fn strip_jsonc_comments_and_trailing_commas(input: &str) -> String {
    let mut no_comments = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;

    while let Some(c) = chars.next() {
        if in_string {
            no_comments.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }

        if c == '"' {
            in_string = true;
            no_comments.push(c);
            continue;
        }

        if c == '/' {
            match chars.peek() {
                Some('/') => {
                    chars.next();
                    while let Some(&nc) = chars.peek() {
                        if nc == '\n' {
                            break;
                        }
                        chars.next();
                    }
                    continue;
                }
                Some('*') => {
                    chars.next();
                    while let Some(nc) = chars.next() {
                        if nc == '*' && chars.peek() == Some(&'/') {
                            chars.next();
                            break;
                        }
                    }
                    continue;
                }
                _ => {}
            }
        }

        no_comments.push(c);
    }

    // Remove trailing commas before `}` or `]` outside strings.
    let mut out = String::with_capacity(no_comments.len());
    let mut out_chars = no_comments.chars().peekable();
    in_string = false;
    escaped = false;

    while let Some(c) = out_chars.next() {
        if in_string {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }

        if c == '"' {
            in_string = true;
            out.push(c);
            continue;
        }

        if c == ',' {
            let mut lookahead = out_chars.clone();
            while let Some(&nc) = lookahead.peek() {
                if nc.is_whitespace() {
                    lookahead.next();
                } else {
                    break;
                }
            }
            if matches!(lookahead.peek(), Some('}' | ']')) {
                continue;
            }
        }

        out.push(c);
    }

    out
}

fn scan_plugin_skills(plugin_dir: &Path, max_depth: u8) -> (Vec<PluginSkillInfo>, usize) {
    let skills_dir = plugin_dir.join("skills");
    if !skills_dir.is_dir() {
        return (Vec::new(), 0);
    }

    let mut skills = Vec::new();
    let mut total_tokens = 0usize;
    collect_plugin_skills_recursive(&skills_dir, 0, max_depth, &mut skills, &mut total_tokens);
    skills.sort_by_key(|s| s.name.to_lowercase());
    (skills, total_tokens)
}

fn collect_plugin_skills_recursive(
    dir: &Path,
    depth: u8,
    max_depth: u8,
    skills: &mut Vec<PluginSkillInfo>,
    total_tokens: &mut usize,
) {
    if depth >= max_depth {
        return;
    }
    let Ok(read_dir) = fs::read_dir(dir) else {
        return;
    };

    for entry in read_dir.flatten() {
        let child = entry.path();
        if !child.is_dir() {
            continue;
        }
        let Some(slug) = entry.file_name().to_str().map(ToOwned::to_owned) else {
            continue;
        };
        if slug.starts_with('.') {
            continue;
        }

        let skill_md = child.join("SKILL.md");
        if skill_md.is_file() {
            if let Ok(content) = fs::read_to_string(&skill_md) {
                let parsed = parse_markdown_frontmatter(&content);
                let name = parsed.name.unwrap_or_else(|| slug.clone());
                let description = parsed.description.unwrap_or_default();
                let upfront_tokens = tokenizer::skill_upfront_tokens(&name, &description);
                *total_tokens += upfront_tokens;
                skills.push(PluginSkillInfo {
                    name,
                    upfront_tokens,
                });
            }
        }

        if depth + 1 < max_depth {
            collect_plugin_skills_recursive(&child, depth + 1, max_depth, skills, total_tokens);
        }
    }
}

fn scan_plugin_agents(plugin_dir: &Path) -> (Vec<String>, usize) {
    let agents_dir = plugin_dir.join("agents");
    if !agents_dir.is_dir() {
        return (Vec::new(), 0);
    }

    let Ok(read_dir) = fs::read_dir(&agents_dir) else {
        return (Vec::new(), 0);
    };

    let mut names = Vec::new();
    let mut total_tokens = 0usize;

    for entry in read_dir.flatten() {
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if stem.starts_with('.') {
            continue;
        }
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        let parsed = parse_markdown_frontmatter(&content);
        let name = parsed.name.unwrap_or_else(|| stem.to_owned());
        let description = parsed.description.unwrap_or_default();
        total_tokens += tokenizer::skill_upfront_tokens(&name, &description);
        names.push(name);
    }

    names.sort_by_key(|n| n.to_lowercase());
    (names, total_tokens)
}

fn scan_plugin_rules(plugin_dir: &Path) -> (Vec<String>, usize) {
    let rules_dir = plugin_dir.join("rules");
    if !rules_dir.is_dir() {
        return (Vec::new(), 0);
    }

    let Ok(read_dir) = fs::read_dir(&rules_dir) else {
        return (Vec::new(), 0);
    };

    let mut names = Vec::new();
    let mut total_tokens = 0usize;

    for entry in read_dir.flatten() {
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let Some(filename) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if filename.starts_with('.') {
            continue;
        }
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        total_tokens += tokenizer::count_tokens(&content);
        names.push(filename.to_owned());
    }

    names.sort_by_key(|n| n.to_lowercase());
    (names, total_tokens)
}

fn scan_plugin_mcp(plugin_dir: &Path, candidates: &[&str]) -> Vec<String> {
    for rel in candidates {
        let mcp_path = plugin_dir.join(rel);
        if !mcp_path.is_file() {
            continue;
        }
        let Ok(raw) = fs::read_to_string(&mcp_path) else {
            continue;
        };
        let Ok(json) = serde_json::from_str::<JsonValue>(&raw) else {
            continue;
        };
        let Some(servers) = json.get("mcpServers").and_then(JsonValue::as_object) else {
            continue;
        };

        let mut names: Vec<String> = servers
            .iter()
            .filter_map(|(name, val)| {
                let disabled = val
                    .as_object()
                    .and_then(|o| o.get("disabled"))
                    .and_then(JsonValue::as_bool)
                    .unwrap_or(false);
                (!disabled).then(|| name.clone())
            })
            .collect();
        names.sort_by_key(|n| n.to_lowercase());
        return names;
    }

    Vec::new()
}

fn non_empty_json_str(obj: &Map<String, JsonValue>, key: &str) -> Option<String> {
    obj.get(key)
        .and_then(JsonValue::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
}

fn format_antigravity_source(marketplace: Option<&str>) -> String {
    match marketplace {
        Some("antigravity-plugins-official") => "Official".to_owned(),
        Some("antigravity-plugins-partners") => "Partner".to_owned(),
        Some(other) => other.to_owned(),
        None => "Local".to_owned(),
    }
}

fn format_codex_source(marketplace: &str) -> String {
    match marketplace {
        "openai-bundled" => "Bundled".to_owned(),
        "openai-primary-runtime" => "OpenAI Runtime".to_owned(),
        "openai-curated" | "openai-curated-remote" => "OpenAI Curated".to_owned(),
        "plugins-cli" => "Plugins CLI".to_owned(),
        other => other.to_owned(),
    }
}

fn scan_claude_plugins(roots: &AgentRoots, warnings: &mut Vec<String>) -> Vec<PluginItem> {
    let claude_dir = &roots.claude_dir;
    if !claude_dir.is_dir() {
        return Vec::new();
    }

    let settings_path = claude_dir.join("settings.json");
    let settings_local_path = claude_dir.join("settings.local.json");
    let installed_path = claude_dir.join("plugins/installed_plugins.json");

    let mut enabled_plugins: BTreeMap<String, bool> = BTreeMap::new();
    for path in [&settings_path, &settings_local_path] {
        read_claude_enabled_plugins(path, &mut enabled_plugins, warnings);
    }

    let config_path_str = if settings_path.is_file() {
        settings_path.to_string_lossy().into_owned()
    } else if installed_path.is_file() {
        installed_path.to_string_lossy().into_owned()
    } else {
        settings_path.to_string_lossy().into_owned()
    };

    let mut plugins = Vec::new();
    let mut seen_ids = BTreeSet::new();

    if installed_path.is_file() {
        if let Ok(raw) = fs::read_to_string(&installed_path) {
            if let Ok(val) =
                serde_json::from_str::<JsonValue>(&strip_jsonc_comments_and_trailing_commas(&raw))
            {
                let entries_obj = val
                    .get("plugins")
                    .and_then(JsonValue::as_object)
                    .or_else(|| val.as_object());

                if let Some(entries) = entries_obj {
                    for (plugin_id, raw_entry) in entries {
                        let record = raw_entry
                            .as_array()
                            .and_then(|arr| arr.first())
                            .and_then(JsonValue::as_object)
                            .or_else(|| raw_entry.as_object());

                        let install_path = record
                            .and_then(|obj| non_empty_json_str(obj, "installPath"))
                            .map(PathBuf::from);
                        let record_version =
                            record.and_then(|obj| non_empty_json_str(obj, "version"));

                        let (plugin_slug, marketplace) = match plugin_id.rsplit_once('@') {
                            Some((slug, market)) => (slug, Some(market)),
                            None => (plugin_id.as_str(), None),
                        };

                        let enabled = enabled_plugins.get(plugin_id).copied().unwrap_or(false);
                        let source = marketplace.map(|m| m.to_owned());

                        if let Some(ref dir) = install_path.filter(|p| p.is_dir()) {
                            let manifest = read_plugin_manifest(dir, CLAUDE_MANIFEST_CANDIDATES);
                            let manifest_obj = manifest.as_ref().and_then(JsonValue::as_object);

                            let name = manifest_obj
                                .and_then(|obj| {
                                    non_empty_json_str(obj, "displayName")
                                        .or_else(|| non_empty_json_str(obj, "name"))
                                })
                                .unwrap_or_else(|| plugin_slug.to_owned());
                            let description =
                                manifest_obj.and_then(|obj| non_empty_json_str(obj, "description"));
                            let version = manifest_obj
                                .and_then(|obj| non_empty_json_str(obj, "version"))
                                .or(record_version);

                            let (skills, skills_upfront_tokens) = scan_plugin_skills(dir, 2);
                            let (agent_names, agents_upfront_tokens) = scan_plugin_agents(dir);
                            let (rule_names, rules_upfront_tokens) = scan_plugin_rules(dir);
                            let mcp_names = scan_plugin_mcp(dir, &[".mcp.json", "mcp_config.json"]);
                            let upfront_tokens = skills_upfront_tokens + agents_upfront_tokens;

                            plugins.push(PluginItem {
                                agent: AgentKind::Claude,
                                id: plugin_id.clone(),
                                name,
                                description,
                                version,
                                source,
                                path: dir.to_string_lossy().into_owned(),
                                config_path: Some(config_path_str.clone()),
                                manage_hint: Some(
                                    "`claude plugin` or `enabledPlugins` in settings.json"
                                        .to_owned(),
                                ),
                                enabled,
                                can_toggle: true,
                                skills,
                                agent_names,
                                mcp_names,
                                rule_names,
                                skills_upfront_tokens,
                                agents_upfront_tokens,
                                rules_upfront_tokens,
                                upfront_tokens,
                            });
                        } else {
                            plugins.push(PluginItem {
                                agent: AgentKind::Claude,
                                id: plugin_id.clone(),
                                name: plugin_slug.to_owned(),
                                description: None,
                                version: record_version,
                                source,
                                path: installed_path.to_string_lossy().into_owned(),
                                config_path: Some(config_path_str.clone()),
                                manage_hint: Some(
                                    "`claude plugin` or `enabledPlugins` in settings.json"
                                        .to_owned(),
                                ),
                                enabled,
                                can_toggle: true,
                                skills: Vec::new(),
                                agent_names: Vec::new(),
                                mcp_names: Vec::new(),
                                rule_names: Vec::new(),
                                skills_upfront_tokens: 0,
                                agents_upfront_tokens: 0,
                                rules_upfront_tokens: 0,
                                upfront_tokens: 0,
                            });
                        }
                        seen_ids.insert(plugin_id.clone());
                    }
                }
            } else {
                warnings.push(format!(
                    "Invalid JSON in Claude installed_plugins.json ({})",
                    installed_path.display()
                ));
            }
        }
    }

    // Include any enabledPlugins entries from settings.json not listed in installed_plugins.json.
    for (plugin_id, enabled) in &enabled_plugins {
        if seen_ids.contains(plugin_id) {
            continue;
        }
        let (plugin_slug, marketplace) = match plugin_id.rsplit_once('@') {
            Some((slug, market)) => (slug, Some(market.to_owned())),
            None => (plugin_id.as_str(), None),
        };
        plugins.push(PluginItem {
            agent: AgentKind::Claude,
            id: plugin_id.clone(),
            name: plugin_slug.to_owned(),
            description: None,
            version: None,
            source: marketplace,
            path: config_path_str.clone(),
            config_path: Some(config_path_str.clone()),
            manage_hint: Some("`claude plugin` or `enabledPlugins` in settings.json".to_owned()),
            enabled: *enabled,
            can_toggle: true,
            skills: Vec::new(),
            agent_names: Vec::new(),
            mcp_names: Vec::new(),
            rule_names: Vec::new(),
            skills_upfront_tokens: 0,
            agents_upfront_tokens: 0,
            rules_upfront_tokens: 0,
            upfront_tokens: 0,
        });
    }

    // Standalone Claude custom subagents (`~/.claude/agents/*.md`)
    let (custom_agents, custom_agents_tokens) = scan_plugin_agents(claude_dir);
    if !custom_agents.is_empty() {
        let agents_dir = claude_dir.join("agents");
        let agents_dir_str = agents_dir.to_string_lossy().into_owned();
        plugins.push(PluginItem {
            agent: AgentKind::Claude,
            id: "claude-custom-agents".to_owned(),
            name: "Custom Subagents".to_owned(),
            description: Some(
                "Markdown subagent definitions loaded from ~/.claude/agents/".to_owned(),
            ),
            version: None,
            source: Some("~/.claude/agents".to_owned()),
            path: agents_dir_str.clone(),
            config_path: Some(agents_dir_str),
            manage_hint: Some("Edit or remove `.md` files in ~/.claude/agents/".to_owned()),
            enabled: true,
            can_toggle: false,
            skills: Vec::new(),
            agent_names: custom_agents,
            mcp_names: Vec::new(),
            rule_names: Vec::new(),
            skills_upfront_tokens: 0,
            agents_upfront_tokens: custom_agents_tokens,
            rules_upfront_tokens: 0,
            upfront_tokens: custom_agents_tokens,
        });
    }

    // Standalone Claude slash commands (`~/.claude/commands/*.md`)
    let commands_dir = claude_dir.join("commands");
    let custom_commands = scan_claude_commands(&commands_dir);
    if !custom_commands.is_empty() {
        let commands_dir_str = commands_dir.to_string_lossy().into_owned();
        plugins.push(PluginItem {
            agent: AgentKind::Claude,
            id: "claude-custom-commands".to_owned(),
            name: "Slash Commands".to_owned(),
            description: Some(
                "On-demand slash commands loaded from ~/.claude/commands/".to_owned(),
            ),
            version: None,
            source: Some("~/.claude/commands".to_owned()),
            path: commands_dir_str.clone(),
            config_path: Some(commands_dir_str),
            manage_hint: Some("Edit or remove `.md` files in ~/.claude/commands/".to_owned()),
            enabled: true,
            can_toggle: false,
            skills: custom_commands,
            agent_names: Vec::new(),
            mcp_names: Vec::new(),
            rule_names: Vec::new(),
            skills_upfront_tokens: 0,
            agents_upfront_tokens: 0,
            rules_upfront_tokens: 0,
            upfront_tokens: 0,
        });
    }

    plugins
}

fn read_claude_enabled_plugins(
    path: &Path,
    out: &mut BTreeMap<String, bool>,
    warnings: &mut Vec<String>,
) {
    let Ok(raw) = fs::read_to_string(path) else {
        return;
    };
    let Ok(val) =
        serde_json::from_str::<JsonValue>(&strip_jsonc_comments_and_trailing_commas(&raw))
    else {
        warnings.push(format!(
            "Invalid JSON in Claude settings ({})",
            path.display()
        ));
        return;
    };
    if let Some(map) = val.get("enabledPlugins").and_then(JsonValue::as_object) {
        for (id, enabled_val) in map {
            if let Some(enabled) = enabled_val.as_bool() {
                out.insert(id.clone(), enabled);
            }
        }
    }
}

fn scan_claude_commands(commands_dir: &Path) -> Vec<PluginSkillInfo> {
    if !commands_dir.is_dir() {
        return Vec::new();
    }
    let Ok(read_dir) = fs::read_dir(commands_dir) else {
        return Vec::new();
    };
    let mut commands = Vec::new();
    for entry in read_dir.flatten() {
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if stem.starts_with('.') {
            continue;
        }
        commands.push(PluginSkillInfo {
            name: format!("/{stem}"),
            upfront_tokens: 0,
        });
    }
    commands.sort_by_key(|c| c.name.to_lowercase());
    commands
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ActiveAgents;
    use tempfile::TempDir;

    fn write_skill(dir: &Path, name: &str, desc: &str) {
        fs::create_dir_all(dir).expect("mkdir skill");
        fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: {desc}\n---\n\nBody\n"),
        )
        .expect("write SKILL.md");
    }

    #[test]
    fn scans_antigravity_plugins_ignores_nested_upstream_and_resolves_enabled_precedence() {
        let tmp = TempDir::new().expect("temp dir");
        let home = tmp.path();
        let config_dir = home.join(".gemini/config");
        let plugins_dir = config_dir.join("plugins");

        // 1. Enabled plugin (no explicit enabled key in config.json) with JSONC manifest, nested upstream skill, agent, rule, and MCP
        let vercel_dir = plugins_dir.join("vercel-plugin");
        fs::create_dir_all(&vercel_dir).expect("mkdir vercel");
        fs::write(
            vercel_dir.join("plugin.json"),
            "{\n  // JSONC comment\n  \"name\": \"vercel-plugin\",\n  \"displayName\": \"Vercel\",\n  \"description\": \"Deploy to Vercel\",\n  \"version\": \"0.56.0\",\n}\n",
        )
        .expect("write vercel manifest");
        write_skill(
            &vercel_dir.join("skills/ai-sdk"),
            "ai-sdk",
            "Build AI apps with the AI SDK.",
        );
        // Must be ignored for Antigravity (nested upstream/SKILL.md)
        write_skill(
            &vercel_dir.join("skills/ai-sdk/upstream"),
            "ai-sdk-upstream",
            "Duplicate upstream copy.",
        );
        fs::create_dir_all(vercel_dir.join("agents")).expect("mkdir agents");
        fs::write(
            vercel_dir.join("agents/ai-architect.md"),
            "---\nname: ai-architect\ndescription: Architects AI workflows.\n---\nPrompt\n",
        )
        .expect("write agent");
        fs::write(vercel_dir.join("agents/ai-architect.md.tmpl"), "template").expect("write tmpl");
        fs::create_dir_all(vercel_dir.join("rules")).expect("mkdir rules");
        fs::write(
            vercel_dir.join("rules/AGENTS.md"),
            "Always check Vercel config.",
        )
        .expect("write rule");
        fs::write(
            vercel_dir.join("mcp_config.json"),
            r#"{"mcpServers":{"vercel":{"serverUrl":"https://mcp.vercel.com"},"off":{"command":"echo","disabled":true}}}"#,
        )
        .expect("write mcp");

        // 2. Config-disabled plugin (manifest does not disable, config.json does)
        let firebase_dir = plugins_dir.join("firebase");
        fs::create_dir_all(&firebase_dir).expect("mkdir firebase");
        fs::write(
            firebase_dir.join("plugin.json"),
            r#"{"name":"firebase","displayName":"Firebase"}"#,
        )
        .expect("write firebase manifest");
        write_skill(
            &firebase_dir.join("skills/auth"),
            "firebase-auth",
            "Auth help.",
        );

        // 3. Manifest-disabled plugin (not listed in config.json)
        let shipped_off = plugins_dir.join("shipped-off");
        fs::create_dir_all(&shipped_off).expect("mkdir shipped-off");
        fs::write(
            shipped_off.join("plugin.json"),
            r#"{"name":"shipped-off","disabled":true}"#,
        )
        .expect("write shipped-off manifest");
        write_skill(&shipped_off.join("skills/demo"), "demo", "Demo.");

        fs::write(
            config_dir.join("config.json"),
            r#"{
              "plugins": {
                "vercel-plugin": {
                  "installedFrom": { "marketplace": "antigravity-plugins-partners", "version": "0.56.0" }
                },
                "firebase": {
                  "enabled": false,
                  "installedFrom": { "marketplace": "antigravity-plugins-official", "version": "1.0.3" }
                }
              }
            }"#,
        )
        .expect("write config.json");

        let cfg = AppConfig {
            active_agents: ActiveAgents {
                antigravity: true,
                codex: false,
                claude: false,
            },
            ..AppConfig::default()
        };
        let roots = config::resolve_agent_roots(&cfg, home);
        let mut warnings = Vec::new();
        let plugins = scan_global_plugins(&cfg, &roots, &mut warnings);

        assert!(warnings.is_empty(), "warnings: {warnings:?}");
        assert_eq!(plugins.len(), 3);

        let vercel = plugins
            .iter()
            .find(|p| p.id == "vercel-plugin")
            .expect("vercel-plugin");
        assert_eq!(vercel.agent, AgentKind::Antigravity);
        assert!(vercel.enabled);
        assert_eq!(vercel.name, "Vercel");
        assert_eq!(vercel.source.as_deref(), Some("Partner"));
        assert_eq!(vercel.skills.len(), 1);
        assert_eq!(vercel.skills[0].name, "ai-sdk");
        assert_eq!(vercel.agent_names, vec!["ai-architect"]);
        assert_eq!(vercel.rule_names, vec!["AGENTS.md"]);
        assert_eq!(vercel.mcp_names, vec!["vercel"]);
        assert!(vercel.skills_upfront_tokens > 0);
        assert!(vercel.agents_upfront_tokens > 0);
        assert!(vercel.rules_upfront_tokens > 0);
        assert_eq!(
            vercel.upfront_tokens,
            vercel.skills_upfront_tokens + vercel.agents_upfront_tokens
        );

        let firebase = plugins
            .iter()
            .find(|p| p.id == "firebase")
            .expect("firebase");
        assert!(!firebase.enabled);
        assert_eq!(firebase.source.as_deref(), Some("Official"));

        let off = plugins
            .iter()
            .find(|p| p.id == "shipped-off")
            .expect("shipped-off");
        assert!(!off.enabled);
        assert_eq!(off.source.as_deref(), Some("Local"));
    }

    #[test]
    fn scans_codex_plugins_including_depth2_skills_and_toggles_toml_safely() {
        let tmp = TempDir::new().expect("temp dir");
        let root = tmp.path();
        let home = root.join("home");
        let backups = root.join("backups");
        let codex_dir = home.join(".codex");
        let cache_dir = codex_dir.join("plugins/cache");

        // 1. Codex vercel-plugin@plugins-cli with depth-2 upstream skill + .mcp.json
        let vercel_cache = cache_dir.join("plugins-cli/vercel-plugin/0.32.0");
        fs::create_dir_all(vercel_cache.join(".codex-plugin")).expect("mkdir manifest");
        fs::write(
            vercel_cache.join(".codex-plugin/plugin.json"),
            r#"{"name":"vercel-plugin","version":"0.32.0","description":"Vercel Codex plugin","interface":{"displayName":"Vercel-plugin"}}"#,
        )
        .expect("write codex manifest");
        write_skill(
            &vercel_cache.join("skills/ai-sdk"),
            "ai-sdk",
            "Primary skill.",
        );
        write_skill(
            &vercel_cache.join("skills/ai-sdk/upstream"),
            "ai-sdk-upstream",
            "Upstream skill.",
        );
        fs::write(
            vercel_cache.join(".mcp.json"),
            r#"{"mcpServers":{"vercel":{"url":"https://mcp.vercel.com"}}}"#,
        )
        .expect("write .mcp.json");

        // 2. Disabled openai-curated plugin stored in openai-curated-remote cache
        let github_cache = cache_dir.join("openai-curated-remote/github/0.1.13");
        fs::create_dir_all(github_cache.join(".codex-plugin")).expect("mkdir github manifest");
        fs::write(
            github_cache.join(".codex-plugin/plugin.json"),
            r#"{"name":"github","version":"0.1.13","interface":{"displayName":"GitHub","shortDescription":"Triage PRs"}}"#,
        )
        .expect("write github manifest");

        let config_toml = codex_dir.join("config.toml");
        fs::write(
            &config_toml,
            "# Keep user comment\nmodel = \"gpt-5\"\n\n[plugins.\"vercel-plugin@plugins-cli\"]\nenabled = true\n\n[plugins.\"github@openai-curated\"]\nenabled = false\n",
        )
        .expect("write config.toml");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&config_toml, fs::Permissions::from_mode(0o600)).expect("set 0600");
        }

        let cfg = AppConfig {
            active_agents: ActiveAgents {
                antigravity: false,
                codex: true,
                claude: false,
            },
            ..AppConfig::default()
        };
        let roots = config::resolve_agent_roots(&cfg, &home);
        let mut warnings = Vec::new();
        let plugins = scan_global_plugins(&cfg, &roots, &mut warnings);

        assert!(warnings.is_empty(), "warnings: {warnings:?}");
        assert_eq!(plugins.len(), 2);

        let vercel = plugins
            .iter()
            .find(|p| p.id == "vercel-plugin@plugins-cli")
            .expect("vercel codex");
        assert_eq!(vercel.agent, AgentKind::Codex);
        assert!(vercel.enabled);
        assert_eq!(vercel.name, "Vercel-plugin");
        assert_eq!(vercel.source.as_deref(), Some("Plugins CLI"));
        assert_eq!(vercel.skills.len(), 2);
        assert_eq!(vercel.mcp_names, vec!["vercel"]);

        let github = plugins
            .iter()
            .find(|p| p.id == "github@openai-curated")
            .expect("github codex");
        assert!(!github.enabled);
        assert_eq!(github.source.as_deref(), Some("OpenAI Curated"));
        assert_eq!(github.description.as_deref(), Some("Triage PRs"));

        // Toggle vercel OFF and back ON via config.toml
        toggle_plugin(
            &cfg,
            &home,
            AgentKind::Codex,
            "vercel-plugin@plugins-cli",
            DesiredState::Disable,
            Some(&backups),
        )
        .expect("disable codex plugin");

        let toml_disabled = fs::read_to_string(&config_toml).expect("read disabled toml");
        assert!(toml_disabled.contains("# Keep user comment"));
        assert!(toml_disabled.contains("model = \"gpt-5\""));
        assert_eq!(fs::read_dir(&backups).expect("read backups").count(), 1);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&config_toml)
                .expect("meta")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
        }

        let after_disable = scan_global_plugins(&cfg, &roots, &mut warnings);
        assert!(
            !after_disable
                .iter()
                .find(|p| p.id == "vercel-plugin@plugins-cli")
                .expect("vercel")
                .enabled
        );

        toggle_plugin(
            &cfg,
            &home,
            AgentKind::Codex,
            "vercel-plugin@plugins-cli",
            DesiredState::Enable,
            Some(&backups),
        )
        .expect("enable codex plugin");

        let after_enable = scan_global_plugins(&cfg, &roots, &mut warnings);
        assert!(
            after_enable
                .iter()
                .find(|p| p.id == "vercel-plugin@plugins-cli")
                .expect("vercel")
                .enabled
        );
    }

    #[test]
    fn toggle_antigravity_plugin_preserves_sibling_keys_creates_backup_and_keeps_0600_mode() {
        let tmp = TempDir::new().expect("temp dir");
        let root = tmp.path();
        let home = root.join("home");
        let backups = root.join("backups");
        let config_dir = home.join(".gemini/config");
        let plugin_dir = config_dir.join("plugins/vercel-plugin");
        fs::create_dir_all(&plugin_dir).expect("mkdir plugin");
        fs::write(
            plugin_dir.join("plugin.json"),
            r#"{"name":"vercel-plugin"}"#,
        )
        .expect("write manifest");

        let config_json = config_dir.join("config.json");
        fs::write(
            &config_json,
            r#"{
  "plugins": {
    "vercel-plugin": {
      "installedFrom": {
        "id": "antigravity-plugins-partners/plugin/vercel-plugin",
        "marketplace": "antigravity-plugins-partners",
        "version": "0.56.0"
      }
    }
  },
  "sidecars": { "demo": true },
  "userSettings": { "theme": "dark" }
}
"#,
        )
        .expect("write config.json");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&config_json, fs::Permissions::from_mode(0o600)).expect("set 0600");
        }

        let cfg = AppConfig::default();

        toggle_plugin(
            &cfg,
            &home,
            AgentKind::Antigravity,
            "vercel-plugin",
            DesiredState::Disable,
            Some(&backups),
        )
        .expect("disable plugin");

        let after_disable: JsonValue =
            serde_json::from_str(&fs::read_to_string(&config_json).expect("read disabled"))
                .expect("parse disabled");
        assert_eq!(
            after_disable["plugins"]["vercel-plugin"]["enabled"],
            JsonValue::Bool(false)
        );
        assert_eq!(
            after_disable["plugins"]["vercel-plugin"]["installedFrom"]["version"],
            "0.56.0"
        );
        assert_eq!(after_disable["sidecars"]["demo"], true);
        assert_eq!(after_disable["userSettings"]["theme"], "dark");
        assert!(backups.is_dir());
        assert_eq!(fs::read_dir(&backups).expect("read backups").count(), 1);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&config_json)
                .expect("meta")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
        }

        toggle_plugin(
            &cfg,
            &home,
            AgentKind::Antigravity,
            "vercel-plugin",
            DesiredState::Enable,
            Some(&backups),
        )
        .expect("enable plugin");

        let after_enable: JsonValue =
            serde_json::from_str(&fs::read_to_string(&config_json).expect("read enabled"))
                .expect("parse enabled");
        assert_eq!(
            after_enable["plugins"]["vercel-plugin"]["enabled"],
            JsonValue::Bool(true)
        );
        assert_eq!(after_enable["userSettings"]["theme"], "dark");
    }

    #[test]
    fn scans_claude_plugins_and_customizations_read_only() {
        let tmp = TempDir::new().expect("temp dir");
        let home = tmp.path();
        let claude_dir = home.join(".claude");
        let plugin_install_dir = claude_dir.join("plugins/cache/official/vercel/1.0.0");
        fs::create_dir_all(plugin_install_dir.join(".claude-plugin")).expect("mkdir manifest");
        fs::write(
            plugin_install_dir.join(".claude-plugin/plugin.json"),
            r#"{"name":"vercel","displayName":"Vercel Claude","description":"Deploy via Claude","version":"1.0.0"}"#,
        )
        .expect("write plugin.json");
        write_skill(
            &plugin_install_dir.join("skills/deploy"),
            "deploy",
            "Deploy to Vercel.",
        );

        fs::create_dir_all(claude_dir.join("plugins")).expect("mkdir plugins");
        fs::write(
            claude_dir.join("plugins/installed_plugins.json"),
            serde_json::json!({
                "version": 2,
                "plugins": {
                    "vercel@official": [{
                        "scope": "user",
                        "installPath": plugin_install_dir.to_string_lossy(),
                        "version": "1.0.0"
                    }]
                }
            })
            .to_string(),
        )
        .expect("write installed_plugins.json");

        fs::write(
            claude_dir.join("settings.json"),
            r#"{"enabledPlugins":{"vercel@official":true,"disabled-one@local":false}}"#,
        )
        .expect("write settings.json");

        fs::create_dir_all(claude_dir.join("agents")).expect("mkdir agents");
        fs::write(
            claude_dir.join("agents/reviewer.md"),
            "---\nname: reviewer\ndescription: Reviews PRs.\n---\nBody\n",
        )
        .expect("write agent");

        fs::create_dir_all(claude_dir.join("commands")).expect("mkdir commands");
        fs::write(claude_dir.join("commands/ship.md"), "Ship it\n").expect("write command");

        let cfg = AppConfig {
            active_agents: ActiveAgents {
                antigravity: false,
                codex: false,
                claude: true,
            },
            ..AppConfig::default()
        };
        let roots = config::resolve_agent_roots(&cfg, home);
        let mut warnings = Vec::new();
        let items = scan_global_plugins(&cfg, &roots, &mut warnings);

        assert!(warnings.is_empty(), "warnings: {warnings:?}");
        assert_eq!(items.len(), 4);

        let vercel = items
            .iter()
            .find(|p| p.id == "vercel@official")
            .expect("vercel plugin");
        assert!(vercel.enabled);
        assert!(vercel.can_toggle);
        assert_eq!(vercel.name, "Vercel Claude");
        assert_eq!(vercel.skills.len(), 1);
        assert!(vercel
            .config_path
            .as_deref()
            .unwrap_or("")
            .ends_with("settings.json"));

        let disabled = items
            .iter()
            .find(|p| p.id == "disabled-one@local")
            .expect("disabled plugin");
        assert!(!disabled.enabled);
        assert!(disabled.can_toggle);

        let agents = items
            .iter()
            .find(|p| p.id == "claude-custom-agents")
            .expect("custom agents");
        assert!(!agents.can_toggle);
        assert_eq!(agents.agent_names, vec!["reviewer"]);

        let commands = items
            .iter()
            .find(|p| p.id == "claude-custom-commands")
            .expect("custom commands");
        assert!(!commands.can_toggle);
        assert_eq!(commands.skills.len(), 1);
        assert_eq!(commands.skills[0].name, "/ship");
        assert_eq!(commands.upfront_tokens, 0);
    }

    #[test]
    fn toggle_claude_plugin_updates_settings_json_preserves_keys_handles_local_override_and_rejects_synthetic() {
        let tmp = TempDir::new().expect("temp dir");
        let root = tmp.path();
        let home = root.join("home");
        let backups = root.join("backups");
        let claude_dir = home.join(".claude");
        fs::create_dir_all(&claude_dir).expect("mkdir .claude");

        let settings_path = claude_dir.join("settings.json");
        fs::write(
            &settings_path,
            r#"{
  "model": "sonnet",
  "enabledPlugins": {
    "vercel-plugin@vercel-vercel-plugin": false,
    "stripe@claude-plugins-official": true
  },
  "extraKnownMarketplaces": {
    "apify-agent-skills": { "source": { "source": "git", "url": "https://example.com/repo.git" } }
  },
  "permissions": { "allow": ["Read(*)"] }
}
"#,
        )
        .expect("write settings.json");

        let settings_local_path = claude_dir.join("settings.local.json");
        fs::write(
            &settings_local_path,
            r#"{
  "enabledPlugins": {
    "vercel-plugin@vercel-vercel-plugin": false
  },
  "customFlag": 123
}
"#,
        )
        .expect("write settings.local.json");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&settings_path, fs::Permissions::from_mode(0o600))
                .expect("set 0600 settings.json");
            fs::set_permissions(&settings_local_path, fs::Permissions::from_mode(0o600))
                .expect("set 0600 settings.local.json");
        }

        let cfg = AppConfig {
            active_agents: ActiveAgents {
                antigravity: false,
                codex: false,
                claude: true,
            },
            ..AppConfig::default()
        };

        // Enable vercel-plugin: updates both settings.json and local override, creates backups, preserves keys & mode
        toggle_plugin(
            &cfg,
            &home,
            AgentKind::Claude,
            "vercel-plugin@vercel-vercel-plugin",
            DesiredState::Enable,
            Some(&backups),
        )
        .expect("enable claude plugin");

        let after_enable: JsonValue =
            serde_json::from_str(&fs::read_to_string(&settings_path).expect("read settings"))
                .expect("parse settings");
        assert_eq!(
            after_enable["enabledPlugins"]["vercel-plugin@vercel-vercel-plugin"],
            JsonValue::Bool(true)
        );
        assert_eq!(
            after_enable["enabledPlugins"]["stripe@claude-plugins-official"],
            JsonValue::Bool(true)
        );
        assert_eq!(after_enable["model"], "sonnet");
        assert_eq!(after_enable["permissions"]["allow"][0], "Read(*)");
        assert_eq!(
            after_enable["extraKnownMarketplaces"]["apify-agent-skills"]["source"]["source"],
            "git"
        );

        let local_after_enable: JsonValue = serde_json::from_str(
            &fs::read_to_string(&settings_local_path).expect("read settings.local"),
        )
        .expect("parse settings.local");
        assert_eq!(
            local_after_enable["enabledPlugins"]["vercel-plugin@vercel-vercel-plugin"],
            JsonValue::Bool(true)
        );
        assert_eq!(local_after_enable["customFlag"], 123);

        assert_eq!(fs::read_dir(&backups).expect("read backups").count(), 2);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            for path in [&settings_path, &settings_local_path] {
                let mode = fs::metadata(path).expect("meta").permissions().mode() & 0o777;
                assert_eq!(mode, 0o600);
            }
        }

        // Disable stripe@claude-plugins-official: should not create or touch unrelated local override
        toggle_plugin(
            &cfg,
            &home,
            AgentKind::Claude,
            "stripe@claude-plugins-official",
            DesiredState::Disable,
            Some(&backups),
        )
        .expect("disable claude plugin");

        let after_disable: JsonValue =
            serde_json::from_str(&fs::read_to_string(&settings_path).expect("read settings"))
                .expect("parse settings");
        assert_eq!(
            after_disable["enabledPlugins"]["stripe@claude-plugins-official"],
            JsonValue::Bool(false)
        );
        let local_after_disable: JsonValue = serde_json::from_str(
            &fs::read_to_string(&settings_local_path).expect("read settings.local"),
        )
        .expect("parse settings.local");
        assert!(local_after_disable["enabledPlugins"]
            .get("stripe@claude-plugins-official")
            .is_none());

        // Synthetic items cannot be toggled
        for synthetic in ["claude-custom-agents", "claude-custom-commands"] {
            let err = toggle_plugin(
                &cfg,
                &home,
                AgentKind::Claude,
                synthetic,
                DesiredState::Disable,
                Some(&backups),
            )
            .expect_err("synthetic should fail");
            assert!(err.to_string().contains("directory-based"));
        }
    }
}

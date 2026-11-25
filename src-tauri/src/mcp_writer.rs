//! Backup-protected, comment- & key-preserving MCP writers for Antigravity, Codex, and Claude.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value as JsonValue};
use toml_edit::{Array, DocumentMut, Item, Table, Value as TomlValue};

use crate::backup;
use crate::config;
use crate::error::AppError;
use crate::git_hygiene;
use crate::mcp_catalog;
use crate::models::{AgentKind, AppConfig, DesiredState, McpCatalogEntry, McpTransport, ToggleScope};

/// Toggles an MCP server ON or OFF across enabled agents for Global or Project scope.
///
/// Automatically ensures the server's definition is persisted in `mcp-catalog.json`
/// before disabling, creates rolling backups, and preserves unrelated JSON keys and TOML comments.
///
/// # Errors
/// Returns [`AppError`] if the server definition cannot be found when enabling, or on I/O/parse failure.
pub fn toggle_mcp_server(
    cfg: &AppConfig,
    home: &Path,
    scope: &ToggleScope,
    server_name: &str,
    desired: DesiredState,
) -> Result<(), AppError> {
    let catalog_path = config::mcp_catalog_path()?;
    toggle_mcp_server_with_paths(cfg, home, scope, server_name, desired, &catalog_path, None)
}

/// Variant of [`toggle_mcp_server`] accepting explicit `catalog_path` and `custom_backups_root` for tests.
///
/// # Errors
/// Returns [`AppError`] on validation, parse, backup, or write failure.
pub fn toggle_mcp_server_with_paths(
    cfg: &AppConfig,
    home: &Path,
    scope: &ToggleScope,
    server_name: &str,
    desired: DesiredState,
    catalog_path: &Path,
    custom_backups_root: Option<&Path>,
) -> Result<(), AppError> {
    let mut catalog = mcp_catalog::load_catalog_from(catalog_path)?;

    match desired {
        DesiredState::Enable => {
            let entry = catalog.servers.get(server_name).cloned().ok_or_else(|| {
                AppError::InvalidPath(format!(
                    "MCP server '{server_name}' is not in mcp-catalog.json; add it first"
                ))
            })?;
            write_server_state(
                cfg,
                home,
                scope,
                server_name,
                Some(&entry),
                custom_backups_root,
            )
        }
        DesiredState::Disable => {
            if !catalog.servers.contains_key(server_name) {
                if let Some(recovered) = recover_server_from_configs(cfg, home, scope, server_name)
                {
                    catalog.servers.insert(server_name.to_owned(), recovered);
                    mcp_catalog::save_catalog_to(&catalog, catalog_path, custom_backups_root)?;
                }
            }
            write_server_state(cfg, home, scope, server_name, None, custom_backups_root)
        }
    }
}

fn write_server_state(
    cfg: &AppConfig,
    home: &Path,
    scope: &ToggleScope,
    server_name: &str,
    entry: Option<&McpCatalogEntry>,
    custom_backups_root: Option<&Path>,
) -> Result<(), AppError> {
    let is_enable = entry.is_some();
    let roots = crate::config::resolve_agent_roots(cfg, home);

    match scope {
        ToggleScope::Global => {
            if cfg.active_agents.antigravity {
                let primary = roots.antigravity_dir.join("config/mcp_config.json");
                mutate_json_mcp_file(
                    &primary,
                    JsonTargetKind::McpServersRoot,
                    server_name,
                    entry,
                    is_enable,
                    custom_backups_root,
                )?;

                let legacy = roots.antigravity_dir.join("antigravity/mcp_config.json");
                if legacy.is_file() && !same_canonical_file(&primary, &legacy) {
                    mutate_json_mcp_file(
                        &legacy,
                        JsonTargetKind::McpServersRoot,
                        server_name,
                        entry,
                        is_enable,
                        custom_backups_root,
                    )?;
                }
            }

            if cfg.active_agents.codex {
                let toml_path = roots.codex_dir.join("config.toml");
                mutate_toml_mcp_file(
                    &toml_path,
                    server_name,
                    entry,
                    is_enable,
                    custom_backups_root,
                )?;
            }

            if cfg.active_agents.claude {
                mutate_json_mcp_file(
                    &roots.claude_json,
                    JsonTargetKind::ClaudeGlobalMcpServers,
                    server_name,
                    entry,
                    is_enable,
                    custom_backups_root,
                )?;
            }
        }
        ToggleScope::Project { project_path } => {
            let project = Path::new(project_path);
            let edits = project_mcp_edits(cfg, home, project, server_name, entry)?;
            let git_add: Vec<_> = edits.iter().filter_map(|edit| edit.git_relative.clone()).collect();
            for edit in edits {
                backup_if_exists(&edit.path, custom_backups_root)?;
                backup::atomic_write_string(&edit.path, &edit.content)?;
            }
            if cfg.git_hygiene && !git_add.is_empty() {
                git_hygiene::sync_git_exclude(project, &git_add, &[], custom_backups_root)?;
            }
        }
    }

    Ok(())
}

pub(crate) struct ProjectMcpEdit {
    pub path: PathBuf,
    pub agent: AgentKind,
    pub content: String,
    git_relative: Option<String>,
}

/// Renders project MCP changes in memory for both preview and apply.
pub(crate) fn project_mcp_edits(
    cfg: &AppConfig,
    home: &Path,
    project: &Path,
    name: &str,
    entry: Option<&McpCatalogEntry>,
) -> Result<Vec<ProjectMcpEdit>, AppError> {
    let enable = entry.is_some();
    let mut edits = Vec::new();
    let mut add = |path: PathBuf, agent, content: Option<String>, relative: Option<&str>| {
        if let Some(content) = content {
            edits.push(ProjectMcpEdit {
                path, agent, content,
                git_relative: relative.filter(|_| enable).map(str::to_owned),
            });
        }
    };
    if cfg.active_agents.antigravity {
        let relative = ".agents/mcp_config.json";
        let path = project.join(relative);
        let content = render_json_mcp_edit(&path, JsonTargetKind::McpServersRoot, name, entry, enable)?;
        add(path, AgentKind::Antigravity, content, Some(relative));
    }
    if cfg.active_agents.codex {
        let relative = ".codex/config.toml";
        let path = project.join(relative);
        let content = render_toml_mcp_edit(&path, name, entry, enable)?;
        add(path, AgentKind::Codex, content, Some(relative));
    }
    if cfg.active_agents.claude {
        let relative = ".mcp.json";
        let path = project.join(relative);
        let content = render_json_mcp_edit(&path, JsonTargetKind::McpServersRoot, name, entry, enable)?;
        add(path, AgentKind::Claude, content, Some(relative));
        if !enable {
            let path = config::resolve_agent_roots(cfg, home).claude_json;
            let content = render_json_mcp_edit(
                &path, JsonTargetKind::ClaudeProjectMcpServers(project), name, None, false,
            )?;
            add(path, AgentKind::Claude, content, None);
        }
    }
    Ok(edits)
}

enum JsonTargetKind<'a> {
    McpServersRoot,
    ClaudeGlobalMcpServers,
    ClaudeProjectMcpServers(&'a Path),
}

fn mutate_json_mcp_file(
    path: &Path,
    kind: JsonTargetKind<'_>,
    server_name: &str,
    entry: Option<&McpCatalogEntry>,
    create_if_missing: bool,
    custom_backups_root: Option<&Path>,
) -> Result<(), AppError> {
    if let Some(content) = render_json_mcp_edit(path, kind, server_name, entry, create_if_missing)? {
        backup_if_exists(path, custom_backups_root)?;
        backup::atomic_write_string(path, &content)?;
    }
    Ok(())
}

fn render_json_mcp_edit(
    path: &Path,
    kind: JsonTargetKind<'_>,
    server_name: &str,
    entry: Option<&McpCatalogEntry>,
    create_if_missing: bool,
) -> Result<Option<String>, AppError> {
    if !path.is_file() && !create_if_missing {
        return Ok(None);
    }

    let mut root_obj = if path.is_file() {
        let raw = fs::read_to_string(path).map_err(|e| AppError::io(path, e))?;
        let val: JsonValue = serde_json::from_str(&raw)?;
        val.as_object().cloned().ok_or_else(|| {
            AppError::InvalidPath(format!("Expected JSON object in {}", path.display()))
        })?
    } else {
        Map::new()
    };

    let mut changed = false;
    match kind {
        JsonTargetKind::McpServersRoot | JsonTargetKind::ClaudeGlobalMcpServers => {
            let mut servers = root_obj
                .get("mcpServers")
                .and_then(JsonValue::as_object)
                .cloned()
                .unwrap_or_default();

            if let Some(entry) = entry {
                let include_claude_type = matches!(kind, JsonTargetKind::ClaudeGlobalMcpServers)
                    || path.file_name().is_some_and(|n| n == ".mcp.json");
                let value = catalog_entry_to_json(entry, include_claude_type);
                if servers.get(server_name) != Some(&value) {
                    servers.insert(server_name.to_owned(), value);
                    changed = true;
                }
            } else if servers.remove(server_name).is_some() {
                changed = true;
            }

            if changed {
                root_obj.insert("mcpServers".to_owned(), JsonValue::Object(servers));
            }
        }
        JsonTargetKind::ClaudeProjectMcpServers(project_path) => {
            let canonical_proj = fs::canonicalize(project_path)
                .unwrap_or_else(|_| project_path.to_path_buf())
                .to_string_lossy()
                .into_owned();
            let raw_proj = project_path.to_string_lossy().into_owned();

            if let Some(projects_obj) = root_obj
                .get_mut("projects")
                .and_then(JsonValue::as_object_mut)
            {
                for key in [canonical_proj, raw_proj] {
                    if let Some(proj_entry) = projects_obj
                        .get_mut(&key)
                        .and_then(JsonValue::as_object_mut)
                    {
                        if let Some(servers) = proj_entry
                            .get_mut("mcpServers")
                            .and_then(JsonValue::as_object_mut)
                        {
                            if servers.remove(server_name).is_some() {
                                changed = true;
                            }
                        }
                    }
                }
            }
        }
    }

    if !changed {
        return Ok(None);
    }

    let formatted = serde_json::to_string_pretty(&JsonValue::Object(root_obj))?;
    Ok(Some(format!("{formatted}\n")))
}

fn catalog_entry_to_json(entry: &McpCatalogEntry, include_type: bool) -> JsonValue {
    let mut obj = Map::new();

    if include_type {
        let type_name = match entry.transport {
            McpTransport::Stdio => Some("stdio"),
            McpTransport::Http => Some("http"),
            McpTransport::Sse => Some("sse"),
            McpTransport::Unknown => None,
        };
        if let Some(t) = type_name {
            obj.insert("type".to_owned(), JsonValue::String(t.to_owned()));
        }
    }

    if let Some(cmd) = &entry.command {
        obj.insert("command".to_owned(), JsonValue::String(cmd.clone()));
        if !entry.args.is_empty() {
            obj.insert(
                "args".to_owned(),
                JsonValue::Array(entry.args.iter().cloned().map(JsonValue::String).collect()),
            );
        }
        if !entry.env.is_empty() {
            let env_map: Map<String, JsonValue> = entry
                .env
                .iter()
                .map(|(k, v)| (k.clone(), JsonValue::String(v.clone())))
                .collect();
            obj.insert("env".to_owned(), JsonValue::Object(env_map));
        }
    } else if let Some(url) = &entry.url {
        obj.insert("url".to_owned(), JsonValue::String(url.clone()));
        if !entry.headers.is_empty() {
            let hdr_map: Map<String, JsonValue> = entry
                .headers
                .iter()
                .map(|(k, v)| (k.clone(), JsonValue::String(v.clone())))
                .collect();
            obj.insert("headers".to_owned(), JsonValue::Object(hdr_map));
        }
    }

    JsonValue::Object(obj)
}

fn mutate_toml_mcp_file(
    path: &Path,
    server_name: &str,
    entry: Option<&McpCatalogEntry>,
    create_if_missing: bool,
    custom_backups_root: Option<&Path>,
) -> Result<(), AppError> {
    if let Some(content) = render_toml_mcp_edit(path, server_name, entry, create_if_missing)? {
        backup_if_exists(path, custom_backups_root)?;
        backup::atomic_write_string(path, &content)?;
    }
    Ok(())
}

fn render_toml_mcp_edit(
    path: &Path,
    server_name: &str,
    entry: Option<&McpCatalogEntry>,
    create_if_missing: bool,
) -> Result<Option<String>, AppError> {
    if !path.is_file() && !create_if_missing {
        return Ok(None);
    }

    let mut doc = if path.is_file() {
        let raw = fs::read_to_string(path).map_err(|e| AppError::io(path, e))?;
        raw.parse::<DocumentMut>().map_err(|e| {
            AppError::InvalidPath(format!("Invalid TOML in {}: {e}", path.display()))
        })?
    } else {
        DocumentMut::new()
    };

    let mut changed = false;

    if let Some(entry) = entry {
        if !doc.as_table().contains_key("mcp_servers") {
            let mut tbl = Table::new();
            tbl.set_implicit(true);
            doc["mcp_servers"] = Item::Table(tbl);
        }

        if let Some(mcp_table) = doc
            .get_mut("mcp_servers")
            .and_then(|item| item.as_table_like_mut())
        {
            let next = catalog_entry_to_toml(entry);
            let current = mcp_table.get(server_name);
            if !current.and_then(Item::as_table).is_some_and(|table| {
                table.to_string() == next.to_string()
            }) {
                mcp_table.insert(server_name, Item::Table(next));
                changed = true;
            }
        } else {
            return Err(AppError::InvalidPath(format!(
                "Expected an mcp_servers table in {}", path.display()
            )));
        }
    } else if let Some(mcp_table) = doc
        .get_mut("mcp_servers")
        .and_then(|item| item.as_table_like_mut())
    {
        if mcp_table.remove(server_name).is_some() {
            changed = true;
        }
    }

    if !changed {
        return Ok(None);
    }

    Ok(Some(doc.to_string()))
}

fn catalog_entry_to_toml(entry: &McpCatalogEntry) -> Table {
    let mut table = Table::new();

    if let Some(cmd) = &entry.command {
        table["command"] = toml_edit::value(cmd.as_str());
        if !entry.args.is_empty() {
            let mut arr = Array::new();
            for arg in &entry.args {
                arr.push(arg.as_str());
            }
            table["args"] = Item::Value(TomlValue::Array(arr));
        }
        if !entry.env.is_empty() {
            let mut env_tbl = Table::new();
            for (k, v) in &entry.env {
                env_tbl[k.as_str()] = toml_edit::value(v.as_str());
            }
            table["env"] = Item::Table(env_tbl);
        }
    } else if let Some(url) = &entry.url {
        table["url"] = toml_edit::value(url.as_str());
        if !entry.headers.is_empty() {
            let mut hdr_tbl = Table::new();
            for (k, v) in &entry.headers {
                hdr_tbl[k.as_str()] = toml_edit::value(v.as_str());
            }
            table["http_headers"] = Item::Table(hdr_tbl);
        }
    }

    table
}

fn recover_server_from_configs(
    cfg: &AppConfig,
    home: &Path,
    scope: &ToggleScope,
    server_name: &str,
) -> Option<McpCatalogEntry> {
    let roots = crate::config::resolve_agent_roots(cfg, home);
    let mut candidates: Vec<PathBuf> = Vec::new();
    match scope {
        ToggleScope::Global => {
            if cfg.active_agents.antigravity {
                candidates.push(roots.antigravity_dir.join("config/mcp_config.json"));
                candidates.push(roots.antigravity_dir.join("antigravity/mcp_config.json"));
            }
            if cfg.active_agents.codex {
                candidates.push(roots.codex_dir.join("config.toml"));
            }
            if cfg.active_agents.claude {
                candidates.push(roots.claude_json);
            }
        }
        ToggleScope::Project { project_path } => {
            let proj = Path::new(project_path);
            candidates.push(proj.join(".agents/mcp_config.json"));
            candidates.push(proj.join(".codex/config.toml"));
            candidates.push(proj.join(".mcp.json"));
        }
    }

    for path in candidates {
        if !path.is_file() {
            continue;
        }
        if let Some(entry) = extract_server_entry_from_file(&path, server_name) {
            return Some(entry);
        }
    }
    None
}

fn extract_server_entry_from_file(path: &Path, server_name: &str) -> Option<McpCatalogEntry> {
    let raw = fs::read_to_string(path).ok()?;
    if path.extension().and_then(|e| e.to_str()) == Some("toml") {
        let doc = raw.parse::<DocumentMut>().ok()?;
        let table = doc
            .get("mcp_servers")
            .and_then(|v| v.as_table_like())?
            .get(server_name)
            .and_then(|v| v.as_table_like())?;

        let command = table
            .get("command")
            .and_then(|v| v.as_str())
            .map(ToOwned::to_owned);
        let url = table
            .get("url")
            .or_else(|| table.get("http_url"))
            .and_then(|v| v.as_str())
            .map(ToOwned::to_owned);
        let args = table
            .get("args")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|i| i.as_str().map(ToOwned::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        let env = table
            .get("env")
            .and_then(|v| v.as_table_like())
            .map(|t| {
                t.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.to_owned(), s.to_owned())))
                    .collect()
            })
            .unwrap_or_default();
        let headers = table
            .get("http_headers")
            .and_then(|v| v.as_table_like())
            .map(|t| {
                t.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.to_owned(), s.to_owned())))
                    .collect()
            })
            .unwrap_or_default();

        let transport = if command.is_some() {
            McpTransport::Stdio
        } else if url.is_some() {
            McpTransport::Http
        } else {
            McpTransport::Unknown
        };

        return Some(McpCatalogEntry {
            name: server_name.to_owned(),
            transport,
            command,
            args,
            url,
            env,
            headers,
            upfront_tokens: None,
            tool_count: None,
            probed_at_ms: None,
        });
    }

    let json: JsonValue = serde_json::from_str(&raw).ok()?;
    let obj = json
        .get("mcpServers")
        .and_then(JsonValue::as_object)?
        .get(server_name)
        .and_then(JsonValue::as_object)?;

    let command = obj
        .get("command")
        .and_then(JsonValue::as_str)
        .map(ToOwned::to_owned);
    let url = obj
        .get("url")
        .or_else(|| obj.get("serverUrl"))
        .or_else(|| obj.get("httpUrl"))
        .and_then(JsonValue::as_str)
        .map(ToOwned::to_owned);
    let type_str = obj
        .get("type")
        .and_then(JsonValue::as_str)
        .map(str::to_ascii_lowercase);
    let transport = match type_str.as_deref() {
        Some("sse") => McpTransport::Sse,
        Some("http") | Some("streamable-http") | Some("streamable_http") => McpTransport::Http,
        Some("stdio") => McpTransport::Stdio,
        _ if command.is_some() => McpTransport::Stdio,
        _ if url.is_some() => McpTransport::Http,
        _ => McpTransport::Unknown,
    };
    let args = obj
        .get("args")
        .and_then(JsonValue::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(ToOwned::to_owned))
                .collect()
        })
        .unwrap_or_default();
    let env = obj
        .get("env")
        .and_then(JsonValue::as_object)
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_owned())))
                .collect()
        })
        .unwrap_or_default();
    let headers = obj
        .get("headers")
        .and_then(JsonValue::as_object)
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_owned())))
                .collect()
        })
        .unwrap_or_default();

    Some(McpCatalogEntry {
        name: server_name.to_owned(),
        transport,
        command,
        args,
        url,
        env,
        headers,
        upfront_tokens: None,
        tool_count: None,
        probed_at_ms: None,
    })
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

fn same_canonical_file(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(ca), Ok(cb)) => ca == cb,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ActiveAgents, McpServerDraft};
    use std::collections::BTreeMap;
    use tempfile::TempDir;

    #[test]
    fn preserves_codex_toml_comments_and_claude_json_unrelated_keys() {
        let tmp = TempDir::new().expect("temp dir");
        let root = tmp.path();
        let home = root.join("home");
        let backups = root.join("backups");
        let catalog_path = root.join("mcp-catalog.json");

        fs::create_dir_all(home.join(".gemini/config")).expect("gemini");
        fs::create_dir_all(home.join(".codex")).expect("codex");

        let toml_path = home.join(".codex/config.toml");
        fs::write(
            &toml_path,
            "# Leading comment\nmodel = \"gpt-5\"\n\n[profiles.fast]\n# profile comment\napproval_policy = \"never\"\n",
        )
        .expect("write config.toml");

        let claude_path = home.join(".claude.json");
        fs::write(
            &claude_path,
            "{\n  \"numStartups\": 42,\n  \"userID\": \"abc\",\n  \"mcpServers\": {}\n}\n",
        )
        .expect("write .claude.json");

        mcp_catalog::upsert_server_to(
            McpServerDraft {
                name: "zeno".to_owned(),
                transport: McpTransport::Stdio,
                command: Some("npx".to_owned()),
                args: vec!["-y".to_owned(), "zeno-mcp".to_owned()],
                url: None,
                env: BTreeMap::from([("API_KEY".to_owned(), "${ZENO_KEY}".to_owned())]),
                headers: BTreeMap::new(),
            },
            &catalog_path,
            Some(&backups),
        )
        .expect("upsert");

        let cfg = AppConfig {
            active_agents: ActiveAgents {
                antigravity: true,
                codex: true,
                claude: true,
            },
            ..AppConfig::default()
        };

        toggle_mcp_server_with_paths(
            &cfg,
            &home,
            &ToggleScope::Global,
            "zeno",
            DesiredState::Enable,
            &catalog_path,
            Some(&backups),
        )
        .expect("enable global mcp");

        let toml_after = fs::read_to_string(&toml_path).expect("read toml");
        assert!(toml_after.contains("# Leading comment"));
        assert!(toml_after.contains("# profile comment"));
        assert!(toml_after.contains("model = \"gpt-5\""));
        assert!(toml_after.contains("[mcp_servers.zeno]"));
        assert!(toml_after.contains("command = \"npx\""));

        let claude_after: JsonValue =
            serde_json::from_str(&fs::read_to_string(&claude_path).expect("read claude"))
                .expect("parse claude");
        assert_eq!(claude_after["numStartups"], 42);
        assert_eq!(claude_after["userID"], "abc");
        assert_eq!(claude_after["mcpServers"]["zeno"]["command"], "npx");

        // Disable and verify catalog entry stays preserved while configs cleanly drop `zeno`
        toggle_mcp_server_with_paths(
            &cfg,
            &home,
            &ToggleScope::Global,
            "zeno",
            DesiredState::Disable,
            &catalog_path,
            Some(&backups),
        )
        .expect("disable global mcp");

        let toml_disabled = fs::read_to_string(&toml_path).expect("read toml disabled");
        assert!(toml_disabled.contains("# Leading comment"));
        assert!(!toml_disabled.contains("[mcp_servers.zeno]"));

        let claude_disabled: JsonValue =
            serde_json::from_str(&fs::read_to_string(&claude_path).expect("read claude disabled"))
                .expect("parse claude disabled");
        assert_eq!(claude_disabled["numStartups"], 42);
        assert!(claude_disabled["mcpServers"].get("zeno").is_none());

        let cat = mcp_catalog::load_catalog_from(&catalog_path).expect("catalog still has zeno");
        assert!(cat.servers.contains_key("zeno"));
    }

    #[test]
    fn project_mcp_toggle_updates_git_exclude_without_touching_gitignore() {
        let tmp = TempDir::new().expect("temp dir");
        let root = tmp.path();
        let home = root.join("home");
        let project = root.join("proj");
        let backups = root.join("backups");
        let catalog_path = root.join("mcp-catalog.json");

        fs::create_dir_all(&home).expect("home");
        fs::create_dir_all(project.join(".git/info")).expect("git");

        mcp_catalog::upsert_server_to(
            McpServerDraft {
                name: "local-db".to_owned(),
                transport: McpTransport::Http,
                command: None,
                args: Vec::new(),
                url: Some("http://localhost:4000/mcp".to_owned()),
                env: BTreeMap::new(),
                headers: BTreeMap::new(),
            },
            &catalog_path,
            Some(&backups),
        )
        .expect("upsert");

        let cfg = AppConfig {
            active_agents: ActiveAgents {
                antigravity: true,
                codex: true,
                claude: false,
            },
            ..AppConfig::default()
        };

        let scope = ToggleScope::Project {
            project_path: project.to_string_lossy().into_owned(),
        };

        toggle_mcp_server_with_paths(
            &cfg,
            &home,
            &scope,
            "local-db",
            DesiredState::Enable,
            &catalog_path,
            Some(&backups),
        )
        .expect("enable project mcp");

        assert!(project.join(".agents/mcp_config.json").is_file());
        assert!(project.join(".codex/config.toml").is_file());
        let exclude = fs::read_to_string(project.join(".git/info/exclude")).expect("exclude");
        assert!(exclude.contains(".agents/mcp_config.json"));
        assert!(exclude.contains(".codex/config.toml"));
    }
}

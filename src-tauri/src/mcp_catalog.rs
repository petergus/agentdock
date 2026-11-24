//! Central MCP Catalog stored at `~/Library/Application Support/AgentDock/mcp-catalog.json`.

use std::fs;
use std::path::Path;

use crate::backup;
use crate::config;
use crate::error::AppError;
use crate::models::{McpCatalog, McpCatalogEntry, McpServerDraft, McpTransport};

/// Loads the central MCP catalog from Application Support, returning empty when absent.
///
/// # Errors
/// Returns [`AppError`] if Application Support cannot be resolved or the catalog JSON is corrupt.
pub fn load_catalog() -> Result<McpCatalog, AppError> {
    let path = config::mcp_catalog_path()?;
    load_catalog_from(&path)
}

/// Loads an [`McpCatalog`] from an explicit file path.
///
/// # Errors
/// Returns [`AppError`] if the existing file cannot be read or parsed as JSON.
pub fn load_catalog_from(path: &Path) -> Result<McpCatalog, AppError> {
    if !path.is_file() {
        return Ok(McpCatalog::default());
    }
    let raw = fs::read_to_string(path).map_err(|e| AppError::io(path, e))?;
    if raw.trim().is_empty() {
        return Ok(McpCatalog::default());
    }
    let catalog: McpCatalog = serde_json::from_str(&raw)?;
    Ok(catalog)
}

/// Saves the central MCP catalog with rolling backup and atomic write.
///
/// # Errors
/// Returns [`AppError`] if Application Support is unavailable or backup/write fails.
pub fn save_catalog(catalog: &McpCatalog) -> Result<(), AppError> {
    let path = config::mcp_catalog_path()?;
    save_catalog_to(catalog, &path, None)
}

/// Saves an [`McpCatalog`] to an explicit path, optionally using a custom backups root.
///
/// # Errors
/// Returns [`AppError`] on serialization, backup, or atomic write failure.
pub fn save_catalog_to(
    catalog: &McpCatalog,
    path: &Path,
    custom_backups_root: Option<&Path>,
) -> Result<(), AppError> {
    if path.is_file() {
        match custom_backups_root {
            Some(dir) => {
                backup::backup_file_in_dir(path, dir)?;
            }
            None => {
                backup::backup_file(path)?;
            }
        }
    }
    let json = serde_json::to_string_pretty(catalog)?;
    backup::atomic_write_string(path, &format!("{json}\n"))
}

/// Merges newly discovered server definitions into `catalog` without overwriting cached probes.
///
/// Returns `true` if `catalog` was modified.
pub fn sync_discovered_entries(catalog: &mut McpCatalog, discovered: &[McpCatalogEntry]) -> bool {
    let mut changed = false;
    for entry in discovered {
        match catalog.servers.get_mut(&entry.name) {
            None => {
                catalog.servers.insert(entry.name.clone(), entry.clone());
                changed = true;
            }
            Some(existing) => {
                if existing.transport == McpTransport::Unknown
                    && entry.transport != McpTransport::Unknown
                {
                    existing.transport = entry.transport;
                    changed = true;
                }
                if existing.command.is_none() && entry.command.is_some() {
                    existing.command = entry.command.clone();
                    changed = true;
                }
                if existing.args.is_empty() && !entry.args.is_empty() {
                    existing.args = entry.args.clone();
                    changed = true;
                }
                if existing.url.is_none() && entry.url.is_some() {
                    existing.url = entry.url.clone();
                    changed = true;
                }
                for (k, v) in &entry.env {
                    if !existing.env.contains_key(k) {
                        existing.env.insert(k.clone(), v.clone());
                        changed = true;
                    }
                }
                for (k, v) in &entry.headers {
                    if !existing.headers.contains_key(k) {
                        existing.headers.insert(k.clone(), v.clone());
                        changed = true;
                    }
                }
            }
        }
    }
    changed
}

/// Validates and stores a server draft in the default central MCP catalog.
///
/// # Errors
/// Returns [`AppError`] if the draft is invalid or saving fails.
pub fn upsert_server(draft: McpServerDraft) -> Result<McpCatalogEntry, AppError> {
    let path = config::mcp_catalog_path()?;
    upsert_server_to(draft, &path, None)
}

/// Validates and stores a server draft into an explicit catalog path.
///
/// # Errors
/// Returns [`AppError`] if validation fails or disk write fails.
pub fn upsert_server_to(
    draft: McpServerDraft,
    catalog_path: &Path,
    custom_backups_root: Option<&Path>,
) -> Result<McpCatalogEntry, AppError> {
    let name = draft.name.trim().to_owned();
    if name.is_empty() || name.contains('/') || name.contains('\\') {
        return Err(AppError::InvalidPath(format!(
            "Invalid MCP server name: '{}'",
            draft.name
        )));
    }

    let command = draft
        .command
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned);
    let url = draft
        .url
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned);

    match draft.transport {
        McpTransport::Stdio if command.is_none() => {
            return Err(AppError::InvalidPath(
                "Stdio MCP servers require a command".to_owned(),
            ));
        }
        McpTransport::Http | McpTransport::Sse if url.is_none() => {
            return Err(AppError::InvalidPath(
                "HTTP/SSE MCP servers require a URL".to_owned(),
            ));
        }
        McpTransport::Unknown => {
            return Err(AppError::InvalidPath(
                "Select Stdio, HTTP, or SSE transport".to_owned(),
            ));
        }
        _ => {}
    }

    let mut catalog = load_catalog_from(catalog_path)?;
    let existing = catalog.servers.get(&name).cloned();

    let entry = McpCatalogEntry {
        name: name.clone(),
        transport: draft.transport,
        command,
        args: draft.args,
        url,
        env: draft.env,
        headers: draft.headers,
        upfront_tokens: existing.as_ref().and_then(|e| e.upfront_tokens),
        tool_count: existing.as_ref().and_then(|e| e.tool_count),
        probed_at_ms: existing.as_ref().and_then(|e| e.probed_at_ms),
    };

    catalog.servers.insert(name, entry.clone());
    save_catalog_to(&catalog, catalog_path, custom_backups_root)?;
    Ok(entry)
}

/// Persists probe metrics (`upfront_tokens`, `tool_count`, `probed_at_ms`) into `mcp-catalog.json`.
///
/// # Errors
/// Returns [`AppError`] if the server is unknown or saving fails.
pub fn record_probe_result(
    name: &str,
    upfront_tokens: usize,
    tool_count: usize,
    probed_at_ms: u64,
) -> Result<(), AppError> {
    let path = config::mcp_catalog_path()?;
    record_probe_result_to(name, upfront_tokens, tool_count, probed_at_ms, &path, None)
}

/// Variant of [`record_probe_result`] taking an explicit catalog path.
///
/// # Errors
/// Returns [`AppError`] if the server is missing or saving fails.
pub fn record_probe_result_to(
    name: &str,
    upfront_tokens: usize,
    tool_count: usize,
    probed_at_ms: u64,
    catalog_path: &Path,
    custom_backups_root: Option<&Path>,
) -> Result<(), AppError> {
    let mut catalog = load_catalog_from(catalog_path)?;
    let entry = catalog.servers.get_mut(name).ok_or_else(|| {
        AppError::InvalidPath(format!("MCP server '{name}' not found in catalog"))
    })?;
    entry.upfront_tokens = Some(upfront_tokens);
    entry.tool_count = Some(tool_count);
    entry.probed_at_ms = Some(probed_at_ms);
    save_catalog_to(&catalog, catalog_path, custom_backups_root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use tempfile::TempDir;

    #[test]
    fn syncs_discovered_without_clobbering_cached_probes_and_upserts_draft() {
        let tmp = TempDir::new().expect("temp dir");
        let cat_path = tmp.path().join("mcp-catalog.json");
        let backups = tmp.path().join("backups");

        let draft = McpServerDraft {
            name: "context7".to_owned(),
            transport: McpTransport::Stdio,
            command: Some("npx".to_owned()),
            args: vec!["-y".to_owned(), "@upstash/context7-mcp".to_owned()],
            url: None,
            env: BTreeMap::from([("API_KEY".to_owned(), "${API_KEY}".to_owned())]),
            headers: BTreeMap::new(),
        };

        upsert_server_to(draft, &cat_path, Some(&backups)).expect("upsert");
        record_probe_result_to(
            "context7",
            1420,
            4,
            1700000000000,
            &cat_path,
            Some(&backups),
        )
        .expect("record probe");

        let mut catalog = load_catalog_from(&cat_path).expect("load catalog");
        let discovered = vec![
            McpCatalogEntry {
                name: "context7".to_owned(),
                transport: McpTransport::Stdio,
                command: Some("npx".to_owned()),
                args: vec!["-y".to_owned(), "@upstash/context7-mcp".to_owned()],
                url: None,
                env: BTreeMap::new(),
                headers: BTreeMap::new(),
                upfront_tokens: None,
                tool_count: None,
                probed_at_ms: None,
            },
            McpCatalogEntry {
                name: "stripe".to_owned(),
                transport: McpTransport::Http,
                command: None,
                args: Vec::new(),
                url: Some("https://mcp.stripe.com".to_owned()),
                env: BTreeMap::new(),
                headers: BTreeMap::new(),
                upfront_tokens: None,
                tool_count: None,
                probed_at_ms: None,
            },
        ];

        assert!(sync_discovered_entries(&mut catalog, &discovered));
        let c7 = catalog.servers.get("context7").expect("context7");
        assert_eq!(c7.upfront_tokens, Some(1420));
        assert_eq!(c7.tool_count, Some(4));
        assert!(catalog.servers.contains_key("stripe"));
    }
}

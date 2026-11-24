//! Ephemeral stdio MCP server schema prober (`initialize` + `tools/list`, 5s timeout).

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value as JsonValue};

use crate::error::AppError;
use crate::mcp_catalog;
use crate::models::{McpCatalogEntry, McpProbeResult, McpTransport};
use crate::tokenizer;

const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

struct ChildGuard(Option<Child>);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Probes a `stdio` MCP server from `mcp-catalog.json`, counts tool schema tokens, and caches the result.
///
/// # Errors
/// Returns [`AppError`] if the server is unknown, non-`stdio`, times out after 5s, or returns invalid JSON-RPC.
pub fn probe_and_cache(server_name: &str) -> Result<McpProbeResult, AppError> {
    let catalog = mcp_catalog::load_catalog()?;
    let entry = catalog.servers.get(server_name).ok_or_else(|| {
        AppError::InvalidPath(format!("MCP server '{server_name}' not found in catalog"))
    })?;

    let result = probe_entry(entry, PROBE_TIMEOUT)?;
    mcp_catalog::record_probe_result(
        &result.name,
        result.upfront_tokens,
        result.tool_count,
        result.probed_at_ms,
    )?;
    Ok(result)
}

/// Probes an explicit [`McpCatalogEntry`] with a custom deadline (used in tests and production).
///
/// # Errors
/// Returns [`AppError`] if transport is not `Stdio`, spawning fails, JSON-RPC errors, or timeout expires.
pub fn probe_entry(entry: &McpCatalogEntry, timeout: Duration) -> Result<McpProbeResult, AppError> {
    if entry.transport != McpTransport::Stdio {
        return Err(AppError::InvalidPath(format!(
            "Only stdio MCP servers can be probed locally ('{}' is {:?})",
            entry.name, entry.transport
        )));
    }

    let command_bin = entry.command.as_deref().ok_or_else(|| {
        AppError::InvalidPath(format!("MCP server '{}' is missing a command", entry.name))
    })?;

    let mut cmd = Command::new(command_bin);
    cmd.args(&entry.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .env("PATH", augmented_path());

    for (k, v) in resolved_env(&entry.env) {
        cmd.env(k, v);
    }

    let child = cmd.spawn().map_err(|e| {
        AppError::Command(format!(
            "Failed to spawn MCP server '{}' ({command_bin}): {e}",
            entry.name
        ))
    })?;

    let mut guard = ChildGuard(Some(child));
    let child_ref = guard
        .0
        .as_mut()
        .ok_or_else(|| AppError::Command("Missing child handle".to_owned()))?;

    let mut stdin = child_ref
        .stdin
        .take()
        .ok_or_else(|| AppError::Command("Failed to open MCP stdin".to_owned()))?;
    let stdout = child_ref
        .stdout
        .take()
        .ok_or_else(|| AppError::Command("Failed to open MCP stdout".to_owned()))?;

    let (tx, rx) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });

    let deadline = Instant::now() + timeout;

    let init_req = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "AgentDock", "version": "0.1.0" }
        }
    });
    send_json_line(&mut stdin, &init_req, &entry.name)?;
    let _init_resp = wait_for_response_id(&rx, 1, deadline, &entry.name)?;

    let initialized_notif = json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    });
    send_json_line(&mut stdin, &initialized_notif, &entry.name)?;

    let tools_req = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {}
    });
    send_json_line(&mut stdin, &tools_req, &entry.name)?;
    let tools_resp = wait_for_response_id(&rx, 2, deadline, &entry.name)?;

    let tools_val = tools_resp
        .get("result")
        .and_then(|r| r.get("tools"))
        .cloned()
        .unwrap_or_else(|| JsonValue::Array(Vec::new()));

    let tool_count = tools_val.as_array().map_or(0, Vec::len);
    let serialized = serde_json::to_string(&tools_val)?;
    let upfront_tokens = tokenizer::count_tokens(&serialized);

    let probed_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    drop(stdin);
    drop(guard);

    Ok(McpProbeResult {
        name: entry.name.clone(),
        upfront_tokens,
        tool_count,
        probed_at_ms,
    })
}

fn send_json_line(stdin: &mut impl Write, val: &JsonValue, name: &str) -> Result<(), AppError> {
    let line = serde_json::to_string(val)?;
    writeln!(stdin, "{line}")
        .and_then(|()| stdin.flush())
        .map_err(|e| AppError::Command(format!("Failed writing JSON-RPC to '{name}': {e}")))
}

fn wait_for_response_id(
    rx: &mpsc::Receiver<String>,
    expected_id: u64,
    deadline: Instant,
    name: &str,
) -> Result<JsonValue, AppError> {
    loop {
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            return Err(AppError::Command(format!(
                "MCP probe timed out after 5s waiting for '{name}'"
            )));
        };

        let line = rx.recv_timeout(remaining).map_err(|_| {
            AppError::Command(format!(
                "MCP server '{name}' closed stdout or timed out before responding"
            ))
        })?;

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(parsed) = serde_json::from_str::<JsonValue>(trimmed) else {
            continue;
        };
        if parsed.get("id").and_then(JsonValue::as_u64) != Some(expected_id) {
            continue;
        }
        if let Some(err_obj) = parsed.get("error") {
            return Err(AppError::Command(format!(
                "MCP server '{name}' returned error: {err_obj}"
            )));
        }
        return Ok(parsed);
    }
}

fn augmented_path() -> String {
    let base = std::env::var("PATH").unwrap_or_default();
    let extra = ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin"];
    let mut parts: Vec<String> = base
        .split(':')
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .collect();
    for dir in extra {
        if !parts.iter().any(|p| p == dir) {
            parts.push(dir.to_owned());
        }
    }
    parts.join(":")
}

fn resolved_env(env_map: &BTreeMap<String, String>) -> Vec<(String, String)> {
    env_map
        .iter()
        .map(|(k, v)| {
            if let Some(inner) = v.strip_prefix("${").and_then(|s| s.strip_suffix('}')) {
                if let Ok(resolved) = std::env::var(inner) {
                    return (k.clone(), resolved);
                }
            }
            (k.clone(), v.clone())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probes_mock_stdio_mcp_server_and_counts_tool_tokens() {
        let script = r#"
import json, sys
for line in sys.stdin:
    req = json.loads(line)
    if req.get("method") == "initialize":
        print(json.dumps({"jsonrpc": "2.0", "id": req["id"], "result": {"protocolVersion": "2024-11-05", "capabilities": {}}}), flush=True)
    elif req.get("method") == "tools/list":
        print(json.dumps({"jsonrpc": "2.0", "id": req["id"], "result": {"tools": [{"name": "search_docs", "description": "Search project notes.", "inputSchema": {"type": "object", "properties": {"query": {"type": "string"}}}}, {"name": "list_tags", "description": "List vault tags.", "inputSchema": {"type": "object"}}]}}), flush=True)
"#;

        let entry = McpCatalogEntry {
            name: "mock-stdio".to_owned(),
            transport: McpTransport::Stdio,
            command: Some("python3".to_owned()),
            args: vec!["-u".to_owned(), "-c".to_owned(), script.to_owned()],
            url: None,
            env: BTreeMap::new(),
            headers: BTreeMap::new(),
            upfront_tokens: None,
            tool_count: None,
            probed_at_ms: None,
        };

        let res = probe_entry(&entry, Duration::from_secs(4)).expect("probe mock server");
        assert_eq!(res.name, "mock-stdio");
        assert_eq!(res.tool_count, 2);
        assert!(res.upfront_tokens > 10);
    }

    #[test]
    fn times_out_cleanly_when_stdio_server_hangs() {
        let entry = McpCatalogEntry {
            name: "hanging-server".to_owned(),
            transport: McpTransport::Stdio,
            command: Some("/bin/sh".to_owned()),
            args: vec!["-c".to_owned(), "sleep 10".to_owned()],
            url: None,
            env: BTreeMap::new(),
            headers: BTreeMap::new(),
            upfront_tokens: None,
            tool_count: None,
            probed_at_ms: None,
        };

        let err = probe_entry(&entry, Duration::from_millis(200))
            .expect_err("hanging server must time out");
        assert!(
            err.to_string().contains("timed out"),
            "unexpected error: {err}"
        );
    }
}

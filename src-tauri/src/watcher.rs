//! Live macOS FSEvents watcher (`notify`) for Library sources, global agent files, and active project.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use notify::{Config as NotifyConfig, Event, RecommendedWatcher, RecursiveMode, Watcher};
use tauri::{AppHandle, Emitter, Manager};

use crate::models::AppConfig;

const DEBOUNCE_WINDOW: Duration = Duration::from_millis(400);

/// Holds the active filesystem watcher handle and currently watched project path.
pub struct WatcherManager {
    _watcher: Option<RecommendedWatcher>,
    watched_project: Option<PathBuf>,
}

impl Default for WatcherManager {
    fn default() -> Self {
        Self::new()
    }
}

impl WatcherManager {
    /// Creates an inactive [`WatcherManager`].
    #[must_use]
    pub const fn new() -> Self {
        Self {
            _watcher: None,
            watched_project: None,
        }
    }

    /// Rebuilds the underlying FSEvents watcher for `cfg`, `home`, and the optional selected project.
    pub fn refresh_watches(
        &mut self,
        app: &AppHandle,
        cfg: &AppConfig,
        home: &Path,
        project_path: Option<PathBuf>,
    ) {
        self.watched_project = project_path.clone();
        self._watcher = start_watcher(app.clone(), cfg, home, project_path.as_deref());
    }

    /// Rebuilds watches using updated `cfg` while preserving the currently watched project.
    pub fn refresh_keeping_project(&mut self, app: &AppHandle, cfg: &AppConfig, home: &Path) {
        let project_path = self.watched_project.clone();
        self._watcher = start_watcher(app.clone(), cfg, home, project_path.as_deref());
    }

    /// Updates only the watched project directory while keeping `cfg` and `home` watches active.
    pub fn set_project(
        &mut self,
        app: &AppHandle,
        cfg: &AppConfig,
        home: &Path,
        project_path: Option<PathBuf>,
    ) {
        if self.watched_project == project_path && self._watcher.is_some() {
            return;
        }
        self.refresh_watches(app, cfg, home, project_path);
    }
}

fn start_watcher(
    app: AppHandle,
    cfg: &AppConfig,
    home: &Path,
    project_path: Option<&Path>,
) -> Option<RecommendedWatcher> {
    let (tx, rx) = mpsc::channel::<PathBuf>();

    let mut watcher = RecommendedWatcher::new(
        move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                for path in event.paths {
                    if is_relevant_change_path(&path) {
                        let _ = tx.send(path);
                        break;
                    }
                }
            }
        },
        NotifyConfig::default(),
    )
    .ok()?;

    let targets = collect_watch_targets(cfg, home, project_path);
    for (path, mode) in targets {
        let _ = watcher.watch(&path, mode);
    }

    std::thread::spawn(move || {
        while rx.recv().is_ok() {
            let deadline = Instant::now() + DEBOUNCE_WINDOW;
            while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
                match rx.recv_timeout(remaining) {
                    Ok(_) => continue,
                    Err(mpsc::RecvTimeoutError::Timeout) => break,
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                }
            }

            if let Some(window) = app.get_webview_window("main") {
                let _ = window.emit("workspace://changed", ());
            }
        }
    });

    Some(watcher)
}

fn collect_watch_targets(
    cfg: &AppConfig,
    home: &Path,
    project_path: Option<&Path>,
) -> Vec<(PathBuf, RecursiveMode)> {
    let mut seen = BTreeSet::new();
    let mut targets = Vec::new();

    let mut add = |p: PathBuf, mode: RecursiveMode| {
        if p.exists() && seen.insert(p.clone()) {
            targets.push((p, mode));
        }
    };

    // Library sources (canonical skills & rules, expanding leading `~/`)
    for src in &cfg.library_sources {
        add(
            crate::config::expand_user_path(home, &src.path),
            RecursiveMode::Recursive,
        );
    }

    // Specific global agent files & folders resolved from custom overrides or home defaults
    let roots = crate::config::resolve_agent_roots(cfg, home);
    for dir in [
        roots.antigravity_dir.join("config/skills"),
        roots.antigravity_dir.join("config/rules"),
        roots.antigravity_dir.join("config/plugins"),
        roots.shared_agents_dir.join("skills"),
        roots.codex_dir.join("skills"),
        roots.codex_dir.join("plugins/cache"),
        roots.claude_dir.join("skills"),
        roots.claude_dir.join("plugins"),
        roots.claude_dir.join("agents"),
        roots.claude_dir.join("commands"),
    ] {
        add(dir, RecursiveMode::Recursive);
    }

    for file in [
        roots.antigravity_dir.join("AGENTS.md"),
        roots.antigravity_dir.join("GEMINI.md"),
        roots.antigravity_dir.join("config/AGENTS.md"),
        roots.antigravity_dir.join("config/config.json"),
        roots.antigravity_dir.join("config/mcp_config.json"),
        roots.antigravity_dir.join("antigravity/mcp_config.json"),
        roots.codex_dir.join("AGENTS.md"),
        roots.codex_dir.join("AGENTS.override.md"),
        roots.codex_dir.join("config.toml"),
        roots.claude_json,
        roots.claude_dir.join("CLAUDE.md"),
        roots.claude_dir.join("settings.json"),
        roots.claude_dir.join("settings.local.json"),
    ] {
        add(file, RecursiveMode::NonRecursive);
    }

    // Active project scope watches
    if let Some(proj) = project_path {
        add(proj.to_path_buf(), RecursiveMode::NonRecursive);
        for sub in [
            proj.join(".agents"),
            proj.join(".agent"),
            proj.join(".codex"),
            proj.join(".claude"),
        ] {
            add(sub, RecursiveMode::Recursive);
        }
        add(proj.join("_support-docs"), RecursiveMode::NonRecursive);
    }

    targets
}

/// Returns `true` only for agent customization files/folders, filtering out noisy temp/log/cache artifacts.
#[must_use]
pub fn is_relevant_change_path(path: &Path) -> bool {
    const IGNORED_COMPONENTS: &[&str] = &[
        ".git",
        "node_modules",
        "target",
        "dist",
        "backups",
        "telemetry",
        "sessions",
        "session-env",
        "cache",
        "file-history",
        "debug",
        "antigravity-browser-profile",
        "attachments",
        "memories",
        "memories_v2",
        ".tmp",
    ];

    for comp in path.components() {
        if let Some(name) = comp.as_os_str().to_str() {
            if IGNORED_COMPONENTS.contains(&name) {
                return false;
            }
        }
    }

    let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };

    if file_name == ".DS_Store"
        || file_name.contains(".agentdock.")
        || file_name.ends_with(".tmp")
        || file_name.ends_with(".bak")
        || file_name.ends_with(".sqlite")
        || file_name.ends_with(".sqlite-wal")
        || file_name.ends_with(".sqlite-shm")
        || file_name.ends_with(".log")
    {
        return false;
    }

    if matches!(
        file_name,
        "SKILL.md"
            | "AGENTS.md"
            | "AGENTS.override.md"
            | "GEMINI.md"
            | "CLAUDE.md"
            | "plugin.json"
            | "mcp_config.json"
            | "config.toml"
            | ".mcp.json"
            | ".claude.json"
            | "settings.json"
            | "settings.local.json"
            | "installed_plugins.json"
    ) {
        return true;
    }

    if file_name == "config.json"
        && path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            == Some("config")
    {
        return true;
    }

    if file_name.ends_with(".md") && !file_name.starts_with('.') {
        return true;
    }

    // Folder additions/removals inside skills, rules, plugins, or _support-docs
    path.parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .is_some_and(|parent| matches!(parent, "skills" | "rules" | "plugins" | "_support-docs"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_relevant_customization_paths_and_ignores_noise() {
        assert!(is_relevant_change_path(Path::new(
            "/Users/peter/BRAIN-LOCAL/09-ai-os/skills/peter-writing/SKILL.md"
        )));
        assert!(is_relevant_change_path(Path::new(
            "/Users/peter/.gemini/config/mcp_config.json"
        )));
        assert!(is_relevant_change_path(Path::new(
            "/Users/peter/.codex/config.toml"
        )));
        assert!(is_relevant_change_path(Path::new(
            "/Users/peter/proj/.mcp.json"
        )));
        assert!(is_relevant_change_path(Path::new(
            "/Users/peter/proj/.agents/rules/style.md"
        )));
        assert!(is_relevant_change_path(Path::new(
            "/Users/peter/.gemini/config/config.json"
        )));
        assert!(is_relevant_change_path(Path::new(
            "/Users/peter/.gemini/config/plugins/vercel-plugin/plugin.json"
        )));
        assert!(is_relevant_change_path(Path::new(
            "/Users/peter/.gemini/config/plugins/vercel-plugin"
        )));
        assert!(is_relevant_change_path(Path::new(
            "/Users/peter/.claude/settings.json"
        )));

        assert!(!is_relevant_change_path(Path::new(
            "/Users/peter/.codex/logs_2.sqlite-wal"
        )));
        assert!(!is_relevant_change_path(Path::new(
            "/Users/peter/proj/.DS_Store"
        )));
        assert!(!is_relevant_change_path(Path::new(
            "/Users/peter/proj/node_modules/pkg/README.md"
        )));
        assert!(!is_relevant_change_path(Path::new(
            "/Users/peter/proj/.agents/mcp_config.json.agentdock.123.tmp"
        )));
    }
}

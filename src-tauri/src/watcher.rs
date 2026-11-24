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

    // Library sources (Obsidian vault skills/rules)
    for src in &cfg.library_sources {
        add(PathBuf::from(&src.path), RecursiveMode::Recursive);
    }

    // Specific global agent files & folders (avoiding recursive watches on noisy ~/.codex or ~/.claude roots)
    for dir in [
        home.join(".gemini/config/skills"),
        home.join(".gemini/config/rules"),
        home.join(".agents/skills"),
        home.join(".claude/skills"),
    ] {
        add(dir, RecursiveMode::Recursive);
    }

    for file in [
        home.join(".gemini/AGENTS.md"),
        home.join(".gemini/GEMINI.md"),
        home.join(".gemini/config/AGENTS.md"),
        home.join(".gemini/config/mcp_config.json"),
        home.join(".gemini/antigravity/mcp_config.json"),
        home.join(".codex/AGENTS.md"),
        home.join(".codex/AGENTS.override.md"),
        home.join(".codex/config.toml"),
        home.join(".claude.json"),
        home.join(".claude/CLAUDE.md"),
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
            | "mcp_config.json"
            | "config.toml"
            | ".mcp.json"
            | ".claude.json"
    ) {
        return true;
    }

    if file_name.ends_with(".md") && !file_name.starts_with('.') {
        return true;
    }

    // Folder additions/removals inside skills, rules, or _support-docs
    path.parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .is_some_and(|parent| matches!(parent, "skills" | "rules" | "_support-docs"))
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

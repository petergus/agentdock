//! Agent filesystem path adapters isolating Antigravity, Codex, and Claude layouts.

pub mod antigravity;
pub mod claude;
pub mod codex;

use std::path::{Path, PathBuf};

use crate::models::{ActiveAgents, AgentKind};

/// Path descriptor specifying whether a scanned location is primary or legacy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathSpec {
    pub path: PathBuf,
    pub is_legacy: bool,
}

impl PathSpec {
    #[must_use]
    pub fn primary(path: PathBuf) -> Self {
        Self {
            path,
            is_legacy: false,
        }
    }

    #[must_use]
    pub fn legacy(path: PathBuf) -> Self {
        Self {
            path,
            is_legacy: true,
        }
    }
}

/// Abstraction over each AI coding agent's global and project filesystem conventions.
pub trait AgentAdapter: Send + Sync {
    /// Returns the agent variant implemented by this adapter.
    fn kind(&self) -> AgentKind;

    /// Global single-file rule files (`AGENTS.md`, `GEMINI.md`, `CLAUDE.md`).
    fn global_rule_files(&self, home: &Path) -> Vec<PathSpec>;

    /// Global rule directories scanned non-recursively for `*.md` files.
    fn global_rule_dirs(&self, home: &Path) -> Vec<PathSpec>;

    /// Global skill parent directories holding `<skill-name>/SKILL.md`.
    fn global_skill_dirs(&self, home: &Path) -> Vec<PathSpec>;

    /// Global MCP configuration files (JSON or TOML).
    fn global_mcp_configs(&self, home: &Path) -> Vec<PathSpec>;

    /// Project-scoped single-file rule files relative to `project`.
    fn project_rule_files(&self, project: &Path) -> Vec<PathSpec>;

    /// Project-scoped rule directories relative to `project`.
    fn project_rule_dirs(&self, project: &Path) -> Vec<PathSpec>;

    /// Project-scoped skill directories relative to `project`.
    fn project_skill_dirs(&self, project: &Path) -> Vec<PathSpec>;

    /// Project-scoped MCP configuration files relative to `project`.
    fn project_mcp_configs(&self, project: &Path) -> Vec<PathSpec>;
}

/// Returns adapters enabled by the user's configuration in deterministic order.
#[must_use]
pub fn enabled_adapters(active: ActiveAgents) -> Vec<Box<dyn AgentAdapter>> {
    let mut adapters: Vec<Box<dyn AgentAdapter>> = Vec::new();
    if active.antigravity {
        adapters.push(Box::new(antigravity::AntigravityAdapter));
    }
    if active.codex {
        adapters.push(Box::new(codex::CodexAdapter));
    }
    if active.claude {
        adapters.push(Box::new(claude::ClaudeAdapter));
    }
    adapters
}

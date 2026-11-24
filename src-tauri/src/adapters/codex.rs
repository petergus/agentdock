//! OpenAI Codex filesystem adapter.

use std::path::Path;

use crate::adapters::{AgentAdapter, PathSpec};
use crate::models::AgentKind;

/// Adapter for OpenAI Codex CLI and Desktop paths.
pub struct CodexAdapter;

impl AgentAdapter for CodexAdapter {
    fn kind(&self) -> AgentKind {
        AgentKind::Codex
    }

    fn global_rule_files(&self, home: &Path) -> Vec<PathSpec> {
        vec![
            PathSpec::primary(home.join(".codex/AGENTS.md")),
            PathSpec::primary(home.join(".codex/AGENTS.override.md")),
        ]
    }

    fn global_rule_dirs(&self, _home: &Path) -> Vec<PathSpec> {
        Vec::new()
    }

    fn global_skill_dirs(&self, home: &Path) -> Vec<PathSpec> {
        vec![PathSpec::primary(home.join(".agents/skills"))]
    }

    fn global_mcp_configs(&self, home: &Path) -> Vec<PathSpec> {
        vec![PathSpec::primary(home.join(".codex/config.toml"))]
    }

    fn project_rule_files(&self, project: &Path) -> Vec<PathSpec> {
        vec![PathSpec::primary(project.join("AGENTS.md"))]
    }

    fn project_rule_dirs(&self, _project: &Path) -> Vec<PathSpec> {
        Vec::new()
    }

    fn project_skill_dirs(&self, project: &Path) -> Vec<PathSpec> {
        vec![PathSpec::primary(project.join(".agents/skills"))]
    }

    fn project_mcp_configs(&self, project: &Path) -> Vec<PathSpec> {
        vec![PathSpec::primary(project.join(".codex/config.toml"))]
    }
}

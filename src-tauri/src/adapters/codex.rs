//! OpenAI Codex filesystem adapter.

use std::path::Path;

use crate::adapters::{AgentAdapter, PathSpec};
use crate::config::AgentRoots;
use crate::models::AgentKind;

/// Adapter for OpenAI Codex CLI and Desktop paths.
pub struct CodexAdapter;

impl AgentAdapter for CodexAdapter {
    fn kind(&self) -> AgentKind {
        AgentKind::Codex
    }

    fn global_rule_files(&self, roots: &AgentRoots) -> Vec<PathSpec> {
        let base = &roots.codex_dir;
        vec![
            PathSpec::primary(base.join("AGENTS.md")),
            PathSpec::primary(base.join("AGENTS.override.md")),
        ]
    }

    fn global_rule_dirs(&self, _roots: &AgentRoots) -> Vec<PathSpec> {
        Vec::new()
    }

    fn global_skill_dirs(&self, roots: &AgentRoots) -> Vec<PathSpec> {
        vec![
            PathSpec::primary(roots.shared_agents_dir.join("skills")),
            PathSpec::legacy(roots.codex_dir.join("skills")),
        ]
    }

    fn global_mcp_configs(&self, roots: &AgentRoots) -> Vec<PathSpec> {
        vec![PathSpec::primary(roots.codex_dir.join("config.toml"))]
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

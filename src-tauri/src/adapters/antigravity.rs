//! Google Antigravity filesystem adapter (primary + legacy/CLI paths).

use std::path::Path;

use crate::adapters::{AgentAdapter, PathSpec};
use crate::config::AgentRoots;
use crate::models::AgentKind;

/// Adapter for Google Antigravity App and CLI paths.
pub struct AntigravityAdapter;

impl AgentAdapter for AntigravityAdapter {
    fn kind(&self) -> AgentKind {
        AgentKind::Antigravity
    }

    fn global_rule_files(&self, roots: &AgentRoots) -> Vec<PathSpec> {
        let base = &roots.antigravity_dir;
        vec![
            PathSpec::primary(base.join("AGENTS.md")),
            PathSpec::primary(base.join("GEMINI.md")),
            PathSpec::primary(base.join("config/AGENTS.md")),
        ]
    }

    fn global_rule_dirs(&self, roots: &AgentRoots) -> Vec<PathSpec> {
        vec![PathSpec::primary(
            roots.antigravity_dir.join("config/rules"),
        )]
    }

    fn global_skill_dirs(&self, roots: &AgentRoots) -> Vec<PathSpec> {
        let base = &roots.antigravity_dir;
        vec![
            PathSpec::primary(base.join("config/skills")),
            PathSpec::legacy(base.join("antigravity/skills")),
            PathSpec::legacy(base.join("antigravity-cli/skills")),
            PathSpec::legacy(base.join("skills")),
            PathSpec::legacy(roots.shared_agents_dir.join("skills")),
        ]
    }

    fn global_mcp_configs(&self, roots: &AgentRoots) -> Vec<PathSpec> {
        let base = &roots.antigravity_dir;
        vec![
            PathSpec::primary(base.join("config/mcp_config.json")),
            PathSpec::legacy(base.join("antigravity/mcp_config.json")),
        ]
    }

    fn project_rule_files(&self, project: &Path) -> Vec<PathSpec> {
        vec![
            PathSpec::primary(project.join("AGENTS.md")),
            PathSpec::primary(project.join("GEMINI.md")),
        ]
    }

    fn project_rule_dirs(&self, project: &Path) -> Vec<PathSpec> {
        vec![
            PathSpec::primary(project.join(".agents/rules")),
            PathSpec::legacy(project.join(".agent/rules")),
        ]
    }

    fn project_skill_dirs(&self, project: &Path) -> Vec<PathSpec> {
        vec![
            PathSpec::primary(project.join(".agents/skills")),
            PathSpec::legacy(project.join(".agent/skills")),
        ]
    }

    fn project_mcp_configs(&self, project: &Path) -> Vec<PathSpec> {
        vec![PathSpec::primary(project.join(".agents/mcp_config.json"))]
    }
}

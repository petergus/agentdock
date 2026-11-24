//! Anthropic Claude Code filesystem adapter.

use std::path::Path;

use crate::adapters::{AgentAdapter, PathSpec};
use crate::models::AgentKind;

/// Adapter for Claude Code global and project paths.
pub struct ClaudeAdapter;

impl AgentAdapter for ClaudeAdapter {
    fn kind(&self) -> AgentKind {
        AgentKind::Claude
    }

    fn global_rule_files(&self, home: &Path) -> Vec<PathSpec> {
        vec![PathSpec::primary(home.join(".claude/CLAUDE.md"))]
    }

    fn global_rule_dirs(&self, _home: &Path) -> Vec<PathSpec> {
        Vec::new()
    }

    fn global_skill_dirs(&self, home: &Path) -> Vec<PathSpec> {
        vec![PathSpec::primary(home.join(".claude/skills"))]
    }

    fn global_mcp_configs(&self, home: &Path) -> Vec<PathSpec> {
        vec![PathSpec::primary(home.join(".claude.json"))]
    }

    fn project_rule_files(&self, project: &Path) -> Vec<PathSpec> {
        vec![PathSpec::primary(project.join("CLAUDE.md"))]
    }

    fn project_rule_dirs(&self, project: &Path) -> Vec<PathSpec> {
        vec![PathSpec::primary(project.join(".claude/rules"))]
    }

    fn project_skill_dirs(&self, project: &Path) -> Vec<PathSpec> {
        vec![PathSpec::primary(project.join(".claude/skills"))]
    }

    fn project_mcp_configs(&self, project: &Path) -> Vec<PathSpec> {
        vec![PathSpec::primary(project.join(".mcp.json"))]
    }
}

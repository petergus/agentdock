//! Google Antigravity filesystem adapter (primary + legacy/CLI paths).

use std::path::Path;

use crate::adapters::{AgentAdapter, PathSpec};
use crate::models::AgentKind;

/// Adapter for Google Antigravity App and CLI paths.
pub struct AntigravityAdapter;

impl AgentAdapter for AntigravityAdapter {
    fn kind(&self) -> AgentKind {
        AgentKind::Antigravity
    }

    fn global_rule_files(&self, home: &Path) -> Vec<PathSpec> {
        vec![
            PathSpec::primary(home.join(".gemini/AGENTS.md")),
            PathSpec::primary(home.join(".gemini/GEMINI.md")),
            PathSpec::primary(home.join(".gemini/config/AGENTS.md")),
        ]
    }

    fn global_rule_dirs(&self, home: &Path) -> Vec<PathSpec> {
        vec![PathSpec::primary(home.join(".gemini/config/rules"))]
    }

    fn global_skill_dirs(&self, home: &Path) -> Vec<PathSpec> {
        vec![
            PathSpec::primary(home.join(".gemini/config/skills")),
            PathSpec::legacy(home.join(".gemini/antigravity/skills")),
            PathSpec::legacy(home.join(".gemini/antigravity-cli/skills")),
            PathSpec::legacy(home.join(".gemini/skills")),
            PathSpec::legacy(home.join(".agents/skills")),
        ]
    }

    fn global_mcp_configs(&self, home: &Path) -> Vec<PathSpec> {
        vec![
            PathSpec::primary(home.join(".gemini/config/mcp_config.json")),
            PathSpec::legacy(home.join(".gemini/antigravity/mcp_config.json")),
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

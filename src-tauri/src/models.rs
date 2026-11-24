//! Domain models shared between the Rust scanner backend and React frontend.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Antigravity startup prompt token budget cap per bucket (Rules, Skills & MCP).
pub const TOKEN_BUDGET_LIMIT: usize = 20_000;

/// Structural prompt wrapper tokens added per upfront skill entry.
pub const SKILL_UPFRONT_OVERHEAD_TOKENS: usize = 12;

/// Fallback estimated token count for an unprobed MCP server.
pub const DEFAULT_MCP_ESTIMATE_TOKENS: usize = 800;

/// Character threshold above which a skill description triggers an amber length warning.
pub const DESCRIPTION_WARNING_CHARS: usize = 200;

/// Supported AI coding agents managed by AgentDock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AgentKind {
    Antigravity,
    Codex,
    Claude,
}

/// User-configured toggle state indicating which coding agents are active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveAgents {
    pub antigravity: bool,
    pub codex: bool,
    pub claude: bool,
}

impl Default for ActiveAgents {
    fn default() -> Self {
        Self {
            antigravity: true,
            codex: true,
            claude: false,
        }
    }
}

impl ActiveAgents {
    /// Returns true if the specified agent is enabled in settings.
    #[must_use]
    pub const fn is_enabled(self, agent: AgentKind) -> bool {
        match agent {
            AgentKind::Antigravity => self.antigravity,
            AgentKind::Codex => self.codex,
            AgentKind::Claude => self.claude,
        }
    }
}

/// A canonical library root (e.g. an Obsidian vault subfolder) holding skills and rules.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySource {
    pub id: String,
    pub label: String,
    pub path: String,
}

/// Persisted application configuration stored in Application Support.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub projects_root: Option<String>,
    pub library_sources: Vec<LibrarySource>,
    pub support_docs_root: Option<String>,
    pub pinned_projects: Vec<String>,
    pub ignore_folders: Vec<String>,
    pub active_agents: ActiveAgents,
    pub git_hygiene: bool,
}

/// Origin classification for a discovered skill directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SkillSource {
    Library { label: String, path: String },
    GlobalOnly,
    ProjectOnly,
    BrokenSymlink { target: String },
}

/// Filesystem installation instance of a skill or rule inside an agent directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledPathInfo {
    pub agent: AgentKind,
    pub path: String,
    pub is_symlink: bool,
    pub is_legacy: bool,
    pub broken: bool,
}

/// Scanned skill entry presented in the Switchboard Skills section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillItem {
    pub id: String,
    pub name: String,
    pub description: String,
    pub description_too_long: bool,
    pub upfront_tokens: usize,
    pub on_demand_tokens: usize,
    pub source: SkillSource,
    pub canonical_path: String,
    pub skill_dir_path: String,
    pub is_symlink: bool,
    pub is_unmanaged_global: bool,
    pub active_globally: bool,
    pub active_in_project: bool,
    pub active_agents: Vec<AgentKind>,
    pub installed_paths: Vec<InstalledPathInfo>,
    pub frontmatter_error: Option<String>,
}

/// Transport protocol used by an MCP server entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum McpTransport {
    Stdio,
    Http,
    Sse,
    Unknown,
}

/// Scanned MCP server entry across agent configuration files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerItem {
    pub name: String,
    pub transport: McpTransport,
    pub command_or_url: String,
    pub args: Vec<String>,
    pub upfront_tokens: usize,
    pub is_estimated: bool,
    pub tool_count: Option<usize>,
    pub probed_at_ms: Option<u64>,
    pub active_globally: bool,
    pub active_in_project: bool,
    pub defined_globally_collision: bool,
    pub active_agents: Vec<AgentKind>,
    pub source_configs: Vec<String>,
}

/// Activation mode governing when a rule contributes to startup system prompt tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RuleTrigger {
    AlwaysOn,
    ModelDecision,
    Glob,
    Invalid,
}

/// Scanned rule markdown file (`AGENTS.md`, `GEMINI.md`, `CLAUDE.md`, or `.agents/rules/*.md`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleItem {
    pub id: String,
    pub filename: String,
    pub path: String,
    pub canonical_path: String,
    pub trigger: RuleTrigger,
    pub description: Option<String>,
    pub glob_pattern: Option<String>,
    pub upfront_tokens: usize,
    pub full_tokens: usize,
    pub source_label: String,
    pub is_symlink: bool,
    pub is_library_rule: bool,
    pub active_globally: bool,
    pub active_in_project: bool,
    pub active_agents: Vec<AgentKind>,
    pub frontmatter_error: Option<String>,
    pub is_editable_trigger: bool,
}

/// Mode by which external support documentation is attached to a project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SupportDocMode {
    Pointer,
    Symlink,
}

/// Attached support doc reference discovered inside a project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupportDocItem {
    pub name: String,
    pub path: String,
    pub description: Option<String>,
    pub mode: SupportDocMode,
    pub exists: bool,
}

/// Token budget breakdown for the active scope (Rules vs. Skills & MCP).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenBudgetBreakdown {
    pub rules_global: usize,
    pub rules_project: usize,
    pub rules_total: usize,
    pub rules_limit: usize,
    pub skills_global: usize,
    pub skills_project: usize,
    pub mcp_global: usize,
    pub mcp_project: usize,
    pub skills_mcp_global: usize,
    pub skills_mcp_project: usize,
    pub skills_mcp_total: usize,
    pub skills_mcp_limit: usize,
    pub effective_total: usize,
}

/// Compact project entry displayed inside the sidebar category tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub id: String,
    pub name: String,
    pub category: String,
    pub path: String,
    pub is_pinned: bool,
    pub has_git: bool,
    pub effective_tokens: usize,
    pub project_extra_tokens: usize,
    pub active_project_skills_count: usize,
    pub active_project_mcp_count: usize,
    pub active_project_rules_count: usize,
}

/// First-level folder category grouping projects in the sidebar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryGroup {
    pub name: String,
    pub projects: Vec<ProjectSummary>,
}

/// Distinguishes Global scope from a specific Project scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ScopeType {
    Global,
    Project,
}

/// Complete switchboard payload for either Global or a selected Project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopeSnapshot {
    pub scope_type: ScopeType,
    pub scope_id: String,
    pub title: String,
    pub path: Option<String>,
    pub budget: TokenBudgetBreakdown,
    pub skills: Vec<SkillItem>,
    pub unmanaged_global_skills_count: usize,
    pub mcp_servers: Vec<McpServerItem>,
    pub rules: Vec<RuleItem>,
    pub support_docs: Vec<SupportDocItem>,
    pub scan_warnings: Vec<String>,
}

/// Top-level workspace payload loaded on launch and refresh.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceOverview {
    pub config: AppConfig,
    pub global_tokens: usize,
    pub global_snapshot: ScopeSnapshot,
    pub categories: Vec<CategoryGroup>,
    pub last_scanned_ms: u64,
}

/// Target scope for Milestone 2 mutation commands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ToggleScope {
    Global,
    Project {
        #[serde(rename = "projectPath")]
        project_path: String,
    },
}

/// Desired activation state for a symlink switchboard toggle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesiredState {
    Enable,
    Disable,
}

/// Result returned when toggling a skill or library rule ON/OFF.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum ToggleOutcome {
    Updated {
        #[serde(rename = "changedPaths")]
        changed_paths: Vec<String>,
    },
    RequiresMoveToLibrary {
        #[serde(rename = "skillId")]
        skill_id: String,
        #[serde(rename = "realPaths")]
        real_paths: Vec<String>,
    },
}

/// Post-move behavior when migrating unmanaged real skill folders to a Library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AfterMoveAction {
    Unlink,
    Relink,
}

/// Dry-run preview entry for moving a real skill directory into a canonical Library.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MovePreviewItem {
    pub skill_id: String,
    pub skill_name: String,
    pub source_real_dir: String,
    pub destination_dir: String,
    pub symlinks_to_update: Vec<String>,
    pub conflict_exists: bool,
    pub warning: Option<String>,
}

/// Execution summary returned after migrating unmanaged skill folders to a Library.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveExecutionResult {
    pub moved: Vec<String>,
    pub skipped: Vec<String>,
    pub warnings: Vec<String>,
}

/// Frontmatter trigger update payload for `.agents/rules/*.md` or library rules.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "trigger", rename_all = "camelCase")]
pub enum RuleTriggerUpdate {
    AlwaysOn,
    ModelDecision { description: String },
    Glob { pattern: String },
}

/// Target markdown file for zero-token Support Docs pointer blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SupportDocTargetFile {
    AgentsMd,
    SupportDocsRule,
}

/// Request payload for attaching an external file or folder to a project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkSupportDocRequest {
    pub project_path: String,
    pub source_path: String,
    pub mode: SupportDocMode,
    pub description: Option<String>,
    pub target_file: Option<SupportDocTargetFile>,
}

/// Persisted central MCP server definition + cached schema probe token counts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpCatalogEntry {
    pub name: String,
    pub transport: McpTransport,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upfront_tokens: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub probed_at_ms: Option<u64>,
}

/// Central MCP catalog persisted at `~/Library/Application Support/AgentDock/mcp-catalog.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpCatalog {
    #[serde(default)]
    pub servers: BTreeMap<String, McpCatalogEntry>,
}

/// Draft payload submitted from the `+ Add MCP` sheet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerDraft {
    pub name: String,
    pub transport: McpTransport,
    pub command: Option<String>,
    pub args: Vec<String>,
    pub url: Option<String>,
    pub env: BTreeMap<String, String>,
    pub headers: BTreeMap<String, String>,
}

/// Result returned after probing a stdio MCP server's `tools/list` schema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpProbeResult {
    pub name: String,
    pub upfront_tokens: usize,
    pub tool_count: usize,
    pub probed_at_ms: u64,
}

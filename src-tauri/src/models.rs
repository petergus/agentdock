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

/// Default directory depth for project discovery under a root folder (`category/project`).
pub const DEFAULT_PROJECT_SCAN_DEPTH: u8 = 2;

/// Default recursive folder depth for library skill/rule discovery.
pub const DEFAULT_LIBRARY_SCAN_DEPTH: u8 = 2;

/// Default max folder depth when counting markdown files inside `<skill>/references/`.
pub const DEFAULT_REFERENCE_SCAN_DEPTH: u8 = 3;

const fn default_project_scan_depth() -> u8 {
    DEFAULT_PROJECT_SCAN_DEPTH
}

const fn default_library_scan_depth() -> u8 {
    DEFAULT_LIBRARY_SCAN_DEPTH
}

const fn default_reference_scan_depth() -> u8 {
    DEFAULT_REFERENCE_SCAN_DEPTH
}

const fn default_true() -> bool {
    true
}

fn default_ignore_folders() -> Vec<String> {
    vec![
        ".git".to_owned(),
        "node_modules".to_owned(),
        "_archive".to_owned(),
        "Archive".to_owned(),
        ".DS_Store".to_owned(),
    ]
}

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

/// How a configured project root discovers repositories underneath it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProjectScanMode {
    /// Discover every non-ignored folder at the exact `depth` below the root.
    #[default]
    ExactDepth,
    /// Descend up to `depth` levels and stop descending once project markers are found.
    AutoDetect,
}

/// Configured directory root scanned for project workspaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRoot {
    pub id: String,
    #[serde(default)]
    pub label: String,
    pub path: String,
    #[serde(default = "default_project_scan_depth")]
    pub depth: u8,
    #[serde(default)]
    pub mode: ProjectScanMode,
}

/// Optional custom global config directory overrides per coding agent.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentPaths {
    #[serde(default)]
    pub antigravity_dir: Option<String>,
    #[serde(default)]
    pub codex_dir: Option<String>,
    #[serde(default)]
    pub claude_dir: Option<String>,
}

/// A canonical library root holding reusable skills and rules.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySource {
    pub id: String,
    pub label: String,
    pub path: String,
    #[serde(default = "default_library_scan_depth")]
    pub depth: u8,
}

/// Reference to a globally paused skill so it can be restored accurately.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PausedSkillRef {
    pub skill_id: String,
    pub canonical_skill_dir: String,
}

/// Reference to a globally paused library rule so it can be restored accurately.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PausedRuleRef {
    pub rule_filename: String,
    pub canonical_rule_file: String,
}

/// Reference to a globally paused plugin so it can be restored accurately.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PausedPluginRef {
    pub agent: AgentKind,
    pub plugin_id: String,
}

/// Persisted pause/restore snapshot for global quick switches.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalPauseState {
    #[serde(default)]
    pub skills_paused: bool,
    #[serde(default)]
    pub paused_skills: Vec<PausedSkillRef>,
    #[serde(default)]
    pub plugins_paused: bool,
    #[serde(default)]
    pub paused_plugins: Vec<PausedPluginRef>,
    #[serde(default)]
    pub mcp_paused: bool,
    #[serde(default)]
    pub paused_mcp: Vec<String>,
    #[serde(default)]
    pub rules_paused: bool,
    #[serde(default)]
    pub paused_rules: Vec<PausedRuleRef>,
}

/// Target bucket for global pause/restore switches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GlobalSwitchTarget {
    All,
    Skills,
    Plugins,
    Mcp,
}

/// Summary returned after toggling a global quick switch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchToggleResult {
    pub changed_count: usize,
    pub skipped_unmanaged: Vec<String>,
    pub warnings: Vec<String>,
    pub pause_state: GlobalPauseState,
}

/// Reusable bundle of skills, MCP servers, and library rules toggled per project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextSet {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub skill_ids: Vec<String>,
    #[serde(default)]
    pub mcp_names: Vec<String>,
    #[serde(default)]
    pub rule_filenames: Vec<String>,
}

/// Outcome returned when toggling a context set on or off for a project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetToggleResult {
    pub applied_count: usize,
    pub skipped_shared_count: usize,
    pub warnings: Vec<String>,
    pub active_set_ids: Vec<String>,
}

/// Finder-style tag color options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TagColor {
    #[default]
    Gray,
    Blue,
    Green,
    Orange,
    Red,
    Purple,
    Yellow,
}

/// Persisted tag definition (either a manual custom tag or an ancestor auto-tag rule).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagDefinition {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub color: TagColor,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_folder: Option<String>,
    #[serde(default)]
    pub hidden: bool,
}

/// Resolved tag attached to a scanned project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTag {
    pub id: String,
    pub label: String,
    pub color: TagColor,
    pub is_auto: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_folder: Option<String>,
}

/// Persisted application configuration stored in Application Support.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub projects_root: Option<String>,
    #[serde(default)]
    pub project_roots: Vec<ProjectRoot>,
    #[serde(default)]
    pub library_sources: Vec<LibrarySource>,
    #[serde(default)]
    pub support_docs_root: Option<String>,
    #[serde(default)]
    pub pinned_projects: Vec<String>,
    #[serde(default = "default_ignore_folders")]
    pub ignore_folders: Vec<String>,
    #[serde(default = "default_reference_scan_depth")]
    pub reference_scan_depth: u8,
    #[serde(default)]
    pub agent_paths: AgentPaths,
    #[serde(default)]
    pub active_agents: ActiveAgents,
    #[serde(default = "default_true")]
    pub git_hygiene: bool,
    #[serde(default)]
    pub global_pause: GlobalPauseState,
    #[serde(default)]
    pub sets: Vec<ContextSet>,
    #[serde(default)]
    pub project_active_sets: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub hidden_projects: Vec<String>,
    #[serde(default)]
    pub tag_definitions: Vec<TagDefinition>,
    #[serde(default)]
    pub project_manual_tags: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub project_muted_auto_tags: BTreeMap<String, Vec<String>>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            projects_root: None,
            project_roots: Vec::new(),
            library_sources: Vec::new(),
            support_docs_root: None,
            pinned_projects: Vec::new(),
            ignore_folders: default_ignore_folders(),
            reference_scan_depth: DEFAULT_REFERENCE_SCAN_DEPTH,
            agent_paths: AgentPaths::default(),
            active_agents: ActiveAgents::default(),
            git_hygiene: true,
            global_pause: GlobalPauseState::default(),
            sets: Vec::new(),
            project_active_sets: BTreeMap::new(),
            hidden_projects: Vec::new(),
            tag_definitions: Vec::new(),
            project_manual_tags: BTreeMap::new(),
            project_muted_auto_tags: BTreeMap::new(),
        }
    }
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
    pub scope: ScopeType,
    pub target: String,
    pub upfront_tokens: Option<usize>,
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

/// One agent's definition of an MCP server in a particular scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpDefinition {
    pub agent: AgentKind,
    pub scope: ScopeType,
    pub path: String,
    pub enabled: bool,
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
    pub definitions: Vec<McpDefinition>,
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

/// Rule discovery and trigger information for a specific agent and installation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleInstallation {
    pub agent: AgentKind,
    pub scope: ScopeType,
    pub path: String,
    pub trigger: RuleTrigger,
    pub glob_pattern: Option<String>,
    pub upfront_tokens: usize,
    pub is_symlink: bool,
    pub error: Option<String>,
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
    pub installations: Vec<RuleInstallation>,
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

/// Summary of a skill bundled inside an agent plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSkillInfo {
    pub name: String,
    pub upfront_tokens: usize,
}

/// Installed Antigravity, Codex, or Claude plugin/customization bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginItem {
    pub agent: AgentKind,
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub version: Option<String>,
    pub source: Option<String>,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manage_hint: Option<String>,
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub can_toggle: bool,
    pub skills: Vec<PluginSkillInfo>,
    pub agent_names: Vec<String>,
    pub mcp_names: Vec<String>,
    pub rule_names: Vec<String>,
    pub skills_upfront_tokens: usize,
    pub agents_upfront_tokens: usize,
    pub rules_upfront_tokens: usize,
    pub upfront_tokens: usize,
}

/// Token budget breakdown for the active scope (Rules vs. Skills, Plugins & MCP).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenBudgetBreakdown {
    pub rules_global: usize,
    pub rules_project: usize,
    pub rules_total: usize,
    pub rules_limit: usize,
    pub skills_global: usize,
    pub skills_project: usize,
    pub plugins_global: usize,
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
    pub is_hidden: bool,
    pub has_git: bool,
    pub ancestor_folders: Vec<String>,
    pub tags: Vec<ProjectTag>,
    pub active_set_ids: Vec<String>,
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
    pub plugins: Vec<PluginItem>,
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
    pub discovered_auto_folders: Vec<String>,
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

/// Broken symlink discovered in Global or Project agent directories.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrokenSymlinkIssue {
    pub path: String,
    pub target: String,
    pub scope_label: String,
    pub project_path: Option<String>,
    pub agent: Option<AgentKind>,
    pub item_kind: String,
    pub original_path: Option<String>,
    pub original_location: Option<String>,
}

/// Loose real skill folder discovered directly inside an agent or project folder instead of a Library Source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StraySkillIssue {
    pub skill_id: String,
    pub skill_name: String,
    pub path: String,
    pub skill_md_path: String,
    pub scope_label: String,
    pub project_path: Option<String>,
    pub agents: Vec<AgentKind>,
    pub upfront_tokens: usize,
    pub exists_in_library: bool,
}

/// Standalone skill duplicating a skill already active inside an enabled plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateSkillIssue {
    pub skill_name: String,
    pub skill_id: String,
    pub standalone_path: String,
    pub scope_label: String,
    pub project_path: Option<String>,
    pub plugin_agent: AgentKind,
    pub plugin_id: String,
    pub plugin_name: String,
    pub duplicate_tokens: usize,
    pub is_symlink: bool,
}

/// Unreferenced or superseded cached plugin directory under `~/.codex/plugins/cache/`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StalePluginCacheItem {
    pub marketplace: String,
    pub plugin_slug: String,
    pub version: String,
    pub path: String,
    pub reason: String,
    pub size_bytes: u64,
}

/// Project `.git/info/exclude` entries managed by AgentDock whose target paths no longer exist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StaleGitExcludeIssue {
    pub project_name: String,
    pub project_path: String,
    pub entries: Vec<String>,
}

/// Synced Claude skill bucket directory under `~/.claude/skills/synced/`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeSyncedBucketItem {
    pub name: String,
    pub path: String,
    pub size_bytes: u64,
    pub entry_count: usize,
}

/// Category of global instruction or rule hygiene issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstructionHygieneKind {
    Frontmatter,
    Oversized,
    PausedSkillRef,
}

/// Hygiene warning for global instruction files (`AGENTS.md`, `GEMINI.md`, `CLAUDE.md`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstructionHygieneIssue {
    pub path: String,
    pub filename: String,
    pub agent: AgentKind,
    pub kind: InstructionHygieneKind,
    pub severity: String,
    pub message: String,
    pub tokens: usize,
}

/// Preview of a suggested template or repair for an instruction/rule hygiene issue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstructionFixPreview {
    pub path: String,
    pub canonical_path: String,
    pub filename: String,
    pub kind: InstructionHygieneKind,
    pub title: String,
    pub why: String,
    pub what_changes: String,
    pub current_content: String,
    pub current_tokens: usize,
    pub suggested_content: String,
    pub suggested_tokens: usize,
}

/// Result returned after approving and writing an instruction/rule fix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstructionFixApplyResult {
    pub path: String,
    pub backup_path: Option<String>,
    pub bytes_written: usize,
    pub tokens: usize,
}

/// Complete workspace health and cleanup audit report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupAuditReport {
    pub total_issues: usize,
    pub broken_symlinks: Vec<BrokenSymlinkIssue>,
    pub duplicate_skills: Vec<DuplicateSkillIssue>,
    pub stale_plugin_caches: Vec<StalePluginCacheItem>,
    pub stale_plugin_cache_bytes: u64,
    pub stale_git_excludes: Vec<StaleGitExcludeIssue>,
    pub claude_synced_buckets: Vec<ClaudeSyncedBucketItem>,
    pub instruction_hygiene: Vec<InstructionHygieneIssue>,
    pub unmanaged_global_skill_ids: Vec<String>,
    pub stray_global_skills: Vec<StraySkillIssue>,
    pub stray_project_skills: Vec<StraySkillIssue>,
}

/// Summary returned after executing a cleanup mutation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupActionResult {
    pub removed_count: usize,
    pub reclaimed_bytes: u64,
    pub warnings: Vec<String>,
}

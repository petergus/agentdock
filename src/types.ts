export type AgentKind = 'antigravity' | 'codex' | 'claude';

export type AgentFilter = 'all' | AgentKind;

export interface ActiveAgents {
  antigravity: boolean;
  codex: boolean;
  claude: boolean;
}

export type ProjectScanMode = 'exactDepth' | 'autoDetect';

export interface ProjectRoot {
  id: string;
  label: string;
  path: string;
  depth: number;
  mode: ProjectScanMode;
}

export interface AgentPaths {
  antigravityDir: string | null;
  codexDir: string | null;
  claudeDir: string | null;
}

export interface LibrarySource {
  id: string;
  label: string;
  path: string;
  depth: number;
}

export interface PausedSkillRef {
  skillId: string;
  canonicalSkillDir: string;
}

export interface PausedRuleRef {
  ruleFilename: string;
  canonicalRuleFile: string;
}

export interface PausedPluginRef {
  agent: AgentKind;
  pluginId: string;
}

export interface GlobalPauseState {
  skillsPaused: boolean;
  pausedSkills: PausedSkillRef[];
  pluginsPaused: boolean;
  pausedPlugins: PausedPluginRef[];
  mcpPaused: boolean;
  pausedMcp: string[];
  rulesPaused: boolean;
  pausedRules: PausedRuleRef[];
}

export type GlobalSwitchTarget = 'all' | 'skills' | 'plugins' | 'mcp';

export interface BatchToggleResult {
  changedCount: number;
  skippedUnmanaged: string[];
  warnings: string[];
  pauseState: GlobalPauseState;
}

export interface ContextSet {
  id: string;
  name: string;
  description: string;
  skillIds: string[];
  mcpNames: string[];
  ruleFilenames: string[];
}

export interface SetToggleResult {
  appliedCount: number;
  skippedSharedCount: number;
  warnings: string[];
  activeSetIds: string[];
}

export type SetOperation = 'apply' | 'disable' | 'repair';

export type SetChangeAction =
  | 'createLink'
  | 'replaceLink'
  | 'removeLink'
  | 'writeConfig'
  | 'removeConfigEntry';

export interface SetItemPreview {
  kind: 'skill' | 'mcp' | 'rule';
  name: string;
  status: 'change' | 'alreadyMatches' | 'shared' | 'blocked';
  detail: string | null;
  files: Array<{
    action: SetChangeAction;
    path: string;
    target: string | null;
    agents: AgentKind[];
  }>;
  estimatedTokens: number | null;
}

export interface SetPreview {
  name: string;
  membershipChanges: boolean;
  alreadyActive: boolean;
  items: SetItemPreview[];
  gitExcludePath: string | null;
  agentImpact: Array<{
    agent: AgentKind;
    estimatedDelta: number;
    hasUncertainChanges: boolean;
  }>;
  revision: string;
}

export type TagColor =
  | 'gray'
  | 'blue'
  | 'green'
  | 'orange'
  | 'red'
  | 'purple'
  | 'yellow';

export interface TagDefinition {
  id: string;
  label: string;
  color: TagColor;
  autoFolder?: string | null;
  hidden: boolean;
}

export interface ProjectTag {
  id: string;
  label: string;
  color: TagColor;
  isAuto: boolean;
  sourceFolder?: string | null;
}

export interface AppConfig {
  projectsRoot?: string | null;
  projectRoots: ProjectRoot[];
  librarySources: LibrarySource[];
  supportDocsRoot: string | null;
  pinnedProjects: string[];
  ignoreFolders: string[];
  referenceScanDepth: number;
  agentPaths: AgentPaths;
  activeAgents: ActiveAgents;
  gitHygiene: boolean;
  globalPause: GlobalPauseState;
  sets: ContextSet[];
  projectActiveSets: Record<string, string[]>;
  hiddenProjects: string[];
  tagDefinitions: TagDefinition[];
  projectManualTags: Record<string, string[]>;
  projectMutedAutoTags: Record<string, string[]>;
}

export type SkillSource =
  | { kind: 'library'; label: string; path: string }
  | { kind: 'globalOnly' }
  | { kind: 'projectOnly' }
  | { kind: 'brokenSymlink'; target: string };

export interface InstalledPathInfo {
  agent: AgentKind;
  scope: ScopeType;
  target: string;
  upfrontTokens: number | null;
  path: string;
  isSymlink: boolean;
  isLegacy: boolean;
  broken: boolean;
}

export interface SkillItem {
  id: string;
  name: string;
  description: string;
  descriptionTooLong: boolean;
  upfrontTokens: number;
  onDemandTokens: number;
  source: SkillSource;
  canonicalPath: string;
  skillDirPath: string;
  isSymlink: boolean;
  isUnmanagedGlobal: boolean;
  activeGlobally: boolean;
  activeInProject: boolean;
  activeAgents: AgentKind[];
  installedPaths: InstalledPathInfo[];
  frontmatterError: string | null;
}

export type McpTransport = 'stdio' | 'http' | 'sse' | 'unknown';

export interface McpServerItem {
  name: string;
  transport: McpTransport;
  commandOrUrl: string;
  args: string[];
  upfrontTokens: number;
  isEstimated: boolean;
  activeGlobally: boolean;
  activeInProject: boolean;
  definedGloballyCollision: boolean;
  activeAgents: AgentKind[];
  sourceConfigs: string[];
  definitions: Array<{ agent: AgentKind; scope: ScopeType; path: string; enabled: boolean }>;
  toolCount: number | null;
  probedAtMs: number | null;
}

export interface McpCatalogEntry {
  name: string;
  transport: McpTransport;
  command: string | null;
  args: string[];
  url: string | null;
  env: Record<string, string>;
  headers: Record<string, string>;
  upfrontTokens: number | null;
  toolCount: number | null;
  probedAtMs: number | null;
}

export interface McpServerDraft {
  name: string;
  transport: McpTransport;
  command: string | null;
  args: string[];
  url: string | null;
  env: Record<string, string>;
  headers: Record<string, string>;
}

export interface McpProbeResult {
  name: string;
  upfrontTokens: number;
  toolCount: number;
  probedAtMs: number;
}

export type RuleTrigger = 'alwaysOn' | 'modelDecision' | 'glob' | 'invalid';

export interface RuleItem {
  id: string;
  filename: string;
  path: string;
  canonicalPath: string;
  trigger: RuleTrigger;
  description: string | null;
  globPattern: string | null;
  upfrontTokens: number;
  fullTokens: number;
  sourceLabel: string;
  isSymlink: boolean;
  isLibraryRule: boolean;
  activeGlobally: boolean;
  activeInProject: boolean;
  activeAgents: AgentKind[];
  frontmatterError: string | null;
  isEditableTrigger: boolean;
  installations: Array<{
    agent: AgentKind;
    scope: ScopeType;
    path: string;
    trigger: RuleTrigger;
    globPattern: string | null;
    upfrontTokens: number;
    isSymlink: boolean;
    error: string | null;
  }>;
}

export type SupportDocMode = 'pointer' | 'symlink';

export interface SupportDocItem {
  name: string;
  path: string;
  description: string | null;
  mode: SupportDocMode;
  exists: boolean;
}

export interface PluginSkillInfo {
  name: string;
  upfrontTokens: number;
}

export interface PluginItem {
  agent: AgentKind;
  id: string;
  name: string;
  description: string | null;
  version: string | null;
  source: string | null;
  path: string;
  configPath?: string | null;
  manageHint?: string | null;
  enabled: boolean;
  canToggle: boolean;
  skills: PluginSkillInfo[];
  agentNames: string[];
  mcpNames: string[];
  ruleNames: string[];
  skillsUpfrontTokens: number;
  agentsUpfrontTokens: number;
  rulesUpfrontTokens: number;
  upfrontTokens: number;
}

export interface TokenBudgetBreakdown {
  rulesGlobal: number;
  rulesProject: number;
  rulesTotal: number;
  rulesLimit: number;
  skillsGlobal: number;
  skillsProject: number;
  pluginsGlobal: number;
  mcpGlobal: number;
  mcpProject: number;
  skillsMcpGlobal: number;
  skillsMcpProject: number;
  skillsMcpTotal: number;
  skillsMcpLimit: number;
  effectiveTotal: number;
}

export interface ProjectSummary {
  id: string;
  name: string;
  category: string;
  path: string;
  isPinned: boolean;
  isHidden: boolean;
  hasGit: boolean;
  ancestorFolders: string[];
  tags: ProjectTag[];
  activeSetIds: string[];
  effectiveTokens: number;
  projectExtraTokens: number;
  activeProjectSkillsCount: number;
  activeProjectMcpCount: number;
  activeProjectRulesCount: number;
}

export interface CategoryGroup {
  name: string;
  projects: ProjectSummary[];
}

export type ScopeType = 'global' | 'project';

export interface ScopeSnapshot {
  scopeType: ScopeType;
  scopeId: string;
  title: string;
  path: string | null;
  budget: TokenBudgetBreakdown;
  skills: SkillItem[];
  plugins: PluginItem[];
  unmanagedGlobalSkillsCount: number;
  mcpServers: McpServerItem[];
  rules: RuleItem[];
  supportDocs: SupportDocItem[];
  scanWarnings: string[];
}

export interface WorkspaceOverview {
  config: AppConfig;
  globalTokens: number;
  globalSnapshot: ScopeSnapshot;
  categories: CategoryGroup[];
  discoveredAutoFolders: string[];
  lastScannedMs: number;
}

export type ToggleScope =
  | { kind: 'global' }
  | { kind: 'project'; projectPath: string };

export type ToggleOutcome =
  | { status: 'updated'; changedPaths: string[] }
  | {
      status: 'requiresMoveToLibrary';
      skillId: string;
      realPaths: string[];
    };

export type AfterMoveAction = 'unlink' | 'relink';

export interface MovePreviewItem {
  skillId: string;
  skillName: string;
  sourceRealDir: string;
  destinationDir: string;
  symlinksToUpdate: string[];
  conflictExists: boolean;
  warning: string | null;
}

export interface MoveExecutionResult {
  moved: string[];
  skipped: string[];
  warnings: string[];
}

export type RuleTriggerUpdate =
  | { trigger: 'alwaysOn' }
  | { trigger: 'modelDecision'; description: string }
  | { trigger: 'glob'; pattern: string };

export type SupportDocTargetFile = 'agentsMd' | 'supportDocsRule';

export interface LinkSupportDocRequest {
  projectPath: string;
  sourcePath: string;
  mode: SupportDocMode;
  description: string | null;
  targetFile: SupportDocTargetFile | null;
}

export interface BrokenSymlinkIssue {
  path: string;
  target: string;
  scopeLabel: string;
  projectPath: string | null;
  agent: AgentKind | null;
  itemKind: 'skill' | 'rule' | 'supportDoc' | string;
  originalPath: string | null;
  originalLocation: string | null;
}

export interface StraySkillIssue {
  skillId: string;
  skillName: string;
  path: string;
  skillMdPath: string;
  scopeLabel: string;
  projectPath: string | null;
  agents: AgentKind[];
  upfrontTokens: number;
  existsInLibrary: boolean;
}

export interface DuplicateSkillIssue {
  skillName: string;
  skillId: string;
  standalonePath: string;
  scopeLabel: string;
  projectPath: string | null;
  pluginAgent: AgentKind;
  pluginId: string;
  pluginName: string;
  duplicateTokens: number;
  isSymlink: boolean;
}

export interface StalePluginCacheItem {
  marketplace: string;
  pluginSlug: string;
  version: string;
  path: string;
  reason: string;
  sizeBytes: number;
}

export interface StaleGitExcludeIssue {
  projectName: string;
  projectPath: string;
  entries: string[];
}

export interface ClaudeSyncedBucketItem {
  name: string;
  path: string;
  sizeBytes: number;
  entryCount: number;
}

export type InstructionHygieneKind =
  | 'frontmatter'
  | 'oversized'
  | 'paused_skill_ref';

export interface InstructionHygieneIssue {
  path: string;
  filename: string;
  agent: AgentKind;
  kind: InstructionHygieneKind;
  severity: string;
  message: string;
  tokens: number;
}

export interface InstructionFixPreview {
  path: string;
  canonicalPath: string;
  filename: string;
  kind: InstructionHygieneKind;
  title: string;
  why: string;
  whatChanges: string;
  currentContent: string;
  currentTokens: number;
  suggestedContent: string;
  suggestedTokens: number;
}

export interface InstructionFixApplyResult {
  path: string;
  backupPath: string | null;
  bytesWritten: number;
  tokens: number;
}

export interface CleanupAuditReport {
  totalIssues: number;
  brokenSymlinks: BrokenSymlinkIssue[];
  duplicateSkills: DuplicateSkillIssue[];
  stalePluginCaches: StalePluginCacheItem[];
  stalePluginCacheBytes: number;
  staleGitExcludes: StaleGitExcludeIssue[];
  claudeSyncedBuckets: ClaudeSyncedBucketItem[];
  instructionHygiene: InstructionHygieneIssue[];
  unmanagedGlobalSkillIds: string[];
  strayGlobalSkills: StraySkillIssue[];
  strayProjectSkills: StraySkillIssue[];
}

export interface CleanupActionResult {
  removedCount: number;
  reclaimedBytes: number;
  warnings: string[];
}

export type AgentKind = 'antigravity' | 'codex' | 'claude';

export type AgentFilter = 'all' | AgentKind;

export interface ActiveAgents {
  antigravity: boolean;
  codex: boolean;
  claude: boolean;
}

export interface LibrarySource {
  id: string;
  label: string;
  path: string;
}

export interface AppConfig {
  projectsRoot: string | null;
  librarySources: LibrarySource[];
  supportDocsRoot: string | null;
  pinnedProjects: string[];
  ignoreFolders: string[];
  activeAgents: ActiveAgents;
  gitHygiene: boolean;
}

export type SkillSource =
  | { kind: 'library'; label: string; path: string }
  | { kind: 'globalOnly' }
  | { kind: 'projectOnly' }
  | { kind: 'brokenSymlink'; target: string };

export interface InstalledPathInfo {
  agent: AgentKind;
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
}

export type SupportDocMode = 'pointer' | 'symlink';

export interface SupportDocItem {
  name: string;
  path: string;
  description: string | null;
  mode: SupportDocMode;
  exists: boolean;
}

export interface TokenBudgetBreakdown {
  rulesGlobal: number;
  rulesProject: number;
  rulesTotal: number;
  rulesLimit: number;
  skillsGlobal: number;
  skillsProject: number;
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
  hasGit: boolean;
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

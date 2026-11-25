import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import type {
  AfterMoveAction,
  AgentKind,
  AppConfig,
  BatchToggleResult,
  CleanupActionResult,
  CleanupAuditReport,
  ContextSet,
  GlobalSwitchTarget,
  LinkSupportDocRequest,
  McpCatalogEntry,
  McpProbeResult,
  McpServerDraft,
  MoveExecutionResult,
  MovePreviewItem,
  RuleTriggerUpdate,
  ScopeSnapshot,
  SetOperation,
  SetPreview,
  SetToggleResult,
  SupportDocMode,
  TagDefinition,
  ToggleOutcome,
  ToggleScope,
  WorkspaceOverview,
} from '@/types';

export const api = {
  getConfig: (): Promise<AppConfig> => invoke('get_config'),
  saveConfig: (config: AppConfig): Promise<AppConfig> =>
    invoke('save_config', { config }),

  togglePinnedProject: (projectPath: string): Promise<AppConfig> =>
    invoke('toggle_pinned_project', { projectPath }),
  scanWorkspace: (): Promise<WorkspaceOverview> => invoke('scan_workspace'),
  scanProject: (projectPath: string, category: string): Promise<ScopeSnapshot> =>
    invoke('scan_project', { projectPath, category }),
  revealInFinder: (path: string): Promise<void> =>
    invoke('reveal_in_finder', { path }),
  openInDefaultApp: (path: string): Promise<void> =>
    invoke('open_in_default_app', { path }),

  toggleSkill: (
    scope: ToggleScope,
    skillId: string,
    canonicalSkillDir: string,
    enable: boolean,
  ): Promise<ToggleOutcome> =>
    invoke('toggle_skill', {
      scope,
      skillId,
      canonicalSkillDir,
      enable,
    }),

  unlinkBrokenSymlink: (
    targetPath: string,
    projectPath: string | null,
  ): Promise<void> =>
    invoke('unlink_broken_symlink', { targetPath, projectPath }),

  toggleLibraryRule: (
    scope: ToggleScope,
    ruleFilename: string,
    canonicalRuleFile: string,
    enable: boolean,
  ): Promise<ToggleOutcome> =>
    invoke('toggle_library_rule', {
      scope,
      ruleFilename,
      canonicalRuleFile,
      enable,
    }),

  previewMoveSkillsToLibrary: (
    skillIds: string[],
    targetLibraryPath: string,
    scope: ToggleScope,
  ): Promise<MovePreviewItem[]> =>
    invoke('preview_move_skills_to_library', {
      skillIds,
      targetLibraryPath,
      scope,
    }),

  executeMoveSkillsToLibrary: (
    skillIds: string[],
    targetLibraryPath: string,
    scope: ToggleScope,
    afterMove: AfterMoveAction,
  ): Promise<MoveExecutionResult> =>
    invoke('execute_move_skills_to_library', {
      skillIds,
      targetLibraryPath,
      scope,
      afterMove,
    }),

  updateRuleTrigger: (
    rulePath: string,
    update: RuleTriggerUpdate,
  ): Promise<void> => invoke('update_rule_trigger', { rulePath, update }),

  linkSupportDoc: (request: LinkSupportDocRequest): Promise<void> =>
    invoke('link_support_doc', { request }),

  unlinkSupportDoc: (
    projectPath: string,
    docPath: string,
    mode: SupportDocMode,
  ): Promise<void> =>
    invoke('unlink_support_doc', { projectPath, docPath, mode }),

  setWatchedProject: (projectPath: string | null): Promise<void> =>
    invoke('set_watched_project', { projectPath }),

  toggleMcpServer: (
    scope: ToggleScope,
    serverName: string,
    enable: boolean,
  ): Promise<void> =>
    invoke('toggle_mcp_server', { scope, serverName, enable }),

  addMcpServer: (
    draft: McpServerDraft,
    enableInScope: ToggleScope | null,
  ): Promise<McpCatalogEntry> =>
    invoke('add_mcp_server', { draft, enableInScope }),

  probeMcpServer: (serverName: string): Promise<McpProbeResult> =>
    invoke('probe_mcp_server', { serverName }),

  setGlobalSwitch: (
    target: GlobalSwitchTarget,
    enable: boolean,
  ): Promise<BatchToggleResult> =>
    invoke('set_global_switch', { target, enable }),

  togglePlugin: (
    agent: AgentKind,
    pluginId: string,
    enable: boolean,
  ): Promise<void> => invoke('toggle_plugin', { agent, pluginId, enable }),

  previewProjectSet: (
    projectPath: string,
    setId: string,
    operation: SetOperation,
  ): Promise<SetPreview> =>
    invoke('preview_project_set', { projectPath, setId, operation }),

  toggleProjectSet: (
    projectPath: string,
    setId: string,
    operation: SetOperation,
    revision: string,
  ): Promise<SetToggleResult> =>
    invoke('toggle_project_set', { projectPath, setId, operation, revision }),

  upsertSet: (set: ContextSet): Promise<AppConfig> =>
    invoke('upsert_set', { set }),

  deleteSet: (setId: string): Promise<AppConfig> =>
    invoke('delete_set', { setId }),

  toggleHiddenProject: (projectPath: string): Promise<AppConfig> =>
    invoke('toggle_hidden_project', { projectPath }),

  setProjectTags: (
    projectPath: string,
    manualTagIds: string[],
    mutedAutoTagIds: string[],
  ): Promise<AppConfig> =>
    invoke('set_project_tags', {
      projectPath,
      manualTagIds,
      mutedAutoTagIds,
    }),

  upsertTagDefinition: (tag: TagDefinition): Promise<AppConfig> =>
    invoke('upsert_tag_definition', { tag }),

  deleteTagDefinition: (tagId: string): Promise<AppConfig> =>
    invoke('delete_tag_definition', { tagId }),

  runCleanupAudit: (): Promise<CleanupAuditReport> =>
    invoke('run_cleanup_audit'),

  unlinkBrokenSymlinksBatch: (
    items: Array<[string, string | null]>,
  ): Promise<CleanupActionResult> =>
    invoke('unlink_broken_symlinks_batch', { items }),

  pruneCodexPluginCaches: (paths: string[]): Promise<CleanupActionResult> =>
    invoke('prune_codex_plugin_caches', { paths }),

  pruneStaleGitExcludes: (
    projectPaths: string[],
  ): Promise<CleanupActionResult> =>
    invoke('prune_stale_git_excludes', { projectPaths }),

  removeClaudeSyncedBucket: (path: string): Promise<CleanupActionResult> =>
    invoke('remove_claude_synced_bucket', { path }),

  previewInstructionFix: (
    path: string,
    kind: import('@/types').InstructionHygieneKind,
  ): Promise<import('@/types').InstructionFixPreview> =>
    invoke('preview_instruction_fix', { path, kind }),

  applyInstructionFix: (
    path: string,
    content: string,
  ): Promise<import('@/types').InstructionFixApplyResult> =>
    invoke('apply_instruction_fix', { path, content }),

  pickDirectory: async (defaultPath?: string | null): Promise<string | null> => {
    const selected = await open({
      directory: true,
      multiple: false,
      defaultPath: defaultPath ?? undefined,
    });
    return typeof selected === 'string' ? selected : null;
  },

  pickFile: async (defaultPath?: string | null): Promise<string | null> => {
    const selected = await open({
      directory: false,
      multiple: false,
      defaultPath: defaultPath ?? undefined,
    });
    return typeof selected === 'string' ? selected : null;
  },
};

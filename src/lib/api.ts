import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import type {
  AfterMoveAction,
  AppConfig,
  LinkSupportDocRequest,
  McpCatalogEntry,
  McpProbeResult,
  McpServerDraft,
  MoveExecutionResult,
  MovePreviewItem,
  RuleTriggerUpdate,
  ScopeSnapshot,
  SupportDocMode,
  ToggleOutcome,
  ToggleScope,
  WorkspaceOverview,
} from '@/types';

export const api = {
  getConfig: (): Promise<AppConfig> => invoke('get_config'),
  saveConfig: (config: AppConfig): Promise<AppConfig> =>
    invoke('save_config', { config }),
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

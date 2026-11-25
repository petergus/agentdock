import type {
  AgentFilter,
  AgentKind,
  RuleTrigger,
  ScopeSnapshot,
  SkillItem,
  TagColor,
  TokenBudgetBreakdown,
} from '@/types';

export const TAG_COLORS: Array<{
  id: TagColor;
  label: string;
  hex: string;
}> = [
  { id: 'gray', label: 'Gray', hex: '#8e8e93' },
  { id: 'blue', label: 'Blue', hex: '#007aff' },
  { id: 'green', label: 'Green', hex: '#34c759' },
  { id: 'orange', label: 'Orange', hex: '#ff9500' },
  { id: 'red', label: 'Red', hex: '#ff3b30' },
  { id: 'purple', label: 'Purple', hex: '#af52de' },
  { id: 'yellow', label: 'Yellow', hex: '#ffcc00' },
];

export function tagColorHex(color: TagColor): string {
  return TAG_COLORS.find((c) => c.id === color)?.hex ?? '#8e8e93';
}

export function formatTokensCompact(tokens: number): string {
  if (tokens >= 1000) {
    const k = tokens / 1000;
    return k >= 10 ? `${Math.round(k)}k` : `${k.toFixed(1).replace(/\.0$/, '')}k`;
  }
  return `${tokens}`;
}

export function formatTokensExact(tokens: number): string {
  return tokens.toLocaleString('en-US');
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  const units = ['KB', 'MB', 'GB'];
  let value = bytes / 1024;
  let unitIdx = 0;
  while (value >= 1024 && unitIdx < units.length - 1) {
    value /= 1024;
    unitIdx += 1;
  }
  const formatted = value >= 10 ? Math.round(value).toString() : value.toFixed(1).replace(/\.0$/, '');
  return `${formatted} ${units[unitIdx]}`;
}

export function agentDisplayName(agent: AgentKind): string {
  switch (agent) {
    case 'antigravity':
      return 'Antigravity';
    case 'codex':
      return 'Codex';
    case 'claude':
      return 'Claude';
  }
}

export function ruleTriggerLabel(trigger: RuleTrigger): string {
  switch (trigger) {
    case 'alwaysOn':
      return 'Always On';
    case 'modelDecision':
      return 'Model Decision';
    case 'glob':
      return 'Glob';
    case 'invalid':
      return 'Invalid Frontmatter';
  }
}

export function skillSourceLabel(skill: SkillItem): string {
  switch (skill.source.kind) {
    case 'library':
      return skill.source.label;
    case 'globalOnly':
      return skill.isUnmanagedGlobal ? 'Loose in Global' : 'Global Only';
    case 'projectOnly':
      return 'Project Folder';
    case 'brokenSymlink':
      return 'Broken Link';
  }
}

export function matchesAgentFilter(
  activeAgents: AgentKind[],
  filter: AgentFilter,
  alwaysIncludeUninstalled = false,
): boolean {
  if (filter === 'all') {
    return true;
  }
  if (activeAgents.length === 0) {
    return alwaysIncludeUninstalled;
  }
  return activeAgents.includes(filter);
}

export function computeFilteredBudget(
  snapshot: ScopeSnapshot,
  filter: AgentFilter,
): TokenBudgetBreakdown {
  if (filter === 'all') {
    return snapshot.budget;
  }

  let rulesGlobal = 0;
  let rulesProject = 0;
  for (const rule of snapshot.rules) {
    if (!rule.activeAgents.includes(filter)) {
      continue;
    }
    if (rule.activeGlobally) {
      rulesGlobal += rule.upfrontTokens;
    } else if (rule.activeInProject) {
      rulesProject += rule.upfrontTokens;
    }
  }

  let skillsGlobal = 0;
  let skillsProject = 0;
  for (const skill of snapshot.skills) {
    if (!skill.activeAgents.includes(filter)) {
      continue;
    }
    if (skill.activeGlobally) {
      skillsGlobal += skill.upfrontTokens;
    } else if (skill.activeInProject) {
      skillsProject += skill.upfrontTokens;
    }
  }

  let pluginsGlobal = 0;
  for (const plugin of snapshot.plugins) {
    if (!plugin.enabled || plugin.agent !== filter) {
      continue;
    }
    pluginsGlobal += plugin.upfrontTokens;
    rulesGlobal += plugin.rulesUpfrontTokens;
  }

  let mcpGlobal = 0;
  let mcpProject = 0;
  for (const mcp of snapshot.mcpServers) {
    if (!mcp.activeAgents.includes(filter)) {
      continue;
    }
    if (mcp.activeGlobally) {
      mcpGlobal += mcp.upfrontTokens;
    } else if (mcp.activeInProject) {
      mcpProject += mcp.upfrontTokens;
    }
  }

  const rulesTotal = rulesGlobal + rulesProject;
  const skillsMcpGlobal = skillsGlobal + pluginsGlobal + mcpGlobal;
  const skillsMcpProject = skillsProject + mcpProject;
  const skillsMcpTotal = skillsMcpGlobal + skillsMcpProject;

  return {
    rulesGlobal,
    rulesProject,
    rulesTotal,
    rulesLimit: snapshot.budget.rulesLimit,
    skillsGlobal,
    skillsProject,
    pluginsGlobal,
    mcpGlobal,
    mcpProject,
    skillsMcpGlobal,
    skillsMcpProject,
    skillsMcpTotal,
    skillsMcpLimit: snapshot.budget.skillsMcpLimit,
    effectiveTotal: rulesTotal + skillsMcpTotal,
  };
}

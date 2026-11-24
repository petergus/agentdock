import type {
  AgentFilter,
  AgentKind,
  RuleTrigger,
  ScopeSnapshot,
  SkillItem,
  TokenBudgetBreakdown,
} from '@/types';

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
      return 'Global Only';
    case 'projectOnly':
      return 'Project Only';
    case 'brokenSymlink':
      return 'Broken Symlink';
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
  const skillsMcpGlobal = skillsGlobal + mcpGlobal;
  const skillsMcpProject = skillsProject + mcpProject;
  const skillsMcpTotal = skillsMcpGlobal + skillsMcpProject;

  return {
    rulesGlobal,
    rulesProject,
    rulesTotal,
    rulesLimit: snapshot.budget.rulesLimit,
    skillsGlobal,
    skillsProject,
    mcpGlobal,
    mcpProject,
    skillsMcpGlobal,
    skillsMcpProject,
    skillsMcpTotal,
    skillsMcpLimit: snapshot.budget.skillsMcpLimit,
    effectiveTotal: rulesTotal + skillsMcpTotal,
  };
}

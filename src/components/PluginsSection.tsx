import { useMemo, useState } from 'react';
import { ChevronRight, FolderOpen, Puzzle } from 'lucide-react';
import type { AgentFilter, AgentKind, PluginItem, ScopeType } from '@/types';
import { formatTokensExact } from '@/lib/format';
import { MacSwitch } from '@/components/ui/MacSwitch';

interface PluginsSectionProps {
  scopeType: ScopeType;
  plugins: PluginItem[];
  agentFilter: AgentFilter;
  busyPluginKeys: Record<string, boolean>;
  onTogglePlugin: (plugin: PluginItem, next: boolean) => void;
  onRevealPath: (path: string) => void;
}

interface AgentSectionMeta {
  agent: AgentKind;
  title: string;
  fallbackHint: string;
}

const AGENT_SECTIONS: AgentSectionMeta[] = [
  {
    agent: 'antigravity',
    title: 'Antigravity Customizations',
    fallbackHint: 'Change in ~/.gemini/config/config.json or via Antigravity Plugins Manager',
  },
  {
    agent: 'codex',
    title: 'Codex Plugins',
    fallbackHint: 'Change in ~/.codex/config.toml ([plugins]) or Codex App',
  },
  {
    agent: 'claude',
    title: 'Claude Plugins & Customizations',
    fallbackHint: 'Change in ~/.claude/settings.json (enabledPlugins) or via claude plugin',
  },
];

export function pluginKey(plugin: Pick<PluginItem, 'agent' | 'id'>): string {
  return `${plugin.agent}:${plugin.id}`;
}

function basenameOfPath(path: string): string {
  const parts = path.split('/').filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

export function PluginsSection({
  scopeType,
  plugins,
  agentFilter,
  busyPluginKeys,
  onTogglePlugin,
  onRevealPath,
}: PluginsSectionProps) {
  const [expandedIds, setExpandedIds] = useState<Record<string, boolean>>({});

  const groupedSections = useMemo(() => {
    return AGENT_SECTIONS.filter(
      (meta) => agentFilter === 'all' || agentFilter === meta.agent,
    )
      .map((meta) => {
        const items = plugins
          .filter((p) => {
            if (p.agent !== meta.agent) {
              return false;
            }
            if (scopeType === 'project' && !p.enabled) {
              return false;
            }
            return true;
          })
          .sort((a, b) => {
            if (a.enabled !== b.enabled) {
              return a.enabled ? -1 : 1;
            }
            return a.name.toLowerCase().localeCompare(b.name.toLowerCase());
          });

        const configPath =
          items.find((p) => p.configPath)?.configPath ?? null;
        const manageHint =
          items.find((p) => p.manageHint)?.manageHint ?? meta.fallbackHint;
        const loadedCount = items.filter((p) => p.enabled).length;
        const disabledCount = items.length - loadedCount;

        return {
          ...meta,
          items,
          configPath,
          manageHint,
          loadedCount,
          disabledCount,
        };
      })
      .filter((section) => section.items.length > 0);
  }, [plugins, scopeType, agentFilter]);

  if (groupedSections.length === 0) {
    return null;
  }

  const toggleExpanded = (key: string) => {
    setExpandedIds((prev) => ({ ...prev, [key]: !prev[key] }));
  };

  return (
    <div className="space-y-6">
      {groupedSections.map((section) => {
        const headingId = `plugins-heading-${section.agent}`;
        const countSummary =
          section.disabledCount > 0
            ? `${section.loadedCount} loaded · ${section.disabledCount} disabled`
            : `${section.loadedCount} loaded`;

        return (
          <section
            key={section.agent}
            aria-labelledby={headingId}
            className="space-y-2.5"
          >
            <div className="flex flex-wrap items-center justify-between gap-2">
              <div className="flex items-baseline gap-2.5">
                <h2
                  id={headingId}
                  className="text-[14px] font-semibold tracking-[-0.01em] text-[var(--text-primary)]"
                >
                  {section.title}
                </h2>
                <span className="tabular-nums text-[12px] text-[var(--text-tertiary)]">
                  {countSummary}
                </span>
              </div>

              <div className="flex items-center gap-2">
                {scopeType === 'project' && (
                  <span className="text-[11.5px] text-[var(--text-tertiary)]">
                    Inherited from Global
                  </span>
                )}
                {section.configPath && (
                  <button
                    type="button"
                    onClick={() =>
                      section.configPath && onRevealPath(section.configPath)
                    }
                    title={`Reveal ${section.configPath} in Finder`}
                    className="mac-btn inline-flex h-7 items-center gap-1.5 px-2.5 text-[11.5px] font-medium"
                  >
                    <FolderOpen className="h-3 w-3" strokeWidth={1.75} />
                    <span>{basenameOfPath(section.configPath)}</span>
                  </button>
                )}
              </div>
            </div>

            <div className="mac-group overflow-hidden">
              {section.items.map((plugin, idx) => {
                const key = pluginKey(plugin);
                const isExpanded = Boolean(expandedIds[key]);
                const hasBundleItems =
                  plugin.skills.length > 0 ||
                  plugin.agentNames.length > 0 ||
                  plugin.ruleNames.length > 0 ||
                  plugin.mcpNames.length > 0 ||
                  Boolean(plugin.manageHint);

                const countParts: string[] = [];
                if (plugin.skills.length > 0) {
                  const isCommands = plugin.id === 'claude-custom-commands';
                  const singular = isCommands ? 'command' : 'skill';
                  const plural = isCommands ? 'commands' : 'skills';
                  countParts.push(
                    `${plugin.skills.length} ${plugin.skills.length === 1 ? singular : plural}`,
                  );
                }
                if (plugin.agentNames.length > 0) {
                  countParts.push(
                    `${plugin.agentNames.length} ${plugin.agentNames.length === 1 ? 'agent' : 'agents'}`,
                  );
                }
                if (plugin.ruleNames.length > 0) {
                  countParts.push(
                    `${plugin.ruleNames.length} ${plugin.ruleNames.length === 1 ? 'rule' : 'rules'}`,
                  );
                }
                if (plugin.mcpNames.length > 0) {
                  countParts.push(`${plugin.mcpNames.length} MCP`);
                }

                const tokenTooltip = [
                  `Skills upfront: ${formatTokensExact(plugin.skillsUpfrontTokens)} tok`,
                  `Subagents upfront: ${formatTokensExact(plugin.agentsUpfrontTokens)} tok`,
                  plugin.rulesUpfrontTokens > 0
                    ? `Plugin rules (counted in Rules): ${formatTokensExact(plugin.rulesUpfrontTokens)} tok`
                    : null,
                  plugin.description ? `\n${plugin.description}` : null,
                ]
                  .filter(Boolean)
                  .join('\n');

                return (
                  <div
                    key={key}
                    className={idx > 0 ? 'mac-row-divider' : undefined}
                  >
                    <div className="flex min-h-[40px] items-center justify-between gap-3 px-4 py-1.5 transition-colors duration-100 hover:bg-[var(--bg-hover)]">
                      <div className="flex min-w-0 flex-1 items-center gap-2">
                        {hasBundleItems ? (
                          <button
                            type="button"
                            onClick={() => toggleExpanded(key)}
                            aria-expanded={isExpanded}
                            aria-label={`${isExpanded ? 'Collapse' : 'Expand'} ${plugin.name} bundle contents`}
                            className="inline-flex h-5 w-5 shrink-0 items-center justify-center rounded text-[var(--text-tertiary)] hover:bg-[var(--bg-subtle)] hover:text-[var(--text-primary)]"
                          >
                            <ChevronRight
                              className={[
                                'h-3.5 w-3.5 transition-transform duration-150',
                                isExpanded ? 'rotate-90' : '',
                              ].join(' ')}
                              strokeWidth={1.75}
                            />
                          </button>
                        ) : (
                          <Puzzle
                            className="h-3.5 w-3.5 shrink-0 text-[var(--text-tertiary)]"
                            strokeWidth={1.75}
                            aria-hidden="true"
                          />
                        )}

                        <button
                          type="button"
                          onClick={() => hasBundleItems && toggleExpanded(key)}
                          className={[
                            'flex min-w-0 flex-1 items-center gap-2 text-left',
                            hasBundleItems ? 'cursor-pointer' : 'cursor-default',
                          ].join(' ')}
                        >
                          <span className="truncate text-[13px] font-medium text-[var(--text-primary)]">
                            {plugin.name}
                          </span>

                          {plugin.source && (
                            <span className="shrink-0 text-[11.5px] text-[var(--text-tertiary)]">
                              {plugin.source}
                            </span>
                          )}

                          {plugin.version && (
                            <span className="hidden shrink-0 text-[11px] text-[var(--text-tertiary)] sm:inline">
                              v{plugin.version}
                            </span>
                          )}

                          {countParts.length > 0 && (
                            <span className="hidden truncate text-[11.5px] text-[var(--text-secondary)] md:inline">
                              · {countParts.join(' · ')}
                            </span>
                          )}
                        </button>
                      </div>

                      <div className="flex shrink-0 items-center gap-2.5">
                        <span
                          title={tokenTooltip}
                          className="tabular-nums min-w-[68px] text-right text-[12px] text-[var(--text-secondary)]"
                        >
                          {formatTokensExact(plugin.upfrontTokens)} tok
                        </span>

                        <button
                          type="button"
                          onClick={() => onRevealPath(plugin.path)}
                          aria-label={`Reveal ${plugin.name} in Finder`}
                          title={`Reveal location: ${plugin.path}`}
                          className="inline-flex h-7 w-7 items-center justify-center rounded-[6px] text-[var(--text-secondary)] hover:bg-[var(--bg-subtle)] hover:text-[var(--text-primary)]"
                        >
                          <FolderOpen className="h-3.5 w-3.5" strokeWidth={1.75} />
                        </button>

                        {scopeType === 'project' ? (
                          <span
                            title="Managed globally — switch to Global scope to toggle"
                            className="inline-flex w-9 items-center justify-center text-[11.5px] font-medium text-[var(--accent)]"
                          >
                            Global
                          </span>
                        ) : plugin.canToggle ? (
                          <MacSwitch
                            checked={plugin.enabled}
                            disabled={Boolean(busyPluginKeys[key])}
                            label={`Toggle ${plugin.name} plugin`}
                            tooltip={
                              plugin.enabled
                                ? `Disable ${plugin.name}`
                                : `Enable ${plugin.name}`
                            }
                            onChange={(next) => onTogglePlugin(plugin, next)}
                          />
                        ) : (
                          <span
                            className={[
                              'inline-flex min-w-[60px] items-center justify-center rounded-full px-2 py-0.5 text-[11px] font-medium',
                              plugin.enabled
                                ? 'bg-[var(--status-green)]/12 text-[var(--status-green)]'
                                : 'bg-[var(--bg-field)] text-[var(--text-tertiary)]',
                            ].join(' ')}
                          >
                            {plugin.enabled ? 'Loaded' : 'Disabled'}
                          </span>
                        )}
                      </div>
                    </div>

                    {isExpanded && hasBundleItems && (
                      <div className="space-y-2.5 border-t border-[var(--hairline-inset)] bg-[var(--bg-field)]/45 px-11 py-3 text-[12px]">
                        {plugin.description && (
                          <p className="text-[11.5px] leading-relaxed text-[var(--text-secondary)]">
                            {plugin.description}
                          </p>
                        )}

                        {plugin.skills.length > 0 && (
                          <div className="space-y-1">
                            <div className="text-[11px] font-medium uppercase tracking-wide text-[var(--text-tertiary)]">
                              {plugin.id === 'claude-custom-commands'
                                ? `Slash Commands (${plugin.skills.length})`
                                : `Bundled Skills (${plugin.skills.length})`}
                            </div>
                            <div className="grid grid-cols-1 gap-x-6 gap-y-1 sm:grid-cols-2">
                              {plugin.skills.map((skill, sIdx) => (
                                <div
                                  key={`${skill.name}-${sIdx}`}
                                  className="flex items-center justify-between gap-2 text-[11.5px]"
                                >
                                  <span className="truncate text-[var(--text-primary)]">
                                    {skill.name}
                                  </span>
                                  <span className="tabular-nums shrink-0 text-[var(--text-tertiary)]">
                                    {formatTokensExact(skill.upfrontTokens)} tok
                                  </span>
                                </div>
                              ))}
                            </div>
                          </div>
                        )}

                        {plugin.agentNames.length > 0 && (
                          <div className="flex flex-wrap items-baseline gap-x-2 gap-y-1 text-[11.5px]">
                            <span className="text-[11px] font-medium uppercase tracking-wide text-[var(--text-tertiary)]">
                              Subagents:
                            </span>
                            <span className="text-[var(--text-secondary)]">
                              {plugin.agentNames.join(', ')}
                            </span>
                          </div>
                        )}

                        {plugin.ruleNames.length > 0 && (
                          <div className="flex flex-wrap items-baseline gap-x-2 gap-y-1 text-[11.5px]">
                            <span className="text-[11px] font-medium uppercase tracking-wide text-[var(--text-tertiary)]">
                              Rules:
                            </span>
                            <span className="text-[var(--text-secondary)]">
                              {plugin.ruleNames.join(', ')} (
                              {formatTokensExact(plugin.rulesUpfrontTokens)} tok)
                            </span>
                          </div>
                        )}

                        {plugin.mcpNames.length > 0 && (
                          <div className="flex flex-wrap items-baseline gap-x-2 gap-y-1 text-[11.5px]">
                            <span className="text-[11px] font-medium uppercase tracking-wide text-[var(--text-tertiary)]">
                              MCP Servers:
                            </span>
                            <span className="text-[var(--text-secondary)]">
                              {plugin.mcpNames.join(', ')}
                            </span>
                          </div>
                        )}

                        {(plugin.manageHint || plugin.configPath) && (
                          <div className="flex flex-wrap items-baseline gap-x-2 gap-y-1 pt-0.5 text-[11.5px]">
                            <span className="text-[11px] font-medium uppercase tracking-wide text-[var(--text-tertiary)]">
                              {plugin.canToggle ? 'Config:' : 'Where to change:'}
                            </span>
                            {plugin.manageHint && (
                              <span className="text-[var(--text-secondary)]">
                                {plugin.manageHint}
                              </span>
                            )}
                            {plugin.configPath && (
                              <button
                                type="button"
                                onClick={() =>
                                  plugin.configPath &&
                                  onRevealPath(plugin.configPath)
                                }
                                className="mono-path text-[11px] text-[var(--accent)] hover:underline"
                              >
                                {plugin.configPath}
                              </button>
                            )}
                          </div>
                        )}
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
          </section>
        );
      })}
    </div>
  );
}

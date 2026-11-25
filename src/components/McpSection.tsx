import { useMemo } from 'react';
import { Gauge, Plus, Server } from 'lucide-react';
import type { AgentFilter, McpServerItem, ScopeType } from '@/types';
import {
  agentDisplayName,
  formatTokensExact,
  matchesAgentFilter,
} from '@/lib/format';
import { MacSwitch } from '@/components/ui/MacSwitch';

const probeDate = new Intl.DateTimeFormat(undefined, {
  dateStyle: 'medium',
  timeStyle: 'short',
});

export function mcpRowId(name: string): string {
  return `mcp-server-${encodeURIComponent(name)}`;
}

interface McpSectionProps {
  scopeType: ScopeType;
  servers: McpServerItem[];
  agentFilter: AgentFilter;
  busyServerNames: Record<string, boolean>;
  probingServerNames: Record<string, boolean>;
  onToggleMcp: (server: McpServerItem, next: boolean) => void;
  onProbeMcp: (server: McpServerItem) => void;
  onOpenAddMcp: () => void;
}

export function McpSection({
  scopeType,
  servers,
  agentFilter,
  busyServerNames,
  probingServerNames,
  onToggleMcp,
  onProbeMcp,
  onOpenAddMcp,
}: McpSectionProps) {
  const visibleServers = useMemo(
    () => servers.filter((s) => matchesAgentFilter(s.activeAgents, agentFilter, true)),
    [servers, agentFilter],
  );

  return (
    <section aria-labelledby="mcp-heading" className="space-y-3">
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-baseline gap-2.5">
          <h2
            id="mcp-heading"
            className="text-[14px] font-semibold tracking-[-0.01em] text-[var(--text-primary)]"
          >
            MCP Servers
          </h2>
          <span className="tabular-nums text-[12px] text-[var(--text-tertiary)]">
            {visibleServers.length}
          </span>
        </div>

        <button
          type="button"
          onClick={onOpenAddMcp}
          className="mac-btn inline-flex h-7 items-center gap-1 px-2.5 text-[12px] font-medium"
        >
          <Plus className="h-3.5 w-3.5" strokeWidth={1.75} />
          Add MCP
        </button>
      </div>

      {visibleServers.length === 0 ? (
        <div className="mac-group px-4 py-6 text-center text-[12.5px] text-[var(--text-secondary)]">
          <Server
            className="mx-auto mb-1.5 h-4 w-4 text-[var(--text-tertiary)]"
            strokeWidth={1.6}
            aria-hidden="true"
          />
          No MCP servers discovered for the selected scope and agent filter.
        </div>
      ) : (
        <div className="mac-group overflow-hidden">
          {visibleServers.map((server, idx) => {
            const isInheritedGlobal =
              scopeType === 'project' &&
              server.activeGlobally &&
              !server.definedGloballyCollision;
            const isChecked =
              scopeType === 'global'
                ? server.activeGlobally
                : server.activeInProject;
            const isBusy = Boolean(busyServerNames[server.name]);
            const isProbing = Boolean(probingServerNames[server.name]);
            const canProbe = server.transport === 'stdio';
            const subtitle = [server.commandOrUrl, ...server.args]
              .filter(Boolean)
              .join(' ');
            const agentsSummary = server.activeAgents
              .map(agentDisplayName)
              .join(', ');

            return (
              <div
                key={server.name}
                id={mcpRowId(server.name)}
                tabIndex={-1}
                className={[
                  'flex min-h-[42px] items-center justify-between gap-3 px-4 py-1.5 transition-colors duration-100 hover:bg-[var(--bg-hover)] focus:bg-[var(--bg-hover)] focus-visible:outline-offset-[-2px]',
                  idx > 0 ? 'mac-row-divider' : '',
                ].join(' ')}
              >
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="truncate text-[13px] font-medium text-[var(--text-primary)]">
                      {server.name}
                    </span>
                    {server.definedGloballyCollision && (
                      <span
                        title="Already active globally — Antigravity uses the Global copy, so turning off the project override removes redundant config."
                        className="shrink-0 text-[11.5px] font-medium text-[var(--status-amber)]"
                      >
                        Defined Globally
                      </span>
                    )}
                    {agentsSummary && agentFilter === 'all' && (
                      <span className="hidden shrink-0 text-[11.5px] text-[var(--text-tertiary)] lg:inline">
                        {agentsSummary}
                      </span>
                    )}
                  </div>
                  {subtitle && (
                    <div
                      title={subtitle}
                      className="mono-path truncate text-[11.5px] text-[var(--text-tertiary)]"
                    >
                      {subtitle}
                    </div>
                  )}
                  {(server.activeGlobally || server.activeInProject) && (
                    <div className="text-[11.5px] text-[var(--text-secondary)]">
                      {server.probedAtMs == null
                        ? 'Configured · No successful probe recorded'
                        : `Configured · Last successful probe ${probeDate.format(server.probedAtMs)}`}
                    </div>
                  )}
                </div>

                <div className="flex shrink-0 items-center gap-2">
                  <span className="tabular-nums text-[12px] text-[var(--text-secondary)]">
                    {server.isEstimated
                      ? `~${formatTokensExact(server.upfrontTokens)} tok (est.)`
                      : server.toolCount !== null
                        ? `${formatTokensExact(server.upfrontTokens)} tok (${server.toolCount} tools)`
                        : `${formatTokensExact(server.upfrontTokens)} tok`}
                  </span>

                  <button
                    type="button"
                    disabled={!canProbe || isProbing || isBusy}
                    aria-label={`Probe tools schema for ${server.name}`}
                    title={
                      !canProbe
                        ? 'Only local stdio MCP servers can be measured automatically'
                        : isProbing
                          ? 'Measuring startup tool schema tokens…'
                          : server.toolCount !== null
                            ? `Re-measure startup tokens (${server.toolCount} tools, ${formatTokensExact(server.upfrontTokens)} tok)`
                            : 'Measure exact startup token cost by querying this server’s tool list (5s timeout)'
                    }
                    onClick={() => onProbeMcp(server)}
                    className={[
                      'inline-flex h-7 w-7 items-center justify-center rounded-[6px] transition-opacity duration-150',
                      canProbe
                        ? isProbing
                          ? 'animate-pulse text-[var(--accent)]'
                          : 'text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]'
                        : 'cursor-not-allowed text-[var(--text-tertiary)] opacity-45',
                    ].join(' ')}
                  >
                    <Gauge className="h-3.5 w-3.5" strokeWidth={1.75} />
                  </button>

                  {isInheritedGlobal ? (
                    <span
                      title="Active globally across projects"
                      className="inline-flex w-9 items-center justify-center text-[11.5px] font-medium text-[var(--accent)]"
                    >
                      Global
                    </span>
                  ) : (
                    <MacSwitch
                      checked={isChecked}
                      disabled={isBusy}
                      onChange={(next) => onToggleMcp(server, next)}
                      label={`Toggle MCP server ${server.name}`}
                      tooltip={
                        server.definedGloballyCollision && isChecked
                          ? 'Disable redundant project override (falls back to global config)'
                          : undefined
                      }
                    />
                  )}
                </div>
              </div>
            );
          })}
        </div>
      )}
    </section>
  );
}

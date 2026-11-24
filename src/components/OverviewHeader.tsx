import { FolderOpen } from 'lucide-react';
import type {
  ActiveAgents,
  AgentFilter,
  ScopeSnapshot,
  TokenBudgetBreakdown,
} from '@/types';
import { formatTokensExact } from '@/lib/format';

interface OverviewHeaderProps {
  snapshot: ScopeSnapshot;
  budget: TokenBudgetBreakdown;
  activeAgents: ActiveAgents;
  agentFilter: AgentFilter;
  onAgentFilterChange: (filter: AgentFilter) => void;
  onRevealPath: (path: string) => void;
}

export function OverviewHeader({
  snapshot,
  budget,
  activeAgents,
  agentFilter,
  onAgentFilterChange,
  onRevealPath,
}: OverviewHeaderProps) {
  const filterOptions: Array<{ id: AgentFilter; label: string; enabled: boolean }> = [
    { id: 'all', label: 'All', enabled: true },
    { id: 'antigravity', label: 'Antigravity', enabled: activeAgents.antigravity },
    { id: 'codex', label: 'Codex', enabled: activeAgents.codex },
    { id: 'claude', label: 'Claude', enabled: activeAgents.claude },
  ];

  const isProject = snapshot.scopeType === 'project';

  return (
    <header className="sticky top-0 z-10 border-b border-[var(--hairline)] bg-[var(--bg-content)]">
      {/* Top Drag & Title Row */}
      <div
        data-tauri-drag-region
        className="flex h-11 items-center justify-between gap-4 px-6"
      >
        <div className="flex min-w-0 items-center gap-2">
          <h1
            data-tauri-drag-region
            className="truncate text-[15px] font-semibold tracking-[-0.01em] text-[var(--text-primary)]"
          >
            {snapshot.title}
          </h1>
          {snapshot.path && (
            <button
              type="button"
              onClick={() => snapshot.path && onRevealPath(snapshot.path)}
              aria-label={`Reveal ${snapshot.title} in Finder`}
              title={`Reveal in Finder: ${snapshot.path}`}
              className="inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-[6px] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
            >
              <FolderOpen className="h-3.5 w-3.5" strokeWidth={1.75} />
            </button>
          )}
        </div>

        {/* Segmented Agent Filter */}
        <div
          role="group"
          aria-label="Filter by agent"
          className="inline-flex shrink-0 items-center rounded-[7px] bg-[var(--bg-subtle)] p-[2px]"
        >
          {filterOptions
            .filter((opt) => opt.enabled || agentFilter === opt.id)
            .map((opt) => {
              const active = agentFilter === opt.id;
              return (
                <button
                  key={opt.id}
                  type="button"
                  aria-pressed={active}
                  onClick={() => onAgentFilterChange(opt.id)}
                  className={[
                    'h-6 rounded-[5px] px-2.5 text-[12px] font-medium transition-opacity duration-150',
                    active
                      ? 'bg-[var(--bg-content)] text-[var(--text-primary)] shadow-[0_1px_2px_rgba(0,0,0,0.08)]'
                      : 'text-[var(--text-secondary)] hover:text-[var(--text-primary)]',
                  ].join(' ')}
                >
                  {opt.label}
                </button>
              );
            })}
        </div>
      </div>

      {/* Horizontal Token Budget Sub-bar */}
      <div className="grid grid-cols-1 gap-5 px-6 pt-1 pb-3.5 md:grid-cols-2">
        <BudgetBar
          label="Rules"
          total={budget.rulesTotal}
          limit={budget.rulesLimit}
          globalTokens={budget.rulesGlobal}
          projectTokens={budget.rulesProject}
          isProject={isProject}
        />

        <BudgetBar
          label="Skills & MCP"
          total={budget.skillsMcpTotal}
          limit={budget.skillsMcpLimit}
          globalTokens={budget.skillsMcpGlobal}
          projectTokens={budget.skillsMcpProject}
          isProject={isProject}
          extraRows={[
            {
              label: 'Skills',
              tokens: budget.skillsGlobal + budget.skillsProject,
            },
            {
              label: 'MCP Servers',
              tokens: budget.mcpGlobal + budget.mcpProject,
            },
          ]}
        />
      </div>
    </header>
  );
}

interface BudgetBarProps {
  label: string;
  total: number;
  limit: number;
  globalTokens: number;
  projectTokens: number;
  isProject: boolean;
  extraRows?: Array<{ label: string; tokens: number }>;
}

function BudgetBar({
  label,
  total,
  limit,
  globalTokens,
  projectTokens,
  isProject,
  extraRows,
}: BudgetBarProps) {
  const ratio = limit > 0 ? Math.min(total / limit, 1) : 0;
  const pct = limit > 0 ? Math.round((total / limit) * 100) : 0;

  const toneColor =
    pct > 85
      ? 'var(--status-red)'
      : pct >= 60
        ? 'var(--status-amber)'
        : 'var(--status-green)';

  const toneLabel = pct > 85 ? 'High' : pct >= 60 ? 'Elevated' : 'Healthy';

  return (
    <div className="group relative" tabIndex={0}>
      <div className="mb-1.5 flex items-baseline justify-between gap-2 text-[12px]">
        <span className="font-medium text-[var(--text-secondary)]">{label}</span>
        <span className="tabular-nums text-[var(--text-primary)]">
          <span className="font-semibold">{formatTokensExact(total)}</span>
          <span className="text-[var(--text-tertiary)]">
            {' '}
            / {formatTokensExact(limit)} tok
          </span>
          <span
            className="ml-2 text-[11.5px] font-medium"
            style={{ color: toneColor }}
          >
            {pct}% ({toneLabel})
          </span>
        </span>
      </div>

      <div
        role="progressbar"
        aria-label={`${label} startup token budget`}
        aria-valuenow={total}
        aria-valuemin={0}
        aria-valuemax={limit}
        className="h-[6px] w-full overflow-hidden rounded-full bg-[var(--bg-subtle)]"
      >
        <div
          className="h-full w-full origin-left rounded-full transition-transform duration-200 ease-[cubic-bezier(0.16,1,0.3,1)]"
          style={{
            backgroundColor: toneColor,
            transform: `scaleX(${ratio})`,
          }}
        />
      </div>

      {/* Hover / Focus Breakdown Popover */}
      <div className="pointer-events-none invisible absolute left-0 top-full z-20 mt-2 w-64 rounded-[8px] border border-[var(--hairline)] bg-[var(--bg-elevated)] p-2.5 text-[12px] opacity-0 shadow-[0_6px_20px_rgba(0,0,0,0.12)] transition-opacity duration-150 group-hover:visible group-hover:opacity-100 group-focus-visible:visible group-focus-visible:opacity-100">
        <div className="flex justify-between py-0.5">
          <span className="text-[var(--text-secondary)]">Global Baseline</span>
          <span className="tabular-nums font-medium text-[var(--text-primary)]">
            {formatTokensExact(globalTokens)} tok
          </span>
        </div>
        {isProject && (
          <div className="flex justify-between py-0.5">
            <span className="text-[var(--text-secondary)]">Project Additions</span>
            <span className="tabular-nums font-medium text-[var(--text-primary)]">
              +{formatTokensExact(projectTokens)} tok
            </span>
          </div>
        )}
        {extraRows && extraRows.length > 0 && (
          <div className="mt-1 space-y-0.5 border-t border-[var(--hairline)] pt-1">
            {extraRows.map((row) => (
              <div
                key={row.label}
                className="flex justify-between text-[11.5px] text-[var(--text-tertiary)]"
              >
                <span>{row.label}</span>
                <span className="tabular-nums">{formatTokensExact(row.tokens)} tok</span>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

import { useState } from 'react';
import { Eye, EyeOff, FolderOpen, Pin, Tag } from 'lucide-react';
import type {
  ActiveAgents,
  AgentFilter,
  ProjectSummary,
  ScopeSnapshot,
  TagDefinition,
  TokenBudgetBreakdown,
} from '@/types';
import { formatTokensExact, tagColorHex } from '@/lib/format';
import { ProjectTagPopover } from '@/components/ProjectTagPopover';

interface OverviewHeaderProps {
  snapshot: ScopeSnapshot;
  budget: TokenBudgetBreakdown;
  showEffectiveContext: boolean;
  onChangeContextView: (show: boolean) => void;
  activeAgents: ActiveAgents;
  agentFilter: AgentFilter;
  onAgentFilterChange: (filter: AgentFilter) => void;
  onRevealPath: (path: string) => void;
  selectedProject: ProjectSummary | null;
  tagDefinitions: TagDefinition[];
  manualTagIds: string[];
  mutedAutoTagIds: string[];
  isPinned?: boolean;
  isPinBusy?: boolean;
  onTogglePin?: () => void;
  isHidden?: boolean;
  isHideBusy?: boolean;
  onToggleHide?: () => void;
  onSaveProjectTags?: (
    manualTagIds: string[],
    mutedAutoTagIds: string[],
  ) => Promise<void>;
  onCreateAndAttachTag?: (tag: TagDefinition) => Promise<void>;
  onOpenTagSettings?: () => void;
}

export function OverviewHeader({
  snapshot,
  budget,
  showEffectiveContext,
  onChangeContextView,
  activeAgents,
  agentFilter,
  onAgentFilterChange,
  onRevealPath,
  selectedProject,
  tagDefinitions,
  manualTagIds,
  mutedAutoTagIds,
  isPinned = false,
  isPinBusy = false,
  onTogglePin,
  isHidden = false,
  isHideBusy = false,
  onToggleHide,
  onSaveProjectTags,
  onCreateAndAttachTag,
  onOpenTagSettings,
}: OverviewHeaderProps) {
  const [tagPopoverOpen, setTagPopoverOpen] = useState(false);

  const filterOptions: Array<{ id: AgentFilter; label: string; enabled: boolean }> = [
    { id: 'all', label: 'All', enabled: true },
    { id: 'antigravity', label: 'Antigravity', enabled: activeAgents.antigravity },
    { id: 'codex', label: 'Codex', enabled: activeAgents.codex },
    { id: 'claude', label: 'Claude', enabled: activeAgents.claude },
  ];

  const isProject = snapshot.scopeType === 'project';

  return (
    <header className="sticky top-0 z-10 border-b border-[var(--hairline)] bg-[var(--bg-toolbar)] backdrop-blur-xl backdrop-saturate-150">
      {/* Top Drag & Title Row (48px aligned with sidebar header) */}
      <div
        data-tauri-drag-region
        className="flex h-12 items-center justify-between gap-4 px-6"
      >
        <div className="flex min-w-0 items-center gap-1.5">
          <h1
            data-tauri-drag-region
            className="truncate text-[15px] font-semibold tracking-[-0.015em] text-[var(--text-primary)]"
          >
            {snapshot.title}
          </h1>
          {snapshot.path && (
            <button
              type="button"
              onClick={() => snapshot.path && onRevealPath(snapshot.path)}
              aria-label={`Reveal ${snapshot.title} in Finder`}
              title={`Reveal in Finder: ${snapshot.path}`}
              className="inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-[6px] text-[var(--text-secondary)] transition-colors duration-100 hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
            >
              <FolderOpen className="h-3.5 w-3.5" strokeWidth={1.75} />
            </button>
          )}
          {isProject && onTogglePin && (
            <button
              type="button"
              disabled={isPinBusy}
              onClick={onTogglePin}
              aria-pressed={isPinned}
              aria-label={isPinned ? 'Unpin project' : 'Pin project'}
              title={isPinned ? 'Unpin from top of sidebar' : 'Pin to top of sidebar'}
              className={[
                'inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-[6px] transition-colors duration-100',
                isPinned
                  ? 'text-[var(--accent)] hover:bg-[var(--bg-hover)]'
                  : 'text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]',
                isPinBusy ? 'cursor-wait opacity-50' : '',
              ].join(' ')}
            >
              <Pin className="h-3.5 w-3.5" strokeWidth={1.75} />
            </button>
          )}
          {isProject && onToggleHide && (
            <button
              type="button"
              disabled={isHideBusy}
              onClick={onToggleHide}
              aria-pressed={isHidden}
              aria-label={isHidden ? 'Unhide project' : 'Hide project'}
              title={
                isHidden
                  ? 'Restore project to sidebar list'
                  : 'Hide project into Hidden section'
              }
              className={[
                'inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-[6px] transition-colors duration-100',
                isHidden
                  ? 'text-[var(--accent)] hover:bg-[var(--bg-hover)]'
                  : 'text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]',
                isHideBusy ? 'cursor-wait opacity-50' : '',
              ].join(' ')}
            >
              {isHidden ? (
                <Eye className="h-3.5 w-3.5" strokeWidth={1.75} />
              ) : (
                <EyeOff className="h-3.5 w-3.5" strokeWidth={1.75} />
              )}
            </button>
          )}

          {/* Project Tags & Tag Picker */}
          {isProject &&
            selectedProject &&
            onSaveProjectTags &&
            onCreateAndAttachTag &&
            onOpenTagSettings && (
              <div className="relative flex min-w-0 items-center gap-2 pl-1.5">
                {selectedProject.tags.map((tag) => (
                  <span
                    key={tag.id}
                    title={
                      tag.isAuto && tag.sourceFolder
                        ? `Auto-tag from folder '${tag.sourceFolder}'`
                        : 'Custom tag'
                    }
                    className="inline-flex shrink-0 items-center gap-1.5 text-[11.5px] text-[var(--text-secondary)]"
                  >
                    <span
                      className="h-2 w-2 rounded-full"
                      style={{ backgroundColor: tagColorHex(tag.color) }}
                      aria-hidden="true"
                    />
                    <span className="max-w-[110px] truncate">{tag.label}</span>
                  </span>
                ))}

                <button
                  type="button"
                  onClick={() => setTagPopoverOpen((prev) => !prev)}
                  aria-expanded={tagPopoverOpen}
                  aria-label="Edit project tags"
                  title="Manage tags for this project"
                  className="inline-flex h-6 shrink-0 items-center gap-1 rounded-[5px] px-1.5 text-[11.5px] font-medium text-[var(--text-secondary)] transition-colors duration-100 hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
                >
                  <Tag className="h-3 w-3" strokeWidth={1.75} />
                  <span>{selectedProject.tags.length === 0 ? 'Add Tag' : 'Tags'}</span>
                </button>

                <ProjectTagPopover
                  open={tagPopoverOpen}
                  project={selectedProject}
                  tagDefinitions={tagDefinitions}
                  manualTagIds={manualTagIds}
                  mutedAutoTagIds={mutedAutoTagIds}
                  onClose={() => setTagPopoverOpen(false)}
                  onSaveProjectTags={onSaveProjectTags}
                  onCreateAndAttachTag={onCreateAndAttachTag}
                  onOpenTagSettings={onOpenTagSettings}
                />
              </div>
            )}
        </div>

        {/* Segmented Agent Filter */}
        <div
          role="group"
          aria-label="Filter by agent"
          className="inline-flex shrink-0 items-center rounded-[7px] bg-[var(--bg-field)] p-[2px] shadow-[inset_0_0.5px_1px_rgba(0,0,0,0.06)]"
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
                    'h-[24px] rounded-[5px] px-2.5 text-[12px] font-medium transition-colors duration-100',
                    active
                      ? 'bg-[var(--bg-control)] text-[var(--text-primary)] shadow-[var(--control-shadow)]'
                      : 'text-[var(--text-secondary)] hover:text-[var(--text-primary)]',
                  ].join(' ')}
                >
                  {opt.label}
                </button>
              );
            })}
        </div>
      </div>

      <div role="group" aria-label="Context view" className="flex gap-1.5 px-6 pb-2">
        <button type="button" className="mac-btn h-7 px-2.5 text-[12px] aria-pressed:border-[var(--accent)] aria-pressed:text-[var(--accent)]" aria-pressed={!showEffectiveContext}
          onClick={() => onChangeContextView(false)}>Switchboard</button>
        <button type="button" className="mac-btn h-7 px-2.5 text-[12px] aria-pressed:border-[var(--accent)] aria-pressed:text-[var(--accent)]" aria-pressed={showEffectiveContext}
          onClick={() => onChangeContextView(true)}>Effective Context</button>
      </div>
      {/* Horizontal Token Budget Sub-bar */}
      {!showEffectiveContext && <div className="grid grid-cols-1 gap-5 px-6 pt-1 pb-3.5 md:grid-cols-2">
        <BudgetBar
          label="Rules"
          total={budget.rulesTotal}
          limit={agentFilter === 'antigravity' ? budget.rulesLimit : 0}
          globalTokens={budget.rulesGlobal}
          projectTokens={budget.rulesProject}
          isProject={isProject}
        />

        <BudgetBar
          label={
            snapshot.plugins.length > 0
              ? 'Skills, Plugins & MCP'
              : 'Skills & MCP'
          }
          total={budget.skillsMcpTotal}
          limit={agentFilter === 'antigravity' ? budget.skillsMcpLimit : 0}
          globalTokens={budget.skillsMcpGlobal}
          projectTokens={budget.skillsMcpProject}
          isProject={isProject}
          extraRows={[
            {
              label: 'Skills',
              tokens: budget.skillsGlobal + budget.skillsProject,
            },
            ...(snapshot.plugins.length > 0 || budget.pluginsGlobal > 0
              ? [
                  {
                    label: 'Plugins',
                    tokens: budget.pluginsGlobal,
                  },
                ]
              : []),
            {
              label: 'MCP Servers',
              tokens: budget.mcpGlobal + budget.mcpProject,
            },
          ]}
        />
      </div>}
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
    <div
      className="group relative"
      tabIndex={0}
      title="Estimated startup tokens based on the scanned configuration."
    >
      <div className="mb-1.5 flex items-baseline justify-between gap-2 text-[12px]">
        <span className="font-medium text-[var(--text-secondary)]">{label}</span>
        <span className="tabular-nums text-[var(--text-primary)]">
          <span className="font-semibold">~{formatTokensExact(total)}</span>
          {limit === 0 && <span className="text-[var(--text-tertiary)]"> tok</span>}
          {limit > 0 && <>
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
          </>}
        </span>
      </div>

      {limit > 0 && <div
        role="progressbar"
        aria-label={`${label} startup token budget`}
        aria-valuenow={Math.min(total, limit)}
        aria-valuemin={0}
        aria-valuemax={limit}
        className="h-[5px] w-full overflow-hidden rounded-full bg-[var(--bg-field)]"
      >
        <div
          className="h-full w-full origin-left rounded-full transition-transform duration-200 ease-[cubic-bezier(0.16,1,0.3,1)]"
          style={{
            backgroundColor: toneColor,
            transform: `scaleX(${ratio})`,
          }}
        />
      </div>}

      {/* Hover / Focus Breakdown Popover */}
      <div className="pointer-events-none invisible absolute left-0 top-full z-20 mt-2 w-64 rounded-[10px] bg-[var(--bg-popover)] p-2.5 text-[12px] opacity-0 shadow-[0_10px_28px_rgba(0,0,0,0.18),0_0_0_0.5px_var(--hairline)] backdrop-blur-2xl transition-opacity duration-150 group-hover:visible group-hover:opacity-100 group-focus-visible:visible group-focus-visible:opacity-100">
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

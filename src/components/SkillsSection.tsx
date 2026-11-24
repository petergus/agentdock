import { useMemo, useState } from 'react';
import {
  AlertTriangle,
  ExternalLink,
  Search,
  Sparkles,
  Unlink,
} from 'lucide-react';
import type { AgentFilter, ScopeType, SkillItem } from '@/types';
import {
  agentDisplayName,
  formatTokensExact,
  matchesAgentFilter,
  skillSourceLabel,
} from '@/lib/format';
import { MacSwitch } from '@/components/ui/MacSwitch';

type SortMode = 'name' | 'tokens' | 'active';

interface SkillsSectionProps {
  scopeType: ScopeType;
  skills: SkillItem[];
  unmanagedGlobalSkillsCount: number;
  agentFilter: AgentFilter;
  busySkillIds: Record<string, boolean>;
  onOpenFile: (path: string) => void;
  onToggleSkill: (skill: SkillItem, next: boolean) => void;
  onUnlinkBrokenSkill: (skill: SkillItem) => void;
  onOpenDeBloat: (skillIds: string[]) => void;
}

export function SkillsSection({
  scopeType,
  skills,
  unmanagedGlobalSkillsCount,
  agentFilter,
  busySkillIds,
  onOpenFile,
  onToggleSkill,
  onUnlinkBrokenSkill,
  onOpenDeBloat,
}: SkillsSectionProps) {
  const [filterText, setFilterText] = useState('');
  const [sortMode, setSortMode] = useState<SortMode>('active');

  const unmanagedIds = useMemo(
    () =>
      skills
        .filter((s) => s.activeGlobally && s.isUnmanagedGlobal)
        .map((s) => s.id),
    [skills],
  );

  const visibleSkills = useMemo(() => {
    const q = filterText.trim().toLowerCase();
    const list = skills.filter((s) => {
      if (!matchesAgentFilter(s.activeAgents, agentFilter, true)) {
        return false;
      }
      if (!q) {
        return true;
      }
      return (
        s.name.toLowerCase().includes(q) ||
        s.id.toLowerCase().includes(q) ||
        s.description.toLowerCase().includes(q)
      );
    });

    list.sort((a, b) => {
      if (sortMode === 'tokens') {
        return (
          b.upfrontTokens - a.upfrontTokens ||
          a.name.toLowerCase().localeCompare(b.name.toLowerCase())
        );
      }
      if (sortMode === 'active') {
        const aOn =
          scopeType === 'global'
            ? a.activeGlobally
            : a.activeGlobally || a.activeInProject;
        const bOn =
          scopeType === 'global'
            ? b.activeGlobally
            : b.activeGlobally || b.activeInProject;
        if (aOn !== bOn) {
          return aOn ? -1 : 1;
        }
      }
      return a.name.toLowerCase().localeCompare(b.name.toLowerCase());
    });

    return list;
  }, [skills, agentFilter, filterText, sortMode, scopeType]);

  return (
    <section aria-labelledby="skills-heading" className="space-y-3">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex items-baseline gap-2.5">
          <h2
            id="skills-heading"
            className="text-[14px] font-semibold tracking-[-0.01em] text-[var(--text-primary)]"
          >
            Skills
          </h2>
          <span className="tabular-nums text-[12px] text-[var(--text-tertiary)]">
            {visibleSkills.length}
          </span>
        </div>

        <div className="flex items-center gap-2">
          <div className="relative flex items-center">
            <Search
              className="pointer-events-none absolute left-2 h-3.5 w-3.5 text-[var(--text-tertiary)]"
              strokeWidth={1.75}
              aria-hidden="true"
            />
            <input
              type="search"
              value={filterText}
              onChange={(e) => setFilterText(e.target.value)}
              placeholder="Filter skills…"
              aria-label="Filter skills"
              className="h-7 w-44 rounded-[6px] border border-[var(--hairline)] bg-[var(--bg-subtle)] pl-7 pr-2.5 text-[12px] text-[var(--text-primary)] placeholder:text-[var(--text-tertiary)]"
            />
          </div>

          <label className="sr-only" htmlFor="skills-sort">
            Sort skills
          </label>
          <select
            id="skills-sort"
            value={sortMode}
            onChange={(e) => setSortMode(e.target.value as SortMode)}
            className="h-7 rounded-[6px] border border-[var(--hairline)] bg-[var(--bg-subtle)] px-2 text-[12px] text-[var(--text-primary)]"
          >
            <option value="active">Active First</option>
            <option value="name">Name (A–Z)</option>
            <option value="tokens">Highest Tokens</option>
          </select>
        </div>
      </div>

      {/* Global De-bloat Banner */}
      {scopeType === 'global' && unmanagedGlobalSkillsCount > 0 && (
        <div className="flex items-center justify-between gap-4 rounded-[8px] border border-[var(--hairline)] bg-[var(--bg-elevated)] px-3.5 py-2.5">
          <div className="flex min-w-0 items-start gap-2.5">
            <AlertTriangle
              className="mt-0.5 h-4 w-4 shrink-0 text-[var(--status-amber)]"
              strokeWidth={1.75}
              aria-hidden="true"
            />
            <div className="text-[12.5px] leading-snug text-[var(--text-primary)]">
              <span className="font-medium">
                {unmanagedGlobalSkillsCount} unmanaged global{' '}
                {unmanagedGlobalSkillsCount === 1 ? 'skill folder' : 'skill folders'} detected
              </span>{' '}
              <span className="text-[var(--text-secondary)]">
                sitting directly inside agent directories instead of your Obsidian Library.
              </span>
            </div>
          </div>
          <button
            type="button"
            onClick={() => onOpenDeBloat(unmanagedIds)}
            className="h-7 shrink-0 rounded-[6px] border border-[var(--hairline)] bg-[var(--bg-content)] px-3 text-[12px] font-medium text-[var(--text-primary)] hover:bg-[var(--bg-hover)]"
          >
            Move to Library & Unlink…
          </button>
        </div>
      )}

      {visibleSkills.length === 0 ? (
        <div className="rounded-[10px] border border-[var(--hairline)] bg-[var(--bg-elevated)] px-4 py-6 text-center text-[12.5px] text-[var(--text-secondary)]">
          <Sparkles
            className="mx-auto mb-1.5 h-4 w-4 text-[var(--text-tertiary)]"
            strokeWidth={1.6}
            aria-hidden="true"
          />
          No skills match the active agent or search filter.
        </div>
      ) : (
        <div className="overflow-hidden rounded-[10px] border border-[var(--hairline)] bg-[var(--bg-content)]">
          {visibleSkills.map((skill, idx) => {
            const isBroken = skill.source.kind === 'brokenSymlink';
            const isInheritedGlobal =
              scopeType === 'project' && skill.activeGlobally;
            const isChecked =
              scopeType === 'global'
                ? skill.activeGlobally
                : skill.activeInProject;
            const isBusy = Boolean(busySkillIds[skill.id]);
            const sourceText = skillSourceLabel(skill);
            const agentsSummary =
              skill.activeAgents.length > 0
                ? skill.activeAgents.map(agentDisplayName).join(', ')
                : null;

            const tokenTooltip = `Upfront: ${formatTokensExact(skill.upfrontTokens)} tok | On-demand body: ${formatTokensExact(skill.onDemandTokens)} tok${skill.description ? `\n\n${skill.description}` : ''}`;

            return (
              <div
                key={skill.id}
                className={[
                  'flex min-h-[38px] items-center justify-between gap-3 px-3.5 py-1.5 hover:bg-[var(--bg-hover)]',
                  idx > 0 ? 'border-t border-[var(--hairline)]' : '',
                  isBusy ? 'opacity-60' : '',
                ].join(' ')}
              >
                <div className="flex min-w-0 flex-1 items-center gap-2.5">
                  <span className="truncate text-[13px] font-medium text-[var(--text-primary)]">
                    {skill.name}
                  </span>

                  {skill.descriptionTooLong && (
                    <span
                      title="Skill description exceeds 200 characters (adds upfront prompt overhead)"
                      className="inline-block h-2 w-2 shrink-0 rounded-full bg-[var(--status-amber)]"
                      aria-label="Long description warning"
                    />
                  )}

                  <span
                    className={[
                      'shrink-0 text-[12px]',
                      isBroken
                        ? 'font-medium text-[var(--status-red)]'
                        : skill.isUnmanagedGlobal
                          ? 'text-[var(--status-amber)]'
                          : 'text-[var(--text-tertiary)]',
                    ].join(' ')}
                    title={
                      skill.frontmatterError ??
                      (skill.isUnmanagedGlobal
                        ? 'Unmanaged real folder inside global agent directory'
                        : undefined)
                    }
                  >
                    {sourceText}
                  </span>

                  {agentsSummary && agentFilter === 'all' && (
                    <span className="hidden truncate text-[11.5px] text-[var(--text-tertiary)] lg:inline">
                      {agentsSummary}
                    </span>
                  )}
                </div>

                <div className="flex shrink-0 items-center gap-2.5">
                  <span
                    title={tokenTooltip}
                    className="tabular-nums min-w-[68px] text-right text-[12px] text-[var(--text-secondary)]"
                  >
                    {formatTokensExact(skill.upfrontTokens)} tok
                  </span>

                  <button
                    type="button"
                    disabled={isBroken}
                    onClick={() => onOpenFile(skill.canonicalPath)}
                    aria-label={`Open ${skill.name} in default app`}
                    title={
                      isBroken
                        ? 'Cannot open broken symlink'
                        : `Open SKILL.md: ${skill.canonicalPath}`
                    }
                    className="inline-flex h-7 w-7 items-center justify-center rounded-[6px] text-[var(--text-secondary)] hover:bg-[var(--bg-subtle)] hover:text-[var(--text-primary)] disabled:cursor-not-allowed disabled:opacity-40"
                  >
                    <ExternalLink className="h-3.5 w-3.5" strokeWidth={1.75} />
                  </button>

                  {isBroken ? (
                    <button
                      type="button"
                      disabled={isBusy}
                      onClick={() => onUnlinkBrokenSkill(skill)}
                      title="Remove broken symlink"
                      className="inline-flex h-7 items-center gap-1 rounded-[6px] px-2 text-[12px] font-medium text-[var(--status-red)] hover:bg-[var(--status-red)]/10 disabled:opacity-50"
                    >
                      <Unlink className="h-3 w-3" strokeWidth={1.75} />
                      Unlink
                    </button>
                  ) : isInheritedGlobal ? (
                    <span
                      title="Active globally for all projects"
                      className="inline-flex w-9 items-center justify-center text-[11.5px] font-medium text-[var(--accent)]"
                    >
                      Global
                    </span>
                  ) : (
                    <MacSwitch
                      checked={isChecked}
                      disabled={isBusy}
                      label={`Toggle skill ${skill.name}`}
                      tooltip={
                        isChecked
                          ? `Disable ${skill.name} in this scope`
                          : `Enable ${skill.name} via symlink`
                      }
                      onChange={(next) => onToggleSkill(skill, next)}
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

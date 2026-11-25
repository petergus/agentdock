import { Boxes, Plus, SlidersHorizontal } from 'lucide-react';
import type { ContextSet, McpServerItem, RuleItem, SetOperation, SkillItem } from '@/types';

interface SetsSectionProps {
  sets: ContextSet[];
  activeSetIds: string[];
  projectSkills: SkillItem[];
  projectMcps: McpServerItem[];
  projectRules: RuleItem[];
  onPreviewSet: (set: ContextSet, operation: SetOperation) => void;
  onOpenSaveSet: () => void;
  onManageSets: () => void;
}

export function SetsSection({
  sets,
  activeSetIds,
  projectSkills,
  projectMcps,
  projectRules,
  onPreviewSet,
  onOpenSaveSet,
  onManageSets,
}: SetsSectionProps) {
  const activeSkillSet = new Set(
    projectSkills
      .filter((s) => s.installedPaths.some((entry) => !entry.broken))
      .map((s) => s.id),
  );
  const activeMcpSet = new Set(
    projectMcps
      .filter((m) => m.definitions.some((entry) => entry.enabled))
      .map((m) => m.name),
  );
  const activeRuleSet = new Set(
    projectRules.flatMap((rule) => rule.installations
      .filter((entry) => entry.error === null)
      .map((entry) => entry.path.split('/').at(-1) ?? rule.filename)),
  );

  return (
    <section aria-labelledby="sets-heading" className="space-y-3">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex items-baseline gap-2.5">
          <h2
            id="sets-heading"
            className="text-[14px] font-semibold tracking-[-0.01em] text-[var(--text-primary)]"
          >
            Sets
          </h2>
          <span className="tabular-nums text-[12px] text-[var(--text-tertiary)]">
            {sets.length}
          </span>
        </div>

        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={onOpenSaveSet}
            className="mac-btn inline-flex h-7 items-center gap-1 px-2.5 text-[12px] font-medium"
          >
            <Plus className="h-3.5 w-3.5" strokeWidth={1.75} />
            Save Current as Set
          </button>
          <button
            type="button"
            onClick={onManageSets}
            className="mac-btn inline-flex h-7 items-center gap-1 px-2.5 text-[12px] font-medium text-[var(--text-secondary)]"
          >
            <SlidersHorizontal className="h-3 w-3" strokeWidth={1.75} />
            Manage
          </button>
        </div>
      </div>

      {sets.length === 0 ? (
        <div className="mac-group px-4 py-4.5 text-[12.5px] text-[var(--text-secondary)]">
          <div className="flex items-center gap-2">
            <Boxes
              className="h-4 w-4 shrink-0 text-[var(--text-tertiary)]"
              strokeWidth={1.6}
              aria-hidden="true"
            />
            <span>
              No reusable Sets yet. Group skills, MCP servers, and rules into a Set to preview and apply them on any project.
            </span>
          </div>
        </div>
      ) : (
        <div className="mac-group overflow-hidden">
          {sets.map((set, idx) => {
            const isTrackedActive = activeSetIds.includes(set.id);
            const totalItems =
              set.skillIds.length +
              set.mcpNames.length +
              set.ruleFilenames.length;
            const presentItems =
              set.skillIds.filter((id) => activeSkillSet.has(id)).length +
              set.mcpNames.filter((name) => activeMcpSet.has(name)).length +
              set.ruleFilenames.filter((fn) => activeRuleSet.has(fn)).length;
            const isPartial =
              isTrackedActive && totalItems > 0 && presentItems < totalItems;

            const parts: string[] = [];
            if (set.skillIds.length > 0) {
              parts.push(
                `${set.skillIds.length} ${set.skillIds.length === 1 ? 'skill' : 'skills'}`,
              );
            }
            if (set.mcpNames.length > 0) {
              parts.push(`${set.mcpNames.length} MCP`);
            }
            if (set.ruleFilenames.length > 0) {
              parts.push(
                `${set.ruleFilenames.length} ${set.ruleFilenames.length === 1 ? 'rule' : 'rules'}`,
              );
            }

            return (
              <div
                key={set.id}
                className={[
                  'flex min-h-[40px] items-center justify-between gap-3 px-4 py-1.5 transition-colors duration-100 hover:bg-[var(--bg-hover)]',
                  idx > 0 ? 'mac-row-divider' : '',
                ].join(' ')}
              >
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="truncate text-[13px] font-medium text-[var(--text-primary)]">
                      {set.name}
                    </span>
                    {parts.length > 0 && (
                      <span className="tabular-nums shrink-0 text-[11.5px] text-[var(--text-tertiary)]">
                        {parts.join(' · ')}
                      </span>
                    )}
                    {isPartial && (
                      <button
                        type="button"
                        title="Preview missing items"
                        aria-label={`Preview missing items for ${set.name}, ${presentItems} of ${totalItems} available`}
                        onClick={() => onPreviewSet(set, 'repair')}
                        className="shrink-0 rounded-[4px] text-[11.5px] font-medium text-[var(--status-amber)] underline"
                      >
                        Partial ({presentItems}/{totalItems})
                      </button>
                    )}
                  </div>
                  {set.description && (
                    <div className="truncate text-[11.5px] text-[var(--text-secondary)]">
                      {set.description}
                    </div>
                  )}
                </div>

                {isTrackedActive && !isPartial && (
                  <span className="shrink-0 text-[12px] text-[var(--text-secondary)]">Active</span>
                )}
                <button
                  type="button"
                  className="mac-btn h-7 shrink-0 px-2.5 text-[12px]"
                  aria-label={`Preview ${isTrackedActive ? 'disabling' : 'applying'} ${set.name}`}
                  onClick={() => onPreviewSet(set, isTrackedActive ? 'disable' : 'apply')}
                >
                  {isTrackedActive ? 'Preview Disable' : 'Preview Apply'}
                </button>
              </div>
            );
          })}
        </div>
      )}
    </section>
  );
}

import { useMemo } from 'react';
import {
  AlertTriangle,
  ExternalLink,
  FileText,
  FolderOpen,
  Link2,
  Plus,
  SlidersHorizontal,
  Unlink,
} from 'lucide-react';
import type {
  AgentFilter,
  RuleItem,
  ScopeType,
  SupportDocItem,
} from '@/types';
import {
  agentDisplayName,
  formatTokensExact,
  matchesAgentFilter,
  ruleTriggerLabel,
} from '@/lib/format';
import { MacSwitch } from '@/components/ui/MacSwitch';

interface RulesDocsSectionProps {
  scopeType: ScopeType;
  rules: RuleItem[];
  supportDocs: SupportDocItem[];
  agentFilter: AgentFilter;
  busyRuleIds: Record<string, boolean>;
  onOpenFile: (path: string) => void;
  onRevealPath: (path: string) => void;
  onToggleLibraryRule: (rule: RuleItem, next: boolean) => void;
  onUnlinkBrokenRule: (rule: RuleItem) => void;
  onEditRuleTrigger: (rule: RuleItem) => void;
  onOpenSupportDocsModal: () => void;
  onUnlinkSupportDoc: (doc: SupportDocItem) => void;
}

export function RulesDocsSection({
  scopeType,
  rules,
  supportDocs,
  agentFilter,
  busyRuleIds,
  onOpenFile,
  onRevealPath,
  onToggleLibraryRule,
  onUnlinkBrokenRule,
  onEditRuleTrigger,
  onOpenSupportDocsModal,
  onUnlinkSupportDoc,
}: RulesDocsSectionProps) {
  const visibleRules = useMemo(
    () => rules.filter((r) => matchesAgentFilter(r.activeAgents, agentFilter, true)),
    [rules, agentFilter],
  );

  return (
    <div className="space-y-7">
      {/* Rules List */}
      <section aria-labelledby="rules-heading" className="space-y-3">
        <div className="flex items-baseline gap-2.5">
          <h2
            id="rules-heading"
            className="text-[14px] font-semibold tracking-[-0.01em] text-[var(--text-primary)]"
          >
            Rules
          </h2>
          <span className="tabular-nums text-[12px] text-[var(--text-tertiary)]">
            {visibleRules.length}
          </span>
        </div>

        {visibleRules.length === 0 ? (
          <div className="mac-group px-4 py-6 text-center text-[12.5px] text-[var(--text-secondary)]">
            <FileText
              className="mx-auto mb-1.5 h-4 w-4 text-[var(--text-tertiary)]"
              strokeWidth={1.6}
              aria-hidden="true"
            />
            No instruction or rule files discovered in this scope.
          </div>
        ) : (
          <div className="mac-group overflow-hidden">
            {visibleRules.map((rule, idx) => {
              const isBroken = rule.sourceLabel === 'Broken Symlink';
              const isInheritedGlobal =
                scopeType === 'project' && rule.activeGlobally;
              const isActive =
                scopeType === 'global'
                  ? rule.activeGlobally
                  : rule.activeInProject;
              const isBusy = Boolean(busyRuleIds[rule.id]);
              const triggerText = ruleTriggerLabel(rule.trigger);
              const triggerHelp =
                rule.trigger === 'alwaysOn'
                  ? 'Loads 100% of this rule into every startup prompt.'
                  : rule.trigger === 'modelDecision'
                    ? 'Only the short description loads upfront; the full rule loads on demand.'
                    : rule.trigger === 'glob'
                      ? '0 tokens upfront — loads only when editing matching files.'
                      : 'Invalid or missing YAML frontmatter trigger.';
              const agentsSummary = rule.activeAgents
                .map(agentDisplayName)
                .join(', ');

              return (
                <div
                  key={rule.id}
                  className={[
                    'flex min-h-[38px] items-center justify-between gap-3 px-4 py-1.5 transition-colors duration-100 hover:bg-[var(--bg-hover)]',
                    idx > 0 ? 'mac-row-divider' : '',
                    isBusy ? 'opacity-60' : '',
                  ].join(' ')}
                >
                  <div className="flex min-w-0 flex-1 items-center gap-2.5">
                    <span className="truncate text-[13px] font-medium text-[var(--text-primary)]">
                      {rule.filename}
                    </span>

                    {rule.isEditableTrigger && !isBroken ? (
                      <button
                        type="button"
                        onClick={() => onEditRuleTrigger(rule)}
                        title={`${triggerHelp} Click to change trigger mode.`}
                        className={[
                          'mac-btn inline-flex h-6 shrink-0 items-center gap-1 px-2 text-[11.5px] font-medium',
                          rule.trigger === 'invalid'
                            ? 'text-[var(--status-red)]'
                            : 'text-[var(--text-primary)]',
                        ].join(' ')}
                      >
                        <SlidersHorizontal
                          className="h-3 w-3 text-[var(--text-tertiary)]"
                          strokeWidth={1.75}
                        />
                        <span>{triggerText}</span>
                      </button>
                    ) : (
                      <span
                        title={triggerHelp}
                        className={[
                          'shrink-0 text-[12px]',
                          rule.trigger === 'invalid'
                            ? 'font-medium text-[var(--status-red)]'
                            : 'text-[var(--text-secondary)]',
                        ].join(' ')}
                      >
                        {triggerText}
                      </span>
                    )}

                    {rule.frontmatterError && (
                      <span
                        title={rule.frontmatterError}
                        className="inline-flex shrink-0 items-center text-[var(--status-amber)]"
                      >
                        <AlertTriangle className="h-3.5 w-3.5" strokeWidth={1.75} />
                      </span>
                    )}

                    <span className="shrink-0 text-[12px] text-[var(--text-tertiary)]">
                      {isBroken ? 'Broken Link' : rule.sourceLabel}
                    </span>

                    {agentsSummary && agentFilter === 'all' && (
                      <span className="hidden truncate text-[11.5px] text-[var(--text-tertiary)] lg:inline">
                        {agentsSummary}
                      </span>
                    )}
                  </div>

                  <div className="flex shrink-0 items-center gap-2.5">
                    <span
                      title={
                        rule.trigger === 'modelDecision'
                          ? `Upfront description: ${formatTokensExact(rule.upfrontTokens)} tok | Full file: ${formatTokensExact(rule.fullTokens)} tok`
                          : `Full file: ${formatTokensExact(rule.fullTokens)} tok`
                      }
                      className="tabular-nums min-w-[68px] text-right text-[12px] text-[var(--text-secondary)]"
                    >
                      {formatTokensExact(rule.upfrontTokens)} tok
                    </span>

                    <button
                      type="button"
                      disabled={isBroken}
                      onClick={() => onOpenFile(rule.canonicalPath)}
                      aria-label={`Open ${rule.filename} in default app`}
                      title={
                        isBroken
                          ? 'Cannot open broken symlink'
                          : `Open in default app: ${rule.canonicalPath}`
                      }
                      className="inline-flex h-7 w-7 items-center justify-center rounded-[6px] text-[var(--text-secondary)] hover:bg-[var(--bg-subtle)] hover:text-[var(--text-primary)] disabled:cursor-not-allowed disabled:opacity-40"
                    >
                      <ExternalLink className="h-3.5 w-3.5" strokeWidth={1.75} />
                    </button>

                    {isBroken ? (
                      <button
                        type="button"
                        disabled={isBusy}
                        onClick={() => onUnlinkBrokenRule(rule)}
                        title="Remove dead rule shortcut (deletes only the broken link, never real files)"
                        className="inline-flex h-7 items-center gap-1 rounded-[6px] px-2 text-[12px] font-medium text-[var(--status-red)] hover:bg-[var(--status-red)]/10 disabled:opacity-50"
                      >
                        <Unlink className="h-3 w-3" strokeWidth={1.75} />
                        Remove Link
                      </button>
                    ) : isInheritedGlobal ? (
                      <span
                        title="Active globally across projects"
                        className="inline-flex w-9 items-center justify-center text-[11.5px] font-medium text-[var(--accent)]"
                      >
                        Global
                      </span>
                    ) : rule.isLibraryRule || rule.isSymlink ? (
                      <MacSwitch
                        checked={isActive}
                        disabled={isBusy}
                        label={`Toggle library rule ${rule.filename}`}
                        tooltip={
                          isActive
                            ? `Unlink ${rule.filename} from this scope`
                            : `Enable ${rule.filename} via symlink`
                        }
                        onChange={(next) => onToggleLibraryRule(rule, next)}
                      />
                    ) : (
                      <span className="inline-flex w-9 items-center justify-center text-[11.5px] text-[var(--text-tertiary)]">
                        {isActive ? 'Active' : 'Off'}
                      </span>
                    )}
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </section>

      {/* Support Docs (Project Scope Only) */}
      {scopeType === 'project' && (
        <section aria-labelledby="support-docs-heading" className="space-y-3">
          <div className="flex items-center justify-between gap-3">
            <div className="flex items-baseline gap-2.5">
              <h2
                id="support-docs-heading"
                className="text-[14px] font-semibold tracking-[-0.01em] text-[var(--text-primary)]"
              >
                Support Docs
              </h2>
              <span className="tabular-nums text-[12px] text-[var(--text-tertiary)]">
                {supportDocs.length}
              </span>
            </div>

            <button
              type="button"
              onClick={onOpenSupportDocsModal}
              className="mac-btn inline-flex h-7 items-center gap-1 px-2.5 text-[12px] font-medium"
            >
              <Plus className="h-3.5 w-3.5" strokeWidth={1.75} />
              Link Support Docs
            </button>
          </div>

          {supportDocs.length === 0 ? (
            <div className="mac-group px-4 py-5 text-[12.5px] text-[var(--text-secondary)]">
              <div className="flex items-center gap-2">
                <Link2
                  className="h-4 w-4 shrink-0 text-[var(--text-tertiary)]"
                  strokeWidth={1.6}
                  aria-hidden="true"
                />
                <span>
                  No external support docs linked yet. Attach notes, specs, or reference folders via zero-token pointers or local symlinks.
                </span>
              </div>
            </div>
          ) : (
            <div className="mac-group overflow-hidden">
              {supportDocs.map((doc, idx) => (
                <div
                  key={`${doc.mode}:${doc.path}`}
                  className={[
                    'flex min-h-[42px] items-center justify-between gap-3 px-4 py-1.5 transition-colors duration-100 hover:bg-[var(--bg-hover)]',
                    idx > 0 ? 'mac-row-divider' : '',
                  ].join(' ')}
                >
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <span className="truncate text-[13px] font-medium text-[var(--text-primary)]">
                        {doc.name}
                      </span>
                      <span className="shrink-0 text-[12px] text-[var(--text-tertiary)]">
                        {doc.mode === 'pointer' ? 'Pointer' : 'Symlink'}
                      </span>
                      {!doc.exists && (
                        <span className="shrink-0 text-[11.5px] font-medium text-[var(--status-red)]">
                          Missing target
                        </span>
                      )}
                    </div>
                    {doc.description && (
                      <div className="truncate text-[11.5px] text-[var(--text-secondary)]">
                        {doc.description}
                      </div>
                    )}
                    <div className="mono-path truncate text-[11.5px] text-[var(--text-tertiary)]">
                      {doc.path}
                    </div>
                  </div>

                  <div className="flex shrink-0 items-center gap-1.5">
                    <button
                      type="button"
                      disabled={!doc.exists}
                      onClick={() => onRevealPath(doc.path)}
                      aria-label={`Reveal ${doc.name} in Finder`}
                      title={`Reveal in Finder: ${doc.path}`}
                      className="inline-flex h-7 w-7 items-center justify-center rounded-[6px] text-[var(--text-secondary)] hover:bg-[var(--bg-subtle)] hover:text-[var(--text-primary)] disabled:cursor-not-allowed disabled:opacity-40"
                    >
                      <FolderOpen className="h-3.5 w-3.5" strokeWidth={1.75} />
                    </button>

                    <button
                      type="button"
                      onClick={() => onUnlinkSupportDoc(doc)}
                      aria-label={`Unlink ${doc.name}`}
                      title={`Unlink ${doc.name}`}
                      className="inline-flex h-7 items-center gap-1 rounded-[6px] px-2 text-[12px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-subtle)] hover:text-[var(--status-red)]"
                    >
                      <Unlink className="h-3 w-3" strokeWidth={1.75} />
                      Unlink
                    </button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </section>
      )}
    </div>
  );
}

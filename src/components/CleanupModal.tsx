import { type ReactNode, useEffect, useMemo, useRef, useState } from 'react';
import {
  AlertTriangle,
  CheckCircle2,
  ChevronRight,
  ExternalLink,
  FolderCog,
  FolderOpen,
  GitBranch,
  HardDrive,
  HelpCircle,
  Info,
  Layers,
  RefreshCw,
  ShieldCheck,
  Sparkles,
  Trash2,
  Unlink,
  X,
} from 'lucide-react';
import type {
  CleanupAuditReport,
  DuplicateSkillIssue,
  InstructionFixPreview,
  InstructionHygieneIssue,
  SkillItem,
  StraySkillIssue,
  ToggleScope,
} from '@/types';
import { api } from '@/lib/api';
import { agentDisplayName, formatBytes, formatTokensExact } from '@/lib/format';

type AuditPillar = 'all' | 'strays' | 'consolidate' | 'optimize';

interface ConsolidateRequest {
  skillIds: string[];
  scope: ToggleScope;
  scopeLabel?: string;
  skills: Array<Pick<SkillItem, 'id' | 'name' | 'upfrontTokens'>>;
}

interface CleanupModalProps {
  open: boolean;
  report: CleanupAuditReport | null;
  onClose: () => void;
  onRefreshAudit: () => Promise<void>;
  onWorkspaceChanged: () => Promise<void>;
  onOpenConsolidate: (req: ConsolidateRequest) => void;
  onRevealPath: (path: string) => void;
  onOpenFile: (path: string) => void;
}

function brokenLinkKindLabel(kind: string): string {
  switch (kind) {
    case 'skill':
      return 'Skill link';
    case 'rule':
      return 'Rule link';
    case 'supportDoc':
      return 'Support doc link';
    default:
      return 'Broken link';
  }
}

function toConsolidateSummary(
  item: Pick<StraySkillIssue, 'skillId' | 'skillName' | 'upfrontTokens'>,
): Pick<SkillItem, 'id' | 'name' | 'upfrontTokens'> {
  return {
    id: item.skillId,
    name: item.skillName,
    upfrontTokens: item.upfrontTokens,
  };
}

interface AuditSectionProps {
  icon: ReactNode;
  title: string;
  count: number;
  badge?: string;
  whatItIs: string;
  recommendation: string;
  action?: ReactNode;
  children: ReactNode;
}

function HelpTooltip({
  label,
  whatItIs,
  recommendation,
}: {
  label: string;
  whatItIs: string;
  recommendation: string;
}) {
  const [open, setOpen] = useState(false);

  return (
    <span
      className="relative inline-flex items-center"
      onMouseEnter={() => setOpen(true)}
      onMouseLeave={() => setOpen(false)}
    >
      <button
        type="button"
        aria-label={`About ${label}`}
        aria-expanded={open}
        onClick={() => setOpen((prev) => !prev)}
        onFocus={() => setOpen(true)}
        onBlur={() => setOpen(false)}
        className="inline-flex h-4 w-4 items-center justify-center rounded-full text-[var(--text-tertiary)] transition-colors hover:text-[var(--text-primary)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--accent)]"
      >
        <HelpCircle className="h-3.5 w-3.5" strokeWidth={1.75} />
      </button>

      {open && (
        <div
          role="tooltip"
          className="pointer-events-none absolute top-full left-0 z-30 mt-1.5 w-72 rounded-[8px] border border-[var(--hairline)] bg-[var(--bg-popover)] p-2.5 text-left shadow-[var(--sheet-shadow)] backdrop-blur-xl"
        >
          <p className="text-[11.5px] leading-relaxed text-[var(--text-secondary)]">
            {whatItIs}
          </p>
          <p className="mt-1.5 text-[11.5px] font-medium leading-relaxed text-[var(--text-primary)]">
            {recommendation}
          </p>
        </div>
      )}
    </span>
  );
}

function AuditSection({
  icon,
  title,
  count,
  badge,
  whatItIs,
  recommendation,
  action,
  children,
}: AuditSectionProps) {
  return (
    <section className="space-y-2">
      <div className="flex flex-wrap items-center justify-between gap-2 px-0.5">
        <div className="flex min-w-0 flex-1 flex-wrap items-center gap-2">
          <h3 className="flex items-center gap-1.5 text-[12.5px] font-semibold text-[var(--text-primary)]">
            {icon}
            <span>
              {title} ({count})
            </span>
          </h3>
          {badge && (
            <span className="rounded-full bg-[var(--bg-field)] px-2 py-[1px] text-[10.5px] font-medium text-[var(--text-secondary)]">
              {badge}
            </span>
          )}
          <HelpTooltip
            label={title}
            whatItIs={whatItIs}
            recommendation={recommendation}
          />
        </div>

        {action && <div className="shrink-0">{action}</div>}
      </div>

      {children}
    </section>
  );
}

export function CleanupModal({
  open,
  report,
  onClose,
  onRefreshAudit,
  onWorkspaceChanged,
  onOpenConsolidate,
  onRevealPath,
  onOpenFile,
}: CleanupModalProps) {
  const dialogRef = useRef<HTMLDialogElement | null>(null);
  const pressedOutsideRef = useRef(false);

  const [scanning, setScanning] = useState(false);
  const [busyKey, setBusyKey] = useState<string | null>(null);
  const [activePillar, setActivePillar] = useState<AuditPillar>('all');
  const [selectedCachePaths, setSelectedCachePaths] = useState<string[]>([]);
  const [projectStraysExpanded, setProjectStraysExpanded] = useState(false);
  const [confirmBucketPath, setConfirmBucketPath] = useState<string | null>(
    null,
  );
  const [fixPreview, setFixPreview] = useState<InstructionFixPreview | null>(
    null,
  );
  const [draftFixContent, setDraftFixContent] = useState('');
  const [showCurrentContent, setShowCurrentContent] = useState(false);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) {
      return;
    }
    if (open) {
      setErrorMsg(null);
      setStatusMessage(null);
      setConfirmBucketPath(null);
      setFixPreview(null);
      setDraftFixContent('');
      setShowCurrentContent(false);
      setActivePillar('all');
      if (!dialog.open) {
        dialog.showModal();
      }
      setScanning(true);
      void onRefreshAudit().finally(() => setScanning(false));
    } else if (dialog.open) {
      dialog.close();
    }
  }, [open, onRefreshAudit]);

  useEffect(() => {
    if (report) {
      setSelectedCachePaths(report.stalePluginCaches.map((c) => c.path));
      const hasUrgent = report.totalIssues > 0;
      setProjectStraysExpanded(
        !hasUrgent || report.strayProjectSkills.length <= 4,
      );
    }
  }, [report]);

  const selectedCacheBytes = useMemo(() => {
    if (!report) {
      return 0;
    }
    const selectedSet = new Set(selectedCachePaths);
    return report.stalePluginCaches
      .filter((c) => selectedSet.has(c.path))
      .reduce((acc, c) => acc + c.sizeBytes, 0);
  }, [report, selectedCachePaths]);

  const counts = useMemo(() => {
    if (!report) {
      return {
        strays: 0,
        consolidate: 0,
        optimize: 0,
        saveableTokens: 0,
        hasAnything: false,
      };
    }
    const strays =
      report.brokenSymlinks.length +
      report.stalePluginCaches.length +
      report.staleGitExcludes.length +
      report.claudeSyncedBuckets.length;
    const consolidate =
      report.strayGlobalSkills.length +
      report.duplicateSkills.length +
      report.strayProjectSkills.length;
    const optimize = report.instructionHygiene.length;
    const saveableTokens =
      report.strayGlobalSkills.reduce((acc, s) => acc + s.upfrontTokens, 0) +
      report.duplicateSkills.reduce((acc, d) => acc + d.duplicateTokens, 0);
    return {
      strays,
      consolidate,
      optimize,
      saveableTokens,
      hasAnything: strays + consolidate + optimize > 0,
    };
  }, [report]);

  const isOutsideDialog = (e: React.PointerEvent<HTMLDialogElement>) => {
    const dialog = dialogRef.current;
    if (!dialog) {
      return false;
    }
    const r = dialog.getBoundingClientRect();
    return (
      e.clientX < r.left ||
      e.clientX > r.right ||
      e.clientY < r.top ||
      e.clientY > r.bottom
    );
  };

  const handleClose = () => {
    if (dialogRef.current) {
      dialogRef.current.returnValue = '';
    }
    onClose();
  };

  const runAction = async (
    key: string,
    action: () => Promise<{
      removedCount: number;
      reclaimedBytes: number;
      warnings: string[];
    } | void>,
    successLabel: (res?: {
      removedCount: number;
      reclaimedBytes: number;
      warnings: string[];
    }) => string,
  ) => {
    setBusyKey(key);
    setErrorMsg(null);
    setStatusMessage(null);
    try {
      const res = await action();
      if (res && res.warnings.length > 0 && res.removedCount === 0) {
        setErrorMsg(res.warnings.join('\n'));
      } else {
        setStatusMessage(successLabel(res ?? undefined));
        if (res && res.warnings.length > 0) {
          setErrorMsg(res.warnings.join('\n'));
        }
      }
      await Promise.all([onWorkspaceChanged(), onRefreshAudit()]);
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    } finally {
      setBusyKey(null);
    }
  };

  const handleUnlinkSymlinks = (
    items: Array<[string, string | null]>,
    key: string,
  ) =>
    void runAction(
      key,
      () => api.unlinkBrokenSymlinksBatch(items),
      (res) => `Removed ${res?.removedCount ?? 0} broken link(s). Real files were untouched.`,
    );

  const handleUnlinkDuplicateSkill = (dup: DuplicateSkillIssue) =>
    void runAction(
      `unlink-dup:${dup.standalonePath}`,
      async () => {
        const outcome = await api.toggleSkill(
          dup.projectPath
            ? { kind: 'project', projectPath: dup.projectPath }
            : { kind: 'global' },
          dup.skillId,
          dup.standalonePath,
          false,
        );
        if (outcome.status === 'requiresMoveToLibrary') {
          return {
            removedCount: 0,
            reclaimedBytes: 0,
            warnings: [
              `"${dup.skillName}" is a real folder, not a link — use "Move to Library…" to consolidate it safely.`,
            ],
          };
        }
        return undefined;
      },
      () => `Removed standalone link for "${dup.skillName}".`,
    );

  const handleMoveDuplicateToLibrary = (dup: DuplicateSkillIssue) => {
    onClose();
    onOpenConsolidate({
      skillIds: [dup.skillId],
      scope: dup.projectPath
        ? { kind: 'project', projectPath: dup.projectPath }
        : { kind: 'global' },
      scopeLabel: dup.scopeLabel,
      skills: [
        {
          id: dup.skillId,
          name: dup.skillName,
          upfrontTokens: dup.duplicateTokens,
        },
      ],
    });
  };

  const handleOpenInstructionFix = async (issue: InstructionHygieneIssue) => {
    setBusyKey(`preview-fix:${issue.path}`);
    setErrorMsg(null);
    setStatusMessage(null);
    try {
      const preview = await api.previewInstructionFix(issue.path, issue.kind);
      setFixPreview(preview);
      setDraftFixContent(preview.suggestedContent);
      setShowCurrentContent(false);
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    } finally {
      setBusyKey(null);
    }
  };

  const handleApplyInstructionFix = async () => {
    if (!fixPreview) {
      return;
    }
    setBusyKey(`apply-fix:${fixPreview.path}`);
    setErrorMsg(null);
    setStatusMessage(null);
    try {
      const res = await api.applyInstructionFix(
        fixPreview.path,
        draftFixContent,
      );
      const backupNote = res.backupPath
        ? ' Original backed up to Application Support.'
        : '';
      setStatusMessage(
        `Updated ${fixPreview.filename} (${formatTokensExact(res.tokens)} tok).${backupNote}`,
      );
      setFixPreview(null);
      setDraftFixContent('');
      await Promise.all([onWorkspaceChanged(), onRefreshAudit()]);
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    } finally {
      setBusyKey(null);
    }
  };

  const handlePruneCaches = () => {
    if (selectedCachePaths.length === 0) {
      return;
    }
    void runAction(
      'prune-caches',
      () => api.pruneCodexPluginCaches(selectedCachePaths),
      (res) =>
        `Deleted ${res?.removedCount ?? 0} old cache folder(s) (${formatBytes(res?.reclaimedBytes ?? 0)} freed).`,
    );
  };

  const handlePruneGitExcludes = (projectPaths: string[]) => {
    if (projectPaths.length === 0) {
      return;
    }
    void runAction(
      'prune-git',
      () => api.pruneStaleGitExcludes(projectPaths),
      (res) =>
        `Tidied ${res?.removedCount ?? 0} leftover .git/info/exclude rule(s).`,
    );
  };

  const handleRemoveClaudeBucket = (path: string, name: string) =>
    void runAction(
      `bucket:${path}`,
      () => api.removeClaudeSyncedBucket(path),
      (res) => {
        setConfirmBucketPath(null);
        return `Deleted Claude synced folder "${name}" (${formatBytes(res?.reclaimedBytes ?? 0)} freed).`;
      },
    );

  const toggleCacheSelection = (path: string) => {
    setSelectedCachePaths((prev) =>
      prev.includes(path) ? prev.filter((p) => p !== path) : [...prev, path],
    );
  };

  const handleRescan = () => {
    setScanning(true);
    setErrorMsg(null);
    void onRefreshAudit().finally(() => setScanning(false));
  };

  const isBusy = busyKey !== null || scanning;

  const showStrays = activePillar === 'all' || activePillar === 'strays';
  const showConsolidate =
    activePillar === 'all' || activePillar === 'consolidate';
  const showOptimize = activePillar === 'all' || activePillar === 'optimize';

  const summaryParts = useMemo(() => {
    if (!report) {
      return '';
    }
    const parts: string[] = [];
    if (report.totalIssues > 0) {
      parts.push(
        `${report.totalIssues} item${report.totalIssues === 1 ? '' : 's'} to review`,
      );
    } else {
      parts.push('No urgent issues');
    }
    if (counts.saveableTokens > 0) {
      parts.push(`up to ${formatTokensExact(counts.saveableTokens)} tok/session saveable`);
    }
    if (report.stalePluginCacheBytes > 0) {
      parts.push(`${formatBytes(report.stalePluginCacheBytes)} old caches`);
    }
    if (report.strayProjectSkills.length > 0) {
      parts.push(
        `${report.strayProjectSkills.length} project skill${report.strayProjectSkills.length === 1 ? '' : 's'} can be consolidated`,
      );
    }
    return parts.join(' · ');
  }, [report, counts.saveableTokens]);

  const pillarTabs: Array<{ id: AuditPillar; label: string; count: number }> = [
    {
      id: 'all',
      label: 'All',
      count: counts.strays + counts.consolidate + counts.optimize,
    },
    { id: 'strays', label: 'Strays', count: counts.strays },
    { id: 'consolidate', label: 'Consolidate', count: counts.consolidate },
    { id: 'optimize', label: 'Optimize', count: counts.optimize },
  ];

  const isSelectedPillarEmpty = useMemo(() => {
    if (!report || !counts.hasAnything) {
      return false;
    }
    if (activePillar === 'strays') {
      return counts.strays === 0;
    }
    if (activePillar === 'consolidate') {
      return counts.consolidate === 0;
    }
    if (activePillar === 'optimize') {
      return counts.optimize === 0;
    }
    return false;
  }, [report, counts, activePillar]);

  return (
    <dialog
      ref={dialogRef}
      aria-labelledby="cleanup-modal-title"
      onClose={handleClose}
      onPointerDown={(e) => {
        pressedOutsideRef.current = isOutsideDialog(e);
      }}
      onPointerUp={(e) => {
        if (pressedOutsideRef.current && isOutsideDialog(e)) {
          dialogRef.current?.close();
        }
        pressedOutsideRef.current = false;
      }}
      className="mac-sheet m-auto w-[800px] max-w-[calc(100vw-48px)] max-h-[calc(100vh-56px)] overflow-hidden p-0"
    >
      <div className="flex max-h-[calc(100vh-56px)] flex-col">
        {/* Sticky Toolbar */}
        <div className="shrink-0 space-y-2.5 border-b border-[var(--hairline)] bg-[var(--bg-toolbar)] px-5 py-3.5 backdrop-blur-xl">
          <div className="flex items-center justify-between gap-3">
            <div className="min-w-0">
              <h2
                id="cleanup-modal-title"
                className="flex items-center gap-2 text-[14px] font-semibold text-[var(--text-primary)]"
              >
                <ShieldCheck
                  className="h-4 w-4 text-[var(--accent)]"
                  strokeWidth={1.85}
                />
                <span>Audit &amp; Optimize</span>
              </h2>
              <p className="text-[12px] text-[var(--text-secondary)]">
                {report
                  ? counts.hasAnything
                    ? summaryParts
                    : 'Your workspace is consolidated, linked cleanly, and free of strays.'
                  : 'Scanning Global and Project workspaces…'}
              </p>
            </div>

            <div className="flex items-center gap-2">
              <button
                type="button"
                disabled={isBusy}
                onClick={handleRescan}
                className="mac-btn inline-flex h-7 items-center gap-1.5 px-2.5 text-[12px] font-medium disabled:opacity-50"
              >
                <RefreshCw
                  className={['h-3.5 w-3.5', scanning ? 'animate-spin' : ''].join(
                    ' ',
                  )}
                  strokeWidth={1.75}
                />
                Re-scan
              </button>

              <button
                type="button"
                onClick={() => dialogRef.current?.close()}
                aria-label="Close audit dialog"
                className="inline-flex h-7 w-7 items-center justify-center rounded-[6px] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
              >
                <X className="h-4 w-4" strokeWidth={1.75} />
              </button>
            </div>
          </div>

          {report && counts.hasAnything && (
            <div className="flex flex-wrap items-center justify-between gap-2">
              <div
                role="group"
                aria-label="Filter audit suggestions by category"
                className="inline-flex items-center rounded-[7px] bg-[var(--bg-field)] p-[2px] shadow-[inset_0_0.5px_1px_rgba(0,0,0,0.06)]"
              >
                {pillarTabs.map((tab) => {
                  const active = activePillar === tab.id;
                  return (
                    <button
                      key={tab.id}
                      type="button"
                      aria-pressed={active}
                      onClick={() => setActivePillar(tab.id)}
                      className={[
                        'inline-flex h-[24px] items-center gap-1.5 rounded-[5px] px-2.5 text-[11.5px] font-medium transition-colors duration-100',
                        active
                          ? 'bg-[var(--bg-control)] text-[var(--text-primary)] shadow-[var(--control-shadow)]'
                          : 'text-[var(--text-secondary)] hover:text-[var(--text-primary)]',
                      ].join(' ')}
                    >
                      <span>{tab.label}</span>
                      <span className="tabular-nums text-[10.5px] text-[var(--text-tertiary)]">
                        {tab.count}
                      </span>
                    </button>
                  );
                })}
              </div>

              <span className="text-[11px] text-[var(--text-tertiary)]">
                Review each suggestion before acting — real skill files are never deleted automatically.
              </span>
            </div>
          )}
        </div>

        {/* Body */}
        <div className="flex-1 space-y-6 overflow-y-auto px-5 py-4">
          {errorMsg && (
            <div className="whitespace-pre-line rounded-[8px] border border-[var(--status-red)]/30 bg-[var(--status-red)]/10 px-3.5 py-2 text-[12px] text-[var(--status-red)]">
              {errorMsg}
            </div>
          )}

          {statusMessage && (
            <div className="flex items-center gap-2 rounded-[8px] border border-[var(--status-green)]/30 bg-[var(--status-green)]/10 px-3.5 py-2 text-[12px] text-[var(--status-green)]">
              <CheckCircle2 className="h-4 w-4 shrink-0" strokeWidth={1.75} />
              <span>{statusMessage}</span>
            </div>
          )}

          {!report ? (
            <div className="mac-group px-4 py-8 text-center text-[12.5px] text-[var(--text-secondary)]">
              Scanning Global and Project directories…
            </div>
          ) : !counts.hasAnything ? (
            <div className="mac-group px-6 py-10 text-center">
              <CheckCircle2
                className="mx-auto mb-2.5 h-7 w-7 text-[var(--status-green)]"
                strokeWidth={1.6}
              />
              <div className="text-[13.5px] font-semibold text-[var(--text-primary)]">
                Workspace is consolidated and tidy
              </div>
              <p className="mx-auto mt-1 max-w-md text-[12px] text-[var(--text-secondary)]">
                No dead shortcuts, loose global skill folders, duplicate plugin skills, oversized instruction files, or leftover caches were found.
              </p>
            </div>
          ) : isSelectedPillarEmpty ? (
            <div className="mac-group px-6 py-8 text-center">
              <CheckCircle2
                className="mx-auto mb-2 h-6 w-6 text-[var(--status-green)]"
                strokeWidth={1.6}
              />
              <div className="text-[13px] font-semibold text-[var(--text-primary)]">
                Nothing to do in {pillarTabs.find((t) => t.id === activePillar)?.label}
              </div>
              <p className="mt-1 text-[12px] text-[var(--text-secondary)]">
                Switch back to All to inspect the remaining audit suggestions.
              </p>
            </div>
          ) : (
            <>
              {/* STRAYS: 1. Broken Shortcuts (Dead Symlinks) */}
              {showStrays && report.brokenSymlinks.length > 0 && (
                <AuditSection
                  icon={
                    <Unlink
                      className="h-3.5 w-3.5 text-[var(--status-red)]"
                      strokeWidth={1.75}
                    />
                  }
                  title="Broken Shortcuts (Dead Symlinks)"
                  count={report.brokenSymlinks.length}
                  badge="Strays"
                  whatItIs="Shortcuts inside agent or project folders pointing to a file or folder that was moved, renamed, or deleted. Agents cannot read anything through them."
                  recommendation="Recommended: Remove dead links. This deletes only the broken shortcut itself — never any real files."
                  action={
                    <button
                      type="button"
                      disabled={isBusy}
                      onClick={() =>
                        handleUnlinkSymlinks(
                          report.brokenSymlinks.map((b) => [
                            b.path,
                            b.projectPath,
                          ]),
                          'unlink-all-broken',
                        )
                      }
                      className="mac-btn h-7 px-2.5 text-[11.5px] font-medium text-[var(--status-red)] disabled:opacity-50"
                    >
                      Remove All Dead Links
                    </button>
                  }
                >
                  <div className="mac-group divide-y divide-[var(--hairline-inset)] overflow-hidden">
                    {report.brokenSymlinks.map((item) => (
                      <div
                        key={item.path}
                        className="flex items-center justify-between gap-3 px-3.5 py-2.5 text-[12px]"
                      >
                        <div className="min-w-0 flex-1 space-y-0.5">
                          <div className="flex flex-wrap items-center gap-2">
                            <span className="truncate font-medium text-[var(--text-primary)]">
                              {item.path.split('/').pop()}
                            </span>
                            <span className="rounded bg-[var(--bg-field)] px-1.5 py-[1px] text-[10.5px] text-[var(--text-secondary)]">
                              {brokenLinkKindLabel(item.itemKind)}
                            </span>
                            {item.originalPath ? (
                              <span className="rounded bg-[var(--status-green)]/12 px-1.5 py-[1px] text-[10.5px] font-medium text-[var(--status-green)]">
                                Safe
                                {item.originalLocation
                                  ? ` · ${item.originalLocation}`
                                  : ' · Original found'}
                              </span>
                            ) : (
                              <span className="rounded bg-[var(--status-red)]/12 px-1.5 py-[1px] text-[10.5px] font-medium text-[var(--status-red)]">
                                Original not found (lost)
                              </span>
                            )}
                            <span className="shrink-0 text-[11px] text-[var(--text-tertiary)]">
                              {item.scopeLabel}
                              {item.agent
                                ? ` · ${agentDisplayName(item.agent)}`
                                : ''}
                            </span>
                          </div>
                          <div className="mono-path truncate text-[11px] text-[var(--text-tertiary)]">
                            Link: {item.path}
                          </div>
                          {item.originalPath && (
                            <div className="flex items-center gap-1.5 text-[11px] text-[var(--status-green)]">
                              <span className="mono-path truncate">
                                Original: {item.originalPath}
                              </span>
                              <button
                                type="button"
                                onClick={() => onRevealPath(item.originalPath!)}
                                title="Reveal original in Finder"
                                aria-label={`Reveal original for ${item.path.split('/').pop()} in Finder`}
                                className="inline-flex h-4 w-4 shrink-0 items-center justify-center rounded text-[var(--status-green)] hover:bg-[var(--status-green)]/15"
                              >
                                <FolderOpen
                                  className="h-3 w-3"
                                  strokeWidth={1.75}
                                />
                              </button>
                            </div>
                          )}
                          <div className="mono-path truncate text-[11px] text-[var(--status-red)]/85">
                            Missing target: {item.target}
                          </div>
                        </div>

                        <div className="flex shrink-0 items-center gap-1.5">
                          <button
                            type="button"
                            onClick={() => onRevealPath(item.path)}
                            title="Reveal broken shortcut in Finder"
                            aria-label={`Reveal ${item.path.split('/').pop()} in Finder`}
                            className="inline-flex h-7 w-7 items-center justify-center rounded-[6px] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
                          >
                            <FolderOpen
                              className="h-3.5 w-3.5"
                              strokeWidth={1.75}
                            />
                          </button>

                          <button
                            type="button"
                            disabled={isBusy}
                            onClick={() =>
                              handleUnlinkSymlinks(
                                [[item.path, item.projectPath]],
                                `unlink:${item.path}`,
                              )
                            }
                            className="mac-btn h-7 px-2.5 text-[11.5px] font-medium text-[var(--status-red)] disabled:opacity-50"
                          >
                            Remove Link
                          </button>
                        </div>
                      </div>
                    ))}
                  </div>
                </AuditSection>
              )}

              {/* CONSOLIDATE: 2. Loose Global Skill Folders */}
              {showConsolidate && report.strayGlobalSkills.length > 0 && (
                <AuditSection
                  icon={
                    <FolderCog
                      className="h-3.5 w-3.5 text-[var(--status-amber)]"
                      strokeWidth={1.75}
                    />
                  }
                  title="Loose Global Skill Folders"
                  count={report.strayGlobalSkills.length}
                  badge="Consolidate"
                  whatItIs="Real skill folders saved directly inside an agent's global directory instead of your central Library. Because AgentDock never deletes real folders, they stay loaded in every session and cannot be toggled off yet."
                  recommendation="Recommended: Move them into your Library. All files stay intact, and you can switch each skill on or off whenever you want."
                  action={
                    <button
                      type="button"
                      onClick={() => {
                        onClose();
                        onOpenConsolidate({
                          skillIds: report.strayGlobalSkills.map(
                            (s) => s.skillId,
                          ),
                          scope: { kind: 'global' },
                          scopeLabel: 'Global',
                          skills: report.strayGlobalSkills.map(
                            toConsolidateSummary,
                          ),
                        });
                      }}
                      className="mac-btn h-7 px-2.5 text-[11.5px] font-medium"
                    >
                      Consolidate All…
                    </button>
                  }
                >
                  <div className="mac-group divide-y divide-[var(--hairline-inset)] overflow-hidden">
                    {report.strayGlobalSkills.map((stray) => (
                      <div
                        key={`${stray.skillId}:${stray.path}`}
                        className="flex flex-wrap items-center justify-between gap-3 px-3.5 py-2.5 text-[12px]"
                      >
                        <div className="min-w-0 flex-1 space-y-0.5">
                          <div className="flex flex-wrap items-center gap-2">
                            <span className="font-medium text-[var(--text-primary)]">
                              {stray.skillName}
                            </span>
                            {stray.agents.length > 0 && (
                              <span className="text-[11px] text-[var(--text-secondary)]">
                                {stray.agents.map(agentDisplayName).join(', ')}
                              </span>
                            )}
                            <span className="tabular-nums text-[11px] text-[var(--status-amber)]">
                              {formatTokensExact(stray.upfrontTokens)} tok/session
                            </span>
                            {stray.existsInLibrary && (
                              <span className="rounded bg-[var(--status-amber)]/15 px-1.5 py-[1px] text-[10.5px] font-medium text-[var(--status-amber)]">
                                Already in Library — review before moving
                              </span>
                            )}
                          </div>
                          <div className="mono-path truncate text-[11px] text-[var(--text-tertiary)]">
                            {stray.path}
                          </div>
                        </div>

                        <div className="flex shrink-0 items-center gap-1.5">
                          <button
                            type="button"
                            onClick={() => onOpenFile(stray.skillMdPath)}
                            title="Open SKILL.md to inspect what this skill does"
                            aria-label={`Open ${stray.skillName} SKILL.md`}
                            className="inline-flex h-7 w-7 items-center justify-center rounded-[6px] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
                          >
                            <ExternalLink
                              className="h-3.5 w-3.5"
                              strokeWidth={1.75}
                            />
                          </button>
                          <button
                            type="button"
                            onClick={() => onRevealPath(stray.path)}
                            title="Reveal folder in Finder"
                            aria-label={`Reveal ${stray.skillName} in Finder`}
                            className="inline-flex h-7 w-7 items-center justify-center rounded-[6px] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
                          >
                            <FolderOpen
                              className="h-3.5 w-3.5"
                              strokeWidth={1.75}
                            />
                          </button>
                          <button
                            type="button"
                            onClick={() => {
                              onClose();
                              onOpenConsolidate({
                                skillIds: [stray.skillId],
                                scope: { kind: 'global' },
                                scopeLabel: 'Global',
                                skills: [toConsolidateSummary(stray)],
                              });
                            }}
                            className="mac-btn h-7 px-2.5 text-[11.5px] font-medium"
                          >
                            Move to Library…
                          </button>
                        </div>
                      </div>
                    ))}
                  </div>
                </AuditSection>
              )}

              {/* CONSOLIDATE: 3. Duplicate Skills (Loaded Twice) */}
              {showConsolidate && report.duplicateSkills.length > 0 && (
                <AuditSection
                  icon={
                    <Sparkles
                      className="h-3.5 w-3.5 text-[var(--status-amber)]"
                      strokeWidth={1.75}
                    />
                  }
                  title="Duplicate Skills (Loaded Twice)"
                  count={report.duplicateSkills.length}
                  badge="Consolidate"
                  whatItIs="These skills are active both as a standalone skill and inside an enabled plugin for the same agent, so you pay their startup token cost twice."
                  recommendation="Recommended: Remove the standalone link if you want the plugin version, or disable the plugin in its own agent config."
                >
                  <div className="mac-group divide-y divide-[var(--hairline-inset)] overflow-hidden">
                    {report.duplicateSkills.map((dup) => (
                      <div
                        key={`${dup.standalonePath}:${dup.pluginAgent}:${dup.pluginId}`}
                        className="flex flex-wrap items-center justify-between gap-3 px-3.5 py-2.5 text-[12px]"
                      >
                        <div className="min-w-0 flex-1 space-y-0.5">
                          <div className="flex flex-wrap items-center gap-2">
                            <span className="font-medium text-[var(--text-primary)]">
                              {dup.skillName}
                            </span>
                            <span className="text-[11px] text-[var(--text-secondary)]">
                              {dup.scopeLabel} · also bundled in{' '}
                              {agentDisplayName(dup.pluginAgent)} plugin &ldquo;
                              {dup.pluginName}&rdquo;
                            </span>
                            <span className="tabular-nums text-[11px] text-[var(--status-amber)]">
                              Saves ~{formatTokensExact(dup.duplicateTokens)} tok/session
                            </span>
                          </div>
                          <div className="mono-path truncate text-[11px] text-[var(--text-tertiary)]">
                            Standalone: {dup.standalonePath}
                          </div>
                        </div>

                        <div className="flex shrink-0 items-center gap-1.5">
                          <button
                            type="button"
                            onClick={() => onRevealPath(dup.standalonePath)}
                            title="Reveal standalone folder in Finder"
                            aria-label={`Reveal ${dup.skillName} in Finder`}
                            className="inline-flex h-7 w-7 items-center justify-center rounded-[6px] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
                          >
                            <FolderOpen
                              className="h-3.5 w-3.5"
                              strokeWidth={1.75}
                            />
                          </button>

                          {dup.isSymlink ? (
                            <button
                              type="button"
                              disabled={isBusy}
                              onClick={() => handleUnlinkDuplicateSkill(dup)}
                              className="mac-btn h-7 px-2.5 text-[11.5px] font-medium disabled:opacity-50"
                            >
                              Remove Standalone Link
                            </button>
                          ) : (
                            <button
                              type="button"
                              disabled={isBusy}
                              onClick={() => handleMoveDuplicateToLibrary(dup)}
                              className="mac-btn h-7 px-2.5 text-[11.5px] font-medium disabled:opacity-50"
                            >
                              Move to Library…
                            </button>
                          )}
                        </div>
                      </div>
                    ))}
                  </div>
                </AuditSection>
              )}

              {/* CONSOLIDATE: 4. Project-Local Skill Folders (Optional) */}
              {showConsolidate && report.strayProjectSkills.length > 0 && (
                <AuditSection
                  icon={
                    <Layers
                      className="h-3.5 w-3.5 text-[var(--accent)]"
                      strokeWidth={1.75}
                    />
                  }
                  title="Project-Local Skill Folders"
                  count={report.strayProjectSkills.length}
                  badge="Optional · Consolidate"
                  whatItIs="Real skill folders stored inside individual project folders rather than your central Library. Project-only skills belong in their repo, while general skills can be centralized."
                  recommendation="Optional: Leave repo-specific skills here, or move reusable ones into your Library so you can toggle them across any project."
                  action={
                    <button
                      type="button"
                      onClick={() => setProjectStraysExpanded((prev) => !prev)}
                      className="mac-btn inline-flex h-7 items-center gap-1 px-2.5 text-[11.5px] font-medium"
                    >
                      <ChevronRight
                        className={[
                          'h-3.5 w-3.5 transition-transform duration-150',
                          projectStraysExpanded ? 'rotate-90' : '',
                        ].join(' ')}
                        strokeWidth={1.75}
                      />
                      <span>
                        {projectStraysExpanded ? 'Hide List' : 'Show List'}
                      </span>
                    </button>
                  }
                >
                  {projectStraysExpanded && (
                    <div className="mac-group max-h-60 divide-y divide-[var(--hairline-inset)] overflow-y-auto">
                      {report.strayProjectSkills.map((stray) => (
                        <div
                          key={`${stray.projectPath}:${stray.skillId}:${stray.path}`}
                          className="flex flex-wrap items-center justify-between gap-3 px-3.5 py-2.5 text-[12px]"
                        >
                          <div className="min-w-0 flex-1 space-y-0.5">
                            <div className="flex flex-wrap items-center gap-2">
                              <span className="font-medium text-[var(--text-primary)]">
                                {stray.skillName}
                              </span>
                              <span className="text-[11px] text-[var(--text-secondary)]">
                                {stray.scopeLabel}
                                {stray.agents.length > 0
                                  ? ` · ${stray.agents.map(agentDisplayName).join(', ')}`
                                  : ''}
                              </span>
                              <span className="tabular-nums text-[11px] text-[var(--text-tertiary)]">
                                {formatTokensExact(stray.upfrontTokens)} tok
                              </span>
                              {stray.existsInLibrary && (
                                <span className="rounded bg-[var(--status-amber)]/15 px-1.5 py-[1px] text-[10.5px] font-medium text-[var(--status-amber)]">
                                  Already in Library
                                </span>
                              )}
                            </div>
                            <div className="mono-path truncate text-[11px] text-[var(--text-tertiary)]">
                              {stray.path}
                            </div>
                          </div>

                          <div className="flex shrink-0 items-center gap-1.5">
                            <button
                              type="button"
                              onClick={() => onOpenFile(stray.skillMdPath)}
                              title="Open SKILL.md"
                              aria-label={`Open ${stray.skillName} SKILL.md`}
                              className="inline-flex h-7 w-7 items-center justify-center rounded-[6px] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
                            >
                              <ExternalLink
                                className="h-3.5 w-3.5"
                                strokeWidth={1.75}
                              />
                            </button>
                            <button
                              type="button"
                              onClick={() => onRevealPath(stray.path)}
                              title="Reveal folder in Finder"
                              aria-label={`Reveal ${stray.skillName} in Finder`}
                              className="inline-flex h-7 w-7 items-center justify-center rounded-[6px] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
                            >
                              <FolderOpen
                                className="h-3.5 w-3.5"
                                strokeWidth={1.75}
                              />
                            </button>
                            {stray.projectPath && (
                              <button
                                type="button"
                                onClick={() => {
                                  const projectPath = stray.projectPath;
                                  if (!projectPath) {
                                    return;
                                  }
                                  onClose();
                                  onOpenConsolidate({
                                    skillIds: [stray.skillId],
                                    scope: { kind: 'project', projectPath },
                                    scopeLabel: stray.scopeLabel,
                                    skills: [toConsolidateSummary(stray)],
                                  });
                                }}
                                className="mac-btn h-7 px-2.5 text-[11.5px] font-medium"
                              >
                                Move to Library…
                              </button>
                            )}
                          </div>
                        </div>
                      ))}
                    </div>
                  )}
                </AuditSection>
              )}

              {/* OPTIMIZE: 5. Global Instructions & Rule Health */}
              {showOptimize && report.instructionHygiene.length > 0 && (
                <AuditSection
                  icon={
                    <AlertTriangle
                      className="h-3.5 w-3.5 text-[var(--status-amber)]"
                      strokeWidth={1.75}
                    />
                  }
                  title="Global Instructions &amp; Rule Health"
                  count={report.instructionHygiene.length}
                  badge="Optimize"
                  whatItIs="Global instruction files are injected into every conversation. Large files eat startup tokens everywhere, and broken rule headers prevent agents from reading triggers properly."
                  recommendation="Recommended: Open flagged files to trim long sections into on-demand skills or fix YAML frontmatter headers."
                >
                  <div className="mac-group divide-y divide-[var(--hairline-inset)] overflow-hidden">
                    {report.instructionHygiene.map((issue, idx) => (
                      <div
                        key={`${issue.path}:${idx}`}
                        className="flex items-center justify-between gap-3 px-3.5 py-2.5 text-[12px]"
                      >
                        <div className="flex min-w-0 flex-1 items-start gap-2">
                          {issue.severity === 'warning' ? (
                            <AlertTriangle
                              className="mt-0.5 h-3.5 w-3.5 shrink-0 text-[var(--status-amber)]"
                              strokeWidth={1.75}
                            />
                          ) : (
                            <Info
                              className="mt-0.5 h-3.5 w-3.5 shrink-0 text-[var(--accent)]"
                              strokeWidth={1.75}
                            />
                          )}
                          <div className="min-w-0 flex-1 space-y-0.5">
                            <div className="flex flex-wrap items-center gap-2">
                              <span className="font-medium text-[var(--text-primary)]">
                                {issue.filename}
                              </span>
                              <span className="text-[11px] text-[var(--text-tertiary)]">
                                {agentDisplayName(issue.agent)} ·{' '}
                                {formatTokensExact(issue.tokens)} tok
                              </span>
                            </div>
                            <p className="text-[11.5px] text-[var(--text-secondary)]">
                              {issue.message}
                            </p>
                          </div>
                        </div>

                        <div className="flex shrink-0 items-center gap-1.5">
                          <button
                            type="button"
                            onClick={() => onRevealPath(issue.path)}
                            title="Reveal file in Finder"
                            aria-label={`Reveal ${issue.filename} in Finder`}
                            className="inline-flex h-7 w-7 items-center justify-center rounded-[6px] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
                          >
                            <FolderOpen
                              className="h-3.5 w-3.5"
                              strokeWidth={1.75}
                            />
                          </button>
                          <button
                            type="button"
                            onClick={() => onOpenFile(issue.path)}
                            title={`Open ${issue.path}`}
                            className="mac-btn inline-flex h-7 items-center gap-1 px-2.5 text-[11.5px] font-medium"
                          >
                            <ExternalLink
                              className="h-3 w-3"
                              strokeWidth={1.75}
                            />
                            Open
                          </button>
                          <button
                            type="button"
                            disabled={isBusy}
                            onClick={() => void handleOpenInstructionFix(issue)}
                            className="mac-btn inline-flex h-7 items-center gap-1 px-2.5 text-[11.5px] font-medium text-[var(--accent)] disabled:opacity-50"
                          >
                            <Sparkles
                              className="h-3 w-3"
                              strokeWidth={1.75}
                            />
                            Review Fix…
                          </button>
                        </div>
                      </div>
                    ))}
                  </div>

                  {fixPreview && (
                    <div className="mac-group mt-2.5 space-y-3 p-3.5 text-[12px]">
                      <div className="flex flex-wrap items-start justify-between gap-2">
                        <div>
                          <div className="flex items-center gap-2">
                            <span className="font-semibold text-[var(--text-primary)]">
                              {fixPreview.title}
                            </span>
                            <span className="rounded-full bg-[var(--bg-field)] px-2 py-[1px] text-[10.5px] font-medium text-[var(--text-secondary)]">
                              {fixPreview.filename} ·{' '}
                              {formatTokensExact(fixPreview.currentTokens)} →{' '}
                              {formatTokensExact(fixPreview.suggestedTokens)} tok
                            </span>
                          </div>
                          <p className="mt-1 text-[11.5px] leading-relaxed text-[var(--text-secondary)]">
                            <span className="font-medium text-[var(--text-primary)]">
                              Why:{' '}
                            </span>
                            {fixPreview.why}
                          </p>
                          <p className="mt-1 text-[11.5px] leading-relaxed text-[var(--text-secondary)]">
                            <span className="font-medium text-[var(--text-primary)]">
                              Change:{' '}
                            </span>
                            {fixPreview.whatChanges}
                          </p>
                        </div>
                        <button
                          type="button"
                          onClick={() => {
                            setFixPreview(null);
                            setDraftFixContent('');
                          }}
                          aria-label="Close fix preview"
                          className="inline-flex h-6 w-6 items-center justify-center rounded-[6px] text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
                        >
                          <X className="h-3.5 w-3.5" strokeWidth={1.75} />
                        </button>
                      </div>

                      <div className="space-y-1.5">
                        <div className="flex items-center justify-between gap-2">
                          <label
                            htmlFor="instruction-fix-draft"
                            className="text-[11px] font-medium text-[var(--text-secondary)]"
                          >
                            Suggested replacement (editable before approval)
                          </label>
                          <div className="flex items-center gap-3 text-[11px]">
                            <button
                              type="button"
                              onClick={() =>
                                setShowCurrentContent((prev) => !prev)
                              }
                              className="text-[var(--text-secondary)] hover:underline"
                            >
                              {showCurrentContent
                                ? 'Hide Current File'
                                : 'View Current File'}
                            </button>
                            {draftFixContent !== fixPreview.suggestedContent && (
                              <button
                                type="button"
                                onClick={() =>
                                  setDraftFixContent(
                                    fixPreview.suggestedContent,
                                  )
                                }
                                className="text-[var(--accent)] hover:underline"
                              >
                                Reset to Template
                              </button>
                            )}
                          </div>
                        </div>

                        {showCurrentContent && (
                          <pre className="max-h-40 overflow-auto rounded-[6px] border border-[var(--hairline-inset)] bg-[var(--bg-field)] p-2.5 font-mono text-[11px] leading-relaxed text-[var(--text-secondary)] whitespace-pre-wrap">
                            {fixPreview.currentContent}
                          </pre>
                        )}

                        <textarea
                          id="instruction-fix-draft"
                          value={draftFixContent}
                          onChange={(e) => setDraftFixContent(e.target.value)}
                          rows={9}
                          spellCheck={false}
                          className="w-full rounded-[6px] border border-[var(--hairline-inset)] bg-[var(--bg-field)] p-2.5 font-mono text-[11.5px] leading-relaxed text-[var(--text-primary)] focus:border-[var(--accent)] focus:outline-none"
                        />
                      </div>

                      <div className="flex flex-wrap items-center justify-between gap-2 border-t border-[var(--hairline-inset)] pt-2.5">
                        <span className="text-[11px] text-[var(--text-tertiary)]">
                          Approving saves a rolling backup of your original file
                          in Application Support before writing.
                        </span>
                        <div className="flex items-center gap-2">
                          <button
                            type="button"
                            disabled={isBusy}
                            onClick={() => {
                              setFixPreview(null);
                              setDraftFixContent('');
                            }}
                            className="mac-btn h-7 px-2.5 text-[11.5px] font-medium disabled:opacity-50"
                          >
                            Cancel
                          </button>
                          <button
                            type="button"
                            disabled={isBusy}
                            onClick={() => void handleApplyInstructionFix()}
                            className="mac-btn-primary inline-flex h-7 items-center gap-1 px-3 text-[11.5px] font-medium disabled:opacity-50"
                          >
                            <ShieldCheck
                              className="h-3.5 w-3.5"
                              strokeWidth={1.75}
                            />
                            Approve &amp; Write File
                          </button>
                        </div>
                      </div>
                    </div>
                  )}
                </AuditSection>
              )}

              {/* STRAYS: 6. Old Codex Plugin Caches */}
              {showStrays && report.stalePluginCaches.length > 0 && (
                <AuditSection
                  icon={
                    <HardDrive
                      className="h-3.5 w-3.5 text-[var(--text-secondary)]"
                      strokeWidth={1.75}
                    />
                  }
                  title="Old Codex Plugin Caches"
                  count={report.stalePluginCaches.length}
                  badge={`${formatBytes(report.stalePluginCacheBytes)} · Strays`}
                  whatItIs="Previous or unconfigured plugin versions left behind in Codex's local cache directory."
                  recommendation="Safe to delete: Only inactive cached copies are removed — active plugins stay untouched."
                  action={
                    <div className="flex items-center gap-2.5 text-[11.5px]">
                      <button
                        type="button"
                        onClick={() =>
                          setSelectedCachePaths(
                            report.stalePluginCaches.map((c) => c.path),
                          )
                        }
                        className="text-[var(--accent)] hover:underline"
                      >
                        Select All
                      </button>
                      <button
                        type="button"
                        onClick={() => setSelectedCachePaths([])}
                        className="text-[var(--text-secondary)] hover:underline"
                      >
                        Clear
                      </button>
                      <button
                        type="button"
                        disabled={isBusy || selectedCachePaths.length === 0}
                        onClick={handlePruneCaches}
                        className="mac-btn inline-flex h-7 items-center gap-1 px-2.5 text-[11.5px] font-medium text-[var(--status-red)] disabled:opacity-50"
                      >
                        <Trash2 className="h-3 w-3" strokeWidth={1.75} />
                        Delete Selected ({selectedCachePaths.length} ·{' '}
                        {formatBytes(selectedCacheBytes)})
                      </button>
                    </div>
                  }
                >
                  <div className="mac-group max-h-52 divide-y divide-[var(--hairline-inset)] overflow-y-auto">
                    {report.stalePluginCaches.map((cache) => {
                      const checked = selectedCachePaths.includes(cache.path);
                      return (
                        <div
                          key={cache.path}
                          className="flex items-center justify-between gap-3 px-3.5 py-2 text-[12px] hover:bg-[var(--bg-hover)]"
                        >
                          <label className="flex min-w-0 flex-1 cursor-pointer items-center gap-2.5">
                            <input
                              type="checkbox"
                              checked={checked}
                              onChange={() => toggleCacheSelection(cache.path)}
                            />
                            <div className="min-w-0 flex-1">
                              <div className="flex items-center gap-2">
                                <span className="truncate font-medium text-[var(--text-primary)]">
                                  {cache.pluginSlug}
                                </span>
                                <span className="shrink-0 text-[11px] text-[var(--text-tertiary)]">
                                  {cache.marketplace} · {cache.version}
                                </span>
                              </div>
                              <div className="truncate text-[11px] text-[var(--text-secondary)]">
                                {cache.reason}
                              </div>
                            </div>
                          </label>

                          <div className="flex shrink-0 items-center gap-2">
                            <span className="tabular-nums text-[11.5px] text-[var(--text-secondary)]">
                              {formatBytes(cache.sizeBytes)}
                            </span>
                            <button
                              type="button"
                              onClick={() => onRevealPath(cache.path)}
                              title="Reveal cache folder in Finder"
                              aria-label={`Reveal ${cache.pluginSlug} cache in Finder`}
                              className="inline-flex h-7 w-7 items-center justify-center rounded-[6px] text-[var(--text-secondary)] hover:bg-[var(--bg-subtle)] hover:text-[var(--text-primary)]"
                            >
                              <FolderOpen
                                className="h-3.5 w-3.5"
                                strokeWidth={1.75}
                              />
                            </button>
                          </div>
                        </div>
                      );
                    })}
                  </div>
                </AuditSection>
              )}

              {/* STRAYS: 7. Leftover Git Hide Rules */}
              {showStrays && report.staleGitExcludes.length > 0 && (
                <AuditSection
                  icon={
                    <GitBranch
                      className="h-3.5 w-3.5 text-[var(--text-secondary)]"
                      strokeWidth={1.75}
                    />
                  }
                  title="Leftover Git Hide Rules"
                  count={report.staleGitExcludes.length}
                  badge="Strays"
                  whatItIs="Old AgentDock lines in .git/info/exclude for linked skills or support docs that were later removed from the project."
                  recommendation="Safe to tidy: Removes only obsolete AgentDock exclude lines (with a rolling backup saved first) and keeps your own rules untouched."
                  action={
                    <button
                      type="button"
                      disabled={isBusy}
                      onClick={() =>
                        handlePruneGitExcludes(
                          report.staleGitExcludes.map((g) => g.projectPath),
                        )
                      }
                      className="mac-btn h-7 px-2.5 text-[11.5px] font-medium disabled:opacity-50"
                    >
                      Tidy All
                    </button>
                  }
                >
                  <div className="mac-group divide-y divide-[var(--hairline-inset)] overflow-hidden">
                    {report.staleGitExcludes.map((proj) => (
                      <div
                        key={proj.projectPath}
                        className="flex items-center justify-between gap-3 px-3.5 py-2 text-[12px]"
                      >
                        <div className="min-w-0 flex-1">
                          <div className="font-medium text-[var(--text-primary)]">
                            {proj.projectName}
                          </div>
                          <div className="mono-path truncate text-[11px] text-[var(--text-tertiary)]">
                            {proj.entries.join(', ')}
                          </div>
                        </div>
                        <button
                          type="button"
                          disabled={isBusy}
                          onClick={() =>
                            handlePruneGitExcludes([proj.projectPath])
                          }
                          className="mac-btn h-7 shrink-0 px-2.5 text-[11.5px] font-medium disabled:opacity-50"
                        >
                          Tidy
                        </button>
                      </div>
                    ))}
                  </div>
                </AuditSection>
              )}

              {/* STRAYS: 8. Claude Synced Skill Buckets */}
              {showStrays && report.claudeSyncedBuckets.length > 0 && (
                <AuditSection
                  icon={
                    <HardDrive
                      className="h-3.5 w-3.5 text-[var(--text-secondary)]"
                      strokeWidth={1.75}
                    />
                  }
                  title="Claude Synced Skill Buckets"
                  count={report.claudeSyncedBuckets.length}
                  badge="Strays"
                  whatItIs="Subfolders inside ~/.claude/skills/synced/ created by Claude's external sync rather than AgentDock."
                  recommendation="Inspect in Finder first before deleting — removing a bucket permanently deletes its local files."
                >
                  <div className="mac-group divide-y divide-[var(--hairline-inset)] overflow-hidden">
                    {report.claudeSyncedBuckets.map((bucket) => {
                      const confirming = confirmBucketPath === bucket.path;
                      return (
                        <div
                          key={bucket.path}
                          className="flex items-center justify-between gap-3 px-3.5 py-2.5 text-[12px]"
                        >
                          <div className="min-w-0 flex-1">
                            <div className="flex items-center gap-2">
                              <span className="truncate font-medium text-[var(--text-primary)]">
                                {bucket.name}
                              </span>
                              <span className="tabular-nums shrink-0 text-[11px] text-[var(--text-tertiary)]">
                                {bucket.entryCount} files ·{' '}
                                {formatBytes(bucket.sizeBytes)}
                              </span>
                            </div>
                            <div className="mono-path truncate text-[11px] text-[var(--text-tertiary)]">
                              {bucket.path}
                            </div>
                          </div>

                          <div className="flex shrink-0 items-center gap-1.5">
                            <button
                              type="button"
                              onClick={() => onRevealPath(bucket.path)}
                              title="Reveal bucket in Finder"
                              aria-label={`Reveal ${bucket.name} in Finder`}
                              className="inline-flex h-7 w-7 items-center justify-center rounded-[6px] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
                            >
                              <FolderOpen
                                className="h-3.5 w-3.5"
                                strokeWidth={1.75}
                              />
                            </button>

                            {confirming ? (
                              <>
                                <button
                                  type="button"
                                  disabled={isBusy}
                                  onClick={() =>
                                    handleRemoveClaudeBucket(
                                      bucket.path,
                                      bucket.name,
                                    )
                                  }
                                  className="mac-btn h-7 px-2.5 text-[11.5px] font-medium text-[var(--status-red)] disabled:opacity-50"
                                >
                                  Confirm Delete
                                </button>
                                <button
                                  type="button"
                                  disabled={isBusy}
                                  onClick={() => setConfirmBucketPath(null)}
                                  className="mac-btn h-7 px-2 text-[11.5px] text-[var(--text-secondary)]"
                                >
                                  Cancel
                                </button>
                              </>
                            ) : (
                              <button
                                type="button"
                                disabled={isBusy}
                                onClick={() => setConfirmBucketPath(bucket.path)}
                                className="mac-btn h-7 px-2.5 text-[11.5px] font-medium text-[var(--status-red)] disabled:opacity-50"
                              >
                                Delete…
                              </button>
                            )}
                          </div>
                        </div>
                      );
                    })}
                  </div>
                </AuditSection>
              )}
            </>
          )}
        </div>
      </div>
    </dialog>
  );
}

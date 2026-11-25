import { useEffect, useMemo, useRef, useState } from 'react';
import {
  AlertTriangle,
  ArrowRight,
  CheckCircle2,
  FolderOpen,
  X,
} from 'lucide-react';
import type {
  AfterMoveAction,
  LibrarySource,
  MovePreviewItem,
  SkillItem,
  ToggleScope,
} from '@/types';
import { api } from '@/lib/api';
import { MacSelect } from '@/components/ui/MacSelect';
import { MacSwitch } from '@/components/ui/MacSwitch';

type ConsolidateSkillSummary = Pick<SkillItem, 'id' | 'name' | 'upfrontTokens'>;

interface DeBloatModalProps {
  open: boolean;
  skills: ConsolidateSkillSummary[];
  initialSelectedIds: string[];
  librarySources: LibrarySource[];
  scope: ToggleScope;
  scopeLabel?: string;
  onClose: () => void;
  onRegisterLibrarySource?: (path: string) => Promise<void>;
  onSuccess: () => Promise<void>;
}

function normalizeTrimmedPath(p: string): string {
  return p.trim().replace(/\/+$/, '');
}

function preferredLibraryPath(sources: LibrarySource[]): string {
  const skillsPreferred = sources.find((src) => {
    const lowerPath = src.path.toLowerCase();
    const lowerLabel = src.label.toLowerCase();
    return !lowerPath.endsWith('/rules') && !lowerLabel.includes('rule');
  });
  return skillsPreferred?.path ?? sources[0]?.path ?? '';
}

export function DeBloatModal({
  open,
  skills,
  initialSelectedIds,
  librarySources,
  scope,
  scopeLabel,
  onClose,
  onRegisterLibrarySource,
  onSuccess,
}: DeBloatModalProps) {
  const dialogRef = useRef<HTMLDialogElement | null>(null);
  const pressedOutsideRef = useRef(false);

  const [targetLibraryPath, setTargetLibraryPath] = useState(() =>
    preferredLibraryPath(librarySources),
  );
  const [registerAsLibrary, setRegisterAsLibrary] = useState(true);
  const [selectedIds, setSelectedIds] = useState<string[]>(initialSelectedIds);
  const [afterMove, setAfterMove] = useState<AfterMoveAction>('unlink');
  const [previews, setPreviews] = useState<MovePreviewItem[]>([]);
  const [loadingPreview, setLoadingPreview] = useState(false);
  const [executing, setExecuting] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) {
      return;
    }
    if (open) {
      setTargetLibraryPath(preferredLibraryPath(librarySources));
      setRegisterAsLibrary(true);
      setSelectedIds(initialSelectedIds);
      setAfterMove('unlink');
      setErrorMsg(null);
      if (!dialog.open) {
        dialog.showModal();
      }
    } else if (dialog.open) {
      dialog.close();
    }
  }, [open, initialSelectedIds, librarySources]);

  const trimmedTargetPath = normalizeTrimmedPath(targetLibraryPath);

  const isKnownLibrarySource = useMemo(
    () =>
      librarySources.some(
        (src) => normalizeTrimmedPath(src.path) === trimmedTargetPath,
      ),
    [librarySources, trimmedTargetPath],
  );

  const targetFolderName = useMemo(() => {
    const parts = trimmedTargetPath.split('/').filter(Boolean);
    return parts.pop() ?? 'Library';
  }, [trimmedTargetPath]);

  useEffect(() => {
    if (!open || !trimmedTargetPath || selectedIds.length === 0) {
      setPreviews([]);
      return;
    }

    let cancelled = false;
    setLoadingPreview(true);
    setErrorMsg(null);

    void api
      .previewMoveSkillsToLibrary(selectedIds, trimmedTargetPath, scope)
      .then((res) => {
        if (!cancelled) {
          setPreviews(res);
        }
      })
      .catch((err) => {
        if (!cancelled) {
          setPreviews([]);
          setErrorMsg(err instanceof Error ? err.message : String(err));
        }
      })
      .finally(() => {
        if (!cancelled) {
          setLoadingPreview(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [open, selectedIds, trimmedTargetPath, scope]);

  const hasConflict = useMemo(
    () => previews.some((p) => p.conflictExists),
    [previews],
  );

  const movableCount = useMemo(
    () => previews.filter((p) => !p.conflictExists).length,
    [previews],
  );

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

  const handleBrowseDestination = async () => {
    const picked = await api.pickDirectory(trimmedTargetPath || null);
    if (picked) {
      setTargetLibraryPath(picked);
    }
  };

  const toggleSelection = (id: string) => {
    setSelectedIds((prev) =>
      prev.includes(id) ? prev.filter((item) => item !== id) : [...prev, id],
    );
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!trimmedTargetPath || movableCount === 0) {
      return;
    }
    setExecuting(true);
    setErrorMsg(null);
    try {
      const movableIds = previews
        .filter((p) => !p.conflictExists)
        .map((p) => p.skillId);
      const res = await api.executeMoveSkillsToLibrary(
        movableIds,
        trimmedTargetPath,
        scope,
        afterMove,
      );
      if (res.warnings.length > 0 && res.moved.length === 0) {
        setErrorMsg(res.warnings.join('\n'));
        return;
      }
      if (
        registerAsLibrary &&
        !isKnownLibrarySource &&
        onRegisterLibrarySource
      ) {
        await onRegisterLibrarySource(trimmedTargetPath);
      }
      await onSuccess();
      handleClose();
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    } finally {
      setExecuting(false);
    }
  };

  return (
    <dialog
      ref={dialogRef}
      aria-labelledby="debloat-modal-title"
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
      className="mac-sheet m-auto w-[680px] max-w-[calc(100vw-48px)] max-h-[calc(100vh-56px)] overflow-hidden p-0"
    >
      <form
        onSubmit={handleSubmit}
        className="flex max-h-[calc(100vh-56px)] flex-col"
      >
        <div className="flex shrink-0 items-center justify-between border-b border-[var(--hairline)] bg-[var(--bg-toolbar)] px-5 py-3.5 backdrop-blur-xl">
          <div className="min-w-0 pr-3">
            <h2 id="debloat-modal-title" className="text-[14px] font-semibold">
              Consolidate Skills into Library
              {scopeLabel ? ` · ${scopeLabel}` : ''}
            </h2>
            <p className="text-[12px] text-[var(--text-secondary)]">
              Safely moves loose skill folders out of agent directories into your central Library so AgentDock can toggle them on and off with lightweight links. No files are deleted.
            </p>
          </div>
          <button
            type="button"
            onClick={() => dialogRef.current?.close()}
            aria-label="Close consolidation dialog"
            className="inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-[6px] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
          >
            <X className="h-4 w-4" strokeWidth={1.75} />
          </button>
        </div>

        <div className="flex-1 space-y-4 overflow-y-auto px-5 py-4">
          {errorMsg && (
            <div className="rounded-[8px] border border-[var(--status-red)]/30 bg-[var(--status-red)]/10 px-3 py-2 text-[12.5px] text-[var(--status-red)]">
              {errorMsg}
            </div>
          )}

          <div className="mac-group grid grid-cols-1 gap-4 p-3.5 sm:grid-cols-2">
            <div className="space-y-2">
              <label
                htmlFor={
                  librarySources.length > 0
                    ? 'debloat-library-select'
                    : 'debloat-library-path'
                }
                className="block text-[12px] font-medium text-[var(--text-primary)]"
              >
                Destination Library Folder
              </label>
              <p className="text-[11.5px] text-[var(--text-secondary)]">
                Where your permanent copy will live (such as your Obsidian vault or central Skills folder).
              </p>

              {librarySources.length > 0 && (
                <MacSelect
                  id="debloat-library-select"
                  value={
                    isKnownLibrarySource
                      ? librarySources.find(
                          (s) =>
                            normalizeTrimmedPath(s.path) === trimmedTargetPath,
                        )?.path ?? ''
                      : '__custom__'
                  }
                  onChange={(e) => {
                    if (e.target.value === '__custom__') {
                      void handleBrowseDestination();
                    } else {
                      setTargetLibraryPath(e.target.value);
                    }
                  }}
                  className="w-full"
                >
                  {librarySources.map((src) => (
                    <option key={src.id} value={src.path}>
                      {src.label} ({src.path})
                    </option>
                  ))}
                  <option value="__custom__">Custom folder…</option>
                </MacSelect>
              )}

              <div className="flex gap-2">
                <input
                  id="debloat-library-path"
                  type="text"
                  autoFocus={librarySources.length === 0}
                  value={targetLibraryPath}
                  onChange={(e) => setTargetLibraryPath(e.target.value)}
                  placeholder="~/Skills or /path/to/library"
                  aria-label="Destination folder path"
                  className="mac-input mono-path h-8 min-w-0 flex-1 px-2.5 text-[12px]"
                />
                <button
                  type="button"
                  onClick={() => void handleBrowseDestination()}
                  className="mac-btn inline-flex h-8 shrink-0 items-center gap-1.5 px-2.5 text-[12px] font-medium"
                >
                  <FolderOpen className="h-3.5 w-3.5" strokeWidth={1.75} />
                  Browse…
                </button>
              </div>

              {trimmedTargetPath && !isKnownLibrarySource && onRegisterLibrarySource && (
                <div className="flex items-center justify-between gap-2 pt-0.5 text-[11.5px] text-[var(--text-secondary)]">
                  <span>Add &ldquo;{targetFolderName}&rdquo; to Library Sources</span>
                  <MacSwitch
                    checked={registerAsLibrary}
                    label={`Add ${targetFolderName} to Library Sources`}
                    onChange={setRegisterAsLibrary}
                  />
                </div>
              )}
            </div>

            <div className="space-y-1.5">
              <span className="block text-[12px] font-medium text-[var(--text-primary)]">
                After Moving
              </span>
              <div className="flex flex-col gap-2 rounded-[8px] border border-[var(--hairline-inset)] bg-[var(--bg-canvas)] px-3 py-2.5 text-[12px]">
                <label className="flex cursor-pointer items-start gap-2">
                  <input
                    type="radio"
                    name="after-move"
                    className="mt-0.5"
                    checked={afterMove === 'unlink'}
                    onChange={() => setAfterMove('unlink')}
                  />
                  <div>
                    <div className="font-medium text-[var(--text-primary)]">
                      Turn off for now (Recommended to save tokens)
                    </div>
                    <div className="text-[11px] text-[var(--text-secondary)]">
                      Moves the folder to your Library and removes it from agent directories. Nothing is lost — switch it on per project anytime.
                    </div>
                  </div>
                </label>
                <label className="flex cursor-pointer items-start gap-2">
                  <input
                    type="radio"
                    name="after-move"
                    className="mt-0.5"
                    checked={afterMove === 'relink'}
                    onChange={() => setAfterMove('relink')}
                  />
                  <div>
                    <div className="font-medium text-[var(--text-primary)]">
                      Keep active via lightweight link (symlink)
                    </div>
                    <div className="text-[11px] text-[var(--text-secondary)]">
                      Moves the folder to your Library and leaves a shortcut behind so the skill keeps working right away, now toggleable.
                    </div>
                  </div>
                </label>
              </div>
            </div>
          </div>

          {/* Skill checklist */}
          <div className="space-y-1.5">
            <div className="flex items-center justify-between px-1">
              <span className="text-[12px] font-semibold text-[var(--text-secondary)]">
                Skills to Consolidate ({selectedIds.length} of {skills.length})
              </span>
              <div className="flex gap-3 text-[12px]">
                <button
                  type="button"
                  onClick={() => setSelectedIds(skills.map((s) => s.id))}
                  className="text-[var(--accent)] hover:underline"
                >
                  Select All
                </button>
                <button
                  type="button"
                  onClick={() => setSelectedIds([])}
                  className="text-[var(--text-secondary)] hover:underline"
                >
                  Clear
                </button>
              </div>
            </div>

            <div className="mac-group max-h-36 divide-y divide-[var(--hairline-inset)] overflow-y-auto">
              {skills.map((skill) => {
                const checked = selectedIds.includes(skill.id);
                return (
                  <label
                    key={skill.id}
                    className="flex cursor-pointer items-center justify-between px-3.5 py-1.5 text-[12.5px] hover:bg-[var(--bg-hover)]"
                  >
                    <span className="flex items-center gap-2">
                      <input
                        type="checkbox"
                        checked={checked}
                        onChange={() => toggleSelection(skill.id)}
                      />
                      <span className="font-medium">{skill.name}</span>
                    </span>
                    <span className="tabular-nums text-[11.5px] text-[var(--text-tertiary)]">
                      {skill.upfrontTokens} tok
                    </span>
                  </label>
                );
              })}
            </div>
          </div>

          {/* Dry-run Path Preview */}
          <div className="space-y-1.5">
            <span className="block px-1 text-[12px] font-semibold text-[var(--text-secondary)]">
              Preview — Exact File Changes {loadingPreview ? '(updating…)' : ''}
            </span>
            {previews.length === 0 ? (
              <div className="mac-group px-3.5 py-3 text-[12px] text-[var(--text-tertiary)]">
                {trimmedTargetPath
                  ? 'Select one or more loose skill folders above to preview moves.'
                  : 'Choose or browse a destination folder above to preview moves.'}
              </div>
            ) : (
              <div className="mac-group max-h-48 divide-y divide-[var(--hairline-inset)] overflow-y-auto">
                {previews.map((item) => (
                  <div key={item.skillId} className="space-y-1 px-3.5 py-2 text-[12px]">
                    <div className="flex items-center justify-between gap-2">
                      <span className="font-medium text-[var(--text-primary)]">
                        {item.skillName}
                      </span>
                      {item.conflictExists ? (
                        <span className="inline-flex items-center gap-1 text-[11.5px] font-medium text-[var(--status-amber)]">
                          <AlertTriangle className="h-3.5 w-3.5" />
                          Already exists in Library (will skip)
                        </span>
                      ) : (
                        <span className="inline-flex items-center gap-1 text-[11.5px] text-[var(--status-green)]">
                          <CheckCircle2 className="h-3.5 w-3.5" />
                          Ready to move
                        </span>
                      )}
                    </div>
                    <div className="mono-path flex flex-wrap items-center gap-1.5 text-[11px] text-[var(--text-secondary)]">
                      <span className="break-all">{item.sourceRealDir}</span>
                      <ArrowRight className="h-3 w-3 shrink-0 text-[var(--text-tertiary)]" />
                      <span className="break-all text-[var(--text-primary)]">
                        {item.destinationDir}
                      </span>
                    </div>
                    {item.symlinksToUpdate.length > 0 && (
                      <div className="text-[11px] text-[var(--text-tertiary)]">
                        Also {afterMove === 'unlink' ? 'unlinks' : 'relinks'}{' '}
                        {item.symlinksToUpdate.length} secondary symlink(s)
                      </div>
                    )}
                  </div>
                ))}
              </div>
            )}
            {hasConflict && (
              <p className="px-1 text-[11.5px] text-[var(--text-secondary)]">
                Existing library folders are never overwritten; conflicting items will be skipped safely.
              </p>
            )}
          </div>
        </div>

        <div className="flex shrink-0 items-center justify-end gap-2.5 border-t border-[var(--hairline)] bg-[var(--bg-toolbar)] px-5 py-3 backdrop-blur-xl">
          <button
            type="button"
            onClick={() => dialogRef.current?.close()}
            className="mac-btn h-8 px-3.5 text-[12.5px] font-medium"
          >
            Cancel
          </button>
          <button
            type="submit"
            disabled={executing || movableCount === 0 || !trimmedTargetPath}
            className="mac-btn-primary h-8 px-4 text-[12.5px] font-medium disabled:cursor-not-allowed disabled:opacity-50"
          >
            {executing
              ? 'Moving…'
              : afterMove === 'unlink'
                ? `Move & Turn Off (${movableCount})`
                : `Move & Keep Linked (${movableCount})`}
          </button>
        </div>
      </form>
    </dialog>
  );
}

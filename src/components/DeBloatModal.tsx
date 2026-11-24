import { useEffect, useMemo, useRef, useState } from 'react';
import { AlertTriangle, ArrowRight, CheckCircle2, X } from 'lucide-react';
import type {
  AfterMoveAction,
  LibrarySource,
  MovePreviewItem,
  SkillItem,
  ToggleScope,
} from '@/types';
import { api } from '@/lib/api';

interface DeBloatModalProps {
  open: boolean;
  skills: SkillItem[];
  initialSelectedIds: string[];
  librarySources: LibrarySource[];
  scope: ToggleScope;
  onClose: () => void;
  onSuccess: () => Promise<void>;
}

export function DeBloatModal({
  open,
  skills,
  initialSelectedIds,
  librarySources,
  scope,
  onClose,
  onSuccess,
}: DeBloatModalProps) {
  const dialogRef = useRef<HTMLDialogElement | null>(null);
  const pressedOutsideRef = useRef(false);

  const [targetLibraryPath, setTargetLibraryPath] = useState(
    librarySources[0]?.path ?? '',
  );
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
      setTargetLibraryPath(librarySources[0]?.path ?? '');
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

  useEffect(() => {
    if (!open || !targetLibraryPath || selectedIds.length === 0) {
      setPreviews([]);
      return;
    }

    let cancelled = false;
    setLoadingPreview(true);
    setErrorMsg(null);

    void api
      .previewMoveSkillsToLibrary(selectedIds, targetLibraryPath, scope)
      .then((res) => {
        if (!cancelled) {
          setPreviews(res);
        }
      })
      .catch((err) => {
        if (!cancelled) {
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
  }, [open, selectedIds, targetLibraryPath, scope]);

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

  const toggleSelection = (id: string) => {
    setSelectedIds((prev) =>
      prev.includes(id) ? prev.filter((item) => item !== id) : [...prev, id],
    );
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!targetLibraryPath || movableCount === 0) {
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
        targetLibraryPath,
        scope,
        afterMove,
      );
      if (res.warnings.length > 0 && res.moved.length === 0) {
        setErrorMsg(res.warnings.join('\n'));
        return;
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
      className="m-auto w-[680px] max-w-[calc(100vw-48px)] max-h-[calc(100vh-56px)] overflow-hidden rounded-[12px] border border-[var(--hairline)] bg-[var(--bg-content)] p-0 text-[var(--text-primary)] shadow-[0_20px_50px_rgba(0,0,0,0.28)]"
    >
      <form
        onSubmit={handleSubmit}
        className="flex max-h-[calc(100vh-56px)] flex-col"
      >
        <div className="flex shrink-0 items-center justify-between border-b border-[var(--hairline)] px-5 py-3.5">
          <div>
            <h2 id="debloat-modal-title" className="text-[15px] font-semibold">
              Move Skills to Library
            </h2>
            <p className="text-[12px] text-[var(--text-secondary)]">
              Safely migrate unmanaged skill folders into your canonical library before unlinking.
            </p>
          </div>
          <button
            type="button"
            onClick={() => dialogRef.current?.close()}
            aria-label="Close migration dialog"
            className="inline-flex h-7 w-7 items-center justify-center rounded-[6px] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
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

          {librarySources.length === 0 ? (
            <div className="rounded-[8px] border border-[var(--status-amber)]/30 bg-[var(--status-amber)]/10 px-3.5 py-3 text-[12.5px] text-[var(--text-primary)]">
              Configure at least one Library Source in Settings before migrating skills.
            </div>
          ) : (
            <>
              <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
                <div className="space-y-1.5">
                  <label
                    htmlFor="debloat-library-select"
                    className="block text-[12px] font-medium text-[var(--text-secondary)]"
                  >
                    Destination Library
                  </label>
                  <select
                    id="debloat-library-select"
                    autoFocus
                    value={targetLibraryPath}
                    onChange={(e) => setTargetLibraryPath(e.target.value)}
                    className="h-8 w-full rounded-[6px] border border-[var(--hairline)] bg-[var(--bg-elevated)] px-2.5 text-[12.5px]"
                  >
                    {librarySources.map((src) => (
                      <option key={src.id} value={src.path}>
                        {src.label} ({src.path})
                      </option>
                    ))}
                  </select>
                </div>

                <div className="space-y-1.5">
                  <span className="block text-[12px] font-medium text-[var(--text-secondary)]">
                    After Moving
                  </span>
                  <div className="flex flex-col gap-1.5 rounded-[6px] border border-[var(--hairline)] bg-[var(--bg-elevated)] px-3 py-2 text-[12px]">
                    <label className="flex cursor-pointer items-center gap-2">
                      <input
                        type="radio"
                        name="after-move"
                        checked={afterMove === 'unlink'}
                        onChange={() => setAfterMove('unlink')}
                      />
                      <span>Unlink from agent folders (frees startup tokens)</span>
                    </label>
                    <label className="flex cursor-pointer items-center gap-2">
                      <input
                        type="radio"
                        name="after-move"
                        checked={afterMove === 'relink'}
                        onChange={() => setAfterMove('relink')}
                      />
                      <span>Keep active via symlink to Library</span>
                    </label>
                  </div>
                </div>
              </div>

              {/* Skill checklist */}
              <div className="space-y-1.5">
                <div className="flex items-center justify-between">
                  <span className="text-[12px] font-medium text-[var(--text-secondary)]">
                    Select Unmanaged Folders ({selectedIds.length} of {skills.length})
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

                <div className="max-h-36 overflow-y-auto rounded-[8px] border border-[var(--hairline)] bg-[var(--bg-elevated)] divide-y divide-[var(--hairline)]">
                  {skills.map((skill) => {
                    const checked = selectedIds.includes(skill.id);
                    return (
                      <label
                        key={skill.id}
                        className="flex cursor-pointer items-center justify-between px-3 py-1.5 text-[12.5px] hover:bg-[var(--bg-hover)]"
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
                <span className="block text-[12px] font-medium text-[var(--text-secondary)]">
                  Dry-Run Path Preview {loadingPreview ? '(updating…)' : ''}
                </span>
                {previews.length === 0 ? (
                  <div className="rounded-[8px] border border-[var(--hairline)] bg-[var(--bg-elevated)] px-3 py-3 text-[12px] text-[var(--text-tertiary)]">
                    Select one or more unmanaged skill folders above to preview moves.
                  </div>
                ) : (
                  <div className="max-h-48 overflow-y-auto rounded-[8px] border border-[var(--hairline)] bg-[var(--bg-elevated)] divide-y divide-[var(--hairline)]">
                    {previews.map((item) => (
                      <div key={item.skillId} className="space-y-1 px-3 py-2 text-[12px]">
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
                  <p className="text-[11.5px] text-[var(--text-secondary)]">
                    Existing library folders are never overwritten; conflicting items will be skipped safely.
                  </p>
                )}
              </div>
            </>
          )}
        </div>

        <div className="flex shrink-0 items-center justify-end gap-2.5 border-t border-[var(--hairline)] bg-[var(--bg-elevated)] px-5 py-3">
          <button
            type="button"
            onClick={() => dialogRef.current?.close()}
            className="h-8 rounded-[6px] border border-[var(--hairline)] bg-[var(--bg-content)] px-3.5 text-[12.5px] font-medium text-[var(--text-primary)] hover:bg-[var(--bg-hover)]"
          >
            Cancel
          </button>
          <button
            type="submit"
            disabled={executing || movableCount === 0 || !targetLibraryPath}
            className="h-8 rounded-[6px] bg-[var(--accent)] px-4 text-[12.5px] font-medium text-[var(--accent-text)] hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-50"
          >
            {executing
              ? 'Moving…'
              : afterMove === 'unlink'
                ? `Move & Unlink (${movableCount})`
                : `Move & Relink (${movableCount})`}
          </button>
        </div>
      </form>
    </dialog>
  );
}

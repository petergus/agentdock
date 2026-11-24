import { useEffect, useRef, useState } from 'react';
import { X } from 'lucide-react';
import type { RuleItem, RuleTrigger, RuleTriggerUpdate } from '@/types';
import { api } from '@/lib/api';
import { formatTokensExact } from '@/lib/format';

interface RuleTriggerModalProps {
  rule: RuleItem | null;
  onClose: () => void;
  onSuccess: () => Promise<void>;
}

export function RuleTriggerModal({
  rule,
  onClose,
  onSuccess,
}: RuleTriggerModalProps) {
  const dialogRef = useRef<HTMLDialogElement | null>(null);
  const pressedOutsideRef = useRef(false);

  const [mode, setMode] = useState<Exclude<RuleTrigger, 'invalid'>>('alwaysOn');
  const [description, setDescription] = useState('');
  const [globPattern, setGlobPattern] = useState('');
  const [saving, setSaving] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) {
      return;
    }
    if (rule) {
      setMode(rule.trigger === 'invalid' ? 'alwaysOn' : rule.trigger);
      setDescription(rule.description ?? '');
      setGlobPattern(rule.globPattern ?? '**/*.ts');
      setErrorMsg(null);
      if (!dialog.open) {
        dialog.showModal();
      }
    } else if (dialog.open) {
      dialog.close();
    }
  }, [rule]);

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

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!rule) {
      return;
    }

    let update: RuleTriggerUpdate;
    if (mode === 'alwaysOn') {
      update = { trigger: 'alwaysOn' };
    } else if (mode === 'modelDecision') {
      if (!description.trim()) {
        setErrorMsg('Model Decision rules require a description.');
        return;
      }
      update = { trigger: 'modelDecision', description: description.trim() };
    } else {
      if (!globPattern.trim()) {
        setErrorMsg('Glob rules require a file pattern.');
        return;
      }
      update = { trigger: 'glob', pattern: globPattern.trim() };
    }

    setSaving(true);
    setErrorMsg(null);
    try {
      await api.updateRuleTrigger(rule.canonicalPath, update);
      await onSuccess();
      handleClose();
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  return (
    <dialog
      ref={dialogRef}
      aria-labelledby="rule-trigger-modal-title"
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
      className="m-auto w-[480px] max-w-[calc(100vw-48px)] overflow-hidden rounded-[12px] border border-[var(--hairline)] bg-[var(--bg-content)] p-0 text-[var(--text-primary)] shadow-[0_20px_50px_rgba(0,0,0,0.28)]"
    >
      <form onSubmit={handleSubmit} className="flex flex-col">
        <div className="flex items-center justify-between border-b border-[var(--hairline)] px-5 py-3.5">
          <div className="min-w-0">
            <h2
              id="rule-trigger-modal-title"
              className="truncate text-[15px] font-semibold"
            >
              Rule Trigger: {rule?.filename}
            </h2>
            <p className="text-[12px] text-[var(--text-secondary)]">
              Updates only YAML frontmatter; markdown body and comments stay untouched.
            </p>
          </div>
          <button
            type="button"
            onClick={() => dialogRef.current?.close()}
            aria-label="Close rule trigger dialog"
            className="inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-[6px] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
          >
            <X className="h-4 w-4" strokeWidth={1.75} />
          </button>
        </div>

        <div className="space-y-3.5 px-5 py-4">
          {errorMsg && (
            <div className="rounded-[8px] border border-[var(--status-red)]/30 bg-[var(--status-red)]/10 px-3 py-2 text-[12.5px] text-[var(--status-red)]">
              {errorMsg}
            </div>
          )}

          <label className="flex cursor-pointer items-start gap-2.5 rounded-[8px] border border-[var(--hairline)] bg-[var(--bg-elevated)] p-3">
            <input
              type="radio"
              name="rule-trigger"
              className="mt-0.5"
              autoFocus
              checked={mode === 'alwaysOn'}
              onChange={() => setMode('alwaysOn')}
            />
            <div className="flex-1 text-[12.5px]">
              <div className="flex justify-between font-medium">
                <span>Always On (always_on)</span>
                <span className="tabular-nums text-[var(--text-secondary)]">
                  {rule ? `${formatTokensExact(rule.fullTokens)} tok` : ''}
                </span>
              </div>
              <div className="text-[11.5px] text-[var(--text-secondary)]">
                Loaded into every startup prompt at 100% of file token cost.
              </div>
            </div>
          </label>

          <label className="flex cursor-pointer items-start gap-2.5 rounded-[8px] border border-[var(--hairline)] bg-[var(--bg-elevated)] p-3">
            <input
              type="radio"
              name="rule-trigger"
              className="mt-0.5"
              checked={mode === 'modelDecision'}
              onChange={() => setMode('modelDecision')}
            />
            <div className="flex-1 space-y-2 text-[12.5px]">
              <div>
                <div className="font-medium">Model Decision (model_decision)</div>
                <div className="text-[11.5px] text-[var(--text-secondary)]">
                  Only the description counts upfront; full rule loads on demand.
                </div>
              </div>
              {mode === 'modelDecision' && (
                <input
                  type="text"
                  value={description}
                  onChange={(e) => setDescription(e.target.value)}
                  placeholder="Describe when the agent should read this rule…"
                  aria-label="Model decision description"
                  className="h-8 w-full rounded-[6px] border border-[var(--hairline)] bg-[var(--bg-content)] px-2.5 text-[12.5px]"
                />
              )}
            </div>
          </label>

          <label className="flex cursor-pointer items-start gap-2.5 rounded-[8px] border border-[var(--hairline)] bg-[var(--bg-elevated)] p-3">
            <input
              type="radio"
              name="rule-trigger"
              className="mt-0.5"
              checked={mode === 'glob'}
              onChange={() => setMode('glob')}
            />
            <div className="flex-1 space-y-2 text-[12.5px]">
              <div>
                <div className="flex justify-between font-medium">
                  <span>Glob Pattern (glob)</span>
                  <span className="tabular-nums text-[var(--status-green)]">
                    0 tok upfront
                  </span>
                </div>
                <div className="text-[11.5px] text-[var(--text-secondary)]">
                  Loaded only when working on files matching the glob pattern.
                </div>
              </div>
              {mode === 'glob' && (
                <input
                  type="text"
                  value={globPattern}
                  onChange={(e) => setGlobPattern(e.target.value)}
                  placeholder="src/**/*.tsx"
                  aria-label="Glob pattern"
                  className="mono-path h-8 w-full rounded-[6px] border border-[var(--hairline)] bg-[var(--bg-content)] px-2.5 text-[12px]"
                />
              )}
            </div>
          </label>
        </div>

        <div className="flex items-center justify-end gap-2.5 border-t border-[var(--hairline)] bg-[var(--bg-elevated)] px-5 py-3">
          <button
            type="button"
            onClick={() => dialogRef.current?.close()}
            className="h-8 rounded-[6px] border border-[var(--hairline)] bg-[var(--bg-content)] px-3.5 text-[12.5px] font-medium hover:bg-[var(--bg-hover)]"
          >
            Cancel
          </button>
          <button
            type="submit"
            disabled={saving}
            className="h-8 rounded-[6px] bg-[var(--accent)] px-4 text-[12.5px] font-medium text-[var(--accent-text)] hover:opacity-90 disabled:opacity-60"
          >
            {saving ? 'Saving…' : 'Update Trigger'}
          </button>
        </div>
      </form>
    </dialog>
  );
}

import { useEffect, useRef, useState } from 'react';
import { FileText, FolderOpen, X } from 'lucide-react';
import type { SupportDocMode, SupportDocTargetFile } from '@/types';
import { api } from '@/lib/api';

interface SupportDocsModalProps {
  open: boolean;
  projectPath: string;
  supportDocsRoot: string | null;
  onClose: () => void;
  onSuccess: () => Promise<void>;
}

export function SupportDocsModal({
  open,
  projectPath,
  supportDocsRoot,
  onClose,
  onSuccess,
}: SupportDocsModalProps) {
  const dialogRef = useRef<HTMLDialogElement | null>(null);
  const pressedOutsideRef = useRef(false);

  const [sourcePath, setSourcePath] = useState('');
  const [mode, setMode] = useState<SupportDocMode>('pointer');
  const [targetFile, setTargetFile] =
    useState<SupportDocTargetFile>('agentsMd');
  const [description, setDescription] = useState('');
  const [saving, setSaving] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) {
      return;
    }
    if (open) {
      setSourcePath('');
      setMode('pointer');
      setTargetFile('agentsMd');
      setDescription('');
      setErrorMsg(null);
      if (!dialog.open) {
        dialog.showModal();
      }
    } else if (dialog.open) {
      dialog.close();
    }
  }, [open]);

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

  const handlePickFolder = async () => {
    const picked = await api.pickDirectory(supportDocsRoot);
    if (picked) {
      setSourcePath(picked);
    }
  };

  const handlePickFile = async () => {
    const picked = await api.pickFile(supportDocsRoot);
    if (picked) {
      setSourcePath(picked);
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!sourcePath.trim()) {
      setErrorMsg('Choose or enter a folder or file path first.');
      return;
    }
    setSaving(true);
    setErrorMsg(null);
    try {
      await api.linkSupportDoc({
        projectPath,
        sourcePath: sourcePath.trim(),
        mode,
        description: mode === 'pointer' ? description.trim() || null : null,
        targetFile: mode === 'pointer' ? targetFile : null,
      });
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
      aria-labelledby="support-docs-modal-title"
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
      className="m-auto w-[560px] max-w-[calc(100vw-48px)] overflow-hidden rounded-[12px] border border-[var(--hairline)] bg-[var(--bg-content)] p-0 text-[var(--text-primary)] shadow-[0_20px_50px_rgba(0,0,0,0.28)]"
    >
      <form onSubmit={handleSubmit} className="flex flex-col">
        <div className="flex items-center justify-between border-b border-[var(--hairline)] px-5 py-3.5">
          <div>
            <h2
              id="support-docs-modal-title"
              className="text-[15px] font-semibold"
            >
              Link Support Docs
            </h2>
            <p className="text-[12px] text-[var(--text-secondary)]">
              Reference external Obsidian notes or spec folders without copying files.
            </p>
          </div>
          <button
            type="button"
            onClick={() => dialogRef.current?.close()}
            aria-label="Close support docs dialog"
            className="inline-flex h-7 w-7 items-center justify-center rounded-[6px] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
          >
            <X className="h-4 w-4" strokeWidth={1.75} />
          </button>
        </div>

        <div className="space-y-4 px-5 py-4">
          {errorMsg && (
            <div className="rounded-[8px] border border-[var(--status-red)]/30 bg-[var(--status-red)]/10 px-3 py-2 text-[12.5px] text-[var(--status-red)]">
              {errorMsg}
            </div>
          )}

          <div className="space-y-1.5">
            <label
              htmlFor="support-doc-source-input"
              className="block text-[12.5px] font-medium text-[var(--text-primary)]"
            >
              External File or Folder
            </label>
            <div className="flex gap-2">
              <input
                id="support-doc-source-input"
                type="text"
                autoFocus
                value={sourcePath}
                onChange={(e) => setSourcePath(e.target.value)}
                placeholder={supportDocsRoot ?? '/Users/peter/BRAIN-LOCAL/...'}
                className="mono-path h-8 flex-1 rounded-[6px] border border-[var(--hairline)] bg-[var(--bg-elevated)] px-2.5 text-[12px]"
              />
              <button
                type="button"
                onClick={handlePickFolder}
                className="inline-flex h-8 items-center gap-1 rounded-[6px] border border-[var(--hairline)] bg-[var(--bg-elevated)] px-2.5 text-[12px] font-medium hover:bg-[var(--bg-hover)]"
              >
                <FolderOpen className="h-3.5 w-3.5" strokeWidth={1.75} />
                Folder…
              </button>
              <button
                type="button"
                onClick={handlePickFile}
                className="inline-flex h-8 items-center gap-1 rounded-[6px] border border-[var(--hairline)] bg-[var(--bg-elevated)] px-2.5 text-[12px] font-medium hover:bg-[var(--bg-hover)]"
              >
                <FileText className="h-3.5 w-3.5" strokeWidth={1.75} />
                File…
              </button>
            </div>
          </div>

          <div className="space-y-2.5">
            <span className="block text-[12.5px] font-medium text-[var(--text-primary)]">
              Link Mode
            </span>

            <label className="flex cursor-pointer items-start gap-2.5 rounded-[8px] border border-[var(--hairline)] bg-[var(--bg-elevated)] p-3">
              <input
                type="radio"
                name="support-doc-mode"
                className="mt-0.5"
                checked={mode === 'pointer'}
                onChange={() => setMode('pointer')}
              />
              <div className="flex-1 space-y-2 text-[12.5px]">
                <div>
                  <div className="font-medium">
                    Zero-Token Rule Pointer (Recommended)
                  </div>
                  <div className="text-[11.5px] text-[var(--text-secondary)]">
                    Inserts a managed pointer block so agents read docs only when needed.
                  </div>
                </div>

                {mode === 'pointer' && (
                  <div className="space-y-2 pt-1">
                    <select
                      aria-label="Target pointer file"
                      value={targetFile}
                      onChange={(e) =>
                        setTargetFile(e.target.value as SupportDocTargetFile)
                      }
                      className="h-7 w-full rounded-[6px] border border-[var(--hairline)] bg-[var(--bg-content)] px-2 text-[12px]"
                    >
                      <option value="agentsMd">
                        Embed in AGENTS.md (AGENTDOCK:SUPPORT_DOCS block)
                      </option>
                      <option value="supportDocsRule">
                        Create .agents/rules/support-docs.md (model_decision)
                      </option>
                    </select>
                    <input
                      type="text"
                      value={description}
                      onChange={(e) => setDescription(e.target.value)}
                      placeholder="Optional 1-line hint (e.g. Client brand specs & API notes)"
                      aria-label="Support doc description"
                      className="h-8 w-full rounded-[6px] border border-[var(--hairline)] bg-[var(--bg-content)] px-2.5 text-[12.5px]"
                    />
                  </div>
                )}
              </div>
            </label>

            <label className="flex cursor-pointer items-start gap-2.5 rounded-[8px] border border-[var(--hairline)] bg-[var(--bg-elevated)] p-3">
              <input
                type="radio"
                name="support-doc-mode"
                className="mt-0.5"
                checked={mode === 'symlink'}
                onChange={() => setMode('symlink')}
              />
              <div className="flex-1 text-[12.5px]">
                <div className="font-medium">Local Reference Symlink</div>
                <div className="text-[11.5px] text-[var(--text-secondary)]">
                  Creates <span className="mono-path">_support-docs/</span> symlink and hides it via{' '}
                  <span className="mono-path">.git/info/exclude</span>.
                </div>
              </div>
            </label>
          </div>
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
            {saving ? 'Linking…' : 'Link Support Doc'}
          </button>
        </div>
      </form>
    </dialog>
  );
}

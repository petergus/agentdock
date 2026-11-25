import { useEffect, useRef, useState } from 'react';
import { FileText, FolderOpen, X } from 'lucide-react';
import type { SupportDocMode, SupportDocTargetFile } from '@/types';
import { api } from '@/lib/api';
import { MacSelect } from '@/components/ui/MacSelect';

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
      className="mac-sheet m-auto w-[560px] max-w-[calc(100vw-48px)] overflow-hidden p-0"
    >
      <form onSubmit={handleSubmit} className="flex flex-col">
        <div className="flex items-center justify-between border-b border-[var(--hairline)] bg-[var(--bg-toolbar)] px-5 py-3.5 backdrop-blur-xl">
          <div>
            <h2
              id="support-docs-modal-title"
              className="text-[14px] font-semibold"
            >
              Link Support Docs
            </h2>
            <p className="text-[12px] text-[var(--text-secondary)]">
              Reference external notes, specs, or documentation folders without copying files.
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
              className="block px-1 text-[12px] font-semibold text-[var(--text-secondary)]"
            >
              External File or Folder
            </label>
            <div className="mac-group flex gap-2 p-2.5">
              <input
                id="support-doc-source-input"
                type="text"
                autoFocus
                value={sourcePath}
                onChange={(e) => setSourcePath(e.target.value)}
                placeholder={supportDocsRoot ?? '/path/to/notes-or-docs'}
                className="mac-input mono-path h-8 flex-1 px-2.5 text-[12px]"
              />
              <button
                type="button"
                onClick={handlePickFolder}
                className="mac-btn inline-flex h-8 items-center gap-1 px-2.5 text-[12px] font-medium"
              >
                <FolderOpen className="h-3.5 w-3.5" strokeWidth={1.75} />
                Folder…
              </button>
              <button
                type="button"
                onClick={handlePickFile}
                className="mac-btn inline-flex h-8 items-center gap-1 px-2.5 text-[12px] font-medium"
              >
                <FileText className="h-3.5 w-3.5" strokeWidth={1.75} />
                File…
              </button>
            </div>
          </div>

          <div className="space-y-2">
            <span className="block px-1 text-[12px] font-semibold text-[var(--text-secondary)]">
              Link Mode
            </span>

            <label className="mac-group flex cursor-pointer items-start gap-2.5 p-3">
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
                    <MacSelect
                      size="sm"
                      aria-label="Target pointer file"
                      value={targetFile}
                      onChange={(e) =>
                        setTargetFile(e.target.value as SupportDocTargetFile)
                      }
                      className="w-full"
                    >
                      <option value="agentsMd">
                        Embed in AGENTS.md (AGENTDOCK:SUPPORT_DOCS block)
                      </option>
                      <option value="supportDocsRule">
                        Create .agents/rules/support-docs.md (model_decision)
                      </option>
                    </MacSelect>
                    <input
                      type="text"
                      value={description}
                      onChange={(e) => setDescription(e.target.value)}
                      placeholder="Optional 1-line hint (e.g. Client brand specs & API notes)"
                      aria-label="Support doc description"
                      className="mac-input h-8 w-full px-2.5 text-[12.5px]"
                    />
                  </div>
                )}
              </div>
            </label>

            <label className="mac-group flex cursor-pointer items-start gap-2.5 p-3">
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

        <div className="flex items-center justify-end gap-2.5 border-t border-[var(--hairline)] bg-[var(--bg-toolbar)] px-5 py-3 backdrop-blur-xl">
          <button
            type="button"
            onClick={() => dialogRef.current?.close()}
            className="mac-btn h-8 px-3.5 text-[12.5px] font-medium"
          >
            Cancel
          </button>
          <button
            type="submit"
            disabled={saving}
            className="mac-btn-primary h-8 px-4 text-[12.5px] font-medium disabled:opacity-60"
          >
            {saving ? 'Linking…' : 'Link Support Doc'}
          </button>
        </div>
      </form>
    </dialog>
  );
}

import { useEffect, useRef, useState } from 'react';
import type { ProjectSummary, SetChangeAction, SetItemPreview, SetOperation, SetPreview } from '@/types';
import { api } from '@/lib/api';
import { agentDisplayName, formatTokensExact } from '@/lib/format';

export interface SetPreviewRequest {
  project: ProjectSummary;
  setId: string;
  setName: string;
  operation: SetOperation;
}

const actionLabels: Record<SetChangeAction, string> = {
  createLink: 'Create symlink',
  replaceLink: 'Replace symlink',
  removeLink: 'Remove symlink',
  writeConfig: 'Write server definition',
  removeConfigEntry: 'Remove server definition',
};

const itemLabels = { skill: 'Skill', rule: 'Rule', mcp: 'MCP' };
const statusLabels = { change: 'Will change', alreadyMatches: 'No file change', shared: 'Kept', blocked: 'Blocked' };

interface Props {
  request: SetPreviewRequest;
  onClose: () => void;
  onApply: (revision: string) => Promise<void>;
}

export function SetPreviewModal({ request, onClose, onApply }: Props) {
  const { project, setId, setName, operation } = request;
  const enable = operation !== 'disable';
  const repair = operation === 'repair';
  const dialogRef = useRef<HTMLDialogElement>(null);
  const [preview, setPreview] = useState<SetPreview | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [applying, setApplying] = useState(false);
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    const dialog = dialogRef.current;
    dialog?.showModal();
    return () => { dialog?.close(); };
  }, []);

  useEffect(() => {
    let current = true;
    api.previewProjectSet(project.path, setId, operation).then(
      (result) => { if (current) setPreview(result); },
      (reason: unknown) => {
        if (current) setError(reason instanceof Error ? reason.message : String(reason));
      },
    );
    return () => { current = false; };
  }, [project.path, setId, operation, attempt]);

  const changeCount = preview?.items.filter((item) => item.status === 'change').length ?? 0;
  const canApply = !!preview && !applying && !error && (changeCount > 0 || preview.membershipChanges);

  async function apply() {
    if (!canApply || !preview) return;
    setApplying(true);
    try {
      await onApply(preview.revision);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setApplying(false);
    }
  }

  return (
    <dialog
      ref={dialogRef}
      aria-labelledby="set-preview-title"
      onCancel={(event) => {
        event.preventDefault();
        if (!applying) onClose();
      }}
      className="mac-sheet m-auto max-h-[calc(100vh-56px)] w-[660px] max-w-[calc(100vw-48px)] overflow-hidden p-0"
    >
      <div className="flex max-h-[calc(100vh-56px)] flex-col">
        <div className="border-b border-[var(--hairline)] px-5 py-4">
          <h2 id="set-preview-title" className="text-[15px] font-semibold">
            {repair ? 'Repair' : enable ? 'Apply' : 'Disable'} {preview?.name ?? setName}
          </h2>
          <p className="mt-1 text-[12px] text-[var(--text-secondary)]">{project.name}</p>
        </div>
        <div className="min-h-24 space-y-3 overflow-y-auto px-5 py-4 text-[13px]" aria-busy={!preview && !error}>
          {error && <p role="alert" className="text-[var(--status-red)]">{error}</p>}
          {!preview && !error && <p role="status">Checking project files…</p>}
          {preview && (
            <>
              <p>
                {repair && preview.items.length === 0 ? 'All Set items are available.' : changeCount ? `${changeCount} ${changeCount === 1 ? 'item will' : 'items will'} change.` : 'No context files would change.'}
                {preview.membershipChanges && ` The Set will be marked ${enable ? 'active' : 'inactive'} for this project.`}
              </p>
              {preview.agentImpact.length > 0 && (
                <div className="space-y-1 rounded-[8px] bg-[var(--bg-field)] px-3 py-2 text-[12px]">
                  <h3 className="font-medium">Estimated startup token change</h3>
                  <ul className="space-y-0.5 text-[var(--text-secondary)]">
                    {preview.agentImpact.map(({ agent, estimatedDelta, hasUncertainChanges }) => (
                      <li key={agent} className="flex flex-wrap justify-between gap-x-3">
                        <span>{agentDisplayName(agent)}</span>
                        <span className="tabular-nums">
                          {hasUncertainChanges && estimatedDelta === 0
                            ? 'Unavailable for these changes'
                            : `${estimatedDelta > 0 ? '+' : ''}${formatTokensExact(estimatedDelta)} tokens${hasUncertainChanges ? ' · partial estimate' : ''}`}
                        </span>
                      </li>
                    ))}
                  </ul>
                </div>
              )}
              {preview.items.length > 0 && <ul className="mac-group">
                {preview.items.map((item) => <PreviewItem key={`${item.kind}:${item.name}`} item={item} />)}
              </ul>}
              {!enable && preview.items.some((item) => item.kind === 'mcp' && item.status === 'change') && (
                <p className="text-[12px] text-[var(--text-secondary)]">
                  AgentDock may save a removed server definition in its local MCP catalog for reuse.
                </p>
              )}
              {preview.gitExcludePath && changeCount > 0 && (
                <p className="text-[12px] text-[var(--text-secondary)]">
                  AgentDock may also update its entries in <span className="break-all font-mono">{preview.gitExcludePath}</span>.
                </p>
              )}
            </>
          )}
        </div>
        <div className="flex justify-end gap-2.5 border-t border-[var(--hairline)] bg-[var(--bg-toolbar)] px-5 py-3">
          <button type="button" className="mac-btn h-8 px-3.5 text-[12px]" autoFocus disabled={applying} onClick={onClose}>
            Cancel
          </button>
          {error ? (
            <button type="button" className="mac-btn-primary h-8 px-3.5 text-[12px]"
              onClick={() => { setPreview(null); setError(null); setAttempt((value) => value + 1); }}>
              Refresh Preview
            </button>
          ) : (
            <button type="button" className="mac-btn-primary h-8 px-3.5 text-[12px] disabled:opacity-50"
              disabled={!canApply} onClick={() => void apply()}>
              {applying ? 'Applying…' : repair ? 'Apply Missing Items' : enable ? 'Apply Set' : 'Disable Set'}
            </button>
          )}
        </div>
      </div>
    </dialog>
  );
}

function PreviewItem({ item }: { item: SetItemPreview }) {
  return (
    <li className="space-y-1.5 px-3 py-2.5 [&+li]:border-t [&+li]:border-[var(--hairline)]">
      <div className="flex flex-wrap items-baseline justify-between gap-x-3">
        <span className="break-all font-medium">{item.name} <span className="text-[11px] text-[var(--text-secondary)]">{itemLabels[item.kind]}</span></span>
        <span className={item.status === 'blocked' ? 'text-[12px] text-[var(--status-amber)]' : 'text-[12px] text-[var(--text-secondary)]'}>
          {statusLabels[item.status]}
        </span>
      </div>
      {item.detail && <p className="break-words text-[12px] text-[var(--text-secondary)]">{item.detail}</p>}
      {item.files.length > 0 && (
        <ul className="space-y-1.5">
          {item.files.map((file) => (
            <li key={file.path} className="space-y-0.5 text-[12px] text-[var(--text-secondary)]">
              <div>{actionLabels[file.action]}{file.agents.length > 0 && ` · ${file.agents.map(agentDisplayName).join(', ')}`}</div>
              <div className="break-all font-mono text-[11px]">{file.path}</div>
              {file.target && <div className="break-all font-mono text-[11px]">{file.action === 'removeLink' ? 'Previous target ·' : '→'} {file.target}</div>}
            </li>
          ))}
        </ul>
      )}
      {item.status === 'change' && item.estimatedTokens !== null && (
        <p className="text-[11px] text-[var(--text-secondary)]">
          Estimated item cost if loaded · ~{formatTokensExact(item.estimatedTokens)} tokens
        </p>
      )}
    </li>
  );
}

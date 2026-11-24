import { useEffect, useRef, useState } from 'react';
import { X } from 'lucide-react';
import type { McpServerDraft, McpTransport, ToggleScope } from '@/types';
import { api } from '@/lib/api';

interface AddMcpModalProps {
  open: boolean;
  scope: ToggleScope;
  scopeLabel: string;
  onClose: () => void;
  onSuccess: () => Promise<void>;
}

export function AddMcpModal({
  open,
  scope,
  scopeLabel,
  onClose,
  onSuccess,
}: AddMcpModalProps) {
  const dialogRef = useRef<HTMLDialogElement | null>(null);
  const pressedOutsideRef = useRef(false);

  const [name, setName] = useState('');
  const [transport, setTransport] = useState<Exclude<McpTransport, 'unknown'>>('stdio');
  const [command, setCommand] = useState('npx');
  const [argsText, setArgsText] = useState('');
  const [url, setUrl] = useState('');
  const [envText, setEnvText] = useState('');
  const [enableImmediately, setEnableImmediately] = useState(true);
  const [saving, setSaving] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) {
      return;
    }
    if (open) {
      setName('');
      setTransport('stdio');
      setCommand('npx');
      setArgsText('');
      setUrl('');
      setEnvText('');
      setEnableImmediately(true);
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

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const cleanName = name.trim();
    if (!cleanName) {
      setErrorMsg('Enter an MCP server name.');
      return;
    }
    if (transport === 'stdio' && !command.trim()) {
      setErrorMsg('Stdio servers require a command (e.g. npx or uvx).');
      return;
    }
    if ((transport === 'http' || transport === 'sse') && !url.trim()) {
      setErrorMsg('HTTP/SSE servers require an endpoint URL.');
      return;
    }

    const args = argsText
      .split(/\s+/)
      .map((part) => part.trim())
      .filter(Boolean);

    const env: Record<string, string> = {};
    for (const rawLine of envText.split('\n')) {
      const line = rawLine.trim();
      if (!line || line.startsWith('#')) {
        continue;
      }
      const eq = line.indexOf('=');
      if (eq <= 0) {
        setErrorMsg(`Invalid env line "${line}" (expected KEY=VALUE).`);
        return;
      }
      const k = line.slice(0, eq).trim();
      const v = line.slice(eq + 1).trim();
      if (k) {
        env[k] = v;
      }
    }

    const draft: McpServerDraft = {
      name: cleanName,
      transport,
      command: transport === 'stdio' ? command.trim() : null,
      args: transport === 'stdio' ? args : [],
      url: transport === 'stdio' ? null : url.trim(),
      env: transport === 'stdio' ? env : {},
      headers: transport === 'stdio' ? {} : env,
    };

    setSaving(true);
    setErrorMsg(null);
    try {
      await api.addMcpServer(draft, enableImmediately ? scope : null);
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
      aria-labelledby="add-mcp-modal-title"
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
      className="m-auto w-[540px] max-w-[calc(100vw-48px)] overflow-hidden rounded-[12px] border border-[var(--hairline)] bg-[var(--bg-content)] p-0 text-[var(--text-primary)] shadow-[0_20px_50px_rgba(0,0,0,0.28)]"
    >
      <form onSubmit={handleSubmit} className="flex flex-col">
        <div className="flex items-center justify-between border-b border-[var(--hairline)] px-5 py-3.5">
          <div>
            <h2 id="add-mcp-modal-title" className="text-[15px] font-semibold">
              Add MCP Server
            </h2>
            <p className="text-[12px] text-[var(--text-secondary)]">
              Saved in AgentDock&apos;s central MCP catalog so you can toggle it per scope.
            </p>
          </div>
          <button
            type="button"
            onClick={() => dialogRef.current?.close()}
            aria-label="Close Add MCP dialog"
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

          <div className="grid grid-cols-1 gap-3.5 sm:grid-cols-3">
            <div className="space-y-1.5 sm:col-span-2">
              <label
                htmlFor="mcp-name-input"
                className="block text-[12.5px] font-medium"
              >
                Server Name
              </label>
              <input
                id="mcp-name-input"
                type="text"
                autoFocus
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="context7"
                className="mono-path h-8 w-full rounded-[6px] border border-[var(--hairline)] bg-[var(--bg-elevated)] px-2.5 text-[12.5px]"
              />
            </div>

            <div className="space-y-1.5">
              <label
                htmlFor="mcp-transport-select"
                className="block text-[12.5px] font-medium"
              >
                Transport
              </label>
              <select
                id="mcp-transport-select"
                value={transport}
                onChange={(e) =>
                  setTransport(e.target.value as Exclude<McpTransport, 'unknown'>)
                }
                className="h-8 w-full rounded-[6px] border border-[var(--hairline)] bg-[var(--bg-elevated)] px-2.5 text-[12.5px]"
              >
                <option value="stdio">Stdio</option>
                <option value="http">HTTP</option>
                <option value="sse">SSE</option>
              </select>
            </div>
          </div>

          {transport === 'stdio' ? (
            <div className="grid grid-cols-1 gap-3.5 sm:grid-cols-3">
              <div className="space-y-1.5">
                <label
                  htmlFor="mcp-command-input"
                  className="block text-[12.5px] font-medium"
                >
                  Command
                </label>
                <input
                  id="mcp-command-input"
                  type="text"
                  value={command}
                  onChange={(e) => setCommand(e.target.value)}
                  placeholder="npx"
                  className="mono-path h-8 w-full rounded-[6px] border border-[var(--hairline)] bg-[var(--bg-elevated)] px-2.5 text-[12px]"
                />
              </div>

              <div className="space-y-1.5 sm:col-span-2">
                <label
                  htmlFor="mcp-args-input"
                  className="block text-[12.5px] font-medium"
                >
                  Arguments (space-separated)
                </label>
                <input
                  id="mcp-args-input"
                  type="text"
                  value={argsText}
                  onChange={(e) => setArgsText(e.target.value)}
                  placeholder="-y @upstash/context7-mcp"
                  className="mono-path h-8 w-full rounded-[6px] border border-[var(--hairline)] bg-[var(--bg-elevated)] px-2.5 text-[12px]"
                />
              </div>
            </div>
          ) : (
            <div className="space-y-1.5">
              <label
                htmlFor="mcp-url-input"
                className="block text-[12.5px] font-medium"
              >
                Endpoint URL
              </label>
              <input
                id="mcp-url-input"
                type="url"
                value={url}
                onChange={(e) => setUrl(e.target.value)}
                placeholder="https://mcp.example.com/mcp"
                className="mono-path h-8 w-full rounded-[6px] border border-[var(--hairline)] bg-[var(--bg-elevated)] px-2.5 text-[12px]"
              />
            </div>
          )}

          <div className="space-y-1.5">
            <label
              htmlFor="mcp-env-input"
              className="block text-[12.5px] font-medium"
            >
              {transport === 'stdio'
                ? 'Environment Variables (optional, KEY=VALUE per line)'
                : 'HTTP Headers (optional, KEY=VALUE per line)'}
            </label>
            <textarea
              id="mcp-env-input"
              rows={3}
              value={envText}
              onChange={(e) => setEnvText(e.target.value)}
              placeholder={
                transport === 'stdio'
                  ? 'API_KEY=${MY_API_KEY}'
                  : 'Authorization=Bearer ${TOKEN}'
              }
              className="mono-path w-full rounded-[6px] border border-[var(--hairline)] bg-[var(--bg-elevated)] p-2.5 text-[12px]"
            />
          </div>

          <label className="flex cursor-pointer items-center gap-2 text-[12.5px]">
            <input
              type="checkbox"
              checked={enableImmediately}
              onChange={(e) => setEnableImmediately(e.target.checked)}
            />
            <span>Enable immediately in {scopeLabel}</span>
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
            {saving ? 'Saving…' : 'Add MCP Server'}
          </button>
        </div>
      </form>
    </dialog>
  );
}

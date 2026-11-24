import { useEffect, useRef, useState } from 'react';
import { FolderOpen, Plus, Trash2, X } from 'lucide-react';
import type { AppConfig, LibrarySource } from '@/types';
import { api } from '@/lib/api';

interface SettingsModalProps {
  open: boolean;
  config: AppConfig;
  onClose: () => void;
  onSave: (nextConfig: AppConfig) => Promise<void>;
}

export function SettingsModal({
  open,
  config,
  onClose,
  onSave,
}: SettingsModalProps) {
  const dialogRef = useRef<HTMLDialogElement | null>(null);
  const pressedOutsideRef = useRef(false);

  const [projectsRoot, setProjectsRoot] = useState(config.projectsRoot ?? '');
  const [supportDocsRoot, setSupportDocsRoot] = useState(
    config.supportDocsRoot ?? '',
  );
  const [ignoreText, setIgnoreText] = useState(config.ignoreFolders.join(', '));
  const [pinnedProjects, setPinnedProjects] = useState<string[]>(
    config.pinnedProjects,
  );
  const [librarySources, setLibrarySources] = useState<LibrarySource[]>(
    config.librarySources,
  );
  const [antigravity, setAntigravity] = useState(config.activeAgents.antigravity);
  const [codex, setCodex] = useState(config.activeAgents.codex);
  const [claude, setClaude] = useState(config.activeAgents.claude);
  const [gitHygiene, setGitHygiene] = useState(config.gitHygiene);
  const [saving, setSaving] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) {
      return;
    }
    if (open) {
      setProjectsRoot(config.projectsRoot ?? '');
      setSupportDocsRoot(config.supportDocsRoot ?? '');
      setIgnoreText(config.ignoreFolders.join(', '));
      setPinnedProjects(config.pinnedProjects);
      setLibrarySources(config.librarySources);
      setAntigravity(config.activeAgents.antigravity);
      setCodex(config.activeAgents.codex);
      setClaude(config.activeAgents.claude);
      setGitHygiene(config.gitHygiene);
      setErrorMsg(null);
      if (!dialog.open) {
        dialog.showModal();
      }
    } else if (dialog.open) {
      dialog.close();
    }
  }, [open, config]);

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
    const dialog = dialogRef.current;
    if (dialog) {
      dialog.returnValue = '';
    }
    onClose();
  };

  const handleBrowseProjectsRoot = async () => {
    const picked = await api.pickDirectory(projectsRoot || null);
    if (picked) {
      setProjectsRoot(picked);
    }
  };

  const handleBrowseSupportDocs = async () => {
    const picked = await api.pickDirectory(supportDocsRoot || null);
    if (picked) {
      setSupportDocsRoot(picked);
    }
  };

  const handleAddPinnedProject = async () => {
    const picked = await api.pickDirectory(projectsRoot || null);
    if (picked && !pinnedProjects.includes(picked)) {
      setPinnedProjects((prev) => [...prev, picked]);
    }
  };

  const handleAddLibrarySource = async () => {
    const picked = await api.pickDirectory(null);
    if (!picked) {
      return;
    }
    const folderName = picked.split('/').filter(Boolean).pop() ?? 'Library';
    const label = picked.includes('BRAIN') ? 'Obsidian' : folderName;
    setLibrarySources((prev) => [
      ...prev,
      {
        id: `lib-${Date.now()}`,
        label,
        path: picked,
      },
    ]);
  };

  const handleBrowseLibraryPath = async (idx: number) => {
    const current = librarySources[idx]?.path ?? null;
    const picked = await api.pickDirectory(current);
    if (!picked) {
      return;
    }
    setLibrarySources((prev) =>
      prev.map((src, i) => (i === idx ? { ...src, path: picked } : src)),
    );
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setSaving(true);
    setErrorMsg(null);

    try {
      const ignoreFolders = ignoreText
        .split(',')
        .map((s) => s.trim())
        .filter(Boolean);

      const cleanedSources = librarySources
        .map((src) => ({
          ...src,
          label: src.label.trim() || 'Library',
          path: src.path.trim(),
        }))
        .filter((src) => src.path.length > 0);

      const nextConfig: AppConfig = {
        projectsRoot: projectsRoot.trim() || null,
        librarySources: cleanedSources,
        supportDocsRoot: supportDocsRoot.trim() || null,
        pinnedProjects,
        ignoreFolders,
        activeAgents: {
          antigravity,
          codex,
          claude,
        },
        gitHygiene,
      };

      await onSave(nextConfig);
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
      aria-labelledby="settings-modal-title"
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
      className="m-auto w-[620px] max-w-[calc(100vw-48px)] max-h-[calc(100vh-56px)] overflow-hidden rounded-[12px] border border-[var(--hairline)] bg-[var(--bg-content)] p-0 text-[var(--text-primary)] shadow-[0_20px_50px_rgba(0,0,0,0.28)]"
    >
      <form onSubmit={handleSubmit} className="flex max-h-[calc(100vh-56px)] flex-col">
        <div className="flex shrink-0 items-center justify-between border-b border-[var(--hairline)] px-5 py-3.5">
          <h2 id="settings-modal-title" className="text-[15px] font-semibold">
            Settings
          </h2>
          <button
            type="button"
            onClick={() => dialogRef.current?.close()}
            aria-label="Close settings"
            className="inline-flex h-7 w-7 items-center justify-center rounded-[6px] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
          >
            <X className="h-4 w-4" strokeWidth={1.75} />
          </button>
        </div>

        <div className="flex-1 space-y-5 overflow-y-auto px-5 py-4">
          {errorMsg && (
            <div className="rounded-[8px] border border-[var(--status-red)]/30 bg-[var(--status-red)]/10 px-3 py-2 text-[12.5px] text-[var(--status-red)]">
              {errorMsg}
            </div>
          )}

          {/* Projects Root */}
          <div className="space-y-1.5">
            <label
              htmlFor="projects-root-input"
              className="block text-[12.5px] font-medium text-[var(--text-primary)]"
            >
              Projects Root Folder (2-level Category / Project)
            </label>
            <div className="flex gap-2">
              <input
                id="projects-root-input"
                type="text"
                autoFocus
                value={projectsRoot}
                onChange={(e) => setProjectsRoot(e.target.value)}
                placeholder="/Users/peter/APP-PROJECTS"
                className="mono-path h-8 flex-1 rounded-[6px] border border-[var(--hairline)] bg-[var(--bg-elevated)] px-2.5 text-[12px] text-[var(--text-primary)]"
              />
              <button
                type="button"
                onClick={handleBrowseProjectsRoot}
                className="inline-flex h-8 items-center gap-1.5 rounded-[6px] border border-[var(--hairline)] bg-[var(--bg-elevated)] px-3 text-[12.5px] font-medium hover:bg-[var(--bg-hover)]"
              >
                <FolderOpen className="h-3.5 w-3.5" strokeWidth={1.75} />
                Browse…
              </button>
            </div>
          </div>

          {/* Ignore Folders */}
          <div className="space-y-1.5">
            <label
              htmlFor="ignore-folders-input"
              className="block text-[12.5px] font-medium text-[var(--text-primary)]"
            >
              Ignored Folder Names (comma-separated)
            </label>
            <input
              id="ignore-folders-input"
              type="text"
              value={ignoreText}
              onChange={(e) => setIgnoreText(e.target.value)}
              className="mono-path h-8 w-full rounded-[6px] border border-[var(--hairline)] bg-[var(--bg-elevated)] px-2.5 text-[12px] text-[var(--text-primary)]"
            />
          </div>

          {/* Library Sources */}
          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <span className="text-[12.5px] font-medium text-[var(--text-primary)]">
                Library Sources (Obsidian Vault / Canonical Skills)
              </span>
              <button
                type="button"
                onClick={handleAddLibrarySource}
                className="inline-flex h-7 items-center gap-1 rounded-[6px] border border-[var(--hairline)] bg-[var(--bg-elevated)] px-2.5 text-[12px] font-medium hover:bg-[var(--bg-hover)]"
              >
                <Plus className="h-3.5 w-3.5" strokeWidth={1.75} />
                Add Source
              </button>
            </div>

            {librarySources.length === 0 ? (
              <div className="rounded-[8px] border border-[var(--hairline)] bg-[var(--bg-elevated)] px-3 py-2.5 text-[12px] text-[var(--text-tertiary)]">
                No library sources configured. Add your Obsidian skills folder to attribute symlinks.
              </div>
            ) : (
              <div className="space-y-2">
                {librarySources.map((src, idx) => (
                  <div key={src.id} className="flex items-center gap-2">
                    <input
                      type="text"
                      value={src.label}
                      aria-label="Library label"
                      onChange={(e) =>
                        setLibrarySources((prev) =>
                          prev.map((item, i) =>
                            i === idx ? { ...item, label: e.target.value } : item,
                          ),
                        )
                      }
                      placeholder="Label"
                      className="h-8 w-28 shrink-0 rounded-[6px] border border-[var(--hairline)] bg-[var(--bg-elevated)] px-2.5 text-[12.5px]"
                    />
                    <input
                      type="text"
                      value={src.path}
                      aria-label="Library path"
                      onChange={(e) =>
                        setLibrarySources((prev) =>
                          prev.map((item, i) =>
                            i === idx ? { ...item, path: e.target.value } : item,
                          ),
                        )
                      }
                      className="mono-path h-8 flex-1 rounded-[6px] border border-[var(--hairline)] bg-[var(--bg-elevated)] px-2.5 text-[12px]"
                    />
                    <button
                      type="button"
                      onClick={() => handleBrowseLibraryPath(idx)}
                      aria-label="Browse library directory"
                      className="inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-[6px] border border-[var(--hairline)] bg-[var(--bg-elevated)] hover:bg-[var(--bg-hover)]"
                    >
                      <FolderOpen className="h-3.5 w-3.5" strokeWidth={1.75} />
                    </button>
                    <button
                      type="button"
                      onClick={() =>
                        setLibrarySources((prev) => prev.filter((_, i) => i !== idx))
                      }
                      aria-label={`Remove ${src.label}`}
                      className="inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-[6px] text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] hover:text-[var(--status-red)]"
                    >
                      <Trash2 className="h-3.5 w-3.5" strokeWidth={1.75} />
                    </button>
                  </div>
                ))}
              </div>
            )}
          </div>

          {/* Support Docs Root */}
          <div className="space-y-1.5">
            <label
              htmlFor="support-docs-input"
              className="block text-[12.5px] font-medium text-[var(--text-primary)]"
            >
              Support Docs Root Folder (Optional)
            </label>
            <div className="flex gap-2">
              <input
                id="support-docs-input"
                type="text"
                value={supportDocsRoot}
                onChange={(e) => setSupportDocsRoot(e.target.value)}
                placeholder="/Users/peter/BRAIN-LOCAL"
                className="mono-path h-8 flex-1 rounded-[6px] border border-[var(--hairline)] bg-[var(--bg-elevated)] px-2.5 text-[12px] text-[var(--text-primary)]"
              />
              <button
                type="button"
                onClick={handleBrowseSupportDocs}
                className="inline-flex h-8 items-center gap-1.5 rounded-[6px] border border-[var(--hairline)] bg-[var(--bg-elevated)] px-3 text-[12.5px] font-medium hover:bg-[var(--bg-hover)]"
              >
                <FolderOpen className="h-3.5 w-3.5" strokeWidth={1.75} />
                Browse…
              </button>
            </div>
          </div>

          {/* Pinned Projects */}
          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <span className="text-[12.5px] font-medium text-[var(--text-primary)]">
                Pinned Projects (Outside Root)
              </span>
              <button
                type="button"
                onClick={handleAddPinnedProject}
                className="inline-flex h-7 items-center gap-1 rounded-[6px] border border-[var(--hairline)] bg-[var(--bg-elevated)] px-2.5 text-[12px] font-medium hover:bg-[var(--bg-hover)]"
              >
                <Plus className="h-3.5 w-3.5" strokeWidth={1.75} />
                Pin Folder
              </button>
            </div>
            {pinnedProjects.length > 0 && (
              <div className="space-y-1.5">
                {pinnedProjects.map((projPath) => (
                  <div key={projPath} className="flex items-center gap-2">
                    <span className="mono-path flex-1 truncate rounded-[6px] border border-[var(--hairline)] bg-[var(--bg-elevated)] px-2.5 py-1.5 text-[12px]">
                      {projPath}
                    </span>
                    <button
                      type="button"
                      onClick={() =>
                        setPinnedProjects((prev) =>
                          prev.filter((p) => p !== projPath),
                        )
                      }
                      aria-label={`Unpin ${projPath}`}
                      className="inline-flex h-7 w-7 items-center justify-center rounded-[6px] text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] hover:text-[var(--status-red)]"
                    >
                      <Trash2 className="h-3.5 w-3.5" strokeWidth={1.75} />
                    </button>
                  </div>
                ))}
              </div>
            )}
          </div>

          {/* Active Agents */}
          <div className="space-y-2">
            <span className="block text-[12.5px] font-medium text-[var(--text-primary)]">
              Active Agents
            </span>
            <div className="flex flex-wrap gap-5 rounded-[8px] border border-[var(--hairline)] bg-[var(--bg-elevated)] px-3.5 py-2.5">
              <label className="flex cursor-pointer items-center gap-2 text-[12.5px]">
                <input
                  type="checkbox"
                  checked={antigravity}
                  onChange={(e) => setAntigravity(e.target.checked)}
                />
                <span>Antigravity</span>
              </label>
              <label className="flex cursor-pointer items-center gap-2 text-[12.5px]">
                <input
                  type="checkbox"
                  checked={codex}
                  onChange={(e) => setCodex(e.target.checked)}
                />
                <span>Codex</span>
              </label>
              <label className="flex cursor-pointer items-center gap-2 text-[12.5px]">
                <input
                  type="checkbox"
                  checked={claude}
                  onChange={(e) => setClaude(e.target.checked)}
                />
                <span>Claude Code</span>
              </label>
            </div>
          </div>

          {/* Git Hygiene */}
          <label className="flex cursor-pointer items-start gap-2.5 rounded-[8px] border border-[var(--hairline)] bg-[var(--bg-elevated)] px-3.5 py-2.5 text-[12.5px]">
            <input
              type="checkbox"
              className="mt-0.5"
              checked={gitHygiene}
              onChange={(e) => setGitHygiene(e.target.checked)}
            />
            <div>
              <div className="font-medium text-[var(--text-primary)]">
                Keep Git Clean via .git/info/exclude
              </div>
              <div className="text-[11.5px] text-[var(--text-secondary)]">
                Hide managed project symlinks & local agent configs locally without modifying .gitignore.
              </div>
            </div>
          </label>
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
            disabled={saving}
            className="h-8 rounded-[6px] bg-[var(--accent)] px-4 text-[12.5px] font-medium text-[var(--accent-text)] hover:opacity-90 disabled:opacity-60"
          >
            {saving ? 'Saving…' : 'Save & Rescan'}
          </button>
        </div>
      </form>
    </dialog>
  );
}

import { useEffect, useMemo, useRef, useState } from 'react';
import { X } from 'lucide-react';
import type { ContextSet, McpServerItem, RuleItem, SkillItem } from '@/types';

interface SaveSetModalProps {
  open: boolean;
  projectName: string;
  skills: SkillItem[];
  mcpServers: McpServerItem[];
  rules: RuleItem[];
  onClose: () => void;
  onSaveSet: (set: ContextSet) => Promise<void>;
}

export function SaveSetModal({
  open,
  projectName,
  skills,
  mcpServers,
  rules,
  onClose,
  onSaveSet,
}: SaveSetModalProps) {
  const dialogRef = useRef<HTMLDialogElement | null>(null);
  const pressedOutsideRef = useRef(false);

  const candidateSkills = useMemo(
    () => skills.filter((s) => s.source.kind === 'library'),
    [skills],
  );
  const candidateRules = useMemo(
    () => rules.filter((r) => r.isLibraryRule || r.isSymlink),
    [rules],
  );

  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [selectedSkills, setSelectedSkills] = useState<string[]>([]);
  const [selectedMcps, setSelectedMcps] = useState<string[]>([]);
  const [selectedRules, setSelectedRules] = useState<string[]>([]);
  const [saving, setSaving] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) {
      return;
    }
    if (open) {
      setName(`${projectName} Stack`);
      setDescription('');
      setSelectedSkills(
        candidateSkills
          .filter((s) => s.activeInProject && !s.activeGlobally)
          .map((s) => s.id),
      );
      setSelectedMcps(
        mcpServers
          .filter((m) => m.activeInProject && !m.activeGlobally)
          .map((m) => m.name),
      );
      setSelectedRules(
        candidateRules
          .filter((r) => r.activeInProject && !r.activeGlobally)
          .map((r) => r.filename),
      );
      setErrorMsg(null);
      if (!dialog.open) {
        dialog.showModal();
      }
    } else if (dialog.open) {
      dialog.close();
    }
  }, [open, projectName, candidateSkills, mcpServers, candidateRules]);

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

  const toggleItem = (
    list: string[],
    setter: (next: string[]) => void,
    value: string,
  ) => {
    setter(
      list.includes(value)
        ? list.filter((item) => item !== value)
        : [...list, value],
    );
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const trimmed = name.trim();
    if (!trimmed) {
      setErrorMsg('Please enter a set name.');
      return;
    }
    setSaving(true);
    setErrorMsg(null);
    try {
      await onSaveSet({
        id: `set-${Date.now()}`,
        name: trimmed,
        description: description.trim(),
        skillIds: selectedSkills,
        mcpNames: selectedMcps,
        ruleFilenames: selectedRules,
      });
      dialogRef.current?.close();
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  return (
    <dialog
      ref={dialogRef}
      aria-labelledby="save-set-modal-title"
      onClose={onClose}
      onPointerDown={(e) => {
        pressedOutsideRef.current = isOutsideDialog(e);
      }}
      onPointerUp={(e) => {
        if (pressedOutsideRef.current && isOutsideDialog(e)) {
          dialogRef.current?.close();
        }
        pressedOutsideRef.current = false;
      }}
      className="mac-sheet m-auto w-[520px] max-w-[calc(100vw-48px)] max-h-[calc(100vh-56px)] overflow-hidden p-0"
    >
      <form
        onSubmit={handleSubmit}
        className="flex max-h-[calc(100vh-56px)] flex-col"
      >
        <div className="flex shrink-0 items-center justify-between border-b border-[var(--hairline)] bg-[var(--bg-toolbar)] px-5 py-3.5 backdrop-blur-xl">
          <h2 id="save-set-modal-title" className="text-[14px] font-semibold">
            Save Current Setup as Set
          </h2>
          <button
            type="button"
            onClick={() => dialogRef.current?.close()}
            aria-label="Close modal"
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

          <div className="mac-group space-y-3 p-3.5">
            <div className="space-y-1.5">
              <label
                htmlFor="set-name-input"
                className="block text-[12px] font-medium text-[var(--text-secondary)]"
              >
                Set Name
              </label>
              <input
                id="set-name-input"
                type="text"
                autoFocus
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="e.g. Web Client Stack"
                className="mac-input h-8 w-full px-2.5 text-[12.5px]"
              />
            </div>

            <div className="space-y-1.5">
              <label
                htmlFor="set-desc-input"
                className="block text-[12px] font-medium text-[var(--text-secondary)]"
              >
                Description (Optional)
              </label>
              <input
                id="set-desc-input"
                type="text"
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                placeholder="Short summary of when to turn this set on"
                className="mac-input h-8 w-full px-2.5 text-[12.5px]"
              />
            </div>
          </div>

          {/* Skills checklist */}
          <div className="space-y-1.5">
            <div className="px-1 text-[12px] font-semibold text-[var(--text-secondary)]">
              Skills ({selectedSkills.length} selected)
            </div>
            <div className="mac-group max-h-36 space-y-0.5 overflow-y-auto p-2">
              {candidateSkills.length === 0 ? (
                <div className="px-1.5 py-1 text-[12px] text-[var(--text-tertiary)]">
                  No library skills available.
                </div>
              ) : (
                candidateSkills.map((skill) => (
                  <label
                    key={skill.id}
                    className="flex cursor-pointer items-center justify-between gap-2 rounded-[5px] px-2 py-1 text-[12px] hover:bg-[var(--bg-hover)]"
                  >
                    <span className="flex min-w-0 items-center gap-2">
                      <input
                        type="checkbox"
                        checked={selectedSkills.includes(skill.id)}
                        onChange={() =>
                          toggleItem(selectedSkills, setSelectedSkills, skill.id)
                        }
                      />
                      <span className="truncate">{skill.name}</span>
                    </span>
                    <span className="tabular-nums shrink-0 text-[11px] text-[var(--text-tertiary)]">
                      {skill.upfrontTokens} tok
                    </span>
                  </label>
                ))
              )}
            </div>
          </div>

          {/* MCP checklist */}
          <div className="space-y-1.5">
            <div className="px-1 text-[12px] font-semibold text-[var(--text-secondary)]">
              MCP Servers ({selectedMcps.length} selected)
            </div>
            <div className="mac-group max-h-32 space-y-0.5 overflow-y-auto p-2">
              {mcpServers.length === 0 ? (
                <div className="px-1.5 py-1 text-[12px] text-[var(--text-tertiary)]">
                  No MCP servers in catalog.
                </div>
              ) : (
                mcpServers.map((srv) => (
                  <label
                    key={srv.name}
                    className="flex cursor-pointer items-center justify-between gap-2 rounded-[5px] px-2 py-1 text-[12px] hover:bg-[var(--bg-hover)]"
                  >
                    <span className="flex min-w-0 items-center gap-2">
                      <input
                        type="checkbox"
                        checked={selectedMcps.includes(srv.name)}
                        onChange={() =>
                          toggleItem(selectedMcps, setSelectedMcps, srv.name)
                        }
                      />
                      <span className="truncate">{srv.name}</span>
                    </span>
                    <span className="tabular-nums shrink-0 text-[11px] text-[var(--text-tertiary)]">
                      {srv.upfrontTokens} tok
                    </span>
                  </label>
                ))
              )}
            </div>
          </div>

          {/* Rules checklist */}
          {candidateRules.length > 0 && (
            <div className="space-y-1.5">
              <div className="px-1 text-[12px] font-semibold text-[var(--text-secondary)]">
                Library Rules ({selectedRules.length} selected)
              </div>
              <div className="mac-group max-h-28 space-y-0.5 overflow-y-auto p-2">
                {candidateRules.map((rule) => (
                  <label
                    key={rule.id}
                    className="flex cursor-pointer items-center justify-between gap-2 rounded-[5px] px-2 py-1 text-[12px] hover:bg-[var(--bg-hover)]"
                  >
                    <span className="flex min-w-0 items-center gap-2">
                      <input
                        type="checkbox"
                        checked={selectedRules.includes(rule.filename)}
                        onChange={() =>
                          toggleItem(
                            selectedRules,
                            setSelectedRules,
                            rule.filename,
                          )
                        }
                      />
                      <span className="truncate">{rule.filename}</span>
                    </span>
                    <span className="tabular-nums shrink-0 text-[11px] text-[var(--text-tertiary)]">
                      {rule.upfrontTokens} tok
                    </span>
                  </label>
                ))}
              </div>
            </div>
          )}
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
            disabled={saving}
            className="mac-btn-primary h-8 px-4 text-[12.5px] font-medium disabled:opacity-60"
          >
            {saving ? 'Saving…' : 'Create Set'}
          </button>
        </div>
      </form>
    </dialog>
  );
}

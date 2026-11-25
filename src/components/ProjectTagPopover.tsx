import { useEffect, useRef, useState } from 'react';
import { Check, Plus, Settings2, X } from 'lucide-react';
import type {
  ProjectSummary,
  TagColor,
  TagDefinition,
} from '@/types';
import { TAG_COLORS, tagColorHex } from '@/lib/format';

interface ProjectTagPopoverProps {
  open: boolean;
  project: ProjectSummary;
  tagDefinitions: TagDefinition[];
  manualTagIds: string[];
  mutedAutoTagIds: string[];
  onClose: () => void;
  onSaveProjectTags: (
    manualTagIds: string[],
    mutedAutoTagIds: string[],
  ) => Promise<void>;
  onCreateAndAttachTag: (tag: TagDefinition) => Promise<void>;
  onOpenTagSettings: () => void;
}

export function ProjectTagPopover({
  open,
  project,
  tagDefinitions,
  manualTagIds,
  mutedAutoTagIds,
  onClose,
  onSaveProjectTags,
  onCreateAndAttachTag,
  onOpenTagSettings,
}: ProjectTagPopoverProps) {
  const popoverRef = useRef<HTMLDivElement | null>(null);
  const [newLabel, setNewLabel] = useState('');
  const [newColor, setNewColor] = useState<TagColor>('blue');
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!open) {
      return;
    }
    const onPointerDown = (e: MouseEvent) => {
      if (
        popoverRef.current &&
        !popoverRef.current.contains(e.target as Node)
      ) {
        onClose();
      }
    };
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        onClose();
      }
    };
    document.addEventListener('mousedown', onPointerDown);
    document.addEventListener('keydown', onKeyDown);
    return () => {
      document.removeEventListener('mousedown', onPointerDown);
      document.removeEventListener('keydown', onKeyDown);
    };
  }, [open, onClose]);

  if (!open) {
    return null;
  }

  // Compute auto-tags derived from project's ancestor folders
  const autoItems = project.ancestorFolders.map((folder) => {
    const rule = tagDefinitions.find(
      (d) => d.autoFolder?.toLowerCase() === folder.toLowerCase(),
    );
    const id = rule ? rule.id : `auto:${folder.toLowerCase()}`;
    const label = rule ? rule.label : folder;
    const color: TagColor = rule ? rule.color : 'gray';
    const hiddenGlobally = rule?.hidden ?? false;
    const muted = mutedAutoTagIds.includes(id);
    return { folder, id, label, color, hiddenGlobally, muted };
  });

  const manualCatalog = tagDefinitions.filter((d) => !d.autoFolder);

  const handleToggleManual = async (tagId: string) => {
    setBusy(true);
    try {
      const next = manualTagIds.includes(tagId)
        ? manualTagIds.filter((id) => id !== tagId)
        : [...manualTagIds, tagId];
      await onSaveProjectTags(next, mutedAutoTagIds);
    } finally {
      setBusy(false);
    }
  };

  const handleToggleAutoMute = async (autoId: string) => {
    setBusy(true);
    try {
      const nextMuted = mutedAutoTagIds.includes(autoId)
        ? mutedAutoTagIds.filter((id) => id !== autoId)
        : [...mutedAutoTagIds, autoId];
      await onSaveProjectTags(manualTagIds, nextMuted);
    } finally {
      setBusy(false);
    }
  };

  const handleCreateTag = async (e: React.FormEvent) => {
    e.preventDefault();
    const label = newLabel.trim();
    if (!label) {
      return;
    }
    setBusy(true);
    try {
      await onCreateAndAttachTag({
        id: `tag-${Date.now()}`,
        label,
        color: newColor,
        autoFolder: null,
        hidden: false,
      });
      setNewLabel('');
    } finally {
      setBusy(false);
    }
  };

  return (
    <div
      ref={popoverRef}
      role="dialog"
      aria-label={`Manage tags for ${project.name}`}
      className="absolute left-0 top-full z-30 mt-1.5 w-72 rounded-[10px] bg-[var(--bg-popover)] p-3 text-[12px] shadow-[0_14px_36px_rgba(0,0,0,0.24),0_0_0_0.5px_var(--hairline)] backdrop-blur-2xl"
    >
      <div className="mb-2 flex items-center justify-between">
        <span className="font-semibold text-[var(--text-primary)]">
          Project Tags
        </span>
        <button
          type="button"
          onClick={onClose}
          aria-label="Close tags popover"
          className="inline-flex h-5 w-5 items-center justify-center rounded text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
        >
          <X className="h-3.5 w-3.5" />
        </button>
      </div>

      {/* Auto-Tags from Ancestor Folders */}
      {autoItems.length > 0 && (
        <div className="mb-2.5 space-y-1">
          <div className="text-[11px] font-medium text-[var(--text-tertiary)]">
            Auto-Tags (from folder path)
          </div>
          {autoItems.map((item) => (
            <button
              key={item.id}
              type="button"
              disabled={busy || item.hiddenGlobally}
              onClick={() => void handleToggleAutoMute(item.id)}
              className="flex w-full items-center justify-between gap-2 rounded-[6px] px-2 py-1 text-left hover:bg-[var(--bg-hover)] disabled:opacity-50"
            >
              <span className="flex min-w-0 items-center gap-2">
                <span
                  className="h-2.5 w-2.5 shrink-0 rounded-full"
                  style={{ backgroundColor: tagColorHex(item.color) }}
                />
                <span className="truncate text-[var(--text-primary)]">
                  {item.label}
                </span>
                {item.label.toLowerCase() !== item.folder.toLowerCase() && (
                  <span className="truncate text-[10.5px] text-[var(--text-tertiary)]">
                    ({item.folder})
                  </span>
                )}
              </span>
              <span className="shrink-0 text-[11px] text-[var(--text-tertiary)]">
                {item.hiddenGlobally ? (
                  'Hidden'
                ) : !item.muted ? (
                  <Check className="h-3.5 w-3.5 text-[var(--accent)]" />
                ) : (
                  'Muted'
                )}
              </span>
            </button>
          ))}
        </div>
      )}

      {/* Manual Tags */}
      <div className="mb-2.5 space-y-1">
        <div className="text-[11px] font-medium text-[var(--text-tertiary)]">
          Custom Tags
        </div>
        {manualCatalog.length === 0 ? (
          <div className="px-2 py-1 text-[11.5px] text-[var(--text-tertiary)]">
            No custom tags yet.
          </div>
        ) : (
          <div className="max-h-36 space-y-0.5 overflow-y-auto">
            {manualCatalog.map((tag) => {
              const checked = manualTagIds.includes(tag.id);
              return (
                <button
                  key={tag.id}
                  type="button"
                  disabled={busy}
                  onClick={() => void handleToggleManual(tag.id)}
                  className="flex w-full items-center justify-between gap-2 rounded-[6px] px-2 py-1 text-left hover:bg-[var(--bg-hover)]"
                >
                  <span className="flex min-w-0 items-center gap-2">
                    <span
                      className="h-2.5 w-2.5 shrink-0 rounded-full"
                      style={{ backgroundColor: tagColorHex(tag.color) }}
                    />
                    <span className="truncate text-[var(--text-primary)]">
                      {tag.label}
                    </span>
                  </span>
                  {checked && (
                    <Check className="h-3.5 w-3.5 shrink-0 text-[var(--accent)]" />
                  )}
                </button>
              );
            })}
          </div>
        )}
      </div>

      {/* Inline quick-create */}
      <form
        onSubmit={handleCreateTag}
        className="space-y-2 border-t border-[var(--hairline-inset)] pt-2"
      >
        <div className="flex items-center gap-1.5">
          <input
            type="text"
            value={newLabel}
            onChange={(e) => setNewLabel(e.target.value)}
            placeholder="New custom tag…"
            className="mac-input h-7 min-w-0 flex-1 px-2 text-[12px]"
          />
          <button
            type="submit"
            disabled={busy || !newLabel.trim()}
            aria-label="Add custom tag"
            className="mac-btn-primary inline-flex h-7 w-7 shrink-0 items-center justify-center disabled:opacity-40"
          >
            <Plus className="h-3.5 w-3.5" />
          </button>
        </div>

        <div className="flex items-center justify-between">
          <div className="flex items-center gap-1.5" role="group" aria-label="Tag color">
            {TAG_COLORS.map((c) => (
              <button
                key={c.id}
                type="button"
                aria-label={c.label}
                aria-pressed={newColor === c.id}
                onClick={() => setNewColor(c.id)}
                className={[
                  'h-3.5 w-3.5 rounded-full transition-transform',
                  newColor === c.id
                    ? 'scale-110 ring-2 ring-[var(--accent)] ring-offset-1 ring-offset-[var(--bg-content)]'
                    : 'opacity-75 hover:opacity-100',
                ].join(' ')}
                style={{ backgroundColor: c.hex }}
              />
            ))}
          </div>

          <button
            type="button"
            onClick={() => {
              onClose();
              onOpenTagSettings();
            }}
            className="inline-flex items-center gap-1 rounded-[5px] px-1.5 py-1 text-[11px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
          >
            <Settings2 className="h-3 w-3" />
            Edit All…
          </button>
        </div>
      </form>
    </div>
  );
}

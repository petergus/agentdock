import {
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
  type RefObject,
} from 'react';
import { Check, Search, Settings2, X } from 'lucide-react';
import type { ProjectTag } from '@/types';
import { tagColorHex } from '@/lib/format';

export type TagMatchMode = 'all' | 'any';

export interface SidebarTagOption {
  tag: ProjectTag;
  projectCount: number;
}

interface SidebarTagFilterPopoverProps {
  open: boolean;
  tags: SidebarTagOption[];
  selectedTagIds: string[];
  matchMode: TagMatchMode;
  triggerRef: RefObject<HTMLButtonElement | null>;
  returnFocusRef?: RefObject<HTMLElement | null>;
  onToggleTag: (tagId: string) => void;
  onMatchModeChange: (mode: TagMatchMode) => void;
  onClearAll: () => void;
  onOpenTagSettings: () => void;
  onClose: () => void;
}

export function SidebarTagFilterPopover({
  open,
  tags,
  selectedTagIds,
  matchMode,
  triggerRef,
  returnFocusRef,
  onToggleTag,
  onMatchModeChange,
  onClearAll,
  onOpenTagSettings,
  onClose,
}: SidebarTagFilterPopoverProps) {
  const listboxId = useId();
  const popoverRef = useRef<HTMLDivElement | null>(null);
  const searchRef = useRef<HTMLInputElement | null>(null);
  const optionRefs = useRef<Record<string, HTMLButtonElement | null>>({});
  const [query, setQuery] = useState('');
  const [activeIndex, setActiveIndex] = useState(0);

  const selectedSet = useMemo(() => new Set(selectedTagIds), [selectedTagIds]);

  const { customTags, folderTags, orderedVisible } = useMemo(() => {
    const q = query.trim().toLowerCase();
    const filtered = q
      ? tags.filter(
          ({ tag }) =>
            tag.label.toLowerCase().includes(q) ||
            (tag.sourceFolder?.toLowerCase().includes(q) ?? false),
        )
      : tags;

    const custom: SidebarTagOption[] = [];
    const folder: SidebarTagOption[] = [];
    for (const item of filtered) {
      if (item.tag.isAuto) {
        folder.push(item);
      } else {
        custom.push(item);
      }
    }

    return {
      customTags: custom,
      folderTags: folder,
      orderedVisible: [...custom, ...folder],
    };
  }, [tags, query]);

  const indexById = useMemo(() => {
    const map = new Map<string, number>();
    orderedVisible.forEach((item, index) => {
      map.set(item.tag.id, index);
    });
    return map;
  }, [orderedVisible]);

  // Reset search state and focus the search field whenever the popover opens
  useEffect(() => {
    if (!open) {
      return;
    }
    setQuery('');
    setActiveIndex(0);
    const frame = window.requestAnimationFrame(() => {
      searchRef.current?.focus();
    });
    return () => window.cancelAnimationFrame(frame);
  }, [open]);

  // Clamp activeIndex if filtering narrows the visible options
  useEffect(() => {
    if (orderedVisible.length === 0) {
      if (activeIndex !== 0) {
        setActiveIndex(0);
      }
      return;
    }
    if (activeIndex >= orderedVisible.length) {
      setActiveIndex(orderedVisible.length - 1);
    }
  }, [orderedVisible.length, activeIndex]);

  // Keep keyboard-highlighted option scrolled into view
  useEffect(() => {
    if (!open) {
      return;
    }
    const activeItem = orderedVisible[activeIndex];
    if (!activeItem) {
      return;
    }
    optionRefs.current[activeItem.tag.id]?.scrollIntoView({
      block: 'nearest',
    });
  }, [open, activeIndex, orderedVisible]);

  // Dismiss on outside click or Escape
  useEffect(() => {
    if (!open) {
      return;
    }
    const onPointerDown = (e: MouseEvent) => {
      const target = e.target as Node;
      if (
        popoverRef.current?.contains(target) ||
        triggerRef.current?.contains(target)
      ) {
        return;
      }
      onClose();
    };
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        e.stopPropagation();
        onClose();
        (returnFocusRef?.current ?? triggerRef.current)?.focus();
      }
    };
    document.addEventListener('mousedown', onPointerDown);
    document.addEventListener('keydown', onKeyDown, true);
    return () => {
      document.removeEventListener('mousedown', onPointerDown);
      document.removeEventListener('keydown', onKeyDown, true);
    };
  }, [open, onClose, triggerRef, returnFocusRef]);

  if (!open) {
    return null;
  }

  const activeItem = orderedVisible[activeIndex];
  const activeOptionId = activeItem
    ? `${listboxId}-opt-${activeItem.tag.id}`
    : undefined;

  const handleContainerKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      // Prevent App.tsx global arrow listener from changing selected project
      e.stopPropagation();
    }
  };

  const handleSearchKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      e.stopPropagation();
      if (orderedVisible.length > 0) {
        setActiveIndex((prev) => (prev + 1) % orderedVisible.length);
      }
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      e.stopPropagation();
      if (orderedVisible.length > 0) {
        setActiveIndex(
          (prev) => (prev - 1 + orderedVisible.length) % orderedVisible.length,
        );
      }
    } else if (e.key === 'Enter') {
      if (activeItem) {
        e.preventDefault();
        onToggleTag(activeItem.tag.id);
      }
    }
  };

  const renderSection = (
    title: string,
    items: SidebarTagOption[],
    isFirstSection: boolean,
  ) => {
    if (items.length === 0) {
      return null;
    }
    return (
      <div>
        <div
          className={[
            'px-2 pb-1 text-[10.5px] font-semibold uppercase tracking-[0.03em] text-[var(--text-tertiary)]',
            isFirstSection ? 'pt-0.5' : 'pt-1.5',
          ].join(' ')}
        >
          {title}
        </div>
        <div className="space-y-0.5">
          {items.map((item) => {
            const idx = indexById.get(item.tag.id) ?? -1;
            const isSelected = selectedSet.has(item.tag.id);
            const isActive = idx === activeIndex;
            return (
              <button
                key={item.tag.id}
                ref={(el) => {
                  optionRefs.current[item.tag.id] = el;
                }}
                id={`${listboxId}-opt-${item.tag.id}`}
                type="button"
                role="option"
                aria-selected={isSelected}
                onMouseEnter={() => {
                  if (idx >= 0) {
                    setActiveIndex(idx);
                  }
                }}
                onClick={() => onToggleTag(item.tag.id)}
                className={[
                  'flex h-[28px] w-full items-center justify-between gap-2 rounded-[6px] px-2 text-left transition-colors duration-75',
                  isActive ? 'bg-[var(--bg-hover)]' : 'hover:bg-[var(--bg-hover)]',
                ].join(' ')}
              >
                <span className="flex min-w-0 items-center gap-2">
                  <span className="inline-flex w-3.5 shrink-0 items-center justify-center">
                    {isSelected && (
                      <Check
                        className="h-3.5 w-3.5 text-[var(--accent)]"
                        strokeWidth={2.2}
                        aria-hidden="true"
                      />
                    )}
                  </span>
                  <span
                    className="h-2.5 w-2.5 shrink-0 rounded-full"
                    style={{ backgroundColor: tagColorHex(item.tag.color) }}
                    aria-hidden="true"
                  />
                  <span
                    className={[
                      'truncate text-[12.5px] text-[var(--text-primary)]',
                      isSelected ? 'font-medium' : '',
                    ].join(' ')}
                  >
                    {item.tag.label}
                  </span>
                </span>
                <span className="tabular-nums shrink-0 text-[11px] text-[var(--text-tertiary)]">
                  {item.projectCount}
                </span>
              </button>
            );
          })}
        </div>
      </div>
    );
  };

  return (
    <div
      ref={popoverRef}
      role="dialog"
      aria-label="Filter projects by tag"
      onKeyDown={handleContainerKeyDown}
      className="absolute left-2.5 top-full z-40 mt-1.5 flex w-[264px] max-w-[calc(100vw-24px)] flex-col overflow-hidden rounded-[10px] bg-[var(--bg-popover)] text-[12px] shadow-[0_14px_36px_rgba(0,0,0,0.24),0_0_0_0.5px_var(--hairline)] backdrop-blur-2xl"
    >
      {/* Search input */}
      <div className="border-b border-[var(--hairline-inset)] p-2">
        <div className="relative flex items-center">
          <Search
            className="pointer-events-none absolute left-2 h-3.5 w-3.5 text-[var(--text-tertiary)]"
            strokeWidth={1.75}
            aria-hidden="true"
          />
          <input
            ref={searchRef}
            type="text"
            role="combobox"
            aria-expanded="true"
            aria-controls={listboxId}
            aria-activedescendant={activeOptionId}
            aria-label="Search tags"
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              setActiveIndex(0);
            }}
            onKeyDown={handleSearchKeyDown}
            placeholder="Search tags…"
            className="h-[26px] w-full rounded-[6px] bg-[var(--bg-field)] pl-7 pr-6 text-[12px] text-[var(--text-primary)] shadow-[inset_0_0.5px_1px_rgba(0,0,0,0.08)] placeholder:text-[var(--text-tertiary)]"
          />
          {query && (
            <button
              type="button"
              onClick={() => {
                setQuery('');
                setActiveIndex(0);
                searchRef.current?.focus();
              }}
              aria-label="Clear tag search"
              className="absolute right-1 inline-flex h-4 w-4 items-center justify-center rounded text-[var(--text-tertiary)] hover:text-[var(--text-primary)]"
            >
              <X className="h-3 w-3" strokeWidth={1.75} />
            </button>
          )}
        </div>
      </div>

      {/* Scrollable grouped tag list */}
      <div
        id={listboxId}
        role="listbox"
        aria-multiselectable="true"
        aria-label="Available tags"
        className="max-h-[260px] overflow-y-auto p-1.5"
      >
        {orderedVisible.length === 0 ? (
          <div className="py-5 text-center text-[11.5px] text-[var(--text-tertiary)]">
            No matching tags
          </div>
        ) : (
          <>
            {renderSection('Custom Tags', customTags, true)}
            {renderSection(
              'Folder Tags',
              folderTags,
              customTags.length === 0,
            )}
          </>
        )}
      </div>

      {/* Footer: Match mode + Clear + Manage */}
      <div className="flex items-center justify-between gap-2 border-t border-[var(--hairline-inset)] px-2.5 py-2">
        <div className="flex items-center">
          {selectedTagIds.length >= 2 ? (
            <div
              role="group"
              aria-label="Tag match mode"
              className="inline-flex items-center rounded-[5px] bg-[var(--bg-field)] p-[2px]"
            >
              {(['all', 'any'] as const).map((mode) => {
                const active = matchMode === mode;
                return (
                  <button
                    key={mode}
                    type="button"
                    aria-pressed={active}
                    onClick={() => onMatchModeChange(mode)}
                    className={[
                      'h-[18px] rounded-[4px] px-1.5 text-[10.5px] font-medium transition-colors duration-100',
                      active
                        ? 'bg-[var(--bg-control)] text-[var(--text-primary)] shadow-[var(--control-shadow)]'
                        : 'text-[var(--text-tertiary)] hover:text-[var(--text-primary)]',
                    ].join(' ')}
                  >
                    {mode === 'all' ? 'All' : 'Any'}
                  </button>
                );
              })}
            </div>
          ) : selectedTagIds.length === 1 ? (
            <span className="text-[11px] text-[var(--text-tertiary)]">
              1 selected
            </span>
          ) : (
            <span className="text-[11px] text-[var(--text-tertiary)]">
              Click or ↵ to toggle
            </span>
          )}
        </div>

        <div className="flex items-center gap-1.5">
          {selectedTagIds.length > 0 && (
            <button
              type="button"
              onClick={onClearAll}
              className="rounded-[5px] px-1.5 py-0.5 text-[11px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
            >
              Clear
            </button>
          )}
          <button
            type="button"
            onClick={() => {
              onClose();
              onOpenTagSettings();
            }}
            className="inline-flex items-center gap-1 rounded-[5px] px-1.5 py-0.5 text-[11px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
          >
            <Settings2 className="h-3 w-3" aria-hidden="true" />
            <span>Manage…</span>
          </button>
        </div>
      </div>
    </div>
  );
}

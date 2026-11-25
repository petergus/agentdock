import { useEffect, useMemo, useRef, useState, type RefObject } from 'react';
import {
  Archive,
  ChevronRight,
  Eye,
  EyeOff,
  Folder,
  FolderOpen,
  FolderTree,
  Globe,
  List,
  Pin,
  RefreshCw,
  Search,
  Settings,
  ShieldCheck,
  Tag,
  X,
} from 'lucide-react';
import type { CategoryGroup, ProjectSummary, ProjectTag } from '@/types';
import {
  formatTokensCompact,
  formatTokensExact,
  tagColorHex,
} from '@/lib/format';
import {
  MacContextMenu,
  type MacContextMenuItem,
} from '@/components/ui/MacContextMenu';
import {
  SidebarTagFilterPopover,
  type SidebarTagOption,
  type TagMatchMode,
} from '@/components/SidebarTagFilterPopover';

type SidebarViewMode = 'grouped' | 'list';

const SIDEBAR_VIEW_STORAGE_KEY = 'agentdock.sidebarViewMode';
const SIDEBAR_WIDTH_STORAGE_KEY = 'agentdock.sidebarWidth';
const DEFAULT_SIDEBAR_WIDTH = 256;
const MIN_SIDEBAR_WIDTH = 220;
const MAX_SIDEBAR_WIDTH = 340;

interface SidebarProps {
  globalTokens: number;
  categories: CategoryGroup[];
  selectedScopeId: string;
  searchQuery: string;
  onSearchChange: (q: string) => void;
  searchInputRef: RefObject<HTMLInputElement | null>;
  onSelectGlobal: () => void;
  onSelectProject: (project: ProjectSummary) => void;
  onTogglePinProject: (projectPath: string) => void;
  pinningPath: string | null;
  onToggleHideProject: (projectPath: string) => void;
  hidingPath: string | null;
  onRevealPath: (path: string) => void;
  onOpenTagSettings: () => void;
  onOpenSettings: () => void;
  onOpenCleanup: () => void;
  cleanupIssueCount: number;
  onRefresh: () => void;
  isRefreshing: boolean;
}

function loadInitialViewMode(): SidebarViewMode {
  try {
    const saved = window.localStorage.getItem(SIDEBAR_VIEW_STORAGE_KEY);
    if (saved === 'list' || saved === 'grouped') {
      return saved;
    }
  } catch {
    // Ignore storage read failures
  }
  return 'grouped';
}

function loadInitialSidebarWidth(): number {
  try {
    const raw = window.localStorage.getItem(SIDEBAR_WIDTH_STORAGE_KEY);
    const parsed = raw ? Number(raw) : NaN;
    if (Number.isFinite(parsed)) {
      return Math.min(MAX_SIDEBAR_WIDTH, Math.max(MIN_SIDEBAR_WIDTH, parsed));
    }
  } catch {
    // Ignore storage read failures
  }
  return DEFAULT_SIDEBAR_WIDTH;
}

interface ProjectRowProps {
  project: ProjectSummary;
  globalTokens: number;
  isSelected: boolean;
  isPinBusy: boolean;
  isHideBusy: boolean;
  showCategorySubtitle?: boolean;
  onSelect: () => void;
  onTogglePin: () => void;
  onToggleHide: () => void;
  onContextMenu: (e: React.MouseEvent, project: ProjectSummary) => void;
}

function ProjectRow({
  project,
  globalTokens,
  isSelected,
  isPinBusy,
  isHideBusy,
  showCategorySubtitle = false,
  onSelect,
  onTogglePin,
  onToggleHide,
  onContextMenu,
}: ProjectRowProps) {
  const tagsTooltip =
    project.tags.length > 0
      ? `\nTags: ${project.tags.map((t) => t.label).join(', ')}`
      : '';
  const tooltip =
    project.projectExtraTokens > 0
      ? `${project.name} (${project.category}): ${formatTokensExact(project.effectiveTokens)} total (${formatTokensExact(globalTokens)} global + ${formatTokensExact(project.projectExtraTokens)} project)${tagsTooltip}`
      : `${project.name} (${project.category}): ${formatTokensExact(project.effectiveTokens)} total (Global baseline)${tagsTooltip}`;

  return (
    <div
      onContextMenu={(e) => {
        e.preventDefault();
        onContextMenu(e, project);
      }}
      className={[
        'group flex items-center gap-0.5 rounded-[7px] pr-1 transition-colors duration-100',
        isSelected
          ? 'mac-sidebar-selected'
          : 'hover:bg-[var(--bg-hover)]',
      ].join(' ')}
    >
      <button
        type="button"
        onClick={onSelect}
        title={tooltip}
        className={[
          'flex h-[28px] min-w-0 flex-1 items-center justify-between rounded-[7px] pl-2 pr-1 text-left',
          isSelected ? 'font-medium' : 'text-[var(--text-primary)]',
        ].join(' ')}
      >
        <span className="flex min-w-0 items-center gap-1.5">
          {project.isPinned ? (
            <Pin
              className={[
                'h-3 w-3 shrink-0',
                isSelected ? 'mac-sidebar-selected-icon' : 'text-[var(--accent)]',
              ].join(' ')}
              strokeWidth={1.9}
              aria-hidden="true"
            />
          ) : (
            <Folder
              className={[
                'h-3.5 w-3.5 shrink-0',
                isSelected
                  ? 'mac-sidebar-selected-icon opacity-90'
                  : 'text-[var(--text-tertiary)]',
              ].join(' ')}
              strokeWidth={1.6}
              aria-hidden="true"
            />
          )}
          <span className="truncate text-[12.5px]">{project.name}</span>
          {project.tags.length > 0 && (
            <span className="flex shrink-0 items-center gap-0.5" aria-hidden="true">
              {project.tags.slice(0, 3).map((tag) => (
                <span
                  key={tag.id}
                  className={[
                    'h-2 w-2 rounded-full',
                    isSelected ? 'ring-1 ring-white/70' : '',
                  ].join(' ')}
                  style={{ backgroundColor: tagColorHex(tag.color) }}
                />
              ))}
            </span>
          )}
          {showCategorySubtitle && (
            <span
              className={[
                'truncate text-[11px]',
                isSelected
                  ? 'mac-sidebar-selected-muted'
                  : 'text-[var(--text-tertiary)]',
              ].join(' ')}
            >
              {project.category}
            </span>
          )}
        </span>
        <span
          className={[
            'tabular-nums ml-1.5 shrink-0 text-[11.5px]',
            isSelected
              ? 'mac-sidebar-selected-muted font-medium'
              : project.projectExtraTokens > 0
                ? 'font-medium text-[var(--text-primary)]'
                : 'text-[var(--text-tertiary)]',
          ].join(' ')}
        >
          {formatTokensCompact(project.effectiveTokens)}
        </span>
      </button>

      <button
        type="button"
        disabled={isHideBusy}
        onClick={onToggleHide}
        aria-label={
          project.isHidden ? `Unhide ${project.name}` : `Hide ${project.name}`
        }
        title={
          project.isHidden
            ? 'Restore from Hidden'
            : 'Hide project into Hidden section'
        }
        className={[
          'inline-flex h-5 w-5 shrink-0 items-center justify-center rounded-[5px] opacity-0 transition-opacity duration-120 group-hover:opacity-100 focus-visible:opacity-100',
          isSelected
            ? 'mac-sidebar-selected-icon hover:bg-white/20'
            : 'text-[var(--text-tertiary)] hover:bg-black/[0.06] hover:text-[var(--text-primary)] dark:hover:bg-white/[0.08]',
          isHideBusy ? 'cursor-wait opacity-50' : '',
        ].join(' ')}
      >
        {project.isHidden ? (
          <Eye className="h-3 w-3" strokeWidth={1.8} />
        ) : (
          <EyeOff className="h-3 w-3" strokeWidth={1.8} />
        )}
      </button>

      {!project.isHidden && (
        <button
          type="button"
          disabled={isPinBusy}
          onClick={onTogglePin}
          aria-label={
            project.isPinned ? `Unpin ${project.name}` : `Pin ${project.name}`
          }
          aria-pressed={project.isPinned}
          title={
            project.isPinned
              ? 'Unpin from top of sidebar'
              : 'Pin to top of sidebar'
          }
          className={[
            'inline-flex h-5 w-5 shrink-0 items-center justify-center rounded-[5px] transition-opacity duration-120',
            isSelected
              ? project.isPinned
                ? 'mac-sidebar-selected-icon opacity-100 hover:bg-white/20'
                : 'mac-sidebar-selected-icon opacity-0 hover:bg-white/20 group-hover:opacity-100 focus-visible:opacity-100'
              : project.isPinned
                ? 'text-[var(--accent)] opacity-100 hover:bg-black/[0.06] dark:hover:bg-white/[0.08]'
                : 'text-[var(--text-tertiary)] opacity-0 hover:bg-black/[0.06] hover:text-[var(--text-primary)] group-hover:opacity-100 focus-visible:opacity-100 dark:hover:bg-white/[0.08]',
            isPinBusy ? 'cursor-wait opacity-50' : '',
          ].join(' ')}
        >
          <Pin className="h-3 w-3" strokeWidth={1.8} />
        </button>
      )}
    </div>
  );
}

export function Sidebar({
  globalTokens,
  categories,
  selectedScopeId,
  searchQuery,
  onSearchChange,
  searchInputRef,
  onSelectGlobal,
  onSelectProject,
  onTogglePinProject,
  pinningPath,
  onToggleHideProject,
  hidingPath,
  onRevealPath,
  onOpenTagSettings,
  onOpenSettings,
  onOpenCleanup,
  cleanupIssueCount,
  onRefresh,
  isRefreshing,
}: SidebarProps) {
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>({});
  const [hiddenOpen, setHiddenOpen] = useState(false);
  const [selectedTagIds, setSelectedTagIds] = useState<string[]>([]);
  const [tagMatchMode, setTagMatchMode] = useState<TagMatchMode>('all');
  const [tagPopoverOpen, setTagPopoverOpen] = useState(false);
  const [viewMode, setViewMode] = useState<SidebarViewMode>(loadInitialViewMode);
  const [sidebarWidth, setSidebarWidth] = useState<number>(
    loadInitialSidebarWidth,
  );
  const [resizing, setResizing] = useState(false);
  const [contextMenu, setContextMenu] = useState<{
    open: boolean;
    x: number;
    y: number;
    project: ProjectSummary | null;
  }>({ open: false, x: 0, y: 0, project: null });

  const tagTriggerRef = useRef<HTMLButtonElement | null>(null);
  const resizeOriginRef = useRef<{ startX: number; startWidth: number } | null>(
    null,
  );

  useEffect(() => {
    try {
      window.localStorage.setItem(SIDEBAR_VIEW_STORAGE_KEY, viewMode);
    } catch {
      // Ignore storage write failures
    }
  }, [viewMode]);

  useEffect(() => {
    try {
      window.localStorage.setItem(
        SIDEBAR_WIDTH_STORAGE_KEY,
        String(sidebarWidth),
      );
    } catch {
      // Ignore storage write failures
    }
  }, [sidebarWidth]);

  useEffect(() => {
    if (!resizing) {
      return;
    }
    const onMouseMove = (e: MouseEvent) => {
      if (!resizeOriginRef.current) {
        return;
      }
      const delta = e.clientX - resizeOriginRef.current.startX;
      const next = Math.min(
        MAX_SIDEBAR_WIDTH,
        Math.max(MIN_SIDEBAR_WIDTH, resizeOriginRef.current.startWidth + delta),
      );
      setSidebarWidth(next);
    };
    const onMouseUp = () => {
      setResizing(false);
      resizeOriginRef.current = null;
    };
    window.addEventListener('mousemove', onMouseMove);
    window.addEventListener('mouseup', onMouseUp);
    return () => {
      window.removeEventListener('mousemove', onMouseMove);
      window.removeEventListener('mouseup', onMouseUp);
    };
  }, [resizing]);

  const allUniqueProjects = useMemo(() => {
    const byId = new Map<string, ProjectSummary>();
    for (const cat of categories) {
      for (const proj of cat.projects) {
        if (!byId.has(proj.id)) {
          byId.set(proj.id, proj);
        }
      }
    }
    return Array.from(byId.values());
  }, [categories]);

  const availableTags = useMemo<SidebarTagOption[]>(() => {
    const map = new Map<string, { tag: ProjectTag; count: number }>();
    for (const proj of allUniqueProjects) {
      if (proj.isHidden) {
        continue;
      }
      const seenInProject = new Set<string>();
      for (const tag of proj.tags) {
        if (seenInProject.has(tag.id)) {
          continue;
        }
        seenInProject.add(tag.id);
        const existing = map.get(tag.id);
        if (existing) {
          existing.count += 1;
        } else {
          map.set(tag.id, { tag, count: 1 });
        }
      }
    }
    return Array.from(map.values())
      .map(({ tag, count }) => ({ tag, projectCount: count }))
      .sort((a, b) => {
        if (b.projectCount !== a.projectCount) {
          return b.projectCount - a.projectCount;
        }
        return a.tag.label.localeCompare(b.tag.label, undefined, {
          sensitivity: 'base',
        });
      });
  }, [allUniqueProjects]);

  const tagById = useMemo(() => {
    const map = new Map<string, SidebarTagOption>();
    for (const option of availableTags) {
      map.set(option.tag.id, option);
    }
    return map;
  }, [availableTags]);

  const selectedTagOptions = useMemo(
    () =>
      selectedTagIds
        .map((id) => tagById.get(id))
        .filter((opt): opt is SidebarTagOption => opt !== undefined),
    [selectedTagIds, tagById],
  );

  useEffect(() => {
    if (availableTags.length === 0) {
      setTagPopoverOpen(false);
    }
    setSelectedTagIds((prev) => {
      if (prev.length === 0) {
        return prev;
      }
      const validIds = new Set(availableTags.map((item) => item.tag.id));
      const next = prev.filter((id) => validIds.has(id));
      return next.length === prev.length ? prev : next;
    });
  }, [availableTags]);

  const handleToggleTagFilter = (tagId: string) => {
    setSelectedTagIds((prev) =>
      prev.includes(tagId)
        ? prev.filter((id) => id !== tagId)
        : [...prev, tagId],
    );
  };

  const nameCounts = useMemo(() => {
    const counts = new Map<string, number>();
    for (const proj of allUniqueProjects) {
      const key = proj.name.toLowerCase();
      counts.set(key, (counts.get(key) ?? 0) + 1);
    }
    return counts;
  }, [allUniqueProjects]);

  const matchesSelectedTags = (p: ProjectSummary) => {
    if (selectedTagIds.length === 0) {
      return true;
    }
    if (tagMatchMode === 'all') {
      return selectedTagIds.every((id) => p.tags.some((t) => t.id === id));
    }
    return selectedTagIds.some((id) => p.tags.some((t) => t.id === id));
  };

  const matchesProjectQuery = (p: ProjectSummary, q: string) => {
    if (!matchesSelectedTags(p)) {
      return false;
    }
    if (!q) {
      return true;
    }
    return (
      p.name.toLowerCase().includes(q) ||
      p.category.toLowerCase().includes(q) ||
      p.tags.some((t) => t.label.toLowerCase().includes(q))
    );
  };

  const visibleProjectsFlat = useMemo(() => {
    const q = searchQuery.trim().toLowerCase();
    const filtered = allUniqueProjects.filter(
      (p) => !p.isHidden && matchesProjectQuery(p, q),
    );
    return [...filtered].sort((a, b) =>
      a.name.localeCompare(b.name, undefined, { sensitivity: 'base' }),
    );
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [allUniqueProjects, searchQuery, selectedTagIds, tagMatchMode]);

  const hiddenProjectsFlat = useMemo(() => {
    const q = searchQuery.trim().toLowerCase();
    const filtered = allUniqueProjects.filter(
      (p) => p.isHidden && matchesProjectQuery(p, q),
    );
    return [...filtered].sort((a, b) =>
      a.name.localeCompare(b.name, undefined, { sensitivity: 'base' }),
    );
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [allUniqueProjects, searchQuery, selectedTagIds, tagMatchMode]);

  const allHiddenCount = useMemo(
    () => allUniqueProjects.filter((p) => p.isHidden).length,
    [allUniqueProjects],
  );

  const pinnedProjects = useMemo(
    () => visibleProjectsFlat.filter((p) => p.isPinned),
    [visibleProjectsFlat],
  );

  const unpinnedProjects = useMemo(
    () => visibleProjectsFlat.filter((p) => !p.isPinned),
    [visibleProjectsFlat],
  );

  const filteredCategories = useMemo(() => {
    const q = searchQuery.trim().toLowerCase();
    const baseCategories = categories.filter((c) => c.name !== 'Pinned');
    return baseCategories
      .map((cat) => {
        const nonHidden = cat.projects.filter((p) => !p.isHidden);
        const catMatch = Boolean(q) && cat.name.toLowerCase().includes(q);
        const matchingProjects = nonHidden.filter((p) => {
          if (!matchesSelectedTags(p)) {
            return false;
          }
          if (!q || catMatch) {
            return true;
          }
          return (
            p.name.toLowerCase().includes(q) ||
            p.tags.some((t) => t.label.toLowerCase().includes(q))
          );
        });
        return { ...cat, projects: matchingProjects };
      })
      .filter((cat) => cat.projects.length > 0);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [categories, searchQuery, selectedTagIds, tagMatchMode]);

  const toggleCategory = (name: string) => {
    setCollapsed((prev) => ({ ...prev, [name]: !prev[name] }));
  };

  const effectiveHiddenOpen =
    hiddenOpen ||
    (searchQuery.trim().length > 0 && hiddenProjectsFlat.length > 0) ||
    (selectedTagIds.length > 0 &&
      visibleProjectsFlat.length === 0 &&
      hiddenProjectsFlat.length > 0);

  const openProjectContextMenu = (
    e: React.MouseEvent,
    project: ProjectSummary,
  ) => {
    setContextMenu({
      open: true,
      x: e.clientX,
      y: e.clientY,
      project,
    });
  };

  const contextMenuItems: MacContextMenuItem[] = useMemo(() => {
    const proj = contextMenu.project;
    if (!proj) {
      return [];
    }
    const items: MacContextMenuItem[] = [
      {
        id: 'reveal',
        label: 'Reveal in Finder',
        icon: <FolderOpen className="h-3.5 w-3.5" strokeWidth={1.75} />,
        onSelect: () => onRevealPath(proj.path),
      },
    ];
    if (!proj.isHidden) {
      items.push({
        id: 'pin',
        label: proj.isPinned ? 'Unpin from Top' : 'Pin to Top',
        icon: <Pin className="h-3.5 w-3.5" strokeWidth={1.75} />,
        onSelect: () => onTogglePinProject(proj.path),
      });
    }
    items.push(
      {
        id: 'tags',
        label: 'Edit Tags in Settings…',
        icon: <Tag className="h-3.5 w-3.5" strokeWidth={1.75} />,
        onSelect: () => onOpenTagSettings(),
      },
      { kind: 'separator', id: 'sep-1' },
      {
        id: 'hide',
        label: proj.isHidden ? 'Restore from Hidden' : 'Hide Project',
        icon: proj.isHidden ? (
          <Eye className="h-3.5 w-3.5" strokeWidth={1.75} />
        ) : (
          <EyeOff className="h-3.5 w-3.5" strokeWidth={1.75} />
        ),
        onSelect: () => onToggleHideProject(proj.path),
      },
    );
    return items;
  }, [
    contextMenu.project,
    onRevealPath,
    onTogglePinProject,
    onOpenTagSettings,
    onToggleHideProject,
  ]);

  const isGlobalSelected = selectedScopeId === 'global';

  return (
    <aside
      style={{ width: `${sidebarWidth}px` }}
      className="relative flex h-full shrink-0 flex-col border-r border-[var(--hairline)]"
    >
      {/* Native macOS traffic-light safe drag header (48px aligned with right toolbar) */}
      <div
        data-tauri-drag-region
        className="flex h-12 shrink-0 items-center pl-[78px] pr-3"
      >
        <span
          data-tauri-drag-region
          className="text-[12.5px] font-semibold tracking-[-0.01em] text-[var(--text-secondary)]"
        >
          AgentDock
        </span>
      </div>

      {/* Unified Filter Bar (⌘K) + Tag Picker Popover + Active Tokens */}
      <div className="relative z-30 px-2.5 pb-2">
        <div className="flex items-center gap-1.5">
          <div className="relative flex min-w-0 flex-1 items-center">
            <Search
              className="pointer-events-none absolute left-2.5 h-3.5 w-3.5 text-[var(--text-tertiary)]"
              strokeWidth={1.75}
              aria-hidden="true"
            />
            <input
              ref={searchInputRef}
              type="search"
              value={searchQuery}
              onChange={(e) => onSearchChange(e.target.value)}
              onKeyDown={(e) => {
                if (
                  e.key === '#' &&
                  availableTags.length > 0 &&
                  e.currentTarget.selectionStart ===
                    e.currentTarget.value.length &&
                  e.currentTarget.selectionEnd ===
                    e.currentTarget.value.length &&
                  (searchQuery.length === 0 || /\s$/.test(searchQuery))
                ) {
                  e.preventDefault();
                  setTagPopoverOpen(true);
                } else if (
                  e.key === 'Backspace' &&
                  searchQuery === '' &&
                  selectedTagIds.length > 0
                ) {
                  setSelectedTagIds((prev) => prev.slice(0, -1));
                }
              }}
              placeholder="Filter…"
              aria-label="Filter projects or tags"
              className="h-[28px] w-full rounded-[7px] bg-[var(--bg-field)] pl-8 pr-10 text-[12.5px] text-[var(--text-primary)] shadow-[inset_0_0.5px_1px_rgba(0,0,0,0.08)] placeholder:text-[var(--text-tertiary)]"
            />
            {searchQuery ? (
              <button
                type="button"
                onClick={() => onSearchChange('')}
                aria-label="Clear filter"
                className="absolute right-1.5 inline-flex h-5 w-5 items-center justify-center rounded text-[var(--text-tertiary)] hover:text-[var(--text-primary)]"
              >
                <X className="h-3 w-3" strokeWidth={1.75} />
              </button>
            ) : (
              <kbd
                aria-hidden="true"
                className="pointer-events-none absolute right-2 text-[10.5px] font-medium text-[var(--text-tertiary)]"
              >
                ⌘K
              </kbd>
            )}
          </div>

          {availableTags.length > 0 && (
            <button
              ref={tagTriggerRef}
              type="button"
              aria-haspopup="dialog"
              aria-expanded={tagPopoverOpen}
              aria-label={
                selectedTagIds.length > 0
                  ? `Filter by tags (${selectedTagIds.length} active)`
                  : 'Filter by tags'
              }
              title="Filter by tags (type # in Filter)"
              onClick={() => setTagPopoverOpen((prev) => !prev)}
              className={[
                'inline-flex h-[28px] shrink-0 items-center justify-center rounded-[7px] transition-colors duration-100',
                selectedTagIds.length > 0
                  ? 'gap-1 bg-[var(--accent)] px-2 font-medium text-white'
                  : tagPopoverOpen
                    ? 'w-[28px] bg-[var(--bg-control)] text-[var(--text-primary)] shadow-[var(--control-shadow)]'
                    : 'w-[28px] bg-[var(--bg-field)] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]',
              ].join(' ')}
            >
              <Tag
                className={
                  selectedTagIds.length > 0 ? 'h-3 w-3 shrink-0' : 'h-3.5 w-3.5'
                }
                strokeWidth={selectedTagIds.length > 0 ? 1.9 : 1.75}
                aria-hidden="true"
              />
              {selectedTagIds.length > 0 && (
                <span className="tabular-nums text-[11px]">
                  {selectedTagIds.length}
                </span>
              )}
            </button>
          )}
        </div>

        {/* Active Tag Tokens (rendered only when 1+ tags are selected) */}
        {selectedTagOptions.length > 0 && (
          <div
            role="group"
            aria-label="Active tag filters"
            className="mt-1.5 flex flex-wrap items-center gap-1"
          >
            {selectedTagOptions.map(({ tag }) => (
              <span
                key={tag.id}
                className="inline-flex h-[22px] max-w-full items-center gap-1 rounded-[5px] bg-[var(--accent)]/12 pl-1.5 pr-0.5 text-[11px] font-medium text-[var(--text-primary)] ring-1 ring-inset ring-[var(--accent)]/25"
              >
                <span
                  className="h-2 w-2 shrink-0 rounded-full"
                  style={{ backgroundColor: tagColorHex(tag.color) }}
                  aria-hidden="true"
                />
                <span className="max-w-[120px] truncate">{tag.label}</span>
                <button
                  type="button"
                  onClick={() => handleToggleTagFilter(tag.id)}
                  aria-label={`Remove ${tag.label} tag filter`}
                  className="inline-flex h-4 w-4 shrink-0 items-center justify-center rounded-[4px] text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
                >
                  <X className="h-2.5 w-2.5" strokeWidth={2} />
                </button>
              </span>
            ))}

            {selectedTagOptions.length > 1 && (
              <button
                type="button"
                onClick={() =>
                  setTagMatchMode((prev) => (prev === 'all' ? 'any' : 'all'))
                }
                title="Click to switch between matching all or any selected tags"
                className="h-[22px] rounded-[5px] bg-[var(--bg-field)] px-1.5 text-[10.5px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
              >
                {tagMatchMode === 'all' ? 'All' : 'Any'}
              </button>
            )}

            <button
              type="button"
              onClick={() => setSelectedTagIds([])}
              className="ml-auto rounded-[4px] px-1 py-0.5 text-[11px] font-medium text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
            >
              Clear
            </button>
          </div>
        )}

        <SidebarTagFilterPopover
          open={tagPopoverOpen}
          tags={availableTags}
          selectedTagIds={selectedTagIds}
          matchMode={tagMatchMode}
          triggerRef={tagTriggerRef}
          returnFocusRef={searchInputRef}
          onToggleTag={handleToggleTagFilter}
          onMatchModeChange={setTagMatchMode}
          onClearAll={() => setSelectedTagIds([])}
          onOpenTagSettings={onOpenTagSettings}
          onClose={() => setTagPopoverOpen(false)}
        />
      </div>

      {/* Pinned Global Scope */}
      <div className="px-2 pb-2">
        <button
          type="button"
          onClick={onSelectGlobal}
          title={`Global baseline: ${formatTokensExact(globalTokens)} startup tokens (⌘1)`}
          className={[
            'flex h-[30px] w-full items-center justify-between rounded-[7px] px-2.5 text-left transition-colors duration-100',
            isGlobalSelected
              ? 'mac-sidebar-selected font-medium'
              : 'text-[var(--text-primary)] hover:bg-[var(--bg-hover)]',
          ].join(' ')}
        >
          <span className="flex min-w-0 items-center gap-2">
            <Globe
              className={[
                'h-3.5 w-3.5 shrink-0',
                isGlobalSelected
                  ? 'mac-sidebar-selected-icon'
                  : 'text-[var(--accent)]',
              ].join(' ')}
              strokeWidth={1.75}
              aria-hidden="true"
            />
            <span className="truncate text-[13px]">Global</span>
          </span>
          <span
            className={[
              'tabular-nums text-[12px]',
              isGlobalSelected
                ? 'mac-sidebar-selected-muted font-medium'
                : 'text-[var(--text-secondary)]',
            ].join(' ')}
          >
            {formatTokensCompact(globalTokens)}
          </span>
        </button>
      </div>

      {/* Hairline + Projects Header + View Mode Switcher */}
      <div className="mx-3 border-t border-[var(--hairline-inset)]" />

      <div className="flex-1 overflow-y-auto overscroll-y-contain px-2 py-2.5">
        <div className="flex items-center justify-between px-2 pb-1.5">
          <span className="text-[11px] font-semibold tracking-[0.02em] text-[var(--text-tertiary)]">
            Projects
          </span>

          <div
            role="group"
            aria-label="Project list layout"
            className="inline-flex items-center rounded-[6px] bg-[var(--bg-field)] p-[2px]"
          >
            <button
              type="button"
              aria-pressed={viewMode === 'list'}
              onClick={() => setViewMode('list')}
              aria-label="One flat list view"
              title="One flat list"
              className={[
                'inline-flex h-[18px] w-[20px] items-center justify-center rounded-[4px] transition-colors duration-100',
                viewMode === 'list'
                  ? 'bg-[var(--bg-control)] text-[var(--text-primary)] shadow-[var(--control-shadow)]'
                  : 'text-[var(--text-tertiary)] hover:text-[var(--text-primary)]',
              ].join(' ')}
            >
              <List className="h-3 w-3" strokeWidth={1.85} />
            </button>
            <button
              type="button"
              aria-pressed={viewMode === 'grouped'}
              onClick={() => setViewMode('grouped')}
              aria-label="Folder tree view"
              title="Grouped by folder"
              className={[
                'inline-flex h-[18px] w-[20px] items-center justify-center rounded-[4px] transition-colors duration-100',
                viewMode === 'grouped'
                  ? 'bg-[var(--bg-control)] text-[var(--text-primary)] shadow-[var(--control-shadow)]'
                  : 'text-[var(--text-tertiary)] hover:text-[var(--text-primary)]',
              ].join(' ')}
            >
              <FolderTree className="h-3 w-3" strokeWidth={1.85} />
            </button>
          </div>
        </div>

        {visibleProjectsFlat.length === 0 && hiddenProjectsFlat.length === 0 ? (
          <div className="space-y-1.5 px-2 py-4 text-[12px] text-[var(--text-tertiary)]">
            <div>
              {allUniqueProjects.length === 0
                ? 'No projects found. Add a Project Location or adjust Scan Depth in Settings.'
                : 'No matching projects.'}
            </div>
            {allUniqueProjects.length > 0 &&
              (selectedTagIds.length > 0 || searchQuery.trim().length > 0) && (
                <button
                  type="button"
                  onClick={() => {
                    setSelectedTagIds([]);
                    onSearchChange('');
                  }}
                  className="text-[11.5px] font-medium text-[var(--accent)] hover:underline"
                >
                  Clear filters
                </button>
              )}
          </div>
        ) : viewMode === 'list' ? (
          <div className="space-y-0.5">
            {pinnedProjects.map((proj) => (
              <ProjectRow
                key={proj.id}
                project={proj}
                globalTokens={globalTokens}
                isSelected={selectedScopeId === proj.id}
                isPinBusy={pinningPath === proj.path}
                isHideBusy={hidingPath === proj.path}
                showCategorySubtitle={(nameCounts.get(proj.name.toLowerCase()) ?? 0) > 1}
                onSelect={() => onSelectProject(proj)}
                onTogglePin={() => onTogglePinProject(proj.path)}
                onToggleHide={() => onToggleHideProject(proj.path)}
                onContextMenu={openProjectContextMenu}
              />
            ))}

            {pinnedProjects.length > 0 && unpinnedProjects.length > 0 && (
              <div className="mx-2 my-1.5 border-t border-[var(--hairline-inset)]" />
            )}

            {unpinnedProjects.map((proj) => (
              <ProjectRow
                key={proj.id}
                project={proj}
                globalTokens={globalTokens}
                isSelected={selectedScopeId === proj.id}
                isPinBusy={pinningPath === proj.path}
                isHideBusy={hidingPath === proj.path}
                showCategorySubtitle={(nameCounts.get(proj.name.toLowerCase()) ?? 0) > 1}
                onSelect={() => onSelectProject(proj)}
                onTogglePin={() => onTogglePinProject(proj.path)}
                onToggleHide={() => onToggleHideProject(proj.path)}
                onContextMenu={openProjectContextMenu}
              />
            ))}
          </div>
        ) : (
          <div className="space-y-1.5">
            {pinnedProjects.length > 0 && (
              <div>
                <div className="flex h-6 items-center justify-between px-2 text-[11px] font-semibold text-[var(--accent)]">
                  <span>Pinned</span>
                  <span className="tabular-nums text-[11px]">
                    {pinnedProjects.length}
                  </span>
                </div>
                <div className="mt-0.5 space-y-0.5">
                  {pinnedProjects.map((proj) => (
                    <ProjectRow
                      key={`pinned:${proj.id}`}
                      project={proj}
                      globalTokens={globalTokens}
                      isSelected={selectedScopeId === proj.id}
                      isPinBusy={pinningPath === proj.path}
                      isHideBusy={hidingPath === proj.path}
                      showCategorySubtitle={(nameCounts.get(proj.name.toLowerCase()) ?? 0) > 1}
                      onSelect={() => onSelectProject(proj)}
                      onTogglePin={() => onTogglePinProject(proj.path)}
                      onToggleHide={() => onToggleHideProject(proj.path)}
                      onContextMenu={openProjectContextMenu}
                    />
                  ))}
                </div>
              </div>
            )}

            {filteredCategories.map((cat) => {
              const isOpen =
                searchQuery.trim().length > 0 ||
                selectedTagIds.length > 0 ||
                !collapsed[cat.name];
              return (
                <div key={cat.name}>
                  <button
                    type="button"
                    aria-expanded={isOpen}
                    onClick={() => toggleCategory(cat.name)}
                    className="flex h-6 w-full items-center justify-between rounded-[5px] px-1.5 text-left text-[11.5px] font-semibold text-[var(--text-secondary)] hover:bg-[var(--bg-hover)]"
                  >
                    <span className="flex min-w-0 items-center gap-1">
                      <ChevronRight
                        className={[
                          'h-3 w-3 shrink-0 text-[var(--text-tertiary)] transition-transform duration-150',
                          isOpen ? 'rotate-90' : 'rotate-0',
                        ].join(' ')}
                        strokeWidth={2}
                        aria-hidden="true"
                      />
                      <span className="truncate">{cat.name}</span>
                    </span>
                    <span className="tabular-nums text-[11px] font-normal text-[var(--text-tertiary)]">
                      {cat.projects.length}
                    </span>
                  </button>

                  {isOpen && (
                    <div className="mt-0.5 space-y-0.5 pl-2.5">
                      {cat.projects.map((proj) => (
                        <ProjectRow
                          key={proj.id}
                          project={proj}
                          globalTokens={globalTokens}
                          isSelected={selectedScopeId === proj.id}
                          isPinBusy={pinningPath === proj.path}
                          isHideBusy={hidingPath === proj.path}
                          onSelect={() => onSelectProject(proj)}
                          onTogglePin={() => onTogglePinProject(proj.path)}
                          onToggleHide={() => onToggleHideProject(proj.path)}
                          onContextMenu={openProjectContextMenu}
                        />
                      ))}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        )}

        {/* Hidden / Archived Projects Collapsible Section */}
        {allHiddenCount > 0 &&
          (hiddenProjectsFlat.length > 0 ||
            (!searchQuery.trim() && selectedTagIds.length === 0)) && (
            <div className="mt-3 border-t border-[var(--hairline-inset)] pt-2">
              <button
                type="button"
                aria-expanded={effectiveHiddenOpen}
                onClick={() => setHiddenOpen((prev) => !prev)}
                className="flex h-6 w-full items-center justify-between rounded-[5px] px-1.5 text-left text-[11.5px] font-semibold text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-secondary)]"
              >
                <span className="flex min-w-0 items-center gap-1.5">
                  <ChevronRight
                    className={[
                      'h-3 w-3 shrink-0 transition-transform duration-150',
                      effectiveHiddenOpen ? 'rotate-90' : 'rotate-0',
                    ].join(' ')}
                    strokeWidth={2}
                    aria-hidden="true"
                  />
                  <Archive className="h-3 w-3 shrink-0" strokeWidth={1.7} />
                  <span className="truncate">Hidden</span>
                </span>
                <span className="tabular-nums text-[11px] font-normal">
                  {hiddenProjectsFlat.length}
                </span>
              </button>

              {effectiveHiddenOpen && (
                <div className="mt-0.5 space-y-0.5 pl-2">
                  {hiddenProjectsFlat.map((proj) => (
                    <ProjectRow
                      key={`hidden:${proj.id}`}
                      project={proj}
                      globalTokens={globalTokens}
                      isSelected={selectedScopeId === proj.id}
                      isPinBusy={pinningPath === proj.path}
                      isHideBusy={hidingPath === proj.path}
                      showCategorySubtitle
                      onSelect={() => onSelectProject(proj)}
                      onTogglePin={() => onTogglePinProject(proj.path)}
                      onToggleHide={() => onToggleHideProject(proj.path)}
                      onContextMenu={openProjectContextMenu}
                    />
                  ))}
                </div>
              )}
            </div>
          )}
      </div>

      {/* Bottom Footer */}
      <div className="flex shrink-0 items-center justify-between gap-1 border-t border-[var(--hairline-inset)] px-2.5 py-2">
        <div className="flex items-center gap-1">
          <button
            type="button"
            onClick={onOpenSettings}
            title="Settings (⌘,)"
            className="inline-flex h-7 items-center gap-1.5 rounded-[6px] px-2 text-[12.5px] font-medium text-[var(--text-secondary)] transition-colors duration-100 hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
          >
            <Settings
              className="h-3.5 w-3.5 shrink-0"
              strokeWidth={1.75}
              aria-hidden="true"
            />
            <span>Settings</span>
          </button>

          <button
            type="button"
            onClick={onOpenCleanup}
            title="Audit strays, consolidate skills &amp; optimize tokens"
            className="inline-flex h-7 items-center gap-1.5 rounded-[6px] px-2 text-[12.5px] font-medium text-[var(--text-secondary)] transition-colors duration-100 hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
          >
            <ShieldCheck
              className="h-3.5 w-3.5 shrink-0"
              strokeWidth={1.75}
              aria-hidden="true"
            />
            <span>Audit</span>
            {cleanupIssueCount > 0 && (
              <span className="tabular-nums rounded-full bg-[var(--status-amber)]/18 px-1.5 py-[1px] text-[10.5px] font-semibold text-[var(--status-amber)]">
                {cleanupIssueCount}
              </span>
            )}
          </button>
        </div>

        <button
          type="button"
          onClick={onRefresh}
          disabled={isRefreshing}
          aria-label="Refresh workspace"
          title="Refresh workspace (⌘R)"
          className="inline-flex h-7 w-7 items-center justify-center rounded-[6px] text-[var(--text-secondary)] transition-colors duration-100 hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] disabled:opacity-50"
        >
          <RefreshCw
            className={['h-3.5 w-3.5', isRefreshing ? 'animate-spin' : ''].join(
              ' ',
            )}
            strokeWidth={1.75}
          />
        </button>
      </div>

      {/* Right-edge resize handle */}
      <div
        role="separator"
        aria-orientation="vertical"
        aria-label="Resize sidebar"
        onMouseDown={(e) => {
          e.preventDefault();
          resizeOriginRef.current = {
            startX: e.clientX,
            startWidth: sidebarWidth,
          };
          setResizing(true);
        }}
        className={[
          'absolute top-0 right-[-2px] bottom-0 z-20 w-[5px] cursor-col-resize transition-colors duration-150',
          resizing ? 'bg-[var(--accent)]/40' : 'hover:bg-[var(--accent)]/25',
        ].join(' ')}
      />

      <MacContextMenu
        open={contextMenu.open}
        x={contextMenu.x}
        y={contextMenu.y}
        items={contextMenuItems}
        onClose={() => setContextMenu((prev) => ({ ...prev, open: false }))}
      />
    </aside>
  );
}

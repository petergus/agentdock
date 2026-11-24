import { useMemo, useState, type RefObject } from 'react';
import {
  ChevronRight,
  Folder,
  Globe,
  Pin,
  RefreshCw,
  Search,
  Settings,
  X,
} from 'lucide-react';
import type { CategoryGroup, ProjectSummary } from '@/types';
import { formatTokensCompact, formatTokensExact } from '@/lib/format';

interface SidebarProps {
  globalTokens: number;
  categories: CategoryGroup[];
  selectedScopeId: string;
  searchQuery: string;
  onSearchChange: (q: string) => void;
  searchInputRef: RefObject<HTMLInputElement | null>;
  onSelectGlobal: () => void;
  onSelectProject: (project: ProjectSummary) => void;
  onOpenSettings: () => void;
  onRefresh: () => void;
  isRefreshing: boolean;
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
  onOpenSettings,
  onRefresh,
  isRefreshing,
}: SidebarProps) {
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>({});

  const filteredCategories = useMemo(() => {
    const q = searchQuery.trim().toLowerCase();
    if (!q) {
      return categories;
    }
    return categories
      .map((cat) => {
        const catMatch = cat.name.toLowerCase().includes(q);
        const matchingProjects = catMatch
          ? cat.projects
          : cat.projects.filter((p) => p.name.toLowerCase().includes(q));
        return { ...cat, projects: matchingProjects };
      })
      .filter((cat) => cat.projects.length > 0);
  }, [categories, searchQuery]);

  const toggleCategory = (name: string) => {
    setCollapsed((prev) => ({ ...prev, [name]: !prev[name] }));
  };

  return (
    <aside className="flex h-full w-[260px] shrink-0 flex-col border-r border-[var(--hairline)]">
      {/* Native macOS traffic-light safe drag header */}
      <div
        data-tauri-drag-region
        className="flex h-11 shrink-0 items-center pl-[78px] pr-3"
      >
        <span
          data-tauri-drag-region
          className="text-[12px] font-semibold tracking-[-0.01em] text-[var(--text-secondary)]"
        >
          AgentDock
        </span>
      </div>

      {/* Quick filter input (⌘K) */}
      <div className="px-3 pb-2.5">
        <div className="relative flex items-center">
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
            placeholder="Filter projects… (⌘K)"
            aria-label="Filter projects"
            className="h-7 w-full rounded-[6px] border border-[var(--hairline)] bg-black/[0.04] pl-8 pr-7 text-[12.5px] text-[var(--text-primary)] placeholder:text-[var(--text-tertiary)] dark:bg-white/[0.06]"
          />
          {searchQuery && (
            <button
              type="button"
              onClick={() => onSearchChange('')}
              aria-label="Clear filter"
              className="absolute right-1 inline-flex h-5 w-5 items-center justify-center rounded text-[var(--text-tertiary)] hover:text-[var(--text-primary)]"
            >
              <X className="h-3 w-3" strokeWidth={1.75} />
            </button>
          )}
        </div>
      </div>

      {/* Pinned Global Scope */}
      <div className="px-2 pb-2">
        <button
          type="button"
          onClick={onSelectGlobal}
          title={`Global baseline: ${formatTokensExact(globalTokens)} startup tokens`}
          className={[
            'flex h-8 w-full items-center justify-between rounded-[6px] px-2.5 text-left transition-opacity duration-150',
            selectedScopeId === 'global'
              ? 'bg-black/[0.09] font-medium text-[var(--text-primary)] dark:bg-white/[0.13]'
              : 'text-[var(--text-primary)] hover:bg-[var(--bg-hover)]',
          ].join(' ')}
        >
          <span className="flex min-w-0 items-center gap-2">
            <Globe
              className="h-3.5 w-3.5 shrink-0 text-[var(--accent)]"
              strokeWidth={1.75}
              aria-hidden="true"
            />
            <span className="truncate text-[13px]">Global</span>
          </span>
          <span className="tabular-nums text-[12px] text-[var(--text-secondary)]">
            {formatTokensCompact(globalTokens)}
          </span>
        </button>
      </div>

      {/* Divider + Projects Navigation Tree */}
      <div className="mx-3 border-t border-[var(--hairline)]" />

      <div className="flex-1 overflow-y-auto px-2 py-2.5">
        <div className="px-2 pb-1.5 text-[11px] font-semibold uppercase tracking-[0.05em] text-[var(--text-tertiary)]">
          Projects
        </div>

        {filteredCategories.length === 0 ? (
          <div className="px-2 py-4 text-[12px] text-[var(--text-tertiary)]">
            {categories.length === 0
              ? 'No categorized projects found. Check Projects Root in Settings.'
              : 'No matching projects.'}
          </div>
        ) : (
          <div className="space-y-1.5">
            {filteredCategories.map((cat) => {
              const isOpen = searchQuery.trim().length > 0 || !collapsed[cat.name];
              return (
                <div key={cat.name}>
                  <button
                    type="button"
                    aria-expanded={isOpen}
                    onClick={() => toggleCategory(cat.name)}
                    className="flex h-7 w-full items-center justify-between rounded-[5px] px-1.5 text-left text-[12px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)]"
                  >
                    <span className="flex min-w-0 items-center gap-1">
                      <ChevronRight
                        className={[
                          'h-3.5 w-3.5 shrink-0 text-[var(--text-tertiary)] transition-transform duration-150',
                          isOpen ? 'rotate-90' : 'rotate-0',
                        ].join(' ')}
                        strokeWidth={1.75}
                        aria-hidden="true"
                      />
                      <span className="truncate">{cat.name}</span>
                    </span>
                    <span className="tabular-nums text-[11px] text-[var(--text-tertiary)]">
                      {cat.projects.length}
                    </span>
                  </button>

                  {isOpen && (
                    <div className="mt-0.5 space-y-0.5 pl-3">
                      {cat.projects.map((proj) => {
                        const isSelected = selectedScopeId === proj.id;
                        const tooltip =
                          proj.projectExtraTokens > 0
                            ? `${proj.name}: ${formatTokensExact(proj.effectiveTokens)} total (${formatTokensExact(globalTokens)} global + ${formatTokensExact(proj.projectExtraTokens)} project)`
                            : `${proj.name}: ${formatTokensExact(proj.effectiveTokens)} total (Global baseline)`;

                        return (
                          <button
                            key={proj.id}
                            type="button"
                            onClick={() => onSelectProject(proj)}
                            title={tooltip}
                            className={[
                              'flex h-[30px] w-full items-center justify-between rounded-[6px] px-2 text-left transition-opacity duration-150',
                              isSelected
                                ? 'bg-black/[0.09] font-medium text-[var(--text-primary)] dark:bg-white/[0.13]'
                                : 'text-[var(--text-primary)] hover:bg-[var(--bg-hover)]',
                            ].join(' ')}
                          >
                            <span className="flex min-w-0 items-center gap-1.5">
                              {proj.isPinned ? (
                                <Pin
                                  className="h-3 w-3 shrink-0 text-[var(--accent)]"
                                  strokeWidth={1.75}
                                  aria-hidden="true"
                                />
                              ) : (
                                <Folder
                                  className="h-3.5 w-3.5 shrink-0 text-[var(--text-tertiary)]"
                                  strokeWidth={1.6}
                                  aria-hidden="true"
                                />
                              )}
                              <span className="truncate text-[12.5px]">{proj.name}</span>
                            </span>
                            <span
                              className={[
                                'tabular-nums ml-2 shrink-0 text-[11.5px]',
                                proj.projectExtraTokens > 0
                                  ? 'font-medium text-[var(--text-primary)]'
                                  : 'text-[var(--text-tertiary)]',
                              ].join(' ')}
                            >
                              {formatTokensCompact(proj.effectiveTokens)}
                            </span>
                          </button>
                        );
                      })}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </div>

      {/* Bottom Footer */}
      <div className="flex shrink-0 items-center justify-between border-t border-[var(--hairline)] px-2.5 py-2">
        <button
          type="button"
          onClick={onOpenSettings}
          title="Settings (⌘,)"
          className="inline-flex h-7 items-center gap-1.5 rounded-[6px] px-2 text-[12.5px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
        >
          <Settings className="h-3.5 w-3.5 shrink-0" strokeWidth={1.75} aria-hidden="true" />
          <span>Settings</span>
        </button>

        <button
          type="button"
          onClick={onRefresh}
          disabled={isRefreshing}
          aria-label="Refresh workspace"
          title="Refresh workspace (⌘R)"
          className="inline-flex h-7 w-7 items-center justify-center rounded-[6px] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] disabled:opacity-50"
        >
          <RefreshCw
            className={['h-3.5 w-3.5', isRefreshing ? 'animate-spin' : ''].join(' ')}
            strokeWidth={1.75}
          />
        </button>
      </div>
    </aside>
  );
}

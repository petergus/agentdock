import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { AlertCircle, X } from 'lucide-react';
import type {
  AgentFilter,
  AppConfig,
  CleanupAuditReport,
  ContextSet,
  GlobalSwitchTarget,
  McpServerItem,
  PluginItem,
  ProjectSummary,
  RuleItem,
  ScopeSnapshot,
  SkillItem,
  SupportDocItem,
  TagDefinition,
  ToggleScope,
  WorkspaceOverview,
} from '@/types';
import { api } from '@/lib/api';
import { computeFilteredBudget } from '@/lib/format';
import { Sidebar } from '@/components/Sidebar';
import { OverviewHeader } from '@/components/OverviewHeader';
import { GlobalQuickControls } from '@/components/GlobalQuickControls';
import { SetsSection } from '@/components/SetsSection';
import { EffectiveContext } from '@/components/EffectiveContext';
import { ProjectHealth } from '@/components/ProjectHealth';
import { SetPreviewModal, type SetPreviewRequest } from '@/components/SetPreviewModal';
import { SaveSetModal } from '@/components/SaveSetModal';
import { PluginsSection, pluginKey } from '@/components/PluginsSection';
import { SkillsSection } from '@/components/SkillsSection';
import { McpSection } from '@/components/McpSection';
import { RulesDocsSection } from '@/components/RulesDocsSection';
import { SettingsModal, type SettingsTab } from '@/components/SettingsModal';
import { CleanupModal } from '@/components/CleanupModal';
import { DeBloatModal } from '@/components/DeBloatModal';
import { RuleTriggerModal } from '@/components/RuleTriggerModal';
import { SupportDocsModal } from '@/components/SupportDocsModal';
import { AddMcpModal } from '@/components/AddMcpModal';
import { SkeletonLoader } from '@/components/ui/SkeletonLoader';

export function App() {
  const [overview, setOverview] = useState<WorkspaceOverview | null>(null);
  const [selectedProject, setSelectedProject] = useState<ProjectSummary | null>(
    null,
  );
  const [projectSnapshot, setProjectSnapshot] = useState<ScopeSnapshot | null>(
    null,
  );
  const [agentFilter, setAgentFilter] = useState<AgentFilter>('all');
  const [searchQuery, setSearchQuery] = useState('');
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [settingsTab, setSettingsTab] = useState<SettingsTab>('general');
  const [cleanupOpen, setCleanupOpen] = useState(false);
  const [cleanupReport, setCleanupReport] = useState<CleanupAuditReport | null>(
    null,
  );
  const [debloatState, setDebloatState] = useState<{
    open: boolean;
    selectedIds: string[];
    scope: ToggleScope;
    scopeLabel?: string;
    skillsOverride?: Array<Pick<SkillItem, 'id' | 'name' | 'upfrontTokens'>>;
  }>({ open: false, selectedIds: [], scope: { kind: 'global' } });
  const [editingRule, setEditingRule] = useState<RuleItem | null>(null);
  const [supportDocsOpen, setSupportDocsOpen] = useState(false);
  const [addMcpOpen, setAddMcpOpen] = useState(false);
  const [saveSetOpen, setSaveSetOpen] = useState(false);
  const [busySkillIds, setBusySkillIds] = useState<Record<string, boolean>>({});
  const [busyPluginKeys, setBusyPluginKeys] = useState<Record<string, boolean>>(
    {},
  );
  const [busyMcpNames, setBusyMcpNames] = useState<Record<string, boolean>>({});
  const [probingMcpNames, setProbingMcpNames] = useState<
    Record<string, boolean>
  >({});
  const [busyRuleIds, setBusyRuleIds] = useState<Record<string, boolean>>({});
  const [busyGlobalSwitch, setBusyGlobalSwitch] =
    useState<GlobalSwitchTarget | null>(null);
  const [globalSkippedUnmanaged, setGlobalSkippedUnmanaged] = useState<
    string[]
  >([]);
  const [setPreviewRequest, setSetPreviewRequest] = useState<SetPreviewRequest | null>(null);
  const [showEffectiveContext, setShowEffectiveContext] = useState(false);
  const [pinningPath, setPinningPath] = useState<string | null>(null);
  const [hidingPath, setHidingPath] = useState<string | null>(null);
  const [initialLoading, setInitialLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [projectLoading, setProjectLoading] = useState(false);
  const [errorBanner, setErrorBanner] = useState<string | null>(null);

  const searchInputRef = useRef<HTMLInputElement | null>(null);
  const refreshInFlightRef = useRef(false);
  const queuedWatcherRefreshRef = useRef(false);
  const selectionVersionRef = useRef(0);

  const refreshCleanupAudit = useCallback(async () => {
    try {
      const report = await api.runCleanupAudit();
      setCleanupReport(report);
    } catch (err) {
      console.warn('Failed to run cleanup audit:', err);
    }
  }, []);

  const refreshAll = useCallback(
    async (activeProject: ProjectSummary | null = selectedProject) => {
      refreshInFlightRef.current = true;
      const selectionVersion = selectionVersionRef.current;
      setRefreshing(true);
      setErrorBanner(null);
      try {
        const nextOverview = await api.scanWorkspace();
        setOverview(nextOverview);

        if (activeProject) {
          const stillExists = nextOverview.categories
            .flatMap((c) => c.projects)
            .find((p) => p.id === activeProject.id);

          if (stillExists) {
            const snap = await api.scanProject(
              stillExists.path,
              stillExists.category,
            );
            if (selectionVersion === selectionVersionRef.current) {
              setSelectedProject(stillExists);
              setProjectSnapshot(snap);
            }
          } else if (selectionVersion === selectionVersionRef.current) {
            setSelectedProject(null);
            setProjectSnapshot(null);
          }
        }
        void refreshCleanupAudit();
      } catch (err) {
        setErrorBanner(err instanceof Error ? err.message : String(err));
      } finally {
        refreshInFlightRef.current = false;
        setInitialLoading(false);
        setRefreshing(false);
      }
    },
    [selectedProject, refreshCleanupAudit],
  );

  const latestRefreshRef = useRef(refreshAll);
  latestRefreshRef.current = refreshAll;

  useEffect(() => {
    void refreshAll(null);
    // Initial workspace load runs once on mount
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Track macOS window focus so sidebar selection pills dim when inactive
  useEffect(() => {
    const syncFocus = () => {
      document.documentElement.dataset.windowFocused = document.hasFocus()
        ? 'true'
        : 'false';
    };
    const onFocus = () => {
      document.documentElement.dataset.windowFocused = 'true';
    };
    const onBlur = () => {
      document.documentElement.dataset.windowFocused = 'false';
    };
    syncFocus();
    window.addEventListener('focus', onFocus);
    window.addEventListener('blur', onBlur);
    return () => {
      window.removeEventListener('focus', onFocus);
      window.removeEventListener('blur', onBlur);
    };
  }, []);

  // Keep native FSEvents watcher aligned with the active Global / Project scope
  useEffect(() => {
    void api
      .setWatchedProject(selectedProject?.path ?? null)
      .catch((err) => console.warn('Failed to update file watcher:', err));
  }, [selectedProject?.path]);

  const handleSelectGlobal = useCallback(() => {
    selectionVersionRef.current += 1;
    setProjectLoading(false);
    setSelectedProject(null);
    setProjectSnapshot(null);
  }, []);

  const handleSelectProject = useCallback(async (project: ProjectSummary) => {
    const selectionVersion = ++selectionVersionRef.current;
    setSelectedProject(project);
    setProjectLoading(true);
    setErrorBanner(null);
    try {
      const snap = await api.scanProject(project.path, project.category);
      if (selectionVersion === selectionVersionRef.current) setProjectSnapshot(snap);
    } catch (err) {
      if (selectionVersion === selectionVersionRef.current) {
        setErrorBanner(err instanceof Error ? err.message : String(err));
      }
    } finally {
      if (selectionVersion === selectionVersionRef.current) setProjectLoading(false);
    }
  }, []);

  const navigableProjects = useMemo(() => {
    if (!overview) {
      return [];
    }
    const seen = new Set<string>();
    const list: ProjectSummary[] = [];
    for (const cat of overview.categories) {
      for (const proj of cat.projects) {
        if (!proj.isHidden && !seen.has(proj.id)) {
          seen.add(proj.id);
          list.push(proj);
        }
      }
    }
    return list;
  }, [overview]);

  // Listen to native macOS menu events, live file watcher notifications, and shortcuts
  useEffect(() => {
    const focusSearch = () => {
      searchInputRef.current?.focus();
      searchInputRef.current?.select();
    };

    const scheduleWatcherRefresh = () => {
      if (refreshInFlightRef.current) {
        queuedWatcherRefreshRef.current = true;
        return;
      }
      const run = async () => {
        await latestRefreshRef.current();
        if (queuedWatcherRefreshRef.current) {
          queuedWatcherRefreshRef.current = false;
          await latestRefreshRef.current();
        }
      };
      void run();
    };

    const unlisteners = [
      listen('menu://settings', () => {
        setSettingsTab('general');
        setSettingsOpen(true);
      }),
      listen('menu://refresh', () => void refreshAll()),
      listen('menu://search', focusSearch),
      listen('menu://global', () => {
        if (!document.querySelector('dialog[open]')) handleSelectGlobal();
      }),
      listen('workspace://changed', scheduleWatcherRefresh),
    ];

    const isEditableTarget = (target: EventTarget | null) => {
      if (!(target instanceof HTMLElement)) {
        return false;
      }
      const tag = target.tagName;
      return (
        tag === 'INPUT' ||
        tag === 'TEXTAREA' ||
        tag === 'SELECT' ||
        target.isContentEditable
      );
    };

    const onKeyDown = (e: KeyboardEvent) => {
      if (e.metaKey && !e.ctrlKey && !e.altKey && !document.querySelector('dialog[open]')) {
        const key = e.key.toLowerCase();
        if (key === 'k') {
          e.preventDefault();
          focusSearch();
        } else if (key === ',') {
          e.preventDefault();
          setSettingsTab('general');
          setSettingsOpen(true);
        } else if (key === 'r') {
          e.preventDefault();
          void refreshAll();
        } else if (key === '1') {
          e.preventDefault();
          handleSelectGlobal();
        }
        return;
      }

      if (
        !e.metaKey &&
        !e.ctrlKey &&
        !e.altKey &&
        (e.key === 'ArrowUp' || e.key === 'ArrowDown') &&
        !isEditableTarget(e.target) &&
        !document.querySelector('dialog[open]')
      ) {
        if (navigableProjects.length === 0) {
          return;
        }
        e.preventDefault();
        const currentIdx = selectedProject
          ? navigableProjects.findIndex((p) => p.id === selectedProject.id)
          : -1;
        if (e.key === 'ArrowDown') {
          if (currentIdx === -1) {
            void handleSelectProject(navigableProjects[0]);
          } else if (currentIdx < navigableProjects.length - 1) {
            void handleSelectProject(navigableProjects[currentIdx + 1]);
          }
        } else {
          if (currentIdx === 0) {
            handleSelectGlobal();
          } else if (currentIdx > 0) {
            void handleSelectProject(navigableProjects[currentIdx - 1]);
          }
        }
      }
    };

    window.addEventListener('keydown', onKeyDown);
    return () => {
      window.removeEventListener('keydown', onKeyDown);
      for (const promise of unlisteners) {
        void promise.then((unlisten) => unlisten());
      }
    };
  }, [
    refreshAll,
    handleSelectGlobal,
    handleSelectProject,
    navigableProjects,
    selectedProject,
  ]);

  const handleSaveConfig = async (nextConfig: AppConfig) => {
    await api.saveConfig(nextConfig);
    await refreshAll(selectedProject);
  };

  const handleRevealPath = async (path: string) => {
    try {
      await api.revealInFinder(path);
    } catch (err) {
      setErrorBanner(err instanceof Error ? err.message : String(err));
    }
  };

  const handleOpenFile = async (path: string) => {
    try {
      await api.openInDefaultApp(path);
    } catch (err) {
      setErrorBanner(err instanceof Error ? err.message : String(err));
    }
  };

  const currentToggleScope: ToggleScope = useMemo(
    () =>
      selectedProject
        ? { kind: 'project', projectPath: selectedProject.path }
        : { kind: 'global' },
    [selectedProject],
  );

  const handleToggleSkill = async (skill: SkillItem, next: boolean) => {
    setBusySkillIds((prev) => ({ ...prev, [skill.id]: true }));
    setErrorBanner(null);
    try {
      const outcome = await api.toggleSkill(
        currentToggleScope,
        skill.id,
        skill.skillDirPath,
        next,
      );
      if (outcome.status === 'requiresMoveToLibrary') {
        setDebloatState({
          open: true,
          selectedIds: [outcome.skillId],
          scope: currentToggleScope,
          scopeLabel: activeSnapshot?.title,
        });
      } else {
        await refreshAll(selectedProject);
      }
    } catch (err) {
      setErrorBanner(err instanceof Error ? err.message : String(err));
    } finally {
      setBusySkillIds((prev) => ({ ...prev, [skill.id]: false }));
    }
  };

  const handleUnlinkBrokenSkill = async (skill: SkillItem) => {
    setBusySkillIds((prev) => ({ ...prev, [skill.id]: true }));
    setErrorBanner(null);
    try {
      const targets =
        skill.installedPaths.length > 0
          ? skill.installedPaths.map((p) => p.path)
          : [skill.skillDirPath];
      for (const targetPath of targets) {
        await api.unlinkBrokenSymlink(
          targetPath,
          selectedProject?.path ?? null,
        );
      }
      await refreshAll(selectedProject);
    } catch (err) {
      setErrorBanner(err instanceof Error ? err.message : String(err));
    } finally {
      setBusySkillIds((prev) => ({ ...prev, [skill.id]: false }));
    }
  };

  const handleToggleLibraryRule = async (rule: RuleItem, next: boolean) => {
    setBusyRuleIds((prev) => ({ ...prev, [rule.id]: true }));
    setErrorBanner(null);
    try {
      const outcome = await api.toggleLibraryRule(
        currentToggleScope,
        rule.filename,
        rule.canonicalPath,
        next,
      );
      if (outcome.status === 'requiresMoveToLibrary') {
        setErrorBanner(
          `Cannot unlink real rule file(s): ${outcome.realPaths.join(', ')}`,
        );
      } else {
        await refreshAll(selectedProject);
      }
    } catch (err) {
      setErrorBanner(err instanceof Error ? err.message : String(err));
    } finally {
      setBusyRuleIds((prev) => ({ ...prev, [rule.id]: false }));
    }
  };

  const handleUnlinkBrokenRule = async (rule: RuleItem) => {
    setBusyRuleIds((prev) => ({ ...prev, [rule.id]: true }));
    setErrorBanner(null);
    try {
      await api.unlinkBrokenSymlink(rule.path, selectedProject?.path ?? null);
      await refreshAll(selectedProject);
    } catch (err) {
      setErrorBanner(err instanceof Error ? err.message : String(err));
    } finally {
      setBusyRuleIds((prev) => ({ ...prev, [rule.id]: false }));
    }
  };

  const handleUnlinkSupportDoc = async (doc: SupportDocItem) => {
    if (!selectedProject) {
      return;
    }
    setErrorBanner(null);
    try {
      await api.unlinkSupportDoc(selectedProject.path, doc.path, doc.mode);
      await refreshAll(selectedProject);
    } catch (err) {
      setErrorBanner(err instanceof Error ? err.message : String(err));
    }
  };

  const handleToggleMcp = async (server: McpServerItem, next: boolean) => {
    setBusyMcpNames((prev) => ({ ...prev, [server.name]: true }));
    setErrorBanner(null);
    try {
      await api.toggleMcpServer(currentToggleScope, server.name, next);
      await refreshAll(selectedProject);
    } catch (err) {
      setErrorBanner(err instanceof Error ? err.message : String(err));
    } finally {
      setBusyMcpNames((prev) => ({ ...prev, [server.name]: false }));
    }
  };

  const handleTogglePlugin = async (plugin: PluginItem, next: boolean) => {
    const key = pluginKey(plugin);
    setBusyPluginKeys((prev) => ({ ...prev, [key]: true }));
    setErrorBanner(null);
    try {
      await api.togglePlugin(plugin.agent, plugin.id, next);
      await refreshAll(selectedProject);
    } catch (err) {
      setErrorBanner(err instanceof Error ? err.message : String(err));
    } finally {
      setBusyPluginKeys((prev) => ({ ...prev, [key]: false }));
    }
  };

  const handleProbeMcp = async (server: McpServerItem) => {
    setProbingMcpNames((prev) => ({ ...prev, [server.name]: true }));
    setErrorBanner(null);
    try {
      await api.probeMcpServer(server.name);
      await refreshAll(selectedProject);
    } catch (err) {
      setErrorBanner(err instanceof Error ? err.message : String(err));
    } finally {
      setProbingMcpNames((prev) => ({ ...prev, [server.name]: false }));
    }
  };

  const handleTogglePinProject = async (projectPath: string) => {
    setPinningPath(projectPath);
    setErrorBanner(null);
    try {
      await api.togglePinnedProject(projectPath);
      await refreshAll(selectedProject);
    } catch (err) {
      setErrorBanner(err instanceof Error ? err.message : String(err));
    } finally {
      setPinningPath(null);
    }
  };

  const handleToggleHideProject = async (projectPath: string) => {
    setHidingPath(projectPath);
    setErrorBanner(null);
    try {
      await api.toggleHiddenProject(projectPath);
      await refreshAll(selectedProject);
    } catch (err) {
      setErrorBanner(err instanceof Error ? err.message : String(err));
    } finally {
      setHidingPath(null);
    }
  };

  const handleGlobalSwitch = async (
    target: GlobalSwitchTarget,
    enable: boolean,
  ) => {
    setBusyGlobalSwitch(target);
    setErrorBanner(null);
    try {
      const result = await api.setGlobalSwitch(target, enable);
      setGlobalSkippedUnmanaged(result.skippedUnmanaged);
      if (result.warnings.length > 0 && result.skippedUnmanaged.length === 0) {
        setErrorBanner(result.warnings.join(' · '));
      }
      await refreshAll(selectedProject);
    } catch (err) {
      setErrorBanner(err instanceof Error ? err.message : String(err));
    } finally {
      setBusyGlobalSwitch(null);
    }
  };

  const handleApplyProjectSet = async (revision: string) => {
    if (!setPreviewRequest) return;
    const { project, setId, operation } = setPreviewRequest;
    const result = await api.toggleProjectSet(project.path, setId, operation, revision);
    await refreshAll(project);
    setSetPreviewRequest(null);
    if (result.warnings.length > 0) {
      setErrorBanner(result.warnings.join(' · '));
    }
  };

  const handleSaveNewSet = async (newSet: ContextSet) => {
    await api.upsertSet(newSet);
    await refreshAll(selectedProject);
    if (selectedProject) {
      setSetPreviewRequest({
        project: selectedProject,
        setId: newSet.id,
        setName: newSet.name,
        operation: 'apply',
      });
    }
  };

  const handleSaveProjectTags = async (
    manualTagIds: string[],
    mutedAutoTagIds: string[],
  ) => {
    if (!selectedProject) {
      return;
    }
    setErrorBanner(null);
    try {
      await api.setProjectTags(
        selectedProject.path,
        manualTagIds,
        mutedAutoTagIds,
      );
      await refreshAll(selectedProject);
    } catch (err) {
      setErrorBanner(err instanceof Error ? err.message : String(err));
    }
  };

  const handleCreateAndAttachTag = async (tag: TagDefinition) => {
    if (!selectedProject || !overview) {
      return;
    }
    setErrorBanner(null);
    try {
      await api.upsertTagDefinition(tag);
      const currentManual =
        overview.config.projectManualTags[selectedProject.path] ?? [];
      await api.setProjectTags(
        selectedProject.path,
        [...currentManual, tag.id],
        overview.config.projectMutedAutoTags[selectedProject.path] ?? [],
      );
      await refreshAll(selectedProject);
    } catch (err) {
      setErrorBanner(err instanceof Error ? err.message : String(err));
    }
  };

  const openSettingsAt = (tab: SettingsTab) => {
    setSettingsTab(tab);
    setSettingsOpen(true);
  };

  const handleRegisterLibrarySource = async (path: string) => {
    if (!overview) {
      return;
    }
    const cleanPath = path.trim().replace(/\/+$/, '');
    const alreadyExists = overview.config.librarySources.some(
      (s) => s.path.trim().replace(/\/+$/, '') === cleanPath,
    );
    if (alreadyExists) {
      return;
    }
    const parts = cleanPath.split('/').filter(Boolean);
    const label = parts.pop() ?? 'Library';
    await api.saveConfig({
      ...overview.config,
      librarySources: [
        ...overview.config.librarySources,
        {
          id: `lib-${Date.now()}`,
          label,
          path: cleanPath,
          depth: 2,
        },
      ],
    });
  };

  const activeSnapshot =
    selectedProject && projectSnapshot?.path === selectedProject.path
      ? projectSnapshot
      : overview?.globalSnapshot ?? null;

  const debloatCandidateSkills = useMemo(() => {
    if (!activeSnapshot) {
      return [];
    }
    return activeSnapshot.skills.filter((s) => {
      if (activeSnapshot.scopeType === 'global') {
        return s.activeGlobally && s.isUnmanagedGlobal;
      }
      return s.source.kind === 'projectOnly';
    });
  }, [activeSnapshot]);

  const filteredBudget = useMemo(() => {
    if (!activeSnapshot) {
      return null;
    }
    return computeFilteredBudget(activeSnapshot, agentFilter);
  }, [activeSnapshot, agentFilter]);

  if (initialLoading || !overview || !activeSnapshot || !filteredBudget) {
    return (
      <div className="h-full w-full">
        {errorBanner ? (
          <div className="flex h-full items-center justify-center bg-[var(--bg-content)] p-8">
            <div className="max-w-md space-y-3 rounded-[10px] border border-[var(--hairline)] bg-[var(--bg-elevated)] p-5">
              <div className="flex items-center gap-2 font-medium text-[var(--status-red)]">
                <AlertCircle className="h-4 w-4 shrink-0" />
                <span>Failed to scan workspace</span>
              </div>
              <p className="text-[12.5px] text-[var(--text-secondary)]">
                {errorBanner}
              </p>
              <button
                type="button"
                onClick={() => void refreshAll(null)}
                className="h-8 rounded-[6px] bg-[var(--accent)] px-3.5 text-[12.5px] font-medium text-[var(--accent-text)]"
              >
                Retry Scan
              </button>
            </div>
          </div>
        ) : (
          <SkeletonLoader />
        )}
      </div>
    );
  }

  return (
    <div className="flex h-full w-full overflow-hidden">
      <Sidebar
        globalTokens={overview.globalTokens}
        categories={overview.categories}
        selectedScopeId={selectedProject ? selectedProject.id : 'global'}
        searchQuery={searchQuery}
        onSearchChange={setSearchQuery}
        searchInputRef={searchInputRef}
        onSelectGlobal={handleSelectGlobal}
        onSelectProject={handleSelectProject}
        onTogglePinProject={(projectPath) =>
          void handleTogglePinProject(projectPath)
        }
        pinningPath={pinningPath}
        onToggleHideProject={(projectPath) =>
          void handleToggleHideProject(projectPath)
        }
        hidingPath={hidingPath}
        onRevealPath={handleRevealPath}
        onOpenTagSettings={() => openSettingsAt('tags')}
        onOpenSettings={() => openSettingsAt('general')}
        onOpenCleanup={() => setCleanupOpen(true)}
        cleanupIssueCount={cleanupReport?.totalIssues ?? 0}
        onRefresh={() => void refreshAll()}
        isRefreshing={refreshing}
      />

      <main className="flex min-w-0 flex-1 flex-col overflow-y-auto overscroll-y-contain bg-[var(--bg-canvas)]">
        <OverviewHeader
          snapshot={activeSnapshot}
          budget={filteredBudget}
          showEffectiveContext={showEffectiveContext}
          onChangeContextView={setShowEffectiveContext}
          activeAgents={overview.config.activeAgents}
          agentFilter={agentFilter}
          onAgentFilterChange={setAgentFilter}
          onRevealPath={handleRevealPath}
          selectedProject={selectedProject}
          tagDefinitions={overview.config.tagDefinitions}
          manualTagIds={
            selectedProject
              ? overview.config.projectManualTags[selectedProject.path] ?? []
              : []
          }
          mutedAutoTagIds={
            selectedProject
              ? overview.config.projectMutedAutoTags[selectedProject.path] ?? []
              : []
          }
          isPinned={selectedProject?.isPinned ?? false}
          isPinBusy={
            selectedProject !== null && pinningPath === selectedProject.path
          }
          onTogglePin={
            selectedProject
              ? () => void handleTogglePinProject(selectedProject.path)
              : undefined
          }
          isHidden={selectedProject?.isHidden ?? false}
          isHideBusy={
            selectedProject !== null && hidingPath === selectedProject.path
          }
          onToggleHide={
            selectedProject
              ? () => void handleToggleHideProject(selectedProject.path)
              : undefined
          }
          onSaveProjectTags={handleSaveProjectTags}
          onCreateAndAttachTag={handleCreateAndAttachTag}
          onOpenTagSettings={() => openSettingsAt('tags')}
        />

        <div className="mx-auto w-full max-w-[960px] space-y-7 px-6 py-5">
          {errorBanner && (
            <div className="flex items-center justify-between gap-3 rounded-[8px] border border-[var(--status-red)]/30 bg-[var(--status-red)]/10 px-3.5 py-2 text-[12.5px] text-[var(--status-red)]">
              <span className="flex items-center gap-2">
                <AlertCircle className="h-4 w-4 shrink-0" />
                <span>{errorBanner}</span>
              </span>
              <button
                type="button"
                aria-label="Dismiss error"
                onClick={() => setErrorBanner(null)}
                className="inline-flex h-6 w-6 items-center justify-center rounded hover:bg-black/10 dark:hover:bg-white/10"
              >
                <X className="h-3.5 w-3.5" />
              </button>
            </div>
          )}

          {(activeSnapshot.scopeType === 'global' || showEffectiveContext) && activeSnapshot.scanWarnings.length > 0 && (
            <div className="rounded-[8px] border border-[var(--hairline)] bg-[var(--bg-elevated)] px-3.5 py-2.5 text-[12px] text-[var(--text-secondary)]">
              {activeSnapshot.scanWarnings.map((warn) => (
                <div key={warn}>{warn}</div>
              ))}
            </div>
          )}

          {projectLoading ? (
            <div className="space-y-2.5 py-4" aria-busy="true">
              <div className="h-4 w-28 rounded bg-[var(--bg-subtle)]" />
              <div className="h-9 w-full rounded bg-[var(--bg-subtle)]" />
              <div className="h-9 w-full rounded bg-[var(--bg-subtle)]" />
            </div>
          ) : showEffectiveContext ? (
            <EffectiveContext
              snapshot={activeSnapshot}
              activeAgents={overview.config.activeAgents}
              agentFilter={agentFilter}
            />
          ) : (
            <>
              {activeSnapshot.scopeType === 'global' && (
                <GlobalQuickControls
                  globalPause={overview.config.globalPause}
                  activeSkillsCount={
                    overview.globalSnapshot.skills.filter(
                      (s) => s.activeGlobally,
                    ).length
                  }
                  activeMcpCount={
                    overview.globalSnapshot.mcpServers.filter(
                      (m) => m.activeGlobally,
                    ).length
                  }
                  busyTarget={busyGlobalSwitch}
                  skippedUnmanaged={globalSkippedUnmanaged}
                  onToggleSwitch={(target, next) =>
                    void handleGlobalSwitch(target, next)
                  }
                  onMoveUnmanagedToLibrary={(ids) =>
                    setDebloatState({
                      open: true,
                      selectedIds: ids,
                      scope: { kind: 'global' },
                      scopeLabel: 'Global',
                      skillsOverride: overview.globalSnapshot.skills.filter(
                        (s) => s.activeGlobally && s.isUnmanagedGlobal,
                      ),
                    })
                  }
                  onDismissNotice={() => setGlobalSkippedUnmanaged([])}
                />
              )}

              {activeSnapshot.scopeType === 'project' && (
                <ProjectHealth
                  key={activeSnapshot.scopeId}
                  snapshot={activeSnapshot}
                  agentFilter={agentFilter}
                  onOpenCleanup={() => setCleanupOpen(true)}
                />
              )}

              {activeSnapshot.scopeType === 'project' && selectedProject && (
                <SetsSection
                  sets={overview.config.sets}
                  activeSetIds={selectedProject.activeSetIds}
                  projectSkills={activeSnapshot.skills}
                  projectMcps={activeSnapshot.mcpServers}
                  projectRules={activeSnapshot.rules}
                  onPreviewSet={(set, operation) => setSetPreviewRequest({
                    project: selectedProject, setId: set.id, setName: set.name, operation,
                  })}
                  onOpenSaveSet={() => setSaveSetOpen(true)}
                  onManageSets={() => openSettingsAt('sets')}
                />
              )}

              <PluginsSection
                scopeType={activeSnapshot.scopeType}
                plugins={activeSnapshot.plugins}
                agentFilter={agentFilter}
                busyPluginKeys={busyPluginKeys}
                onTogglePlugin={(plugin, next) =>
                  void handleTogglePlugin(plugin, next)
                }
                onRevealPath={handleRevealPath}
              />

              <SkillsSection
                scopeType={activeSnapshot.scopeType}
                skills={activeSnapshot.skills}
                unmanagedGlobalSkillsCount={
                  activeSnapshot.unmanagedGlobalSkillsCount
                }
                agentFilter={agentFilter}
                busySkillIds={busySkillIds}
                onOpenFile={handleOpenFile}
                onToggleSkill={(skill, next) => void handleToggleSkill(skill, next)}
                onUnlinkBrokenSkill={(skill) => void handleUnlinkBrokenSkill(skill)}
                onOpenDeBloat={(ids) =>
                  setDebloatState({
                    open: true,
                    selectedIds: ids,
                    scope: currentToggleScope,
                    scopeLabel: activeSnapshot.title,
                  })
                }
              />

              <McpSection
                scopeType={activeSnapshot.scopeType}
                servers={activeSnapshot.mcpServers}
                agentFilter={agentFilter}
                busyServerNames={busyMcpNames}
                probingServerNames={probingMcpNames}
                onToggleMcp={(server, next) => void handleToggleMcp(server, next)}
                onProbeMcp={(server) => void handleProbeMcp(server)}
                onOpenAddMcp={() => setAddMcpOpen(true)}
              />

              <RulesDocsSection
                scopeType={activeSnapshot.scopeType}
                rules={activeSnapshot.rules}
                supportDocs={activeSnapshot.supportDocs}
                agentFilter={agentFilter}
                busyRuleIds={busyRuleIds}
                onOpenFile={handleOpenFile}
                onRevealPath={handleRevealPath}
                onToggleLibraryRule={(rule, next) =>
                  void handleToggleLibraryRule(rule, next)
                }
                onUnlinkBrokenRule={(rule) => void handleUnlinkBrokenRule(rule)}
                onEditRuleTrigger={(rule) => setEditingRule(rule)}
                onOpenSupportDocsModal={() => setSupportDocsOpen(true)}
                onUnlinkSupportDoc={(doc) => void handleUnlinkSupportDoc(doc)}
              />
            </>
          )}
        </div>
      </main>

      <SettingsModal
        open={settingsOpen}
        initialTab={settingsTab}
        config={overview.config}
        discoveredAutoFolders={overview.discoveredAutoFolders}
        availableSkills={overview.globalSnapshot.skills}
        availableMcps={overview.globalSnapshot.mcpServers}
        availableRules={overview.globalSnapshot.rules}
        onClose={() => setSettingsOpen(false)}
        onSave={handleSaveConfig}
      />

      <CleanupModal
        open={cleanupOpen}
        report={cleanupReport}
        onClose={() => setCleanupOpen(false)}
        onRefreshAudit={refreshCleanupAudit}
        onWorkspaceChanged={() => refreshAll(selectedProject)}
        onOpenConsolidate={(req) =>
          setDebloatState({
            open: true,
            selectedIds: req.skillIds,
            scope: req.scope,
            scopeLabel: req.scopeLabel,
            skillsOverride: req.skills,
          })
        }
        onRevealPath={(path) => void handleRevealPath(path)}
        onOpenFile={(path) => void handleOpenFile(path)}
      />

      <DeBloatModal
        open={debloatState.open}
        skills={
          debloatState.skillsOverride ??
          (debloatCandidateSkills.length > 0
            ? debloatCandidateSkills
            : activeSnapshot.skills.filter((s) =>
                debloatState.selectedIds.includes(s.id),
              ))
        }
        initialSelectedIds={debloatState.selectedIds}
        librarySources={overview.config.librarySources}
        scope={debloatState.scope}
        scopeLabel={debloatState.scopeLabel}
        onClose={() =>
          setDebloatState({
            open: false,
            selectedIds: [],
            scope: { kind: 'global' },
          })
        }
        onRegisterLibrarySource={handleRegisterLibrarySource}
        onSuccess={() => refreshAll(selectedProject)}
      />

      <RuleTriggerModal
        rule={editingRule}
        onClose={() => setEditingRule(null)}
        onSuccess={() => refreshAll(selectedProject)}
      />

      <AddMcpModal
        open={addMcpOpen}
        scope={currentToggleScope}
        scopeLabel={selectedProject ? selectedProject.name : 'Global'}
        onClose={() => setAddMcpOpen(false)}
        onSuccess={() => refreshAll(selectedProject)}
      />

      {selectedProject && (
        <SupportDocsModal
          open={supportDocsOpen}
          projectPath={selectedProject.path}
          supportDocsRoot={overview.config.supportDocsRoot}
          onClose={() => setSupportDocsOpen(false)}
          onSuccess={() => refreshAll(selectedProject)}
        />
      )}

      {setPreviewRequest && (
        <SetPreviewModal
          request={setPreviewRequest}
          onClose={() => setSetPreviewRequest(null)}
          onApply={handleApplyProjectSet}
        />
      )}

      {selectedProject && (
        <SaveSetModal
          open={saveSetOpen}
          projectName={selectedProject.name}
          skills={activeSnapshot.skills}
          mcpServers={activeSnapshot.mcpServers}
          rules={activeSnapshot.rules}
          onClose={() => setSaveSetOpen(false)}
          onSaveSet={handleSaveNewSet}
        />
      )}
    </div>
  );
}
export default App;

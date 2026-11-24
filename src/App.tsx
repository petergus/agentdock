import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { AlertCircle, X } from 'lucide-react';
import type {
  AgentFilter,
  AppConfig,
  McpServerItem,
  ProjectSummary,
  RuleItem,
  ScopeSnapshot,
  SkillItem,
  SupportDocItem,
  ToggleScope,
  WorkspaceOverview,
} from '@/types';
import { api } from '@/lib/api';
import { computeFilteredBudget } from '@/lib/format';
import { Sidebar } from '@/components/Sidebar';
import { OverviewHeader } from '@/components/OverviewHeader';
import { SkillsSection } from '@/components/SkillsSection';
import { McpSection } from '@/components/McpSection';
import { RulesDocsSection } from '@/components/RulesDocsSection';
import { SettingsModal } from '@/components/SettingsModal';
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
  const [debloatState, setDebloatState] = useState<{
    open: boolean;
    selectedIds: string[];
  }>({ open: false, selectedIds: [] });
  const [editingRule, setEditingRule] = useState<RuleItem | null>(null);
  const [supportDocsOpen, setSupportDocsOpen] = useState(false);
  const [addMcpOpen, setAddMcpOpen] = useState(false);
  const [busySkillIds, setBusySkillIds] = useState<Record<string, boolean>>({});
  const [busyMcpNames, setBusyMcpNames] = useState<Record<string, boolean>>({});
  const [probingMcpNames, setProbingMcpNames] = useState<
    Record<string, boolean>
  >({});
  const [busyRuleIds, setBusyRuleIds] = useState<Record<string, boolean>>({});
  const [initialLoading, setInitialLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [projectLoading, setProjectLoading] = useState(false);
  const [errorBanner, setErrorBanner] = useState<string | null>(null);

  const searchInputRef = useRef<HTMLInputElement | null>(null);
  const refreshInFlightRef = useRef(false);
  const queuedWatcherRefreshRef = useRef(false);

  const refreshAll = useCallback(
    async (activeProject: ProjectSummary | null = selectedProject) => {
      refreshInFlightRef.current = true;
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
            setSelectedProject(stillExists);
            setProjectSnapshot(snap);
          } else {
            setSelectedProject(null);
            setProjectSnapshot(null);
          }
        }
      } catch (err) {
        setErrorBanner(err instanceof Error ? err.message : String(err));
      } finally {
        refreshInFlightRef.current = false;
        setInitialLoading(false);
        setRefreshing(false);
      }
    },
    [selectedProject],
  );

  const latestRefreshRef = useRef(refreshAll);
  latestRefreshRef.current = refreshAll;

  useEffect(() => {
    void refreshAll(null);
    // Initial workspace load runs once on mount
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Keep native FSEvents watcher aligned with the active Global / Project scope
  useEffect(() => {
    void api
      .setWatchedProject(selectedProject?.path ?? null)
      .catch((err) => console.warn('Failed to update file watcher:', err));
  }, [selectedProject?.path]);

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
      listen('menu://settings', () => setSettingsOpen(true)),
      listen('menu://refresh', () => void refreshAll()),
      listen('menu://search', focusSearch),
      listen('workspace://changed', scheduleWatcherRefresh),
    ];

    const onKeyDown = (e: KeyboardEvent) => {
      if (!e.metaKey || e.ctrlKey || e.altKey) {
        return;
      }
      const key = e.key.toLowerCase();
      if (key === 'k') {
        e.preventDefault();
        focusSearch();
      } else if (key === ',') {
        e.preventDefault();
        setSettingsOpen(true);
      } else if (key === 'r') {
        e.preventDefault();
        void refreshAll();
      }
    };

    window.addEventListener('keydown', onKeyDown);
    return () => {
      window.removeEventListener('keydown', onKeyDown);
      for (const promise of unlisteners) {
        void promise.then((unlisten) => unlisten());
      }
    };
  }, [refreshAll]);

  const handleSelectGlobal = () => {
    setSelectedProject(null);
    setProjectSnapshot(null);
  };

  const handleSelectProject = async (project: ProjectSummary) => {
    setSelectedProject(project);
    setProjectLoading(true);
    setErrorBanner(null);
    try {
      const snap = await api.scanProject(project.path, project.category);
      setProjectSnapshot(snap);
    } catch (err) {
      setErrorBanner(err instanceof Error ? err.message : String(err));
    } finally {
      setProjectLoading(false);
    }
  };

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
        setDebloatState({ open: true, selectedIds: [outcome.skillId] });
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

  const activeSnapshot =
    selectedProject && projectSnapshot
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
        onOpenSettings={() => setSettingsOpen(true)}
        onRefresh={() => void refreshAll()}
        isRefreshing={refreshing}
      />

      <main className="flex min-w-0 flex-1 flex-col overflow-y-auto bg-[var(--bg-content)]">
        <OverviewHeader
          snapshot={activeSnapshot}
          budget={filteredBudget}
          activeAgents={overview.config.activeAgents}
          agentFilter={agentFilter}
          onAgentFilterChange={setAgentFilter}
          onRevealPath={handleRevealPath}
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

          {activeSnapshot.scanWarnings.length > 0 && (
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
          ) : (
            <>
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
                  setDebloatState({ open: true, selectedIds: ids })
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
        config={overview.config}
        onClose={() => setSettingsOpen(false)}
        onSave={handleSaveConfig}
      />

      <DeBloatModal
        open={debloatState.open}
        skills={
          debloatCandidateSkills.length > 0
            ? debloatCandidateSkills
            : activeSnapshot.skills.filter((s) =>
                debloatState.selectedIds.includes(s.id),
              )
        }
        initialSelectedIds={debloatState.selectedIds}
        librarySources={overview.config.librarySources}
        scope={currentToggleScope}
        onClose={() => setDebloatState({ open: false, selectedIds: [] })}
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
    </div>
  );
}
export default App;

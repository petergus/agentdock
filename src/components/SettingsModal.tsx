import { useEffect, useMemo, useRef, useState } from 'react';
import {
  Boxes,
  ChevronRight,
  FolderOpen,
  Plus,
  Settings2,
  Tag,
  Trash2,
  X,
} from 'lucide-react';
import type {
  AppConfig,
  ContextSet,
  LibrarySource,
  McpServerItem,
  ProjectRoot,
  ProjectScanMode,
  RuleItem,
  SkillItem,
  TagColor,
  TagDefinition,
} from '@/types';
import { api } from '@/lib/api';
import { TAG_COLORS, tagColorHex } from '@/lib/format';
import { MacSwitch } from '@/components/ui/MacSwitch';
import { MacSelect } from '@/components/ui/MacSelect';

export type SettingsTab = 'general' | 'tags' | 'sets';

interface SettingsModalProps {
  open: boolean;
  initialTab?: SettingsTab;
  config: AppConfig;
  discoveredAutoFolders: string[];
  availableSkills: SkillItem[];
  availableMcps: McpServerItem[];
  availableRules: RuleItem[];
  onClose: () => void;
  onSave: (nextConfig: AppConfig) => Promise<void>;
}

const PROJECT_DEPTH_OPTIONS: Array<{
  value: `${ProjectScanMode}:${number}`;
  label: string;
}> = [
  { value: 'exactDepth:1', label: 'Depth 1 (Direct)' },
  { value: 'exactDepth:2', label: 'Depth 2 (Category / Project)' },
  { value: 'exactDepth:3', label: 'Depth 3 (Nested)' },
  { value: 'exactDepth:4', label: 'Depth 4 (Deep)' },
  { value: 'exactDepth:5', label: 'Depth 5' },
  { value: 'exactDepth:6', label: 'Depth 6' },
  { value: 'autoDetect:1', label: 'Auto (depth 1)' },
  { value: 'autoDetect:2', label: 'Auto (up to 2 deep)' },
  { value: 'autoDetect:3', label: 'Auto (up to 3 deep)' },
  { value: 'autoDetect:4', label: 'Auto (up to 4 deep)' },
  { value: 'autoDetect:5', label: 'Auto (up to 5 deep)' },
  { value: 'autoDetect:6', label: 'Auto (up to 6 deep)' },
];

function folderNameFromPath(rawPath: string, fallback: string): string {
  const parts = rawPath.split('/').filter(Boolean);
  return parts.pop() ?? fallback;
}

function initialProjectRoots(config: AppConfig): ProjectRoot[] {
  if (config.projectRoots && config.projectRoots.length > 0) {
    return config.projectRoots;
  }
  if (config.projectsRoot && config.projectsRoot.trim()) {
    return [
      {
        id: 'default-projects',
        label: folderNameFromPath(config.projectsRoot, 'Projects'),
        path: config.projectsRoot,
        depth: 2,
        mode: 'exactDepth',
      },
    ];
  }
  return [];
}

function buildInitialAutoTagRules(
  config: AppConfig,
  discoveredAutoFolders: string[],
): TagDefinition[] {
  const map = new Map<string, TagDefinition>();
  for (const def of config.tagDefinitions) {
    if (def.autoFolder) {
      map.set(def.autoFolder.toLowerCase(), { ...def });
    }
  }
  for (const folder of discoveredAutoFolders) {
    const key = folder.toLowerCase();
    if (!map.has(key)) {
      map.set(key, {
        id: `auto:${key}`,
        label: folder,
        color: 'gray',
        autoFolder: folder,
        hidden: false,
      });
    }
  }
  return Array.from(map.values()).sort((a, b) =>
    (a.autoFolder ?? a.label).localeCompare(b.autoFolder ?? b.label, undefined, {
      sensitivity: 'base',
    }),
  );
}

export function SettingsModal({
  open,
  initialTab = 'general',
  config,
  discoveredAutoFolders,
  availableSkills,
  availableMcps,
  availableRules,
  onClose,
  onSave,
}: SettingsModalProps) {
  const dialogRef = useRef<HTMLDialogElement | null>(null);
  const pressedOutsideRef = useRef(false);

  const [activeTab, setActiveTab] = useState<SettingsTab>(initialTab);
  const [projectRoots, setProjectRoots] = useState<ProjectRoot[]>(() =>
    initialProjectRoots(config),
  );
  const [ignoreText, setIgnoreText] = useState(config.ignoreFolders.join(', '));
  const [librarySources, setLibrarySources] = useState<LibrarySource[]>(
    config.librarySources,
  );
  const [referenceScanDepth, setReferenceScanDepth] = useState<number>(
    config.referenceScanDepth ?? 3,
  );
  const [antigravityDir, setAntigravityDir] = useState(
    config.agentPaths?.antigravityDir ?? '',
  );
  const [codexDir, setCodexDir] = useState(config.agentPaths?.codexDir ?? '');
  const [claudeDir, setClaudeDir] = useState(config.agentPaths?.claudeDir ?? '');
  const [antigravity, setAntigravity] = useState(config.activeAgents.antigravity);
  const [codex, setCodex] = useState(config.activeAgents.codex);
  const [claude, setClaude] = useState(config.activeAgents.claude);
  const [gitHygiene, setGitHygiene] = useState(config.gitHygiene);

  const [autoTagRules, setAutoTagRules] = useState<TagDefinition[]>(() =>
    buildInitialAutoTagRules(config, discoveredAutoFolders),
  );
  const [customTags, setCustomTags] = useState<TagDefinition[]>(() =>
    config.tagDefinitions.filter((d) => !d.autoFolder),
  );

  const [sets, setSets] = useState<ContextSet[]>(() => config.sets ?? []);
  const [selectedSetId, setSelectedSetId] = useState<string | null>(
    () => config.sets?.[0]?.id ?? null,
  );

  const [saving, setSaving] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) {
      return;
    }
    if (open) {
      setActiveTab(initialTab);
      setProjectRoots(initialProjectRoots(config));
      setIgnoreText(config.ignoreFolders.join(', '));
      setLibrarySources(config.librarySources);
      setReferenceScanDepth(config.referenceScanDepth ?? 3);
      setAntigravityDir(config.agentPaths?.antigravityDir ?? '');
      setCodexDir(config.agentPaths?.codexDir ?? '');
      setClaudeDir(config.agentPaths?.claudeDir ?? '');
      setAntigravity(config.activeAgents.antigravity);
      setCodex(config.activeAgents.codex);
      setClaude(config.activeAgents.claude);
      setGitHygiene(config.gitHygiene);
      setAutoTagRules(buildInitialAutoTagRules(config, discoveredAutoFolders));
      setCustomTags(config.tagDefinitions.filter((d) => !d.autoFolder));
      setSets(config.sets ?? []);
      setSelectedSetId((prev) =>
        prev && (config.sets ?? []).some((s) => s.id === prev)
          ? prev
          : config.sets?.[0]?.id ?? null,
      );
      setErrorMsg(null);
      if (!dialog.open) {
        dialog.showModal();
      }
    } else if (dialog.open) {
      dialog.close();
    }
  }, [open, initialTab, config, discoveredAutoFolders]);

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

  const handleAddProjectRoot = async () => {
    const picked = await api.pickDirectory(projectRoots[0]?.path ?? null);
    if (!picked) {
      return;
    }
    const label = folderNameFromPath(picked, 'Projects');
    setProjectRoots((prev) => [
      ...prev,
      {
        id: `root-${Date.now()}`,
        label,
        path: picked,
        depth: 2,
        mode: 'autoDetect',
      },
    ]);
  };

  const handleBrowseProjectRootPath = async (idx: number) => {
    const current = projectRoots[idx]?.path ?? null;
    const picked = await api.pickDirectory(current);
    if (!picked) {
      return;
    }
    setProjectRoots((prev) =>
      prev.map((item, i) =>
        i === idx
          ? {
              ...item,
              path: picked,
              label: item.label.trim()
                ? item.label
                : folderNameFromPath(picked, 'Projects'),
            }
          : item,
      ),
    );
  };

  const handleAddLibrarySource = async () => {
    const picked = await api.pickDirectory(null);
    if (!picked) {
      return;
    }
    const folderName = folderNameFromPath(picked, 'Library');
    setLibrarySources((prev) => [
      ...prev,
      {
        id: `lib-${Date.now()}`,
        label: folderName,
        path: picked,
        depth: 2,
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

  const handleBrowseAgentDir = async (
    current: string,
    setter: (value: string) => void,
  ) => {
    const picked = await api.pickDirectory(current || null);
    if (picked) {
      setter(picked);
    }
  };

  const handleAddCustomTag = () => {
    setCustomTags((prev) => [
      ...prev,
      {
        id: `tag-${Date.now()}`,
        label: 'New Tag',
        color: 'blue',
        autoFolder: null,
        hidden: false,
      },
    ]);
  };

  const handleAddSet = () => {
    const nextId = `set-${Date.now()}`;
    const newSet: ContextSet = {
      id: nextId,
      name: 'New Set',
      description: '',
      skillIds: [],
      mcpNames: [],
      ruleFilenames: [],
    };
    setSets((prev) => [...prev, newSet]);
    setSelectedSetId(nextId);
  };

  const librarySkills = useMemo(
    () => availableSkills.filter((s) => s.source.kind === 'library'),
    [availableSkills],
  );
  const libraryRules = useMemo(
    () => availableRules.filter((r) => r.isLibraryRule || r.isSymlink),
    [availableRules],
  );

  const selectedSet = useMemo(
    () => sets.find((s) => s.id === selectedSetId) ?? null,
    [sets, selectedSetId],
  );

  const updateSelectedSet = (updater: (set: ContextSet) => ContextSet) => {
    if (!selectedSetId) {
      return;
    }
    setSets((prev) =>
      prev.map((s) => (s.id === selectedSetId ? updater(s) : s)),
    );
  };

  const toggleSetMember = (
    key: 'skillIds' | 'mcpNames' | 'ruleFilenames',
    value: string,
  ) => {
    updateSelectedSet((set) => {
      const list = set[key];
      const next = list.includes(value)
        ? list.filter((item) => item !== value)
        : [...list, value];
      return { ...set, [key]: next };
    });
  };

  const selectedSetTokens = useMemo(() => {
    if (!selectedSet) {
      return 0;
    }
    let total = 0;
    for (const id of selectedSet.skillIds) {
      total += availableSkills.find((s) => s.id === id)?.upfrontTokens ?? 0;
    }
    for (const name of selectedSet.mcpNames) {
      total += availableMcps.find((m) => m.name === name)?.upfrontTokens ?? 0;
    }
    for (const fn of selectedSet.ruleFilenames) {
      total += availableRules.find((r) => r.filename === fn)?.upfrontTokens ?? 0;
    }
    return total;
  }, [selectedSet, availableSkills, availableMcps, availableRules]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setSaving(true);
    setErrorMsg(null);

    try {
      const ignoreFolders = ignoreText
        .split(',')
        .map((s) => s.trim())
        .filter(Boolean);

      const cleanedProjectRoots = projectRoots
        .map((root) => ({
          ...root,
          label:
            root.label.trim() || folderNameFromPath(root.path, 'Projects'),
          path: root.path.trim(),
          depth: Math.min(6, Math.max(1, Number(root.depth) || 2)),
        }))
        .filter((root) => root.path.length > 0);

      const cleanedSources = librarySources
        .map((src) => ({
          ...src,
          label: src.label.trim() || folderNameFromPath(src.path, 'Library'),
          path: src.path.trim(),
          depth: Math.min(5, Math.max(1, Number(src.depth) || 2)),
        }))
        .filter((src) => src.path.length > 0);

      const existingAutoFolders = new Set(
        config.tagDefinitions
          .filter((d) => d.autoFolder)
          .map((d) => d.autoFolder!.toLowerCase()),
      );
      const savedAutoTags = autoTagRules
        .filter((rule) => {
          const folder = rule.autoFolder ?? '';
          const isCustomized =
            rule.hidden ||
            rule.color !== 'gray' ||
            rule.label.trim() !== folder;
          return (
            isCustomized || existingAutoFolders.has(folder.toLowerCase())
          );
        })
        .map((rule) => ({
          ...rule,
          label: rule.label.trim() || rule.autoFolder || 'Tag',
        }));

      const savedCustomTags = customTags
        .map((t) => ({
          ...t,
          label: t.label.trim(),
          autoFolder: null,
        }))
        .filter((t) => t.label.length > 0);

      const cleanedSets = sets
        .map((s) => ({
          ...s,
          name: s.name.trim(),
          description: s.description.trim(),
        }))
        .filter((s) => s.name.length > 0);

      const nextConfig: AppConfig = {
        ...config,
        projectsRoot: null,
        projectRoots: cleanedProjectRoots,
        librarySources: cleanedSources,
        ignoreFolders,
        referenceScanDepth: Math.min(
          10,
          Math.max(1, Number(referenceScanDepth) || 3),
        ),
        agentPaths: {
          antigravityDir: antigravityDir.trim() || null,
          codexDir: codexDir.trim() || null,
          claudeDir: claudeDir.trim() || null,
        },
        activeAgents: {
          antigravity,
          codex,
          claude,
        },
        gitHygiene,
        tagDefinitions: [...savedAutoTags, ...savedCustomTags],
        sets: cleanedSets,
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
      className="mac-sheet m-auto w-[740px] max-w-[calc(100vw-48px)] max-h-[calc(100vh-56px)] overflow-hidden p-0"
    >
      <form
        onSubmit={handleSubmit}
        className="flex max-h-[calc(100vh-56px)] flex-col"
      >
        {/* Classic macOS Preferences Centered Icon-Tab Toolbar */}
        <div className="relative shrink-0 border-b border-[var(--hairline)] bg-[var(--bg-toolbar)] px-5 pt-3 pb-2.5 backdrop-blur-xl">
          <div className="flex items-center justify-between">
            <span className="w-7" aria-hidden="true" />
            <h2
              id="settings-modal-title"
              className="text-[13px] font-semibold text-[var(--text-primary)]"
            >
              Settings
            </h2>
            <button
              type="button"
              onClick={() => dialogRef.current?.close()}
              aria-label="Close settings"
              className="inline-flex h-7 w-7 items-center justify-center rounded-[6px] text-[var(--text-secondary)] transition-colors duration-100 hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
            >
              <X className="h-4 w-4" strokeWidth={1.75} />
            </button>
          </div>

          <div
            role="tablist"
            aria-label="Settings sections"
            className="mt-2 flex items-center justify-center gap-1.5"
          >
            {(
              [
                { id: 'general', label: 'General', icon: Settings2 },
                { id: 'tags', label: 'Tags', icon: Tag },
                { id: 'sets', label: 'Sets', icon: Boxes },
              ] as const
            ).map((tab) => {
              const Icon = tab.icon;
              const selected = activeTab === tab.id;
              return (
                <button
                  key={tab.id}
                  type="button"
                  role="tab"
                  aria-selected={selected}
                  onClick={() => setActiveTab(tab.id)}
                  className={[
                    'flex min-w-[74px] flex-col items-center gap-0.5 rounded-[8px] px-3 py-1.5 text-[11px] font-medium transition-colors duration-100',
                    selected
                      ? 'bg-[var(--bg-control)] text-[var(--accent)] shadow-[var(--control-shadow)]'
                      : 'text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]',
                  ].join(' ')}
                >
                  <Icon className="h-[17px] w-[17px]" strokeWidth={1.7} />
                  <span>{tab.label}</span>
                </button>
              );
            })}
          </div>
        </div>

        {/* Body */}
        <div className="flex-1 space-y-5 overflow-y-auto px-5 py-4">
          {errorMsg && (
            <div className="rounded-[8px] border border-[var(--status-red)]/30 bg-[var(--status-red)]/10 px-3 py-2 text-[12.5px] text-[var(--status-red)]">
              {errorMsg}
            </div>
          )}

          {activeTab === 'general' && (
            <>
              {/* Project Locations & Scan Depth */}
              <div className="space-y-2">
                <div className="flex items-center justify-between gap-2 px-1">
                  <div>
                    <span className="block text-[12px] font-semibold text-[var(--text-secondary)]">
                      Project Locations & Scan Depth
                    </span>
                    <span className="text-[11.5px] text-[var(--text-tertiary)]">
                      Folders containing repositories, scanned by exact depth or marker auto-detection.
                    </span>
                  </div>
                  <button
                    type="button"
                    onClick={handleAddProjectRoot}
                    className="mac-btn inline-flex h-7 shrink-0 items-center gap-1 px-2.5 text-[12px] font-medium"
                  >
                    <Plus className="h-3.5 w-3.5" strokeWidth={1.75} />
                    Add Location
                  </button>
                </div>

                {projectRoots.length === 0 ? (
                  <div className="mac-group px-3.5 py-2.5 text-[12px] text-[var(--text-tertiary)]">
                    No project locations configured. Add a workspace folder (for example ~/Projects or ~/Code).
                  </div>
                ) : (
                  <div className="mac-group overflow-hidden">
                    {projectRoots.map((root, idx) => (
                      <div
                        key={root.id}
                        className={[
                          'flex items-center gap-2 px-3 py-2',
                          idx > 0 ? 'mac-row-divider' : '',
                        ].join(' ')}
                      >
                        <input
                          type="text"
                          value={root.label}
                          aria-label="Location group label"
                          onChange={(e) =>
                            setProjectRoots((prev) =>
                              prev.map((item, i) =>
                                i === idx
                                  ? { ...item, label: e.target.value }
                                  : item,
                              ),
                            )
                          }
                          placeholder="Label"
                          className="mac-input h-8 w-24 shrink-0 px-2.5 text-[12.5px]"
                        />
                        <input
                          type="text"
                          autoFocus={idx === 0}
                          value={root.path}
                          aria-label="Project root path"
                          onChange={(e) =>
                            setProjectRoots((prev) =>
                              prev.map((item, i) =>
                                i === idx
                                  ? { ...item, path: e.target.value }
                                  : item,
                              ),
                            )
                          }
                          placeholder="~/Projects"
                          className="mac-input mono-path h-8 min-w-0 flex-1 px-2.5 text-[12px]"
                        />
                        <MacSelect
                          aria-label="Project scan depth"
                          value={`${root.mode}:${root.depth}`}
                          onChange={(e) => {
                            const [modeRaw, depthRaw] = e.target.value.split(':');
                            const nextMode =
                              modeRaw === 'autoDetect'
                                ? 'autoDetect'
                                : 'exactDepth';
                            const nextDepth = Number(depthRaw) || 2;
                            setProjectRoots((prev) =>
                              prev.map((item, i) =>
                                i === idx
                                  ? {
                                      ...item,
                                      mode: nextMode,
                                      depth: nextDepth,
                                    }
                                  : item,
                              ),
                            );
                          }}
                        >
                          {PROJECT_DEPTH_OPTIONS.map((opt) => (
                            <option key={opt.value} value={opt.value}>
                              {opt.label}
                            </option>
                          ))}
                        </MacSelect>
                        <button
                          type="button"
                          onClick={() => handleBrowseProjectRootPath(idx)}
                          aria-label="Browse project location"
                          className="mac-btn inline-flex h-8 w-8 shrink-0 items-center justify-center"
                        >
                          <FolderOpen className="h-3.5 w-3.5" strokeWidth={1.75} />
                        </button>
                        <button
                          type="button"
                          onClick={() =>
                            setProjectRoots((prev) =>
                              prev.filter((_, i) => i !== idx),
                            )
                          }
                          aria-label={`Remove ${root.label || root.path}`}
                          className="inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-[6px] text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] hover:text-[var(--status-red)]"
                        >
                          <Trash2 className="h-3.5 w-3.5" strokeWidth={1.75} />
                        </button>
                      </div>
                    ))}
                  </div>
                )}
              </div>

              {/* Ignore Folders */}
              <div className="space-y-1.5">
                <label
                  htmlFor="ignore-folders-input"
                  className="block px-1 text-[12px] font-semibold text-[var(--text-secondary)]"
                >
                  Ignored Folder Names (comma-separated)
                </label>
                <div className="mac-group p-2.5">
                  <input
                    id="ignore-folders-input"
                    type="text"
                    value={ignoreText}
                    onChange={(e) => setIgnoreText(e.target.value)}
                    placeholder=".git, node_modules, _archive, Archive"
                    className="mac-input mono-path h-8 w-full px-2.5 text-[12px]"
                  />
                </div>
              </div>

              {/* Library Sources */}
              <div className="space-y-2">
                <div className="flex items-center justify-between gap-2 px-1">
                  <div>
                    <span className="block text-[12px] font-semibold text-[var(--text-secondary)]">
                      Library Sources (Canonical Skills & Rules)
                    </span>
                    <span className="text-[11.5px] text-[var(--text-tertiary)]">
                      Shared skill and rule folders scanned for symlinking across agents.
                    </span>
                  </div>
                  <button
                    type="button"
                    onClick={handleAddLibrarySource}
                    className="mac-btn inline-flex h-7 shrink-0 items-center gap-1 px-2.5 text-[12px] font-medium"
                  >
                    <Plus className="h-3.5 w-3.5" strokeWidth={1.75} />
                    Add Source
                  </button>
                </div>

                {librarySources.length === 0 ? (
                  <div className="mac-group px-3.5 py-2.5 text-[12px] text-[var(--text-tertiary)]">
                    No library sources configured. Add a canonical skills or rules folder to attribute symlinks.
                  </div>
                ) : (
                  <div className="mac-group overflow-hidden">
                    {librarySources.map((src, idx) => (
                      <div
                        key={src.id}
                        className={[
                          'flex items-center gap-2 px-3 py-2',
                          idx > 0 ? 'mac-row-divider' : '',
                        ].join(' ')}
                      >
                        <input
                          type="text"
                          value={src.label}
                          aria-label="Library label"
                          onChange={(e) =>
                            setLibrarySources((prev) =>
                              prev.map((item, i) =>
                                i === idx
                                  ? { ...item, label: e.target.value }
                                  : item,
                              ),
                            )
                          }
                          placeholder="Label"
                          className="mac-input h-8 w-24 shrink-0 px-2.5 text-[12.5px]"
                        />
                        <input
                          type="text"
                          value={src.path}
                          aria-label="Library path"
                          onChange={(e) =>
                            setLibrarySources((prev) =>
                              prev.map((item, i) =>
                                i === idx
                                  ? { ...item, path: e.target.value }
                                  : item,
                              ),
                            )
                          }
                          placeholder="~/Skills"
                          className="mac-input mono-path h-8 min-w-0 flex-1 px-2.5 text-[12px]"
                        />
                        <MacSelect
                          aria-label="Library scan depth"
                          value={src.depth ?? 2}
                          onChange={(e) => {
                            const nextDepth = Number(e.target.value) || 2;
                            setLibrarySources((prev) =>
                              prev.map((item, i) =>
                                i === idx ? { ...item, depth: nextDepth } : item,
                              ),
                            );
                          }}
                          className="w-28 shrink-0"
                        >
                          {[1, 2, 3, 4, 5].map((d) => (
                            <option key={d} value={d}>
                              {d === 1 ? '1 level' : `Up to ${d} deep`}
                            </option>
                          ))}
                        </MacSelect>
                        <button
                          type="button"
                          onClick={() => handleBrowseLibraryPath(idx)}
                          aria-label="Browse library directory"
                          className="mac-btn inline-flex h-8 w-8 shrink-0 items-center justify-center"
                        >
                          <FolderOpen className="h-3.5 w-3.5" strokeWidth={1.75} />
                        </button>
                        <button
                          type="button"
                          onClick={() =>
                            setLibrarySources((prev) =>
                              prev.filter((_, i) => i !== idx),
                            )
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

              {/* Active Agents & Git Hygiene */}
              <div className="space-y-2">
                <span className="block px-1 text-[12px] font-semibold text-[var(--text-secondary)]">
                  Active Agents & Hygiene
                </span>
                <div className="mac-group overflow-hidden">
                  <div className="flex items-center justify-between px-3.5 py-2.5">
                    <span className="text-[12.5px] font-medium">Antigravity</span>
                    <MacSwitch
                      checked={antigravity}
                      label="Enable Antigravity"
                      onChange={setAntigravity}
                    />
                  </div>
                  <div className="mac-row-divider flex items-center justify-between px-3.5 py-2.5">
                    <span className="text-[12.5px] font-medium">Codex</span>
                    <MacSwitch
                      checked={codex}
                      label="Enable Codex"
                      onChange={setCodex}
                    />
                  </div>
                  <div className="mac-row-divider flex items-center justify-between px-3.5 py-2.5">
                    <span className="text-[12.5px] font-medium">Claude Code</span>
                    <MacSwitch
                      checked={claude}
                      label="Enable Claude Code"
                      onChange={setClaude}
                    />
                  </div>
                  <div className="mac-row-divider flex items-center justify-between gap-4 px-3.5 py-2.5">
                    <div>
                      <div className="text-[12.5px] font-medium text-[var(--text-primary)]">
                        Keep Git Clean via .git/info/exclude
                      </div>
                      <div className="text-[11.5px] text-[var(--text-secondary)]">
                        Hide managed project symlinks & local agent configs without modifying .gitignore.
                      </div>
                    </div>
                    <MacSwitch
                      checked={gitHygiene}
                      label="Keep Git Clean via .git/info/exclude"
                      onChange={setGitHygiene}
                    />
                  </div>
                </div>
              </div>

              {/* Advanced Scan & Custom Agent Paths */}
              <details className="mac-group group px-3.5 py-2.5">
                <summary className="flex cursor-pointer list-none items-center gap-1.5 text-[12.5px] font-medium text-[var(--text-primary)]">
                  <ChevronRight className="h-3.5 w-3.5 text-[var(--text-secondary)] transition-transform duration-150 group-open:rotate-90" />
                  Advanced: Reference Depth & Custom Agent Directories
                </summary>

                <div className="mt-3 space-y-3 border-t border-[var(--hairline-inset)] pt-3">
                  <div className="flex items-center justify-between gap-4">
                    <div>
                      <label
                        htmlFor="ref-depth-input"
                        className="block text-[12px] font-medium text-[var(--text-primary)]"
                      >
                        Skill References Max Depth
                      </label>
                      <span className="text-[11.5px] text-[var(--text-secondary)]">
                        Subfolder depth scanned under each skill&apos;s references/ directory for on-demand token counts.
                      </span>
                    </div>
                    <input
                      id="ref-depth-input"
                      type="number"
                      min={1}
                      max={10}
                      value={referenceScanDepth}
                      onChange={(e) =>
                        setReferenceScanDepth(
                          Math.min(10, Math.max(1, Number(e.target.value) || 3)),
                        )
                      }
                      className="mac-input tabular-nums h-8 w-20 shrink-0 px-2.5 text-[12.5px]"
                    />
                  </div>

                  <div className="space-y-2">
                    <div className="text-[12px] font-medium text-[var(--text-primary)]">
                      Custom Global Agent Config Directories (Leave blank for defaults)
                    </div>

                    {[
                      {
                        id: 'ag-dir',
                        label: 'Antigravity',
                        placeholder: '~/.gemini',
                        value: antigravityDir,
                        setter: setAntigravityDir,
                      },
                      {
                        id: 'codex-dir',
                        label: 'Codex',
                        placeholder: '~/.codex',
                        value: codexDir,
                        setter: setCodexDir,
                      },
                      {
                        id: 'claude-dir',
                        label: 'Claude Code',
                        placeholder: '~/.claude',
                        value: claudeDir,
                        setter: setClaudeDir,
                      },
                    ].map((item) => (
                      <div key={item.id} className="flex items-center gap-2">
                        <label
                          htmlFor={item.id}
                          className="w-24 shrink-0 text-[12px] text-[var(--text-secondary)]"
                        >
                          {item.label}
                        </label>
                        <input
                          id={item.id}
                          type="text"
                          value={item.value}
                          onChange={(e) => item.setter(e.target.value)}
                          placeholder={item.placeholder}
                          className="mac-input mono-path h-8 min-w-0 flex-1 px-2.5 text-[12px]"
                        />
                        <button
                          type="button"
                          onClick={() =>
                            handleBrowseAgentDir(item.value, item.setter)
                          }
                          aria-label={`Browse ${item.label} config folder`}
                          className="mac-btn inline-flex h-8 w-8 shrink-0 items-center justify-center"
                        >
                          <FolderOpen className="h-3.5 w-3.5" strokeWidth={1.75} />
                        </button>
                      </div>
                    ))}
                  </div>
                </div>
              </details>
            </>
          )}

          {activeTab === 'tags' && (
            <div className="space-y-5">
              {/* Auto-Tag Rules from Ancestor Folders */}
              <div className="space-y-2">
                <div className="px-1">
                  <h3 className="text-[12px] font-semibold text-[var(--text-secondary)]">
                    Ancestor Folder Auto-Tags
                  </h3>
                  <p className="text-[11.5px] text-[var(--text-tertiary)]">
                    Projects inherit tags from their parent folder names. Editing a label, color, or visibility here updates all current and future matching projects.
                  </p>
                </div>

                {autoTagRules.length === 0 ? (
                  <div className="mac-group px-3.5 py-2.5 text-[12px] text-[var(--text-tertiary)]">
                    No ancestor folders discovered yet. Add a Project Location first.
                  </div>
                ) : (
                  <div className="mac-group overflow-hidden">
                    {autoTagRules.map((rule, idx) => (
                      <div
                        key={rule.id}
                        className={[
                          'flex items-center justify-between gap-2.5 px-3.5 py-2',
                          idx > 0 ? 'mac-row-divider' : '',
                          rule.hidden ? 'opacity-60' : '',
                        ].join(' ')}
                      >
                        <span
                          className="h-2.5 w-2.5 shrink-0 rounded-full"
                          style={{ backgroundColor: tagColorHex(rule.color) }}
                          aria-hidden="true"
                        />
                        <span className="mono-path w-32 shrink-0 truncate text-[11.5px] text-[var(--text-secondary)]">
                          {rule.autoFolder}
                        </span>
                        <input
                          type="text"
                          value={rule.label}
                          aria-label={`Tag label for folder ${rule.autoFolder}`}
                          onChange={(e) =>
                            setAutoTagRules((prev) =>
                              prev.map((item, i) =>
                                i === idx
                                  ? { ...item, label: e.target.value }
                                  : item,
                              ),
                            )
                          }
                          className="mac-input h-7 min-w-0 flex-1 px-2 text-[12px]"
                        />
                        <MacSelect
                          size="sm"
                          aria-label={`Color for ${rule.autoFolder}`}
                          value={rule.color}
                          onChange={(e) =>
                            setAutoTagRules((prev) =>
                              prev.map((item, i) =>
                                i === idx
                                  ? {
                                      ...item,
                                      color: e.target.value as TagColor,
                                    }
                                  : item,
                              ),
                            )
                          }
                          className="w-24 shrink-0"
                        >
                          {TAG_COLORS.map((c) => (
                            <option key={c.id} value={c.id}>
                              {c.label}
                            </option>
                          ))}
                        </MacSelect>
                        <MacSwitch
                          checked={!rule.hidden}
                          label={`Enable auto-tag for ${rule.autoFolder}`}
                          tooltip={
                            rule.hidden
                              ? 'Disabled globally for this folder'
                              : 'Active for all projects under this folder'
                          }
                          onChange={(enabled) =>
                            setAutoTagRules((prev) =>
                              prev.map((item, i) =>
                                i === idx
                                  ? { ...item, hidden: !enabled }
                                  : item,
                              ),
                            )
                          }
                        />
                      </div>
                    ))}
                  </div>
                )}
              </div>

              {/* Custom Manual Tags */}
              <div className="space-y-2">
                <div className="flex items-center justify-between px-1">
                  <div>
                    <h3 className="text-[12px] font-semibold text-[var(--text-secondary)]">
                      Custom Tags
                    </h3>
                    <p className="text-[11.5px] text-[var(--text-tertiary)]">
                      Manual tags you can attach to any individual project.
                    </p>
                  </div>
                  <button
                    type="button"
                    onClick={handleAddCustomTag}
                    className="mac-btn inline-flex h-7 items-center gap-1 px-2.5 text-[12px] font-medium"
                  >
                    <Plus className="h-3.5 w-3.5" strokeWidth={1.75} />
                    Add Tag
                  </button>
                </div>

                {customTags.length === 0 ? (
                  <div className="mac-group px-3.5 py-2.5 text-[12px] text-[var(--text-tertiary)]">
                    No custom tags created yet.
                  </div>
                ) : (
                  <div className="mac-group overflow-hidden">
                    {customTags.map((tag, idx) => (
                      <div
                        key={tag.id}
                        className={[
                          'flex items-center gap-2.5 px-3.5 py-2',
                          idx > 0 ? 'mac-row-divider' : '',
                        ].join(' ')}
                      >
                        <span
                          className="h-2.5 w-2.5 shrink-0 rounded-full"
                          style={{ backgroundColor: tagColorHex(tag.color) }}
                          aria-hidden="true"
                        />
                        <input
                          type="text"
                          value={tag.label}
                          aria-label="Custom tag label"
                          onChange={(e) =>
                            setCustomTags((prev) =>
                              prev.map((item, i) =>
                                i === idx
                                  ? { ...item, label: e.target.value }
                                  : item,
                              ),
                            )
                          }
                          className="mac-input h-7 min-w-0 flex-1 px-2.5 text-[12.5px]"
                        />
                        <MacSelect
                          size="sm"
                          aria-label="Custom tag color"
                          value={tag.color}
                          onChange={(e) =>
                            setCustomTags((prev) =>
                              prev.map((item, i) =>
                                i === idx
                                  ? {
                                      ...item,
                                      color: e.target.value as TagColor,
                                    }
                                  : item,
                              ),
                            )
                          }
                          className="w-24 shrink-0"
                        >
                          {TAG_COLORS.map((c) => (
                            <option key={c.id} value={c.id}>
                              {c.label}
                            </option>
                          ))}
                        </MacSelect>
                        <button
                          type="button"
                          onClick={() =>
                            setCustomTags((prev) =>
                              prev.filter((_, i) => i !== idx),
                            )
                          }
                          aria-label={`Delete tag ${tag.label}`}
                          className="inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-[6px] text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] hover:text-[var(--status-red)]"
                        >
                          <Trash2 className="h-3.5 w-3.5" strokeWidth={1.75} />
                        </button>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            </div>
          )}

          {activeTab === 'sets' && (
            <div className="grid grid-cols-1 gap-4 sm:grid-cols-[200px_minmax(0,1fr)]">
              {/* Sets List */}
              <div className="space-y-2">
                <div className="flex items-center justify-between px-1">
                  <span className="text-[12px] font-semibold text-[var(--text-secondary)]">
                    Saved Sets ({sets.length})
                  </span>
                  <button
                    type="button"
                    onClick={handleAddSet}
                    className="mac-btn inline-flex h-6 items-center gap-1 px-2 text-[11.5px] font-medium"
                  >
                    <Plus className="h-3 w-3" />
                    New
                  </button>
                </div>

                {sets.length === 0 ? (
                  <div className="mac-group p-3 text-[12px] text-[var(--text-tertiary)]">
                    Click New to create a reusable bundle of skills, MCPs, and rules.
                  </div>
                ) : (
                  <div className="mac-group overflow-hidden p-1">
                    {sets.map((s) => {
                      const active = s.id === selectedSetId;
                      const count =
                        s.skillIds.length +
                        s.mcpNames.length +
                        s.ruleFilenames.length;
                      return (
                        <button
                          key={s.id}
                          type="button"
                          onClick={() => setSelectedSetId(s.id)}
                          className={[
                            'flex w-full items-center justify-between gap-2 rounded-[6px] px-2.5 py-1.5 text-left text-[12.5px] transition-colors duration-100',
                            active
                              ? 'bg-[var(--accent)] font-medium text-white'
                              : 'text-[var(--text-primary)] hover:bg-[var(--bg-hover)]',
                          ].join(' ')}
                        >
                          <span className="truncate">{s.name || 'Untitled'}</span>
                          <span
                            className={[
                              'tabular-nums shrink-0 text-[11px]',
                              active
                                ? 'text-white/80'
                                : 'text-[var(--text-tertiary)]',
                            ].join(' ')}
                          >
                            {count}
                          </span>
                        </button>
                      );
                    })}
                  </div>
                )}
              </div>

              {/* Selected Set Editor */}
              {selectedSet ? (
                <div className="mac-group space-y-3 p-3.5">
                  <div className="flex items-start justify-between gap-2">
                    <div className="flex-1 space-y-2">
                      <input
                        type="text"
                        value={selectedSet.name}
                        aria-label="Set name"
                        onChange={(e) =>
                          updateSelectedSet((s) => ({
                            ...s,
                            name: e.target.value,
                          }))
                        }
                        placeholder="Set name"
                        className="mac-input h-8 w-full px-2.5 text-[12.5px] font-medium"
                      />
                      <input
                        type="text"
                        value={selectedSet.description}
                        aria-label="Set description"
                        onChange={(e) =>
                          updateSelectedSet((s) => ({
                            ...s,
                            description: e.target.value,
                          }))
                        }
                        placeholder="Optional description"
                        className="mac-input h-7 w-full px-2.5 text-[12px]"
                      />
                    </div>
                    <button
                      type="button"
                      onClick={() => {
                        const remaining = sets.filter(
                          (s) => s.id !== selectedSet.id,
                        );
                        setSets(remaining);
                        setSelectedSetId(remaining[0]?.id ?? null);
                      }}
                      aria-label={`Delete set ${selectedSet.name}`}
                      className="inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-[6px] text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] hover:text-[var(--status-red)]"
                    >
                      <Trash2 className="h-3.5 w-3.5" />
                    </button>
                  </div>

                  <div className="tabular-nums text-[11.5px] text-[var(--text-secondary)]">
                    Estimated set overhead:{' '}
                    <span className="font-medium text-[var(--text-primary)]">
                      {selectedSetTokens.toLocaleString('en-US')} tok
                    </span>
                  </div>

                  {/* Skills */}
                  <div className="space-y-1">
                    <div className="text-[11.5px] font-medium text-[var(--text-secondary)]">
                      Library Skills ({selectedSet.skillIds.length})
                    </div>
                    <div className="max-h-28 space-y-0.5 overflow-y-auto rounded-[7px] border border-[var(--hairline-inset)] bg-[var(--bg-canvas)] p-1.5">
                      {librarySkills.length === 0 ? (
                        <div className="px-2 py-1 text-[11.5px] text-[var(--text-tertiary)]">
                          No library skills found.
                        </div>
                      ) : (
                        librarySkills.map((skill) => (
                          <label
                            key={skill.id}
                            className="flex cursor-pointer items-center justify-between gap-2 rounded-[5px] px-1.5 py-1 text-[12px] hover:bg-[var(--bg-hover)]"
                          >
                            <span className="flex min-w-0 items-center gap-2">
                              <input
                                type="checkbox"
                                checked={selectedSet.skillIds.includes(skill.id)}
                                onChange={() =>
                                  toggleSetMember('skillIds', skill.id)
                                }
                              />
                              <span className="truncate">{skill.name}</span>
                            </span>
                            <span className="tabular-nums text-[11px] text-[var(--text-tertiary)]">
                              {skill.upfrontTokens} tok
                            </span>
                          </label>
                        ))
                      )}
                    </div>
                  </div>

                  {/* MCP Servers */}
                  <div className="space-y-1">
                    <div className="text-[11.5px] font-medium text-[var(--text-secondary)]">
                      MCP Servers ({selectedSet.mcpNames.length})
                    </div>
                    <div className="max-h-28 space-y-0.5 overflow-y-auto rounded-[7px] border border-[var(--hairline-inset)] bg-[var(--bg-canvas)] p-1.5">
                      {availableMcps.length === 0 ? (
                        <div className="px-2 py-1 text-[11.5px] text-[var(--text-tertiary)]">
                          No MCP servers discovered.
                        </div>
                      ) : (
                        availableMcps.map((srv) => (
                          <label
                            key={srv.name}
                            className="flex cursor-pointer items-center justify-between gap-2 rounded-[5px] px-1.5 py-1 text-[12px] hover:bg-[var(--bg-hover)]"
                          >
                            <span className="flex min-w-0 items-center gap-2">
                              <input
                                type="checkbox"
                                checked={selectedSet.mcpNames.includes(srv.name)}
                                onChange={() =>
                                  toggleSetMember('mcpNames', srv.name)
                                }
                              />
                              <span className="truncate">{srv.name}</span>
                            </span>
                            <span className="tabular-nums text-[11px] text-[var(--text-tertiary)]">
                              {srv.upfrontTokens} tok
                            </span>
                          </label>
                        ))
                      )}
                    </div>
                  </div>

                  {/* Library Rules */}
                  {libraryRules.length > 0 && (
                    <div className="space-y-1">
                      <div className="text-[11.5px] font-medium text-[var(--text-secondary)]">
                        Library Rules ({selectedSet.ruleFilenames.length})
                      </div>
                      <div className="max-h-24 space-y-0.5 overflow-y-auto rounded-[7px] border border-[var(--hairline-inset)] bg-[var(--bg-canvas)] p-1.5">
                        {libraryRules.map((rule) => (
                          <label
                            key={rule.id}
                            className="flex cursor-pointer items-center justify-between gap-2 rounded-[5px] px-1.5 py-1 text-[12px] hover:bg-[var(--bg-hover)]"
                          >
                            <span className="flex min-w-0 items-center gap-2">
                              <input
                                type="checkbox"
                                checked={selectedSet.ruleFilenames.includes(
                                  rule.filename,
                                )}
                                onChange={() =>
                                  toggleSetMember(
                                    'ruleFilenames',
                                    rule.filename,
                                  )
                                }
                              />
                              <span className="truncate">{rule.filename}</span>
                            </span>
                            <span className="tabular-nums text-[11px] text-[var(--text-tertiary)]">
                              {rule.upfrontTokens} tok
                            </span>
                          </label>
                        ))}
                      </div>
                    </div>
                  )}
                </div>
              ) : (
                <div className="mac-group flex items-center justify-center p-6 text-[12px] text-[var(--text-tertiary)]">
                  Select or create a Set to configure its skills, MCP servers, and rules.
                </div>
              )}
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
            {saving ? 'Saving…' : 'Save & Rescan'}
          </button>
        </div>
      </form>
    </dialog>
  );
}

import { AlertTriangle, Layers, Server, Sparkles } from 'lucide-react';
import type { GlobalPauseState, GlobalSwitchTarget } from '@/types';
import { MacSwitch } from '@/components/ui/MacSwitch';

interface GlobalQuickControlsProps {
  globalPause: GlobalPauseState;
  activeSkillsCount: number;
  activeMcpCount: number;
  busyTarget: GlobalSwitchTarget | null;
  skippedUnmanaged: string[];
  onToggleSwitch: (target: GlobalSwitchTarget, enable: boolean) => void;
  onMoveUnmanagedToLibrary: (skillIds: string[]) => void;
  onDismissNotice: () => void;
}

export function GlobalQuickControls({
  globalPause,
  activeSkillsCount,
  activeMcpCount,
  busyTarget,
  skippedUnmanaged,
  onToggleSwitch,
  onMoveUnmanagedToLibrary,
  onDismissNotice,
}: GlobalQuickControlsProps) {
  const allPaused = globalPause.skillsPaused && globalPause.mcpPaused;
  const allOn = !allPaused;
  const skillsOn = !globalPause.skillsPaused;
  const mcpOn = !globalPause.mcpPaused;
  const isBusy = busyTarget !== null;

  return (
    <section aria-label="Global quick switches" className="space-y-2.5">
      <div className="mac-group grid grid-cols-1 divide-y divide-[var(--hairline-inset)] overflow-hidden sm:grid-cols-3 sm:divide-x sm:divide-y-0">
        {/* All Global */}
        <div className="flex items-center justify-between gap-3 px-4 py-2.5">
          <div className="min-w-0">
            <div className="flex items-center gap-1.5 text-[12.5px] font-medium text-[var(--text-primary)]">
              <Layers
                className="h-3.5 w-3.5 shrink-0 text-[var(--accent)]"
                strokeWidth={1.75}
                aria-hidden="true"
              />
              <span className="truncate">All Global</span>
            </div>
            <div className="truncate text-[11.5px] text-[var(--text-secondary)]">
              {allPaused
                ? `Paused · ${globalPause.pausedSkills.length + globalPause.pausedMcp.length + globalPause.pausedRules.length} saved`
                : `${activeSkillsCount} skills · ${activeMcpCount} MCP`}
            </div>
          </div>
          <MacSwitch
            checked={allOn}
            disabled={isBusy}
            label="Toggle all global skills, MCP servers, and library rules"
            tooltip={
              allOn
                ? 'Pause all managed global skills, MCP servers, and library rules'
                : 'Restore all previously paused global items'
            }
            onChange={(next) => onToggleSwitch('all', next)}
          />
        </div>

        {/* All Skills */}
        <div className="flex items-center justify-between gap-3 px-4 py-2.5">
          <div className="min-w-0">
            <div className="flex items-center gap-1.5 text-[12.5px] font-medium text-[var(--text-primary)]">
              <Sparkles
                className="h-3.5 w-3.5 shrink-0 text-[var(--text-secondary)]"
                strokeWidth={1.75}
                aria-hidden="true"
              />
              <span className="truncate">All Skills</span>
            </div>
            <div className="text-[11.5px] text-[var(--text-secondary)]">
              {globalPause.skillsPaused
                ? `Paused · ${globalPause.pausedSkills.length} saved`
                : `${activeSkillsCount} active globally`}
            </div>
          </div>
          <MacSwitch
            checked={skillsOn}
            disabled={isBusy}
            label="Toggle all global skills"
            tooltip={
              skillsOn
                ? 'Pause all Library-managed global skills'
                : 'Restore previously paused global skills'
            }
            onChange={(next) => onToggleSwitch('skills', next)}
          />
        </div>

        {/* All MCP */}
        <div className="flex items-center justify-between gap-3 px-4 py-2.5">
          <div className="min-w-0">
            <div className="flex items-center gap-1.5 text-[12.5px] font-medium text-[var(--text-primary)]">
              <Server
                className="h-3.5 w-3.5 shrink-0 text-[var(--text-secondary)]"
                strokeWidth={1.75}
                aria-hidden="true"
              />
              <span className="truncate">All MCP</span>
            </div>
            <div className="text-[11.5px] text-[var(--text-secondary)]">
              {globalPause.mcpPaused
                ? `Paused · ${globalPause.pausedMcp.length} saved`
                : `${activeMcpCount} active globally`}
            </div>
          </div>
          <MacSwitch
            checked={mcpOn}
            disabled={isBusy}
            label="Toggle all global MCP servers"
            tooltip={
              mcpOn
                ? 'Pause all global MCP servers'
                : 'Restore previously paused global MCP servers'
            }
            onChange={(next) => onToggleSwitch('mcp', next)}
          />
        </div>
      </div>

      {skippedUnmanaged.length > 0 && (
        <div className="mac-group flex flex-wrap items-center justify-between gap-3 px-4 py-2.5 text-[12px]">
          <div className="flex items-center gap-2 text-[var(--text-primary)]">
            <AlertTriangle
              className="h-3.5 w-3.5 shrink-0 text-[var(--status-amber)]"
              strokeWidth={1.75}
              aria-hidden="true"
            />
            <span>
              Kept {skippedUnmanaged.length} loose skill{' '}
              {skippedUnmanaged.length === 1 ? 'folder' : 'folders'} active (
              {skippedUnmanaged.join(', ')}) because AgentDock never deletes real folders. Move them to your Library to pause and resume them with one switch.
            </span>
          </div>
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={() => onMoveUnmanagedToLibrary(skippedUnmanaged)}
              className="mac-btn h-[26px] px-2.5 text-[11.5px] font-medium"
            >
              Consolidate to Library…
            </button>
            <button
              type="button"
              onClick={onDismissNotice}
              className="h-[26px] rounded-[6px] px-2 text-[11.5px] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)]"
            >
              Dismiss
            </button>
          </div>
        </div>
      )}
    </section>
  );
}

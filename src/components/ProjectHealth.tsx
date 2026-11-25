import { AlertCircle } from 'lucide-react';
import type { AgentFilter, ScopeSnapshot } from '@/types';
import { mcpRowId } from '@/components/McpSection';

export function ProjectHealth({
  snapshot,
  agentFilter,
  onOpenCleanup,
}: {
  snapshot: ScopeSnapshot;
  agentFilter: AgentFilter;
  onOpenCleanup: () => void;
}) {
  const matchesAgent = (agent: AgentFilter) => agentFilter === 'all' || agent === agentFilter;
  const brokenLinks = new Set([
    ...snapshot.skills.flatMap((skill) => skill.installedPaths
      .filter((entry) => entry.scope === 'project' && entry.broken && matchesAgent(entry.agent))
      .map((entry) => entry.path)),
    ...snapshot.rules.flatMap((rule) => rule.installations
      .filter((entry) => entry.scope === 'project' && entry.isSymlink && entry.error?.startsWith('Broken symlink') && matchesAgent(entry.agent))
      .map((entry) => entry.path)),
  ]);
  const configIssues = [...new Set([
    ...snapshot.scanWarnings,
    ...snapshot.rules.flatMap((rule) => rule.installations
      .filter((entry) => entry.scope === 'project' && entry.error && !brokenLinks.has(entry.path) && matchesAgent(entry.agent))
      .map((entry) => `${entry.path}: ${entry.error}`)),
  ])];
  const unverified = snapshot.mcpServers.filter((server) =>
    server.probedAtMs == null
    && server.definitions.some((definition) => definition.enabled && matchesAgent(definition.agent)),
  );

  const issueCounts = [
    [brokenLinks.size, 'broken link', 'broken links'],
    [configIssues.length, 'configuration issue', 'configuration issues'],
    [unverified.length, 'unverified MCP server', 'unverified MCP servers'],
  ] as const;
  const summary = issueCounts.filter(([count]) => count > 0)
    .map(([count, singular, plural]) => `${count} ${count === 1 ? singular : plural}`)
    .join(' · ');

  if (!summary) return null;

  return (
    <details className="mac-group px-3.5 py-2.5 text-[12px]">
      <summary className="cursor-pointer rounded-[4px] font-medium">
        <AlertCircle aria-hidden="true" className="mx-1 inline h-3.5 w-3.5 text-[var(--status-amber)]" />
        {summary}
      </summary>
      <div className="mt-2 space-y-2 text-[var(--text-secondary)]">
        {brokenLinks.size > 0 && (
          <div>
            <ul className="mb-1">
              {[...brokenLinks].map((path) => <li key={path} className="break-all">{path}</li>)}
            </ul>
            <button type="button" className="mac-btn px-2 py-1" onClick={onOpenCleanup}>
              Open Cleanup Audit
            </button>
          </div>
        )}
        {configIssues.length > 0 && (
          <ul className="space-y-0.5">
            {configIssues.map((issue) => <li key={issue} className="break-words">{issue}</li>)}
          </ul>
        )}
        {unverified.length > 0 && (
          <ul className="space-y-0.5" aria-label="Unverified MCP servers">
            {unverified.map((server) => (
              <li key={server.name}>
                <button
                  type="button"
                  className="break-all text-left text-[var(--accent)] underline"
                  onClick={() => {
                    const row = document.getElementById(mcpRowId(server.name));
                    row?.focus({ preventScroll: true });
                    row?.scrollIntoView({ block: 'center' });
                  }}
                >
                  {server.name}
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
    </details>
  );
}

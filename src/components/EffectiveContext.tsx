import { useState } from 'react';
import { MacSelect } from '@/components/ui/MacSelect';
import { agentDisplayName, formatTokensExact } from '@/lib/format';
import type { ActiveAgents, AgentFilter, AgentKind, ScopeSnapshot } from '@/types';

interface ContextSource {
  path: string;
  label: string;
  detail?: string;
}

interface ContextEntry {
  id: string;
  name: string;
  status: string;
  tokens?: number;
  sources: ContextSource[];
}

const scopeLabel = { global: 'Global', project: 'Project' };
const agentOrder: AgentKind[] = ['antigravity', 'codex', 'claude'];

function ContextGroup({ title, entries }: { title: string; entries: ContextEntry[] }) {
  return (
    <section className="space-y-2">
      <h3 className="text-[13px] font-semibold">{title}</h3>
      {entries.length === 0 ? (
        <p className="text-[12px] text-[var(--text-secondary)]">None discovered.</p>
      ) : (
        <ul className="mac-group">
          {entries.map(({ id, name, status, tokens, sources }) => (
            <li key={id} className="px-3.5 py-2.5 [&+li]:border-t [&+li]:border-[var(--hairline)]">
              <div className="flex flex-wrap items-baseline justify-between gap-x-3">
                <span className="text-[13px] font-medium break-all">{name}</span>
                <span className="text-[12px] text-[var(--text-secondary)]">
                  {status}{tokens !== undefined && ` · ~${formatTokensExact(tokens)} tokens`}
                </span>
              </div>
              <ul className="mt-1.5 space-y-1.5">
                {sources.map(({ path, label, detail }, index) => (
                  <li key={`${path}:${label}:${index}`} className="text-[12px] text-[var(--text-secondary)]">
                    <span className="font-medium">{label}</span>
                    <span className="ml-2 break-all font-mono text-[11px]">{path}</span>
                    {detail && <p className="mt-0.5 break-words">{detail}</p>}
                  </li>
                ))}
              </ul>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

export function EffectiveContext({
  snapshot,
  activeAgents,
  agentFilter,
}: {
  snapshot: ScopeSnapshot;
  activeAgents: ActiveAgents;
  agentFilter: AgentFilter;
}) {
  const agents = agentOrder.filter((agent) => activeAgents[agent]);
  const [preferredAgent, setPreferredAgent] = useState<AgentKind | null>(null);
  const agent = agentFilter !== 'all' && activeAgents[agentFilter]
    ? agentFilter
    : preferredAgent && agents.includes(preferredAgent) ? preferredAgent : agents[0];

  if (!agent) {
    return <p className="text-[13px] text-[var(--text-secondary)]">Enable an agent in Settings to inspect its context.</p>;
  }

  const skills = snapshot.skills.flatMap<ContextEntry>((skill) => {
    const paths = skill.installedPaths.filter((entry) => entry.agent === agent);
    if (!paths.length) return [];
    const valid = paths.filter((entry) => !entry.broken);
    const multipleTargets = new Set(valid.map((entry) => entry.target)).size > 1;
    return [{
      id: skill.id,
      name: skill.name,
      status: valid.length ? multipleTargets ? 'Multiple sources' : 'Listed · content on demand' : 'Broken link',
      tokens: multipleTargets ? undefined : valid[0]?.upfrontTokens ?? undefined,
      sources: paths.map((entry) => ({
        path: entry.path,
        label: [scopeLabel[entry.scope], entry.isSymlink && 'Symlink', entry.isLegacy && 'Legacy'].filter(Boolean).join(' · '),
        detail: entry.broken ? `Missing target: ${entry.target}` : entry.isSymlink ? `Target: ${entry.target}` : undefined,
      })),
    }];
  });

  const rules = snapshot.rules.flatMap<ContextEntry>((rule) =>
    rule.installations.filter((entry) => entry.agent === agent).map((entry) => ({
      id: `${entry.scope}:${entry.path}`,
      name: entry.path.split('/').at(-1) ?? rule.filename,
      status: entry.error ? 'Needs attention'
        : entry.trigger === 'alwaysOn' ? 'Startup'
        : entry.trigger === 'modelDecision' ? 'Model decides' : 'Matching files',
      tokens: entry.error || entry.trigger === 'glob' ? undefined : entry.upfrontTokens,
      sources: [{
        path: entry.path,
        label: `${scopeLabel[entry.scope]}${entry.isSymlink ? ' · Symlink' : ''}`,
        detail: [entry.error, entry.globPattern && `Pattern: ${entry.globPattern}`,
          entry.isSymlink && rule.canonicalPath !== entry.path && `Target: ${rule.canonicalPath}`].filter(Boolean).join(' · '),
      }],
    })),
  );

  const servers = snapshot.mcpServers.flatMap<ContextEntry>((server) => {
    const definitions = server.definitions.filter((definition) => definition.agent === agent);
    if (!definitions.length) return [];
    const enabled = definitions.filter((definition) => definition.enabled);
    const globalWins = agent === 'antigravity'
      && enabled.some((definition) => definition.scope === 'global');
    return [{
      id: server.name,
      name: server.name,
      status: enabled.length === 0 ? 'Disabled'
        : server.isEstimated ? 'Configured · schema estimated' : 'Configured · cached schema',
      tokens: enabled.length > 0 ? server.upfrontTokens : undefined,
      sources: definitions.map((definition) => ({
        path: definition.path,
        label: `${scopeLabel[definition.scope]}${definition.enabled ? '' : ' · Disabled'}`,
        detail: definition.enabled && globalWins && definition.scope === 'project'
          ? 'The global definition takes precedence.' : undefined,
      })),
    }];
  });

  const plugins = snapshot.plugins
    .filter((plugin) => plugin.enabled && plugin.agent === agent)
    .map<ContextEntry>((plugin) => ({
      id: plugin.id,
      name: plugin.name,
      status: 'Enabled',
      sources: [{
        path: plugin.path,
        label: 'Plugin',
        detail: [
          plugin.skills.length > 0 && `Skills: ${plugin.skills.map((skill) => skill.name).join(', ')}`,
          plugin.ruleNames.length > 0 && `Rules: ${plugin.ruleNames.join(', ')}`,
          plugin.mcpNames.length > 0 && `MCP: ${plugin.mcpNames.join(', ')}`,
          plugin.agentNames.length > 0 && `Agents: ${plugin.agentNames.join(', ')}`,
        ].filter(Boolean).join(' · '),
      }, ...(plugin.configPath ? [{ path: plugin.configPath, label: 'Config' }] : [])],
    }));

  return (
    <section aria-labelledby="effective-context-heading" className="space-y-5">
      <div className="space-y-1">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <h2 id="effective-context-heading" className="text-[15px] font-semibold">{agentDisplayName(agent)} Context</h2>
          {agentFilter === 'all' && agents.length > 1 && (
            <MacSelect aria-label="Context agent" value={agent} size="sm"
              onChange={(event) => setPreferredAgent(agents.find((option) => option === event.target.value) ?? null)}>
              {agents.map((option) => <option key={option} value={option}>{agentDisplayName(option)}</option>)}
            </MacSelect>
          )}
        </div>
        <p className="text-[12px] text-[var(--text-secondary)]">
          Based on the files AgentDock can scan for a new session. Token counts are estimates.
        </p>
      </div>
      <ContextGroup title="Rules" entries={rules} />
      <ContextGroup title="Skills" entries={skills} />
      <ContextGroup title="MCP Servers" entries={servers} />
      <ContextGroup title="Plugins" entries={plugins} />
    </section>
  );
}

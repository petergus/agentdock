# AgentDock

**A native macOS context switchboard for AI coding agents.**

Manage **Skills**, **MCP Servers**, **Rules**, **Plugins**, **Reusable Sets**, and **Support Docs** across **Google Antigravity**, **OpenAI Codex**, and **Claude Code** from a single, fast macOS window—with live startup token budgets and zero-copy symlinks.

[![macOS](https://img.shields.io/badge/macOS-13.0%2B-000000?logo=apple&logoColor=white)](https://github.com/petergus/agentdock/releases)
[![Tauri v2](https://img.shields.io/badge/Tauri-v2-FFC131?logo=tauri&logoColor=white)](https://v2.tauri.app/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

<!-- Drop your main app screenshot at docs/images/app-screenshot.png -->
![AgentDock Screenshot](docs/images/app-screenshot.png)

---

## Before & After

<!-- Drop your before/after images at docs/images/before.png and docs/images/after.png -->
| Before | After |
| :---: | :---: |
| ![Before AgentDock](docs/images/before.png) | ![After AgentDock](docs/images/after.png) |

---

## Download

Grab the latest `.dmg` from [**GitHub Releases**](https://github.com/petergus/agentdock/releases).

> **Note for unsigned builds:** If macOS Gatekeeper warns that the app is from an unidentified developer, right-click `AgentDock.app` → **Open**, or run:
> ```bash
> xattr -cr /Applications/AgentDock.app
> ```

---

## Why AgentDock?

AI coding agents load rules, skills, and MCP tool schemas into their startup system prompt. Without visibility, global folders accumulate duplicate skills, forgotten MCP servers, and bloated instructions—silently burning context before you type a single prompt.

AgentDock treats your agent setup like a **switchboard, not an IDE**:

- **Zero-Copy Single Source of Truth:** Keep canonical skills and reusable rules in your own Library folders (including an **Obsidian vault** synced via iCloud/Dropbox). Flipping a switch **ON** creates a symlink; flipping it **OFF** removes only the symlink.
- **Instant Startup Token Budgets:** Uses `tiktoken-rs` (`cl100k_base`) to track live **Rules** and **Skills & MCP** token overhead against a 20,000-token budget, with `Global + Project` breakdowns.
- **Cross-Agent Deduplication:** Supports **Google Antigravity**, **OpenAI Codex**, and **Claude Code** side by side with per-agent context views and project health diagnostics.
- **Non-Destructive & Git-Clean:** Never deletes real (non-symlink) files on toggle-off, preserves comments and unknown keys in `config.toml` / JSON configs, creates rolling backups before writes, and hides managed symlinks in `.git/info/exclude` (never touching `.gitignore`).

---

## Key Features

- **Global & Per-Project Switchboard:** Switch between `Global` and any discovered or pinned project in a translucent macOS vibrancy sidebar (`⌘K` quick filter, tags, and hidden-project controls).
- **Skills & Global De-Bloat:** Toggle symlinked skills per scope, inspect upfront vs. on-demand token costs, and move unmanaged real skill folders out of global agent directories into your Library in one click.
- **MCP Catalog & Live Tool-Schema Probing:** Auto-discovers MCP servers across Antigravity, Codex, and Claude configs, lets you add `stdio` / `http` servers, and probes `stdio` servers (`initialize` + `tools/list`) to measure exact tool-schema token costs.
- **Rules & Support Docs:** Inspect `AGENTS.md`, `GEMINI.md`, `CLAUDE.md`, and `.agents/rules/*.md`, switch rule trigger modes (`always_on`, `model_decision`, `glob`) safely, and attach project support docs as zero-token rule pointers or local reference symlinks.
- **Reusable Sets:** Save working combinations of skills, MCP servers, and rules as named Sets, preview diffs before applying, and repair partial sets.
- **Plugins & Cleanup Audit:** Toggle Antigravity, Codex, and Claude plugins, audit broken symlinks, duplicate entries, stale caches, and instruction issues.
- **Global Quick Switch:** Pause or restore global customizations cleanly without touching unmanaged files.

---

## Supported Agent Paths

| Agent | Global | Project |
| :--- | :--- | :--- |
| **Antigravity** | `~/.gemini/AGENTS.md`, `~/.gemini/GEMINI.md`, `~/.gemini/config/rules/*.md`, `~/.gemini/config/skills/`, `~/.gemini/config/mcp_config.json` | `AGENTS.md`, `GEMINI.md`, `.agents/rules/*.md`, `.agents/skills/`, `.agents/mcp_config.json` |
| **OpenAI Codex** | `~/.codex/AGENTS.md`, `~/.agents/skills/`, `~/.codex/config.toml` | `AGENTS.md`, `.agents/skills/`, `.codex/config.toml` |
| **Claude Code** | `~/.claude/CLAUDE.md`, `~/.claude/skills/`, `~/.claude.json` | `CLAUDE.md`, `.claude/rules/*.md`, `.claude/skills/`, `.mcp.json` |

---

## Development

### Prerequisites

- **macOS 13.0+**
- **Node.js** (20+) & **pnpm**
- **Rust** (1.80+)

### Commands

```bash
# Install frontend dependencies
pnpm install

# Run in development mode (Vite + Tauri)
pnpm tauri dev

# Typecheck & build frontend
pnpm build

# Run Rust backend tests
cargo test --manifest-path src-tauri/Cargo.toml

# Build macOS .app and .dmg bundle
pnpm tauri build
```

---

## License

MIT — see [LICENSE](LICENSE).

# AgentDock — Native macOS Agent Context Switchboard

## 1. Product Goal & Design Philosophy
Build **AgentDock**, a fast, single-window macOS desktop application for managing AI coding agent customizations (**Skills, MCP Servers, Rules, and Support Docs**) across **Google Antigravity**, **OpenAI Codex**, and **Claude Code**.

### Core Principles
1. **Simple Switchboard, Not an IDE:** No menu bar / tray icon, no built-in Markdown/code editors, no API proxy or billing features. The app opens as a clean full macOS window showing exact **startup token overhead** and **ON/OFF switches** for **Global** and **Per-Project** scopes.
2. **Zero-Copy Single Source of Truth (Obsidian-First):** Canonical skills and reusable rules live in user-defined Library folders (including an **Obsidian vault** synced via iCloud/Dropbox). Flipping a switch ON creates a **symlink**; flipping it OFF removes only the symlink.
3. **Instant Token Visibility:** Always display live startup system-prompt token budgets so global and project sessions stay well below Antigravity's 20,000-token budgets.
4. **Non-Destructive & Git-Clean:** Never delete real (non-symlink) files on toggle-off. Preserve comments/unknown keys in all config files, keep rolling backups before writes, and optionally hide managed project symlinks via `.git/info/exclude` (never modifying `.gitignore`).

---

## 2. Tech Stack & Native macOS Visual Architecture

### Stack
* **Desktop Shell & Backend:** **Tauri v2** (Rust 1.80+)
* **Frontend:** **React 19 + TypeScript + Vite + Tailwind CSS v4 + shadcn/ui + Lucide Icons**
* **Package Manager:** `pnpm`

### Key Rust Crates (`src-tauri/Cargo.toml`)
* `tauri` (v2, with `macos-private-api` if needed for seamless vibrancy), `tauri-plugin-dialog`, `tauri-plugin-opener`
* `window-vibrancy` (or Tauri v2 native `windowEffects` with `Sidebar` material)
* `serde`, `serde_json` — JSON config parsing and key-preserving updates
* `toml_edit` — Comment- and formatting-preserving edits for Codex `config.toml`
* `serde_yaml` (or lightweight YAML frontmatter parser) — `SKILL.md` and `.agents/rules/*.md` frontmatter parsing
* `tiktoken-rs` — Accurate local `cl100k_base` token counting
* `notify` — Native macOS FSEvents filesystem watching for live UI refresh
* `walkdir`, `dirs` — Fast directory traversal and home-path resolution

### Native macOS Look & Feel Requirements
Design the UI to feel indistinguishable from a modern Apple / Swiss-minimalist macOS utility (similar to Linear, Things, or macOS System Settings):
* **Window Chrome:** Configure `tauri.conf.json` with `"titleBarStyle": "Overlay"`, `"hiddenTitle": true`, `"transparent": true`, minimum size `960x640`, default size `1180x760`. Place macOS traffic-light padding (`pl-[78px]`) in the top sidebar header and mark top drag bars with `data-tauri-drag-region`.
* **Sidebar Material:** Apply native macOS sidebar vibrancy (`Effect::Sidebar`) behind the translucent left sidebar, with an opaque system-adaptive content pane on the right (plus an automatic opaque fallback when macOS Reduce Transparency is enabled).
* **Typography & Color Tokens:**
  * Font stack: `-apple-system, BlinkMacSystemFont, "SF Pro Text", "SF Pro Display", sans-serif` (and `"SF Mono", ui-monospace` for token counts and file paths).
  * Automatic macOS Light & Dark mode (`prefers-color-scheme`) with subtle 1px hairline dividers (`border-black/10 dark:border-white/10`), native-proportioned macOS toggle switches, and compact row density (36–40px rows).
  * No heavy drop shadows, no neon gradients, no marketing copy.

---

## 3. Verified Agent Filesystem Adapter Map

Isolate all filesystem paths behind a clean Rust `AgentAdapter` trait supporting **Antigravity**, **Codex**, and **Claude Code**.

| Agent | Global Rules | Global Skills | Global MCP | Project Rules | Project Skills | Project MCP |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **Antigravity** | `~/.gemini/AGENTS.md`, `~/.gemini/GEMINI.md`, `~/.gemini/config/AGENTS.md`, `~/.gemini/config/rules/*.md` | **Primary:** `~/.gemini/config/skills/` *(App + CLI)*<br>**Also scan legacy/CLI:** `~/.gemini/antigravity/skills/`, `~/.gemini/antigravity-cli/skills/`, `~/.gemini/skills/`, `~/.agents/skills/` | **Primary:** `~/.gemini/config/mcp_config.json`<br>**Also scan legacy:** `~/.gemini/antigravity/mcp_config.json` | `AGENTS.md`, `GEMINI.md`, `.agents/rules/*.md` *(and legacy `.agent/rules/*.md`)* | `.agents/skills/` *(and legacy `.agent/skills/`)* | `.agents/mcp_config.json` |
| **OpenAI Codex** | `~/.codex/AGENTS.md` *(and `AGENTS.override.md`)* | `~/.agents/skills/` | `~/.codex/config.toml` (`[mcp_servers.<name>]`) | `AGENTS.md` | `.agents/skills/` | `.codex/config.toml` (`[mcp_servers.<name>]`) |
| **Claude Code** | `~/.claude/CLAUDE.md` | `~/.claude/skills/` | `~/.claude.json` | `CLAUDE.md`, `.claude/rules/*.md` | `.claude/skills/` | `.mcp.json` |

### Cross-Agent Deduplication Rules
1. **Shared Project Skills Folder:** Enabling a skill on a project creates a single symlink in `<project>/.agents/skills/<skill-name>` (read natively by both **Antigravity** and **Codex**). If **Claude Code** is enabled in Settings, optionally mirror the symlink to `<project>/.claude/skills/<skill-name>`.
2. **Antigravity MCP Name Collision Guard:** Antigravity loads both global and workspace `.agents/mcp_config.json`; if both define the same server name, the global/running server wins. Show a clear `"Defined Globally"` indicator on project scope when an MCP server is already active globally.
3. **Antigravity Flat Rule Scanning:** Antigravity scans only immediate `*.md` children inside `.agents/rules/` and requires valid YAML frontmatter with snake_case `trigger` (`always_on`, `model_decision`, or `glob`). Validate this and flag broken/missing frontmatter in the UI.

---

## 4. App Layout & Single-Screen Switchboard UX

### 4.1 First-Run / Settings Sheet (Modal via Gear Icon)
Stored in `~/Library/Application Support/AgentDock/config.json`:
* **Projects Root Folder:** One local directory structured 2 levels deep as `<ProjectsRoot>/<Category>/<Project>` (e.g., `~/Projects/Clients/Acme`). Include an editable ignore list (default: `.git`, `node_modules`, `_archive`, `Archive`, `.DS_Store`) plus ability to pin individual project folders outside the root.
* **Library Sources (1 or more):** Directories containing canonical skills and reusable rules—specifically supporting **Obsidian Vault subfolders** (label each source, e.g., `Obsidian` or `Local Library`, so badges appear cleanly in the UI).
* **Support Docs Root Folder (Optional):** Default starting directory when attaching support docs to a project.
* **Active Agents:** Checkboxes for `Antigravity` (default ON), `Codex` (default ON), `Claude Code` (default OFF until enabled).
* **Git Hygiene Toggle:** `Add managed project symlinks & local agent configs to .git/info/exclude` (default ON).

### 4.2 Left Sidebar (Translucent Vibrancy)
* **Top Search / Quick Filter** (`⌘K` focuses search to jump to any project or filter items).
* **Global** pinned at top with a live right-aligned startup token badge (e.g., `2.1k`).
* **Divider + `PROJECTS` section:** Collapsible category headers matching the first-level subfolders inside `Projects Root`.
  * Each project row displays: Project Name + right-aligned effective token total (`Global + Project`, e.g., `3.4k`).
  * Clicking **Global** or any **Project** switches the right-hand switchboard immediately.
* **Bottom Footer:** Compact Settings gear button + manual Refresh button.

### 4.3 Right Main Pane (Switchboard)

#### Sticky Top Overview Header
* **Left:** Current scope title (`Global` or `Category / Project Name`) + small `Reveal in Finder` button + quick agent filter pills (`All | Antigravity | Codex | Claude`).
* **Right / Sub-bar:** Two crisp horizontal token budget bars mirroring Antigravity's limits:
  * **Rules:** `X / 20,000 tokens` (green `< 60%`, amber `60–85%`, red `> 85%`)
  * **Skills & MCP:** `Y / 20,000 tokens`
  * Hovering either bar shows the breakdown: `Global Baseline + Project Additions`.

#### Section 1: Skills
* Compact filter/search input + sort toggle (`Name` / `Highest Tokens` / `Active First`).
* **De-bloat Banner (Global scope only, shown if unmanaged real skill folders exist):**
  * Detects real directories (non-symlinks) sitting directly inside Antigravity/Codex/Claude global folders.
  * Offers a one-click **"Move to Library & Unlink"** action (with dry-run path preview) so users can safely move bloated global skill packs into their Library/Obsidian without losing them.
* **Skill Row Anatomy:**
  * **Name** (folder name / YAML `name`)
  * **Source Badge:** `Obsidian`, `Library`, `Global Only`, or `Broken Symlink` (with one-click Fix/Unlink)
  * **Upfront Token Pill:** Token count of YAML `name + description` (amber dot if description > 200 chars). Tooltip shows `Upfront: X tok | Full body (on-demand): Y tok`.
  * **Action Icon:** `Open in Finder / Default App` (opens the canonical `SKILL.md` in Obsidian or system editor—no built-in editor needed).
  * **Control:**
    * On **Global**: Toggle switch (creates/removes symlink in enabled agents' global skill directories).
    * On **Project**: If already active globally, displays subtle `Global` badge; otherwise shows a Toggle Switch (creates/removes symlink in `<project>/.agents/skills/<name>`, plus `.claude/skills/` if Claude is enabled).

#### Section 2: MCP Servers
* Auto-populates a central **MCP Catalog** (`~/Library/Application Support/AgentDock/mcp-catalog.json`) by importing all servers discovered across Antigravity, Codex, and Claude configs, plus a simple `+ Add MCP` sheet (Name, Transport `stdio`/`http`, Command/URL, Args, Env var references).
* **MCP Row Anatomy:**
  * **Server Name** + subtle command/URL subtitle
  * **Token Pill:** Cached upfront tool-schema token count + small `Probe` refresh icon. Clicking `Probe` briefly spawns stdio MCP servers (timeout 5s), sends JSON-RPC `initialize` + `tools/list`, counts exact tokens across returned tool schemas via `tiktoken-rs`, and caches the result.
  * **Toggle Switch:**
    * On **Global**: Enables/disables the server in `~/.gemini/config/mcp_config.json` (and syncs legacy `~/.gemini/antigravity/mcp_config.json` if present), `~/.codex/config.toml`, and `~/.claude.json`.
    * On **Project**: Shows `Global` badge if inherited globally, or toggles the server in `<project>/.agents/mcp_config.json`, `<project>/.codex/config.toml`, and `<project>/.mcp.json`.

#### Section 3: Rules & Support Docs
* **Rules List:**
  * Shows all active rules for the selected scope (`AGENTS.md`, `GEMINI.md`, `CLAUDE.md`, `.agents/rules/*.md`) plus any reusable rule `.md` files found in configured Libraries/Obsidian.
  * Each row displays: **Filename**, **Trigger Pill** (`Always On`, `Model Decision`, `Glob`), **Upfront Token Cost** (`Always On` = full file; `Model Decision` = description only; `Glob` = `0 tok`), **Open in Editor** icon, and a **Toggle Switch** (for symlinkable library rules).
  * For `.agents/rules/*.md`, clicking the **Trigger Pill** allows quickly switching between `always_on`, `model_decision`, and `glob` (updating only YAML frontmatter safely).
* **Support Docs (Project Scope Only):**
  * `+ Link Support Docs` button opens native macOS file/folder picker (starting at configured Support Docs Root).
  * User picks link mode:
    1. **Zero-Token Rule Pointer (Default — Recommended):** Inserts/updates a managed block inside `<project>/AGENTS.md` (and optionally a `model_decision` `.agents/rules/support-docs.md`) bounded by `<!-- AGENTDOCK:SUPPORT_DOCS_START -->` and `<!-- AGENTDOCK:SUPPORT_DOCS_END -->`, listing each attached path + an optional 1-line description so agents read docs only when needed.
    2. **Local Reference Symlink:** Creates `<project>/_support-docs/<folder-name>` symlink and adds `_support-docs/` to `.git/info/exclude`.
  * Each attached doc shows its name, path, mode badge (`Pointer` / `Symlink`), `Reveal in Finder` icon, and a toggle/unlink button.

---

## 5. Token Estimation Specification (Rust Backend)
Use `tiktoken-rs` (`cl100k_base`) for fast, consistent local counts:
1. **Skills:**
   * **Upfront tokens** = `tokens(name) + tokens(description) + 12` (structural overhead).
   * **On-demand tokens** = `tokens(markdown_body)` + sum of `references/*.md` (displayed only on hover, NOT counted against the 20k startup bar).
2. **Rules:**
   * `AGENTS.md`, `GEMINI.md`, `CLAUDE.md`, or frontmatter `trigger: always_on` → **100% of file tokens** count toward Rules budget.
   * Frontmatter `trigger: model_decision` → **only frontmatter `description` tokens** count toward Rules budget.
   * Frontmatter `trigger: glob` (or Claude `paths:`) → **0 upfront tokens**.
3. **MCP Servers:**
   * When probed via `tools/list`, count `tokens(serde_json::to_string(&tools))`.
   * Before first probe (or if remote/unprobed), show `~800 tok (est.)` with a subtle `Probe` icon.

---

## 6. Hard Safety & Integrity Rules
1. **Strict Symlink Verification on Toggle-Off:** Before removing any skill, rule, or support doc entry, call `std::fs::symlink_metadata(&target_path)`. Only remove if `file_type().is_symlink() == true`. If it is a real directory/file, abort unlink and prompt the user with **"Move to Library & Unlink?"**.
2. **Atomic Writes & Rolling Backups:** Before modifying any config file (`mcp_config.json`, `config.toml`, `.claude.json`, `AGENTS.md`, `.git/info/exclude`), copy the existing file to `~/Library/Application Support/AgentDock/backups/<timestamp>_<filename>` (retain latest 25 snapshots) and write updates via temporary file + atomic rename.
3. **Comment- & Key-Preserving Edits:**
   * Use `toml_edit::DocumentMut` for `.codex/config.toml` so comments, profiles, and non-MCP keys are never touched.
   * Mutate only the `mcpServers` object inside JSON MCP files; preserve all other top-level keys.
4. **Symlink Resolution & Obsidian/iCloud Safety:** Resolve symlinks using `std::fs::read_link` + canonicalization, gracefully catching broken symlinks or evicted iCloud placeholders without crashing the scanner.
5. **Local-Only Git Exclusions:** When Git Hygiene is enabled and `<project>/.git` exists, manage an isolated `# BEGIN AGENTDOCK` / `# END AGENTDOCK` block inside `<project>/.git/info/exclude` for symlinks and generated local configs. Never modify `.gitignore`.

---

## 7. Phased Build Plan (3 Shippable Milestones)

Build and test each milestone sequentially before advancing:

### Milestone 1 — Native macOS Shell, Scanner & Live Token Overview (Read-Only)
* Scaffold Tauri v2 + React + TypeScript + Tailwind v4 + shadcn/ui project.
* Configure macOS overlay titlebar, native sidebar vibrancy (`Effect::Sidebar`), SF Pro typography, and light/dark mode tokens.
* Implement Settings modal (`Projects Root`, `Library Sources`, `Support Docs Root`, `Active Agents`) persisted to `~/Library/Application Support/AgentDock/config.json`.
* Implement Rust read-only scanners for all Antigravity (including legacy paths), Codex, and Claude global + 2-level categorized project folders, symlink source detection (`Obsidian` / `Library` / `Global Only`), and `tiktoken-rs` token bars.
* **Verify:** Launch via `pnpm tauri dev` and confirm the app detects existing global skills, Obsidian symlinks, MCP servers, and categorized projects with accurate token counts.

### Milestone 2 — Symlink Switchboard, Global De-bloat & Support Docs
* Implement safe symlink creation/unlinking commands in Rust with strict `is_symlink()` guards and `.git/info/exclude` hygiene.
* Wire ON/OFF switches for Skills and Library Rules in both Global and Project scopes with instant token bar recalculation.
* Add the **"Move Unmanaged Global Skills to Library"** migration sheet (with dry-run path preview).
* Implement **Support Docs** attachment on Project scope (Zero-Token managed pointer block in `AGENTS.md` / `.agents/rules/support-docs.md` AND optional `_support-docs/` symlink mode) plus rule frontmatter trigger-mode switching.
* **Verify:** Toggling a skill ON/OFF in a test project cleanly creates/removes `.agents/skills/<name>`, updates `.git/info/exclude`, never touches the Obsidian source file, and updates the token bar immediately.

### Milestone 3 — MCP Catalog, Config Writers, Tool Probing & Live Watch
* Build central MCP Catalog auto-import + `+ Add MCP` sheet.
* Implement backup-protected, key/comment-preserving writers for Antigravity (`mcp_config.json`), Codex (`config.toml` via `toml_edit`), and Claude (`.mcp.json` / `~/.claude.json`), including Global inheritance indicators.
* Add stdio MCP JSON-RPC `initialize` + `tools/list` probe command with 5-second timeout and local token caching.
* Add `notify` FSEvents file watcher for Library and agent config paths so external edits in Obsidian or CLI refresh the UI automatically.
* Final macOS polish: `⌘K` quick filter, `⌘,` settings shortcut, `⌘R` refresh, `Reveal in Finder` / `Open in Default App` row actions, and production `.app` build (`pnpm tauri build`).
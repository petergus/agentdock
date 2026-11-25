# AgentDock Roadmap

## Effective Context and Set Preview

- Implemented per-agent context and read-only Set previews. Build and 44 Rust tests passed.
- Native click-through is pending. The Tauri dev server ran, but the inspection tool could not attach to its window.

## Configuration changes and recovery

- Extend change previews to global pause and cleanup, including affected projects, conflicts, and estimated token changes.
- Add local change history and one-step undo for a batch. Detect external edits before restoring files and show which changes need manual resolution.

## Reusable Sets

- Implemented Set repair. **Partial** opens a preview of missing items and blockers. **Apply Missing Items** restores eligible entries while preserving available ones.
- Add **Compare With Project** and **Copy Set to Project** for reusing a working setup across repositories.

## Project health

- Implemented a project summary for broken links, configuration issues, and unverified MCP servers.
- Broken links open Cleanup Audit. Unverified servers link to their MCP rows, which show the last successful probe when available.
- Pending native verification of summary expansion, focus navigation, and probe status.

## Keyboard navigation

- Turn **⌘K** into a command palette that opens projects, skills, and config files.
- Include contextual actions such as **Preview Set** and **Show Project Issues**, with keyboard navigation and accessible focus behavior.

//! Pause and restore batch switching for Global scope (All Global, All Skills, All MCP).

use std::collections::BTreeSet;
use std::path::Path;

use crate::config;
use crate::error::AppError;
use crate::mcp_writer;
use crate::models::{
    AgentKind, AppConfig, BatchToggleResult, DesiredState, GlobalSwitchTarget, PausedPluginRef,
    PausedRuleRef, PausedSkillRef, ToggleOutcome, ToggleScope,
};
use crate::plugins;
use crate::scanner;
use crate::symlinks;

/// Pauses or restores Global skills, MCP servers, and/or library rules without deleting real files.
///
/// When pausing (`enable == false`), captures the currently active global items into
/// `cfg.global_pause` before disabling them, so flipping back ON (`enable == true`) restores
/// the exact previous configuration.
///
/// # Errors
/// Returns [`AppError`] if the MCP catalog path or filesystem operations fail.
pub fn set_global_switch(
    cfg: &mut AppConfig,
    home: &Path,
    target: GlobalSwitchTarget,
    enable: bool,
    catalog_path: Option<&Path>,
    custom_backups_root: Option<&Path>,
) -> Result<BatchToggleResult, AppError> {
    let resolved_catalog = match catalog_path {
        Some(p) => p.to_path_buf(),
        None => config::mcp_catalog_path()?,
    };

    let mut changed_count = 0usize;
    let mut skipped_unmanaged = Vec::new();
    let mut warnings = Vec::new();

    let include_skills = matches!(target, GlobalSwitchTarget::All | GlobalSwitchTarget::Skills);
    let include_plugins = matches!(target, GlobalSwitchTarget::Plugins);
    let include_mcp = matches!(target, GlobalSwitchTarget::All | GlobalSwitchTarget::Mcp);
    let include_rules = matches!(target, GlobalSwitchTarget::All);

    if !enable {
        let global = scanner::scan_global_only_for_home(cfg, home);

        if include_skills {
            let mut newly_paused = Vec::new();
            for skill in global.skills.iter().filter(|s| s.active_globally) {
                let outcome = symlinks::toggle_skill(
                    cfg,
                    home,
                    &ToggleScope::Global,
                    &skill.id,
                    Path::new(&skill.skill_dir_path),
                    DesiredState::Disable,
                )?;
                match outcome {
                    ToggleOutcome::Updated { .. } => {
                        changed_count += 1;
                        newly_paused.push(PausedSkillRef {
                            skill_id: skill.id.clone(),
                            canonical_skill_dir: skill.skill_dir_path.clone(),
                        });
                    }
                    ToggleOutcome::RequiresMoveToLibrary { skill_id, .. } => {
                        warnings.push(format!(
                            "Skipped unmanaged skill '{skill_id}' (real directory, not a symlink)"
                        ));
                        skipped_unmanaged.push(skill_id);
                    }
                }
            }

            if cfg.global_pause.skills_paused {
                let mut seen: BTreeSet<String> = cfg
                    .global_pause
                    .paused_skills
                    .iter()
                    .map(|s| s.skill_id.clone())
                    .collect();
                for item in newly_paused {
                    if seen.insert(item.skill_id.clone()) {
                        cfg.global_pause.paused_skills.push(item);
                    }
                }
            } else {
                cfg.global_pause.paused_skills = newly_paused;
            }
            cfg.global_pause.skills_paused = true;
        }

        if include_plugins {
            let mut newly_paused = Vec::new();
            for plugin in global.plugins.iter().filter(|p| p.enabled && p.can_toggle) {
                match plugins::toggle_plugin(
                    cfg,
                    home,
                    plugin.agent,
                    &plugin.id,
                    DesiredState::Disable,
                    custom_backups_root,
                ) {
                    Ok(()) => {
                        changed_count += 1;
                        newly_paused.push(PausedPluginRef {
                            agent: plugin.agent,
                            plugin_id: plugin.id.clone(),
                        });
                    }
                    Err(err) => {
                        warnings.push(format!("Failed to pause plugin '{}': {err}", plugin.id));
                    }
                }
            }

            if cfg.global_pause.plugins_paused {
                let mut seen: BTreeSet<(AgentKind, String)> = cfg
                    .global_pause
                    .paused_plugins
                    .iter()
                    .map(|p| (p.agent, p.plugin_id.clone()))
                    .collect();
                for item in newly_paused {
                    if seen.insert((item.agent, item.plugin_id.clone())) {
                        cfg.global_pause.paused_plugins.push(item);
                    }
                }
            } else {
                cfg.global_pause.paused_plugins = newly_paused;
            }
            cfg.global_pause.plugins_paused = true;
        }

        if include_mcp {
            let mut newly_paused = Vec::new();
            for server in global.mcp_servers.iter().filter(|m| m.active_globally) {
                match mcp_writer::toggle_mcp_server_with_paths(
                    cfg,
                    home,
                    &ToggleScope::Global,
                    &server.name,
                    DesiredState::Disable,
                    &resolved_catalog,
                    custom_backups_root,
                ) {
                    Ok(()) => {
                        changed_count += 1;
                        newly_paused.push(server.name.clone());
                    }
                    Err(err) => {
                        warnings.push(format!("Failed to pause MCP '{}': {err}", server.name));
                    }
                }
            }

            if cfg.global_pause.mcp_paused {
                let mut seen: BTreeSet<String> =
                    cfg.global_pause.paused_mcp.iter().cloned().collect();
                for name in newly_paused {
                    if seen.insert(name.clone()) {
                        cfg.global_pause.paused_mcp.push(name);
                    }
                }
            } else {
                cfg.global_pause.paused_mcp = newly_paused;
            }
            cfg.global_pause.mcp_paused = true;
        }

        if include_rules {
            let mut newly_paused = Vec::new();
            for rule in global
                .rules
                .iter()
                .filter(|r| r.active_globally && (r.is_library_rule || r.is_symlink))
            {
                let outcome = symlinks::toggle_library_rule(
                    cfg,
                    home,
                    &ToggleScope::Global,
                    &rule.filename,
                    Path::new(&rule.canonical_path),
                    DesiredState::Disable,
                )?;
                match outcome {
                    ToggleOutcome::Updated { .. } => {
                        changed_count += 1;
                        newly_paused.push(PausedRuleRef {
                            rule_filename: rule.filename.clone(),
                            canonical_rule_file: rule.canonical_path.clone(),
                        });
                    }
                    ToggleOutcome::RequiresMoveToLibrary { skill_id, .. } => {
                        warnings.push(format!("Skipped non-symlink rule '{skill_id}'"));
                    }
                }
            }

            if cfg.global_pause.rules_paused {
                let mut seen: BTreeSet<String> = cfg
                    .global_pause
                    .paused_rules
                    .iter()
                    .map(|r| r.rule_filename.clone())
                    .collect();
                for item in newly_paused {
                    if seen.insert(item.rule_filename.clone()) {
                        cfg.global_pause.paused_rules.push(item);
                    }
                }
            } else {
                cfg.global_pause.paused_rules = newly_paused;
            }
            cfg.global_pause.rules_paused = true;
        }
    } else {
        if include_skills {
            let saved = std::mem::take(&mut cfg.global_pause.paused_skills);
            for entry in saved {
                let dir = Path::new(&entry.canonical_skill_dir);
                if !dir.is_dir() {
                    warnings.push(format!(
                        "Skipped restoring skill '{}': directory missing ({})",
                        entry.skill_id, entry.canonical_skill_dir
                    ));
                    continue;
                }
                match symlinks::toggle_skill(
                    cfg,
                    home,
                    &ToggleScope::Global,
                    &entry.skill_id,
                    dir,
                    DesiredState::Enable,
                ) {
                    Ok(ToggleOutcome::Updated { .. }) => changed_count += 1,
                    Ok(ToggleOutcome::RequiresMoveToLibrary { skill_id, .. }) => {
                        warnings.push(format!(
                            "Cannot restore skill '{skill_id}': real directory already exists at target"
                        ));
                    }
                    Err(err) => {
                        warnings.push(format!(
                            "Failed to restore skill '{}': {err}",
                            entry.skill_id
                        ));
                    }
                }
            }
            cfg.global_pause.skills_paused = false;
        }

        if include_plugins {
            let saved = std::mem::take(&mut cfg.global_pause.paused_plugins);
            for entry in saved {
                match plugins::toggle_plugin(
                    cfg,
                    home,
                    entry.agent,
                    &entry.plugin_id,
                    DesiredState::Enable,
                    custom_backups_root,
                ) {
                    Ok(()) => changed_count += 1,
                    Err(err) => {
                        warnings.push(format!(
                            "Failed to restore plugin '{}': {err}",
                            entry.plugin_id
                        ));
                    }
                }
            }
            cfg.global_pause.plugins_paused = false;
        }

        if include_mcp {
            let saved = std::mem::take(&mut cfg.global_pause.paused_mcp);
            for name in saved {
                match mcp_writer::toggle_mcp_server_with_paths(
                    cfg,
                    home,
                    &ToggleScope::Global,
                    &name,
                    DesiredState::Enable,
                    &resolved_catalog,
                    custom_backups_root,
                ) {
                    Ok(()) => changed_count += 1,
                    Err(err) => warnings.push(format!("Failed to restore MCP '{name}': {err}")),
                }
            }
            cfg.global_pause.mcp_paused = false;
        }

        if include_rules {
            let saved = std::mem::take(&mut cfg.global_pause.paused_rules);
            for entry in saved {
                let file = Path::new(&entry.canonical_rule_file);
                if !file.is_file() {
                    warnings.push(format!(
                        "Skipped restoring rule '{}': file missing ({})",
                        entry.rule_filename, entry.canonical_rule_file
                    ));
                    continue;
                }
                match symlinks::toggle_library_rule(
                    cfg,
                    home,
                    &ToggleScope::Global,
                    &entry.rule_filename,
                    file,
                    DesiredState::Enable,
                ) {
                    Ok(ToggleOutcome::Updated { .. }) => changed_count += 1,
                    Ok(ToggleOutcome::RequiresMoveToLibrary { skill_id, .. }) => {
                        warnings.push(format!(
                            "Cannot restore rule '{skill_id}': real file already exists at target"
                        ));
                    }
                    Err(err) => {
                        warnings.push(format!(
                            "Failed to restore rule '{}': {err}",
                            entry.rule_filename
                        ));
                    }
                }
            }
            cfg.global_pause.rules_paused = false;
        }
    }

    Ok(BatchToggleResult {
        changed_count,
        skipped_unmanaged,
        warnings,
        pause_state: cfg.global_pause.clone(),
    })
}

/// Reconciles `cfg.global_pause` when a user manually re-enables an item in Global scope.
pub fn clear_paused_skill(cfg: &mut AppConfig, skill_id: &str) {
    cfg.global_pause
        .paused_skills
        .retain(|s| s.skill_id != skill_id);
    if cfg.global_pause.paused_skills.is_empty() {
        cfg.global_pause.skills_paused = false;
    }
}

/// Reconciles `cfg.global_pause` when a user manually re-enables an MCP server in Global scope.
pub fn clear_paused_mcp(cfg: &mut AppConfig, server_name: &str) {
    cfg.global_pause.paused_mcp.retain(|n| n != server_name);
    if cfg.global_pause.paused_mcp.is_empty() {
        cfg.global_pause.mcp_paused = false;
    }
}

/// Reconciles `cfg.global_pause` when a user manually re-enables a library rule in Global scope.
pub fn clear_paused_rule(cfg: &mut AppConfig, rule_filename: &str) {
    cfg.global_pause
        .paused_rules
        .retain(|r| r.rule_filename != rule_filename);
    if cfg.global_pause.paused_rules.is_empty() {
        cfg.global_pause.rules_paused = false;
    }
}

/// Reconciles `cfg.global_pause` when a user manually re-enables a plugin in Global scope.
pub fn clear_paused_plugin(cfg: &mut AppConfig, agent: AgentKind, plugin_id: &str) {
    cfg.global_pause
        .paused_plugins
        .retain(|p| !(p.agent == agent && p.plugin_id == plugin_id));
    if cfg.global_pause.paused_plugins.is_empty() {
        cfg.global_pause.plugins_paused = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp_catalog;
    use crate::models::{ActiveAgents, LibrarySource, McpCatalog, McpCatalogEntry, McpTransport};
    use std::collections::BTreeMap;
    use std::fs;
    use tempfile::TempDir;

    fn make_skill(dir: &Path, name: &str) {
        fs::create_dir_all(dir).expect("mkdir skill");
        fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: Desc for {name}\n---\nBody\n"),
        )
        .expect("write skill");
    }

    #[test]
    fn global_switch_pauses_and_restores_without_touching_unmanaged_dirs() {
        let tmp = TempDir::new().expect("temp dir");
        let root = tmp.path();
        let home = root.join("home");
        let lib = root.join("Library");
        let catalog_path = root.join("mcp-catalog.json");
        let backups_root = root.join("backups");

        make_skill(&lib.join("managed-skill"), "managed-skill");
        // Real unmanaged skill sitting directly in global agent folder
        make_skill(
            &home.join(".gemini/config/skills/real-unmanaged"),
            "real-unmanaged",
        );
        // Installed Antigravity plugin
        let demo_plugin = home.join(".gemini/config/plugins/demo-plugin");
        fs::create_dir_all(&demo_plugin).expect("mkdir plugin");
        fs::write(
            demo_plugin.join("plugin.json"),
            r#"{"name":"demo-plugin","displayName":"Demo Plugin"}"#,
        )
        .expect("write plugin manifest");
        make_skill(
            &demo_plugin.join("skills/demo-plugin-skill"),
            "demo-plugin-skill",
        );
        // Installed Codex plugin
        fs::create_dir_all(home.join(".codex")).expect("mkdir codex");
        fs::write(
            home.join(".codex/config.toml"),
            "[plugins.\"demo-codex@openai-bundled\"]\nenabled = true\n",
        )
        .expect("write codex config");

        let mut cfg = AppConfig {
            library_sources: vec![LibrarySource {
                id: "lib".to_owned(),
                label: "Lib".to_owned(),
                path: lib.to_string_lossy().into_owned(),
                depth: 2,
            }],
            active_agents: ActiveAgents {
                antigravity: true,
                codex: true,
                claude: false,
            },
            ..AppConfig::default()
        };

        // Enable managed-skill globally
        symlinks::toggle_skill(
            &cfg,
            &home,
            &ToggleScope::Global,
            "managed-skill",
            &lib.join("managed-skill"),
            DesiredState::Enable,
        )
        .expect("enable managed");

        // Seed MCP server in catalog & enable globally
        let mut catalog = McpCatalog::default();
        catalog.servers.insert(
            "demo-mcp".to_owned(),
            McpCatalogEntry {
                name: "demo-mcp".to_owned(),
                transport: McpTransport::Stdio,
                command: Some("npx".to_owned()),
                args: vec!["-y".to_owned(), "demo".to_owned()],
                url: None,
                env: BTreeMap::new(),
                headers: BTreeMap::new(),
                upfront_tokens: Some(400),
                tool_count: None,
                probed_at_ms: None,
            },
        );
        mcp_catalog::save_catalog_to(&catalog, &catalog_path, Some(&backups_root))
            .expect("save catalog");
        mcp_writer::toggle_mcp_server_with_paths(
            &cfg,
            &home,
            &ToggleScope::Global,
            "demo-mcp",
            DesiredState::Enable,
            &catalog_path,
            Some(&backups_root),
        )
        .expect("enable mcp");

        // Pause All Global
        let pause_res = set_global_switch(
            &mut cfg,
            &home,
            GlobalSwitchTarget::All,
            false,
            Some(&catalog_path),
            Some(&backups_root),
        )
        .expect("pause all");

        assert!(cfg.global_pause.skills_paused);
        assert!(!cfg.global_pause.plugins_paused);
        assert!(cfg.global_pause.mcp_paused);
        assert!(cfg.global_pause.paused_plugins.is_empty());
        assert_eq!(pause_res.skipped_unmanaged, vec!["real-unmanaged"]);
        assert!(home
            .join(".gemini/config/skills/real-unmanaged/SKILL.md")
            .is_file());
        assert!(!home.join(".gemini/config/skills/managed-skill").exists());

        let snap_paused = scanner::scan_global_only_for_home(&cfg, &home);
        assert!(
            snap_paused
                .plugins
                .iter()
                .find(|p| p.id == "demo-plugin")
                .expect("plugin")
                .enabled
        );
        assert!(
            snap_paused
                .plugins
                .iter()
                .find(|p| p.id == "demo-codex@openai-bundled")
                .expect("codex plugin")
                .enabled
        );
        assert!(
            !snap_paused
                .mcp_servers
                .iter()
                .find(|m| m.name == "demo-mcp")
                .expect("mcp")
                .active_globally
        );

        // Restore All Global
        let restore_res = set_global_switch(
            &mut cfg,
            &home,
            GlobalSwitchTarget::All,
            true,
            Some(&catalog_path),
            Some(&backups_root),
        )
        .expect("restore all");

        assert!(!cfg.global_pause.skills_paused);
        assert!(!cfg.global_pause.plugins_paused);
        assert!(!cfg.global_pause.mcp_paused);
        assert_eq!(restore_res.changed_count, 2);
        assert!(home.join(".gemini/config/skills/managed-skill").exists());

        let snap_restored = scanner::scan_global_only_for_home(&cfg, &home);
        assert!(
            snap_restored
                .plugins
                .iter()
                .find(|p| p.id == "demo-plugin")
                .expect("plugin")
                .enabled
        );
        assert!(
            snap_restored
                .plugins
                .iter()
                .find(|p| p.id == "demo-codex@openai-bundled")
                .expect("codex plugin")
                .enabled
        );
        assert!(
            snap_restored
                .mcp_servers
                .iter()
                .find(|m| m.name == "demo-mcp")
                .expect("mcp")
                .active_globally
        );
    }
}

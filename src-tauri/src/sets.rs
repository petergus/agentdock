//! Reusable Context Sets with read-only previews and overlap protection.

use std::collections::{hash_map::DefaultHasher, BTreeMap, BTreeSet};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::adapters::enabled_adapters;
use crate::config;
use crate::error::AppError;
use crate::mcp_catalog;
use crate::mcp_writer;
use crate::models::{
    AgentKind, AppConfig, DesiredState, ScopeSnapshot, SetToggleResult, ToggleOutcome, ToggleScope,
};
use crate::{scanner, symlinks};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum ItemKind {
    Skill,
    Mcp,
    Rule,
}

#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Action {
    CreateLink,
    ReplaceLink,
    RemoveLink,
    WriteConfig,
    RemoveConfigEntry,
}

#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileChange {
    action: Action,
    path: PathBuf,
    target: Option<PathBuf>,
    agents: Vec<AgentKind>,
}

#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Status {
    Change,
    AlreadyMatches,
    Shared,
    Blocked,
}

#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetItemPreview {
    kind: ItemKind,
    name: String,
    status: Status,
    detail: Option<String>,
    files: Vec<FileChange>,
    estimated_tokens: Option<usize>,
}

#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentImpact {
    agent: AgentKind,
    estimated_delta: i64,
    has_uncertain_changes: bool,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetPreview {
    pub name: String,
    membership_changes: bool,
    already_active: bool,
    items: Vec<SetItemPreview>,
    git_exclude_path: Option<PathBuf>,
    agent_impact: Vec<AgentImpact>,
    pub revision: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SetOperation {
    Apply,
    Disable,
    Repair,
}

pub fn preview_project_set(
    cfg: &AppConfig,
    home: &Path,
    project: &Path,
    set_id: &str,
    operation: SetOperation,
    catalog_path: &Path,
) -> Result<SetPreview, AppError> {
    if !project.is_dir() {
        return Err(AppError::InvalidPath(format!(
            "Project folder does not exist: {}",
            project.display()
        )));
    }
    let enable = operation != SetOperation::Disable;
    let set = cfg
        .sets
        .iter()
        .find(|set| set.id == set_id)
        .ok_or_else(|| AppError::InvalidPath(format!("Context set not found: {set_id}")))?;
    let key = config::canonical_path_key(home, &project.to_string_lossy());
    let active = cfg.project_active_sets.get(&key);
    let already_active = active.is_some_and(|ids| ids.iter().any(|id| id == set_id));
    if operation == SetOperation::Repair && !already_active {
        return Err(AppError::InvalidPath(
            "Activate this Set before repairing it.".to_owned(),
        ));
    }
    let others: Vec<_> = cfg
        .sets
        .iter()
        .filter(|other| other.id != set_id && active.is_some_and(|ids| ids.contains(&other.id)))
        .collect();
    let library = scanner::scan_libraries_for_home(cfg, home);
    let catalog = mcp_catalog::load_catalog_from(catalog_path)?;
    let mut snapshot = (operation == SetOperation::Repair)
        .then(|| scanner::scan_project_with_catalog(cfg, home, project, "", &library, &catalog));
    let scope = ToggleScope::Project {
        project_path: project.to_string_lossy().into_owned(),
    };
    let adapters = enabled_adapters(cfg.active_agents);
    let git_exclude_path = cfg
        .git_hygiene
        .then(|| crate::git_hygiene::resolve_git_exclude_path(project))
        .flatten();
    let mut items = Vec::new();
    let mut revision = DefaultHasher::new();
    set_id.hash(&mut revision);
    operation.hash(&mut revision);
    key.hash(&mut revision);
    already_active.hash(&mut revision);
    set.name.hash(&mut revision);
    git_exclude_path.hash(&mut revision);

    for (kind, names) in [
        (ItemKind::Skill, &set.skill_ids),
        (ItemKind::Rule, &set.rule_filenames),
        (ItemKind::Mcp, &set.mcp_names),
    ] {
        for name in names.iter().collect::<BTreeSet<_>>() {
            if snapshot
                .as_ref()
                .is_some_and(|snapshot| item_available(snapshot, kind, name))
            {
                continue;
            }
            let mut item = SetItemPreview {
                kind,
                name: name.clone(),
                status: Status::AlreadyMatches,
                detail: None,
                files: Vec::new(),
                estimated_tokens: None,
            };
            let shared: Vec<_> = others
                .iter()
                .filter(|other| match kind {
                    ItemKind::Skill => other.skill_ids.contains(name),
                    ItemKind::Rule => other.rule_filenames.contains(name),
                    ItemKind::Mcp => other.mcp_names.contains(name),
                })
                .map(|other| other.name.as_str())
                .collect();

            if !enable && !shared.is_empty() {
                item.status = Status::Shared;
                item.detail = Some(format!("Used by {}", shared.join(", ")));
                items.push(item);
                continue;
            }

            let changes: Result<Vec<FileChange>, AppError> = if kind == ItemKind::Mcp {
                let entry = catalog.servers.get(name);
                item.estimated_tokens = entry
                    .and_then(|entry| entry.upfront_tokens)
                    .or(Some(crate::models::DEFAULT_MCP_ESTIMATE_TOKENS));
                if enable && entry.is_none() {
                    Err(AppError::InvalidPath(
                        "Server is missing from the MCP catalog".to_owned(),
                    ))
                } else {
                    mcp_writer::project_mcp_edits(
                        cfg,
                        home,
                        project,
                        name,
                        entry.filter(|_| enable),
                    )
                    .map(|edits| {
                        edits
                            .into_iter()
                            .map(|edit| {
                                fs::read(&edit.path).ok().hash(&mut revision);
                                edit.content.hash(&mut revision);
                                FileChange {
                                    action: if enable {
                                        Action::WriteConfig
                                    } else {
                                        Action::RemoveConfigEntry
                                    },
                                    path: edit.path,
                                    target: None,
                                    agents: vec![edit.agent],
                                }
                            })
                            .collect()
                    })
                }
            } else {
                let source = match kind {
                    ItemKind::Skill => library
                        .skills
                        .get(name)
                        .map(|s| (s.canonical_dir.as_path(), s.upfront_tokens)),
                    ItemKind::Rule => library
                        .rules
                        .values()
                        .find(|r| &r.filename == name)
                        .map(|r| (r.canonical_path.as_path(), r.upfront_tokens)),
                    ItemKind::Mcp => unreachable!(),
                };
                item.estimated_tokens = source.map(|(_, tokens)| tokens);
                if enable && source.is_none() {
                    Err(AppError::InvalidPath(
                        "Item is missing from the configured libraries".to_owned(),
                    ))
                } else {
                    let target = source.filter(|_| enable).map(|(path, _)| path);
                    symlinks::preview_set_links(
                        cfg,
                        home,
                        &scope,
                        name,
                        kind == ItemKind::Rule,
                        target,
                    )
                    .map(|links| {
                        links
                            .into_iter()
                            .map(|(path, old)| {
                                old.hash(&mut revision);
                                let agents = adapters
                                    .iter()
                                    .filter(|adapter| {
                                        let dirs = if kind == ItemKind::Rule {
                                            adapter.project_rule_dirs(project)
                                        } else {
                                            adapter.project_skill_dirs(project)
                                        };
                                        dirs.iter()
                                            .any(|dir| Some(dir.path.as_path()) == path.parent())
                                    })
                                    .map(|adapter| adapter.kind())
                                    .collect();
                                FileChange {
                                    action: match (enable, old.is_some()) {
                                        (true, true) => Action::ReplaceLink,
                                        (true, false) => Action::CreateLink,
                                        (false, _) => Action::RemoveLink,
                                    },
                                    path,
                                    target: target.map(Path::to_path_buf).or(old),
                                    agents,
                                }
                            })
                            .collect()
                    })
                }
            };
            match changes {
                Ok(files) if !files.is_empty() => {
                    item.status = Status::Change;
                    item.files = files;
                    if operation == SetOperation::Repair {
                        item.detail = Some(
                            match kind {
                                ItemKind::Skill => "No working skill installation was found.",
                                ItemKind::Mcp => "No enabled server definition was found.",
                                ItemKind::Rule => "No usable rule installation was found.",
                            }
                            .to_owned(),
                        );
                    }
                }
                Ok(_) if operation == SetOperation::Repair => {
                    item.status = Status::Blocked;
                    item.detail = Some("No applicable file change was found. Check the item’s source and enabled agents.".to_owned());
                }
                Ok(_) => {}
                Err(err) => {
                    item.status = Status::Blocked;
                    item.detail = Some(err.to_string());
                }
            }
            items.push(item);
        }
    }

    let agent_impact = if items.iter().any(|item| item.status == Status::Change) {
        let snapshot = snapshot.get_or_insert_with(|| {
            scanner::scan_project_with_catalog(cfg, home, project, "", &library, &catalog)
        });
        adapters
            .iter()
            .filter_map(|adapter| estimate_impact(snapshot, &items, adapter.kind()))
            .collect()
    } else {
        Vec::new()
    };
    items.hash(&mut revision);
    agent_impact.hash(&mut revision);
    let membership_changes = already_active != enable
        && (!enable || items.iter().any(|item| item.status != Status::Blocked));
    Ok(SetPreview {
        name: set.name.clone(),
        already_active,
        membership_changes,
        items,
        git_exclude_path,
        agent_impact,
        revision: format!("{:016x}", revision.finish()),
    })
}

fn item_available(snapshot: &ScopeSnapshot, kind: ItemKind, name: &str) -> bool {
    match kind {
        ItemKind::Skill => snapshot.skills.iter().any(|skill| {
            skill.id == name && skill.installed_paths.iter().any(|entry| !entry.broken)
        }),
        ItemKind::Mcp => snapshot.mcp_servers.iter().any(|server| {
            server.name == name && server.definitions.iter().any(|entry| entry.enabled)
        }),
        ItemKind::Rule => snapshot.rules.iter().any(|rule| {
            rule.installations.iter().any(|entry| {
                entry.error.is_none()
                    && Path::new(&entry.path)
                        .file_name()
                        .is_some_and(|file| file == name)
            })
        }),
    }
}

/// Estimates the net change for newly available or removed entries. Replacements with
/// ambiguous precedence or schema cost are left out of the numeric delta.
fn estimate_impact(
    snapshot: &ScopeSnapshot,
    items: &[SetItemPreview],
    agent: AgentKind,
) -> Option<AgentImpact> {
    let mut impact = AgentImpact {
        agent,
        estimated_delta: 0,
        has_uncertain_changes: false,
    };
    let mut affected = false;

    for item in items {
        let files: Vec<_> = item
            .files
            .iter()
            .filter(|file| file.agents.contains(&agent))
            .collect();
        if files.is_empty() {
            continue;
        }
        affected = true;

        // The scanned path identifies the installation, while the target identifies
        // the context entry. A global and project link to the same source count once.
        let mut sources: BTreeMap<PathBuf, (String, Option<usize>)> = match item.kind {
            ItemKind::Skill => snapshot
                .skills
                .iter()
                .filter(|skill| skill.id == item.name)
                .flat_map(|skill| &skill.installed_paths)
                .filter(|path| path.agent == agent && !path.broken)
                .map(|path| {
                    (
                        PathBuf::from(&path.path),
                        (path.target.clone(), path.upfront_tokens),
                    )
                })
                .collect(),
            ItemKind::Rule => snapshot
                .rules
                .iter()
                .flat_map(|rule| {
                    rule.installations
                        .iter()
                        .filter(move |entry| {
                            entry.agent == agent
                                && entry.error.is_none()
                                && Path::new(&entry.path)
                                    .file_name()
                                    .is_some_and(|name| name == item.name.as_str())
                        })
                        .map(|entry| {
                            (
                                PathBuf::from(&entry.path),
                                (rule.canonical_path.clone(), Some(entry.upfront_tokens)),
                            )
                        })
                })
                .collect(),
            ItemKind::Mcp => snapshot
                .mcp_servers
                .iter()
                .filter(|server| server.name == item.name)
                .flat_map(|server| &server.definitions)
                .filter(|definition| definition.agent == agent && definition.enabled)
                .map(|definition| {
                    (
                        PathBuf::from(&definition.path),
                        (item.name.clone(), item.estimated_tokens),
                    )
                })
                .collect(),
        };
        let before = unique_sources(&sources);
        let mut uncertain = false;
        for file in files {
            match file.action {
                Action::CreateLink | Action::ReplaceLink | Action::WriteConfig => {
                    if file.action == Action::WriteConfig && sources.contains_key(&file.path) {
                        uncertain = true;
                    }
                    let key = file.target.as_ref().map_or_else(
                        || item.name.clone(),
                        |target| target.to_string_lossy().into_owned(),
                    );
                    sources.insert(file.path.clone(), (key, item.estimated_tokens));
                }
                Action::RemoveLink | Action::RemoveConfigEntry => {
                    sources.remove(&file.path);
                }
            }
        }
        let after = unique_sources(&sources);
        // Same-named skills from different targets or an MCP definition winning in
        // another scope can change what loads without changing the visible name.
        uncertain |= item.kind == ItemKind::Skill && (before.len() > 1 || after.len() > 1);
        uncertain |= item.kind == ItemKind::Mcp && !before.is_empty() && !after.is_empty();
        if uncertain {
            impact.has_uncertain_changes = true;
            continue;
        }

        for (key, tokens) in &after {
            if !before.contains_key(key) {
                update_delta(&mut impact, *tokens, 1);
            }
        }
        for (key, tokens) in &before {
            if !after.contains_key(key) {
                update_delta(&mut impact, *tokens, -1);
            }
        }
    }
    affected.then_some(impact)
}

fn unique_sources(
    sources: &BTreeMap<PathBuf, (String, Option<usize>)>,
) -> BTreeMap<String, Option<usize>> {
    sources.values().cloned().collect()
}

fn update_delta(impact: &mut AgentImpact, tokens: Option<usize>, sign: i64) {
    if let Some(amount) = tokens.and_then(|tokens| i64::try_from(tokens).ok()) {
        impact.estimated_delta = impact.estimated_delta.saturating_add(amount * sign);
    } else {
        impact.has_uncertain_changes = true;
    }
}

/// Executes the current plan. When a revision is supplied, reject stale previews before any writes.
#[expect(
    clippy::too_many_arguments,
    reason = "The existing writer accepts explicit project, catalog, and test backup paths"
)]
pub fn toggle_project_set(
    cfg: &mut AppConfig,
    home: &Path,
    project: &Path,
    set_id: &str,
    operation: SetOperation,
    catalog_path: Option<&Path>,
    custom_backups_root: Option<&Path>,
    expected_revision: Option<&str>,
) -> Result<SetToggleResult, AppError> {
    let default_catalog_path;
    let catalog_path = match catalog_path {
        Some(path) => path,
        None => {
            default_catalog_path = config::mcp_catalog_path()?;
            &default_catalog_path
        }
    };
    let preview = preview_project_set(cfg, home, project, set_id, operation, catalog_path)?;
    let enable = operation != SetOperation::Disable;
    if expected_revision.is_some_and(|revision| revision != preview.revision) {
        return Err(AppError::InvalidPath(
            "The Set or project changed. Refresh the preview before applying.".to_owned(),
        ));
    }
    let scope = ToggleScope::Project {
        project_path: project.to_string_lossy().into_owned(),
    };
    let key = config::canonical_path_key(home, &project.to_string_lossy());
    let desired = if enable {
        DesiredState::Enable
    } else {
        DesiredState::Disable
    };
    let mut applied_count = 0;
    let mut skipped_shared_count = 0;
    let mut warnings = Vec::new();
    let mut has_matching_item = false;

    for item in preview.items {
        match item.status {
            Status::Shared => {
                skipped_shared_count += 1;
                continue;
            }
            Status::AlreadyMatches => {
                has_matching_item = true;
                continue;
            }
            Status::Blocked => {
                warnings.push(format!(
                    "{}: {}",
                    item.name,
                    item.detail.as_deref().unwrap_or("Blocked")
                ));
                continue;
            }
            Status::Change => {}
        }
        let target = item
            .files
            .first()
            .and_then(|file| file.target.as_deref())
            .unwrap_or_else(|| Path::new("."));
        let result = match item.kind {
            ItemKind::Skill => {
                symlinks::toggle_skill(cfg, home, &scope, &item.name, target, desired)
            }
            ItemKind::Rule => {
                symlinks::toggle_library_rule(cfg, home, &scope, &item.name, target, desired)
            }
            ItemKind::Mcp => mcp_writer::toggle_mcp_server_with_paths(
                cfg,
                home,
                &scope,
                &item.name,
                desired,
                catalog_path,
                custom_backups_root,
            )
            .map(|()| ToggleOutcome::Updated {
                changed_paths: Vec::new(),
            }),
        };
        match result {
            Ok(ToggleOutcome::Updated { .. }) => applied_count += 1,
            Ok(ToggleOutcome::RequiresMoveToLibrary { .. }) => {
                warnings.push(format!(
                    "{}: A real file or directory occupies the destination",
                    item.name
                ));
            }
            Err(err) => warnings.push(format!("{}: {err}", item.name)),
        }
    }

    if enable && (applied_count > 0 || has_matching_item) {
        let ids = cfg.project_active_sets.entry(key.clone()).or_default();
        if !ids.iter().any(|id| id == set_id) {
            ids.push(set_id.to_owned());
        }
    } else if !enable {
        if let Some(ids) = cfg.project_active_sets.get_mut(&key) {
            ids.retain(|id| id != set_id);
            if ids.is_empty() {
                cfg.project_active_sets.remove(&key);
            }
        }
    }

    let active_set_ids = cfg
        .project_active_sets
        .get(&key)
        .cloned()
        .unwrap_or_default();
    Ok(SetToggleResult {
        applied_count,
        skipped_shared_count,
        warnings,
        active_set_ids,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp_catalog;
    use crate::models::{
        ActiveAgents, ContextSet, LibrarySource, McpCatalog, McpCatalogEntry, McpTransport,
    };
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
    fn toggling_overlapping_sets_preserves_shared_items_and_updates_git_exclude() {
        let tmp = TempDir::new().expect("temp dir");
        let root = tmp.path();
        let home = root.join("home");
        let lib = root.join("Library");
        let project = root.join("proj");
        let catalog_path = root.join("mcp-catalog.json");
        let backups_root = root.join("backups");

        fs::create_dir_all(project.join(".git/info")).expect("git");
        make_skill(&lib.join("shared-skill"), "shared-skill");
        make_skill(&lib.join("only-a-skill"), "only-a-skill");
        fs::write(
            lib.join("shared-rule.md"),
            "---\ntrigger: always_on\n---\nRule\n",
        )
        .expect("rule");

        let mut catalog = McpCatalog::default();
        catalog.servers.insert(
            "shared-mcp".to_owned(),
            McpCatalogEntry {
                name: "shared-mcp".to_owned(),
                transport: McpTransport::Stdio,
                command: Some("npx".to_owned()),
                args: vec![],
                url: None,
                env: BTreeMap::new(),
                headers: BTreeMap::new(),
                upfront_tokens: Some(300),
                tool_count: None,
                probed_at_ms: None,
            },
        );
        mcp_catalog::save_catalog_to(&catalog, &catalog_path, Some(&backups_root))
            .expect("save catalog");

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
            sets: vec![
                ContextSet {
                    id: "set-a".to_owned(),
                    name: "Set A".to_owned(),
                    description: String::new(),
                    skill_ids: vec!["shared-skill".to_owned(), "only-a-skill".to_owned()],
                    mcp_names: vec!["shared-mcp".to_owned()],
                    rule_filenames: vec!["shared-rule.md".to_owned()],
                },
                ContextSet {
                    id: "set-b".to_owned(),
                    name: "Set B".to_owned(),
                    description: String::new(),
                    skill_ids: vec!["shared-skill".to_owned()],
                    mcp_names: vec!["shared-mcp".to_owned()],
                    rule_filenames: vec![],
                },
            ],
            ..AppConfig::default()
        };

        let catalog_before = fs::read(&catalog_path).expect("read catalog");
        let preview = preview_project_set(
            &cfg,
            &home,
            &project,
            "set-a",
            SetOperation::Apply,
            &catalog_path,
        )
        .expect("preview set-a");
        assert!(preview.membership_changes);
        assert_eq!(
            preview
                .items
                .iter()
                .filter(|item| item.status == Status::Change)
                .count(),
            4
        );
        assert!(preview
            .agent_impact
            .iter()
            .all(|impact| impact.estimated_delta > 0));
        assert!(
            !project.join(".agents").exists(),
            "preview must not create paths"
        );
        assert!(cfg.project_active_sets.is_empty());
        assert_eq!(
            fs::read(&catalog_path).expect("read catalog"),
            catalog_before
        );

        let conflict = project.join(".agents/skills/shared-skill");
        fs::create_dir_all(&conflict).expect("create new conflicting path");
        let stale = toggle_project_set(
            &mut cfg,
            &home,
            &project,
            "set-a",
            SetOperation::Apply,
            Some(&catalog_path),
            Some(&backups_root),
            Some(&preview.revision),
        );
        assert!(
            stale.is_err(),
            "changed project must require another preview"
        );
        assert!(cfg.project_active_sets.is_empty());
        assert!(!project.join(".agents/skills/only-a-skill").exists());
        let blocked = preview_project_set(
            &cfg,
            &home,
            &project,
            "set-a",
            SetOperation::Apply,
            &catalog_path,
        )
        .expect("preview with conflict");
        assert!(blocked
            .items
            .iter()
            .any(|item| item.name == "shared-skill" && item.status == Status::Blocked));
        fs::remove_dir(&conflict).expect("remove fixture conflict");

        let res_a = toggle_project_set(
            &mut cfg,
            &home,
            &project,
            "set-a",
            SetOperation::Apply,
            Some(&catalog_path),
            Some(&backups_root),
            Some(&preview.revision),
        )
        .expect("enable set-a");
        assert_eq!(res_a.applied_count, 4);
        assert!(project.join(".agents/skills/shared-skill").exists());
        assert!(project.join(".agents/skills/only-a-skill").exists());
        assert!(project.join(".agents/rules/shared-rule.md").exists());

        toggle_project_set(
            &mut cfg,
            &home,
            &project,
            "set-b",
            SetOperation::Apply,
            Some(&catalog_path),
            Some(&backups_root),
            None,
        )
        .expect("enable set-b");

        let skill_path = project.join(".agents/skills/only-a-skill");
        fs::remove_file(&skill_path).expect("simulate missing Set skill");
        let config_path = project.join(".agents/mcp_config.json");
        let custom_mcp = r#"{"mcpServers":{"shared-mcp":{"command":"custom-command"}}}"#;
        fs::write(&config_path, custom_mcp).expect("customize existing server");
        let repair = preview_project_set(
            &cfg,
            &home,
            &project,
            "set-a",
            SetOperation::Repair,
            &catalog_path,
        )
        .expect("preview repair");
        assert_eq!(repair.items.len(), 1);
        assert_eq!(repair.items[0].name, "only-a-skill");
        assert_eq!(repair.items[0].status, Status::Change);
        assert!(!repair.membership_changes);
        assert!(!skill_path.exists(), "repair preview must be read-only");
        let repaired = toggle_project_set(
            &mut cfg,
            &home,
            &project,
            "set-a",
            SetOperation::Repair,
            Some(&catalog_path),
            Some(&backups_root),
            Some(&repair.revision),
        )
        .expect("repair missing item");
        assert_eq!(repaired.applied_count, 1);
        assert!(skill_path.exists());
        assert_eq!(
            fs::read_to_string(config_path).expect("read existing server"),
            custom_mcp
        );
        let complete = preview_project_set(
            &cfg,
            &home,
            &project,
            "set-a",
            SetOperation::Repair,
            &catalog_path,
        )
        .expect("preview complete Set");
        assert!(complete.items.is_empty());

        // Disabling Set A removes only-a-skill and shared-rule.md, while keeping shared-skill & shared-mcp for Set B
        let removal = preview_project_set(
            &cfg,
            &home,
            &project,
            "set-a",
            SetOperation::Disable,
            &catalog_path,
        )
        .expect("preview disabling set-a");
        assert_eq!(
            removal
                .items
                .iter()
                .filter(|item| item.status == Status::Shared)
                .count(),
            2
        );
        assert!(project.join(".agents/rules/shared-rule.md").exists());
        let off_a = toggle_project_set(
            &mut cfg,
            &home,
            &project,
            "set-a",
            SetOperation::Disable,
            Some(&catalog_path),
            Some(&backups_root),
            Some(&removal.revision),
        )
        .expect("disable set-a");
        assert_eq!(off_a.skipped_shared_count, 2);
        assert_eq!(off_a.active_set_ids, vec!["set-b"]);
        assert!(project.join(".agents/skills/shared-skill").exists());
        assert!(!project.join(".agents/skills/only-a-skill").exists());
        assert!(!project.join(".agents/rules/shared-rule.md").exists());
    }
}

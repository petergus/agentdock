//! Workspace health audit and safe cleanup actions across Global and Project scopes.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use toml_edit::DocumentMut;

use crate::backup;
use crate::config::{self, AgentRoots};
use crate::error::AppError;
use crate::frontmatter::parse_markdown_frontmatter;
use crate::git_hygiene;
use crate::models::{
    AgentKind, AppConfig, BrokenSymlinkIssue, ClaudeSyncedBucketItem, CleanupActionResult,
    CleanupAuditReport, DuplicateSkillIssue, InstructionFixApplyResult, InstructionFixPreview,
    InstructionHygieneIssue, InstructionHygieneKind, PluginItem, ScopeSnapshot, SkillSource,
    StaleGitExcludeIssue, StalePluginCacheItem, StraySkillIssue,
};
use crate::plugins;
use crate::scanner;
use crate::symlinks;
use crate::tokenizer;

/// Token threshold above which an always-loaded global instruction file triggers a hygiene warning.
pub const GLOBAL_INSTRUCTION_WARNING_TOKENS: usize = 2_500;

/// Performs a full read-only health and cleanup audit across Global scope and all projects.
///
/// # Errors
/// Returns [`AppError`] if the user home directory cannot be resolved.
pub fn audit_workspace(cfg: &AppConfig) -> Result<CleanupAuditReport, AppError> {
    let home = config::home_dir()?;
    Ok(audit_workspace_for_home(cfg, &home))
}

/// Performs a read-only cleanup audit against an explicit `home` directory.
#[must_use]
pub fn audit_workspace_for_home(cfg: &AppConfig, home: &Path) -> CleanupAuditReport {
    let global = scanner::scan_global_only_for_home(cfg, home);
    let projects = scanner::discover_project_entries(cfg, home);
    let roots = config::resolve_agent_roots(cfg, home);

    let stray_global_skills = collect_stray_global_skills(&global);
    let unmanaged_global_skill_ids: Vec<String> = stray_global_skills
        .iter()
        .map(|s| s.skill_id.clone())
        .collect();
    let stray_project_skills = collect_stray_project_skills(cfg, &global, &projects);

    let broken_symlinks = collect_broken_symlinks(cfg, home, &global, &projects);
    let duplicate_skills = collect_duplicate_skills(cfg, &global, &projects);
    let (stale_plugin_caches, stale_plugin_cache_bytes) =
        collect_stale_codex_plugin_caches(cfg, &roots);
    let stale_git_excludes = collect_stale_git_excludes(&projects);
    let claude_synced_buckets = collect_claude_synced_buckets(cfg, &roots);
    let instruction_hygiene = collect_instruction_hygiene(cfg, home, &global);

    let total_issues = broken_symlinks.len()
        + stray_global_skills.len()
        + duplicate_skills.len()
        + stale_plugin_caches.len()
        + stale_git_excludes.len()
        + claude_synced_buckets.len()
        + instruction_hygiene.len();

    CleanupAuditReport {
        total_issues,
        broken_symlinks,
        duplicate_skills,
        stale_plugin_caches,
        stale_plugin_cache_bytes,
        stale_git_excludes,
        claude_synced_buckets,
        instruction_hygiene,
        unmanaged_global_skill_ids,
        stray_global_skills,
        stray_project_skills,
    }
}

/// Unlinks a batch of verified broken symlinks across Global and Project scopes.
///
/// # Errors
/// Returns [`AppError`] only if initial configuration loading fails; per-item failures are
/// reported inside [`CleanupActionResult::warnings`].
pub fn unlink_broken_symlinks_batch(
    cfg: &AppConfig,
    items: &[(String, Option<String>)],
) -> Result<CleanupActionResult, AppError> {
    let mut removed_count = 0usize;
    let mut warnings = Vec::new();

    for (target_path, project_path) in items {
        let target = Path::new(target_path);
        let proj = project_path.as_deref().map(Path::new);
        match symlinks::unlink_broken_symlink(cfg, target, proj) {
            Ok(()) => removed_count += 1,
            Err(err) => warnings.push(format!("Skipped '{target_path}': {err}")),
        }
    }

    Ok(CleanupActionResult {
        removed_count,
        reclaimed_bytes: 0,
        warnings,
    })
}

/// Safely removes stale Codex plugin cache directories after verifying each path is
/// strictly inside `<codex_dir>/plugins/cache/` at depth >= 2 and is not active.
///
/// # Errors
/// Returns [`AppError`] on critical path resolution issues; per-item refusals or I/O errors
/// are returned in [`CleanupActionResult::warnings`].
pub fn prune_codex_plugin_caches(
    cfg: &AppConfig,
    home: &Path,
    paths: &[String],
) -> Result<CleanupActionResult, AppError> {
    let roots = config::resolve_agent_roots(cfg, home);
    let cache_root = roots.codex_dir.join("plugins/cache");
    let Ok(canon_cache_root) = fs::canonicalize(&cache_root) else {
        return Ok(CleanupActionResult {
            removed_count: 0,
            reclaimed_bytes: 0,
            warnings: vec![format!(
                "Codex plugin cache directory does not exist: {}",
                cache_root.display()
            )],
        });
    };

    let active_paths = plugins::active_codex_plugin_cache_paths(&roots);
    let mut removed_count = 0usize;
    let mut reclaimed_bytes = 0u64;
    let mut warnings = Vec::new();

    for raw_path in paths {
        let candidate = Path::new(raw_path);
        let Ok(meta) = fs::symlink_metadata(candidate) else {
            warnings.push(format!("Path does not exist: {raw_path}"));
            continue;
        };
        if meta.file_type().is_symlink() || !meta.is_dir() {
            warnings.push(format!(
                "Refusing to remove non-directory or symlink: {raw_path}"
            ));
            continue;
        }
        let Ok(canon_target) = fs::canonicalize(candidate) else {
            warnings.push(format!("Cannot canonicalize path: {raw_path}"));
            continue;
        };
        let Ok(rel) = canon_target.strip_prefix(&canon_cache_root) else {
            warnings.push(format!(
                "Refusing to delete path outside Codex plugin cache: {raw_path}"
            ));
            continue;
        };
        if rel.components().count() < 3 {
            warnings.push(format!(
                "Refusing to delete top-level marketplace or plugin folder: {raw_path}"
            ));
            continue;
        }
        if active_paths.contains(&canon_target) {
            warnings.push(format!(
                "Refusing to delete active Codex plugin cache: {raw_path}"
            ));
            continue;
        }

        let size = dir_size_bytes(&canon_target);
        match fs::remove_dir_all(&canon_target) {
            Ok(()) => {
                removed_count += 1;
                reclaimed_bytes += size;
                if let Some(slug_parent) = canon_target.parent() {
                    if slug_parent != canon_cache_root
                        && fs::read_dir(slug_parent).is_ok_and(|mut r| r.next().is_none())
                    {
                        let _ = fs::remove_dir(slug_parent);
                    }
                }
            }
            Err(err) => warnings.push(format!("Failed to remove '{raw_path}': {err}")),
        }
    }

    Ok(CleanupActionResult {
        removed_count,
        reclaimed_bytes,
        warnings,
    })
}

/// Removes stale AgentDock-managed `.git/info/exclude` entries across the requested projects.
///
/// # Errors
/// Returns [`AppError`] only on unrecoverable configuration errors; per-project failures
/// are collected in [`CleanupActionResult::warnings`].
pub fn prune_stale_git_excludes(
    _cfg: &AppConfig,
    _home: &Path,
    project_paths: &[String],
    custom_backups_root: Option<&Path>,
) -> Result<CleanupActionResult, AppError> {
    let mut removed_count = 0usize;
    let mut warnings = Vec::new();

    for project_str in project_paths {
        let project = Path::new(project_str);
        if !project.is_dir() {
            warnings.push(format!("Project folder does not exist: {project_str}"));
            continue;
        }
        let stale = stale_entries_for_project(project);
        if stale.is_empty() {
            continue;
        }
        let stale_len = stale.len();
        match git_hygiene::sync_git_exclude(project, &[], &stale, custom_backups_root) {
            Ok(()) => removed_count += stale_len,
            Err(err) => warnings.push(format!("Failed to clean '{project_str}': {err}")),
        }
    }

    Ok(CleanupActionResult {
        removed_count,
        reclaimed_bytes: 0,
        warnings,
    })
}

/// Safely removes a direct child bucket directory inside `<claude_dir>/skills/synced/`.
///
/// # Errors
/// Returns [`AppError`] if the path is not a valid direct child directory of `<claude_dir>/skills/synced/`
/// or if deletion fails.
pub fn remove_claude_synced_bucket(
    cfg: &AppConfig,
    home: &Path,
    path: &str,
) -> Result<CleanupActionResult, AppError> {
    let roots = config::resolve_agent_roots(cfg, home);
    let synced_root = roots.claude_dir.join("skills/synced");
    let canon_root = fs::canonicalize(&synced_root).map_err(|e| AppError::io(&synced_root, e))?;

    let target = Path::new(path);
    let meta = fs::symlink_metadata(target).map_err(|e| AppError::io(target, e))?;
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return Err(AppError::InvalidPath(format!(
            "Refusing to remove non-directory or symlink: {path}"
        )));
    }
    let canon_target = fs::canonicalize(target).map_err(|e| AppError::io(target, e))?;
    let rel = canon_target.strip_prefix(&canon_root).map_err(|_| {
        AppError::InvalidPath(format!(
            "Refusing to delete path outside Claude synced skills: {path}"
        ))
    })?;
    if rel.components().count() != 1 {
        return Err(AppError::InvalidPath(format!(
            "Refusing to delete non-bucket path: {path}"
        )));
    }

    let size = dir_size_bytes(&canon_target);
    fs::remove_dir_all(&canon_target).map_err(|e| AppError::io(&canon_target, e))?;

    Ok(CleanupActionResult {
        removed_count: 1,
        reclaimed_bytes: size,
        warnings: Vec::new(),
    })
}

fn collect_broken_symlinks(
    cfg: &AppConfig,
    home: &Path,
    global: &crate::models::ScopeSnapshot,
    projects: &[(String, String, PathBuf)],
) -> Vec<BrokenSymlinkIssue> {
    let mut seen_paths = BTreeSet::new();
    let mut out = Vec::new();

    for skill in &global.skills {
        let target_hint = match &skill.source {
            SkillSource::BrokenSymlink { target } => target.clone(),
            _ => String::new(),
        };
        for inst in &skill.installed_paths {
            if !inst.broken || !seen_paths.insert(inst.path.clone()) {
                continue;
            }
            let target = if target_hint.is_empty() {
                read_broken_link_target(Path::new(&inst.path))
            } else {
                target_hint.clone()
            };
            let (original_path, original_location) =
                find_broken_symlink_original("skill", &inst.path, &target, global, cfg, home);
            out.push(BrokenSymlinkIssue {
                path: inst.path.clone(),
                target,
                scope_label: "Global".to_owned(),
                project_path: None,
                agent: Some(inst.agent),
                item_kind: "skill".to_owned(),
                original_path,
                original_location,
            });
        }
    }

    for rule in &global.rules {
        if rule.source_label != "Broken Symlink" || !seen_paths.insert(rule.path.clone()) {
            continue;
        }
        let target = rule
            .frontmatter_error
            .as_deref()
            .and_then(|s| s.strip_prefix("Broken symlink -> "))
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| read_broken_link_target(Path::new(&rule.path)));
        let (original_path, original_location) =
            find_broken_symlink_original("rule", &rule.path, &target, global, cfg, home);
        out.push(BrokenSymlinkIssue {
            path: rule.path.clone(),
            target,
            scope_label: "Global".to_owned(),
            project_path: None,
            agent: rule.active_agents.first().copied(),
            item_kind: "rule".to_owned(),
            original_path,
            original_location,
        });
    }

    for (cat, proj_name, proj_path) in projects {
        let scope_label = format!("{cat} / {proj_name}");
        let proj_str = proj_path.to_string_lossy().into_owned();
        let dirs: [(&str, Option<AgentKind>, &str); 7] = [
            (".agents/skills", Some(AgentKind::Antigravity), "skill"),
            (".agent/skills", Some(AgentKind::Antigravity), "skill"),
            (".claude/skills", Some(AgentKind::Claude), "skill"),
            (".agents/rules", Some(AgentKind::Antigravity), "rule"),
            (".agent/rules", Some(AgentKind::Antigravity), "rule"),
            (".claude/rules", Some(AgentKind::Claude), "rule"),
            ("_support-docs", None, "supportDoc"),
        ];

        for (rel_dir, agent, item_kind) in dirs {
            let dir = proj_path.join(rel_dir);
            let Ok(read_dir) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in read_dir.flatten() {
                let entry_path = entry.path();
                let Ok(meta) = fs::symlink_metadata(&entry_path) else {
                    continue;
                };
                if meta.file_type().is_symlink() && !entry_path.exists() {
                    let path_str = entry_path.to_string_lossy().into_owned();
                    if seen_paths.insert(path_str.clone()) {
                        let target = read_broken_link_target(&entry_path);
                        let (original_path, original_location) = find_broken_symlink_original(
                            item_kind, &path_str, &target, global, cfg, home,
                        );
                        out.push(BrokenSymlinkIssue {
                            path: path_str,
                            target,
                            scope_label: scope_label.clone(),
                            project_path: Some(proj_str.clone()),
                            agent,
                            item_kind: item_kind.to_owned(),
                            original_path,
                            original_location,
                        });
                    }
                }
            }
        }
    }

    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

fn find_broken_symlink_original(
    item_kind: &str,
    link_path: &str,
    target: &str,
    global: &ScopeSnapshot,
    cfg: &AppConfig,
    home: &Path,
) -> (Option<String>, Option<String>) {
    let link_name = Path::new(link_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    let target_name = Path::new(target)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    let matches_name = |candidate: &str| {
        !candidate.is_empty() && (candidate == link_name || candidate == target_name)
    };

    match item_kind {
        "skill" => {
            let mut fallback: Option<(String, String)> = None;
            for skill in &global.skills {
                if matches!(skill.source, SkillSource::BrokenSymlink { .. }) {
                    continue;
                }
                let dir_name = Path::new(&skill.skill_dir_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");
                if !(matches_name(&skill.id) || matches_name(&skill.name) || matches_name(dir_name))
                {
                    continue;
                }
                if skill.skill_dir_path == link_path || !Path::new(&skill.skill_dir_path).exists() {
                    continue;
                }
                match &skill.source {
                    SkillSource::Library { label, .. } => {
                        return (
                            Some(skill.skill_dir_path.clone()),
                            Some(format!("Library ({label})")),
                        );
                    }
                    _ => {
                        if fallback.is_none() {
                            fallback = Some((
                                skill.skill_dir_path.clone(),
                                "Global agent folder".to_owned(),
                            ));
                        }
                    }
                }
            }
            if let Some((path, loc)) = fallback {
                return (Some(path), Some(loc));
            }
        }
        "rule" => {
            for rule in &global.rules {
                if rule.source_label == "Broken Symlink"
                    || rule.canonical_path == link_path
                    || !Path::new(&rule.canonical_path).is_file()
                {
                    continue;
                }
                if matches_name(&rule.filename) {
                    let loc = if rule.is_library_rule {
                        format!("Library ({})", rule.source_label)
                    } else {
                        rule.source_label.clone()
                    };
                    return (Some(rule.canonical_path.clone()), Some(loc));
                }
            }
        }
        "supportDoc" => {
            if let Some(root_raw) = cfg.support_docs_root.as_deref() {
                let root = if let Some(stripped) = root_raw.strip_prefix("~/") {
                    home.join(stripped)
                } else {
                    let p = PathBuf::from(root_raw);
                    if p.is_absolute() {
                        p
                    } else {
                        home.join(p)
                    }
                };
                if root.is_dir() {
                    if let Some(found) = find_named_entry(&root, link_name, target_name, 3) {
                        return (
                            Some(found.to_string_lossy().into_owned()),
                            Some("Support Docs".to_owned()),
                        );
                    }
                }
            }
        }
        _ => {}
    }

    (None, None)
}

fn find_named_entry(
    dir: &Path,
    link_name: &str,
    target_name: &str,
    depth: usize,
) -> Option<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return None;
    };
    let mut subdirs = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name_str) = name.to_str() else {
            continue;
        };
        if name_str.starts_with('.') || name_str == "node_modules" || name_str == "target" {
            continue;
        }
        let path = entry.path();
        if (!link_name.is_empty() && name_str == link_name)
            || (!target_name.is_empty() && name_str == target_name)
        {
            if path.exists() {
                return Some(path);
            }
        }
        if depth > 1 && path.is_dir() {
            subdirs.push(path);
        }
    }
    for sub in subdirs {
        if let Some(found) = find_named_entry(&sub, link_name, target_name, depth - 1) {
            return Some(found);
        }
    }
    None
}

fn read_broken_link_target(path: &Path) -> String {
    fs::read_link(path)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "missing target".to_owned())
}

fn collect_stray_global_skills(global: &ScopeSnapshot) -> Vec<StraySkillIssue> {
    let mut out: Vec<StraySkillIssue> = global
        .skills
        .iter()
        .filter(|s| s.active_globally && s.is_unmanaged_global)
        .map(|s| {
            // Prefer the actual real (non-symlink) directory in an agent folder, because
            // if a skill with the same slug also exists in a Library Source, `skill_dir_path`
            // points to the Library copy rather than the loose folder on disk.
            let real_dir = s
                .installed_paths
                .iter()
                .find(|p| !p.is_symlink && !p.broken)
                .map(|p| p.path.clone())
                .unwrap_or_else(|| s.skill_dir_path.clone());
            let candidate_md = Path::new(&real_dir).join("SKILL.md");
            let skill_md_path = if candidate_md.is_file() {
                candidate_md.to_string_lossy().into_owned()
            } else {
                s.canonical_path.clone()
            };

            StraySkillIssue {
                skill_id: s.id.clone(),
                skill_name: s.name.clone(),
                path: real_dir,
                skill_md_path,
                scope_label: "Global".to_owned(),
                project_path: None,
                agents: s.active_agents.clone(),
                upfront_tokens: s.upfront_tokens,
                exists_in_library: matches!(s.source, SkillSource::Library { .. }),
            }
        })
        .collect();

    out.sort_by(|a, b| {
        b.upfront_tokens.cmp(&a.upfront_tokens).then_with(|| {
            a.skill_name
                .to_lowercase()
                .cmp(&b.skill_name.to_lowercase())
        })
    });
    out
}

fn collect_stray_project_skills(
    cfg: &AppConfig,
    global: &ScopeSnapshot,
    projects: &[(String, String, PathBuf)],
) -> Vec<StraySkillIssue> {
    let mut library_skill_ids = BTreeSet::new();
    for skill in &global.skills {
        if matches!(skill.source, SkillSource::Library { .. }) {
            library_skill_ids.insert(skill.id.to_ascii_lowercase());
            library_skill_ids.insert(skill.name.to_ascii_lowercase());
        }
    }

    let mut shared_agents = Vec::new();
    if cfg.active_agents.antigravity {
        shared_agents.push(AgentKind::Antigravity);
    }
    if cfg.active_agents.codex {
        shared_agents.push(AgentKind::Codex);
    }

    let mut dirs: Vec<(&str, Vec<AgentKind>)> = Vec::new();
    if !shared_agents.is_empty() {
        dirs.push((".agents/skills", shared_agents));
    }
    if cfg.active_agents.antigravity {
        dirs.push((".agent/skills", vec![AgentKind::Antigravity]));
    }
    if cfg.active_agents.claude {
        dirs.push((".claude/skills", vec![AgentKind::Claude]));
    }

    let mut out = Vec::new();
    for (cat, proj_name, proj_path) in projects {
        let scope_label = format!("{cat} / {proj_name}");
        let proj_str = proj_path.to_string_lossy().into_owned();
        let mut by_slug: BTreeMap<String, StraySkillIssue> = BTreeMap::new();

        for (rel_dir, agents) in &dirs {
            let dir = proj_path.join(rel_dir);
            let Ok(read_dir) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in read_dir.flatten() {
                let entry_path = entry.path();
                let Some(slug) = entry.file_name().to_str().map(ToOwned::to_owned) else {
                    continue;
                };
                if slug.starts_with('.') {
                    continue;
                }
                let Ok(meta) = fs::symlink_metadata(&entry_path) else {
                    continue;
                };
                if meta.file_type().is_symlink() || !meta.is_dir() {
                    continue;
                }
                let skill_md = entry_path.join("SKILL.md");
                if !skill_md.is_file() {
                    continue;
                }

                if let Some(existing) = by_slug.get_mut(&slug) {
                    for &agent in agents {
                        if !existing.agents.contains(&agent) {
                            existing.agents.push(agent);
                        }
                    }
                    continue;
                }

                let Ok(content) = fs::read_to_string(&skill_md) else {
                    continue;
                };
                let parsed = parse_markdown_frontmatter(&content);
                let skill_name = parsed.name.unwrap_or_else(|| slug.clone());
                let desc = parsed.description.unwrap_or_default();
                let upfront_tokens = tokenizer::skill_upfront_tokens(&skill_name, &desc);
                let exists_in_library = library_skill_ids.contains(&slug.to_ascii_lowercase())
                    || library_skill_ids.contains(&skill_name.to_ascii_lowercase());

                by_slug.insert(
                    slug.clone(),
                    StraySkillIssue {
                        skill_id: slug,
                        skill_name,
                        path: entry_path.to_string_lossy().into_owned(),
                        skill_md_path: skill_md.to_string_lossy().into_owned(),
                        scope_label: scope_label.clone(),
                        project_path: Some(proj_str.clone()),
                        agents: agents.clone(),
                        upfront_tokens,
                        exists_in_library,
                    },
                );
            }
        }

        out.extend(by_slug.into_values());
    }

    out.sort_by(|a, b| {
        a.scope_label
            .to_lowercase()
            .cmp(&b.scope_label.to_lowercase())
            .then_with(|| {
                a.skill_name
                    .to_lowercase()
                    .cmp(&b.skill_name.to_lowercase())
            })
    });
    out
}

fn collect_duplicate_skills(
    cfg: &AppConfig,
    global: &crate::models::ScopeSnapshot,
    projects: &[(String, String, PathBuf)],
) -> Vec<DuplicateSkillIssue> {
    let mut plugin_skill_map: BTreeMap<(AgentKind, String), &PluginItem> = BTreeMap::new();
    for plugin in global.plugins.iter().filter(|p| p.enabled) {
        for skill in &plugin.skills {
            plugin_skill_map
                .entry((plugin.agent, skill.name.to_ascii_lowercase()))
                .or_insert(plugin);
        }
    }

    if plugin_skill_map.is_empty() {
        return Vec::new();
    }

    let mut seen: BTreeSet<(String, AgentKind, String)> = BTreeSet::new();
    let mut out = Vec::new();

    for skill in &global.skills {
        if !skill.active_globally || matches!(skill.source, SkillSource::BrokenSymlink { .. }) {
            continue;
        }
        let name_key = skill.name.to_ascii_lowercase();
        let id_key = skill.id.to_ascii_lowercase();
        for &agent in &skill.active_agents {
            let matched = plugin_skill_map
                .get(&(agent, name_key.clone()))
                .or_else(|| plugin_skill_map.get(&(agent, id_key.clone())));
            let Some(&plugin) = matched else {
                continue;
            };
            let standalone_path = skill
                .installed_paths
                .iter()
                .find(|p| p.agent == agent && !p.broken)
                .map(|p| p.path.clone())
                .unwrap_or_else(|| skill.skill_dir_path.clone());
            if seen.insert((standalone_path.clone(), plugin.agent, plugin.id.clone())) {
                out.push(DuplicateSkillIssue {
                    skill_name: skill.name.clone(),
                    skill_id: skill.id.clone(),
                    standalone_path,
                    scope_label: "Global".to_owned(),
                    project_path: None,
                    plugin_agent: plugin.agent,
                    plugin_id: plugin.id.clone(),
                    plugin_name: plugin.name.clone(),
                    duplicate_tokens: skill.upfront_tokens,
                    is_symlink: skill.is_symlink,
                });
            }
        }
    }

    for (cat, proj_name, proj_path) in projects {
        let scope_label = format!("{cat} / {proj_name}");
        let proj_str = proj_path.to_string_lossy().into_owned();

        let mut dirs: Vec<(&str, Vec<AgentKind>)> = Vec::new();
        let mut shared_agents = Vec::new();
        if cfg.active_agents.antigravity {
            shared_agents.push(AgentKind::Antigravity);
        }
        if cfg.active_agents.codex {
            shared_agents.push(AgentKind::Codex);
        }
        if !shared_agents.is_empty() {
            dirs.push((".agents/skills", shared_agents));
        }
        if cfg.active_agents.antigravity {
            dirs.push((".agent/skills", vec![AgentKind::Antigravity]));
        }
        if cfg.active_agents.claude {
            dirs.push((".claude/skills", vec![AgentKind::Claude]));
        }

        for (rel_dir, agents) in dirs {
            let dir = proj_path.join(rel_dir);
            let Ok(read_dir) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in read_dir.flatten() {
                let entry_path = entry.path();
                let Some(slug) = entry.file_name().to_str().map(ToOwned::to_owned) else {
                    continue;
                };
                if slug.starts_with('.') {
                    continue;
                }
                let Ok(meta) = fs::symlink_metadata(&entry_path) else {
                    continue;
                };
                let is_symlink = meta.file_type().is_symlink();
                if !entry_path.is_dir() {
                    continue;
                }
                let skill_md = entry_path.join("SKILL.md");
                if !skill_md.is_file() {
                    continue;
                }
                let Ok(content) = fs::read_to_string(&skill_md) else {
                    continue;
                };
                let parsed = parse_markdown_frontmatter(&content);
                let name = parsed.name.unwrap_or_else(|| slug.clone());
                let desc = parsed.description.unwrap_or_default();
                let tokens = tokenizer::skill_upfront_tokens(&name, &desc);
                let name_key = name.to_ascii_lowercase();
                let id_key = slug.to_ascii_lowercase();
                let standalone_path = entry_path.to_string_lossy().into_owned();

                for &agent in &agents {
                    let matched = plugin_skill_map
                        .get(&(agent, name_key.clone()))
                        .or_else(|| plugin_skill_map.get(&(agent, id_key.clone())));
                    let Some(&plugin) = matched else {
                        continue;
                    };
                    if seen.insert((standalone_path.clone(), plugin.agent, plugin.id.clone())) {
                        out.push(DuplicateSkillIssue {
                            skill_name: name.clone(),
                            skill_id: slug.clone(),
                            standalone_path: standalone_path.clone(),
                            scope_label: scope_label.clone(),
                            project_path: Some(proj_str.clone()),
                            plugin_agent: plugin.agent,
                            plugin_id: plugin.id.clone(),
                            plugin_name: plugin.name.clone(),
                            duplicate_tokens: tokens,
                            is_symlink,
                        });
                    }
                }
            }
        }
    }

    out.sort_by(|a, b| {
        b.duplicate_tokens.cmp(&a.duplicate_tokens).then_with(|| {
            a.skill_name
                .to_lowercase()
                .cmp(&b.skill_name.to_lowercase())
        })
    });
    out
}

fn collect_stale_codex_plugin_caches(
    cfg: &AppConfig,
    roots: &AgentRoots,
) -> (Vec<StalePluginCacheItem>, u64) {
    if !cfg.active_agents.codex {
        return (Vec::new(), 0);
    }

    let cache_root = roots.codex_dir.join("plugins/cache");
    if !cache_root.is_dir() {
        return (Vec::new(), 0);
    }

    let active_paths = plugins::active_codex_plugin_cache_paths(roots);
    let configured_plugins = read_configured_codex_slugs(roots);
    let mut items = Vec::new();
    let mut total_bytes = 0u64;

    let Ok(markets) = fs::read_dir(&cache_root) else {
        return (Vec::new(), 0);
    };
    for market_entry in markets.flatten() {
        let market_path = market_entry.path();
        let Some(marketplace) = market_entry.file_name().to_str().map(ToOwned::to_owned) else {
            continue;
        };
        if !market_path.is_dir() || marketplace.starts_with('.') {
            continue;
        }

        let Ok(slugs) = fs::read_dir(&market_path) else {
            continue;
        };
        for slug_entry in slugs.flatten() {
            let slug_path = slug_entry.path();
            let Some(plugin_slug) = slug_entry.file_name().to_str().map(ToOwned::to_owned) else {
                continue;
            };
            if !slug_path.is_dir() || plugin_slug.starts_with('.') {
                continue;
            }

            let Ok(versions) = fs::read_dir(&slug_path) else {
                continue;
            };
            for ver_entry in versions.flatten() {
                let ver_path = ver_entry.path();
                let Some(version) = ver_entry.file_name().to_str().map(ToOwned::to_owned) else {
                    continue;
                };
                if version.starts_with('.') {
                    continue;
                }
                let Ok(meta) = fs::symlink_metadata(&ver_path) else {
                    continue;
                };
                if meta.file_type().is_symlink() || !meta.is_dir() {
                    continue;
                }
                let canon = fs::canonicalize(&ver_path).unwrap_or_else(|_| ver_path.clone());
                if active_paths.contains(&canon) {
                    continue;
                }

                let reason =
                    if configured_plugins.contains(&(marketplace.clone(), plugin_slug.clone())) {
                        "Superseded by active cached version".to_owned()
                    } else {
                        "Not configured in config.toml".to_owned()
                    };
                let size_bytes = dir_size_bytes(&ver_path);
                total_bytes += size_bytes;
                items.push(StalePluginCacheItem {
                    marketplace: marketplace.clone(),
                    plugin_slug: plugin_slug.clone(),
                    version,
                    path: ver_path.to_string_lossy().into_owned(),
                    reason,
                    size_bytes,
                });
            }
        }
    }

    items.sort_by(|a, b| {
        b.size_bytes
            .cmp(&a.size_bytes)
            .then_with(|| a.plugin_slug.cmp(&b.plugin_slug))
    });
    (items, total_bytes)
}

fn read_configured_codex_slugs(roots: &AgentRoots) -> BTreeSet<(String, String)> {
    let mut configured = BTreeSet::new();
    let config_toml_path = roots.codex_dir.join("config.toml");
    let Ok(raw) = fs::read_to_string(&config_toml_path) else {
        return configured;
    };
    let Ok(doc) = raw.parse::<DocumentMut>() else {
        return configured;
    };
    let Some(plugins_table) = doc.get("plugins").and_then(|item| item.as_table_like()) else {
        return configured;
    };
    for (plugin_id, item) in plugins_table.iter() {
        if item.as_table_like().is_none() {
            continue;
        }
        let (slug, marketplace) = match plugin_id.rsplit_once('@') {
            Some((s, m)) => (s.to_owned(), m.to_owned()),
            None => (plugin_id.to_owned(), "local".to_owned()),
        };
        if marketplace == "openai-curated" {
            configured.insert(("openai-curated-remote".to_owned(), slug.clone()));
        }
        configured.insert((marketplace, slug));
    }
    configured
}

fn collect_stale_git_excludes(projects: &[(String, String, PathBuf)]) -> Vec<StaleGitExcludeIssue> {
    let mut out = Vec::new();
    for (_cat, proj_name, proj_path) in projects {
        let entries = stale_entries_for_project(proj_path);
        if !entries.is_empty() {
            out.push(StaleGitExcludeIssue {
                project_name: proj_name.clone(),
                project_path: proj_path.to_string_lossy().into_owned(),
                entries,
            });
        }
    }
    out.sort_by(|a, b| {
        a.project_name
            .to_lowercase()
            .cmp(&b.project_name.to_lowercase())
    });
    out
}

fn stale_entries_for_project(project_path: &Path) -> Vec<String> {
    git_hygiene::read_managed_exclude_entries(project_path)
        .into_iter()
        .filter(|entry| {
            let trimmed = entry.trim_end_matches('/');
            if trimmed.is_empty() {
                return false;
            }
            let target = project_path.join(trimmed);
            !target.exists() && fs::symlink_metadata(&target).is_err()
        })
        .collect()
}

fn collect_claude_synced_buckets(
    cfg: &AppConfig,
    roots: &AgentRoots,
) -> Vec<ClaudeSyncedBucketItem> {
    if !cfg.active_agents.claude {
        return Vec::new();
    }
    let synced_dir = roots.claude_dir.join("skills/synced");
    if !synced_dir.is_dir() {
        return Vec::new();
    }

    let Ok(read_dir) = fs::read_dir(&synced_dir) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    for entry in read_dir.flatten() {
        let path = entry.path();
        let Some(name) = entry.file_name().to_str().map(ToOwned::to_owned) else {
            continue;
        };
        if name.starts_with('.') {
            continue;
        }
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.file_type().is_symlink() || !meta.is_dir() {
            continue;
        }
        let (size_bytes, entry_count) = dir_size_and_file_count(&path);
        out.push(ClaudeSyncedBucketItem {
            name,
            path: path.to_string_lossy().into_owned(),
            size_bytes,
            entry_count,
        });
    }

    out.sort_by_key(|a| std::cmp::Reverse(a.size_bytes));
    out
}

fn collect_library_markers(cfg: &AppConfig, home: &Path) -> Vec<String> {
    let mut library_markers: Vec<String> = Vec::new();
    for src in &cfg.library_sources {
        let raw = src.path.trim();
        if !raw.is_empty() {
            library_markers.push(raw.to_owned());
        }
        let expanded = config::expand_user_path(home, raw);
        let exp_str = expanded.to_string_lossy().into_owned();
        if !exp_str.is_empty() && !library_markers.contains(&exp_str) {
            library_markers.push(exp_str);
        }
    }
    library_markers
}

fn collect_instruction_hygiene(
    cfg: &AppConfig,
    home: &Path,
    global: &crate::models::ScopeSnapshot,
) -> Vec<InstructionHygieneIssue> {
    let mut out = Vec::new();
    let library_markers = collect_library_markers(cfg, home);

    for rule in global.rules.iter().filter(|r| r.active_globally) {
        let primary_agent = rule
            .active_agents
            .first()
            .copied()
            .unwrap_or(AgentKind::Antigravity);

        if let Some(err) = &rule.frontmatter_error {
            out.push(InstructionHygieneIssue {
                path: rule.path.clone(),
                filename: rule.filename.clone(),
                agent: primary_agent,
                kind: InstructionHygieneKind::Frontmatter,
                severity: "warning".to_owned(),
                message: format!("Invalid rule frontmatter: {err}"),
                tokens: rule.full_tokens,
            });
        }

        let is_instruction_file = matches!(
            rule.filename.as_str(),
            "AGENTS.md" | "AGENTS.override.md" | "GEMINI.md" | "CLAUDE.md"
        );
        if !is_instruction_file {
            continue;
        }

        if rule.full_tokens > GLOBAL_INSTRUCTION_WARNING_TOKENS {
            out.push(InstructionHygieneIssue {
                path: rule.path.clone(),
                filename: rule.filename.clone(),
                agent: primary_agent,
                kind: InstructionHygieneKind::Oversized,
                severity: "warning".to_owned(),
                message: format!(
                    "Loads {} tokens into every session (recommended limit: {GLOBAL_INSTRUCTION_WARNING_TOKENS}). Move detailed sections into on-demand skills or rules.",
                    rule.full_tokens
                ),
                tokens: rule.full_tokens,
            });
        }

        if cfg.global_pause.skills_paused {
            if let Ok(content) = fs::read_to_string(&rule.canonical_path) {
                let references_skill_dirs = content.contains(".agents/skills")
                    || content.contains("config/skills")
                    || library_markers.iter().any(|m| content.contains(m));
                if references_skill_dirs {
                    out.push(InstructionHygieneIssue {
                        path: rule.path.clone(),
                        filename: rule.filename.clone(),
                        agent: primary_agent,
                        kind: InstructionHygieneKind::PausedSkillRef,
                        severity: "info".to_owned(),
                        message: "Mentions skill folder paths while Global Skills are paused — agents may still manually read referenced SKILL.md files.".to_owned(),
                        tokens: rule.full_tokens,
                    });
                }
            }
        }
    }

    out
}

/// Generates a suggested replacement template or repair preview for a flagged instruction or rule file.
///
/// # Errors
/// Returns [`AppError`] if the target path is invalid or cannot be read.
pub fn preview_instruction_fix(
    cfg: &AppConfig,
    home: &Path,
    path: &str,
    kind: InstructionHygieneKind,
) -> Result<InstructionFixPreview, AppError> {
    let raw_path = Path::new(path);
    let canonical = fs::canonicalize(raw_path).unwrap_or_else(|_| raw_path.to_path_buf());
    if !canonical.is_file() {
        return Err(AppError::InvalidPath(format!(
            "Instruction or rule file does not exist: {}",
            canonical.display()
        )));
    }

    let filename = raw_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("instructions.md")
        .to_owned();
    let current_content =
        fs::read_to_string(&canonical).map_err(|e| AppError::io(&canonical, e))?;
    let current_tokens = tokenizer::count_tokens(&current_content);

    let (title, why, what_changes, suggested_content) = match kind {
        InstructionHygieneKind::Frontmatter => {
            let body = extract_body_for_broken_frontmatter(&current_content);
            let repaired = if body.is_empty() {
                "---\ntrigger: always_on\n---\n".to_owned()
            } else {
                format!("---\ntrigger: always_on\n---\n\n{body}")
            };
            (
                "Repair Rule YAML Frontmatter".to_owned(),
                "Agents parse the YAML header (--- ... ---) at the top of rule files to decide when to activate the rule. A broken header can prevent the rule from loading.".to_owned(),
                "Replaces the broken YAML frontmatter with a valid `trigger: always_on` header while keeping your rule body intact.".to_owned(),
                repaired,
            )
        }
        InstructionHygieneKind::Oversized => {
            let template = build_lean_global_instruction_template(&filename);
            (
                "Replace with Lean Global Instruction Template".to_owned(),
                format!(
                    "{filename} is injected into every conversation across all projects. At {current_tokens} tokens (above the {GLOBAL_INSTRUCTION_WARNING_TOKENS}-token limit), it burns startup context on every prompt even when unrelated."
                ),
                "Replaces the oversized file with a compact (~100-token) global template. Your full original file is backed up to Application Support before writing so you can move detailed sections into on-demand skills.".to_owned(),
                template,
            )
        }
        InstructionHygieneKind::PausedSkillRef => {
            let markers = collect_library_markers(cfg, home);
            let cleaned = strip_paused_skill_references(&current_content, &markers);
            (
                "Remove Paused Skill Directory References".to_owned(),
                "Global Skills are paused in AgentDock, but this file still mentions skill folder paths. Agents reading it may still manually open SKILL.md files from those directories.".to_owned(),
                "Removes lines referencing `.agents/skills`, `config/skills`, or your configured skill library paths while preserving all other instructions.".to_owned(),
                cleaned,
            )
        }
    };

    let suggested_tokens = tokenizer::count_tokens(&suggested_content);

    Ok(InstructionFixPreview {
        path: path.to_owned(),
        canonical_path: canonical.to_string_lossy().into_owned(),
        filename,
        kind,
        title,
        why,
        what_changes,
        current_content,
        current_tokens,
        suggested_content,
        suggested_tokens,
    })
}

/// Backs up `path` to AgentDock's rolling snapshots and atomically writes `content`.
///
/// # Errors
/// Returns [`AppError`] if the file does not exist, cannot be backed up, or cannot be written.
pub fn apply_instruction_fix(
    path: &str,
    content: &str,
    custom_backups_root: Option<&Path>,
) -> Result<InstructionFixApplyResult, AppError> {
    let raw_path = Path::new(path);
    let canonical = fs::canonicalize(raw_path).unwrap_or_else(|_| raw_path.to_path_buf());
    if !canonical.is_file() {
        return Err(AppError::InvalidPath(format!(
            "Instruction or rule file does not exist: {}",
            canonical.display()
        )));
    }

    let backup_path = match custom_backups_root {
        Some(dir) => backup::backup_file_in_dir(&canonical, dir)?,
        None => backup::backup_file(&canonical)?,
    };

    backup::atomic_write_string(&canonical, content)?;

    Ok(InstructionFixApplyResult {
        path: canonical.to_string_lossy().into_owned(),
        backup_path: backup_path.map(|p| p.to_string_lossy().into_owned()),
        bytes_written: content.len(),
        tokens: tokenizer::count_tokens(content),
    })
}

fn extract_body_for_broken_frontmatter(raw: &str) -> String {
    let parsed = parse_markdown_frontmatter(raw);
    if parsed.frontmatter_raw.is_some() {
        return parsed.body.trim_start_matches('\n').to_owned();
    }
    let normalized = raw.replace("\r\n", "\n");
    if let Some(rest) = normalized.strip_prefix("---\n") {
        // Unclosed frontmatter block: drop lines until the first blank line or markdown heading.
        let mut lines = rest.lines().peekable();
        while let Some(line) = lines.peek() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                break;
            }
            lines.next();
        }
        return lines
            .collect::<Vec<_>>()
            .join("\n")
            .trim_start_matches('\n')
            .to_owned();
    }
    normalized.trim_start_matches('\n').to_owned()
}

fn build_lean_global_instruction_template(filename: &str) -> String {
    let title = match filename {
        "CLAUDE.md" => "# Global Instructions (Claude)",
        "GEMINI.md" => "# Global Instructions (Gemini / Antigravity)",
        "AGENTS.md" | "AGENTS.override.md" => "# Global Instructions (Agents)",
        _ => "# Global Instructions",
    };
    format!(
        "{title}\n\n\
## Core Principles\n\
- Maintain high code quality, strict typing, and clean modular architecture.\n\
- Preserve existing comments, docstrings, and unrelated structure unless requested.\n\
- Keep responses concise and focused on the user's explicit objective.\n\n\
## Workflow & Scope\n\
- Read relevant files before editing; verify changes with targeted tests or builds.\n\
- Keep project-specific conventions in project instruction files and detailed playbooks in on-demand skills.\n"
    )
}

fn strip_paused_skill_references(content: &str, library_markers: &[String]) -> String {
    let mut kept: Vec<&str> = Vec::new();
    let mut consecutive_blanks = 0usize;

    for line in content.lines() {
        let mentions_skill_dir = line.contains(".agents/skills")
            || line.contains("config/skills")
            || library_markers.iter().any(|m| line.contains(m));
        if mentions_skill_dir {
            continue;
        }
        if line.trim().is_empty() {
            consecutive_blanks += 1;
            if consecutive_blanks > 1 {
                continue;
            }
        } else {
            consecutive_blanks = 0;
        }
        kept.push(line);
    }

    let mut out = kept.join("\n");
    if content.ends_with('\n') && !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn dir_size_bytes(dir: &Path) -> u64 {
    dir_size_and_file_count(dir).0
}

fn dir_size_and_file_count(dir: &Path) -> (u64, usize) {
    let mut total_bytes = 0u64;
    let mut file_count = 0usize;
    for entry in walkdir::WalkDir::new(dir)
        .follow_links(false)
        .into_iter()
        .flatten()
    {
        if entry.file_type().is_file() {
            file_count += 1;
            if let Ok(meta) = entry.metadata() {
                total_bytes += meta.len();
            }
        }
    }
    (total_bytes, file_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        ActiveAgents, GlobalPauseState, LibrarySource, ProjectRoot, ProjectScanMode,
    };
    use std::os::unix::fs::symlink;
    use tempfile::TempDir;

    fn make_skill(dir: &Path, name: &str, desc: &str) {
        fs::create_dir_all(dir).expect("mkdir skill");
        fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: {desc}\n---\nBody\n"),
        )
        .expect("write SKILL.md");
    }

    #[test]
    fn audits_broken_symlinks_duplicates_stale_caches_git_excludes_claude_buckets_and_instructions()
    {
        let tmp = TempDir::new().expect("temp dir");
        let root = tmp.path();
        let home = root.join("home");
        let projects_root = root.join("APP-PROJECTS");
        let proj_demo = projects_root.join("Apps/demo");
        let lib_dir = root.join("VaultSkills");

        fs::create_dir_all(home.join(".gemini/config")).expect("gemini");
        fs::create_dir_all(home.join(".codex/skills")).expect("codex skills");
        fs::create_dir_all(home.join(".claude/skills/synced/bucket-1")).expect("claude bucket");
        fs::create_dir_all(proj_demo.join(".git/info")).expect("git info");
        fs::create_dir_all(proj_demo.join(".agents/skills")).expect("proj skills");
        fs::create_dir_all(&lib_dir).expect("lib");

        // 1. Broken symlink in ~/.codex/skills and in project _support-docs
        symlink(
            home.join(".agents/skills/missing"),
            home.join(".codex/skills/missing-skill"),
        )
        .expect("broken codex link");
        fs::create_dir_all(proj_demo.join("_support-docs")).expect("support docs");
        symlink(
            root.join("missing-doc.md"),
            proj_demo.join("_support-docs/missing-doc.md"),
        )
        .expect("broken doc link");

        // 2. Unmanaged global skill + duplicate with enabled Codex plugin
        make_skill(
            &home.join(".codex/skills/pdf"),
            "pdf",
            "Standalone PDF skill.",
        );

        // Codex config with active plugin `pdf@openai-primary-runtime`
        let active_cache = home.join(".codex/plugins/cache/openai-primary-runtime/pdf/2.0.0");
        fs::create_dir_all(active_cache.join(".codex-plugin")).expect("active cache");
        fs::write(
            active_cache.join(".codex-plugin/plugin.json"),
            r#"{"name":"pdf","version":"2.0.0"}"#,
        )
        .expect("manifest");
        make_skill(&active_cache.join("skills/pdf"), "pdf", "Plugin PDF skill.");

        // Stale superseded cache + unconfigured remote cache
        let stale_superseded = home.join(".codex/plugins/cache/openai-primary-runtime/pdf/1.0.0");
        fs::create_dir_all(&stale_superseded).expect("stale 1.0.0");
        fs::write(stale_superseded.join("README.md"), "old version").expect("write old");

        let stale_unconfigured =
            home.join(".codex/plugins/cache/openai-curated-remote/figma/20.0.0");
        fs::create_dir_all(&stale_unconfigured).expect("stale figma");
        fs::write(stale_unconfigured.join("README.md"), "unused remote").expect("write unused");

        fs::write(
            home.join(".codex/config.toml"),
            "[plugins.\"pdf@openai-primary-runtime\"]\nenabled = true\n",
        )
        .expect("write codex config");

        // 3. Stale .git/info/exclude entry (missing skill path) + valid real project skill
        make_skill(
            &proj_demo.join(".agents/skills/kept-skill"),
            "kept-skill",
            "Still exists.",
        );
        make_skill(&lib_dir.join("kept-skill"), "kept-skill", "Library copy.");
        fs::write(
            proj_demo.join(".git/info/exclude"),
            "# BEGIN AGENTDOCK\n.agents/skills/ghost-skill\n.agents/skills/kept-skill\n# END AGENTDOCK\n",
        )
        .expect("write exclude");

        // 4. Claude synced bucket payload
        fs::write(
            home.join(".claude/skills/synced/bucket-1/note.md"),
            "synced bucket file",
        )
        .expect("write bucket file");

        // 5. Global instruction hygiene: oversized + paused library pointer
        let big_body = "word ".repeat(3000);
        fs::write(
            home.join(".gemini/config/AGENTS.md"),
            format!("Skills live in {}\n\n{big_body}", lib_dir.display()),
        )
        .expect("write AGENTS.md");

        let cfg = AppConfig {
            project_roots: vec![ProjectRoot {
                id: "p1".to_owned(),
                label: "APP-PROJECTS".to_owned(),
                path: projects_root.to_string_lossy().into_owned(),
                depth: 3,
                mode: ProjectScanMode::AutoDetect,
            }],
            library_sources: vec![LibrarySource {
                id: "lib".to_owned(),
                label: "Vault".to_owned(),
                path: lib_dir.to_string_lossy().into_owned(),
                depth: 2,
            }],
            active_agents: ActiveAgents {
                antigravity: true,
                codex: true,
                claude: true,
            },
            global_pause: GlobalPauseState {
                skills_paused: true,
                ..GlobalPauseState::default()
            },
            ..AppConfig::default()
        };

        let report = audit_workspace_for_home(&cfg, &home);

        assert_eq!(report.broken_symlinks.len(), 2);
        assert!(report
            .broken_symlinks
            .iter()
            .any(|b| b.item_kind == "skill"));
        assert!(report
            .broken_symlinks
            .iter()
            .any(|b| b.item_kind == "supportDoc"));
        assert_eq!(report.unmanaged_global_skill_ids, vec!["pdf"]);
        assert_eq!(report.stray_global_skills.len(), 1);
        assert_eq!(report.stray_global_skills[0].skill_id, "pdf");
        assert!(!report.stray_global_skills[0].exists_in_library);
        assert_eq!(
            report.stray_global_skills[0].path,
            home.join(".codex/skills/pdf").to_string_lossy()
        );
        assert_eq!(report.stray_project_skills.len(), 1);
        assert_eq!(report.stray_project_skills[0].skill_id, "kept-skill");
        assert!(report.stray_project_skills[0].exists_in_library);
        assert_eq!(report.duplicate_skills.len(), 1);
        assert_eq!(report.duplicate_skills[0].skill_name, "pdf");
        assert_eq!(report.duplicate_skills[0].plugin_agent, AgentKind::Codex);
        assert_eq!(report.stale_plugin_caches.len(), 2);
        assert!(report.stale_plugin_cache_bytes > 0);
        assert_eq!(report.stale_git_excludes.len(), 1);
        assert_eq!(
            report.stale_git_excludes[0].entries,
            vec![".agents/skills/ghost-skill"]
        );
        assert_eq!(report.claude_synced_buckets.len(), 1);
        assert_eq!(report.claude_synced_buckets[0].name, "bucket-1");
        assert_eq!(report.claude_synced_buckets[0].entry_count, 1);
        assert_eq!(report.instruction_hygiene.len(), 2);
        assert!(report.total_issues >= 10);
    }

    #[test]
    fn prunes_stale_caches_git_excludes_and_claude_buckets_safely_refusing_active_or_outside_paths()
    {
        let tmp = TempDir::new().expect("temp dir");
        let root = tmp.path();
        let home = root.join("home");
        let backups = root.join("backups");
        let proj = root.join("proj");

        let active_cache = home.join(".codex/plugins/cache/openai-primary-runtime/pdf/2.0.0");
        let stale_cache = home.join(".codex/plugins/cache/openai-primary-runtime/pdf/1.0.0");
        let outside_dir = root.join("keep-me");
        let claude_bucket = home.join(".claude/skills/synced/old-bucket");

        fs::create_dir_all(&active_cache).expect("active");
        fs::create_dir_all(&stale_cache).expect("stale");
        fs::create_dir_all(&outside_dir).expect("outside");
        fs::create_dir_all(&claude_bucket).expect("claude bucket");
        fs::create_dir_all(proj.join(".git/info")).expect("git info");

        fs::write(stale_cache.join("data.bin"), vec![1u8; 64]).expect("write stale");
        fs::write(active_cache.join("data.bin"), vec![2u8; 64]).expect("write active");
        fs::write(claude_bucket.join("skill.md"), "bucket").expect("write bucket");
        fs::write(
            home.join(".codex/config.toml"),
            "[plugins.\"pdf@openai-primary-runtime\"]\nenabled = true\n",
        )
        .expect("write codex toml");
        fs::write(
            proj.join(".git/info/exclude"),
            "*.log\n\n# BEGIN AGENTDOCK\n.agents/skills/stale\n# END AGENTDOCK\n",
        )
        .expect("write exclude");

        let cfg = AppConfig {
            active_agents: ActiveAgents {
                antigravity: true,
                codex: true,
                claude: true,
            },
            ..AppConfig::default()
        };

        // 1. Prune caches: stale succeeds, active and outside_dir are refused
        let cache_res = prune_codex_plugin_caches(
            &cfg,
            &home,
            &[
                stale_cache.to_string_lossy().into_owned(),
                active_cache.to_string_lossy().into_owned(),
                outside_dir.to_string_lossy().into_owned(),
            ],
        )
        .expect("prune caches");
        assert_eq!(cache_res.removed_count, 1);
        assert_eq!(cache_res.reclaimed_bytes, 64);
        assert_eq!(cache_res.warnings.len(), 2);
        assert!(!stale_cache.exists());
        assert!(active_cache.is_dir());
        assert!(outside_dir.is_dir());

        // 2. Prune stale .git/info/exclude preserving user lines
        let git_res = prune_stale_git_excludes(
            &cfg,
            &home,
            &[proj.to_string_lossy().into_owned()],
            Some(&backups),
        )
        .expect("prune git excludes");
        assert_eq!(git_res.removed_count, 1);
        assert_eq!(
            fs::read_to_string(proj.join(".git/info/exclude")).expect("read exclude"),
            "*.log\n"
        );

        // 3. Remove Claude synced bucket; refuse outside path
        let bucket_res = remove_claude_synced_bucket(&cfg, &home, &claude_bucket.to_string_lossy())
            .expect("remove bucket");
        assert_eq!(bucket_res.removed_count, 1);
        assert!(!claude_bucket.exists());

        let err = remove_claude_synced_bucket(&cfg, &home, &outside_dir.to_string_lossy())
            .expect_err("must refuse outside path");
        assert!(err.to_string().contains("Refusing"));
        assert!(outside_dir.is_dir());
    }

    #[cfg(unix)]
    #[test]
    fn broken_symlink_reports_safe_library_original_or_missing() {
        let dir = TempDir::new().expect("tempdir");
        let home = dir.path().join("home");
        let lib_dir = dir.path().join("vault");
        let safe_skill = lib_dir.join("reverse-engineering-saas");
        fs::create_dir_all(&safe_skill).expect("create safe skill");
        fs::write(
            safe_skill.join("SKILL.md"),
            "---\nname: reverse-engineering-saas\ndescription: test\n---\nBody\n",
        )
        .expect("write SKILL.md");

        let agents_skills = home.join(".agents/skills");
        fs::create_dir_all(&agents_skills).expect("create agents skills");

        std::os::unix::fs::symlink(
            dir.path().join("old-moved-folder/reverse-engineering-saas"),
            agents_skills.join("reverse-engineering-saas"),
        )
        .expect("create broken symlink with library copy");

        std::os::unix::fs::symlink(
            dir.path().join("deleted-forever/ghost-skill"),
            agents_skills.join("ghost-skill"),
        )
        .expect("create truly lost broken symlink");

        let cfg = AppConfig {
            library_sources: vec![LibrarySource {
                id: "lib".to_owned(),
                label: "Vault".to_owned(),
                path: lib_dir.to_string_lossy().into_owned(),
                depth: 2,
            }],
            active_agents: ActiveAgents {
                antigravity: true,
                codex: false,
                claude: false,
            },
            ..AppConfig::default()
        };

        let report = audit_workspace_for_home(&cfg, &home);
        let safe_issue = report
            .broken_symlinks
            .iter()
            .find(|b| b.path.ends_with("reverse-engineering-saas"))
            .expect("safe broken symlink present");
        let canon_safe_skill = fs::canonicalize(&safe_skill).expect("canonicalize safe skill");
        assert_eq!(
            safe_issue.original_path.as_deref(),
            Some(canon_safe_skill.to_string_lossy().as_ref())
        );
        assert_eq!(
            safe_issue.original_location.as_deref(),
            Some("Library (Vault)")
        );

        let lost_issue = report
            .broken_symlinks
            .iter()
            .find(|b| b.path.ends_with("ghost-skill"))
            .expect("lost broken symlink present");
        assert_eq!(lost_issue.original_path, None);
        assert_eq!(lost_issue.original_location, None);
    }

    #[test]
    fn previews_and_applies_instruction_fixes_with_backup() {
        let tmp = TempDir::new().expect("temp dir");
        let root = tmp.path();
        let home = root.join("home");
        let backups = root.join("backups");
        fs::create_dir_all(home.join(".gemini")).expect("gemini dir");
        fs::create_dir_all(home.join(".agents/rules")).expect("rules dir");

        let gemini_md = home.join(".gemini/GEMINI.md");
        let long_body = "# Global\n\n".to_owned() + &"word ".repeat(3000);
        fs::write(&gemini_md, &long_body).expect("write gemini");

        let cfg = AppConfig::default();
        let preview = preview_instruction_fix(
            &cfg,
            &home,
            gemini_md.to_string_lossy().as_ref(),
            InstructionHygieneKind::Oversized,
        )
        .expect("preview oversized");
        assert!(preview.current_tokens > GLOBAL_INSTRUCTION_WARNING_TOKENS);
        assert!(preview.suggested_tokens < 250);
        assert!(preview.suggested_content.contains("Global Instructions"));

        let broken_rule = home.join(".agents/rules/broken.md");
        fs::write(&broken_rule, "---\ntrigger: [invalid\n---\n\nKeep me.\n")
            .expect("write broken rule");
        let rule_preview = preview_instruction_fix(
            &cfg,
            &home,
            broken_rule.to_string_lossy().as_ref(),
            InstructionHygieneKind::Frontmatter,
        )
        .expect("preview frontmatter");
        assert!(rule_preview
            .suggested_content
            .starts_with("---\ntrigger: always_on\n---"));
        assert!(rule_preview.suggested_content.contains("Keep me."));

        let applied = apply_instruction_fix(
            gemini_md.to_string_lossy().as_ref(),
            &preview.suggested_content,
            Some(&backups),
        )
        .expect("apply fix");
        assert!(applied.backup_path.is_some());
        assert_eq!(
            fs::read_to_string(&gemini_md).expect("read updated"),
            preview.suggested_content
        );
    }
}

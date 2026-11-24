//! Read-only filesystem scanner for skills, MCP servers, rules, support docs, and token budgets.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value as JsonValue;
use toml_edit::DocumentMut;

use crate::adapters::{enabled_adapters, AgentAdapter, PathSpec};
use crate::config;
use crate::error::AppError;
use crate::frontmatter::{classify_rule_trigger, parse_markdown_frontmatter};
use crate::mcp_catalog;
use crate::models::InstalledPathInfo;
use crate::models::{
    AgentKind, AppConfig, CategoryGroup, LibrarySource, McpCatalog, McpCatalogEntry, McpServerItem,
    McpTransport, ProjectSummary, RuleItem, RuleTrigger, ScopeSnapshot, ScopeType, SkillItem,
    SkillSource, SupportDocItem, SupportDocMode, TokenBudgetBreakdown, WorkspaceOverview,
    DEFAULT_MCP_ESTIMATE_TOKENS, DESCRIPTION_WARNING_CHARS, TOKEN_BUDGET_LIMIT,
};
use crate::tokenizer;

const SUPPORT_DOCS_START: &str = "<!-- AGENTDOCK:SUPPORT_DOCS_START -->";
const SUPPORT_DOCS_END: &str = "<!-- AGENTDOCK:SUPPORT_DOCS_END -->";

#[derive(Debug, Clone)]
struct CanonicalLibraryRoot {
    label: String,
    raw_path: PathBuf,
    canonical_path: PathBuf,
}

#[derive(Debug, Clone)]
struct LibraryCatalog {
    roots: Vec<CanonicalLibraryRoot>,
    skills: BTreeMap<String, LibrarySkillEntry>,
    rules: BTreeMap<String, LibraryRuleEntry>,
    warnings: Vec<String>,
}

#[derive(Debug, Clone)]
struct LibrarySkillEntry {
    slug: String,
    name: String,
    description: String,
    description_too_long: bool,
    upfront_tokens: usize,
    on_demand_tokens: usize,
    label: String,
    source_root: String,
    canonical_dir: PathBuf,
    skill_md_path: PathBuf,
    frontmatter_error: Option<String>,
}

#[derive(Debug, Clone)]
struct LibraryRuleEntry {
    id: String,
    filename: String,
    canonical_path: PathBuf,
    trigger: RuleTrigger,
    description: Option<String>,
    glob_pattern: Option<String>,
    upfront_tokens: usize,
    full_tokens: usize,
    source_label: String,
    frontmatter_error: Option<String>,
}

#[derive(Debug, Clone)]
enum TargetResolution {
    Valid {
        canonical: PathBuf,
        is_symlink: bool,
    },
    BrokenSymlink {
        target: String,
    },
}

/// Performs a full read-only scan of Global scope and all discovered 2-level categorized Projects.
///
/// Auto-imports any newly discovered global MCP definitions into `mcp-catalog.json`.
///
/// # Errors
/// Returns [`AppError`] if the user home directory cannot be resolved.
pub fn scan_workspace(cfg: &AppConfig) -> Result<WorkspaceOverview, AppError> {
    let home = config::home_dir()?;
    let mut catalog = mcp_catalog::load_catalog().unwrap_or_default();
    let mut discovered_entries = Vec::new();
    let library = scan_libraries(&cfg.library_sources);
    let global_snapshot = scan_global_scope(
        cfg,
        &home,
        &library,
        &catalog,
        Some(&mut discovered_entries),
    );
    if mcp_catalog::sync_discovered_entries(&mut catalog, &discovered_entries) {
        let _ = mcp_catalog::save_catalog(&catalog);
    }

    let categories = discover_projects(cfg, &home, &library, &catalog, &global_snapshot);
    let global_tokens = global_snapshot.budget.effective_total;

    let last_scanned_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    Ok(WorkspaceOverview {
        config: cfg.clone(),
        global_tokens,
        global_snapshot,
        categories,
        last_scanned_ms,
    })
}

struct ScanContext<'a> {
    cfg: &'a AppConfig,
    home: &'a Path,
    library: &'a LibraryCatalog,
    catalog: &'a McpCatalog,
    global_snapshot: &'a ScopeSnapshot,
}

/// Scans a single project directory and computes its effective token breakdown against Global.
///
/// # Errors
/// Returns [`AppError`] if the user home directory cannot be resolved or `project_path` is invalid.
pub fn scan_project(
    cfg: &AppConfig,
    project_path: &Path,
    category: &str,
) -> Result<ScopeSnapshot, AppError> {
    let home = config::home_dir()?;
    if !project_path.is_dir() {
        return Err(AppError::InvalidPath(format!(
            "Project folder does not exist: {}",
            project_path.display()
        )));
    }
    let mut catalog = mcp_catalog::load_catalog().unwrap_or_default();
    let mut discovered_entries = Vec::new();
    let library = scan_libraries(&cfg.library_sources);
    let global_snapshot = scan_global_scope(
        cfg,
        &home,
        &library,
        &catalog,
        Some(&mut discovered_entries),
    );
    let ctx = ScanContext {
        cfg,
        home: &home,
        library: &library,
        catalog: &catalog,
        global_snapshot: &global_snapshot,
    };
    let snap =
        scan_project_with_baseline(&ctx, project_path, category, Some(&mut discovered_entries));
    if mcp_catalog::sync_discovered_entries(&mut catalog, &discovered_entries) {
        let _ = mcp_catalog::save_catalog(&catalog);
    }
    Ok(snap)
}

/// Scans a single project against an explicit `home` directory (used internally and in tests).
///
/// # Errors
/// Returns [`AppError`] if `project_path` is not an existing directory.
pub fn scan_project_for_home(
    cfg: &AppConfig,
    home: &Path,
    project_path: &Path,
    category: &str,
) -> Result<ScopeSnapshot, AppError> {
    if !project_path.is_dir() {
        return Err(AppError::InvalidPath(format!(
            "Project folder does not exist: {}",
            project_path.display()
        )));
    }
    let catalog = McpCatalog::default();
    let library = scan_libraries(&cfg.library_sources);
    let global_snapshot = scan_global_scope(cfg, home, &library, &catalog, None);
    let ctx = ScanContext {
        cfg,
        home,
        library: &library,
        catalog: &catalog,
        global_snapshot: &global_snapshot,
    };
    Ok(scan_project_with_baseline(
        &ctx,
        project_path,
        category,
        None,
    ))
}

fn scan_libraries(sources: &[LibrarySource]) -> LibraryCatalog {
    let mut catalog = LibraryCatalog {
        roots: Vec::new(),
        skills: BTreeMap::new(),
        rules: BTreeMap::new(),
        warnings: Vec::new(),
    };

    for src in sources {
        let raw_path = PathBuf::from(&src.path);
        let Ok(canonical_root) = fs::canonicalize(&raw_path) else {
            catalog.warnings.push(format!(
                "Library source '{}' not accessible: {}",
                src.label, src.path
            ));
            continue;
        };

        if !canonical_root.is_dir() {
            catalog.warnings.push(format!(
                "Library source '{}' is not a directory: {}",
                src.label, src.path
            ));
            continue;
        }

        catalog.roots.push(CanonicalLibraryRoot {
            label: src.label.clone(),
            raw_path: raw_path.clone(),
            canonical_path: canonical_root.clone(),
        });

        let direct_skills = collect_skills_in_parent(&canonical_root, &src.label, &src.path);
        let nested_skills_dir = canonical_root.join("skills");
        let nested_skills = if nested_skills_dir.is_dir() {
            collect_skills_in_parent(&nested_skills_dir, &src.label, &src.path)
        } else {
            Vec::new()
        };

        let found_skill_dirs = !direct_skills.is_empty() || !nested_skills.is_empty();
        for skill in direct_skills.into_iter().chain(nested_skills) {
            catalog.skills.entry(skill.slug.clone()).or_insert(skill);
        }

        let rules_subdir = canonical_root.join("rules");
        if rules_subdir.is_dir() {
            collect_library_rules_in_dir(&rules_subdir, &src.label, &mut catalog.rules);
        }

        // Only scan immediate root `*.md` as reusable rules when this library root isn't purely a skills folder
        // (avoids mistaking index docs like `Skill Library.md` inside a skills vault folder for agent rules).
        let label_lower = src.label.to_ascii_lowercase();
        let path_lower = canonical_root.to_string_lossy().to_ascii_lowercase();
        let looks_like_skills_root =
            found_skill_dirs && (label_lower.contains("skill") || path_lower.ends_with("/skills"));
        if !looks_like_skills_root {
            collect_library_rules_in_dir(&canonical_root, &src.label, &mut catalog.rules);
        }
    }

    catalog
}

fn collect_skills_in_parent(
    parent: &Path,
    label: &str,
    source_root: &str,
) -> Vec<LibrarySkillEntry> {
    let mut entries = Vec::new();
    let Ok(read_dir) = fs::read_dir(parent) else {
        return entries;
    };

    for entry in read_dir.flatten() {
        let path = entry.path();
        let Some(slug) = entry.file_name().to_str().map(ToOwned::to_owned) else {
            continue;
        };
        if slug.starts_with('.') {
            continue;
        }
        if !path.is_dir() {
            continue;
        }
        let skill_md = path.join("SKILL.md");
        if !skill_md.is_file() {
            continue;
        }

        if let Some(parsed_entry) = parse_library_skill(&slug, &path, &skill_md, label, source_root)
        {
            entries.push(parsed_entry);
        }
    }

    entries
}

fn parse_library_skill(
    slug: &str,
    skill_dir: &Path,
    skill_md: &Path,
    label: &str,
    source_root: &str,
) -> Option<LibrarySkillEntry> {
    let content = fs::read_to_string(skill_md).ok()?;
    let parsed = parse_markdown_frontmatter(&content);
    let name = parsed.name.clone().unwrap_or_else(|| slug.to_owned());
    let description = parsed.description.clone().unwrap_or_default();
    let description_too_long = description.chars().count() > DESCRIPTION_WARNING_CHARS;
    let upfront_tokens = tokenizer::skill_upfront_tokens(&name, &description);
    let on_demand_tokens =
        tokenizer::count_tokens(&parsed.body) + count_reference_tokens(skill_dir);
    let canonical_dir = fs::canonicalize(skill_dir).unwrap_or_else(|_| skill_dir.to_path_buf());
    let canonical_md = fs::canonicalize(skill_md).unwrap_or_else(|_| skill_md.to_path_buf());

    Some(LibrarySkillEntry {
        slug: slug.to_owned(),
        name,
        description,
        description_too_long,
        upfront_tokens,
        on_demand_tokens,
        label: label.to_owned(),
        source_root: source_root.to_owned(),
        canonical_dir,
        skill_md_path: canonical_md,
        frontmatter_error: parsed.yaml_error,
    })
}

fn collect_library_rules_in_dir(
    dir: &Path,
    source_label: &str,
    out: &mut BTreeMap<String, LibraryRuleEntry>,
) {
    let Ok(read_dir) = fs::read_dir(dir) else {
        return;
    };

    for entry in read_dir.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(filename) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !filename.ends_with(".md") || is_ignored_library_markdown(filename) {
            continue;
        }
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        let canonical_path = fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        let parsed = parse_markdown_frontmatter(&content);
        let (trigger, frontmatter_error) = classify_rule_trigger(&parsed, false);
        let full_tokens = tokenizer::count_tokens(&content);
        let upfront_tokens =
            rule_upfront_tokens(trigger, parsed.description.as_deref(), full_tokens);
        let key = canonical_path.to_string_lossy().into_owned();

        out.entry(key.clone()).or_insert(LibraryRuleEntry {
            id: key,
            filename: filename.to_owned(),
            canonical_path,
            trigger,
            description: parsed.description,
            glob_pattern: parsed.glob_pattern,
            upfront_tokens,
            full_tokens,
            source_label: source_label.to_owned(),
            frontmatter_error,
        });
    }
}

fn is_ignored_library_markdown(filename: &str) -> bool {
    let upper = filename.to_ascii_uppercase();
    filename.starts_with('_')
        || filename.starts_with('.')
        || upper == "SKILL.MD"
        || upper == "README.MD"
        || upper == "INDEX.MD"
        || upper == "AGENTS.MD"
        || upper == "CLAUDE.MD"
        || upper == "GEMINI.MD"
        || upper == "SKILL LIBRARY.MD"
}

fn count_reference_tokens(skill_dir: &Path) -> usize {
    let refs_dir = skill_dir.join("references");
    if !refs_dir.is_dir() {
        return 0;
    }
    let mut total = 0;
    for entry in walkdir::WalkDir::new(refs_dir)
        .max_depth(3)
        .into_iter()
        .flatten()
    {
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("md") {
            if let Ok(content) = fs::read_to_string(path) {
                total += tokenizer::count_tokens(&content);
            }
        }
    }
    total
}

fn inspect_target(path: &Path) -> Option<TargetResolution> {
    let meta = fs::symlink_metadata(path).ok()?;
    let is_symlink = meta.file_type().is_symlink();
    if !is_symlink {
        let canonical = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        return Some(TargetResolution::Valid {
            canonical,
            is_symlink: false,
        });
    }

    let raw_target = match fs::read_link(path) {
        Ok(t) => t,
        Err(_) => {
            return Some(TargetResolution::BrokenSymlink {
                target: "unreadable symlink".to_owned(),
            });
        }
    };

    let joined = if raw_target.is_absolute() {
        raw_target.clone()
    } else {
        path.parent()
            .map(|p| p.join(&raw_target))
            .unwrap_or_else(|| raw_target.clone())
    };

    match fs::canonicalize(&joined) {
        Ok(canonical) if canonical.exists() => Some(TargetResolution::Valid {
            canonical,
            is_symlink: true,
        }),
        _ => Some(TargetResolution::BrokenSymlink {
            target: raw_target.to_string_lossy().into_owned(),
        }),
    }
}

fn attribute_library_source(canonical: &Path, library: &LibraryCatalog) -> Option<SkillSource> {
    for root in &library.roots {
        if canonical.starts_with(&root.canonical_path) {
            return Some(SkillSource::Library {
                label: root.label.clone(),
                path: root.raw_path.to_string_lossy().into_owned(),
            });
        }
    }
    None
}

fn scan_global_scope(
    cfg: &AppConfig,
    home: &Path,
    library: &LibraryCatalog,
    catalog: &McpCatalog,
    discovered_mcp: Option<&mut Vec<McpCatalogEntry>>,
) -> ScopeSnapshot {
    let adapters = enabled_adapters(cfg.active_agents);
    let mut warnings = library.warnings.clone();

    let (skills, unmanaged_count) = scan_global_skills(&adapters, home, library, &mut warnings);
    let mcp_servers = scan_global_mcp(&adapters, home, catalog, discovered_mcp, &mut warnings);
    let rules = scan_global_rules(&adapters, home, library);

    let skills_global: usize = skills
        .iter()
        .filter(|s| s.active_globally)
        .map(|s| s.upfront_tokens)
        .sum();
    let mcp_global: usize = mcp_servers
        .iter()
        .filter(|m| m.active_globally)
        .map(|m| m.upfront_tokens)
        .sum();
    let rules_global: usize = rules
        .iter()
        .filter(|r| r.active_globally)
        .map(|r| r.upfront_tokens)
        .sum();

    let skills_mcp_global = skills_global + mcp_global;
    let budget = TokenBudgetBreakdown {
        rules_global,
        rules_project: 0,
        rules_total: rules_global,
        rules_limit: TOKEN_BUDGET_LIMIT,
        skills_global,
        skills_project: 0,
        mcp_global,
        mcp_project: 0,
        skills_mcp_global,
        skills_mcp_project: 0,
        skills_mcp_total: skills_mcp_global,
        skills_mcp_limit: TOKEN_BUDGET_LIMIT,
        effective_total: rules_global + skills_mcp_global,
    };

    ScopeSnapshot {
        scope_type: ScopeType::Global,
        scope_id: "global".to_owned(),
        title: "Global".to_owned(),
        path: Some(home.to_string_lossy().into_owned()),
        budget,
        skills,
        unmanaged_global_skills_count: unmanaged_count,
        mcp_servers,
        rules,
        support_docs: Vec::new(),
        scan_warnings: warnings,
    }
}

fn collect_all_global_skill_dirs(home: &Path) -> Vec<PathBuf> {
    let candidates = [
        home.join(".gemini/config/skills"),
        home.join(".gemini/antigravity/skills"),
        home.join(".gemini/antigravity-cli/skills"),
        home.join(".gemini/skills"),
        home.join(".agents/skills"),
        home.join(".claude/skills"),
    ];
    candidates
        .into_iter()
        .filter_map(|p| fs::canonicalize(&p).ok())
        .collect()
}

fn scan_global_skills(
    adapters: &[Box<dyn AgentAdapter>],
    home: &Path,
    library: &LibraryCatalog,
    warnings: &mut Vec<String>,
) -> (Vec<SkillItem>, usize) {
    let mut map: BTreeMap<String, SkillItem> = BTreeMap::new();
    let global_dirs_canonical = collect_all_global_skill_dirs(home);

    for (slug, lib) in &library.skills {
        map.insert(
            slug.clone(),
            SkillItem {
                id: slug.clone(),
                name: lib.name.clone(),
                description: lib.description.clone(),
                description_too_long: lib.description_too_long,
                upfront_tokens: lib.upfront_tokens,
                on_demand_tokens: lib.on_demand_tokens,
                source: SkillSource::Library {
                    label: lib.label.clone(),
                    path: lib.source_root.clone(),
                },
                canonical_path: lib.skill_md_path.to_string_lossy().into_owned(),
                skill_dir_path: lib.canonical_dir.to_string_lossy().into_owned(),
                is_symlink: false,
                is_unmanaged_global: false,
                active_globally: false,
                active_in_project: false,
                active_agents: Vec::new(),
                installed_paths: Vec::new(),
                frontmatter_error: lib.frontmatter_error.clone(),
            },
        );
    }

    for adapter in adapters {
        let agent = adapter.kind();
        for dir_spec in adapter.global_skill_dirs(home) {
            if !dir_spec.path.is_dir() {
                continue;
            }
            let Ok(read_dir) = fs::read_dir(&dir_spec.path) else {
                warnings.push(format!(
                    "Unable to read skill directory: {}",
                    dir_spec.path.display()
                ));
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

                let Some(resolution) = inspect_target(&entry_path) else {
                    continue;
                };

                match resolution {
                    TargetResolution::BrokenSymlink { target } => {
                        let installed = InstalledPathInfo {
                            agent,
                            path: entry_path.to_string_lossy().into_owned(),
                            is_symlink: true,
                            is_legacy: dir_spec.is_legacy,
                            broken: true,
                        };
                        let item = map.entry(slug.clone()).or_insert_with(|| SkillItem {
                            id: slug.clone(),
                            name: slug.clone(),
                            description: format!("Broken symlink pointing to {target}"),
                            description_too_long: false,
                            upfront_tokens: 0,
                            on_demand_tokens: 0,
                            source: SkillSource::BrokenSymlink {
                                target: target.clone(),
                            },
                            canonical_path: entry_path.to_string_lossy().into_owned(),
                            skill_dir_path: entry_path.to_string_lossy().into_owned(),
                            is_symlink: true,
                            is_unmanaged_global: false,
                            active_globally: true,
                            active_in_project: false,
                            active_agents: Vec::new(),
                            installed_paths: Vec::new(),
                            frontmatter_error: Some(format!("Broken symlink -> {target}")),
                        });
                        item.active_globally = true;
                        push_unique_agent(&mut item.active_agents, agent);
                        item.installed_paths.push(installed);
                    }
                    TargetResolution::Valid {
                        canonical,
                        is_symlink,
                    } => {
                        let skill_md = canonical.join("SKILL.md");
                        if !canonical.is_dir() || !skill_md.is_file() {
                            continue;
                        }

                        // A directory is unmanaged in global scope if it lives directly as a real directory
                        // inside any global skill dir (including when another agent symlinks into `~/.agents/skills/<name>`).
                        let lives_inside_global_dir = !is_symlink
                            || global_dirs_canonical
                                .iter()
                                .any(|g| canonical.parent() == Some(g.as_path()));
                        let library_source = attribute_library_source(&canonical, library);
                        let is_unmanaged = library_source.is_none() && lives_inside_global_dir;

                        let installed = InstalledPathInfo {
                            agent,
                            path: entry_path.to_string_lossy().into_owned(),
                            is_symlink,
                            is_legacy: dir_spec.is_legacy,
                            broken: false,
                        };

                        if let Some(existing) = map.get_mut(&slug) {
                            existing.active_globally = true;
                            existing.is_symlink |= is_symlink;
                            existing.is_unmanaged_global |= is_unmanaged;
                            if is_unmanaged && matches!(existing.source, SkillSource::GlobalOnly) {
                                existing.source = SkillSource::GlobalOnly;
                            }
                            push_unique_agent(&mut existing.active_agents, agent);
                            existing.installed_paths.push(installed);
                        } else if let Some(parsed) =
                            parse_skill_from_disk(&slug, &canonical, &skill_md)
                        {
                            let source = library_source.unwrap_or(SkillSource::GlobalOnly);
                            map.insert(
                                slug.clone(),
                                SkillItem {
                                    id: slug,
                                    name: parsed.name,
                                    description: parsed.description,
                                    description_too_long: parsed.description_too_long,
                                    upfront_tokens: parsed.upfront_tokens,
                                    on_demand_tokens: parsed.on_demand_tokens,
                                    source,
                                    canonical_path: parsed
                                        .skill_md_path
                                        .to_string_lossy()
                                        .into_owned(),
                                    skill_dir_path: canonical.to_string_lossy().into_owned(),
                                    is_symlink,
                                    is_unmanaged_global: is_unmanaged,
                                    active_globally: true,
                                    active_in_project: false,
                                    active_agents: vec![agent],
                                    installed_paths: vec![installed],
                                    frontmatter_error: parsed.frontmatter_error,
                                },
                            );
                        }
                    }
                }
            }
        }
    }

    let mut skills: Vec<SkillItem> = map.into_values().collect();
    skills.sort_by_key(|a| a.name.to_lowercase());
    let unmanaged_count = skills
        .iter()
        .filter(|s| s.active_globally && s.is_unmanaged_global)
        .count();

    (skills, unmanaged_count)
}

struct ParsedSkillDisk {
    name: String,
    description: String,
    description_too_long: bool,
    upfront_tokens: usize,
    on_demand_tokens: usize,
    skill_md_path: PathBuf,
    frontmatter_error: Option<String>,
}

fn parse_skill_from_disk(
    slug: &str,
    canonical_dir: &Path,
    skill_md: &Path,
) -> Option<ParsedSkillDisk> {
    let content = fs::read_to_string(skill_md).ok()?;
    let parsed = parse_markdown_frontmatter(&content);
    let name = parsed.name.unwrap_or_else(|| slug.to_owned());
    let description = parsed.description.unwrap_or_default();
    let description_too_long = description.chars().count() > DESCRIPTION_WARNING_CHARS;
    let upfront_tokens = tokenizer::skill_upfront_tokens(&name, &description);
    let on_demand_tokens =
        tokenizer::count_tokens(&parsed.body) + count_reference_tokens(canonical_dir);

    Some(ParsedSkillDisk {
        name,
        description,
        description_too_long,
        upfront_tokens,
        on_demand_tokens,
        skill_md_path: fs::canonicalize(skill_md).unwrap_or_else(|_| skill_md.to_path_buf()),
        frontmatter_error: parsed.yaml_error,
    })
}

fn scan_global_mcp(
    adapters: &[Box<dyn AgentAdapter>],
    home: &Path,
    catalog: &McpCatalog,
    mut discovered_mcp: Option<&mut Vec<McpCatalogEntry>>,
    warnings: &mut Vec<String>,
) -> Vec<McpServerItem> {
    let mut map: BTreeMap<String, McpServerItem> = BTreeMap::new();

    for entry in catalog.servers.values() {
        let (upfront_tokens, is_estimated) = match entry.upfront_tokens {
            Some(tok) if tok > 0 => (tok, false),
            _ => (DEFAULT_MCP_ESTIMATE_TOKENS, true),
        };
        let command_or_url = entry
            .command
            .clone()
            .or_else(|| entry.url.clone())
            .unwrap_or_default();
        map.insert(
            entry.name.clone(),
            McpServerItem {
                name: entry.name.clone(),
                transport: entry.transport,
                command_or_url,
                args: entry.args.clone(),
                upfront_tokens,
                is_estimated,
                tool_count: entry.tool_count,
                probed_at_ms: entry.probed_at_ms,
                active_globally: false,
                active_in_project: false,
                defined_globally_collision: false,
                active_agents: Vec::new(),
                source_configs: Vec::new(),
            },
        );
    }

    for adapter in adapters {
        let agent = adapter.kind();
        for spec in adapter.global_mcp_configs(home) {
            if !spec.path.is_file() {
                continue;
            }
            let parsed = parse_mcp_file(&spec.path, agent, None, warnings);
            if let Some(sink) = discovered_mcp.as_deref_mut() {
                collect_catalog_entries(&parsed, sink);
            }
            merge_mcp_records(&mut map, parsed, true, false, catalog);
        }
    }

    map.into_values().collect()
}

struct RawMcpRecord {
    name: String,
    transport: McpTransport,
    command_or_url: String,
    args: Vec<String>,
    env: BTreeMap<String, String>,
    headers: BTreeMap<String, String>,
    enabled: bool,
    agent: AgentKind,
    config_path: String,
}

fn collect_catalog_entries(records: &[RawMcpRecord], out: &mut Vec<McpCatalogEntry>) {
    for rec in records {
        let (command, url) = match rec.transport {
            McpTransport::Stdio => (
                (!rec.command_or_url.is_empty()).then(|| rec.command_or_url.clone()),
                None,
            ),
            McpTransport::Http | McpTransport::Sse => (
                None,
                (!rec.command_or_url.is_empty()).then(|| rec.command_or_url.clone()),
            ),
            McpTransport::Unknown => (
                (!rec.command_or_url.is_empty()).then(|| rec.command_or_url.clone()),
                None,
            ),
        };
        out.push(McpCatalogEntry {
            name: rec.name.clone(),
            transport: rec.transport,
            command,
            args: rec.args.clone(),
            url,
            env: rec.env.clone(),
            headers: rec.headers.clone(),
            upfront_tokens: None,
            tool_count: None,
            probed_at_ms: None,
        });
    }
}

fn parse_mcp_file(
    path: &Path,
    agent: AgentKind,
    claude_project_path: Option<&Path>,
    warnings: &mut Vec<String>,
) -> Vec<RawMcpRecord> {
    let Ok(raw) = fs::read_to_string(path) else {
        warnings.push(format!("Unable to read MCP config: {}", path.display()));
        return Vec::new();
    };

    if path.extension().and_then(|e| e.to_str()) == Some("toml") {
        return parse_codex_toml_mcp(&raw, path, agent, warnings);
    }

    let Ok(json) = serde_json::from_str::<JsonValue>(&raw) else {
        warnings.push(format!("Invalid JSON in MCP config: {}", path.display()));
        return Vec::new();
    };

    let mut records = Vec::new();
    let config_str = path.to_string_lossy().into_owned();

    if let Some(project_path) = claude_project_path {
        let canonical_proj = fs::canonicalize(project_path)
            .unwrap_or_else(|_| project_path.to_path_buf())
            .to_string_lossy()
            .into_owned();
        let raw_proj = project_path.to_string_lossy().into_owned();

        if let Some(projects_obj) = json.get("projects").and_then(JsonValue::as_object) {
            let entry = projects_obj
                .get(&canonical_proj)
                .or_else(|| projects_obj.get(&raw_proj));
            if let Some(servers_obj) = entry
                .and_then(|p| p.get("mcpServers"))
                .and_then(JsonValue::as_object)
            {
                collect_json_mcp_object(servers_obj, agent, &config_str, &mut records);
            }
        }
    } else if let Some(servers_obj) = json.get("mcpServers").and_then(JsonValue::as_object) {
        collect_json_mcp_object(servers_obj, agent, &config_str, &mut records);
    }

    records
}

fn collect_json_mcp_object(
    servers_obj: &serde_json::Map<String, JsonValue>,
    agent: AgentKind,
    config_path: &str,
    out: &mut Vec<RawMcpRecord>,
) {
    for (name, val) in servers_obj {
        let Some(obj) = val.as_object() else {
            continue;
        };
        let disabled = obj
            .get("disabled")
            .and_then(JsonValue::as_bool)
            .unwrap_or(false);
        let command = obj
            .get("command")
            .and_then(JsonValue::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let url = obj
            .get("url")
            .or_else(|| obj.get("serverUrl"))
            .or_else(|| obj.get("httpUrl"))
            .and_then(JsonValue::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let type_str = obj
            .get("type")
            .and_then(JsonValue::as_str)
            .map(str::to_ascii_lowercase);

        let transport = match type_str.as_deref() {
            Some("sse") => McpTransport::Sse,
            Some("http") | Some("streamable-http") | Some("streamable_http") => McpTransport::Http,
            Some("stdio") => McpTransport::Stdio,
            _ if command.is_some() => McpTransport::Stdio,
            _ if url.is_some() => McpTransport::Http,
            _ => McpTransport::Unknown,
        };

        let command_or_url = command.or(url).unwrap_or("").to_owned();
        let args = obj
            .get("args")
            .and_then(JsonValue::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(ToOwned::to_owned))
                    .collect()
            })
            .unwrap_or_default();

        let env = extract_json_string_map(obj.get("env"));
        let headers = extract_json_string_map(obj.get("headers"));

        out.push(RawMcpRecord {
            name: name.clone(),
            transport,
            command_or_url,
            args,
            env,
            headers,
            enabled: !disabled,
            agent,
            config_path: config_path.to_owned(),
        });
    }
}

fn extract_json_string_map(val: Option<&JsonValue>) -> BTreeMap<String, String> {
    let Some(obj) = val.and_then(JsonValue::as_object) else {
        return BTreeMap::new();
    };
    obj.iter()
        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_owned())))
        .collect()
}

fn parse_codex_toml_mcp(
    raw: &str,
    path: &Path,
    agent: AgentKind,
    warnings: &mut Vec<String>,
) -> Vec<RawMcpRecord> {
    let Ok(doc) = raw.parse::<DocumentMut>() else {
        warnings.push(format!("Invalid TOML in Codex config: {}", path.display()));
        return Vec::new();
    };
    let Some(servers_table) = doc.get("mcp_servers").and_then(|item| item.as_table_like()) else {
        return Vec::new();
    };

    let config_str = path.to_string_lossy().into_owned();
    let mut records = Vec::new();

    for (name, item) in servers_table.iter() {
        let Some(table) = item.as_table_like() else {
            continue;
        };
        let enabled = table
            .get("enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let command = table
            .get("command")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let url = table
            .get("url")
            .or_else(|| table.get("http_url"))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty());

        let transport = if command.is_some() {
            McpTransport::Stdio
        } else if url.is_some() {
            McpTransport::Http
        } else {
            McpTransport::Unknown
        };

        let args = table
            .get("args")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(ToOwned::to_owned))
                    .collect()
            })
            .unwrap_or_default();

        let env = table
            .get("env")
            .and_then(|v| v.as_table_like())
            .map(|t| {
                t.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.to_owned(), s.to_owned())))
                    .collect()
            })
            .unwrap_or_default();

        let headers = table
            .get("http_headers")
            .and_then(|v| v.as_table_like())
            .map(|t| {
                t.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.to_owned(), s.to_owned())))
                    .collect()
            })
            .unwrap_or_default();

        records.push(RawMcpRecord {
            name: name.to_owned(),
            transport,
            command_or_url: command.or(url).unwrap_or("").to_owned(),
            args,
            env,
            headers,
            enabled,
            agent,
            config_path: config_str.clone(),
        });
    }

    records
}

fn merge_mcp_records(
    map: &mut BTreeMap<String, McpServerItem>,
    records: Vec<RawMcpRecord>,
    is_global_scope: bool,
    is_project_scope: bool,
    catalog: &McpCatalog,
) {
    for rec in records {
        let cached = catalog.servers.get(&rec.name);
        let (upfront_tokens, is_estimated, tool_count, probed_at_ms) = match cached {
            Some(c) if c.upfront_tokens.is_some_and(|t| t > 0) => (
                c.upfront_tokens.unwrap_or(DEFAULT_MCP_ESTIMATE_TOKENS),
                false,
                c.tool_count,
                c.probed_at_ms,
            ),
            _ => (DEFAULT_MCP_ESTIMATE_TOKENS, true, None, None),
        };

        let entry = map
            .entry(rec.name.clone())
            .or_insert_with(|| McpServerItem {
                name: rec.name.clone(),
                transport: rec.transport,
                command_or_url: rec.command_or_url.clone(),
                args: rec.args.clone(),
                upfront_tokens,
                is_estimated,
                tool_count,
                probed_at_ms,
                active_globally: false,
                active_in_project: false,
                defined_globally_collision: false,
                active_agents: Vec::new(),
                source_configs: Vec::new(),
            });

        if is_global_scope && rec.enabled {
            entry.active_globally = true;
        }
        if is_project_scope && rec.enabled {
            entry.active_in_project = true;
        }
        if entry.command_or_url.is_empty() && !rec.command_or_url.is_empty() {
            entry.command_or_url = rec.command_or_url;
            entry.args = rec.args;
            entry.transport = rec.transport;
        }
        push_unique_agent(&mut entry.active_agents, rec.agent);
        push_unique_string(&mut entry.source_configs, rec.config_path);
    }
}

fn scan_global_rules(
    adapters: &[Box<dyn AgentAdapter>],
    home: &Path,
    library: &LibraryCatalog,
) -> Vec<RuleItem> {
    let mut map: BTreeMap<String, RuleItem> = BTreeMap::new();

    for lib_rule in library.rules.values() {
        map.insert(
            lib_rule.id.clone(),
            RuleItem {
                id: lib_rule.id.clone(),
                filename: lib_rule.filename.clone(),
                path: lib_rule.canonical_path.to_string_lossy().into_owned(),
                canonical_path: lib_rule.canonical_path.to_string_lossy().into_owned(),
                trigger: lib_rule.trigger,
                description: lib_rule.description.clone(),
                glob_pattern: lib_rule.glob_pattern.clone(),
                upfront_tokens: lib_rule.upfront_tokens,
                full_tokens: lib_rule.full_tokens,
                source_label: lib_rule.source_label.clone(),
                is_symlink: false,
                is_library_rule: true,
                active_globally: false,
                active_in_project: false,
                active_agents: Vec::new(),
                frontmatter_error: lib_rule.frontmatter_error.clone(),
                is_editable_trigger: true,
            },
        );
    }

    for adapter in adapters {
        let agent = adapter.kind();
        for spec in adapter.global_rule_files(home) {
            scan_single_rule_file(
                &spec,
                agent,
                RuleFileKind::InstructionFile,
                library,
                true,
                false,
                &mut map,
            );
        }
        for dir_spec in adapter.global_rule_dirs(home) {
            scan_rule_dir(&dir_spec, agent, library, true, false, &mut map);
        }
    }

    let mut rules: Vec<RuleItem> = map.into_values().collect();
    rules.sort_by_key(|a| a.filename.to_lowercase());
    rules
}

#[derive(Clone, Copy)]
enum RuleFileKind {
    InstructionFile,
    DirectoryRule { is_claude_rule: bool },
}

fn scan_rule_dir(
    dir_spec: &PathSpec,
    agent: AgentKind,
    library: &LibraryCatalog,
    is_global: bool,
    is_project: bool,
    map: &mut BTreeMap<String, RuleItem>,
) {
    if !dir_spec.path.is_dir() {
        return;
    }
    let Ok(read_dir) = fs::read_dir(&dir_spec.path) else {
        return;
    };

    let is_claude_rule = agent == AgentKind::Claude;
    for entry in read_dir.flatten() {
        let path = entry.path();
        let Some(name) = entry.file_name().to_str().map(ToOwned::to_owned) else {
            continue;
        };
        if name.starts_with('.') || !name.ends_with(".md") {
            continue;
        }
        // Antigravity scans immediate `*.md` children only (flat rule scanning).
        let spec = PathSpec {
            path,
            is_legacy: dir_spec.is_legacy,
        };
        scan_single_rule_file(
            &spec,
            agent,
            RuleFileKind::DirectoryRule { is_claude_rule },
            library,
            is_global,
            is_project,
            map,
        );
    }
}

fn scan_single_rule_file(
    spec: &PathSpec,
    agent: AgentKind,
    kind: RuleFileKind,
    library: &LibraryCatalog,
    is_global: bool,
    is_project: bool,
    map: &mut BTreeMap<String, RuleItem>,
) {
    let Some(resolution) = inspect_target(&spec.path) else {
        return;
    };

    let filename = spec
        .path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("rule.md")
        .to_owned();

    match resolution {
        TargetResolution::BrokenSymlink { target } => {
            let key = spec.path.to_string_lossy().into_owned();
            let item = map.entry(key.clone()).or_insert_with(|| RuleItem {
                id: key.clone(),
                filename,
                path: key.clone(),
                canonical_path: key,
                trigger: RuleTrigger::Invalid,
                description: None,
                glob_pattern: None,
                upfront_tokens: 0,
                full_tokens: 0,
                source_label: "Broken Symlink".to_owned(),
                is_symlink: true,
                is_library_rule: false,
                active_globally: is_global,
                active_in_project: is_project,
                active_agents: Vec::new(),
                frontmatter_error: Some(format!("Broken symlink -> {target}")),
                is_editable_trigger: false,
            });
            push_unique_agent(&mut item.active_agents, agent);
        }
        TargetResolution::Valid {
            canonical,
            is_symlink,
        } => {
            if !canonical.is_file() {
                return;
            }
            let Ok(content) = fs::read_to_string(&canonical) else {
                return;
            };
            let canonical_str = canonical.to_string_lossy().into_owned();

            if let Some(existing) = map.get_mut(&canonical_str) {
                if is_global {
                    existing.active_globally = true;
                }
                if is_project {
                    existing.active_in_project = true;
                }
                existing.is_symlink |= is_symlink;
                push_unique_agent(&mut existing.active_agents, agent);
                return;
            }

            let full_tokens = tokenizer::count_tokens(&content);
            let (trigger, description, glob_pattern, upfront_tokens, frontmatter_error, editable) =
                match kind {
                    RuleFileKind::InstructionFile => {
                        (RuleTrigger::AlwaysOn, None, None, full_tokens, None, false)
                    }
                    RuleFileKind::DirectoryRule { is_claude_rule } => {
                        let parsed = parse_markdown_frontmatter(&content);
                        let (trig, err) = classify_rule_trigger(&parsed, is_claude_rule);
                        let up =
                            rule_upfront_tokens(trig, parsed.description.as_deref(), full_tokens);
                        (
                            trig,
                            parsed.description,
                            parsed.glob_pattern,
                            up,
                            err,
                            !is_claude_rule,
                        )
                    }
                };

            let source_label = match attribute_library_source(&canonical, library) {
                Some(SkillSource::Library { label, .. }) => label,
                _ if is_global => "Global".to_owned(),
                _ => "Project".to_owned(),
            };

            map.insert(
                canonical_str.clone(),
                RuleItem {
                    id: canonical_str.clone(),
                    filename,
                    path: spec.path.to_string_lossy().into_owned(),
                    canonical_path: canonical_str,
                    trigger,
                    description,
                    glob_pattern,
                    upfront_tokens,
                    full_tokens,
                    source_label,
                    is_symlink,
                    is_library_rule: false,
                    active_globally: is_global,
                    active_in_project: is_project,
                    active_agents: vec![agent],
                    frontmatter_error,
                    is_editable_trigger: editable,
                },
            );
        }
    }
}

fn rule_upfront_tokens(
    trigger: RuleTrigger,
    description: Option<&str>,
    full_tokens: usize,
) -> usize {
    match trigger {
        RuleTrigger::AlwaysOn | RuleTrigger::Invalid => full_tokens,
        RuleTrigger::ModelDecision => description.map(tokenizer::count_tokens).unwrap_or(0),
        RuleTrigger::Glob => 0,
    }
}

fn discover_projects(
    cfg: &AppConfig,
    home: &Path,
    library: &LibraryCatalog,
    catalog: &McpCatalog,
    global_snapshot: &ScopeSnapshot,
) -> Vec<CategoryGroup> {
    let ignore_set: BTreeSet<String> = cfg
        .ignore_folders
        .iter()
        .map(|s| s.to_ascii_lowercase())
        .collect();
    let pinned_canonicals: BTreeSet<PathBuf> = cfg
        .pinned_projects
        .iter()
        .filter_map(|p| fs::canonicalize(p).ok())
        .collect();

    let mut seen_projects: BTreeSet<PathBuf> = BTreeSet::new();
    let mut groups: Vec<CategoryGroup> = Vec::new();

    if let Some(root_str) = &cfg.projects_root {
        let root = PathBuf::from(root_str);
        if root.is_dir() {
            if let Ok(cat_entries) = fs::read_dir(&root) {
                let mut cat_dirs: Vec<(String, PathBuf)> = cat_entries
                    .flatten()
                    .filter_map(|e| {
                        let p = e.path();
                        let name = e.file_name().to_str()?.to_owned();
                        if !p.is_dir() || is_ignored_dir(&name, &ignore_set) {
                            return None;
                        }
                        Some((name, p))
                    })
                    .collect();

                cat_dirs.sort_by_key(|a| a.0.to_lowercase());

                for (cat_name, cat_path) in cat_dirs {
                    let Ok(proj_entries) = fs::read_dir(&cat_path) else {
                        continue;
                    };
                    let mut projects = Vec::new();
                    for entry in proj_entries.flatten() {
                        let proj_path = entry.path();
                        let Some(proj_name) = entry.file_name().to_str().map(ToOwned::to_owned)
                        else {
                            continue;
                        };
                        if !proj_path.is_dir() || is_ignored_dir(&proj_name, &ignore_set) {
                            continue;
                        }
                        let canonical =
                            fs::canonicalize(&proj_path).unwrap_or_else(|_| proj_path.clone());
                        if !seen_projects.insert(canonical.clone()) {
                            continue;
                        }
                        let is_pinned = pinned_canonicals.contains(&canonical);
                        let ctx = ScanContext {
                            cfg,
                            home,
                            library,
                            catalog,
                            global_snapshot,
                        };
                        let snap = scan_project_with_baseline(&ctx, &proj_path, &cat_name, None);
                        projects.push(build_project_summary(
                            &proj_name, &cat_name, &proj_path, is_pinned, &snap,
                        ));
                    }

                    projects.sort_by_key(|a| a.name.to_lowercase());
                    if !projects.is_empty() {
                        groups.push(CategoryGroup {
                            name: cat_name,
                            projects,
                        });
                    }
                }
            }
        }
    }

    let mut pinned_extra = Vec::new();
    for pinned_str in &cfg.pinned_projects {
        let pinned_path = PathBuf::from(pinned_str);
        if !pinned_path.is_dir() {
            continue;
        }
        let canonical = fs::canonicalize(&pinned_path).unwrap_or_else(|_| pinned_path.clone());
        if !seen_projects.insert(canonical) {
            continue;
        }
        let proj_name = pinned_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Pinned")
            .to_owned();
        let ctx = ScanContext {
            cfg,
            home,
            library,
            catalog,
            global_snapshot,
        };
        let snap = scan_project_with_baseline(&ctx, &pinned_path, "Pinned", None);
        pinned_extra.push(build_project_summary(
            &proj_name,
            "Pinned",
            &pinned_path,
            true,
            &snap,
        ));
    }

    if !pinned_extra.is_empty() {
        pinned_extra.sort_by_key(|a| a.name.to_lowercase());
        groups.insert(
            0,
            CategoryGroup {
                name: "Pinned".to_owned(),
                projects: pinned_extra,
            },
        );
    }

    groups
}

fn is_ignored_dir(name: &str, ignore_set: &BTreeSet<String>) -> bool {
    name.starts_with('.') || ignore_set.contains(&name.to_ascii_lowercase())
}

fn build_project_summary(
    name: &str,
    category: &str,
    path: &Path,
    is_pinned: bool,
    snap: &ScopeSnapshot,
) -> ProjectSummary {
    let project_extra_tokens = snap.budget.rules_project + snap.budget.skills_mcp_project;
    let active_project_skills_count = snap
        .skills
        .iter()
        .filter(|s| s.active_in_project && !s.active_globally)
        .count();
    let active_project_mcp_count = snap
        .mcp_servers
        .iter()
        .filter(|m| m.active_in_project && !m.active_globally)
        .count();
    let active_project_rules_count = snap
        .rules
        .iter()
        .filter(|r| r.active_in_project && !r.active_globally)
        .count();

    ProjectSummary {
        id: path.to_string_lossy().into_owned(),
        name: name.to_owned(),
        category: category.to_owned(),
        path: path.to_string_lossy().into_owned(),
        is_pinned,
        has_git: path.join(".git").exists(),
        effective_tokens: snap.budget.effective_total,
        project_extra_tokens,
        active_project_skills_count,
        active_project_mcp_count,
        active_project_rules_count,
    }
}

fn scan_project_with_baseline(
    ctx: &ScanContext<'_>,
    project_path: &Path,
    category: &str,
    mut discovered_mcp: Option<&mut Vec<McpCatalogEntry>>,
) -> ScopeSnapshot {
    let ScanContext {
        cfg,
        home,
        library,
        catalog,
        global_snapshot,
    } = *ctx;
    let adapters = enabled_adapters(cfg.active_agents);
    let mut warnings = Vec::new();

    // 1. Skills: clone baseline items and overlay project installations
    let mut skill_map: BTreeMap<String, SkillItem> = global_snapshot
        .skills
        .iter()
        .cloned()
        .map(|s| (s.id.clone(), s))
        .collect();

    for adapter in &adapters {
        let agent = adapter.kind();
        for dir_spec in adapter.project_skill_dirs(project_path) {
            if !dir_spec.path.is_dir() {
                continue;
            }
            let Ok(read_dir) = fs::read_dir(&dir_spec.path) else {
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
                let Some(resolution) = inspect_target(&entry_path) else {
                    continue;
                };

                match resolution {
                    TargetResolution::BrokenSymlink { target } => {
                        let installed = InstalledPathInfo {
                            agent,
                            path: entry_path.to_string_lossy().into_owned(),
                            is_symlink: true,
                            is_legacy: dir_spec.is_legacy,
                            broken: true,
                        };
                        let item = skill_map.entry(slug.clone()).or_insert_with(|| SkillItem {
                            id: slug.clone(),
                            name: slug.clone(),
                            description: format!("Broken symlink pointing to {target}"),
                            description_too_long: false,
                            upfront_tokens: 0,
                            on_demand_tokens: 0,
                            source: SkillSource::BrokenSymlink {
                                target: target.clone(),
                            },
                            canonical_path: entry_path.to_string_lossy().into_owned(),
                            skill_dir_path: entry_path.to_string_lossy().into_owned(),
                            is_symlink: true,
                            is_unmanaged_global: false,
                            active_globally: false,
                            active_in_project: true,
                            active_agents: Vec::new(),
                            installed_paths: Vec::new(),
                            frontmatter_error: Some(format!("Broken symlink -> {target}")),
                        });
                        item.active_in_project = true;
                        push_unique_agent(&mut item.active_agents, agent);
                        item.installed_paths.push(installed);
                    }
                    TargetResolution::Valid {
                        canonical,
                        is_symlink,
                    } => {
                        let skill_md = canonical.join("SKILL.md");
                        if !canonical.is_dir() || !skill_md.is_file() {
                            continue;
                        }
                        let installed = InstalledPathInfo {
                            agent,
                            path: entry_path.to_string_lossy().into_owned(),
                            is_symlink,
                            is_legacy: dir_spec.is_legacy,
                            broken: false,
                        };

                        if let Some(existing) = skill_map.get_mut(&slug) {
                            existing.active_in_project = true;
                            existing.is_symlink |= is_symlink;
                            push_unique_agent(&mut existing.active_agents, agent);
                            existing.installed_paths.push(installed);
                        } else if let Some(parsed) =
                            parse_skill_from_disk(&slug, &canonical, &skill_md)
                        {
                            let source = attribute_library_source(&canonical, library)
                                .unwrap_or(SkillSource::ProjectOnly);
                            skill_map.insert(
                                slug.clone(),
                                SkillItem {
                                    id: slug,
                                    name: parsed.name,
                                    description: parsed.description,
                                    description_too_long: parsed.description_too_long,
                                    upfront_tokens: parsed.upfront_tokens,
                                    on_demand_tokens: parsed.on_demand_tokens,
                                    source,
                                    canonical_path: parsed
                                        .skill_md_path
                                        .to_string_lossy()
                                        .into_owned(),
                                    skill_dir_path: canonical.to_string_lossy().into_owned(),
                                    is_symlink,
                                    is_unmanaged_global: false,
                                    active_globally: false,
                                    active_in_project: true,
                                    active_agents: vec![agent],
                                    installed_paths: vec![installed],
                                    frontmatter_error: parsed.frontmatter_error,
                                },
                            );
                        }
                    }
                }
            }
        }
    }

    let mut skills: Vec<SkillItem> = skill_map.into_values().collect();
    skills.sort_by_key(|a| a.name.to_lowercase());

    // 2. MCP: overlay project MCP configs onto global baseline & detect global collisions
    let mut mcp_map: BTreeMap<String, McpServerItem> = global_snapshot
        .mcp_servers
        .iter()
        .cloned()
        .map(|m| (m.name.clone(), m))
        .collect();

    for adapter in &adapters {
        let agent = adapter.kind();
        for spec in adapter.project_mcp_configs(project_path) {
            if spec.path.is_file() {
                let parsed = parse_mcp_file(&spec.path, agent, None, &mut warnings);
                if let Some(sink) = discovered_mcp.as_deref_mut() {
                    collect_catalog_entries(&parsed, sink);
                }
                merge_mcp_records(&mut mcp_map, parsed, false, true, catalog);
            }
        }
        if agent == AgentKind::Claude {
            let claude_json = home.join(".claude.json");
            if claude_json.is_file() {
                let parsed = parse_mcp_file(&claude_json, agent, Some(project_path), &mut warnings);
                if let Some(sink) = discovered_mcp.as_deref_mut() {
                    collect_catalog_entries(&parsed, sink);
                }
                merge_mcp_records(&mut mcp_map, parsed, false, true, catalog);
            }
        }
    }

    for server in mcp_map.values_mut() {
        if server.active_globally && server.active_in_project {
            server.defined_globally_collision = true;
        }
    }
    let mcp_servers: Vec<McpServerItem> = mcp_map.into_values().collect();

    // 3. Rules: overlay project rules onto global + library baseline
    let mut rule_map: BTreeMap<String, RuleItem> = global_snapshot
        .rules
        .iter()
        .cloned()
        .map(|r| (r.id.clone(), r))
        .collect();

    for adapter in &adapters {
        let agent = adapter.kind();
        for spec in adapter.project_rule_files(project_path) {
            scan_single_rule_file(
                &spec,
                agent,
                RuleFileKind::InstructionFile,
                library,
                false,
                true,
                &mut rule_map,
            );
        }
        for dir_spec in adapter.project_rule_dirs(project_path) {
            scan_rule_dir(&dir_spec, agent, library, false, true, &mut rule_map);
        }
    }

    let mut rules: Vec<RuleItem> = rule_map.into_values().collect();
    rules.sort_by_key(|a| a.filename.to_lowercase());

    // 4. Support Docs
    let support_docs = scan_support_docs(project_path);

    // 5. Token budget breakdown (avoiding double counting any item already active globally)
    let rules_global = global_snapshot.budget.rules_global;
    let rules_project: usize = rules
        .iter()
        .filter(|r| r.active_in_project && !r.active_globally)
        .map(|r| r.upfront_tokens)
        .sum();
    let skills_global = global_snapshot.budget.skills_global;
    let skills_project: usize = skills
        .iter()
        .filter(|s| s.active_in_project && !s.active_globally)
        .map(|s| s.upfront_tokens)
        .sum();
    let mcp_global = global_snapshot.budget.mcp_global;
    let mcp_project: usize = mcp_servers
        .iter()
        .filter(|m| m.active_in_project && !m.active_globally)
        .map(|m| m.upfront_tokens)
        .sum();

    let skills_mcp_global = skills_global + mcp_global;
    let skills_mcp_project = skills_project + mcp_project;
    let rules_total = rules_global + rules_project;
    let skills_mcp_total = skills_mcp_global + skills_mcp_project;

    let project_name = project_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Project");
    let title = format!("{category} / {project_name}");

    ScopeSnapshot {
        scope_type: ScopeType::Project,
        scope_id: project_path.to_string_lossy().into_owned(),
        title,
        path: Some(project_path.to_string_lossy().into_owned()),
        budget: TokenBudgetBreakdown {
            rules_global,
            rules_project,
            rules_total,
            rules_limit: TOKEN_BUDGET_LIMIT,
            skills_global,
            skills_project,
            mcp_global,
            mcp_project,
            skills_mcp_global,
            skills_mcp_project,
            skills_mcp_total,
            skills_mcp_limit: TOKEN_BUDGET_LIMIT,
            effective_total: rules_total + skills_mcp_total,
        },
        skills,
        unmanaged_global_skills_count: 0,
        mcp_servers,
        rules,
        support_docs,
        scan_warnings: warnings,
    }
}

fn scan_support_docs(project_path: &Path) -> Vec<SupportDocItem> {
    let mut docs = Vec::new();
    let mut seen_paths = BTreeSet::new();

    for pointer_file in [
        project_path.join("AGENTS.md"),
        project_path.join(".agents/rules/support-docs.md"),
    ] {
        if let Ok(content) = fs::read_to_string(&pointer_file) {
            for doc in parse_support_docs_block(&content) {
                if seen_paths.insert(doc.path.clone()) {
                    docs.push(doc);
                }
            }
        }
    }

    let symlink_dir = project_path.join("_support-docs");
    if let Ok(read_dir) = fs::read_dir(&symlink_dir) {
        for entry in read_dir.flatten() {
            let entry_path = entry.path();
            let Some(name) = entry.file_name().to_str().map(ToOwned::to_owned) else {
                continue;
            };
            if name.starts_with('.') {
                continue;
            }
            if let Some(res) = inspect_target(&entry_path) {
                let (resolved_path, exists) = match res {
                    TargetResolution::Valid { canonical, .. } => {
                        (canonical.to_string_lossy().into_owned(), true)
                    }
                    TargetResolution::BrokenSymlink { target } => (target, false),
                };
                if seen_paths.insert(resolved_path.clone()) {
                    docs.push(SupportDocItem {
                        name,
                        path: resolved_path,
                        description: None,
                        mode: SupportDocMode::Symlink,
                        exists,
                    });
                }
            }
        }
    }

    docs
}

fn parse_support_docs_block(content: &str) -> Vec<SupportDocItem> {
    let Some(start) = content.find(SUPPORT_DOCS_START) else {
        return Vec::new();
    };
    let rest = &content[start + SUPPORT_DOCS_START.len()..];
    let Some(end) = rest.find(SUPPORT_DOCS_END) else {
        return Vec::new();
    };
    let block = &rest[..end];

    let mut out = Vec::new();
    for line in block.lines() {
        let trimmed = line.trim().trim_start_matches('-').trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(doc) = parse_support_doc_line(trimmed) {
            out.push(doc);
        }
    }
    out
}

fn parse_support_doc_line(line: &str) -> Option<SupportDocItem> {
    // Supports `- [Label](path) — description` or `- `path` — description`
    if let Some(after_bracket) = line.strip_prefix('[') {
        if let Some((name, tail)) = after_bracket.split_once("](") {
            if let Some((raw_path, desc_tail)) = tail.split_once(')') {
                let path = raw_path.strip_prefix("file://").unwrap_or(raw_path).trim();
                let description = clean_doc_description(desc_tail);
                return Some(SupportDocItem {
                    name: name.trim().to_owned(),
                    path: path.to_owned(),
                    description,
                    mode: SupportDocMode::Pointer,
                    exists: Path::new(path).exists(),
                });
            }
        }
    }

    if let Some(after_tick) = line.strip_prefix('`') {
        if let Some((raw_path, desc_tail)) = after_tick.split_once('`') {
            let path = raw_path.trim();
            let name = Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(path)
                .to_owned();
            return Some(SupportDocItem {
                name,
                path: path.to_owned(),
                description: clean_doc_description(desc_tail),
                mode: SupportDocMode::Pointer,
                exists: Path::new(path).exists(),
            });
        }
    }

    None
}

fn clean_doc_description(tail: &str) -> Option<String> {
    let cleaned = tail.trim().trim_start_matches(['—', '-', ':']).trim();
    (!cleaned.is_empty()).then(|| cleaned.to_owned())
}

fn push_unique_agent(vec: &mut Vec<AgentKind>, agent: AgentKind) {
    if !vec.contains(&agent) {
        vec.push(agent);
    }
}

fn push_unique_string(vec: &mut Vec<String>, val: String) {
    if !vec.contains(&val) {
        vec.push(val);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ActiveAgents;
    use std::os::unix::fs::symlink;
    use tempfile::TempDir;

    fn make_skill(dir: &Path, name: &str, desc: &str, body: &str) {
        fs::create_dir_all(dir).expect("create skill dir");
        let md = format!("---\nname: {name}\ndescription: {desc}\n---\n\n{body}\n");
        fs::write(dir.join("SKILL.md"), md).expect("write SKILL.md");
    }

    #[test]
    fn attributes_library_symlinks_detects_broken_and_unmanaged_and_dedups() {
        let tmp = TempDir::new().expect("temp dir");
        let root = tmp.path();
        let home = root.join("home");
        let vault_skills = root.join("BRAIN-LOCAL/09-ai-os/skills");
        let projects_root = root.join("APP-PROJECTS");

        fs::create_dir_all(&home).expect("home");
        fs::create_dir_all(&vault_skills).expect("vault");
        fs::create_dir_all(&projects_root).expect("projects");

        // Canonical skill in Obsidian library
        make_skill(
            &vault_skills.join("peter-writing"),
            "peter-writing",
            "Writes prose in Peter voice.",
            "Long detailed instructions body.",
        );

        // Symlinked into Antigravity primary AND Codex/legacy `.agents/skills` -> must dedup!
        let gemini_skills = home.join(".gemini/config/skills");
        let agents_skills = home.join(".agents/skills");
        fs::create_dir_all(&gemini_skills).expect("gemini skills");
        fs::create_dir_all(&agents_skills).expect("agents skills");

        symlink(
            vault_skills.join("peter-writing"),
            gemini_skills.join("peter-writing"),
        )
        .expect("symlink gemini");
        symlink(
            vault_skills.join("peter-writing"),
            agents_skills.join("peter-writing"),
        )
        .expect("symlink agents");

        // Real unmanaged skill sitting directly in `.agents/skills` AND symlinked from gemini
        make_skill(
            &agents_skills.join("rogue-pack"),
            "rogue-pack",
            "Unmanaged skill sitting directly in global skills dir.",
            "Body",
        );
        symlink(
            agents_skills.join("rogue-pack"),
            gemini_skills.join("rogue-pack"),
        )
        .expect("symlink rogue");

        // Broken symlink
        symlink(
            vault_skills.join("missing-skill"),
            gemini_skills.join("missing-skill"),
        )
        .expect("broken symlink");

        let cfg = AppConfig {
            projects_root: Some(projects_root.to_string_lossy().into_owned()),
            library_sources: vec![LibrarySource {
                id: "obsidian".to_owned(),
                label: "Obsidian".to_owned(),
                path: vault_skills.to_string_lossy().into_owned(),
            }],
            support_docs_root: None,
            pinned_projects: Vec::new(),
            ignore_folders: vec!["_archive".to_owned(), "node_modules".to_owned()],
            active_agents: ActiveAgents {
                antigravity: true,
                codex: true,
                claude: false,
            },
            git_hygiene: true,
        };

        let catalog = McpCatalog::default();
        let library = scan_libraries(&cfg.library_sources);
        let global = scan_global_scope(&cfg, &home, &library, &catalog, None);

        assert_eq!(global.skills.len(), 3);
        let writing = global
            .skills
            .iter()
            .find(|s| s.id == "peter-writing")
            .expect("peter-writing");
        assert!(writing.active_globally);
        assert!(!writing.is_unmanaged_global);
        assert!(
            matches!(writing.source, SkillSource::Library { ref label, .. } if label == "Obsidian")
        );
        assert_eq!(writing.active_agents.len(), 2);
        assert_eq!(
            global.budget.skills_global,
            writing.upfront_tokens
                + global
                    .skills
                    .iter()
                    .find(|s| s.id == "rogue-pack")
                    .expect("rogue")
                    .upfront_tokens
        );

        let rogue = global
            .skills
            .iter()
            .find(|s| s.id == "rogue-pack")
            .expect("rogue");
        assert!(rogue.is_unmanaged_global);
        assert_eq!(global.unmanaged_global_skills_count, 1);

        let broken = global
            .skills
            .iter()
            .find(|s| s.id == "missing-skill")
            .expect("broken");
        assert!(matches!(broken.source, SkillSource::BrokenSymlink { .. }));
    }

    #[test]
    fn scans_mcp_toml_json_collision_and_two_level_projects() {
        let tmp = TempDir::new().expect("temp dir");
        let root = tmp.path();
        let home = root.join("home");
        let projects_root = root.join("APP-PROJECTS");
        let proj_acme = projects_root.join("Clients/Acme");
        let archived = projects_root.join("_archive/OldSite");

        fs::create_dir_all(home.join(".gemini/config")).expect("gemini");
        fs::create_dir_all(home.join(".codex")).expect("codex");
        fs::create_dir_all(proj_acme.join(".agents/rules")).expect("acme rules");
        fs::create_dir_all(&archived).expect("archived");

        // Global Antigravity MCP + disabled server
        fs::write(
            home.join(".gemini/config/mcp_config.json"),
            r#"{"mcpServers":{"zeno":{"command":"npx","args":["-y","zeno"]},"off-srv":{"command":"echo","disabled":true}}}"#,
        )
        .expect("write global mcp");

        // Global Codex MCP with comments
        fs::write(
            home.join(".codex/config.toml"),
            "# keep comment\n[mcp_servers.zeno]\ncommand = \"npx\"\nargs = [\"-y\", \"zeno\"]\n",
        )
        .expect("write codex toml");

        // Project MCP colliding with `zeno` + project-only `local-db`
        fs::write(
            proj_acme.join(".agents/mcp_config.json"),
            r#"{"mcpServers":{"zeno":{"command":"npx"},"local-db":{"url":"http://localhost:3000/mcp"}}}"#,
        )
        .expect("write project mcp");

        // Project rules: AGENTS.md (shared Antigravity+Codex) + invalid rule frontmatter
        fs::write(proj_acme.join("AGENTS.md"), "Always follow strict types.").expect("AGENTS");
        fs::write(
            proj_acme.join(".agents/rules/bad.md"),
            "---\ntrigger: alwaysOn\n---\nBad camelCase",
        )
        .expect("bad rule");

        let cfg = AppConfig {
            projects_root: Some(projects_root.to_string_lossy().into_owned()),
            library_sources: Vec::new(),
            support_docs_root: None,
            pinned_projects: Vec::new(),
            ignore_folders: vec!["_archive".to_owned()],
            active_agents: ActiveAgents::default(),
            git_hygiene: true,
        };

        let catalog = McpCatalog::default();
        let library = scan_libraries(&cfg.library_sources);
        let global = scan_global_scope(&cfg, &home, &library, &catalog, None);
        let categories = discover_projects(&cfg, &home, &library, &catalog, &global);

        assert_eq!(categories.len(), 1);
        assert_eq!(categories[0].name, "Clients");
        assert_eq!(categories[0].projects.len(), 1);
        assert_eq!(categories[0].projects[0].name, "Acme");

        let ctx = ScanContext {
            cfg: &cfg,
            home: &home,
            library: &library,
            catalog: &catalog,
            global_snapshot: &global,
        };
        let proj_snap = scan_project_with_baseline(&ctx, &proj_acme, "Clients", None);

        let zeno = proj_snap
            .mcp_servers
            .iter()
            .find(|m| m.name == "zeno")
            .expect("zeno");
        assert!(zeno.defined_globally_collision);
        assert_eq!(proj_snap.budget.mcp_global, DEFAULT_MCP_ESTIMATE_TOKENS);
        assert_eq!(proj_snap.budget.mcp_project, DEFAULT_MCP_ESTIMATE_TOKENS);

        let off_srv = proj_snap
            .mcp_servers
            .iter()
            .find(|m| m.name == "off-srv")
            .expect("off-srv");
        assert!(!off_srv.active_globally);

        let bad_rule = proj_snap
            .rules
            .iter()
            .find(|r| r.filename == "bad.md")
            .expect("bad rule");
        assert_eq!(bad_rule.trigger, RuleTrigger::Invalid);
        assert!(bad_rule.frontmatter_error.is_some());

        let agents_rule = proj_snap
            .rules
            .iter()
            .find(|r| r.filename == "AGENTS.md")
            .expect("AGENTS.md");
        assert_eq!(agents_rule.active_agents.len(), 2);
    }

    #[test]
    #[ignore = "live filesystem diagnostic against Peter's local directories"]
    fn live_filesystem_diagnostic() {
        let cfg = config::load_config().expect("load live config");
        let overview = scan_workspace(&cfg).expect("scan live workspace");
        println!(
            "Global tokens: {} (Rules: {}, Skills: {}, MCP: {})",
            overview.global_tokens,
            overview.global_snapshot.budget.rules_total,
            overview.global_snapshot.budget.skills_global,
            overview.global_snapshot.budget.mcp_global
        );
        println!(
            "Global skills: {} total ({} active globally, {} unmanaged)",
            overview.global_snapshot.skills.len(),
            overview
                .global_snapshot
                .skills
                .iter()
                .filter(|s| s.active_globally)
                .count(),
            overview.global_snapshot.unmanaged_global_skills_count
        );
        println!("Global MCP: {}", overview.global_snapshot.mcp_servers.len());
        println!("Global Rules: {}", overview.global_snapshot.rules.len());
        println!("Categories: {}", overview.categories.len());
        for cat in &overview.categories {
            println!("  Category '{}': {} projects", cat.name, cat.projects.len());
            for p in cat.projects.iter().take(5) {
                println!(
                    "    - {} (effective={} tok, extra={} tok)",
                    p.name, p.effective_tokens, p.project_extra_tokens
                );
            }
        }
        assert!(!overview.global_snapshot.skills.is_empty());
    }
}

//! Safe YAML frontmatter `trigger` editor for `.agents/rules/*.md` and Library rules.

use std::fs;
use std::path::Path;

use serde_yaml::{Mapping, Value};

use crate::backup;
use crate::error::AppError;
use crate::frontmatter::parse_markdown_frontmatter;
use crate::models::RuleTriggerUpdate;

/// Updates only the YAML frontmatter trigger fields on `rule_path` while preserving markdown body.
///
/// Resolves symlinks to their canonical source before backing up and atomically rewriting.
///
/// # Errors
/// Returns [`AppError`] if the file cannot be read, resolved, backed up, serialized, or written.
pub fn update_rule_trigger(rule_path: &Path, update: &RuleTriggerUpdate) -> Result<(), AppError> {
    update_rule_trigger_with_backups(rule_path, update, None)
}

/// Variant of [`update_rule_trigger`] allowing a custom backups root for isolated testing.
///
/// # Errors
/// Returns [`AppError`] on I/O or YAML serialization failure.
pub fn update_rule_trigger_with_backups(
    rule_path: &Path,
    update: &RuleTriggerUpdate,
    custom_backups_root: Option<&Path>,
) -> Result<(), AppError> {
    let canonical = fs::canonicalize(rule_path).unwrap_or_else(|_| rule_path.to_path_buf());
    if !canonical.is_file() {
        return Err(AppError::InvalidPath(format!(
            "Rule file does not exist: {}",
            canonical.display()
        )));
    }

    let raw = fs::read_to_string(&canonical).map_err(|e| AppError::io(&canonical, e))?;
    let parsed = parse_markdown_frontmatter(&raw);

    let mut mapping = match &parsed.frontmatter_raw {
        Some(raw_yaml) => match serde_yaml::from_str::<Value>(raw_yaml) {
            Ok(Value::Mapping(m)) => m,
            _ => Mapping::new(),
        },
        None => Mapping::new(),
    };

    apply_trigger_to_mapping(&mut mapping, update)?;

    let serialized_yaml = serde_yaml::to_string(&Value::Mapping(mapping))
        .map_err(|e| AppError::Command(format!("Failed to serialize rule YAML: {e}")))?;
    let yaml_body = serialized_yaml
        .strip_prefix("---\n")
        .unwrap_or(&serialized_yaml)
        .trim_end();

    let body = parsed.body.trim_start_matches('\n');
    let next_contents = if body.is_empty() {
        format!("---\n{yaml_body}\n---\n")
    } else {
        format!("---\n{yaml_body}\n---\n\n{body}")
    };

    match custom_backups_root {
        Some(dir) => {
            backup::backup_file_in_dir(&canonical, dir)?;
        }
        None => {
            backup::backup_file(&canonical)?;
        }
    }

    backup::atomic_write_string(&canonical, &next_contents)
}

fn apply_trigger_to_mapping(
    mapping: &mut Mapping,
    update: &RuleTriggerUpdate,
) -> Result<(), AppError> {
    let trigger_key = Value::String("trigger".to_owned());
    let desc_key = Value::String("description".to_owned());
    let glob_key = Value::String("glob".to_owned());
    let globs_key = Value::String("globs".to_owned());

    match update {
        RuleTriggerUpdate::AlwaysOn => {
            mapping.insert(trigger_key, Value::String("always_on".to_owned()));
        }
        RuleTriggerUpdate::ModelDecision { description } => {
            let trimmed = description.trim();
            if trimmed.is_empty() {
                return Err(AppError::InvalidPath(
                    "`model_decision` rules require a short description".to_owned(),
                ));
            }
            mapping.insert(trigger_key, Value::String("model_decision".to_owned()));
            mapping.insert(desc_key, Value::String(trimmed.to_owned()));
            mapping.remove(&glob_key);
            mapping.remove(&globs_key);
        }
        RuleTriggerUpdate::Glob { pattern } => {
            let trimmed = pattern.trim();
            if trimmed.is_empty() {
                return Err(AppError::InvalidPath(
                    "`glob` rules require a file glob pattern (e.g. src/**/*.ts)".to_owned(),
                ));
            }
            mapping.insert(trigger_key, Value::String("glob".to_owned()));
            mapping.insert(glob_key, Value::String(trimmed.to_owned()));
            mapping.remove(&globs_key);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontmatter::classify_rule_trigger;
    use crate::models::RuleTrigger;
    use tempfile::TempDir;

    #[test]
    fn switches_trigger_modes_while_preserving_body_and_custom_keys() {
        let tmp = TempDir::new().expect("temp dir");
        let backups = tmp.path().join("backups");
        let rule_path = tmp.path().join("style.md");
        fs::write(
            &rule_path,
            "---\ntrigger: always_on\nowner: peter\n---\n\n# Style Guide\nKeep functions small.\n",
        )
        .expect("write rule");

        update_rule_trigger_with_backups(
            &rule_path,
            &RuleTriggerUpdate::ModelDecision {
                description: "Apply when styling React UI".to_owned(),
            },
            Some(&backups),
        )
        .expect("switch to model_decision");

        let updated = fs::read_to_string(&rule_path).expect("read updated");
        assert!(updated.contains("owner: peter"));
        assert!(updated.contains("# Style Guide\nKeep functions small."));
        let parsed = parse_markdown_frontmatter(&updated);
        assert_eq!(
            classify_rule_trigger(&parsed, false),
            (RuleTrigger::ModelDecision, None)
        );

        update_rule_trigger_with_backups(
            &rule_path,
            &RuleTriggerUpdate::Glob {
                pattern: "src/**/*.tsx".to_owned(),
            },
            Some(&backups),
        )
        .expect("switch to glob");

        let globbed = fs::read_to_string(&rule_path).expect("read globbed");
        let parsed_glob = parse_markdown_frontmatter(&globbed);
        assert_eq!(
            classify_rule_trigger(&parsed_glob, false),
            (RuleTrigger::Glob, None)
        );
        assert_eq!(parsed_glob.glob_pattern.as_deref(), Some("src/**/*.tsx"));
    }

    #[test]
    fn prepends_frontmatter_when_missing() {
        let tmp = TempDir::new().expect("temp dir");
        let backups = tmp.path().join("backups");
        let rule_path = tmp.path().join("legacy.md");
        fs::write(&rule_path, "# Plain rule\nDo not leak secrets.\n").expect("write legacy");

        update_rule_trigger_with_backups(&rule_path, &RuleTriggerUpdate::AlwaysOn, Some(&backups))
            .expect("prepend trigger");

        let content = fs::read_to_string(&rule_path).expect("read content");
        let parsed = parse_markdown_frontmatter(&content);
        assert_eq!(
            classify_rule_trigger(&parsed, false),
            (RuleTrigger::AlwaysOn, None)
        );
        assert!(content.contains("# Plain rule\nDo not leak secrets."));
    }
}

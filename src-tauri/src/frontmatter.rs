//! YAML frontmatter parsing and Antigravity rule validation.

use serde_yaml::{Mapping, Value};

use crate::models::RuleTrigger;

/// Parsed markdown file separated into optional YAML frontmatter and body content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedMarkdown {
    pub frontmatter_raw: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub trigger_raw: Option<String>,
    pub glob_pattern: Option<String>,
    pub paths: Vec<String>,
    pub body: String,
    pub yaml_error: Option<String>,
}

/// Splits leading YAML frontmatter (`---` ... `---`, LF or CRLF) from markdown body.
#[must_use]
pub fn parse_markdown_frontmatter(input: &str) -> ParsedMarkdown {
    let normalized = input.replace("\r\n", "\n");
    let stripped = normalized.strip_prefix('\u{feff}').unwrap_or(&normalized);

    let Some(after_open) = stripped.strip_prefix("---\n") else {
        return ParsedMarkdown {
            frontmatter_raw: None,
            name: None,
            description: None,
            trigger_raw: None,
            glob_pattern: None,
            paths: Vec::new(),
            body: normalized,
            yaml_error: None,
        };
    };

    let Some((raw_yaml, body_rest)) = split_closing_fence(after_open) else {
        return ParsedMarkdown {
            frontmatter_raw: None,
            name: None,
            description: None,
            trigger_raw: None,
            glob_pattern: None,
            paths: Vec::new(),
            body: normalized,
            yaml_error: Some("Unclosed YAML frontmatter fence".to_owned()),
        };
    };

    let mut out = ParsedMarkdown {
        frontmatter_raw: Some(raw_yaml.to_owned()),
        name: None,
        description: None,
        trigger_raw: None,
        glob_pattern: None,
        paths: Vec::new(),
        body: body_rest.to_owned(),
        yaml_error: None,
    };

    match serde_yaml::from_str::<Value>(raw_yaml) {
        Ok(Value::Mapping(map)) => populate_from_mapping(&map, &mut out),
        Ok(_) => {
            out.yaml_error = Some("Frontmatter must be a YAML mapping".to_owned());
            fallback_extract_lines(raw_yaml, &mut out);
        }
        Err(err) => {
            out.yaml_error = Some(format!("Malformed YAML frontmatter: {err}"));
            fallback_extract_lines(raw_yaml, &mut out);
        }
    }

    out
}

fn split_closing_fence(after_open: &str) -> Option<(&str, &str)> {
    if let Some(rest) = after_open.strip_prefix("---\n") {
        return Some(("", rest));
    }
    if let Some(idx) = after_open.find("\n---\n") {
        let yaml = &after_open[..idx];
        let body = &after_open[idx + "\n---\n".len()..];
        return Some((yaml, body));
    }
    if let Some(yaml) = after_open.strip_suffix("\n---") {
        return Some((yaml, ""));
    }
    None
}

fn populate_from_mapping(map: &Mapping, out: &mut ParsedMarkdown) {
    out.name = get_string(map, "name");
    out.description = get_string(map, "description");
    out.trigger_raw = get_string(map, "trigger");
    out.glob_pattern = get_string(map, "glob").or_else(|| get_string(map, "globs"));
    out.paths = get_string_list(map, "paths");
}

fn get_string(map: &Mapping, key: &str) -> Option<String> {
    let val = map.get(Value::String(key.to_owned()))?;
    match val {
        Value::String(s) => {
            let trimmed = s.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_owned())
        }
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn get_string_list(map: &Mapping, key: &str) -> Vec<String> {
    let Some(val) = map.get(Value::String(key.to_owned())) else {
        return Vec::new();
    };
    match val {
        Value::Sequence(seq) => seq
            .iter()
            .filter_map(|v| v.as_str().map(str::trim).filter(|s| !s.is_empty()))
            .map(ToOwned::to_owned)
            .collect(),
        Value::String(s) => s
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(ToOwned::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}

/// Resilient line-level extractor used only when strict YAML fails (e.g. unquoted colons in description).
fn fallback_extract_lines(raw_yaml: &str, out: &mut ParsedMarkdown) {
    for line in raw_yaml.lines() {
        let trimmed = line.trim();
        if let Some((key, val)) = trimmed.split_once(':') {
            let key = key.trim();
            let val = val.trim().trim_matches('"').trim_matches('\'').trim();
            if val.is_empty() {
                continue;
            }
            match key {
                "name" if out.name.is_none() => out.name = Some(val.to_owned()),
                "description" if out.description.is_none() => {
                    out.description = Some(val.to_owned())
                }
                "trigger" if out.trigger_raw.is_none() => out.trigger_raw = Some(val.to_owned()),
                "glob" | "globs" if out.glob_pattern.is_none() => {
                    out.glob_pattern = Some(val.to_owned());
                }
                _ => {}
            }
        }
    }
}

/// Classifies rule activation trigger and returns validation error message if invalid for Antigravity rules.
#[must_use]
pub fn classify_rule_trigger(
    parsed: &ParsedMarkdown,
    is_claude_rule: bool,
) -> (RuleTrigger, Option<String>) {
    if is_claude_rule && !parsed.paths.is_empty() && parsed.trigger_raw.is_none() {
        return (RuleTrigger::Glob, parsed.yaml_error.clone());
    }

    if let Some(err) = &parsed.yaml_error {
        return (RuleTrigger::Invalid, Some(err.clone()));
    }

    if parsed.frontmatter_raw.is_none() {
        return (
            RuleTrigger::Invalid,
            Some("Missing YAML frontmatter with snake_case `trigger` field".to_owned()),
        );
    }

    match parsed.trigger_raw.as_deref() {
        Some("always_on") => (RuleTrigger::AlwaysOn, None),
        Some("model_decision") => {
            if parsed.description.as_deref().map_or(true, str::is_empty) {
                (
                    RuleTrigger::ModelDecision,
                    Some("`model_decision` rule should define a `description`".to_owned()),
                )
            } else {
                (RuleTrigger::ModelDecision, None)
            }
        }
        Some("glob") => {
            if parsed.glob_pattern.as_deref().map_or(true, str::is_empty) && parsed.paths.is_empty()
            {
                (
                    RuleTrigger::Glob,
                    Some("`glob` rule is missing a `glob` pattern".to_owned()),
                )
            } else {
                (RuleTrigger::Glob, None)
            }
        }
        Some(other) => (
            RuleTrigger::Invalid,
            Some(format!(
                "Invalid `trigger: {other}` (expected `always_on`, `model_decision`, or `glob`)"
            )),
        ),
        None => (
            RuleTrigger::Invalid,
            Some("Missing `trigger` (`always_on`, `model_decision`, or `glob`)".to_owned()),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_skill_frontmatter_and_body() {
        let md = "---\nname: agentdock\ndescription: Native macOS context switchboard.\n---\n\n# Body\nHello";
        let parsed = parse_markdown_frontmatter(md);
        assert_eq!(parsed.name.as_deref(), Some("agentdock"));
        assert_eq!(
            parsed.description.as_deref(),
            Some("Native macOS context switchboard.")
        );
        assert_eq!(parsed.body.trim(), "# Body\nHello");
        assert!(parsed.yaml_error.is_none());
    }

    #[test]
    fn validates_snake_case_rule_triggers() {
        let ok = parse_markdown_frontmatter("---\ntrigger: always_on\n---\nRule body");
        assert_eq!(
            classify_rule_trigger(&ok, false),
            (RuleTrigger::AlwaysOn, None)
        );

        let camel = parse_markdown_frontmatter("---\ntrigger: alwaysOn\n---\nRule body");
        let (trigger, err) = classify_rule_trigger(&camel, false);
        assert_eq!(trigger, RuleTrigger::Invalid);
        assert!(err.unwrap_or_default().contains("alwaysOn"));

        let missing = parse_markdown_frontmatter("# No frontmatter\nRule body");
        let (trigger, err) = classify_rule_trigger(&missing, false);
        assert_eq!(trigger, RuleTrigger::Invalid);
        assert!(err.unwrap_or_default().contains("Missing YAML frontmatter"));
    }

    #[test]
    fn supports_fallback_when_description_has_unquoted_colon() {
        let md = "---\nname: colon-skill\ndescription: Trigger on: deploy, build\n---\nBody text";
        let parsed = parse_markdown_frontmatter(md);
        assert_eq!(parsed.name.as_deref(), Some("colon-skill"));
        assert_eq!(
            parsed.description.as_deref(),
            Some("Trigger on: deploy, build")
        );
        assert!(parsed.yaml_error.is_some());
    }

    #[test]
    fn supports_claude_paths_glob_rule() {
        let md = "---\npaths:\n  - src/**/*.rs\n---\nRust instructions";
        let parsed = parse_markdown_frontmatter(md);
        assert_eq!(
            classify_rule_trigger(&parsed, true),
            (RuleTrigger::Glob, None)
        );
    }
}

//! Skill loader, prompt enhancement discovery, and invocation coordinator.
//!
//! Discovers `.m31a/skills` and `SKILL.md` documents, injecting domain-specific guidance into agent context.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct SkillInterface {
    pub display_name: Option<String>,
    pub short_description: Option<String>,
    pub icon_small: Option<PathBuf>,
    pub icon_large: Option<PathBuf>,
    pub brand_color: Option<String>,
    pub default_prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillPolicy {
    pub allow_implicit_invocation: bool,
    pub allowed_products: Vec<String>,
}

impl Default for SkillPolicy {
    fn default() -> Self {
        Self {
            allow_implicit_invocation: true,
            allowed_products: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct SkillDependencies {
    pub binaries: Vec<String>,
    pub env_vars: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SkillScope {
    Workspace,
    User,
    Plugin,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillInfo {
    pub name: String,
    pub description: String,
    pub short_description: Option<String>,
    pub path: PathBuf,
    pub instructions: String,
    pub scope: SkillScope,
    pub interface: Option<SkillInterface>,
    pub policy: SkillPolicy,
    pub dependencies: SkillDependencies,
    pub plugin_id: Option<String>,
}

impl SkillInfo {
    /// Checks whether all binary and environment variable dependencies are met on this machine.
    pub fn check_dependencies(&self) -> Result<(), Vec<String>> {
        let mut missing = Vec::new();

        for bin in &self.dependencies.binaries {
            if which_binary(bin).is_none() {
                missing.push(format!("binary '{}' not found in PATH", bin));
            }
        }

        for var in &self.dependencies.env_vars {
            if std::env::var(var).is_err() {
                missing.push(format!("environment variable '{}' is not set", var));
            }
        }

        if missing.is_empty() {
            Ok(())
        } else {
            Err(missing)
        }
    }
}

fn which_binary(bin: &str) -> Option<PathBuf> {
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(bin);
            if candidate.is_file() {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(meta) = candidate.metadata()
                        && meta.permissions().mode() & 0o111 != 0
                    {
                        return Some(candidate);
                    }
                }
                #[cfg(not(unix))]
                return Some(candidate);
            }
        }
    }
    None
}

/// Extracted tool/skill mentions from text input.
#[derive(Debug, Default, Clone)]
pub struct ToolMentions {
    pub names: HashSet<String>,
    pub paths: HashSet<String>,
    pub plain_names: HashSet<String>,
}

impl ToolMentions {
    pub fn is_empty(&self) -> bool {
        self.names.is_empty() && self.paths.is_empty()
    }
}

/// Parses `$skill-name`, `@skill-name`, or `[$skill-name](resource_path)` mentions from user text.
pub fn extract_tool_mentions(text: &str) -> ToolMentions {
    let mut mentions = ToolMentions::default();
    let text_bytes = text.as_bytes();
    let mut i = 0;

    while i < text_bytes.len() {
        // Linked mention: [$name](path)
        if text_bytes[i] == b'['
            && let Some((name, path, next_i)) = parse_linked_mention(text, i)
        {
            if !is_common_env_var(&name) {
                mentions.names.insert(name.clone());
                mentions.paths.insert(path);
            }
            i = next_i;
            continue;
        }

        // Inline sigil: $name or @name
        if text_bytes[i] == b'$' || text_bytes[i] == b'@' {
            let start = i + 1;
            let mut end = start;
            while end < text_bytes.len() && is_mention_char(text_bytes[end]) {
                end += 1;
            }
            if end > start {
                let name = &text[start..end];
                if !is_common_env_var(name) {
                    mentions.names.insert(name.to_string());
                    mentions.plain_names.insert(name.to_string());
                }
                i = end;
                continue;
            }
        }

        i += 1;
    }

    mentions
}

fn parse_linked_mention(text: &str, start: usize) -> Option<(String, String, usize)> {
    let remaining = &text[start..];
    if !remaining.starts_with('[') {
        return None;
    }
    let close_bracket = remaining.find(']')?;
    let inner_name = &remaining[1..close_bracket];

    let clean_name = if inner_name.starts_with('$') || inner_name.starts_with('@') {
        &inner_name[1..]
    } else {
        inner_name
    };

    let rest = &remaining[close_bracket + 1..];
    let trimmed_rest = rest.trim_start();
    if !trimmed_rest.starts_with('(') {
        return None;
    }
    let close_paren = trimmed_rest.find(')')?;
    let path = trimmed_rest[1..close_paren].trim().to_string();

    let bytes_consumed = (close_bracket + 1) + (rest.len() - trimmed_rest.len()) + close_paren + 1;
    Some((clean_name.to_string(), path, start + bytes_consumed))
}

fn is_mention_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b':'
}

fn is_common_env_var(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    matches!(
        upper.as_str(),
        "PATH"
            | "HOME"
            | "USER"
            | "SHELL"
            | "PWD"
            | "TMPDIR"
            | "TEMP"
            | "TMP"
            | "LANG"
            | "TERM"
            | "XDG_CONFIG_HOME"
            | "EDITOR"
            | "TERM_PROGRAM"
    )
}

pub struct SkillLoader;

impl SkillLoader {
    /// Discovers all available skills across workspace roots (walking up to project root),
    /// user config (`.agents/skills`, `.m31a/skills`), and system/plugins (§4.5).
    pub fn discover_skills(cwd: &Path) -> Vec<SkillInfo> {
        let mut skill_roots = Vec::new();

        // 1. Workspace root skills — walk up from cwd to root (§4.5)
        let mut curr: Option<&Path> = Some(cwd);
        let mut visited_dirs = HashSet::new();
        while let Some(dir) = curr {
            if visited_dirs.contains(&dir.to_path_buf()) {
                break;
            }
            visited_dirs.insert(dir.to_path_buf());

            // Check standard ecosystem skill paths in order
            skill_roots.push((dir.join(".agents").join("skills"), SkillScope::Workspace));
            skill_roots.push((dir.join(".m31a").join("skills"), SkillScope::Workspace));
            skill_roots.push((dir.join(".codex").join("skills"), SkillScope::Workspace));
            skill_roots.push((dir.join("skills"), SkillScope::Workspace));

            // Stop walking up if we find a git boundary
            if dir.join(".git").exists() {
                break;
            }
            curr = dir.parent();
        }

        // 2. User home skills (~/.agents/skills, ~/.m31a/skills, ~/.gemini/skills)
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let home_path = PathBuf::from(home);
        skill_roots.push((home_path.join(".agents").join("skills"), SkillScope::User));
        skill_roots.push((home_path.join(".m31a").join("skills"), SkillScope::User));
        skill_roots.push((home_path.join(".codex").join("skills"), SkillScope::User));
        skill_roots.push((home_path.join(".gemini").join("skills"), SkillScope::User));
        skill_roots.push((
            home_path.join(".gemini").join("config").join("skills"),
            SkillScope::User,
        ));

        // 3. System / Admin skills
        skill_roots.push((
            home_path
                .join(".gemini")
                .join("antigravity-cli")
                .join("builtin")
                .join("skills"),
            SkillScope::System,
        ));
        skill_roots.push((PathBuf::from("/etc/m31a/skills"), SkillScope::System));
        skill_roots.push((PathBuf::from("/etc/codex/skills"), SkillScope::System));

        let mut discovered = Vec::new();

        for (root, scope) in skill_roots {
            if root.exists() && root.is_dir() {
                Self::scan_directory(&root, scope, &mut discovered);
            }
        }

        discovered
    }

    fn scan_directory(dir: &Path, scope: SkillScope, acc: &mut Vec<SkillInfo>) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let skill_file = path.join("SKILL.md");
                    if skill_file.exists() {
                        if let Ok(skill) = Self::parse_skill_file(&skill_file, scope)
                            && !acc.iter().any(|s| s.name == skill.name)
                        {
                            acc.push(skill);
                        }
                    } else {
                        Self::scan_directory(&path, scope, acc);
                    }
                } else if path.file_name().and_then(|n| n.to_str()) == Some("SKILL.md")
                    && let Ok(skill) = Self::parse_skill_file(&path, scope)
                    && !acc.iter().any(|s| s.name == skill.name)
                {
                    acc.push(skill);
                }
            }
        }
    }

    pub fn parse_skill_file(path: &Path, scope: SkillScope) -> Result<SkillInfo> {
        let text = fs::read_to_string(path)
            .with_context(|| format!("Failed to read skill file: {:?}", path))?;

        let mut name = String::new();
        let mut description = String::new();
        let mut short_description = None;
        let mut instructions = String::new();
        let mut allow_implicit = true;
        let mut binaries = Vec::new();
        let mut env_vars = Vec::new();
        let mut display_name = None;

        let lines: Vec<&str> = text.lines().collect();
        if lines.first().map(|l| l.trim()) == Some("---") {
            let mut end_frontmatter_idx = None;
            let mut current_section: Option<&str> = None;

            for (idx, line) in lines.iter().enumerate().skip(1) {
                if line.trim() == "---" {
                    end_frontmatter_idx = Some(idx);
                    break;
                }

                let trimmed = line.trim();
                if trimmed.starts_with('-') && current_section.is_some() {
                    let item = trimmed
                        .trim_start_matches('-')
                        .trim()
                        .trim_matches('"')
                        .trim_matches('\'');
                    match current_section {
                        Some("binaries") => binaries.push(item.to_string()),
                        Some("env_vars") => env_vars.push(item.to_string()),
                        _ => {}
                    }
                    continue;
                }

                if let Some((k, v)) = line.split_once(':') {
                    let key = k.trim();
                    let val = v.trim().trim_matches('"').trim_matches('\'');
                    current_section = None;

                    match key {
                        "name" => name = val.to_string(),
                        "description" => description = val.to_string(),
                        "short_description" => short_description = Some(val.to_string()),
                        "display_name" => display_name = Some(val.to_string()),
                        "allow_implicit_invocation" => {
                            allow_implicit = val.parse().unwrap_or(true);
                        }
                        "binaries" => {
                            current_section = Some("binaries");
                        }
                        "env_vars" => {
                            current_section = Some("env_vars");
                        }
                        _ => {}
                    }
                }
            }

            if let Some(end_idx) = end_frontmatter_idx {
                instructions = lines[end_idx + 1..].join("\n").trim().to_string();
            }
        } else {
            instructions = text.trim().to_string();
        }

        if name.is_empty() {
            name = path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("unnamed_skill")
                .to_string();
        }

        let interface = if display_name.is_some() || short_description.is_some() {
            Some(SkillInterface {
                display_name,
                short_description: short_description.clone(),
                ..Default::default()
            })
        } else {
            None
        };

        Ok(SkillInfo {
            name,
            description,
            short_description,
            path: path.to_path_buf(),
            instructions,
            scope,
            interface,
            policy: SkillPolicy {
                allow_implicit_invocation: allow_implicit,
                allowed_products: Vec::new(),
            },
            dependencies: SkillDependencies { binaries, env_vars },
            plugin_id: None,
        })
    }

    /// Resolves which skills are explicitly mentioned in the user prompt.
    pub fn resolve_explicit_mentions(prompt: &str, skills: &[SkillInfo]) -> Vec<SkillInfo> {
        let mentions = extract_tool_mentions(prompt);
        if mentions.is_empty() {
            return Vec::new();
        }

        let mut matched = Vec::new();
        for skill in skills {
            let path_str = skill.path.to_string_lossy();
            if mentions.names.contains(&skill.name)
                || mentions.paths.contains(path_str.as_ref())
                || (skill.path.parent().is_some()
                    && mentions
                        .paths
                        .contains(skill.path.parent().unwrap().to_str().unwrap_or("")))
            {
                matched.push(skill.clone());
            }
        }
        matched
    }

    /// Renders a concise skills catalog for system prompt injection.
    pub fn render_skills_prompt(skills: &[SkillInfo]) -> String {
        if skills.is_empty() {
            return String::new();
        }

        let mut prompt = String::from("\n## Available Skills Catalog\n\n");
        for s in skills {
            prompt.push_str(&format!(
                "- **{}** (`{}`): {}\n",
                s.name,
                s.path.display(),
                s.description
            ));
        }
        prompt.push_str("\nYou can inspect detailed instructions for any skill by invoking the `use_skill` tool with the skill's name.\n");
        prompt
    }

    /// Renders full instructions for explicitly mentioned skills.
    pub fn render_explicit_instructions(skills: &[SkillInfo]) -> String {
        if skills.is_empty() {
            return String::new();
        }

        let mut out = String::from("\n## Active Skill Instructions (Explicitly Requested)\n\n");
        for s in skills {
            out.push_str(&format!("### Skill: {}\n", s.name));
            if let Some(desc) = &s.short_description {
                out.push_str(&format!("*{desc}*\n\n"));
            }
            out.push_str(&s.instructions);
            out.push_str("\n\n---\n\n");
        }
        out
    }

    /// Detects implicit skill invocations from shell commands or queries (§4.5).
    /// Parses shell commands for skill doc reads or script runs.
    pub fn detect_implicit_invocation(
        command_line: &str,
        skills: &[SkillInfo],
    ) -> Option<SkillInfo> {
        let trimmed = command_line.trim();
        for skill in skills {
            if !skill.policy.allow_implicit_invocation {
                continue;
            }
            // Check if command references skill name or skill directory/scripts
            let name_pattern = format!("skills/{}", skill.name);
            let skill_dir_name = skill
                .path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("");
            if trimmed.contains(&name_pattern)
                || (!skill_dir_name.is_empty() && trimmed.contains(skill_dir_name))
                || trimmed.contains(&format!("${}", skill.name))
            {
                return Some(skill.clone());
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_parse_skill_file() {
        let dir = tempdir().unwrap();
        let skill_file = dir.path().join("SKILL.md");
        let content = r#"---
name: code-auditor
description: Performs rigorous static security and sanity audits on codebases.
short_description: Security auditor
allow_implicit_invocation: false
binaries:
  - cargo
  - git
---

# Instructions
Run cargo clippy and audit dependencies for CVEs.
"#;
        fs::write(&skill_file, content).unwrap();

        let skill = SkillLoader::parse_skill_file(&skill_file, SkillScope::Workspace).unwrap();
        assert_eq!(skill.name, "code-auditor");
        assert_eq!(
            skill.description,
            "Performs rigorous static security and sanity audits on codebases."
        );
        assert_eq!(skill.short_description.as_deref(), Some("Security auditor"));
        assert!(!skill.policy.allow_implicit_invocation);
        assert_eq!(skill.dependencies.binaries, vec!["cargo", "git"]);
        assert!(skill.instructions.contains("Run cargo clippy"));
    }

    #[test]
    fn test_extract_tool_mentions() {
        let text = "Please use $reverse-engineer-anything and @code-auditor for this task, but ignore $PATH and $HOME.";
        let mentions = extract_tool_mentions(text);

        assert!(mentions.names.contains("reverse-engineer-anything"));
        assert!(mentions.names.contains("code-auditor"));
        assert!(!mentions.names.contains("PATH"));
        assert!(!mentions.names.contains("HOME"));
    }

    #[test]
    fn test_parse_linked_mention() {
        let text = "Check out [$my-custom-skill](/path/to/skill/SKILL.md) and see if it works.";
        let mentions = extract_tool_mentions(text);

        assert!(mentions.names.contains("my-custom-skill"));
        assert!(mentions.paths.contains("/path/to/skill/SKILL.md"));
    }

    #[test]
    fn test_resolve_explicit_mentions() {
        let dir = tempdir().unwrap();
        let skill_file = dir.path().join("SKILL.md");
        fs::write(
            &skill_file,
            "---\nname: my-skill\ndescription: test\n---\nInstructions",
        )
        .unwrap();

        let skill = SkillLoader::parse_skill_file(&skill_file, SkillScope::Workspace).unwrap();
        let all_skills = vec![skill];

        let matched = SkillLoader::resolve_explicit_mentions("Can you run $my-skill?", &all_skills);
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].name, "my-skill");

        let unmatched =
            SkillLoader::resolve_explicit_mentions("Can you run $other-skill?", &all_skills);
        assert_eq!(unmatched.len(), 0);
    }

    #[test]
    fn test_detect_implicit_invocation() {
        let dir = tempdir().unwrap();
        let skill_dir = dir.path().join("code-reviewer");
        fs::create_dir_all(&skill_dir).unwrap();
        let skill_file = skill_dir.join("SKILL.md");
        fs::write(&skill_file, "---\nname: code-reviewer\ndescription: Reviewer\nallow_implicit_invocation: true\n---\nInstructions").unwrap();

        let skill = SkillLoader::parse_skill_file(&skill_file, SkillScope::Workspace).unwrap();
        let skills = vec![skill];

        let matched = SkillLoader::detect_implicit_invocation(
            "python3 code-reviewer/scripts/audit.py",
            &skills,
        );
        assert!(matched.is_some());
        assert_eq!(matched.unwrap().name, "code-reviewer");
    }
}

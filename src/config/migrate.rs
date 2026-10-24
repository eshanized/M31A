//! Non-destructive migration from legacy TOML definitions to canonical Markdown with YAML front matter.
//!
//! Provides conversion functions and automated workspace/global migration for:
//! - Prompts (`*.toml` -> `*.md`)
//! - Profiles (`assets/profiles/*.toml` -> `assets/profiles/*.md`)
//! - Skills (`assets/skills/*/SKILL.toml` -> `assets/skills/*/skill.md`)
//! - Verification adapters (`assets/verification_adapters/*.toml` -> `assets/verification_adapters/*.md`)
//! - User-defined slash commands (`prompts/commands/*.toml` -> `prompts/commands/*.md`)
//! - Declarative workflow manifests (`*.toml` -> `*.md`)
//! - Configuration files (`config.toml`, `M31A.toml` -> `config.md`)

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::document::{
    DocumentKind, MarkdownDocument, atomic_write_document, serialize_markdown_document,
};
use crate::prompt::contract::PromptContract;

/// Converts a `toml::Value` directly into an equivalent `serde_yaml::Value`.
pub fn toml_to_yaml_value(toml_val: toml::Value) -> Result<serde_yaml::Value, String> {
    serde_yaml::to_value(&toml_val).map_err(|e| format!("Failed to convert TOML to YAML: {e}"))
}

/// Converts a legacy TOML prompt string into a canonical Markdown document with YAML front matter.
pub fn convert_prompt_toml_to_markdown(toml_str: &str) -> Result<String, String> {
    // 1. Verify that the TOML parses into a valid PromptContract
    let parsed_contract = PromptContract::from_toml_str(toml_str)
        .map_err(|e| format!("Invalid prompt contract: {e}"))?;

    // 2. Parse raw TOML value to extract metadata and template body
    let mut toml_val: toml::Value = toml_str
        .parse()
        .map_err(|e: toml::de::Error| format!("TOML syntax error: {e}"))?;

    let template_body = if let toml::Value::Table(ref mut tbl) = toml_val {
        let body = if let Some(toml::Value::Table(body_tbl)) = tbl.remove("body") {
            body_tbl
                .get("template")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        } else if let Some(toml::Value::Table(template_tbl)) = tbl.remove("template") {
            template_tbl
                .get("body")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        } else {
            None
        };
        body.unwrap_or_else(|| parsed_contract.template_body.clone())
    } else {
        parsed_contract.template_body.clone()
    };

    // 3. Convert metadata table to YAML
    let yaml_data = toml_to_yaml_value(toml_val)?;

    // 4. Construct MarkdownDocument
    let doc = MarkdownDocument::new(DocumentKind::Prompt, 1, yaml_data, template_body);

    // 5. Serialize to Markdown
    let md_str =
        serialize_markdown_document(&doc).map_err(|e| format!("Serialization error: {e}"))?;

    // 6. Verify round-trip parse
    let reparsed = PromptContract::from_markdown_str(&md_str)
        .map_err(|e| format!("Reparsed contract invalid: {e}"))?;

    if reparsed.id != parsed_contract.id || reparsed.version != parsed_contract.version {
        return Err(format!(
            "Identity mismatch after conversion: expected {} v{}, got {} v{}",
            parsed_contract.id, parsed_contract.version, reparsed.id, reparsed.version
        ));
    }

    Ok(md_str)
}

/// Converts a legacy TOML configuration string into canonical Markdown format.
pub fn convert_config_toml_to_markdown(toml_str: &str) -> Result<String, String> {
    let toml_val: toml::Value = toml_str
        .parse()
        .map_err(|e: toml::de::Error| format!("TOML syntax error: {e}"))?;

    // Validate that it parses as AppConfig
    let _app_config: crate::config::schema::AppConfig = toml_val
        .clone()
        .try_into()
        .map_err(|e: toml::de::Error| format!("Invalid configuration: {e}"))?;

    let yaml_data = toml_to_yaml_value(toml_val)?;

    let doc = MarkdownDocument::new(
        DocumentKind::Config,
        1,
        yaml_data,
        "# M31A Configuration\n\nAuthoritative configuration values are stored in the YAML front matter above.\n",
    );

    serialize_markdown_document(&doc).map_err(|e| format!("Serialization error: {e}"))
}

/// Converts a legacy TOML profile string into canonical Markdown format.
pub fn convert_profile_toml_to_markdown(
    toml_str: &str,
    profile_name: &str,
) -> Result<String, String> {
    let toml_val: toml::Value = toml_str
        .parse()
        .map_err(|e: toml::de::Error| format!("TOML syntax error: {e}"))?;

    let yaml_data = toml_to_yaml_value(toml_val)?;

    let body = format!(
        "# Profile: {profile_name}\n\nExecution profile declaring capabilities, runtime constraints, and policy overrides.\n"
    );

    let doc = MarkdownDocument::new(DocumentKind::Profile, 1, yaml_data, body);
    serialize_markdown_document(&doc).map_err(|e| format!("Serialization error: {e}"))
}

/// Converts a legacy SKILL.toml string into canonical Markdown format.
pub fn convert_skill_toml_to_markdown(toml_str: &str) -> Result<String, String> {
    let manifest = crate::skill::manifest::SkillManifest::parse_toml(toml_str)?;

    let toml_val: toml::Value = toml_str
        .parse()
        .map_err(|e: toml::de::Error| format!("TOML syntax error: {e}"))?;

    let yaml_data = toml_to_yaml_value(toml_val)?;

    let body = format!(
        "# {}\n\n{}\n",
        manifest.name,
        manifest.procedure.instructions.trim()
    );

    let doc = MarkdownDocument::new(DocumentKind::Skill, 1, yaml_data, body);
    serialize_markdown_document(&doc).map_err(|e| format!("Serialization error: {e}"))
}

/// Converts a legacy verification adapter TOML string into canonical Markdown format.
pub fn convert_adapter_toml_to_markdown(
    toml_str: &str,
    adapter_name: &str,
) -> Result<String, String> {
    let toml_val: toml::Value = toml_str
        .parse()
        .map_err(|e: toml::de::Error| format!("TOML syntax error: {e}"))?;

    let yaml_data = toml_to_yaml_value(toml_val)?;

    let body = format!(
        "# {adapter_name} Verification Adapter\n\nCanonical build, test, and lint commands for {adapter_name} projects.\n"
    );

    let doc = MarkdownDocument::new(DocumentKind::Adapter, 1, yaml_data, body);
    serialize_markdown_document(&doc).map_err(|e| format!("Serialization error: {e}"))
}

/// Converts a legacy user command TOML string into canonical Markdown format.
pub fn convert_command_toml_to_markdown(toml_str: &str) -> Result<String, String> {
    let parsed = crate::interaction::user_commands::parse_user_command_toml(toml_str, None)
        .map_err(|e| format!("Invalid command TOML: {e}"))?;

    let mut toml_val: toml::Value = toml_str
        .parse()
        .map_err(|e: toml::de::Error| format!("TOML syntax error: {e}"))?;

    let template_body = if let toml::Value::Table(ref mut tbl) = toml_val {
        if let Some(toml::Value::Table(template_tbl)) = tbl.remove("template") {
            template_tbl
                .get("body")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| parsed.template_body.clone())
        } else {
            parsed.template_body.clone()
        }
    } else {
        parsed.template_body.clone()
    };

    let yaml_data = toml_to_yaml_value(toml_val)?;

    let doc = MarkdownDocument::new(DocumentKind::Command, 1, yaml_data, template_body.trim());
    serialize_markdown_document(&doc).map_err(|e| format!("Serialization error: {e}"))
}

/// Converts a legacy workflow manifest TOML string into canonical Markdown format.
pub fn convert_workflow_toml_to_markdown(toml_str: &str) -> Result<String, String> {
    let parsed = crate::workflow::manifest::WorkflowManifest::from_toml_str(toml_str)
        .map_err(|e| format!("Invalid workflow TOML: {e}"))?;

    let toml_val: toml::Value = toml_str
        .parse()
        .map_err(|e: toml::de::Error| format!("TOML syntax error: {e}"))?;

    let yaml_data = toml_to_yaml_value(toml_val)?;

    let body = format!(
        "# {}\n\n{}\n",
        parsed.workflow.name,
        parsed.workflow.description.trim()
    );

    let doc = MarkdownDocument::new(DocumentKind::Workflow, 1, yaml_data, body);
    serialize_markdown_document(&doc).map_err(|e| format!("Serialization error: {e}"))
}

/// Converts a legacy TOML policy string into canonical Markdown format with YAML front matter.
pub fn convert_policy_toml_to_markdown(toml_str: &str) -> Result<String, String> {
    let parsed_doc = crate::policy::rule::PolicyDocument::from_toml_str(toml_str)
        .map_err(|e| format!("Invalid policy document: {e}"))?;

    let toml_val: toml::Value = toml_str
        .parse()
        .map_err(|e: toml::de::Error| format!("TOML syntax error: {e}"))?;

    let yaml_data = toml_to_yaml_value(toml_val)?;

    let name = parsed_doc.name.as_deref().unwrap_or("Policy");
    let body =
        format!("# {name}\n\nDeclarative policy rules defined in the YAML front matter above.\n");

    let doc = MarkdownDocument::new(DocumentKind::Policy, 1, yaml_data, body);
    let md_str =
        serialize_markdown_document(&doc).map_err(|e| format!("Serialization error: {e}"))?;

    // Verify round-trip parse
    crate::policy::rule::PolicyDocument::from_markdown_str(&md_str)
        .map_err(|e| format!("Reparsed policy document invalid: {e}"))?;

    Ok(md_str)
}

/// Entry in a migration report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationEntry {
    pub legacy_path: String,
    pub canonical_path: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Summary report emitted by a migration run.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MigrationReport {
    pub migrated: Vec<MigrationEntry>,
    pub already_current: Vec<String>,
    pub skipped: Vec<String>,
    pub conflicts: Vec<String>,
    pub errors: Vec<MigrationEntry>,
}

/// Discovers and migrates legacy TOML files in a workspace or specific file path non-destructively.
pub fn migrate_path(target: &Path, dry_run: bool, in_place: bool) -> MigrationReport {
    let mut report = MigrationReport::default();

    if target.is_file() {
        migrate_single_file(target, dry_run, in_place, &mut report);
    } else if target.is_dir() {
        collect_and_migrate_dir(target, dry_run, in_place, &mut report, 0);
    }

    report
}

fn collect_and_migrate_dir(
    dir: &Path,
    dry_run: bool,
    in_place: bool,
    report: &mut MigrationReport,
    depth: usize,
) {
    if depth > 10 {
        return;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let filename = path.file_name().unwrap_or_default().to_string_lossy();
        if filename.starts_with('.') && filename != ".m31a" {
            continue;
        }
        if filename == "target" || filename == ".git" {
            continue;
        }
        if path.is_dir() {
            collect_and_migrate_dir(&path, dry_run, in_place, report, depth + 1);
        } else if path.is_file() && path.extension().is_some_and(|e| e == "toml") {
            // Exclude Cargo.toml, rust-toolchain.toml, etc.
            if filename == "Cargo.toml"
                || filename == "rust-toolchain.toml"
                || filename == "Cargo.lock"
            {
                report.skipped.push(path.display().to_string());
                continue;
            }
            migrate_single_file(&path, dry_run, in_place, report);
        }
    }
}

fn migrate_single_file(
    legacy_path: &Path,
    dry_run: bool,
    in_place: bool,
    report: &mut MigrationReport,
) {
    let filename = legacy_path
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("");
    let parent = legacy_path.parent().unwrap_or(Path::new(""));

    let canonical_path = if filename == "M31A.toml" || filename == "config.toml" {
        parent.join("config.md")
    } else if filename == "SKILL.toml" {
        parent.join("skill.md")
    } else if filename.ends_with(".toml") {
        parent.join(format!("{}.md", filename.trim_end_matches(".toml")))
    } else {
        report.skipped.push(legacy_path.display().to_string());
        return;
    };

    if canonical_path.exists() {
        report.conflicts.push(format!(
            "Canonical file already exists at '{}'; keeping legacy '{}' unchanged",
            canonical_path.display(),
            legacy_path.display()
        ));
        return;
    }

    let content = match std::fs::read_to_string(legacy_path) {
        Ok(c) => c,
        Err(e) => {
            report.errors.push(MigrationEntry {
                legacy_path: legacy_path.display().to_string(),
                canonical_path: canonical_path.display().to_string(),
                status: "failed".to_string(),
                reason: Some(format!("read error: {e}")),
            });
            return;
        }
    };

    // Determine domain from filename / content
    let converted = if filename == "M31A.toml" || filename == "config.toml" {
        convert_config_toml_to_markdown(&content)
    } else if filename == "SKILL.toml" {
        convert_skill_toml_to_markdown(&content)
    } else if filename == "profile.toml"
        || filename.ends_with(".profile.toml")
        || legacy_path.to_string_lossy().contains("profiles")
        || content.contains("verification_tier")
        || content.contains("capabilities")
    {
        let prof_name = filename
            .trim_end_matches(".profile.toml")
            .trim_end_matches(".toml");
        convert_profile_toml_to_markdown(&content, prof_name)
    } else if content.contains("[adapter]") {
        convert_adapter_toml_to_markdown(&content, filename.trim_end_matches(".toml"))
    } else if content.contains("kind = \"command\"")
        || legacy_path.to_string_lossy().contains("commands")
    {
        convert_command_toml_to_markdown(&content)
    } else if content.contains("[workflow]") {
        convert_workflow_toml_to_markdown(&content)
    } else if content.contains("[policy]") || filename.starts_with("policy") {
        convert_policy_toml_to_markdown(&content)
    } else if content.contains("role =") || content.contains("[prompt]") {
        convert_prompt_toml_to_markdown(&content)
    } else {
        // Fallback generic config conversion
        convert_config_toml_to_markdown(&content)
    };

    match converted {
        Ok(markdown_content) => {
            if !dry_run {
                if let Err(e) = atomic_write_document(&canonical_path, &markdown_content) {
                    report.errors.push(MigrationEntry {
                        legacy_path: legacy_path.display().to_string(),
                        canonical_path: canonical_path.display().to_string(),
                        status: "failed".to_string(),
                        reason: Some(format!("write error: {e}")),
                    });
                    return;
                }
                if in_place && canonical_path != legacy_path && legacy_path.exists() {
                    let _ = std::fs::remove_file(legacy_path);
                }
            }
            report.migrated.push(MigrationEntry {
                legacy_path: legacy_path.display().to_string(),
                canonical_path: canonical_path.display().to_string(),
                status: if dry_run { "planned" } else { "migrated" }.to_string(),
                reason: None,
            });
        }
        Err(e) => {
            report.errors.push(MigrationEntry {
                legacy_path: legacy_path.display().to_string(),
                canonical_path: canonical_path.display().to_string(),
                status: "failed".to_string(),
                reason: Some(e),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migrate_prompt_toml() {
        let toml_str = r#"
id = "agent.test_migrated"
version = 2
kind = "agent"
role = "implementer"
description = "Migration test"
strategy = "standard"

[inputs]
required = [
    { name = "task_id", type = "string", description = "Task identifier" }
]

[body]
template = """
Executing task {{ task_id }}
"""
"#;
        let md = convert_prompt_toml_to_markdown(toml_str).unwrap();
        assert!(md.starts_with("---\nkind: prompt\n"));
        assert!(md.contains("id: agent.test_migrated"));
        assert!(md.contains("Executing task {{ task_id }}"));

        let reparsed = PromptContract::from_markdown_str(&md).unwrap();
        assert_eq!(reparsed.id, "agent.test_migrated");
        assert_eq!(reparsed.version, 2);
    }

    #[test]
    fn test_migrate_config_toml() {
        let toml_str = r#"
profile = "safe"

[runtime]
concurrency_limit = 2
timeout_secs = 120
"#;
        let md = convert_config_toml_to_markdown(toml_str).unwrap();
        assert!(md.starts_with("---\nkind: config\n"));
        assert!(md.contains("profile: safe"));
    }

    #[test]
    fn test_migrate_profile_toml() {
        let toml_str = r#"
name = "custom_profile"
autonomy_mode = "safe"
verification_tier = 2
capabilities = ["workspace_fs_read"]
"#;
        let md = convert_profile_toml_to_markdown(toml_str, "custom_profile").unwrap();
        assert!(md.starts_with("---\nkind: profile\n"));
        assert!(md.contains("name: custom_profile"));
    }

    #[test]
    fn test_migrate_skill_toml() {
        let toml_str = r#"
schema_version = 1
id = "test-skill"
name = "Test Skill"
version = "1.0.0"
description = "A skill test"
required_capabilities = ["workspace_fs_read"]

[execution]
mode = "in_task"

[procedure]
instructions = "Do step 1."

[verification]
tier = 1

[risk_profile]
level = "low"
"#;
        let md = convert_skill_toml_to_markdown(toml_str).unwrap();
        assert!(md.starts_with("---\nkind: skill\n"));
        assert!(md.contains("id: test-skill"));
    }

    #[test]
    fn test_migrate_adapter_toml() {
        let toml_str = r#"
[adapter]
project_type = "rust"
manifest_file = "Cargo.toml"
source_dir = "src"
compiler_command = "cargo check"
test_command = "cargo test"
linter_command = "cargo clippy -- -D warnings"
timeout_secs = 300
"#;
        let md = convert_adapter_toml_to_markdown(toml_str, "rust").unwrap();
        assert!(md.starts_with("---\nkind: adapter\n"));
        assert!(md.contains("project_type: rust"));
    }

    #[test]
    fn test_migrate_path_file() {
        let tmp = tempfile::tempdir().unwrap();
        let toml_path = tmp.path().join("config.toml");
        std::fs::write(&toml_path, "profile = \"safe\"\n").unwrap();

        let dry_report = migrate_path(&toml_path, true, false);
        assert_eq!(dry_report.migrated.len(), 1);
        assert!(!tmp.path().join("config.md").exists());

        let run_report = migrate_path(&toml_path, false, true);
        assert_eq!(run_report.migrated.len(), 1);
        assert!(tmp.path().join("config.md").exists());
        assert!(!toml_path.exists());
    }
}

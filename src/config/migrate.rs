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
//!
//! Migration is governed by an explicit allowlist and domain validation. Third-party manifests
//! (`pyproject.toml`, `Cargo.toml`, etc.) and unknown TOML files are never converted or deleted.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::document::{
    DocumentKind, MarkdownDocument, atomic_write_document, serialize_markdown_document,
};
use crate::prompt::contract::PromptContract;

/// Supported M31A legacy document domains.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyDomain {
    Config,
    Policy,
    Skill,
    Profile,
    Prompt,
    Workflow,
    Adapter,
    Command,
}

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

/// Returns true if a path is located inside an ineligible directory (e.g. tests, fixtures, examples, src, target, .git, etc.).
pub fn is_ineligible_migration_path(path: &Path) -> bool {
    for comp in path.components() {
        let s = comp.as_os_str().to_string_lossy();
        if s == "tests"
            || s == "fixtures"
            || s == "examples"
            || s == "src"
            || s == "target"
            || s == ".git"
            || s == "node_modules"
            || s == "vendor"
            || s == ".cargo"
        {
            return true;
        }
    }
    false
}

/// Detects whether a legacy file belongs to an eligible M31A domain using explicit rules.
///
/// Returns `Some((domain, canonical_destination_path))` if recognized and eligible.
/// Returns `None` if the file is unknown, third-party, or toolchain-owned.
#[allow(clippy::collapsible_if)]
pub fn detect_legacy_m31a_document(path: &Path, content: &str) -> Option<(LegacyDomain, PathBuf)> {
    if is_ineligible_migration_path(path) {
        return None;
    }

    let filename = path.file_name().and_then(|f| f.to_str()).unwrap_or("");
    let parent = path.parent().unwrap_or_else(|| Path::new(""));

    // 1. Toolchain & third-party manifests are unconditionally excluded
    if is_external_or_toolchain_manifest(filename) {
        return None;
    }

    // Example configurations are not active configurations and must not be treated as ordinary workspace config
    if filename == "config.example.toml" {
        return None;
    }

    if path.extension().and_then(|e| e.to_str()) != Some("toml") {
        return None;
    }

    let toml_val: toml::Value = content.parse().ok()?;

    let parent_comps: Vec<String> = parent
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect();
    let in_prompts_dir = parent_comps.iter().any(|c| c == "prompts");
    let in_commands_dir = parent_comps.iter().any(|c| c == "commands");
    let in_workflows_dir = parent_comps.iter().any(|c| c == "workflows");
    let in_profiles_dir = parent_comps.iter().any(|c| c == "profiles");
    let in_adapters_dir = parent_comps
        .iter()
        .any(|c| c == "verification_adapters" || c == "adapters");

    // 2. Configuration: M31A.toml or config.toml
    if filename == "M31A.toml" || filename == "config.toml" {
        if toml_val
            .clone()
            .try_into::<crate::config::schema::AppConfig>()
            .is_ok()
        {
            return Some((LegacyDomain::Config, parent.join("config.md")));
        }
    }

    // 3. Skills: SKILL.toml or skill.toml
    if filename == "SKILL.toml" || filename == "skill.toml" {
        if crate::skill::manifest::SkillManifest::parse_toml(content).is_ok() {
            return Some((LegacyDomain::Skill, parent.join("skill.md")));
        }
    }

    // 4. Policy: policy.toml, organization.toml, or *.policy.toml
    if filename == "policy.toml"
        || filename == "organization.toml"
        || filename.ends_with(".policy.toml")
    {
        if crate::policy::rule::PolicyDocument::from_toml_str(content).is_ok() {
            let target_name = format!("{}.md", filename.trim_end_matches(".toml"));
            return Some((LegacyDomain::Policy, parent.join(target_name)));
        }
    }

    // 5. Verification adapters: adapter.toml or in adapters directory with [adapter] table
    if filename == "adapter.toml" || filename.ends_with(".adapter.toml") || in_adapters_dir {
        if toml_val.get("adapter").is_some() {
            let target_name = format!("{}.md", filename.trim_end_matches(".toml"));
            return Some((LegacyDomain::Adapter, parent.join(target_name)));
        }
    }

    // 6. User commands: command.toml or in commands directory
    if filename == "command.toml" || filename.ends_with(".command.toml") || in_commands_dir {
        if (toml_val.get("kind").and_then(|v| v.as_str()) == Some("command")
            || toml_val.get("template").is_some())
            && crate::interaction::user_commands::parse_user_command_toml(content, None).is_ok()
        {
            let target_name = format!("{}.md", filename.trim_end_matches(".toml"));
            return Some((LegacyDomain::Command, parent.join(target_name)));
        }
    }

    // 7. Workflows: greenfield.toml, *.workflow.toml, or in workflows directory
    if filename == "greenfield.toml" || filename.ends_with(".workflow.toml") || in_workflows_dir {
        if toml_val.get("workflow").is_some()
            && crate::workflow::manifest::WorkflowManifest::from_toml_str(content).is_ok()
        {
            let target_name = format!("{}.md", filename.trim_end_matches(".toml"));
            return Some((LegacyDomain::Workflow, parent.join(target_name)));
        }
    }

    // 8. Profiles: profile.toml, *.profile.toml, or in profiles directory
    if filename == "profile.toml" || filename.ends_with(".profile.toml") || in_profiles_dir {
        if toml_val.get("capabilities").is_some()
            || toml_val.get("runtime_constraints").is_some()
            || toml_val.get("policy_overrides").is_some()
            || toml_val.get("verification_tier").is_some()
        {
            let prof_name = filename
                .trim_end_matches(".profile.toml")
                .trim_end_matches(".toml");
            return Some((
                LegacyDomain::Profile,
                parent.join(format!("{prof_name}.md")),
            ));
        }
    }

    // 9. Prompts: inside prompts directory, or recognized prompt contracts
    if in_prompts_dir
        || filename.ends_with(".prompt.toml")
        || filename.starts_with("discovery.")
        || filename.starts_with("decompose.")
        || filename.starts_with("safety.")
    {
        if (toml_val.get("role").is_some()
            || toml_val.get("inputs").is_some()
            || toml_val.get("body").is_some())
            && PromptContract::from_toml_str(content).is_ok()
        {
            let target_name = format!("{}.md", filename.trim_end_matches(".toml"));
            return Some((LegacyDomain::Prompt, parent.join(target_name)));
        }
    }

    // Unknown or non-allowlisted TOML: do not guess configuration
    None
}

/// Verifies that a filename corresponds to an external or toolchain-owned file.
fn is_external_or_toolchain_manifest(filename: &str) -> bool {
    matches!(
        filename,
        "Cargo.toml"
            | "Cargo.lock"
            | "rust-toolchain.toml"
            | "pyproject.toml"
            | "package.json"
            | "package-lock.json"
            | "tsconfig.json"
            | "poetry.lock"
            | "Pipfile"
            | "Pipfile.lock"
            | "Makefile"
    ) || filename.ends_with(".lock")
}

/// Verifies a written canonical Markdown document against its domain schema.
fn verify_migrated_document(
    canonical_path: &Path,
    domain: LegacyDomain,
    content: &str,
) -> Result<(), String> {
    match domain {
        LegacyDomain::Config => {
            let parsed = crate::config::schema::parse_and_validate_config(content)
                .map_err(|e| format!("Verification failed: config parse error: {e}"))?;
            crate::config::schema::validate_config(&parsed)
                .map_err(|e| format!("Verification failed: config validation error: {e}"))?;
        }
        LegacyDomain::Policy => {
            let doc = crate::policy::rule::PolicyDocument::from_markdown_str(content)
                .map_err(|e| format!("Verification failed: policy parse error: {e}"))?;
            if doc.version != crate::policy::rule::CURRENT_POLICY_SCHEMA_VERSION {
                return Err(format!(
                    "Verification failed: policy schema version mismatch: {}",
                    doc.version
                ));
            }
        }
        LegacyDomain::Skill => {
            crate::skill::manifest::SkillManifest::parse_markdown(content)
                .map_err(|e| format!("Verification failed: skill parse error: {e}"))?;
        }
        LegacyDomain::Profile => {
            crate::config::profile::ProfileResolver::parse_profile_document(content)
                .map_err(|e| format!("Verification failed: profile parse error: {e}"))?;
        }
        LegacyDomain::Prompt => {
            crate::prompt::contract::PromptContract::from_markdown_str(content)
                .map_err(|e| format!("Verification failed: prompt parse error: {e}"))?;
        }
        LegacyDomain::Adapter => {
            let val = crate::document::parse_markdown_value(
                content,
                crate::document::DocumentKind::Adapter,
            )
            .map_err(|e| format!("Verification failed: adapter document error: {e}"))?;
            #[allow(dead_code)]
            #[derive(serde::Deserialize)]
            struct Declared {
                adapter: DeclaredAdapter,
            }
            #[allow(dead_code)]
            #[derive(serde::Deserialize)]
            struct DeclaredAdapter {
                manifest_file: String,
                compiler_command: String,
                test_command: String,
                linter_command: String,
            }
            let _: Declared = serde_yaml::from_value(val.data)
                .map_err(|e| format!("Verification failed: adapter schema error: {e}"))?;
        }
        LegacyDomain::Command => {
            crate::interaction::user_commands::parse_user_command_markdown(
                content,
                Some(canonical_path.display().to_string()),
            )
            .map_err(|e| format!("Verification failed: user command parse error: {e}"))?;
        }
        LegacyDomain::Workflow => {
            crate::workflow::manifest::WorkflowManifest::from_markdown_str(content)
                .map_err(|e| format!("Verification failed: workflow parse error: {e}"))?;
        }
    }
    Ok(())
}

/// Discovers and migrates legacy TOML files in a workspace or specific file path safely.
pub fn migrate_path(target: &Path, dry_run: bool, in_place: bool) -> MigrationReport {
    let mut report = MigrationReport::default();

    if target.is_file() {
        migrate_single_file(target, dry_run, in_place, &mut report);
    } else if target.is_dir() {
        let root_canonical = match std::fs::canonicalize(target) {
            Ok(c) => c,
            Err(e) => {
                report.errors.push(MigrationEntry {
                    legacy_path: target.display().to_string(),
                    canonical_path: "".to_string(),
                    status: "failed".to_string(),
                    reason: Some(format!(
                        "Failed to canonicalize directory {}: {e}",
                        target.display()
                    )),
                });
                return report;
            }
        };
        collect_and_migrate_dir(target, &root_canonical, dry_run, in_place, &mut report, 0);
    } else {
        report.errors.push(MigrationEntry {
            legacy_path: target.display().to_string(),
            canonical_path: "".to_string(),
            status: "failed".to_string(),
            reason: Some("Target path does not exist".to_string()),
        });
    }

    report
}

fn collect_and_migrate_dir(
    dir: &Path,
    root_canonical: &Path,
    dry_run: bool,
    in_place: bool,
    report: &mut MigrationReport,
    depth: usize,
) {
    if depth >= 10 {
        report.skipped.push(format!(
            "{}: maximum directory traversal depth (10) reached",
            dir.display()
        ));
        return;
    }
    let read_dir = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            report.errors.push(MigrationEntry {
                legacy_path: dir.display().to_string(),
                canonical_path: "".to_string(),
                status: "failed".to_string(),
                reason: Some(format!("directory read error: {e}")),
            });
            return;
        }
    };

    for entry_res in read_dir {
        let entry = match entry_res {
            Ok(entry) => entry,
            Err(e) => {
                report.errors.push(MigrationEntry {
                    legacy_path: dir.display().to_string(),
                    canonical_path: "".to_string(),
                    status: "failed".to_string(),
                    reason: Some(format!("directory entry read error: {e}")),
                });
                continue;
            }
        };

        let path = entry.path();
        let filename = path.file_name().unwrap_or_default().to_string_lossy();
        if filename.starts_with('.') && filename != ".m31a" && filename != ".m31" {
            continue;
        }

        // Check if directory name is ineligible
        if filename == "target"
            || filename == ".git"
            || filename == "node_modules"
            || filename == "vendor"
            || filename == "tests"
            || filename == "fixtures"
            || filename == "examples"
            || filename == "src"
            || filename == ".cargo"
        {
            report
                .skipped
                .push(format!("{}: ineligible directory", path.display()));
            continue;
        }

        // Symlink checks: must not escape root_canonical and must not be followed if it's a directory
        let is_symlink = entry.file_type().map(|ft| ft.is_symlink()).unwrap_or(false);
        if is_symlink {
            let resolved = match std::fs::canonicalize(&path) {
                Ok(r) => r,
                Err(e) => {
                    report.errors.push(MigrationEntry {
                        legacy_path: path.display().to_string(),
                        canonical_path: "".to_string(),
                        status: "failed".to_string(),
                        reason: Some(format!("symlink resolution error: {e}")),
                    });
                    continue;
                }
            };
            if !resolved.starts_with(root_canonical) {
                report.skipped.push(format!(
                    "{}: symlink resolves outside migration root ({})",
                    path.display(),
                    resolved.display()
                ));
                continue;
            }
            if resolved.components().any(|c| c.as_os_str() == ".git") {
                report
                    .skipped
                    .push(format!("{}: symlink points to .git", path.display()));
                continue;
            }
            if resolved.is_dir() {
                report.skipped.push(format!(
                    "{}: skipping directory symlink to prevent recursion/cycles",
                    path.display()
                ));
                continue;
            }
        }

        if path.is_dir() {
            collect_and_migrate_dir(&path, root_canonical, dry_run, in_place, report, depth + 1);
        } else if path.is_file() && path.extension().is_some_and(|e| e == "toml") {
            migrate_single_file(&path, dry_run, in_place, report);
        }
    }
}

pub fn migrate_single_file(
    legacy_path: &Path,
    dry_run: bool,
    in_place: bool,
    report: &mut MigrationReport,
) {
    if is_ineligible_migration_path(legacy_path) {
        report.skipped.push(format!(
            "{}: ineligible path for migration (tests/fixtures/examples/src/target/etc.)",
            legacy_path.display()
        ));
        return;
    }

    let filename = legacy_path
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("");

    // Check third party / toolchain exclusions first
    if is_external_or_toolchain_manifest(filename) {
        report.skipped.push(format!(
            "{}: external or toolchain manifest",
            legacy_path.display()
        ));
        return;
    }

    if filename == "config.example.toml" {
        report.skipped.push(format!(
            "{}: example template is not an active configuration; example migration requires explicit context",
            legacy_path.display()
        ));
        return;
    }

    if legacy_path.extension().and_then(|e| e.to_str()) != Some("toml") {
        report
            .skipped
            .push(format!("{}: not a TOML document", legacy_path.display()));
        return;
    }

    let content = match std::fs::read_to_string(legacy_path) {
        Ok(c) => c,
        Err(e) => {
            report.errors.push(MigrationEntry {
                legacy_path: legacy_path.display().to_string(),
                canonical_path: "".to_string(),
                status: "failed".to_string(),
                reason: Some(format!("read error: {e}")),
            });
            return;
        }
    };

    let Some((domain, canonical_path)) = detect_legacy_m31a_document(legacy_path, &content) else {
        report.skipped.push(format!(
            "{}: not a recognized M31A legacy document",
            legacy_path.display()
        ));
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

    let converted = match domain {
        LegacyDomain::Config => convert_config_toml_to_markdown(&content),
        LegacyDomain::Skill => convert_skill_toml_to_markdown(&content),
        LegacyDomain::Policy => convert_policy_toml_to_markdown(&content),
        LegacyDomain::Adapter => {
            let prof_name = filename.trim_end_matches(".toml");
            convert_adapter_toml_to_markdown(&content, prof_name)
        }
        LegacyDomain::Command => convert_command_toml_to_markdown(&content),
        LegacyDomain::Workflow => convert_workflow_toml_to_markdown(&content),
        LegacyDomain::Profile => {
            let prof_name = filename
                .trim_end_matches(".profile.toml")
                .trim_end_matches(".toml");
            convert_profile_toml_to_markdown(&content, prof_name)
        }
        LegacyDomain::Prompt => convert_prompt_toml_to_markdown(&content),
    };

    let markdown_content = match converted {
        Ok(c) => c,
        Err(e) => {
            report.errors.push(MigrationEntry {
                legacy_path: legacy_path.display().to_string(),
                canonical_path: canonical_path.display().to_string(),
                status: "failed".to_string(),
                reason: Some(e),
            });
            return;
        }
    };

    // Full in-memory semantic validation before proceeding (applies to both dry-run and live migration)
    if let Err(e) = verify_migrated_document(&canonical_path, domain, &markdown_content) {
        report.errors.push(MigrationEntry {
            legacy_path: legacy_path.display().to_string(),
            canonical_path: canonical_path.display().to_string(),
            status: "failed".to_string(),
            reason: Some(format!("Semantic verification failed: {e}")),
        });
        return;
    }

    if dry_run {
        report.migrated.push(MigrationEntry {
            legacy_path: legacy_path.display().to_string(),
            canonical_path: canonical_path.display().to_string(),
            status: "planned".to_string(),
            reason: None,
        });
        return;
    }

    // Live migration: write candidate to a sibling temporary file first
    let parent_dir = canonical_path.parent().unwrap_or_else(|| Path::new("."));
    let canon_filename = canonical_path
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("migrated.md");
    let tmp_candidate = parent_dir.join(format!(".{canon_filename}.tmp.{}", std::process::id()));

    if let Err(e) = atomic_write_document(&tmp_candidate, &markdown_content) {
        report.errors.push(MigrationEntry {
            legacy_path: legacy_path.display().to_string(),
            canonical_path: canonical_path.display().to_string(),
            status: "failed".to_string(),
            reason: Some(format!("candidate write error: {e}")),
        });
        return;
    }

    // Reopen and strictly verify the candidate on disk
    let written_str = match std::fs::read_to_string(&tmp_candidate) {
        Ok(s) => s,
        Err(e) => {
            let cleanup_res = std::fs::remove_file(&tmp_candidate);
            let cleanup_msg = cleanup_res
                .err()
                .map(|ce| format!("; temporary file cleanup failed: {ce}"))
                .unwrap_or_default();
            report.errors.push(MigrationEntry {
                legacy_path: legacy_path.display().to_string(),
                canonical_path: canonical_path.display().to_string(),
                status: "failed".to_string(),
                reason: Some(format!(
                    "failed to re-read candidate document: {e}{cleanup_msg}"
                )),
            });
            return;
        }
    };

    if let Err(e) = verify_migrated_document(&tmp_candidate, domain, &written_str) {
        let cleanup_res = std::fs::remove_file(&tmp_candidate);
        let cleanup_msg = cleanup_res
            .err()
            .map(|ce| format!("; temporary file cleanup failed: {ce}"))
            .unwrap_or_default();
        report.errors.push(MigrationEntry {
            legacy_path: legacy_path.display().to_string(),
            canonical_path: canonical_path.display().to_string(),
            status: "failed".to_string(),
            reason: Some(format!("candidate verification error: {e}{cleanup_msg}")),
        });
        return;
    }

    // Promote verified candidate to destination canonical path
    if let Err(e) = std::fs::rename(&tmp_candidate, &canonical_path) {
        let cleanup_res = std::fs::remove_file(&tmp_candidate);
        let cleanup_msg = cleanup_res
            .err()
            .map(|ce| format!("; temporary file cleanup failed: {ce}"))
            .unwrap_or_default();
        report.errors.push(MigrationEntry {
            legacy_path: legacy_path.display().to_string(),
            canonical_path: canonical_path.display().to_string(),
            status: "failed".to_string(),
            reason: Some(format!(
                "failed to promote candidate to canonical destination: {e}{cleanup_msg}"
            )),
        });
        return;
    }

    // In-place deletion: only delete legacy file after canonical file is successfully written, verified, and placed
    if in_place && canonical_path != *legacy_path && legacy_path.exists() {
        if let Err(e) = std::fs::remove_file(legacy_path) {
            report.errors.push(MigrationEntry {
                legacy_path: legacy_path.display().to_string(),
                canonical_path: canonical_path.display().to_string(),
                status: "failed".to_string(),
                reason: Some(format!("deletion error: {e}")),
            });
            return;
        }
    }

    report.migrated.push(MigrationEntry {
        legacy_path: legacy_path.display().to_string(),
        canonical_path: canonical_path.display().to_string(),
        status: "migrated".to_string(),
        reason: None,
    });
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

    #[test]
    fn test_unrelated_manifests_are_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let pyproject = tmp.path().join("pyproject.toml");
        std::fs::write(&pyproject, "[tool.poetry]\nname = \"demo\"\n").unwrap();

        let cargo_toml = tmp.path().join("Cargo.toml");
        std::fs::write(&cargo_toml, "[package]\nname = \"test\"\n").unwrap();

        let unknown_toml = tmp.path().join("arbitrary.toml");
        std::fs::write(&unknown_toml, "[custom_section]\nkey = \"value\"\n").unwrap();

        let report = migrate_path(tmp.path(), false, true);
        assert_eq!(report.migrated.len(), 0);
        assert_eq!(report.errors.len(), 0);
        assert!(pyproject.exists());
        assert!(cargo_toml.exists());
        assert!(unknown_toml.exists());
        assert_eq!(report.skipped.len(), 3);
    }

    #[test]
    fn test_conflict_preserves_both_files() {
        let tmp = tempfile::tempdir().unwrap();
        let toml_path = tmp.path().join("config.toml");
        let md_path = tmp.path().join("config.md");
        std::fs::write(&toml_path, "profile = \"safe\"\n").unwrap();
        std::fs::write(
            &md_path,
            "---\nkind: config\nschema_version: 1\ndata:\n  profile: balanced\n---\n# Existing\n",
        )
        .unwrap();

        let report = migrate_path(&toml_path, false, true);
        assert_eq!(report.migrated.len(), 0);
        assert_eq!(report.conflicts.len(), 1);
        assert!(toml_path.exists());
        assert!(md_path.exists());
    }

    #[test]
    fn test_malformed_legacy_toml_reports_error_and_does_not_delete() {
        let tmp = tempfile::tempdir().unwrap();
        let toml_path = tmp.path().join("config.toml");
        std::fs::write(&toml_path, "invalid [[ toml syntax").unwrap();

        let report = migrate_path(&toml_path, false, true);
        assert_eq!(report.migrated.len(), 0);
        // It's skipped because it's not a valid M31A document
        assert!(toml_path.exists());
    }
}

//! Multi-Tier Skill Discovery (SKL-01, D-09).
//!
//! Scans across Builtin -> System -> User -> Workspace precedence tiers,
//! parses SKILL.toml files, and calculates cryptographic SHA-256 content hashes.

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

use crate::skill::manifest::SkillManifest;

/// Precedence tier for skill definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SkillOriginTier {
    /// Embedded default skills packaged directly in the binary.
    Builtin = 0,
    /// System-wide administrator skills (/etc/m31a/skills).
    System = 1,
    /// User-specific global skills (~/.config/m31a/skills).
    User = 2,
    /// Workspace-specific project skills (<workspace>/.m31a/skills).
    Workspace = 3,
}

impl std::fmt::Display for SkillOriginTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SkillOriginTier::Builtin => write!(f, "builtin"),
            SkillOriginTier::System => write!(f, "system"),
            SkillOriginTier::User => write!(f, "user"),
            SkillOriginTier::Workspace => write!(f, "workspace"),
        }
    }
}

/// Discovered and validated skill package ready for registry insertion.
#[derive(Debug, Clone, PartialEq)]
pub struct SkillPackage {
    pub manifest: SkillManifest,
    pub origin: SkillOriginTier,
    pub source_path: Option<PathBuf>,
    pub content_hash: String,
}

/// Computes SHA-256 hex digest for content integrity.
pub fn compute_sha256(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Embedded default builtin skill manifests.
pub fn builtin_skills() -> Vec<SkillPackage> {
    let raw_skills = vec![
        (
            "fix-test-failure",
            r#"
schema_version = 1
id = "fix-test-failure"
name = "Fix Test Failure"
version = "1.0.0"
description = "Diagnoses failed tests, patches code, and verifies regression fixes"
required_capabilities = ["workspace_fs_write", "compiler_exec"]

[execution]
mode = "in_task"

[procedure]
instructions = "1. Inspect compiler/test stderr. 2. Locate failing test assertion. 3. Apply fix. 4. Re-run test."
steps = [
  { name = "reproduce", instruction = "cargo test", allowed_tools = ["test_runner"] },
  { name = "patch", instruction = "Apply code patch", allowed_tools = ["file_editor"] },
  { name = "verify", instruction = "Verify pass", allowed_tools = ["test_runner"] }
]

[verification]
tier = 3
commands = ["cargo test"]
evidence_required = ["test_summary.log"]

[risk_profile]
level = "medium"
requires_approval = false
"#,
        ),
        (
            "refactor-module",
            r#"
schema_version = 1
id = "refactor-module"
name = "Refactor Module"
version = "1.0.0"
description = "Safely restructures code without altering external behavior"
required_capabilities = ["workspace_fs_write", "compiler_exec"]

[execution]
mode = "sub_dag"

[procedure]
instructions = "1. Establish baseline test pass. 2. Restructure files. 3. Fix compiler errors. 4. Verify tests pass."
steps = [
  { name = "baseline", instruction = "Verify green baseline", allowed_tools = ["test_runner"] },
  { name = "edit", instruction = "Refactor code structure", allowed_tools = ["file_editor"] },
  { name = "compile", instruction = "Verify compile", allowed_tools = ["compiler"] },
  { name = "regression", instruction = "Run full test suite", allowed_tools = ["test_runner"] }
]

[verification]
tier = 4
commands = ["cargo clippy --all-targets -- -D warnings", "cargo test"]
evidence_required = ["clippy_report.log"]

[risk_profile]
level = "medium"
requires_approval = false
"#,
        ),
        (
            "prepare-release",
            r#"
schema_version = 1
id = "prepare-release"
name = "Prepare Release"
version = "1.0.0"
description = "Prepares release artifacts, updates changelog, and validates commit tags"
required_capabilities = ["workspace_fs_write", "git_ops"]

[execution]
mode = "sub_dag"

[procedure]
instructions = "1. Validate clean git working tree. 2. Bump versions. 3. Build release. 4. Tag release."
steps = [
  { name = "clean_check", instruction = "Ensure git status is clean", allowed_tools = ["git_status"] },
  { name = "verify_all", instruction = "Run all verification gates", allowed_tools = ["verifier"] },
  { name = "package", instruction = "Build release artifacts", allowed_tools = ["builder"] }
]

[verification]
tier = 5
commands = ["cargo check --release", "cargo test --all-targets"]
evidence_required = ["release_metadata.json"]

[risk_profile]
level = "high"
requires_approval = true
"#,
        ),
    ];

    raw_skills
        .into_iter()
        .filter_map(|(_id, toml_str)| {
            let manifest = SkillManifest::parse_toml(toml_str).ok()?;
            let hash = compute_sha256(toml_str);
            Some(SkillPackage {
                manifest,
                origin: SkillOriginTier::Builtin,
                source_path: None,
                content_hash: hash,
            })
        })
        .collect()
}

/// Scanner discovering skills from filesystem directories.
pub struct SkillDiscovery;

impl SkillDiscovery {
    /// Discover all skills from a specific directory tier.
    pub fn discover_directory(base_dir: &Path, tier: SkillOriginTier) -> Vec<SkillPackage> {
        let mut packages = Vec::new();
        if !base_dir.exists() || !base_dir.is_dir() {
            return packages;
        }

        let entries = match std::fs::read_dir(base_dir) {
            Ok(e) => e,
            Err(_) => return packages,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let manifest_path = path.join("SKILL.toml");
                if manifest_path.is_file()
                    && let Ok(content) = std::fs::read_to_string(&manifest_path)
                    && let Ok(manifest) = SkillManifest::parse_toml(&content)
                {
                    let content_hash = compute_sha256(&content);
                    packages.push(SkillPackage {
                        manifest,
                        origin: tier,
                        source_path: Some(manifest_path),
                        content_hash,
                    });
                }
            }
        }

        packages
    }

    /// Run discovery across all 4 tiers in ascending precedence order.
    pub fn discover_all(workspace_root: Option<&Path>) -> Vec<SkillPackage> {
        let mut all = Vec::new();

        // 1. Built-in tier (lowest precedence baseline)
        all.extend(builtin_skills());

        let platform_paths = crate::config::PlatformPaths::new();

        // 2. System tier
        let system_dir = platform_paths.system_config_dir().join("skills");
        all.extend(Self::discover_directory(
            &system_dir,
            SkillOriginTier::System,
        ));

        // 3. User tier
        let user_skills = platform_paths.config_dir().join("skills");
        all.extend(Self::discover_directory(
            &user_skills,
            SkillOriginTier::User,
        ));

        // 4. Workspace tier (highest precedence)
        if let Some(ws) = workspace_root {
            let ws_skills = ws.join(".m31a").join("skills");
            all.extend(Self::discover_directory(
                &ws_skills,
                SkillOriginTier::Workspace,
            ));
        }

        all
    }
}

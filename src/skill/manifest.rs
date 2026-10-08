//! Declarative SKILL.toml Manifest Schema (SKL-01, D-09).
//!
//! Enforces strict schema validation, capability declaration, execution mode,
//! procedural steps, verification commands, and risk profile.

use std::fmt;
use std::str::FromStr;

use semver::Version;
use serde::{Deserialize, Serialize};

/// Maximum supported SKILL.toml schema version.
pub const CURRENT_SKILL_SCHEMA_VERSION: u32 = 1;

/// Execution paradigm for the skill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillExecutionMode {
    /// Compiles into an autonomous sub-DAG validated by scheduler.
    SubDag,
    /// Compiles into structured prompt guidance within a single task.
    InTask,
}

impl fmt::Display for SkillExecutionMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SkillExecutionMode::SubDag => write!(f, "sub_dag"),
            SkillExecutionMode::InTask => write!(f, "in_task"),
        }
    }
}

impl FromStr for SkillExecutionMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "sub_dag" | "subdag" => Ok(SkillExecutionMode::SubDag),
            "in_task" | "intask" => Ok(SkillExecutionMode::InTask),
            other => Err(format!("Unknown skill execution mode: {}", other)),
        }
    }
}

/// Execution configuration container.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillExecutionConfig {
    pub mode: SkillExecutionMode,
}

/// A discrete step definition within a skill procedure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillStepDefinition {
    pub name: String,
    #[serde(default)]
    pub agent_role: Option<String>,
    pub instruction: String,
    #[serde(default)]
    pub allowed_tools: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<String>,
}

/// Procedural implementation specification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillProcedure {
    pub instructions: String,
    #[serde(default)]
    pub steps: Vec<SkillStepDefinition>,
}

/// Verification requirements and commands for skill completion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillVerificationSpec {
    /// Minimum verification tier required (1 to 7).
    pub tier: u8,
    #[serde(default)]
    pub commands: Vec<String>,
    #[serde(default)]
    pub evidence_required: Vec<String>,
}

/// Risk level classification for the skill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillRiskLevel {
    Low,
    Medium,
    High,
}

impl fmt::Display for SkillRiskLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SkillRiskLevel::Low => write!(f, "low"),
            SkillRiskLevel::Medium => write!(f, "medium"),
            SkillRiskLevel::High => write!(f, "high"),
        }
    }
}

/// Risk governance and approval requirements.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillRiskProfile {
    pub level: SkillRiskLevel,
    #[serde(default)]
    pub requires_approval: bool,
}

/// Authoritative declarative manifest schema for SKILL.toml.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillManifest {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub version: Version,
    pub description: String,
    #[serde(default)]
    pub required_capabilities: Vec<String>,
    #[serde(default = "default_input_schema")]
    pub input_schema: serde_json::Value,
    pub execution: SkillExecutionConfig,
    pub procedure: SkillProcedure,
    pub verification: SkillVerificationSpec,
    pub risk_profile: SkillRiskProfile,
}

fn default_input_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {}
    })
}

impl SkillManifest {
    /// Parse and strictly validate a SKILL.toml string.
    pub fn parse_toml(content: &str) -> Result<Self, String> {
        let manifest: Self =
            toml::from_str(content).map_err(|e| format!("Failed to parse SKILL.toml: {}", e))?;

        if manifest.schema_version == 0 || manifest.schema_version > CURRENT_SKILL_SCHEMA_VERSION {
            return Err(format!(
                "Unsupported skill schema_version: {}. Maximum supported is {}",
                manifest.schema_version, CURRENT_SKILL_SCHEMA_VERSION
            ));
        }

        if manifest.id.trim().is_empty() {
            return Err("Skill id cannot be empty".to_string());
        }

        if manifest.verification.tier < 1 || manifest.verification.tier > 7 {
            return Err(format!(
                "Invalid verification tier {}: must be between 1 and 7",
                manifest.verification.tier
            ));
        }

        Ok(manifest)
    }
}

// ---------------------------------------------------------------------------
// Hardened Plugin Manifest & Sandboxing (Issue 17)
// ---------------------------------------------------------------------------

use sha2::{Digest, Sha256};

/// Hardened manifest schema for external plugins and skills.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginManifest {
    pub name: String,
    pub version: Version,
    pub description: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub required_capabilities: Vec<String>,
    #[serde(default)]
    pub allowed_paths: Vec<String>,
    #[serde(default)]
    pub network_domains: Vec<String>,
    #[serde(default = "default_plugin_timeout")]
    pub timeout_secs: u64,
    /// Mandatory SHA256 integrity checksum hex string.
    pub checksum: String,
}

fn default_plugin_timeout() -> u64 {
    30
}

/// Errors raised during plugin manifest validation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PluginValidationError {
    #[error("Plugin name cannot be empty")]
    EmptyName,
    #[error("Invalid checksum format: '{0}'. Must be a 64-character lowercase hex string")]
    InvalidChecksumFormat(String),
    #[error("Checksum mismatch: expected '{expected}', computed '{actual}'")]
    ChecksumMismatch { expected: String, actual: String },
    #[error(
        "Illegal path '{0}' in allowed_paths: directory traversal ('..') is strictly prohibited"
    )]
    IllegalPathTraversal(String),
    #[error("Disallowed network domain '{0}': loopback and private networks are forbidden")]
    DisallowedNetworkDomain(String),
    #[error("Timeout {0}s exceeds maximum allowed ceiling of 600s")]
    TimeoutCeilingExceeded(u64),
}

impl PluginManifest {
    /// Parse and validate a plugin manifest TOML string.
    pub fn parse_toml(content: &str) -> Result<Self, String> {
        toml::from_str(content).map_err(|e| format!("Failed to parse plugin manifest TOML: {e}"))
    }

    /// Validate security invariants: checksum integrity, sandbox paths, network domains, and resource limits.
    pub fn validate_sandbox(&self, raw_bytes: Option<&[u8]>) -> Result<(), PluginValidationError> {
        if self.name.trim().is_empty() {
            return Err(PluginValidationError::EmptyName);
        }

        let trimmed_checksum = self.checksum.trim().to_lowercase();
        if trimmed_checksum.len() != 64 || !trimmed_checksum.chars().all(|c| c.is_ascii_hexdigit())
        {
            return Err(PluginValidationError::InvalidChecksumFormat(
                self.checksum.clone(),
            ));
        }

        if let Some(bytes) = raw_bytes {
            let mut hasher = Sha256::new();
            hasher.update(bytes);
            let computed = format!("{:x}", hasher.finalize());
            if computed != trimmed_checksum {
                return Err(PluginValidationError::ChecksumMismatch {
                    expected: trimmed_checksum,
                    actual: computed,
                });
            }
        }

        for path in &self.allowed_paths {
            if path.contains("..") || path.starts_with('/') || path.starts_with('\\') {
                return Err(PluginValidationError::IllegalPathTraversal(path.clone()));
            }
        }

        for domain in &self.network_domains {
            let d_lower = domain.to_lowercase();
            if d_lower == "localhost"
                || d_lower.starts_with("127.")
                || d_lower.starts_with("10.")
                || d_lower.starts_with("192.168.")
                || d_lower.starts_with("172.16.")
                || d_lower.starts_with("169.254.")
                || d_lower == "0.0.0.0"
                || d_lower == "::1"
            {
                return Err(PluginValidationError::DisallowedNetworkDomain(
                    domain.clone(),
                ));
            }
        }

        if self.timeout_secs > 600 {
            return Err(PluginValidationError::TimeoutCeilingExceeded(
                self.timeout_secs,
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_skill_manifest() {
        let toml_str = r#"
schema_version = 1
id = "fix-test-failure"
name = "Fix Test Failure"
version = "1.0.0"
description = "Diagnoses and resolves automated test regressions"
required_capabilities = ["workspace_fs_write", "compiler_exec"]

[execution]
mode = "in_task"

[procedure]
instructions = "Inspect test log, locate failure, modify code, and verify."
steps = [
  { name = "reproduce", instruction = "cargo test", allowed_tools = ["test_runner"] }
]

[verification]
tier = 3
commands = ["cargo test"]
evidence_required = ["test_output.log"]

[risk_profile]
level = "medium"
requires_approval = false
"#;

        let manifest = SkillManifest::parse_toml(toml_str).unwrap();
        assert_eq!(manifest.id, "fix-test-failure");
        assert_eq!(manifest.version.to_string(), "1.0.0");
        assert_eq!(manifest.execution.mode, SkillExecutionMode::InTask);
        assert_eq!(manifest.verification.tier, 3);
        assert_eq!(manifest.risk_profile.level, SkillRiskLevel::Medium);
    }

    #[test]
    fn test_reject_unknown_fields() {
        let toml_str = r#"
schema_version = 1
id = "invalid-skill"
name = "Invalid"
version = "1.0.0"
description = "Contains unknown field"
unrecognized_field = "malicious_payload"

[execution]
mode = "sub_dag"

[procedure]
instructions = "Do work"

[verification]
tier = 1

[risk_profile]
level = "low"
"#;

        assert!(SkillManifest::parse_toml(toml_str).is_err());
    }

    #[test]
    fn test_reject_unsupported_schema_version() {
        let toml_str = r#"
schema_version = 99
id = "future-skill"
name = "Future"
version = "2.0.0"
description = "Future version"

[execution]
mode = "in_task"

[procedure]
instructions = "Do work"

[verification]
tier = 1

[risk_profile]
level = "low"
"#;

        assert!(SkillManifest::parse_toml(toml_str).is_err());
    }
}

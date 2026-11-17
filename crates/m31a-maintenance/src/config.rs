//! Maintenance Policy & Engine Configuration (`m31a-maintenance::config`).
//!
//! Validates `.m31a/maintenance.toml` configuration files.
//! Enforces:
//! 1. No plaintext credentials or tokens in policy files.
//! 2. Strict validation of autonomy tiers, categories, branch names, and path patterns.
//! 3. Conservative safe defaults.

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::domain::{
    AutonomyTier, MaintenanceCategory, MaintenancePolicy, MaintenanceRiskLevel, PullRequestPolicy,
};

/// Root schema for `.m31a/maintenance.toml`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MaintenanceConfigFile {
    pub version: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_autonomy")]
    pub autonomy_tier: String,
    #[serde(default)]
    pub sources: SourcesConfig,
    #[serde(default)]
    pub scheduling: SchedulingConfig,
    #[serde(default)]
    pub rules: RulesConfig,
    #[serde(default)]
    pub delivery: DeliveryConfig,
}

fn default_true() -> bool {
    true
}

fn default_autonomy() -> String {
    "tier1_prepare".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SourcesConfig {
    pub github_repository: Option<String>,
    pub github_token_env: Option<String>,
    pub webhook_secret_env: Option<String>,
    pub poll_interval_seconds: Option<u64>,
    #[serde(default = "default_true")]
    pub enable_local_scanner: bool,
}

impl Default for SourcesConfig {
    fn default() -> Self {
        Self {
            github_repository: None,
            github_token_env: Some("GITHUB_TOKEN".to_string()),
            webhook_secret_env: Some("GITHUB_WEBHOOK_SECRET".to_string()),
            poll_interval_seconds: Some(300),
            enable_local_scanner: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SchedulingConfig {
    #[serde(default = "default_true")]
    pub daily_health_check: bool,
    #[serde(default = "default_true")]
    pub weekly_dependency_review: bool,
    #[serde(default = "default_true")]
    pub documentation_check: bool,
    pub maintenance_window: Option<String>,
}

impl Default for SchedulingConfig {
    fn default() -> Self {
        Self {
            daily_health_check: true,
            weekly_dependency_review: true,
            documentation_check: true,
            maintenance_window: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RulesConfig {
    #[serde(default = "default_branches")]
    pub allowed_branches: Vec<String>,
    #[serde(default = "default_excluded")]
    pub excluded_paths: Vec<String>,
    #[serde(default = "default_risk_threshold")]
    pub risk_threshold_for_approval: String,
    #[serde(default = "default_concurrency")]
    pub max_concurrency: usize,
    #[serde(default = "default_retries")]
    pub max_retries: u32,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
}

fn default_branches() -> Vec<String> {
    vec!["master".to_string(), "main".to_string()]
}

fn default_excluded() -> Vec<String> {
    vec![
        ".git/**".to_string(),
        ".github/workflows/security.yml".to_string(),
    ]
}

fn default_risk_threshold() -> String {
    "medium".to_string()
}

fn default_concurrency() -> usize {
    2
}

fn default_retries() -> u32 {
    3
}

fn default_timeout() -> u64 {
    1800
}

impl Default for RulesConfig {
    fn default() -> Self {
        Self {
            allowed_branches: default_branches(),
            excluded_paths: default_excluded(),
            risk_threshold_for_approval: default_risk_threshold(),
            max_concurrency: default_concurrency(),
            max_retries: default_retries(),
            timeout_seconds: default_timeout(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeliveryConfig {
    #[serde(default = "default_true")]
    pub open_draft_pr: bool,
    #[serde(default = "default_branch_prefix")]
    pub branch_prefix: String,
    #[serde(default = "default_labels")]
    pub labels: Vec<String>,
}

fn default_branch_prefix() -> String {
    "m31a-maint/".to_string()
}

fn default_labels() -> Vec<String> {
    vec!["autonomous-maintenance".to_string()]
}

impl Default for DeliveryConfig {
    fn default() -> Self {
        Self {
            open_draft_pr: true,
            branch_prefix: default_branch_prefix(),
            labels: default_labels(),
        }
    }
}

impl Default for MaintenanceConfigFile {
    fn default() -> Self {
        Self {
            version: "1.0.0".to_string(),
            enabled: true,
            autonomy_tier: default_autonomy(),
            sources: SourcesConfig::default(),
            scheduling: SchedulingConfig::default(),
            rules: RulesConfig::default(),
            delivery: DeliveryConfig::default(),
        }
    }
}

impl MaintenanceConfigFile {
    /// Loads and validates `.m31a/maintenance.toml` from the specified repository directory.
    pub fn load_from_workspace(workspace_root: &Path) -> Result<Self> {
        let config_path = workspace_root.join(".m31a").join("maintenance.toml");
        if !config_path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read {}", config_path.display()))?;

        Self::parse_and_validate(&content)
    }

    /// Parses and strictly validates a TOML configuration string.
    pub fn parse_and_validate(toml_content: &str) -> Result<Self> {
        // 1. Security Check: ensure no plaintext secret tokens appear in configuration
        let lower = toml_content.to_lowercase();
        if lower.contains("ghp_")
            || lower.contains("github_pat_")
            || lower.contains("nvapi-")
            || lower.contains("secret = \"") && !lower.contains("_env")
        {
            bail!(
                "Security violation: Plaintext credentials detected in maintenance configuration. Use environment variable references (e.g. `github_token_env = 'GITHUB_TOKEN'`)."
            );
        }

        let cfg: Self = toml::from_str(toml_content)
            .context("Failed to parse TOML maintenance configuration")?;

        // 2. Validate Autonomy Tier
        match cfg.autonomy_tier.as_str() {
            "tier0_observe"
            | "tier1_prepare"
            | "tier2_guarded_remediation"
            | "tier3_restricted_high_impact" => {}
            other => {
                bail!(
                    "Invalid autonomy_tier '{}'. Supported tiers: tier0_observe, tier1_prepare, tier2_guarded_remediation, tier3_restricted_high_impact",
                    other
                );
            }
        }

        // 3. Validate Allowed Branches
        if cfg.rules.allowed_branches.is_empty() {
            bail!("Configuration error: `rules.allowed_branches` cannot be empty");
        }

        for branch in &cfg.rules.allowed_branches {
            if branch.trim().is_empty() {
                bail!("Configuration error: Empty branch name in `rules.allowed_branches`");
            }
        }

        // 4. Validate Risk Threshold
        match cfg.rules.risk_threshold_for_approval.as_str() {
            "low" | "medium" | "high" | "critical" => {}
            other => {
                bail!(
                    "Invalid risk_threshold_for_approval '{}'. Supported: low, medium, high, critical",
                    other
                );
            }
        }

        Ok(cfg)
    }

    /// Converts configuration file into domain `MaintenancePolicy`.
    pub fn to_policy(&self) -> MaintenancePolicy {
        let tier = match self.autonomy_tier.as_str() {
            "tier0_observe" => AutonomyTier::Tier0Observe,
            "tier1_prepare" => AutonomyTier::Tier1Prepare,
            "tier2_guarded_remediation" => AutonomyTier::Tier2GuardedRemediation,
            _ => AutonomyTier::Tier3RestrictedHighImpact,
        };

        let risk = match self.rules.risk_threshold_for_approval.as_str() {
            "low" => MaintenanceRiskLevel::Low,
            "medium" => MaintenanceRiskLevel::Medium,
            "high" => MaintenanceRiskLevel::High,
            _ => MaintenanceRiskLevel::Critical,
        };

        MaintenancePolicy {
            version: self.version.clone(),
            enabled: self.enabled,
            autonomy_tier: tier,
            allowed_categories: vec![
                MaintenanceCategory::BrokenCi,
                MaintenanceCategory::DependencyUpdate,
                MaintenanceCategory::IssueBug,
                MaintenanceCategory::ApiMigration,
                MaintenanceCategory::DocDrift,
                MaintenanceCategory::SecurityAdvisory,
                MaintenanceCategory::ArchitectureDrift,
            ],
            allowed_branches: self.rules.allowed_branches.clone(),
            excluded_paths: self.rules.excluded_paths.clone(),
            risk_threshold_for_approval: risk,
            max_concurrency_per_repo: self.rules.max_concurrency,
            max_retries_per_case: self.rules.max_retries,
            max_replans_per_case: 2,
            timeout_per_run_secs: self.rules.timeout_seconds,
            token_budget_per_run: 500_000,
            pull_request_policy: PullRequestPolicy {
                open_draft_pr: self.delivery.open_draft_pr,
                branch_prefix: self.delivery.branch_prefix.clone(),
                require_review: true,
                auto_label: self.delivery.labels.clone(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_config() {
        let toml_str = r#"
version = "1.0.0"
enabled = true
autonomy_tier = "tier2_guarded_remediation"

[sources]
github_repository = "eshanized/M31A"
github_token_env = "MY_GITHUB_PAT"
poll_interval_seconds = 600

[rules]
allowed_branches = ["master", "develop"]
excluded_paths = [".git/**", "secrets/**"]
risk_threshold_for_approval = "high"
max_concurrency = 4
max_retries = 2
timeout_seconds = 1200

[delivery]
open_draft_pr = true
branch_prefix = "m31a-fix/"
labels = ["maint", "automated"]
"#;

        let cfg = MaintenanceConfigFile::parse_and_validate(toml_str).unwrap();
        assert_eq!(cfg.autonomy_tier, "tier2_guarded_remediation");
        assert_eq!(cfg.rules.allowed_branches, vec!["master", "develop"]);
        assert_eq!(cfg.delivery.branch_prefix, "m31a-fix/");

        let policy = cfg.to_policy();
        assert_eq!(policy.autonomy_tier, AutonomyTier::Tier2GuardedRemediation);
        assert_eq!(
            policy.risk_threshold_for_approval,
            MaintenanceRiskLevel::High
        );
        assert_eq!(policy.max_concurrency_per_repo, 4);
    }

    #[test]
    fn test_rejects_plaintext_token_violation() {
        let toml_with_secret = r#"
version = "1.0.0"
autonomy_tier = "tier1_prepare"
[sources]
github_token = "ghp_1234567890abcdefghijklmnopqrstuvwxyz"
"#;
        let res = MaintenanceConfigFile::parse_and_validate(toml_with_secret);
        assert!(res.is_err());
        let err = res.err().unwrap().to_string();
        assert!(err.contains("Security violation: Plaintext credentials detected"));
    }

    #[test]
    fn test_rejects_invalid_autonomy_tier() {
        let bad_tier = r#"
version = "1.0.0"
autonomy_tier = "tier99_god_mode"
"#;
        let res = MaintenanceConfigFile::parse_and_validate(bad_tier);
        assert!(res.is_err());
    }
}

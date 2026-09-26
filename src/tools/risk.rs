//! Baseline risk classification and dynamic effective risk evaluation (TL-01, D-08).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Baseline risk classification for capabilities and tools, ordered by severity.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum RiskClass {
    /// Non-mutating read-only operations.
    ReadOnly = 0,
    /// Low-risk mutations (scratch files, temporary outputs).
    LowRiskMutation = 1,
    /// High-risk mutations (source code edits, configuration changes, git commits).
    HighRiskMutation = 2,
    /// Execution of external commands or processes.
    ProcessExecution = 3,
    /// External communication over network or web APIs.
    ExternalCommunication = 4,
}

/// Non-bypassable dynamic risk computation combining base risk with argument modifiers.
///
/// Invariant: Effective risk can NEVER decrease below the base risk classification (D-08).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EffectiveRisk {
    base_risk: RiskClass,
    effective_risk: RiskClass,
    reasons: Vec<String>,
}

impl EffectiveRisk {
    /// Initialize dynamic risk from the tool's base risk.
    pub fn new(base_risk: RiskClass) -> Self {
        Self {
            base_risk,
            effective_risk: base_risk,
            reasons: Vec::new(),
        }
    }

    /// Construct with explicit candidate risk, strictly guaranteeing `effective >= base`.
    pub fn from_candidate(
        base_risk: RiskClass,
        candidate_risk: RiskClass,
        reason: impl Into<String>,
    ) -> Self {
        let mut risk = Self::new(base_risk);
        risk.escalate(candidate_risk, reason);
        risk
    }

    /// Base risk classification.
    pub fn base_risk(&self) -> RiskClass {
        self.base_risk
    }

    /// Computed effective risk classification (guaranteed >= base_risk).
    pub fn effective_risk(&self) -> RiskClass {
        self.effective_risk
    }

    /// Escalation reasons.
    pub fn reasons(&self) -> &[String] {
        &self.reasons
    }

    /// Escalate effective risk to a candidate risk level.
    ///
    /// If candidate risk is higher than current effective risk, escalates and records reason.
    /// If candidate risk is lower or equal, this is a NO-OP (risk can NEVER decrease below base or current effective).
    pub fn escalate(&mut self, candidate: RiskClass, reason: impl Into<String>) {
        if candidate > self.effective_risk {
            self.effective_risk = candidate;
            self.reasons.push(reason.into());
        }
    }

    /// Evaluates target path sensitivity to escalate file mutations.
    ///
    /// If base risk is mutating (`LowRiskMutation`), targeting sensitive dotfiles,
    /// repository configuration, or credentials escalates to `HighRiskMutation`.
    pub fn evaluate_file_path(base_risk: RiskClass, path: &Path) -> Self {
        let mut risk = Self::new(base_risk);
        if base_risk == RiskClass::LowRiskMutation && is_sensitive_path(path) {
            risk.escalate(
                RiskClass::HighRiskMutation,
                format!(
                    "target path '{}' is a sensitive configuration or dotfile",
                    path.display()
                ),
            );
        }
        risk
    }
}

/// Helper to determine if a path is sensitive (dotfiles, configuration, credentials, git internals).
pub fn is_sensitive_path(path: &Path) -> bool {
    let path_str = path.to_string_lossy();
    let components: Vec<&str> = path.iter().filter_map(|c| c.to_str()).collect();

    for comp in &components {
        // Dotfiles or directories (excluding "." and "..")
        if comp.starts_with('.') && *comp != "." && *comp != ".." {
            return true;
        }
        // Critical project configuration files
        if matches!(
            *comp,
            "Cargo.toml"
                | "Cargo.lock"
                | "package.json"
                | "package-lock.json"
                | "yarn.lock"
                | "pnpm-lock.yaml"
                | "tsconfig.json"
                | "Makefile"
                | "CMakeLists.txt"
                | "Dockerfile"
                | "docker-compose.yml"
                | "docker-compose.yaml"
        ) {
            return true;
        }
        // Sensitive file extensions
        if comp.ends_with(".pem")
            || comp.ends_with(".key")
            || comp.ends_with(".crt")
            || comp.ends_with(".pfx")
            || comp.ends_with(".env")
        {
            return true;
        }
    }

    path_str.contains(".git/") || path_str.contains("/.git")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_risk_order() {
        assert!(RiskClass::ReadOnly < RiskClass::LowRiskMutation);
        assert!(RiskClass::LowRiskMutation < RiskClass::HighRiskMutation);
        assert!(RiskClass::HighRiskMutation < RiskClass::ProcessExecution);
        assert!(RiskClass::ProcessExecution < RiskClass::ExternalCommunication);
    }

    #[test]
    fn test_effective_risk_never_decreases() {
        let mut risk = EffectiveRisk::new(RiskClass::HighRiskMutation);
        assert_eq!(risk.effective_risk(), RiskClass::HighRiskMutation);

        // Attempting to escalate to a lower risk must be a no-op
        risk.escalate(RiskClass::ReadOnly, "lower risk proposal");
        assert_eq!(risk.effective_risk(), RiskClass::HighRiskMutation);
        assert!(risk.reasons().is_empty());

        risk.escalate(RiskClass::LowRiskMutation, "lower risk proposal");
        assert_eq!(risk.effective_risk(), RiskClass::HighRiskMutation);
        assert!(risk.reasons().is_empty());

        // Escalating to higher risk succeeds
        risk.escalate(RiskClass::ProcessExecution, "spawning subprocess");
        assert_eq!(risk.effective_risk(), RiskClass::ProcessExecution);
        assert_eq!(risk.reasons().len(), 1);

        // Even from_candidate clamps at base_risk
        let clamped = EffectiveRisk::from_candidate(
            RiskClass::ProcessExecution,
            RiskClass::ReadOnly,
            "attempted downgrade",
        );
        assert_eq!(clamped.effective_risk(), RiskClass::ProcessExecution);
    }

    #[test]
    fn test_sensitive_path_escalation() {
        let risk1 = EffectiveRisk::evaluate_file_path(
            RiskClass::LowRiskMutation,
            Path::new("src/scratch.txt"),
        );
        assert_eq!(risk1.effective_risk(), RiskClass::LowRiskMutation);

        let risk2 =
            EffectiveRisk::evaluate_file_path(RiskClass::LowRiskMutation, Path::new(".env"));
        assert_eq!(risk2.effective_risk(), RiskClass::HighRiskMutation);

        let risk3 =
            EffectiveRisk::evaluate_file_path(RiskClass::LowRiskMutation, Path::new("Cargo.toml"));
        assert_eq!(risk3.effective_risk(), RiskClass::HighRiskMutation);

        let risk4 =
            EffectiveRisk::evaluate_file_path(RiskClass::LowRiskMutation, Path::new(".git/config"));
        assert_eq!(risk4.effective_risk(), RiskClass::HighRiskMutation);
    }
}

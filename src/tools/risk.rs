//! Baseline risk classification and dynamic effective risk evaluation (TL-01, D-08).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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

    /// Evaluates target path sensitivity to escalate file mutations,
    /// checking both lexical user-provided alias and canonical effective target on disk (F-09).
    pub fn evaluate_file_target(
        base_risk: RiskClass,
        lexical_path: &Path,
        effective_target: Option<&Path>,
    ) -> Self {
        let mut risk = Self::new(base_risk);
        if base_risk == RiskClass::LowRiskMutation {
            if is_sensitive_path(lexical_path) {
                risk.escalate(
                    RiskClass::HighRiskMutation,
                    format!(
                        "target path '{}' is a sensitive configuration or dotfile",
                        lexical_path.display()
                    ),
                );
            } else if let Some(target) = effective_target {
                if is_sensitive_path(target) {
                    risk.escalate(
                        RiskClass::HighRiskMutation,
                        format!(
                            "target path '{}' resolves through symlink to sensitive target '{}'",
                            lexical_path.display(),
                            target.display()
                        ),
                    );
                }
            }
        }
        risk
    }

    /// Evaluates target path sensitivity to escalate file mutations.
    ///
    /// If base risk is mutating (`LowRiskMutation`), targeting sensitive dotfiles,
    /// repository configuration, or credentials escalates to `HighRiskMutation`.
    /// Also resolves real filesystem target if file or parent exists on disk (F-09).
    pub fn evaluate_file_path(base_risk: RiskClass, path: &Path) -> Self {
        let effective = if path.exists() {
            path.canonicalize().ok()
        } else if let Some(parent) = path.parent() {
            parent
                .canonicalize()
                .ok()
                .map(|p| p.join(path.file_name().unwrap_or_default()))
        } else {
            None
        };
        Self::evaluate_file_target(base_risk, path, effective.as_deref())
    }
}

/// Helper to determine if a path is sensitive (dotfiles, configuration, credentials, git internals).
///
/// Security invariant (NEW-01): there is NO broad `.tmp*` exemption. A
/// filename that merely starts with `.tmp` (e.g. `.tmp.env`,
/// `.tmpsecrets`, `.tmp-private-key`, `.tmpdir/.ssh/id_ed25519`) is still
/// a dotfile and is still classified sensitive. Reduced risk applies ONLY
/// to paths inside an explicitly runtime-managed temporary namespace (see
/// [`is_runtime_managed_temp_path`]), which requires verified location,
/// ownership, and lifecycle — never a bare lexical prefix.
pub fn is_sensitive_path(path: &Path) -> bool {
    // Runtime-managed temporary files are the ONLY exempt namespace, and
    // only when they are NOT themselves sensitive aliases. A managed temp
    // path that aliases a sensitive name (e.g. `<tmp>/.tmp.env`,
    // `<tmp>/.ssh/id_ed25519`) remains sensitive.
    if is_runtime_managed_temp_path(path) && !is_sensitive_temp_alias(path) {
        return false;
    }
    is_sensitive_path_inner(path)
}

/// Runtime-managed temporary namespace predicate.
///
/// Returns true only for paths rooted in the OS temporary directory
/// (`std::env::temp_dir()`) or in a directory explicitly designated by the
/// runtime via the `M31A_MANAGED_TMP` environment variable. This is a
/// location check, not a filename-prefix check: arbitrary user workspace
/// paths such as `.tmp.env` or `.tmpsecrets` never satisfy it.
pub fn is_runtime_managed_temp_path(path: &Path) -> bool {
    let canon_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let managed_roots: Vec<PathBuf> = {
        let mut roots = vec![std::env::temp_dir()];
        if let Ok(extra) = std::env::var("M31A_MANAGED_TMP") {
            if !extra.trim().is_empty() {
                roots.push(PathBuf::from(extra));
            }
        }
        roots
    };
    managed_roots.iter().any(|root| {
        let canon_root = root.canonicalize().unwrap_or_else(|_| root.clone());
        canon_path.starts_with(&canon_root)
    })
}

/// Detect sensitive aliases smuggled inside the managed-temp namespace
/// (e.g. `<tmp>/.tmp.env`, `<tmp>/.ssh/id_ed25519`, `<tmp>/.git/config`).
/// These retain sensitive classification despite the managed location.
fn is_sensitive_temp_alias(path: &Path) -> bool {
    // Evaluate the file name and every component relative to the managed
    // root with the same inner rules (dotfiles, sensitive names/extensions,
    // protected components). If any suffix component is sensitive, the alias
    // is sensitive.
    is_sensitive_path_inner(path)
}

fn is_sensitive_path_inner(path: &Path) -> bool {
    let path_str = path.to_string_lossy();
    let components: Vec<&str> = path.iter().filter_map(|c| c.to_str()).collect();

    for comp in &components {
        // Dotfiles or directories (excluding "." and "..").
        // NEW-01: NO `.tmp*` exemption. Any dotfile component — including
        // `.tmp.env`, `.tmpsecrets`, `.tmpcredentials`,
        // `.tmp-private-key` — is sensitive.
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

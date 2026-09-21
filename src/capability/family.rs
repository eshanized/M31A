//! Core capability families enum (CTL-01, D-01).

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// The 15 core capability families recognized by the M31A runtime (CTL-01).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityFamily {
    /// File operations within the workspace boundary.
    Filesystem,
    /// Bounded shell execution.
    Shell,
    /// Direct command/process spawning and supervision.
    Process,
    /// Interactive terminal sessions.
    Terminal,
    /// Code graph, symbol navigation, and semantic repository search.
    Repository,
    /// Version control operations.
    Git,
    /// Web fetching and external documentation search.
    Web,
    /// Network connectivity, reachability, and DNS resolution.
    Network,
    /// Supervised long-running background jobs.
    Jobs,
    /// Isolated execution environment and boundary management.
    Sandbox,
    /// LLM inference and reasoning invocations.
    Model,
    /// Contextual memory and key-value recall.
    Memory,
    /// Automated quality checks, tests, formatting, and linting.
    Verification,
    /// Immutable artifact storage and retrieval.
    Artifacts,
    /// Telemetry, metric recording, and event emission.
    Telemetry,
}

impl CapabilityFamily {
    /// Returns a slice of all 15 core capability families.
    pub const fn all() -> &'static [Self] {
        &[
            Self::Filesystem,
            Self::Shell,
            Self::Process,
            Self::Terminal,
            Self::Repository,
            Self::Git,
            Self::Web,
            Self::Network,
            Self::Jobs,
            Self::Sandbox,
            Self::Model,
            Self::Memory,
            Self::Verification,
            Self::Artifacts,
            Self::Telemetry,
        ]
    }

    /// Returns the canonical snake_case string identifier for this capability family.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Filesystem => "filesystem",
            Self::Shell => "shell",
            Self::Process => "process",
            Self::Terminal => "terminal",
            Self::Repository => "repository",
            Self::Git => "git",
            Self::Web => "web",
            Self::Network => "network",
            Self::Jobs => "jobs",
            Self::Sandbox => "sandbox",
            Self::Model => "model",
            Self::Memory => "memory",
            Self::Verification => "verification",
            Self::Artifacts => "artifacts",
            Self::Telemetry => "telemetry",
        }
    }
}

impl fmt::Display for CapabilityFamily {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for CapabilityFamily {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "filesystem" | "fs" => Ok(Self::Filesystem),
            "shell" => Ok(Self::Shell),
            "process" => Ok(Self::Process),
            "terminal" => Ok(Self::Terminal),
            "repository" | "repo" => Ok(Self::Repository),
            "git" => Ok(Self::Git),
            "web" => Ok(Self::Web),
            "network" | "net" => Ok(Self::Network),
            "jobs" | "job" => Ok(Self::Jobs),
            "sandbox" => Ok(Self::Sandbox),
            "model" => Ok(Self::Model),
            "memory" => Ok(Self::Memory),
            "verification" | "qa" => Ok(Self::Verification),
            "artifacts" | "artifact" => Ok(Self::Artifacts),
            "telemetry" => Ok(Self::Telemetry),
            other => Err(format!("unknown capability family: {other}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_15_families_present() {
        assert_eq!(CapabilityFamily::all().len(), 15);
    }

    #[test]
    fn test_family_round_trip() {
        for family in CapabilityFamily::all() {
            let s = family.to_string();
            let parsed: CapabilityFamily = s.parse().unwrap();
            assert_eq!(*family, parsed);
        }
    }
}

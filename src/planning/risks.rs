//! Planning risks, assumptions, and unknowns with explicit criticality ratings (PLN-01, PLN-02, D-08).

use crate::planning::requirements::Provenance;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Criticality level for risks, assumptions, and unknowns (D-08).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Criticality {
    Low,
    Medium,
    High,
    Critical,
}

impl Criticality {
    /// True if the criticality is High or Critical, requiring explicit mitigation/resolution to unblock tasks.
    pub fn is_blocking_threshold(&self) -> bool {
        matches!(self, Self::High | Self::Critical)
    }
}

impl fmt::Display for Criticality {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

impl std::str::FromStr for Criticality {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            "critical" => Ok(Self::Critical),
            other => Err(format!("Unknown criticality level: {}", other)),
        }
    }
}

/// A planning risk with severity and mitigation strategy (D-08).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanningRisk {
    pub id: String,
    pub description: String,
    pub criticality: Criticality,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mitigation: Option<String>,
    pub provenance: Provenance,
}

impl PlanningRisk {
    pub fn new(
        id: impl Into<String>,
        description: impl Into<String>,
        criticality: Criticality,
        provenance: Provenance,
    ) -> Self {
        Self {
            id: id.into(),
            description: description.into(),
            criticality,
            mitigation: None,
            provenance,
        }
    }

    pub fn with_mitigation(mut self, mitigation: impl Into<String>) -> Self {
        self.mitigation = Some(mitigation.into());
        self
    }
}

/// A planning assumption with criticality and validation/resolution strategy (D-08).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanningAssumption {
    pub id: String,
    pub description: String,
    pub criticality: Criticality,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution_strategy: Option<String>,
    pub provenance: Provenance,
}

impl PlanningAssumption {
    pub fn new(
        id: impl Into<String>,
        description: impl Into<String>,
        criticality: Criticality,
        provenance: Provenance,
    ) -> Self {
        Self {
            id: id.into(),
            description: description.into(),
            criticality,
            resolution_strategy: None,
            provenance,
        }
    }

    pub fn with_resolution_strategy(mut self, strategy: impl Into<String>) -> Self {
        self.resolution_strategy = Some(strategy.into());
        self
    }
}

/// The classified fate of an unknown piece of domain/technical information.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnknownFate {
    /// Safe to select a reasonable, reversible default without bothering the operator.
    SafeToInfer,
    /// Unresolved domain/technical uncertainty that warrants bounded research.
    Researchable,
    /// Genuinely consequential architectural, financial, or security decision requiring user input.
    UserDecisionRequired,
    /// Critical blocker that completely prevents execution until clarified.
    Blocking,
}

fn default_unknown_fate() -> UnknownFate {
    UnknownFate::SafeToInfer
}

impl fmt::Display for UnknownFate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SafeToInfer => write!(f, "safe_to_infer"),
            Self::Researchable => write!(f, "researchable"),
            Self::UserDecisionRequired => write!(f, "user_decision_required"),
            Self::Blocking => write!(f, "blocking"),
        }
    }
}

/// A planning unknown that must be resolved prior to or during execution (D-08).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanningUnknown {
    pub id: String,
    pub description: String,
    pub criticality: Criticality,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discovery_plan: Option<String>,
    pub provenance: Provenance,
    #[serde(default = "default_unknown_fate")]
    pub fate: UnknownFate,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
}

impl PlanningUnknown {
    pub fn new(
        id: impl Into<String>,
        description: impl Into<String>,
        criticality: Criticality,
        provenance: Provenance,
    ) -> Self {
        Self {
            id: id.into(),
            description: description.into(),
            criticality,
            discovery_plan: None,
            provenance,
            fate: UnknownFate::SafeToInfer,
            resolution: None,
        }
    }

    pub fn with_discovery_plan(mut self, plan: impl Into<String>) -> Self {
        self.discovery_plan = Some(plan.into());
        self
    }

    pub fn with_fate(mut self, fate: UnknownFate) -> Self {
        self.fate = fate;
        self
    }

    pub fn with_resolution(mut self, resolution: impl Into<String>) -> Self {
        self.resolution = Some(resolution.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planning::requirements::{ProvenanceSourceType, TrustLevel};

    #[test]
    fn test_critical_assumption_construction_and_serde() {
        let prov = Provenance::new(
            ProvenanceSourceType::ToolOutput,
            TrustLevel::VerifiedRepository,
            "static_inspector",
        )
        .with_location("Cargo.toml");

        let assumption = PlanningAssumption::new(
            "ASM-01",
            "Target environment has libsqlite3 available or bundled",
            Criticality::Critical,
            prov,
        )
        .with_resolution_strategy("Verify compilation with bundled feature enabled");

        assert_eq!(assumption.criticality, Criticality::Critical);
        assert!(assumption.criticality.is_blocking_threshold());

        let json = serde_json::to_string_pretty(&assumption).expect("serialize assumption");
        let deserialized: PlanningAssumption =
            serde_json::from_str(&json).expect("deserialize assumption");

        assert_eq!(assumption.id, deserialized.id);
        assert_eq!(assumption.description, deserialized.description);
        assert_eq!(assumption.criticality, deserialized.criticality);
        assert_eq!(
            assumption.resolution_strategy,
            deserialized.resolution_strategy
        );
        assert_eq!(assumption.provenance.actor, deserialized.provenance.actor);
    }

    #[test]
    fn test_planning_risk_and_unknown() {
        let prov = Provenance::new(
            ProvenanceSourceType::UserPrompt,
            TrustLevel::UntrustedExternal,
            "operator",
        );

        let risk = PlanningRisk::new(
            "RSK-01",
            "High token usage during plan validation",
            Criticality::Medium,
            prov.clone(),
        )
        .with_mitigation("Run validation strictly offline with zero LLM queries");

        assert_eq!(risk.criticality, Criticality::Medium);
        assert!(!risk.criticality.is_blocking_threshold());

        let unknown = PlanningUnknown::new(
            "UNK-01",
            "Does repository contain yarn or npm lockfiles?",
            Criticality::High,
            prov,
        )
        .with_discovery_plan("Execute ConstraintDiscoveryEngine over repo root");

        assert_eq!(unknown.criticality, Criticality::High);
        assert!(unknown.criticality.is_blocking_threshold());
    }
}

//! Engineering requirements, epistemic classifications, and provenance trust models (PLN-01, PLN-04, D-05, D-06).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Six canonical epistemic truth levels for requirements and facts (PLN-04, D-05).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EpistemicStatus {
    ExplicitUserRequirement,
    VerifiedFact,
    InferredFact,
    Hypothesis,
    Assumption,
    Unknown,
}

impl fmt::Display for EpistemicStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExplicitUserRequirement => write!(f, "explicit_user_requirement"),
            Self::VerifiedFact => write!(f, "verified_fact"),
            Self::InferredFact => write!(f, "inferred_fact"),
            Self::Hypothesis => write!(f, "hypothesis"),
            Self::Assumption => write!(f, "assumption"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

impl std::str::FromStr for EpistemicStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "explicit_user_requirement" | "user_requirement" => Ok(Self::ExplicitUserRequirement),
            "verified_fact" | "verified" => Ok(Self::VerifiedFact),
            "inferred_fact" | "inferred" => Ok(Self::InferredFact),
            "hypothesis" => Ok(Self::Hypothesis),
            "assumption" => Ok(Self::Assumption),
            "unknown" => Ok(Self::Unknown),
            other => Err(format!("Unknown epistemic status: {}", other)),
        }
    }
}

/// Authority and trust classification of a provenance source (D-06, CTX-03).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustLevel {
    UntrustedModelProposal,
    UntrustedExternal,
    VerifiedRepository,
    AuthoritativeRuntime,
}

impl TrustLevel {
    /// Indicates whether this trust level is authoritative enough to establish a VerifiedFact (D-06).
    pub fn can_verify_facts(&self) -> bool {
        matches!(self, Self::VerifiedRepository | Self::AuthoritativeRuntime)
    }
}

impl fmt::Display for TrustLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AuthoritativeRuntime => write!(f, "authoritative_runtime"),
            Self::VerifiedRepository => write!(f, "verified_repository"),
            Self::UntrustedExternal => write!(f, "untrusted_external"),
            Self::UntrustedModelProposal => write!(f, "untrusted_model_proposal"),
        }
    }
}

impl std::str::FromStr for TrustLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "authoritative_runtime" | "runtime" => Ok(Self::AuthoritativeRuntime),
            "verified_repository" | "repository" => Ok(Self::VerifiedRepository),
            "untrusted_external" | "external" => Ok(Self::UntrustedExternal),
            "untrusted_model_proposal" | "model" | "proposal" => Ok(Self::UntrustedModelProposal),
            other => Err(format!("Unknown trust level: {}", other)),
        }
    }
}

/// Source categorization for provenance records (D-06).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceSourceType {
    UserPrompt,
    RepositoryFile,
    ToolOutput,
    ExternalDoc,
    RuntimePolicy,
}

impl fmt::Display for ProvenanceSourceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UserPrompt => write!(f, "user_prompt"),
            Self::RepositoryFile => write!(f, "repository_file"),
            Self::ToolOutput => write!(f, "tool_output"),
            Self::ExternalDoc => write!(f, "external_doc"),
            Self::RuntimePolicy => write!(f, "runtime_policy"),
        }
    }
}

impl std::str::FromStr for ProvenanceSourceType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "user_prompt" | "prompt" => Ok(Self::UserPrompt),
            "repository_file" | "file" => Ok(Self::RepositoryFile),
            "tool_output" | "tool" => Ok(Self::ToolOutput),
            "external_doc" | "doc" => Ok(Self::ExternalDoc),
            "runtime_policy" | "policy" => Ok(Self::RuntimePolicy),
            other => Err(format!("Unknown provenance source type: {}", other)),
        }
    }
}

/// Strongly typed provenance record binding claims to authoritative sources (D-06).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    pub source_type: ProvenanceSourceType,
    pub trust_level: TrustLevel,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub actor: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl Provenance {
    pub fn new(
        source_type: ProvenanceSourceType,
        trust_level: TrustLevel,
        actor: impl Into<String>,
    ) -> Self {
        Self {
            source_type,
            trust_level,
            location: None,
            timestamp: Utc::now(),
            actor: actor.into(),
            reason: None,
        }
    }

    pub fn with_location(mut self, location: impl Into<String>) -> Self {
        self.location = Some(location.into());
        self
    }

    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

/// Audit record for an epistemic status transition (D-05, D-06).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpistemicRevision {
    pub from_status: EpistemicStatus,
    pub to_status: EpistemicStatus,
    pub provenance: Provenance,
    pub reason: String,
    pub timestamp: DateTime<Utc>,
}

impl EpistemicRevision {
    pub fn new(
        from_status: EpistemicStatus,
        to_status: EpistemicStatus,
        provenance: Provenance,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            from_status,
            to_status,
            provenance,
            reason: reason.into(),
            timestamp: Utc::now(),
        }
    }
}

use crate::ids::RequirementId;
use std::ops::Deref;

/// Spec-level identifier for engineering requirements (e.g. "PLN-01", "REQ-INIT-01").
///
/// Distinct from runtime UUIDv7 `RequirementId` (FINDING-10).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RequirementKey(pub String);

impl RequirementKey {
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Deref for RequirementKey {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl fmt::Display for RequirementKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for RequirementKey {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for RequirementKey {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// Canonical categories for engineering requirements.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum RequirementCategory {
    #[default]
    Functional,
    NonFunctional,
    Security,
    Reliability,
    Performance,
    Operational,
    Compatibility,
    UxProduct,
    Deployment,
    Compliance,
}

impl fmt::Display for RequirementCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Functional => write!(f, "functional"),
            Self::NonFunctional => write!(f, "non_functional"),
            Self::Security => write!(f, "security"),
            Self::Reliability => write!(f, "reliability"),
            Self::Performance => write!(f, "performance"),
            Self::Operational => write!(f, "operational"),
            Self::Compatibility => write!(f, "compatibility"),
            Self::UxProduct => write!(f, "ux_product"),
            Self::Deployment => write!(f, "deployment"),
            Self::Compliance => write!(f, "compliance"),
        }
    }
}

impl std::str::FromStr for RequirementCategory {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().replace(['-', ' '], "_").as_str() {
            "functional" | "func" => Ok(Self::Functional),
            "non_functional" | "nonfunctional" | "nf" => Ok(Self::NonFunctional),
            "security" | "sec" => Ok(Self::Security),
            "reliability" | "rel" => Ok(Self::Reliability),
            "performance" | "perf" => Ok(Self::Performance),
            "operational" | "ops" => Ok(Self::Operational),
            "compatibility" | "compat" => Ok(Self::Compatibility),
            "ux_product" | "ux" | "product" => Ok(Self::UxProduct),
            "deployment" | "deploy" => Ok(Self::Deployment),
            "compliance" | "comp" => Ok(Self::Compliance),
            other => Err(format!("Unknown requirement category: {}", other)),
        }
    }
}

/// Priority classification for engineering requirements.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum RequirementPriority {
    Must,
    #[default]
    Should,
    Could,
    Deferred,
}

impl fmt::Display for RequirementPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Must => write!(f, "must"),
            Self::Should => write!(f, "should"),
            Self::Could => write!(f, "could"),
            Self::Deferred => write!(f, "deferred"),
        }
    }
}

impl std::str::FromStr for RequirementPriority {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "must" | "mandatory" | "p0" => Ok(Self::Must),
            "should" | "p1" => Ok(Self::Should),
            "could" | "nice_to_have" | "p2" => Ok(Self::Could),
            "deferred" | "backlog" | "p3" => Ok(Self::Deferred),
            other => Err(format!("Unknown requirement priority: {}", other)),
        }
    }
}

/// Machine-readable engineering requirement with evidence-backed epistemic classification (PLN-01, PLN-04, FINDING-10).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EngineeringRequirement {
    pub key: RequirementKey,
    pub id: RequirementId,
    pub description: String,
    pub current_status: EpistemicStatus,
    pub provenance: Provenance,
    #[serde(default)]
    pub history: Vec<EpistemicRevision>,
    #[serde(default)]
    pub satisfaction_criteria: Vec<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub category: RequirementCategory,
    #[serde(default)]
    pub priority: RequirementPriority,
    #[serde(default)]
    pub rationale: Option<String>,
    #[serde(default)]
    pub dependencies: Vec<RequirementKey>,
    #[serde(default)]
    pub risks: Vec<String>,
}

impl EngineeringRequirement {
    pub fn new(
        key: impl Into<RequirementKey>,
        description: impl Into<String>,
        initial_status: EpistemicStatus,
        provenance: Provenance,
        satisfaction_criteria: Vec<String>,
    ) -> Self {
        let priority = match initial_status {
            EpistemicStatus::ExplicitUserRequirement => RequirementPriority::Must,
            _ => RequirementPriority::Should,
        };
        Self {
            key: key.into(),
            id: RequirementId::new(),
            description: description.into(),
            current_status: initial_status,
            provenance,
            history: Vec::new(),
            satisfaction_criteria,
            title: None,
            category: RequirementCategory::Functional,
            priority,
            rationale: None,
            dependencies: Vec::new(),
            risks: Vec::new(),
        }
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn with_category(mut self, category: RequirementCategory) -> Self {
        self.category = category;
        self
    }

    pub fn with_priority(mut self, priority: RequirementPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_rationale(mut self, rationale: impl Into<String>) -> Self {
        self.rationale = Some(rationale.into());
        self
    }

    pub fn with_dependencies(mut self, dependencies: Vec<RequirementKey>) -> Self {
        self.dependencies = dependencies;
        self
    }

    pub fn with_risks(mut self, risks: Vec<String>) -> Self {
        self.risks = risks;
        self
    }

    pub fn title_str(&self) -> &str {
        self.title.as_deref().unwrap_or(&self.description)
    }

    pub fn statement(&self) -> &str {
        &self.description
    }

    /// Transitions epistemic status while enforcing evidence trust invariants (D-05, D-06).
    pub fn transition_status(
        &mut self,
        new_status: EpistemicStatus,
        evidence: Provenance,
        reason: impl Into<String>,
    ) -> Result<(), String> {
        let reason_str = reason.into();

        // Enforcement: Cannot promote to VerifiedFact without authoritative or verified repository evidence
        if new_status == EpistemicStatus::VerifiedFact && !evidence.trust_level.can_verify_facts() {
            return Err(format!(
                "Cannot transition to VerifiedFact: provenance trust level '{:?}' is not authoritative",
                evidence.trust_level
            ));
        }

        let revision = EpistemicRevision::new(
            self.current_status,
            new_status,
            evidence.clone(),
            &reason_str,
        );
        self.history.push(revision);
        self.current_status = new_status;
        self.provenance = evidence;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_epistemic_status_serde_and_display() {
        let statuses = [
            (
                EpistemicStatus::ExplicitUserRequirement,
                "\"explicit_user_requirement\"",
            ),
            (EpistemicStatus::VerifiedFact, "\"verified_fact\""),
            (EpistemicStatus::InferredFact, "\"inferred_fact\""),
            (EpistemicStatus::Hypothesis, "\"hypothesis\""),
            (EpistemicStatus::Assumption, "\"assumption\""),
            (EpistemicStatus::Unknown, "\"unknown\""),
        ];

        for (status, json_str) in statuses {
            let serialized = serde_json::to_string(&status).unwrap();
            assert_eq!(serialized, json_str);
            let deserialized: EpistemicStatus = serde_json::from_str(json_str).unwrap();
            assert_eq!(deserialized, status);
        }
    }

    #[test]
    fn test_trust_level_serde_and_verification_rules() {
        assert!(TrustLevel::AuthoritativeRuntime.can_verify_facts());
        assert!(TrustLevel::VerifiedRepository.can_verify_facts());
        assert!(!TrustLevel::UntrustedExternal.can_verify_facts());
        assert!(!TrustLevel::UntrustedModelProposal.can_verify_facts());
    }

    #[test]
    fn test_provenance_full_roundtrip() {
        let prov = Provenance::new(
            ProvenanceSourceType::RepositoryFile,
            TrustLevel::VerifiedRepository,
            "static_analyzer",
        )
        .with_location("Cargo.toml:L15")
        .with_reason("Parsed crate manifest metadata");

        let json = serde_json::to_string(&prov).unwrap();
        let deserialized: Provenance = serde_json::from_str(&json).unwrap();
        assert_eq!(prov, deserialized);
    }

    #[test]
    fn test_requirement_status_transition_with_provenance_guard() {
        let initial_prov = Provenance::new(
            ProvenanceSourceType::UserPrompt,
            TrustLevel::UntrustedExternal,
            "user",
        );
        let mut req = EngineeringRequirement::new(
            "REQ-01",
            "Support SQLite WAL mode",
            EpistemicStatus::ExplicitUserRequirement,
            initial_prov,
            vec!["WAL mode enabled".to_string()],
        );

        // Attempt invalid promotion to VerifiedFact using untrusted model proposal
        let model_prov = Provenance::new(
            ProvenanceSourceType::ToolOutput,
            TrustLevel::UntrustedModelProposal,
            "model_planner",
        );
        let err = req.transition_status(
            EpistemicStatus::VerifiedFact,
            model_prov,
            "Model claims WAL mode is verified",
        );
        assert!(err.is_err());
        assert!(err.unwrap_err().contains("not authoritative"));
        assert_eq!(req.current_status, EpistemicStatus::ExplicitUserRequirement);

        // Valid promotion using authoritative runtime evidence
        let runtime_prov = Provenance::new(
            ProvenanceSourceType::RuntimePolicy,
            TrustLevel::AuthoritativeRuntime,
            "runtime_sqlite_verifier",
        );
        let ok = req.transition_status(
            EpistemicStatus::VerifiedFact,
            runtime_prov,
            "Runtime verified PRAGMA journal_mode=WAL",
        );
        assert!(ok.is_ok());
        assert_eq!(req.current_status, EpistemicStatus::VerifiedFact);
        assert_eq!(req.history.len(), 1);
        assert_eq!(
            req.history[0].from_status,
            EpistemicStatus::ExplicitUserRequirement
        );
        assert_eq!(req.history[0].to_status, EpistemicStatus::VerifiedFact);
    }

    #[test]
    fn test_requirement_category_and_priority() {
        let prov = Provenance::new(
            ProvenanceSourceType::UserPrompt,
            TrustLevel::AuthoritativeRuntime,
            "operator",
        );
        let req = EngineeringRequirement::new(
            "REQ-SEC-01",
            "Enforce token authentication",
            EpistemicStatus::ExplicitUserRequirement,
            prov,
            vec!["Reject unauthenticated requests".to_string()],
        )
        .with_title("Token Auth")
        .with_category(RequirementCategory::Security)
        .with_priority(RequirementPriority::Must)
        .with_rationale("Security posture requirement")
        .with_dependencies(vec![RequirementKey::new("REQ-INIT-01")])
        .with_risks(vec!["Token leakage".to_string()]);

        assert_eq!(req.category, RequirementCategory::Security);
        assert_eq!(req.priority, RequirementPriority::Must);
        assert_eq!(req.title_str(), "Token Auth");
        assert_eq!(req.statement(), "Enforce token authentication");
        assert_eq!(req.dependencies.len(), 1);
        assert_eq!(req.risks.len(), 1);

        let json = serde_json::to_string(&req).unwrap();
        let deserialized: EngineeringRequirement = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.category, RequirementCategory::Security);
        assert_eq!(deserialized.priority, RequirementPriority::Must);
        assert_eq!(deserialized.title, Some("Token Auth".to_string()));
    }
}

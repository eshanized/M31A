//! Verification subsystem (VER-01–VER-05).
//!
//! Provides the 7-tier verification hierarchy engine, concrete runner adapters,
//! Tier 6 independent reviewer agent with fresh-context read-only isolation,
//! and evidence-backed completion gate with cryptographic snapshot binding.

pub mod adapter;
pub mod diagnostician;
pub mod executor;
pub mod gate;
pub mod hierarchy;
pub mod reviewer;
pub mod runners;
pub mod types;

pub use executor::{
    ApprovedVerificationCommand, approve_verification_command, execute_approved,
    execute_governed_verification,
};

pub use adapter::{ProjectAdapter, ProjectType};
pub use diagnostician::{
    DiagnosticHypothesis, DiagnosticReport, DiagnosticianContext, ModelDiagnostician,
    RecoveryRecommendation,
};
pub use gate::{EvidenceCompletionGate, ToolCapabilityClass};
pub use hierarchy::VerificationHierarchyEngine;
pub use reviewer::{
    IndependentReviewer, ReviewDecision, ReviewFinding, ReviewFindingSeverity, ReviewVerdict,
    ReviewerAgentContext, ReviewerExecutionError,
};
pub use types::{
    CheckStatus, CheckTier, CompilerDiagnosticItem, CompletionGateDecision,
    EvidenceEpistemicStatus, FailureEvidence, RequirementCheckCoverage, TestFailureItem,
    VerificationCheck,
};

//! Kernel module - cross-cutting runtime invariants and lifecycle concerns (D-18)
//!
//! Per D-18, kernel/ owns cross-cutting runtime invariants, lifecycle concerns,
//! and the neutral cross-layer seam contracts (AD-007).

pub mod cancellation;
pub mod change;
pub mod invariants;
pub mod memory;
pub mod plan;
pub mod seams;

pub use cancellation::MissionRuntime;
pub use change::{
    ChangeProposal, ChangeProposalId, ChangeProvenanceRecord, ChangeSurface, DiffReviewReport,
    DiffReviewViolation, FileMutationOp, FileMutationProposal, FilePrecondition,
    ImplementationHypothesis, ReconciliationReport, ReconciliationViolation,
};
pub use invariants::{RuntimeInvariants, contains_protected_component, is_protected_component};
pub use memory::{
    AssumptionStatus, DecisionStatus, EngineeringAssumption, EngineeringDecision,
    EngineeringMemorySnapshot, FailureDiagnosisRecord, FileHashRecord, MemoryScope, RepairStatus,
    ReviewFindingRecord, ReviewFindingSeverity, ReviewFindingStatus, VerificationRecord,
    VerificationValidity,
};
pub use plan::{
    CandidatePlan, CandidateTask, CandidateTaskKey, CapabilityAccessMode, CapabilityRequirement,
    ResourceEstimate, TaskResult, VerificationStrategy,
};

//! Planning subsystem module (PLN-01..PLN-05, PLN-22).
//!
//! Owns candidate task decomposition, engineering requirements with epistemic classification,
//! static repository constraint intake, deterministic multi-stage plan validation, and
//! one-way planning artifact projections.
//!
//! Contracts include: PlanQualityMetrics telemetry, CandidatePlanAssumption, TaskRiskLevel,
//! ReplanningTrigger, and PlanRevisionRecord as kernel contracts.

pub mod constraints;
pub mod metrics;
pub mod projections;
pub mod requirements;
pub mod review;
pub mod risks;
pub mod service;
pub mod validation;

// Canonical plan domain models re-exported from kernel.
pub use crate::kernel::plan::{
    CandidatePlan, CandidateTask, CandidateTaskKey, CapabilityAccessMode, CapabilityRequirement,
    MAX_COMPOSITE_DEPTH, ResourceEstimate, VerificationStrategy,
};
pub use constraints::{ConstraintDiscoveryEngine, RepositoryConstraint};
pub use metrics::PlanQualityMetrics;
pub use projections::{
    MissionProjection, PlanProjection, ProjectionFrontmatter, ProjectionRenderer,
    RequirementsProjection, StateProjection, is_projection_stale, mission_projections_dir,
};
pub use requirements::{
    EngineeringRequirement, EpistemicRevision, EpistemicStatus, Provenance, ProvenanceSourceType,
    RequirementCategory, RequirementKey, RequirementPriority, TrustLevel,
};
pub use review::{
    AuthorizationDecision, ExecutionAuthorization, PlanReviewSession, PlanReviewStatus,
    PlanRevision, RevisionAuthorType, TaskGraphValidationError, TaskReviewSession,
    TaskReviewStatus, TaskRevision, validate_candidate_tasks,
};
pub use risks::{Criticality, PlanningAssumption, PlanningRisk, PlanningUnknown, UnknownFate};
pub use service::PlanServiceImpl;
pub use service::{ObjectiveClassification, classify_objective};
pub use validation::{PlanValidator, ValidationError, ValidationReport, ValidationWarning};

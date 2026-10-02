//! The 11 non-bypassable typed state-machine stages of the tool execution pipeline (TL-03, D-09).
//!
//! State transitions:
//! `ActionRequest`
//!   -> Stage 1: `ToolResolutionStage` (`ResolvedToolState`)
//!   -> Stage 2: `ArgDecodingStage` (`DecodedArgsState`)
//!   -> Stage 3: `SchemaValidationStage` (`SchemaValidatedState`)
//!   -> Stage 4: `SemanticValidationStage` (`SemanticallyValidatedState`)
//!   -> Stage 5: `CapabilityCheckStage` (`CapabilityAuthorizedState`)
//!   -> Stage 6: `ResourceScopeStage` (`ResourceScopedState`)
//!   -> Stage 7: `PolicyGateStage` (`PolicyEvaluatedState`)
//!   -> Stage 8: `ApprovalResolutionStage` (`ExecutionAuthorizedState`)
//!   -> Stage 9: `ToolExecutionStage` (`RawExecutionState`) [MUTATING SIDE-EFFECTS HERE ONLY]
//!   -> Stage 10: `CaptureNormalizationStage` (`NormalizedResultState`)
//!   -> Stage 11: `TelemetryDurableRecordStage` (`ActionResult`)

pub mod approval_resolution;
pub mod capability_check;
pub mod capture_normalization;
pub mod decoding;
pub mod execution;
pub mod policy_gate;
pub mod resolution;
pub mod resource_scope;
pub mod schema_validation;
pub mod semantic_validation;
pub mod telemetry_record;

pub use approval_resolution::{ApprovalResolutionStage, ExecutionAuthorizedState};
pub use capability_check::{CapabilityAuthorizedState, CapabilityCheckStage};
pub use capture_normalization::{
    AuditDigest, CaptureNormalizationStage, DiagnosticOutput, ModelVisibleOutput,
    NormalizedResultState, PipelineOutputEvidence, RawExecutionEvidence,
};
pub use decoding::{ArgDecodingStage, DecodedArgsState};
pub use execution::{RawExecutionState, ToolExecutionStage};
pub use policy_gate::{PolicyEvaluatedState, PolicyGateStage};
pub use resolution::{ResolvedToolState, ToolResolutionStage};
pub use resource_scope::{ResourceScopeStage, ResourceScopedState};
pub use schema_validation::{SchemaValidatedState, SchemaValidationStage};
pub use semantic_validation::{SemanticValidationStage, SemanticallyValidatedState};
pub use telemetry_record::TelemetryDurableRecordStage;

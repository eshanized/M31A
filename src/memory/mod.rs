//! Long-Horizon Engineering Memory & Project State Continuity.
//!
//! Subordinates memory to runtime state and repository reality.
//! Enforces:
//! - Strict single-crate Rust architecture
//! - Zero second databases, zero second event stores, zero second checkpoint systems
//! - Epistemic assumption lifecycle (Active -> Tested -> Confirmed / Invalidated)
//! - Durable review findings surviving sessions
//! - Durable architectural decisions with supersession
//! - Failure diagnosis and failed repair memory to avoid repeating mutations
//! - Task-aware structured memory retrieval
//! - Repository-aware memory reconciliation against workspace reality
//! - Session continuity across restarts

pub mod continuity;
pub mod reconciler;
pub mod repository;
pub mod retrieval;
pub mod types;

pub use continuity::{EngineeringContinuityState, SessionContinuityEngine};
pub use reconciler::{MemoryReconciler, MemoryReconciliationReport};
pub use repository::{EngineeringMemoryStore, SqliteEngineeringMemoryRepository};
pub use retrieval::{MemoryRetrievalCriteria, TaskAwareMemoryRetriever};
pub use types::{
    AssumptionStatus, DecisionStatus, EngineeringAssumption, EngineeringDecision,
    EngineeringMemorySnapshot, FailureDiagnosisRecord, FileHashRecord, MemoryScope, RepairStatus,
    ReviewFindingRecord, ReviewFindingSeverity, ReviewFindingStatus, VerificationRecord,
    VerificationValidity,
};

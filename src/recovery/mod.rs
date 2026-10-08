//! Failure Controller, Layered Recovery, and Replanning Subsystem.
//!
//! Provides deterministic failure classification,
//! layered class-specific recovery budgets with exponential backoff and SQLite audit,
//! differential DAG replanning with task completion preservation,
//! ReplanScope classification, and ReplanningTrigger provenance tracking.

pub mod adapter;
pub mod budget;
pub mod classifier;
pub mod replan;

pub use adapter::ProductionRecoveryEngine;
pub use budget::{
    AttemptStrategyClassification, BackoffConfig, BudgetEvaluation, RecoveryAttemptRecord,
    RecoveryBudgetTracker, compute_mutation_fingerprint,
};
pub use classifier::{FailureClass, FailureClassifier};
pub use replan::{
    DifferentialReplanEngine, DifferentialReplanOutcome, DifferentialReplanRequest,
    ReplanAuthority, ReplanScope, classify_scope,
};

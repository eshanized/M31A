//! `m31a-maintenance`: Autonomous Repository Maintenance Engine.
//!
//! Provides signal intake, deduplication, deterministic triage, repository-intelligence
//! guided maintenance planning, isolated workflow execution with Initiative 2 verification,
//! and reviewable pull request / deliverable preparation.

pub mod config;
pub mod delivery;
pub mod domain;
pub mod intake;
pub mod planner;
pub mod scheduler;
pub mod storage;
pub mod triage;
pub mod workflow;

pub use config::*;
pub use delivery::*;
pub use domain::*;
pub use intake::*;
pub use planner::*;
pub use scheduler::*;
pub use storage::MaintenanceStore;
pub use triage::TriageEngine;
pub use workflow::*;

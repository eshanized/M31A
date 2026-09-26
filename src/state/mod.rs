//! State module - authoritative domain state models (KRN-02)
//!
//! Per D-14, state/ owns authoritative domain state models.
//! Per D-11, these models persist current state while an append-only
//! transition log captures audit/replay data.

pub mod agent;
pub mod budget;
pub mod completion;
pub mod intake;
pub mod mission;
pub mod mission_service;
pub mod policy_context;
pub mod recovery;
pub mod session;
pub mod task;

pub use agent::Agent;
pub use budget::ResourceBudget;
pub use completion::{CompletionContext, CompletionGate, CompletionGateError};
pub use intake::{AutonomyMode, IntakeValidationError, MissionIntake, NormalizedIntake};
pub use mission::Mission;
pub use mission_service::MissionService;
pub use policy_context::PolicyContext;
pub use recovery::{CrashClassification, StateReconstructionEngine};
pub use session::{Session, SessionState};
pub use task::{BlockingReason, Task, TaskResult};

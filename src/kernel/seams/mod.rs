//! Neutral cross-layer seam contracts (AD-007).
//!
//! This module defines the shared interface traits that lower-level subsystems
//! (scheduler, agent, verification, policy, recovery, planning) implement and
//! that the higher-level AutonomyController consumes.
//!
//! Dependency direction:
//!   kernel::seams  ← depends on: ids, state, planning, context, model, prompt, events
//!   controller     ← depends on: kernel::seams  (downward only)
//!   scheduler      ← depends on: kernel::seams  (downward only)
//!   agent          ← depends on: kernel::seams  (downward only)
//!   verification   ← depends on: kernel::seams  (downward only)
//!   policy         ← depends on: kernel::seams  (downward only)
//!   recovery       ← depends on: kernel::seams  (downward only)
//!
//! No module in kernel::seams imports from controller, scheduler impl,
//! agent impl, verification impl, policy impl, or UI/CLI/TUI/persistence.

pub mod context;
pub mod escalation;
pub mod execution;
pub mod planner;
pub mod policy;
pub mod recovery;
pub mod scheduler;
pub mod verifier;

pub use context::{
    CompiledContext, ContextCompilationContract, ContextCompilationRequest, ContextCompiler,
    ContextError, ContextSectionContract,
};
pub use escalation::{
    EscalationChannel, EscalationError, EscalationRequest, EscalationRequestId, EscalationResponse,
    PendingEscalation,
};
pub use execution::{
    ExecutionError, WorkExecutionHandle, WorkExecutionRequest, WorkExecutionResult,
    WorkerDispatcher,
};
pub use planner::{
    PlanError, PlanRequest, PlanResponse, PlanService, ReplanRequest, ReplanResponse,
};
pub use policy::{
    PolicyDecision, PolicyDecisionContract, PolicyError, PolicyEvaluationRequest, PolicyGate,
    ResolvedAction, resolve_decision,
};
pub use recovery::{
    FailureClassification, FailureClassificationRequest, RecoveryAction, RecoveryEngine,
    RecoveryError, RecoveryStrategyRequest,
};
pub use scheduler::{ReadyWorkResponse, SchedulerError, WorkItem, WorkScheduler};
pub use verifier::{
    CompletionGateOutcome, TaskVerificationRequest, VerificationEngine, VerificationError,
    VerificationOutcome,
};

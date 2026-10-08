//! Agent Runtime Subsystem.
//!
//! Owns specialized bounded agent execution, role profiles, fresh context
//! compilation, worker supervision, handoff arbitration, and completion gating.

pub mod action;
pub mod adaptive;
pub mod delegation;
pub mod dispatcher;
pub mod engine;
pub mod envelope;
pub mod events;
pub mod handoff;
pub mod intent;
pub mod intent_repository;
pub mod model_policy;
pub mod profile;
pub mod registry;
pub mod runner;
pub mod supervisor;

pub use action::{
    ActionExecutionRecord, ActionUserOption, AgentAction, AgentObservation, DiagnosticRecord,
};
pub use adaptive::{
    AdaptiveBudget, AdaptiveReplanOutcome, AdaptiveReplanRequest, AssumptionInvalidationReport,
    DiagnosticEvidence, ExecutionFailureCategory, ExecutionObservation, StallDetector,
    StallEvaluation, StrategyTransitionRecord, SubagentFailureEvidence, render_recovery_context,
    validate_strategy_transition,
};
pub use engine::{AgentEngine, AgentEngineState, AgentTurnOutcome, StructuredToolResult};

pub use crate::state_machine::agent::AgentRole;

pub use dispatcher::ProductionWorkerDispatcher;
pub use envelope::{
    CapabilityEnvelope, CapabilityIntersectionError, calculate_eligible_capabilities,
};
pub use events::*;
pub use handoff::{
    AgentHandoffArbiter, AgentHandoffRepository, DEFAULT_MAX_HANDOFFS_PER_TASK, HandoffError,
    HandoffProposal, HandoffRecord, MAX_DELEGATION_DEPTH,
};
pub use intent::{
    AssumptionInvalidation, DecisionResolution, DecisionStatus, FactOrigin, FormedTask,
    FormedTaskStatus, IntentAssumption, IntentDecision, IntentError, IntentFact, IntentState,
    IntentUnknown, ResearchResult, SteeringConstraint, TaskShape, UnknownResolution,
    deserialize_intent_state, serialize_intent_state,
};
pub use intent_repository::SqliteIntentRepository;
pub use model_policy::{
    DeterministicLifecycleModelCaller, ModelCaller, ModelPolicy, ModelProposal,
    ModelResolutionError, ModelToolCall, ProviderModelCaller, ResolvedModelSelection,
    RoutedModelCaller, StepBudget, StepLimitExceeded, TestModelCaller,
};
pub use profile::{
    AgentProfile, ContextPolicy, ProfileOverride, ProfileOverrideError, TerminationPolicy,
};
pub use registry::{
    DEFAULT_PER_ROLE_CONCURRENCY, InferenceRule, RoleDefinition, RoleError, RoleRegistry,
};
pub use runner::{ActionDispatcher, ActionRequest, ActionResult, AgentStepRecord, WorkerRunner};
pub use supervisor::{AgentOutcome, FailureClass, FailureEvidence, WorkerSupervisor};

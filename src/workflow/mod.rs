//! Workflow domain contracts, manifest parsing, prompt catalog, quality gates, and persistence.
//!
//! # Architecture Invariant
//!
//! Workflow orchestrates above Mission and Task:
//!
//! ```text
//! Workflow
//!    ↓
//! WorkflowStep
//!    ↓
//! Mission
//!    ↓
//! TaskGraph
//!    ↓
//! Scheduler
//!    ↓
//! AgentRuntime
//! ```
//!
//! "The model proposes. The runtime decides."

pub mod compiler;
pub mod definition;
pub mod engine;
pub mod error;
pub mod manifest;
pub mod provenance;
pub mod quality_gate;
pub mod repository;
pub mod state;

// Re-export primary domain identifiers
pub use crate::ids::{ArtifactId, WorkflowRunId, WorkflowStepRunId};

// Re-export definition contracts and validation
pub use definition::{
    InputBinding, OutputBinding, QualityGate, RecoveryStrategy, WorkflowDefinition,
    WorkflowStepDefinition, validate_artifact_path,
};

// Re-export error model
pub use error::WorkflowError;

// Re-export state aggregates and lifecycle state machines
pub use state::{
    WorkflowArtifact, WorkflowArtifactStatus, WorkflowMode, WorkflowRun, WorkflowRunState,
    WorkflowStepRun, WorkflowStepState,
};

// Re-export repository contracts and SQLite implementation
pub use repository::{SqliteWorkflowRepository, WorkflowRepository};

// Re-export manifest structures and parsing
pub use manifest::{
    CURRENT_MANIFEST_VERSION, InputBindingManifest, MAX_MANIFEST_SIZE_BYTES, OutputBindingManifest,
    QualityGateManifest, WorkflowHeaderManifest, WorkflowManifest, WorkflowStepManifest,
    deserialize_flexible_role, parse_role_flexible,
};

// Re-export quality gates and evaluation
pub use quality_gate::{
    QualityGateContext, QualityGateEvaluator, QualityGateResult, QualityGateSeverity,
    QualityGateTier, QualityGateViolation, Tier1QualityGateEvaluator, Tier2QualityGateEvaluator,
};

// Re-export provenance
pub use provenance::{StepProvenance, WorkflowProvenance};

// Re-export compiler
pub use compiler::{CompiledWorkflow, LoweredWorkflow, WorkflowCompiler};

// Re-export workflow engine runtime
pub use engine::{
    ReplanRequest, WorkflowEngine, WorkflowExecutionSnapshot, WorkflowReplanner, WorkflowRunHandle,
    WorkflowStartRequest,
};

// Genesis discovery, environment probing, and research orchestration
pub mod genesis;
pub use genesis::{
    AmbiguityAssessment, BrownfieldMap, CodebaseTopology, Confidence, ConsensusPoint,
    ContradictionResolution, DiscoveryFact, DiscoveryPillar, DiscoverySession, DiscoveryTurn,
    EnvironmentCategory, EnvironmentFact, FactSource, GenesisController, GenesisError, GenesisMode,
    GenesisOptions, GenesisPipelineOutcome, GenesisRequest, OpenUnknown, OperationalInvariants,
    ProjectBoundaries, ProjectCharter, RejectedAlternative, ResearchDecision, ResearchDimension,
    ResearchEvidence, ResearchFinding, ResearchOrchestrator, ResearchSourceType, ResearchSummary,
    ResearchSynthesizer, TechnicalPreferences, TradeoffAnalysis, WorkspaceEnvironment,
    evaluate_research_decision,
};

// Requirements, Architecture, ADRs, and Roadmap planning
pub mod planning;
pub use planning::{
    AdrId, AdrRegistry, ArchitectureComponent, ArchitectureDecisionRecord, ArchitectureDocument,
    ArchitectureInterface, ArchitectureSubsystem, ArchitectureSynthesizer, ComponentStatus,
    DecisionStatus, PlanningChangeSet, PlanningCoordinator, PlanningInvalidator,
    PlanningLifecycleState, PlanningPipelineOutcome, PlanningQualityGates, PlanningState,
    PlanningStateMachine, RequirementsDocument, RequirementsSynthesizer, RiskEntry, RiskRegister,
    RiskStatus, Roadmap, RoadmapDependency, RoadmapPhase, RoadmapPhaseStatus, RoadmapSynthesizer,
    TraceLink, TraceabilityMatrix,
};

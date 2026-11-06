//! `m31a-agent-engine`: Production reactive turn loop with dynamic steering, auto-compaction, and parallel tool dispatch.

pub mod agents;
pub mod delta_batcher;
pub mod dispatcher;
pub mod history_normalizer;
pub mod integration;
pub mod oscillation;
pub mod speculative;
pub mod stream_recovery;
pub mod turn_loop;
pub mod vibe_loop;
pub mod waves;
pub mod workflow;

pub use agents::EngineAgentRuntime;
pub use delta_batcher::DeltaBatcher;
pub use dispatcher::{AuthoritativeToolDispatcher, CodeModeDispatcherBridge, DispatchContext};
pub use history_normalizer::normalize_tool_history;
pub use integration::{
    FileSnapshot, IntegrationOutcome, IntegrationRequest, WorkflowIntegrationService,
};
pub use oscillation::{OscillationAlert, OscillationDetector, OscillationKind};
pub use speculative::{
    CandidateEvaluation, SpeculativeArena, SpeculativeBranchConfig, SpeculativeCandidateState,
    SpeculativeExecutionReport, SpeculativeSandboxGuard,
};
pub use stream_recovery::{StreamOutcome, classify_stream_failure};
pub use turn_loop::{AgentEngine, EngineEvent, PendingApproval};
pub use vibe_loop::{VibeCheckpoint, VibeConfig, VibeEngine, VibeEvent, VibeTimeline};
pub use waves::{
    PhaseWaveResult, PlanExecutionOutcome, WaveExecutionSummary, WaveScheduler,
    WorkflowTaskExecutionOutcome, WorkflowWaveResult, WorkflowWaveSummary,
};
pub use workflow::{MAX_WORKFLOW_REPLAN_REVISIONS, WorkflowExecutionConfig, WorkflowOrchestrator};

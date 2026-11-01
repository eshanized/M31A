//! `m31a-agent-engine`: Production reactive turn loop with dynamic steering, auto-compaction, and parallel tool dispatch.

pub mod agents;
pub mod delta_batcher;
pub mod history_normalizer;
pub mod oscillation;
pub mod speculative;
pub mod stream_recovery;
pub mod turn_loop;
pub mod vibe_loop;
pub mod waves;

pub use agents::EngineAgentRuntime;
pub use delta_batcher::DeltaBatcher;
pub use history_normalizer::normalize_tool_history;
pub use oscillation::{OscillationAlert, OscillationDetector, OscillationKind};
pub use speculative::{
    CandidateEvaluation, SpeculativeArena, SpeculativeBranchConfig, SpeculativeExecutionReport,
    SpeculativeSandboxGuard,
};
pub use stream_recovery::{StreamOutcome, classify_stream_failure};
pub use turn_loop::{AgentEngine, EngineEvent, PendingApproval};
pub use vibe_loop::{VibeCheckpoint, VibeConfig, VibeEngine, VibeEvent, VibeTimeline};
pub use waves::{PhaseWaveResult, PlanExecutionOutcome, WaveExecutionSummary, WaveScheduler};

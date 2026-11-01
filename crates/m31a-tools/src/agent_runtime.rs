//! Dynamic agent runtime injection interface enabling agents to manage child agents and tools.
//!
//! The tool registry exposes `spawn_agent`/`send_input`/`wait_agent`/
//! `close_agent`, but it cannot construct child engines itself: the engine
//! crate depends on this crate, so an engine-backed implementation would be a
//! circular dependency. Instead the registry accepts an injected
//! `AgentRuntime` (host wiring: the CLI/TUI/app-server build the real runtime
//! from `m31a_agent_engine::agents::EngineAgentRuntime`). When no runtime is
//! injected the tools keep their legacy descriptive behavior — a degraded,
//! no-orchestration mode documented in the adoption report.

use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;

pub use m31a_core::types::{AgentRunStatus, TreeTerminationReport};

// A task-local holding the agent id of the engine whose `run_turn` is
// currently executing a tool call. The runtime sets it around each child's
// `run_turn`, so a grandchild spawn can be attributed to its parent and
// depth-limited.
tokio::task_local! {
    pub static CALLER_AGENT_ID: String;
}

/// Returns the calling agent's id, or `None` when the call originates from a
/// root engine (not a spawned child).
pub fn caller_agent_id() -> Option<String> {
    CALLER_AGENT_ID.try_with(|v| v.clone()).ok()
}

/// A spawn request from the `spawn_agent` tool.
#[derive(Debug, Clone)]
pub struct AgentSpawnRequest {
    pub task_name: String,
    pub prompt: String,
    pub role_name: String,
    /// Explicit model override from the tool args (wins over the role's
    /// `model_override`).
    pub model_override: Option<String>,
    /// Depth of this agent in the spawn tree (0 = root, 1 = child, etc.)
    pub depth: u32,
    /// Maximum allowed depth (from config). If exceeded, spawn is rejected.
    pub max_depth: u32,
    /// Whether to isolate this child agent in a dedicated Git worktree branch.
    pub isolate_worktree: bool,
}

/// The immediate response to a successful spawn.
#[derive(Debug, Clone)]
pub struct SpawnedAgent {
    pub agent_id: String,
    pub nickname: String,
    pub model: String,
    pub role: String,
    pub sandbox: String,
    /// Isolated Git worktree directory if created.
    pub worktree_path: Option<PathBuf>,
}

/// Outcome of waiting on one agent.
#[derive(Debug, Clone)]
pub struct AgentWaitOutcome {
    pub agent_id: String,
    /// Running | Completed | Failed | Cancelled | Closed | Unknown
    pub status: String,
    /// Final assistant answer when the agent reached a terminal state.
    pub answer: Option<String>,
}

/// A live agent registered with the runtime.
#[derive(Debug, Clone)]
pub struct LiveAgentInfo {
    pub agent_id: String,
    pub role: String,
    pub task_name: String,
    pub status: String,
}

/// Runtime that actually runs child agents. Implemented by
/// `m31a_agent_engine::agents::EngineAgentRuntime`.
#[async_trait]
pub trait AgentRuntime: Send + Sync {
    /// Spawns a child agent; returns immediately with its identity. The child
    /// runs asynchronously on its own task.
    async fn spawn_agent(&self, req: AgentSpawnRequest) -> Result<SpawnedAgent>;

    /// Delivers a steering message to a running agent.
    async fn send_input(&self, agent_id: &str, message: &str, interrupt: bool) -> Result<String>;

    /// Waits for the given agents to reach a terminal state (or timeout).
    async fn wait_agents(
        &self,
        agent_ids: &[String],
        timeout_ms: Option<u64>,
    ) -> Result<Vec<AgentWaitOutcome>>;

    /// Cancels and closes one agent.
    async fn close_agent(&self, agent_id: &str) -> Result<String>;

    /// Cancels and closes an agent and every descendant in the spawn tree, returning a termination report.
    async fn terminate_tree(&self, agent_id: &str) -> Result<TreeTerminationReport>;

    /// Live agents currently tracked by the runtime.
    async fn list_live(&self) -> Vec<LiveAgentInfo>;

    /// Integrates validated changes from a completed child agent worktree into the parent workspace.
    async fn integrate_agent(
        &self,
        agent_id: &str,
        commit_message: Option<&str>,
    ) -> Result<String> {
        let _ = (agent_id, commit_message);
        Err(anyhow::anyhow!("Integration not supported by this runtime"))
    }

    /// Get the maximum allowed agent depth from config.
    fn max_depth(&self) -> u32 {
        3 // Default, can be overridden by implementations
    }

    /// Returns current status label of an agent if tracked by the runtime.
    async fn get_status(&self, agent_id: &str) -> Option<String> {
        let _ = agent_id;
        None
    }

    /// Returns current in-memory transcript of an agent if tracked by the runtime.
    async fn get_transcript(&self, agent_id: &str) -> Option<Vec<String>> {
        let _ = agent_id;
        None
    }
}

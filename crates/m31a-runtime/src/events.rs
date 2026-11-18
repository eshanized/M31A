//! Presentation-independent runtime events for M31A.
//!
//! Exposes strongly-typed events capturing all execution, security, turn, tool,
//! and lifecycle transitions without coupling to terminal or UI frameworks.

use crate::lifecycle::ShutdownReport;
use m31a_core::types::TokenUsage;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum RuntimeEvent {
    // Lifecycle
    Initializing,
    Ready,
    ShuttingDown,
    ShutdownComplete {
        report: ShutdownReport,
    },
    RuntimeError {
        error: String,
    },

    // Session & Thread
    ThreadCreated {
        thread_id: String,
        title: String,
    },

    // Turn Lifecycle
    TurnSubmitted {
        thread_id: String,
        turn_id: String,
    },
    TurnAdmitted {
        thread_id: String,
        turn_id: String,
    },
    TurnCompleted {
        thread_id: String,
        turn_id: String,
        usage: TokenUsage,
    },
    TurnCancelled {
        thread_id: String,
        turn_id: String,
    },
    TurnFailed {
        thread_id: String,
        turn_id: String,
        error: String,
    },

    // Streaming Deltas
    ReasoningDelta {
        thread_id: String,
        turn_id: String,
        delta: String,
    },
    ContentDelta {
        thread_id: String,
        turn_id: String,
        delta: String,
    },

    // Tool Invocation
    ToolStarted {
        thread_id: String,
        turn_id: String,
        call_id: String,
        tool_name: String,
        arguments: String,
    },
    ToolFinished {
        thread_id: String,
        turn_id: String,
        call_id: String,
        tool_name: String,
        output: String,
        success: bool,
    },

    // Security & Elicitation
    ApprovalRequested {
        thread_id: String,
        turn_id: String,
        request_id: String,
        command: String,
        reason: String,
    },
    QuestionRequested {
        thread_id: String,
        turn_id: String,
        question_id: String,
        question: String,
        options: Vec<String>,
        multi_select: bool,
    },

    // Agents & Workflows
    AgentSpawned {
        parent_thread_id: String,
        child_thread_id: String,
        role: String,
    },
    AgentFinished {
        parent_thread_id: String,
        child_thread_id: String,
        success: bool,
    },
    WorkflowStep {
        workflow_id: String,
        step: usize,
        total_steps: usize,
        message: String,
    },

    // Workspace & Environment
    WorkspaceChanged {
        paths: Vec<PathBuf>,
    },
    ModelChanged {
        old_model: String,
        new_model: String,
    },
    ProviderStatus {
        status: String,
        message: String,
    },
    VerificationResult {
        check_name: String,
        passed: bool,
        output: String,
    },
}

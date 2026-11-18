//! Execution handles, turn contexts, and in-flight elicitation requests.

use m31a_core::CollaborationMode;
use std::path::PathBuf;
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

/// In-flight execution handle for an active agent turn.
#[derive(Clone)]
pub struct ActiveTurnHandle {
    pub thread_id: String,
    pub turn_id: String,
    pub steer_tx: mpsc::UnboundedSender<String>,
    pub cancel_token: CancellationToken,
}

/// Pending tool execution or security approval request awaiting client decision.
#[derive(Debug)]
pub struct PendingApprovalEntry {
    pub thread_id: String,
    pub turn_id: String,
    pub request_id: String,
    pub response_tx: oneshot::Sender<m31a_elicitation::ApprovalDecision>,
}

/// Pending interactive question request awaiting user response.
#[derive(Debug)]
pub struct PendingQuestionEntry {
    pub thread_id: String,
    pub turn_id: String,
    pub question_id: String,
    pub response_tx: oneshot::Sender<Vec<String>>,
}

/// Parameters defining a turn submission to the runtime.
#[derive(Debug, Clone)]
pub struct TurnRequest {
    pub thread_id: String,
    pub prompt: String,
    pub images: Vec<PathBuf>,
    pub collaboration_mode: CollaborationMode,
    pub ephemeral: bool,
    pub self_heal: bool,
    pub max_heal_iterations: usize,
    pub max_steps: usize,
}

impl TurnRequest {
    pub fn new(thread_id: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            thread_id: thread_id.into(),
            prompt: prompt.into(),
            images: Vec::new(),
            collaboration_mode: CollaborationMode::Default,
            ephemeral: false,
            self_heal: false,
            max_heal_iterations: 3,
            max_steps: 20,
        }
    }

    pub fn with_plan_mode(mut self, plan: bool) -> Self {
        if plan {
            self.collaboration_mode = CollaborationMode::Plan;
        }
        self
    }

    pub fn with_ephemeral(mut self, ephemeral: bool) -> Self {
        self.ephemeral = ephemeral;
        self
    }

    pub fn with_self_heal(mut self, self_heal: bool, max_iterations: usize) -> Self {
        self.self_heal = self_heal;
        self.max_heal_iterations = max_iterations;
        self
    }
}

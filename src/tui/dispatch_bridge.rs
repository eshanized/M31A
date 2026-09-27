//! Runtime Dispatch Bridge (COP-04, D-06, T-13-10).
//!
//! Enforces runtime boundary invariants: TUI views submit strongly typed
//! commands to the kernel channel rather than mutating SQLite directly.

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::mpsc;

use crate::cli::dispatch::RuntimeCommand;

/// Structured request payload for authoring and launching a mission.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateMissionRequest {
    pub objective: String,
    pub constraints: Vec<String>,
    pub acceptance_criteria: Vec<String>,
    pub profile: String,
    pub autonomy_mode: String,
    pub max_budget_usd: f64,
    pub target_branch: String,
}

impl CreateMissionRequest {
    /// Validate mission parameters before submission.
    pub fn validate(&self) -> Result<(), DispatchError> {
        if self.objective.trim().is_empty() {
            return Err(DispatchError::ValidationError(
                "Mission objective cannot be empty".to_string(),
            ));
        }

        if self.max_budget_usd <= 0.0 {
            return Err(DispatchError::ValidationError(
                "Budget ceiling must be greater than $0.00".to_string(),
            ));
        }

        if self.target_branch.trim().is_empty() {
            return Err(DispatchError::ValidationError(
                "Target git branch cannot be empty".to_string(),
            ));
        }

        Ok(())
    }
}

/// Errors originating from dispatching runtime commands.
#[derive(Debug, Error)]
pub enum DispatchError {
    #[error("Validation failed: {0}")]
    ValidationError(String),

    #[error("Runtime channel closed; kernel is not listening")]
    ChannelClosed,

    #[error("Dispatch failed: {0}")]
    SendFailed(String),
}

/// Dispatch bridge communicating between TUI event loop and runtime scheduler.
#[derive(Debug, Clone)]
pub struct DispatchBridge {
    sender: mpsc::Sender<RuntimeCommand>,
}

impl DispatchBridge {
    /// Create new dispatch bridge over a tokio mpsc channel.
    pub fn new(sender: mpsc::Sender<RuntimeCommand>) -> Self {
        Self { sender }
    }

    /// Submit a mission creation request asynchronously to the runtime kernel.
    pub async fn dispatch_mission(&self, req: CreateMissionRequest) -> Result<(), DispatchError> {
        req.validate()?;

        let cmd = RuntimeCommand::RunMission {
            prompt: req.objective,
            profile: Some(req.profile),
            wait_for_approval: req.autonomy_mode == "Supervised",
        };

        self.sender
            .send(cmd)
            .await
            .map_err(|e| DispatchError::SendFailed(e.to_string()))?;

        Ok(())
    }

    /// Non-blocking try_dispatch for synchronous UI loop handling.
    pub fn try_dispatch_mission(&self, req: CreateMissionRequest) -> Result<(), DispatchError> {
        req.validate()?;

        let cmd = RuntimeCommand::RunMission {
            prompt: req.objective,
            profile: Some(req.profile),
            wait_for_approval: req.autonomy_mode == "Supervised",
        };

        self.sender.try_send(cmd).map_err(|e| match e {
            mpsc::error::TrySendError::Closed(_) => DispatchError::ChannelClosed,
            mpsc::error::TrySendError::Full(_) => {
                DispatchError::SendFailed("Command channel queue is full".to_string())
            }
        })?;

        Ok(())
    }

    /// Dispatch any arbitrary RuntimeCommand through the channel.
    pub fn try_dispatch_command(&self, cmd: RuntimeCommand) -> Result<(), DispatchError> {
        self.sender.try_send(cmd).map_err(|e| match e {
            mpsc::error::TrySendError::Closed(_) => DispatchError::ChannelClosed,
            mpsc::error::TrySendError::Full(_) => {
                DispatchError::SendFailed("Command channel queue is full".to_string())
            }
        })?;
        Ok(())
    }
}

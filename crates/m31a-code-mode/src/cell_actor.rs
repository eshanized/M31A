//! Sovereign interactive cell actor and lifecycle state machine.
//!
//! Manages stateful execution cells, turn step yields, and lifecycle transitions for Code Mode scripts.
//! Manages stateful execution cells for Code Mode:
//! - Lifecycle transitions (`Pending`, `Running`, `SuspendedYield`, `Completed`, `Failed`, `Terminated`).
//! - Cooperative yield-and-resume workflows allowing scripts to suspend, emit intermediate
//!   results, wait for user elicitation or async tools, and resume with persistent state.
//! - Strict safety budgets for execution duration, tool call quantities, and output sizes.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum CellActorError {
    #[error("Invalid state transition from {from:?} to {to:?}")]
    InvalidTransition {
        from: CellLifecycleState,
        to: CellLifecycleState,
    },
    #[error("Cell exceeded maximum tool call budget ({limit} calls)")]
    ToolCallBudgetExceeded { limit: usize },
    #[error("Cell exceeded maximum execution time limit ({limit_ms}ms)")]
    ExecutionTimeout { limit_ms: u64 },
    #[error("Cannot resume cell in state {0:?}")]
    NotResumable(CellLifecycleState),
}

/// Lifecycle states of a Code Mode execution cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CellLifecycleState {
    /// Newly spawned, awaiting initialization.
    Pending,
    /// Actively evaluating instructions or running isolate.
    Running,
    /// Suspended cooperatively awaiting input, tool completion, or user response.
    SuspendedYield,
    /// Successfully finished execution.
    Completed,
    /// Terminated due to unhandled runtime exception or budget violation.
    Failed,
    /// Explicitly cancelled or killed by supervisor.
    Terminated,
}

/// Reason why an active cell yielded execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CellYieldReason {
    /// Explicit script call to yield waiting for user feedback or approval.
    AwaitingUserInput { prompt: String },
    /// Cooperative timeslice yield allowing the agent event loop to turn.
    CooperativeStep { step_index: usize },
    /// Waiting for background async process or external tool response.
    AsyncToolWait { call_id: String },
}

/// Safety constraints enforced on each cell actor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellExecutionBudget {
    /// Maximum wall-clock execution time allowed for the cell in milliseconds.
    pub max_execution_time_ms: u64,
    /// Maximum number of tool calls permitted within a single script cell.
    pub max_tool_calls: usize,
    /// Maximum output characters allowed before truncation.
    pub max_output_chars: usize,
}

impl Default for CellExecutionBudget {
    fn default() -> Self {
        Self {
            max_execution_time_ms: 15_000, // 15 seconds
            max_tool_calls: 50,
            max_output_chars: 50_000,
        }
    }
}

/// Immutable snapshot of a cell's state at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CellCheckpoint {
    pub cell_id: Uuid,
    pub turn_index: usize,
    pub state: CellLifecycleState,
    pub accumulated_output: String,
    pub variables: HashMap<String, serde_json::Value>,
    pub tool_calls_executed: usize,
    pub started_at: DateTime<Utc>,
    pub last_yielded_at: Option<DateTime<Utc>>,
}

/// Stateful actor supervising a Code Mode script cell.
pub struct CellActor {
    cell_id: Uuid,
    turn_index: usize,
    state: CellLifecycleState,
    budget: CellExecutionBudget,
    accumulated_output: String,
    variables: HashMap<String, serde_json::Value>,
    tool_calls_executed: usize,
    started_at: DateTime<Utc>,
    last_yielded_at: Option<DateTime<Utc>>,
    yield_reason: Option<CellYieldReason>,
    failure_message: Option<String>,
}

impl CellActor {
    pub fn new(turn_index: usize, budget: CellExecutionBudget) -> Self {
        Self {
            cell_id: Uuid::new_v4(),
            turn_index,
            state: CellLifecycleState::Pending,
            budget,
            accumulated_output: String::new(),
            variables: HashMap::new(),
            tool_calls_executed: 0,
            started_at: Utc::now(),
            last_yielded_at: None,
            yield_reason: None,
            failure_message: None,
        }
    }

    pub fn cell_id(&self) -> Uuid {
        self.cell_id
    }

    pub fn state(&self) -> CellLifecycleState {
        self.state
    }

    pub fn accumulated_output(&self) -> &str {
        &self.accumulated_output
    }

    pub fn yield_reason(&self) -> Option<&CellYieldReason> {
        self.yield_reason.as_ref()
    }

    pub fn failure_message(&self) -> Option<&str> {
        self.failure_message.as_deref()
    }

    pub fn is_resumable(&self) -> bool {
        self.state == CellLifecycleState::SuspendedYield
    }

    /// Transition to running state to begin executing script.
    pub fn start(&mut self) -> Result<(), CellActorError> {
        match self.state {
            CellLifecycleState::Pending => {
                self.state = CellLifecycleState::Running;
                self.started_at = Utc::now();
                Ok(())
            }
            other => Err(CellActorError::InvalidTransition {
                from: other,
                to: CellLifecycleState::Running,
            }),
        }
    }

    /// Record a tool invocation, verifying tool call budget limits.
    pub fn record_tool_call(&mut self) -> Result<(), CellActorError> {
        if self.tool_calls_executed >= self.budget.max_tool_calls {
            self.fail(format!(
                "Exceeded maximum tool call limit of {}",
                self.budget.max_tool_calls
            ));
            return Err(CellActorError::ToolCallBudgetExceeded {
                limit: self.budget.max_tool_calls,
            });
        }
        self.tool_calls_executed += 1;
        Ok(())
    }

    /// Append stdout/stderr log output, enforcing output budget.
    pub fn append_output(&mut self, text: &str) {
        if self.accumulated_output.len() + text.len() > self.budget.max_output_chars {
            let remaining = self
                .budget
                .max_output_chars
                .saturating_sub(self.accumulated_output.len());
            if remaining > 0 {
                self.accumulated_output.push_str(&text[..remaining]);
            }
            self.accumulated_output
                .push_str("\n[... Cell output budget reached ...]");
        } else {
            self.accumulated_output.push_str(text);
        }
    }

    /// Store a persistent variable in the cell's KV memory.
    pub fn set_variable(&mut self, key: impl Into<String>, value: serde_json::Value) {
        self.variables.insert(key.into(), value);
    }

    /// Retrieve a variable from the cell's KV memory.
    pub fn get_variable(&self, key: &str) -> Option<&serde_json::Value> {
        self.variables.get(key)
    }

    /// Suspend cell execution and yield back to agent loop.
    pub fn yield_execution(
        &mut self,
        reason: CellYieldReason,
        intermediate_output: &str,
    ) -> Result<(), CellActorError> {
        if self.state != CellLifecycleState::Running {
            return Err(CellActorError::InvalidTransition {
                from: self.state,
                to: CellLifecycleState::SuspendedYield,
            });
        }

        if !intermediate_output.is_empty() {
            self.append_output(intermediate_output);
        }

        self.state = CellLifecycleState::SuspendedYield;
        self.last_yielded_at = Some(Utc::now());
        self.yield_reason = Some(reason);
        Ok(())
    }

    /// Resume execution of a previously suspended cell.
    pub fn resume(&mut self, resume_message: Option<&str>) -> Result<(), CellActorError> {
        if self.state != CellLifecycleState::SuspendedYield {
            return Err(CellActorError::NotResumable(self.state));
        }

        if let Some(msg) = resume_message {
            self.append_output(&format!("\n[Resumed with input: {msg}]\n"));
        }

        self.state = CellLifecycleState::Running;
        self.yield_reason = None;
        Ok(())
    }

    /// Mark cell execution as successfully completed.
    pub fn complete(&mut self, final_output: &str) -> Result<(), CellActorError> {
        if self.state != CellLifecycleState::Running
            && self.state != CellLifecycleState::SuspendedYield
        {
            return Err(CellActorError::InvalidTransition {
                from: self.state,
                to: CellLifecycleState::Completed,
            });
        }

        if !final_output.is_empty() {
            self.append_output(final_output);
        }

        self.state = CellLifecycleState::Completed;
        Ok(())
    }

    /// Mark cell execution as failed.
    pub fn fail(&mut self, error_message: impl Into<String>) {
        let msg = error_message.into();
        self.append_output(&format!("\n[Cell Error]: {msg}\n"));
        self.failure_message = Some(msg);
        self.state = CellLifecycleState::Failed;
    }

    /// Terminate cell execution immediately.
    pub fn terminate(&mut self) {
        self.state = CellLifecycleState::Terminated;
    }

    /// Create an immutable checkpoint representation of the cell.
    pub fn checkpoint(&self) -> CellCheckpoint {
        CellCheckpoint {
            cell_id: self.cell_id,
            turn_index: self.turn_index,
            state: self.state,
            accumulated_output: self.accumulated_output.clone(),
            variables: self.variables.clone(),
            tool_calls_executed: self.tool_calls_executed,
            started_at: self.started_at,
            last_yielded_at: self.last_yielded_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cell_lifecycle_start_and_complete() {
        let mut actor = CellActor::new(1, CellExecutionBudget::default());
        assert_eq!(actor.state(), CellLifecycleState::Pending);

        actor.start().expect("start success");
        assert_eq!(actor.state(), CellLifecycleState::Running);

        actor.append_output("Processing data...\n");
        actor.complete("Done.").expect("complete success");
        assert_eq!(actor.state(), CellLifecycleState::Completed);
        assert!(actor.accumulated_output().contains("Processing data"));
        assert!(actor.accumulated_output().contains("Done."));
    }

    #[test]
    fn test_cell_yield_and_resume_flow() {
        let mut actor = CellActor::new(1, CellExecutionBudget::default());
        actor.start().unwrap();

        actor.set_variable("counter", serde_json::json!(42));

        let yield_reason = CellYieldReason::AwaitingUserInput {
            prompt: "Please approve deployment".to_string(),
        };
        actor
            .yield_execution(yield_reason, "Suspended at step 1")
            .unwrap();
        assert_eq!(actor.state(), CellLifecycleState::SuspendedYield);
        assert!(actor.is_resumable());

        actor.resume(Some("Approved by user")).unwrap();
        assert_eq!(actor.state(), CellLifecycleState::Running);
        assert!(!actor.is_resumable());
        assert_eq!(
            actor.get_variable("counter").unwrap(),
            &serde_json::json!(42)
        );

        actor.complete("All steps finished").unwrap();
        assert_eq!(actor.state(), CellLifecycleState::Completed);
    }

    #[test]
    fn test_tool_call_budget_enforcement() {
        let budget = CellExecutionBudget {
            max_execution_time_ms: 10_000,
            max_tool_calls: 3,
            max_output_chars: 1_000,
        };
        let mut actor = CellActor::new(1, budget);
        actor.start().unwrap();

        assert!(actor.record_tool_call().is_ok());
        assert!(actor.record_tool_call().is_ok());
        assert!(actor.record_tool_call().is_ok());

        // 4th call breaches budget of 3!
        let err = actor.record_tool_call().unwrap_err();
        assert!(matches!(
            err,
            CellActorError::ToolCallBudgetExceeded { limit: 3 }
        ));
        assert_eq!(actor.state(), CellLifecycleState::Failed);
        assert!(
            actor
                .failure_message()
                .unwrap()
                .contains("Exceeded maximum tool call limit")
        );
    }

    #[test]
    fn test_checkpoint_generation() {
        let mut actor = CellActor::new(5, CellExecutionBudget::default());
        actor.start().unwrap();
        actor.set_variable("status", serde_json::json!("in_progress"));
        actor.append_output("step 1 ok");

        let cp = actor.checkpoint();
        assert_eq!(cp.cell_id, actor.cell_id());
        assert_eq!(cp.turn_index, 5);
        assert_eq!(cp.state, CellLifecycleState::Running);
        assert_eq!(cp.variables.get("status").unwrap(), "in_progress");
        assert_eq!(cp.accumulated_output, "step 1 ok");
    }
}

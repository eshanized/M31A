//! Supervised async worker lifecycle, panic containment, and dual timeouts (AGT-05, D-05, D-07, D-08).
//!
//! Owns async worker execution inside Tokio tasks, guaranteeing that worker thread panics
//! are converted into structured failure evidence without crashing the M31A process,
//! and that cancellation and timeouts await confirmed worker termination (Law 8).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use crate::ids::{AgentId, TaskId};

/// Structured classification of worker execution failures (D-08).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureClass {
    WorkerPanic,
    StallTimeout,
    WallClockTimeout,
    PolicyDenial,
    ToolFailure,
    ModelError,
    ContextBudgetExceeded,
}

/// Structured evidence capturing failure context for recovery and audit (D-08).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailureEvidence {
    pub failure_class: FailureClass,
    pub message: String,
    pub step_number: u32,
    pub occurred_at: DateTime<Utc>,
    pub is_panic: bool,
    pub diagnostics: HashMap<String, String>,
}

/// Strongly typed terminal outcome of an agent execution (D-08).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum AgentOutcome {
    Succeeded {
        output: String,
        steps_consumed: u32,
    },
    Cancelled {
        reason: String,
        steps_consumed: u32,
    },
    TimedOut {
        deadline_secs: u64,
        steps_consumed: u32,
    },
    Stalled {
        last_progress_secs_ago: u64,
        steps_consumed: u32,
    },
    StepLimitExceeded {
        limit: u32,
        consumed: u32,
    },
    Failed(FailureEvidence),
}

impl AgentOutcome {
    /// check if the outcome represents success.
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Succeeded { .. })
    }
}

/// Execution state tracked by the inference-aware supervisor (P3-F).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionActivity {
    Idle {
        last_progress_at: DateTime<Utc>,
    },
    InferenceActive {
        started_at: DateTime<Utc>,
    },
    ToolActive {
        tool_name: String,
        started_at: DateTime<Utc>,
    },
}

/// Thread-safe execution activity tracker bridging worker progress and supervisor watchdog.
#[derive(Debug)]
pub struct ExecutionActivityTracker {
    activity: RwLock<ExecutionActivity>,
    last_progress_at: Arc<RwLock<DateTime<Utc>>>,
    accumulated_usage: Arc<RwLock<crate::model::types::TokenUsage>>,
}

impl Default for ExecutionActivityTracker {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            activity: RwLock::new(ExecutionActivity::Idle {
                last_progress_at: now,
            }),
            last_progress_at: Arc::new(RwLock::new(now)),
            accumulated_usage: Arc::new(RwLock::new(crate::model::types::TokenUsage::default())),
        }
    }
}

impl ExecutionActivityTracker {
    pub fn new(last_progress_at: Arc<RwLock<DateTime<Utc>>>) -> Self {
        let now = Utc::now();
        Self {
            activity: RwLock::new(ExecutionActivity::Idle {
                last_progress_at: now,
            }),
            last_progress_at,
            accumulated_usage: Arc::new(RwLock::new(crate::model::types::TokenUsage::default())),
        }
    }

    pub async fn mark_idle(&self) {
        let now = Utc::now();
        *self.activity.write().await = ExecutionActivity::Idle {
            last_progress_at: now,
        };
        *self.last_progress_at.write().await = now;
    }

    pub async fn mark_inference_start(&self) {
        let now = Utc::now();
        *self.activity.write().await = ExecutionActivity::InferenceActive { started_at: now };
        *self.last_progress_at.write().await = now;
    }

    pub async fn mark_tool_start(&self, tool_name: &str) {
        let now = Utc::now();
        *self.activity.write().await = ExecutionActivity::ToolActive {
            tool_name: tool_name.to_string(),
            started_at: now,
        };
        *self.last_progress_at.write().await = now;
    }

    pub async fn touch(&self) {
        let now = Utc::now();
        *self.last_progress_at.write().await = now;
    }

    pub async fn record_token_usage(&self, usage: &crate::model::types::TokenUsage) {
        let mut guard = self.accumulated_usage.write().await;
        guard.accumulate(usage);
    }

    pub async fn total_token_usage(&self) -> crate::model::types::TokenUsage {
        self.accumulated_usage.read().await.clone()
    }

    pub async fn current_activity(&self) -> ExecutionActivity {
        self.activity.read().await.clone()
    }

    pub fn last_progress_handle(&self) -> Arc<RwLock<DateTime<Utc>>> {
        Arc::clone(&self.last_progress_at)
    }
}

/// Async worker supervisor enforcing panic containment, progress tracking, and dual timeouts (D-05, D-07).
pub struct WorkerSupervisor {
    pub agent_id: AgentId,
    pub task_id: TaskId,
    pub cancellation_token: CancellationToken,
    pub last_progress_at: Arc<RwLock<DateTime<Utc>>>,
    pub activity_tracker: Arc<ExecutionActivityTracker>,
    pub stall_timeout: Duration,
    pub wall_clock_timeout: Duration,
    pub grace_period: Duration,
}

impl WorkerSupervisor {
    /// Create a new worker supervisor with specified timeouts and cancellation token.
    pub fn new(
        agent_id: AgentId,
        task_id: TaskId,
        cancellation_token: CancellationToken,
        stall_timeout: Duration,
        wall_clock_timeout: Duration,
        grace_period: Duration,
    ) -> Self {
        let last_progress_at = Arc::new(RwLock::new(Utc::now()));
        let activity_tracker =
            Arc::new(ExecutionActivityTracker::new(Arc::clone(&last_progress_at)));
        Self {
            agent_id,
            task_id,
            cancellation_token,
            last_progress_at,
            activity_tracker,
            stall_timeout,
            wall_clock_timeout,
            grace_period,
        }
    }

    /// Obtain a shared handle to the progress tracker for updating timestamps during execution.
    pub fn progress_tracker(&self) -> Arc<RwLock<DateTime<Utc>>> {
        Arc::clone(&self.last_progress_at)
    }

    /// Obtain a shared handle to the execution activity tracker (P3-F).
    pub fn activity_tracker(&self) -> Arc<ExecutionActivityTracker> {
        Arc::clone(&self.activity_tracker)
    }

    /// Record progress at the current timestamp.
    pub async fn touch_progress(&self) {
        let mut guard = self.last_progress_at.write().await;
        *guard = Utc::now();
    }

    /// Execute an async worker closure under full supervision:
    /// 1. Catches worker panics and converts JoinError into AgentOutcome::Failed (T-06-04).
    /// 2. Enforces per-step stall detection and wall-clock deadline timeouts (D-07).
    /// 3. Executes cooperative cancellation escalation with confirmed task termination (T-06-05, Law 8).
    pub async fn run_supervised<F>(&self, worker_fn: F) -> AgentOutcome
    where
        F: Future<Output = AgentOutcome> + Send + 'static,
    {
        // Reset progress marker to start of supervision
        self.touch_progress().await;

        let mut worker_handle = tokio::spawn(worker_fn);
        let token = self.cancellation_token.clone();
        let activity_tracker = Arc::clone(&self.activity_tracker);
        let stall_timeout = self.stall_timeout;
        let wall_clock_timeout = self.wall_clock_timeout;
        let grace_period = self.grace_period;

        let stall_check_interval = if stall_timeout < Duration::from_millis(200) {
            Duration::from_millis(20)
        } else {
            (stall_timeout / 4).clamp(Duration::from_millis(50), Duration::from_secs(1))
        };

        let wall_clock_deadline = tokio::time::sleep(wall_clock_timeout);
        tokio::pin!(wall_clock_deadline);

        loop {
            tokio::select! {
                // 1. Worker task completed (or panicked)
                join_res = &mut worker_handle => {
                    match join_res {
                        Ok(outcome) => return outcome,
                        Err(join_err) => {
                            if join_err.is_panic() {
                                let panic_msg = if let Ok(panic_str) = join_err.into_panic().downcast::<String>() {
                                    *panic_str
                                } else {
                                    "worker thread panicked with non-string payload".to_string()
                                };
                                return AgentOutcome::Failed(FailureEvidence {
                                    failure_class: FailureClass::WorkerPanic,
                                    message: format!("isolated worker panic: {}", panic_msg),
                                    step_number: 0,
                                    occurred_at: Utc::now(),
                                    is_panic: true,
                                    diagnostics: HashMap::new(),
                                });
                            } else if join_err.is_cancelled() {
                                return AgentOutcome::Cancelled {
                                    reason: "worker task aborted".to_string(),
                                    steps_consumed: 0,
                                };
                            } else {
                                return AgentOutcome::Failed(FailureEvidence {
                                    failure_class: FailureClass::ToolFailure,
                                    message: format!("worker join error: {}", join_err),
                                    step_number: 0,
                                    occurred_at: Utc::now(),
                                    is_panic: false,
                                    diagnostics: HashMap::new(),
                                });
                            }
                        }
                    }
                }

                // 2. Cooperative cancellation requested from parent/external caller
                _ = token.cancelled() => {
                    // Give worker task a bounded grace period to exit cooperatively
                    let wait_grace = tokio::time::timeout(grace_period, &mut worker_handle).await;
                    if wait_grace.is_err() {
                        // Escalation: abort handle and await confirmed exit
                        worker_handle.abort();
                        let _ = worker_handle.await;
                    }
                    return AgentOutcome::Cancelled {
                        reason: "cancellation requested".to_string(),
                        steps_consumed: 0,
                    };
                }

                // 3. Wall-clock overall deadline reached
                () = &mut wall_clock_deadline => {
                    token.cancel();
                    let wait_grace = tokio::time::timeout(grace_period, &mut worker_handle).await;
                    if wait_grace.is_err() {
                        worker_handle.abort();
                        let _ = worker_handle.await;
                    }
                    return AgentOutcome::TimedOut {
                        deadline_secs: wall_clock_timeout.as_secs(),
                        steps_consumed: 0,
                    };
                }

                // 4. Periodic stall detector checking activity state and duration (P3-F)
                _ = tokio::time::sleep(stall_check_interval) => {
                    let activity = activity_tracker.current_activity().await;
                    let now = Utc::now();
                    let stall_chrono = match chrono::Duration::from_std(stall_timeout) {
                        Ok(d) => d,
                        Err(_) => chrono::TimeDelta::MAX,
                    };

                    let is_stalled = match activity {
                        ExecutionActivity::Idle { last_progress_at } => {
                            let elapsed = now.signed_duration_since(last_progress_at);
                            if elapsed > stall_chrono {
                                Some(elapsed)
                            } else {
                                None
                            }
                        }
                        ExecutionActivity::InferenceActive { started_at } => {
                            let elapsed = now.signed_duration_since(started_at);
                            // Remote model inference watchdog: 180s threshold (accommodating remote NIM queuing and generation)
                            let inf_limit = chrono::Duration::seconds(180).max(stall_chrono * 3);
                            if elapsed > inf_limit {
                                tracing::warn!(
                                    elapsed_secs = elapsed.num_seconds(),
                                    "Inference watchdog timeout exceeded (180s limit)"
                                );
                                Some(elapsed)
                            } else {
                                None
                            }
                        }
                        ExecutionActivity::ToolActive { ref tool_name, started_at } => {
                            let elapsed = now.signed_duration_since(started_at);
                            // Tool execution watchdog: 180s threshold
                            let tool_limit = chrono::Duration::seconds(180).max(stall_chrono * 3);
                            if elapsed > tool_limit {
                                tracing::warn!(
                                    tool = %tool_name,
                                    elapsed_secs = elapsed.num_seconds(),
                                    "Tool execution watchdog timeout exceeded (180s limit)"
                                );
                                Some(elapsed)
                            } else {
                                None
                            }
                        }
                    };

                    if let Some(elapsed) = is_stalled {
                        token.cancel();
                        let wait_grace = tokio::time::timeout(grace_period, &mut worker_handle).await;
                        if wait_grace.is_err() {
                            worker_handle.abort();
                            let _ = worker_handle.await;
                        }
                        return AgentOutcome::Stalled {
                            last_progress_secs_ago: elapsed.num_seconds().max(0) as u64,
                            steps_consumed: 0,
                        };
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_supervisor_normal_success() {
        let supervisor = WorkerSupervisor::new(
            AgentId::new(),
            TaskId::new(),
            CancellationToken::new(),
            Duration::from_secs(5),
            Duration::from_secs(10),
            Duration::from_millis(100),
        );

        let outcome = supervisor
            .run_supervised(async {
                AgentOutcome::Succeeded {
                    output: "done".to_string(),
                    steps_consumed: 2,
                }
            })
            .await;

        assert_eq!(
            outcome,
            AgentOutcome::Succeeded {
                output: "done".to_string(),
                steps_consumed: 2,
            }
        );
    }

    #[tokio::test]
    async fn test_supervisor_panic_containment() {
        let supervisor = WorkerSupervisor::new(
            AgentId::new(),
            TaskId::new(),
            CancellationToken::new(),
            Duration::from_secs(5),
            Duration::from_secs(10),
            Duration::from_millis(100),
        );

        let outcome = supervisor
            .run_supervised(async {
                panic!("critical worker crash");
            })
            .await;

        match outcome {
            AgentOutcome::Failed(evidence) => {
                assert_eq!(evidence.failure_class, FailureClass::WorkerPanic);
                assert!(evidence.is_panic);
                assert!(evidence.message.contains("isolated worker panic"));
            }
            other => panic!("expected FailureOutcome with WorkerPanic, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_supervisor_cooperative_cancellation() {
        let token = CancellationToken::new();
        let supervisor = WorkerSupervisor::new(
            AgentId::new(),
            TaskId::new(),
            token.clone(),
            Duration::from_secs(5),
            Duration::from_secs(10),
            Duration::from_millis(50),
        );

        let worker_token = token.clone();
        let outcome_fut = supervisor.run_supervised(async move {
            loop {
                if worker_token.is_cancelled() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            AgentOutcome::Cancelled {
                reason: "worker acknowledged token".to_string(),
                steps_consumed: 1,
            }
        });

        // Cancel after 20ms
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(20)).await;
            token.cancel();
        });

        let outcome = outcome_fut.await;
        assert!(matches!(outcome, AgentOutcome::Cancelled { .. }));
    }

    #[tokio::test]
    async fn test_supervisor_stall_detection() {
        let supervisor = WorkerSupervisor::new(
            AgentId::new(),
            TaskId::new(),
            CancellationToken::new(),
            Duration::from_millis(60),
            Duration::from_secs(5),
            Duration::from_millis(20),
        );

        // Worker sleeps indefinitely without touching progress
        let outcome = supervisor
            .run_supervised(async {
                tokio::time::sleep(Duration::from_secs(2)).await;
                AgentOutcome::Succeeded {
                    output: "late".to_string(),
                    steps_consumed: 1,
                }
            })
            .await;

        assert!(matches!(outcome, AgentOutcome::Stalled { .. }));
    }

    #[tokio::test]
    async fn test_supervisor_wall_clock_timeout() {
        let supervisor = WorkerSupervisor::new(
            AgentId::new(),
            TaskId::new(),
            CancellationToken::new(),
            Duration::from_secs(10),   // long stall timeout
            Duration::from_millis(60), // short wall clock deadline
            Duration::from_millis(20),
        );

        let tracker = supervisor.progress_tracker();
        // Worker actively touches progress to prevent stall, but runs past wall clock deadline
        let outcome = supervisor
            .run_supervised(async move {
                for _ in 0..10 {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    *tracker.write().await = Utc::now();
                }
                AgentOutcome::Succeeded {
                    output: "finished".to_string(),
                    steps_consumed: 10,
                }
            })
            .await;

        assert!(matches!(outcome, AgentOutcome::TimedOut { .. }));
    }

    #[tokio::test]
    async fn test_supervisor_inference_active_tolerates_slow_inference() {
        let supervisor = WorkerSupervisor::new(
            AgentId::new(),
            TaskId::new(),
            CancellationToken::new(),
            Duration::from_millis(50), // short 50ms stall timeout for idle
            Duration::from_secs(5),    // long wall clock
            Duration::from_millis(20),
        );

        let activity = supervisor.activity_tracker();
        // Worker marks inference active and runs longer than stall timeout (100ms > 50ms)
        let outcome = supervisor
            .run_supervised(async move {
                activity.mark_inference_start().await;
                tokio::time::sleep(Duration::from_millis(100)).await;
                activity.mark_idle().await;
                AgentOutcome::Succeeded {
                    output: "model finished".to_string(),
                    steps_consumed: 1,
                }
            })
            .await;

        // Must succeed because InferenceActive is granted higher watchdog limit
        assert_eq!(
            outcome,
            AgentOutcome::Succeeded {
                output: "model finished".to_string(),
                steps_consumed: 1,
            }
        );
    }
}

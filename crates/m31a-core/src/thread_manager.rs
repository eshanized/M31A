//! Thread manager, submission state machine, and agent lifecycle coordinator.
//!
//! Provides the core runtime coordination for autonomous agent threads:
//! - Multi-thread lifecycle tracking (`Idle`, `RunningTurn`, `AwaitingInput`, `Suspended`, `Closed`).
//! - Out-of-band steering (`SteerSubmission`) into active turns without restarting execution.
//! - Idle-safe turn submissions (`StartIfIdleSubmission`).
//! - Turn input requests, approval resumes, and cancellation tokens.
//! - Recursive agent tree shutdown reports (`AgentTreeShutdownReport`).

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Unique identifier for an agent conversation thread.
pub type ThreadId = String;

/// Unique identifier for a single execution turn within a thread.
pub type TurnId = String;

/// Thread execution status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThreadLifecycleStatus {
    /// Thread is open and ready to accept new turns.
    Idle,
    /// Thread is actively running a turn.
    RunningTurn { turn_id: TurnId, started_at: i64 },
    /// Thread is paused waiting for user input, clarification, or tool approval.
    AwaitingInput { turn_id: TurnId, prompt: String },
    /// Thread execution is suspended (can be resumed).
    Suspended { turn_id: TurnId, reason: String },
    /// Thread has concluded and will accept no further turns.
    Closed { closed_at: i64 },
}

/// Startup options when spawning a new agent thread.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThreadSpawnOptions {
    pub thread_id: Option<ThreadId>,
    pub parent_thread_id: Option<ThreadId>,
    pub model: String,
    pub role: Option<String>,
    pub working_dir: Option<String>,
    pub extra_env: HashMap<String, String>,
    pub initial_instructions: Option<String>,
}

impl Default for ThreadSpawnOptions {
    fn default() -> Self {
        Self {
            thread_id: None,
            parent_thread_id: None,
            model: "meta/llama-3.3-70b-instruct".to_string(),
            role: None,
            working_dir: None,
            extra_env: HashMap::new(),
            initial_instructions: None,
        }
    }
}

/// Turn input payload submitted to a thread.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TurnInput {
    /// Standard user message initiating or continuing dialogue.
    UserMessage(String),
    /// Mid-turn steering instruction to redirect active planning.
    Steer(String),
    /// Decision answering a pending tool execution approval.
    ToolApprovalResponse {
        approved: bool,
        reason: Option<String>,
    },
    /// Answer to an interactive user clarification/elicitation prompt.
    ElicitationAnswer { answer: String },
    /// Resume a previously suspended turn.
    Resume,
}

/// Result of attempting to submit an input to a thread.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TurnSubmissionOutcome {
    /// Turn was successfully launched.
    Started { turn_id: TurnId },
    /// Steering directive successfully delivered to running turn.
    Steered { turn_id: TurnId },
    /// Thread was not idle and could not accept a new top-level turn.
    RejectedNotIdle { current_status: String },
    /// Thread is closed.
    RejectedThreadClosed,
    /// Pending approval or clarification resolved; turn resumed.
    Resumed { turn_id: TurnId },
}

/// Summary report when shutting down an individual thread.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreadShutdownReport {
    pub thread_id: ThreadId,
    pub was_running_turn: Option<TurnId>,
    pub closed_at: i64,
}

/// Hierarchical report when shutting down a root agent and all spawned descendants.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentTreeShutdownReport {
    pub root_thread_id: ThreadId,
    pub terminated_threads: Vec<ThreadShutdownReport>,
    pub total_closed: usize,
}

/// In-memory state of an active thread.
#[derive(Debug, Clone)]
struct ThreadState {
    id: ThreadId,
    parent_id: Option<ThreadId>,
    children: HashSet<ThreadId>,
    options: ThreadSpawnOptions,
    status: ThreadLifecycleStatus,
    active_steering_notes: Vec<String>,
    turn_counter: u32,
    created_at: i64,
}

/// Thread manager coordinating agent threads, turn submissions, steering, and shutdowns.
#[derive(Debug, Clone)]
pub struct ThreadManager {
    threads: Arc<Mutex<HashMap<ThreadId, ThreadState>>>,
}

impl Default for ThreadManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ThreadManager {
    pub fn new() -> Self {
        Self {
            threads: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Spawn a new agent thread with options.
    pub fn spawn_thread(&self, options: ThreadSpawnOptions) -> ThreadId {
        let thread_id = options
            .thread_id
            .clone()
            .unwrap_or_else(|| format!("thread-{}", Uuid::new_v4().simple()));

        let parent_id = options.parent_thread_id.clone();
        let mut threads = self.threads.lock().unwrap();

        // If parent exists, record child link
        if let Some(ref p_id) = parent_id
            && let Some(parent) = threads.get_mut(p_id)
        {
            parent.children.insert(thread_id.clone());
        }

        let state = ThreadState {
            id: thread_id.clone(),
            parent_id,
            children: HashSet::new(),
            options,
            status: ThreadLifecycleStatus::Idle,
            active_steering_notes: Vec::new(),
            turn_counter: 0,
            created_at: Utc::now().timestamp_millis(),
        };

        threads.insert(thread_id.clone(), state);
        thread_id
    }

    /// Retrieve the lifecycle status of a thread.
    pub fn status(&self, thread_id: &str) -> Option<ThreadLifecycleStatus> {
        self.threads
            .lock()
            .unwrap()
            .get(thread_id)
            .map(|t| t.status.clone())
    }

    /// Submit turn input to a thread following lifecycle state machine rules.
    pub fn submit_input(&self, thread_id: &str, input: TurnInput) -> TurnSubmissionOutcome {
        let mut threads = self.threads.lock().unwrap();
        let state = match threads.get_mut(thread_id) {
            Some(s) => s,
            None => {
                return TurnSubmissionOutcome::RejectedNotIdle {
                    current_status: "thread_not_found".to_string(),
                };
            }
        };

        match (&state.status, input) {
            (ThreadLifecycleStatus::Closed { .. }, _) => {
                TurnSubmissionOutcome::RejectedThreadClosed
            }
            (ThreadLifecycleStatus::Idle, TurnInput::UserMessage(_msg)) => {
                state.turn_counter += 1;
                let turn_id = format!("turn-{}-{}", state.turn_counter, Uuid::new_v4().simple());
                state.status = ThreadLifecycleStatus::RunningTurn {
                    turn_id: turn_id.clone(),
                    started_at: Utc::now().timestamp_millis(),
                };
                TurnSubmissionOutcome::Started { turn_id }
            }
            (ThreadLifecycleStatus::RunningTurn { turn_id, .. }, TurnInput::Steer(steer_note)) => {
                state.active_steering_notes.push(steer_note);
                TurnSubmissionOutcome::Steered {
                    turn_id: turn_id.clone(),
                }
            }
            (
                ThreadLifecycleStatus::AwaitingInput { turn_id, .. },
                TurnInput::ToolApprovalResponse { .. },
            )
            | (
                ThreadLifecycleStatus::AwaitingInput { turn_id, .. },
                TurnInput::ElicitationAnswer { .. },
            )
            | (ThreadLifecycleStatus::Suspended { turn_id, .. }, TurnInput::Resume) => {
                let tid = turn_id.clone();
                state.status = ThreadLifecycleStatus::RunningTurn {
                    turn_id: tid.clone(),
                    started_at: Utc::now().timestamp_millis(),
                };
                TurnSubmissionOutcome::Resumed { turn_id: tid }
            }
            (current, _) => TurnSubmissionOutcome::RejectedNotIdle {
                current_status: format!("{:?}", current),
            },
        }
    }

    /// Drain any pending steering notes delivered to a running turn.
    pub fn take_pending_steering_notes(&self, thread_id: &str) -> Vec<String> {
        let mut threads = self.threads.lock().unwrap();
        if let Some(state) = threads.get_mut(thread_id) {
            std::mem::take(&mut state.active_steering_notes)
        } else {
            Vec::new()
        }
    }

    /// Transition a running turn into AwaitingInput (e.g. for user approval or clarification).
    pub fn pause_for_input(&self, thread_id: &str, prompt: impl Into<String>) -> bool {
        let mut threads = self.threads.lock().unwrap();
        if let Some(state) = threads.get_mut(thread_id)
            && let ThreadLifecycleStatus::RunningTurn { turn_id, .. } = &state.status
        {
            state.status = ThreadLifecycleStatus::AwaitingInput {
                turn_id: turn_id.clone(),
                prompt: prompt.into(),
            };
            return true;
        }
        false
    }

    /// Mark a turn as finished, returning the thread to Idle status.
    pub fn complete_turn(&self, thread_id: &str) -> bool {
        let mut threads = self.threads.lock().unwrap();
        if let Some(state) = threads.get_mut(thread_id) {
            match state.status {
                ThreadLifecycleStatus::RunningTurn { .. }
                | ThreadLifecycleStatus::AwaitingInput { .. }
                | ThreadLifecycleStatus::Suspended { .. } => {
                    state.status = ThreadLifecycleStatus::Idle;
                    state.active_steering_notes.clear();
                    true
                }
                _ => false,
            }
        } else {
            false
        }
    }

    /// Gracefully shutdown an agent thread and all of its spawned children recursively.
    pub fn shutdown_tree(&self, root_thread_id: &str) -> AgentTreeShutdownReport {
        let mut threads = self.threads.lock().unwrap();
        let mut terminated = Vec::new();
        let now = Utc::now().timestamp_millis();

        let mut queue = vec![root_thread_id.to_string()];
        let mut visited = HashSet::new();

        while let Some(current_id) = queue.pop() {
            if !visited.insert(current_id.clone()) {
                continue;
            }

            if let Some(state) = threads.get_mut(&current_id) {
                // Enqueue all spawned children
                for child in &state.children {
                    queue.push(child.clone());
                }

                let was_running = match &state.status {
                    ThreadLifecycleStatus::RunningTurn { turn_id, .. } => Some(turn_id.clone()),
                    ThreadLifecycleStatus::AwaitingInput { turn_id, .. } => Some(turn_id.clone()),
                    ThreadLifecycleStatus::Suspended { turn_id, .. } => Some(turn_id.clone()),
                    _ => None,
                };

                state.status = ThreadLifecycleStatus::Closed { closed_at: now };
                state.active_steering_notes.clear();

                terminated.push(ThreadShutdownReport {
                    thread_id: current_id,
                    was_running_turn: was_running,
                    closed_at: now,
                });
            }
        }

        let count = terminated.len();
        AgentTreeShutdownReport {
            root_thread_id: root_thread_id.to_string(),
            terminated_threads: terminated,
            total_closed: count,
        }
    }

    /// List direct child thread IDs of a parent.
    pub fn list_children(&self, parent_thread_id: &str) -> Vec<ThreadId> {
        let threads = self.threads.lock().unwrap();
        threads
            .get(parent_thread_id)
            .map(|t| t.children.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Retrieve parent thread ID if this thread was spawned by a parent agent.
    pub fn get_parent(&self, thread_id: &str) -> Option<ThreadId> {
        self.threads
            .lock()
            .unwrap()
            .get(thread_id)
            .and_then(|t| t.parent_id.clone())
    }

    /// Retrieve spawn options configured for this thread.
    pub fn get_options(&self, thread_id: &str) -> Option<ThreadSpawnOptions> {
        self.threads
            .lock()
            .unwrap()
            .get(thread_id)
            .map(|t| t.options.clone())
    }

    /// Retrieve the creation timestamp (milliseconds since Unix epoch).
    pub fn get_created_at(&self, thread_id: &str) -> Option<i64> {
        self.threads
            .lock()
            .unwrap()
            .get(thread_id)
            .map(|t| t.created_at)
    }

    /// List all currently known thread IDs in memory.
    pub fn list_all_thread_ids(&self) -> Vec<ThreadId> {
        self.threads
            .lock()
            .unwrap()
            .values()
            .map(|t| t.id.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thread_spawn_and_lifecycle_transitions() {
        let tm = ThreadManager::new();
        let tid = tm.spawn_thread(ThreadSpawnOptions::default());

        assert_eq!(tm.status(&tid), Some(ThreadLifecycleStatus::Idle));

        // Start a turn
        let res = tm.submit_input(&tid, TurnInput::UserMessage("Write quicksort".to_string()));
        let turn_id = match res {
            TurnSubmissionOutcome::Started { turn_id } => turn_id,
            other => panic!("Expected Started, got {:?}", other),
        };

        match tm.status(&tid).unwrap() {
            ThreadLifecycleStatus::RunningTurn {
                turn_id: current_tid,
                ..
            } => {
                assert_eq!(current_tid, turn_id);
            }
            other => panic!("Expected RunningTurn, got {:?}", other),
        }

        // Steer while running
        let steer_res = tm.submit_input(&tid, TurnInput::Steer("Make it in-place".to_string()));
        assert_eq!(
            steer_res,
            TurnSubmissionOutcome::Steered {
                turn_id: turn_id.clone()
            }
        );

        let notes = tm.take_pending_steering_notes(&tid);
        assert_eq!(notes, vec!["Make it in-place"]);

        // Complete turn
        assert!(tm.complete_turn(&tid));
        assert_eq!(tm.status(&tid), Some(ThreadLifecycleStatus::Idle));
    }

    #[test]
    fn test_thread_awaiting_input_and_resume() {
        let tm = ThreadManager::new();
        let tid = tm.spawn_thread(ThreadSpawnOptions::default());

        tm.submit_input(&tid, TurnInput::UserMessage("Run tests".to_string()));
        assert!(tm.pause_for_input(&tid, "Approve running bash command rm -rf?"));

        // Reject new turn while awaiting approval
        let rejected = tm.submit_input(&tid, TurnInput::UserMessage("Another request".to_string()));
        assert!(matches!(
            rejected,
            TurnSubmissionOutcome::RejectedNotIdle { .. }
        ));

        // Send approval
        let resume_res = tm.submit_input(
            &tid,
            TurnInput::ToolApprovalResponse {
                approved: true,
                reason: Some("Allowed by developer".to_string()),
            },
        );
        assert!(matches!(resume_res, TurnSubmissionOutcome::Resumed { .. }));
    }

    #[test]
    fn test_shutdown_agent_tree_hierarchy() {
        let tm = ThreadManager::new();
        let root = tm.spawn_thread(ThreadSpawnOptions::default());

        let child_opts1 = ThreadSpawnOptions {
            parent_thread_id: Some(root.clone()),
            ..Default::default()
        };
        let child1 = tm.spawn_thread(child_opts1);

        let child_opts2 = ThreadSpawnOptions {
            parent_thread_id: Some(child1.clone()),
            ..Default::default()
        };
        let grandchild = tm.spawn_thread(child_opts2);

        // Run turn on grandchild
        tm.submit_input(&grandchild, TurnInput::UserMessage("Research".to_string()));

        let report = tm.shutdown_tree(&root);
        assert_eq!(report.total_closed, 3);
        assert!(matches!(
            tm.status(&root),
            Some(ThreadLifecycleStatus::Closed { .. })
        ));
        assert!(matches!(
            tm.status(&child1),
            Some(ThreadLifecycleStatus::Closed { .. })
        ));
        assert!(matches!(
            tm.status(&grandchild),
            Some(ThreadLifecycleStatus::Closed { .. })
        ));

        // Attempts to submit to closed thread must be rejected
        let after_close = tm.submit_input(&root, TurnInput::UserMessage("Ping".to_string()));
        assert_eq!(after_close, TurnSubmissionOutcome::RejectedThreadClosed);
    }
}

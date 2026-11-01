//! Sovereign turn admission controller and queue governor.
//!
//! Governs turn submission concurrency, admission queues, and single-turn-per-thread serialization.
//! Coordinates multi-client turn submissions across active threads:
//! - Strictly guarantees single-active-turn execution per conversation thread.
//! - Prioritized pending turn queue (UserInteractive > AutonomousSteer > BackgroundTask > ScheduledCron).
//! - Out-of-band steering into actively executing turns without thread abortion.
//! - Global concurrency governance and per-thread queue bounds.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::RwLock;
use uuid::Uuid;

/// Priority classification for incoming turn admission requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnPriority {
    /// Scheduled cron or background timer trigger (lowest priority).
    ScheduledCron = 0,
    /// Asynchronous tool callback or background agent subtask.
    BackgroundTask = 1,
    /// Mid-flight steering instruction injected into a running turn.
    AutonomousSteer = 2,
    /// Direct user interaction from TUI, CLI, or IDE extension (highest priority).
    UserInteractive = 3,
}

/// A request to begin or enqueue a conversation turn.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TurnAdmissionRequest {
    pub request_id: Uuid,
    pub thread_id: String,
    pub priority: TurnPriority,
    pub user_prompt: String,
    pub created_at: DateTime<Utc>,
}

impl TurnAdmissionRequest {
    pub fn new(
        thread_id: impl Into<String>,
        priority: TurnPriority,
        user_prompt: impl Into<String>,
    ) -> Self {
        Self {
            request_id: Uuid::new_v4(),
            thread_id: thread_id.into(),
            priority,
            user_prompt: user_prompt.into(),
            created_at: Utc::now(),
        }
    }
}

/// The outcome of submitting a turn for admission.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "status", content = "payload")]
pub enum TurnAdmissionDecision {
    /// Admitted immediately for execution on the thread.
    Admitted { lease_id: Uuid },
    /// Queued behind currently running turns on this thread.
    Queued {
        queue_position: usize,
        estimated_wait_ms: u64,
    },
    /// Steered directly into the active running turn.
    SteeredIntoActiveTurn { active_lease_id: Uuid },
    /// Rejected due to queue capacity limits or system overload.
    RejectedBusy { reason: String },
}

/// Configuration options for the turn admission controller.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnAdmissionConfig {
    /// Maximum concurrent active turns across all threads globally.
    pub max_global_active_turns: usize,
    /// Maximum pending turns queued per individual thread.
    pub max_queued_per_thread: usize,
}

impl Default for TurnAdmissionConfig {
    fn default() -> Self {
        Self {
            max_global_active_turns: 10,
            max_queued_per_thread: 5,
        }
    }
}

struct ActiveTurnEntry {
    lease_id: Uuid,
    request: TurnAdmissionRequest,
}

/// Thread-safe admission controller managing turn execution concurrency.
pub struct TurnAdmissionController {
    config: TurnAdmissionConfig,
    active_turns: RwLock<HashMap<String, ActiveTurnEntry>>,
    queued_turns: RwLock<HashMap<String, VecDeque<TurnAdmissionRequest>>>,
}

impl Default for TurnAdmissionController {
    fn default() -> Self {
        Self::new(TurnAdmissionConfig::default())
    }
}

impl TurnAdmissionController {
    pub fn new(config: TurnAdmissionConfig) -> Self {
        Self {
            config,
            active_turns: RwLock::new(HashMap::new()),
            queued_turns: RwLock::new(HashMap::new()),
        }
    }

    /// Submit a turn admission request.
    pub fn submit_turn(&self, request: TurnAdmissionRequest) -> TurnAdmissionDecision {
        let thread_id = request.thread_id.clone();

        // 1. Check if the turn can be steered into an actively running turn
        if request.priority == TurnPriority::AutonomousSteer {
            let active = self.active_turns.read().unwrap();
            if let Some(entry) = active.get(&thread_id) {
                return TurnAdmissionDecision::SteeredIntoActiveTurn {
                    active_lease_id: entry.lease_id,
                };
            }
        }

        // 2. Check if thread already has an active turn running
        let mut active = self.active_turns.write().unwrap();
        if active.get(&thread_id).is_some() {
            // Thread is busy: enqueue the request
            let mut queues = self.queued_turns.write().unwrap();
            let thread_queue = queues.entry(thread_id.clone()).or_default();

            if thread_queue.len() >= self.config.max_queued_per_thread {
                return TurnAdmissionDecision::RejectedBusy {
                    reason: format!(
                        "Thread '{thread_id}' queue capacity limit reached ({}/{} items)",
                        thread_queue.len(),
                        self.config.max_queued_per_thread
                    ),
                };
            }

            // Insert sorted by priority descending (higher priority first)
            let mut insert_idx = thread_queue.len();
            for (idx, item) in thread_queue.iter().enumerate() {
                if request.priority > item.priority {
                    insert_idx = idx;
                    break;
                }
            }
            thread_queue.insert(insert_idx, request);

            let pos = insert_idx + 1;
            let est_wait = (pos as u64) * 3_000; // estimated 3s per turn

            return TurnAdmissionDecision::Queued {
                queue_position: pos,
                estimated_wait_ms: est_wait,
            };
        }

        // 3. Thread is free: check global active turn limit
        if active.len() >= self.config.max_global_active_turns {
            return TurnAdmissionDecision::RejectedBusy {
                reason: format!(
                    "Global active turn capacity ({}) reached across all sessions",
                    self.config.max_global_active_turns
                ),
            };
        }

        // 4. Admit the turn immediately
        let lease_id = Uuid::new_v4();
        active.insert(thread_id, ActiveTurnEntry { lease_id, request });

        TurnAdmissionDecision::Admitted { lease_id }
    }

    /// Complete an active turn on a thread.
    ///
    /// If there are pending queued turns on this thread, automatically admits
    /// the next highest priority turn and returns `Some((new_lease_id, request))`.
    pub fn complete_turn(
        &self,
        thread_id: &str,
        lease_id: Uuid,
    ) -> Option<(Uuid, TurnAdmissionRequest)> {
        let mut active = self.active_turns.write().unwrap();

        // Verify lease ID matches
        {
            let entry = active.get(thread_id)?;
            if entry.lease_id != lease_id {
                return None;
            }
        }

        active.remove(thread_id);

        // Check if there is a queued turn for this thread
        let mut queues = self.queued_turns.write().unwrap();
        if let Some(thread_queue) = queues.get_mut(thread_id)
            && let Some(next_req) = thread_queue.pop_front()
        {
            let new_lease = Uuid::new_v4();
            active.insert(
                thread_id.to_string(),
                ActiveTurnEntry {
                    lease_id: new_lease,
                    request: next_req.clone(),
                },
            );
            return Some((new_lease, next_req));
        }

        None
    }

    /// Cancel all pending queued turns for a specific thread.
    pub fn cancel_queued_turns(&self, thread_id: &str) -> usize {
        let mut queues = self.queued_turns.write().unwrap();
        if let Some(thread_queue) = queues.get_mut(thread_id) {
            let count = thread_queue.len();
            thread_queue.clear();
            count
        } else {
            0
        }
    }

    /// Query whether a thread currently has an actively running turn.
    pub fn is_thread_active(&self, thread_id: &str) -> bool {
        let active = self.active_turns.read().unwrap();
        active.contains_key(thread_id)
    }

    /// Retrieve the admission request for a thread's active turn, if running.
    pub fn active_turn_request(&self, thread_id: &str) -> Option<TurnAdmissionRequest> {
        let active = self.active_turns.read().unwrap();
        active.get(thread_id).map(|e| e.request.clone())
    }

    /// Total number of currently active turns running globally.
    pub fn active_turn_count(&self) -> usize {
        let active = self.active_turns.read().unwrap();
        active.len()
    }

    /// Number of queued turns for a specific thread.
    pub fn queued_turn_count(&self, thread_id: &str) -> usize {
        let queues = self.queued_turns.read().unwrap();
        queues.get(thread_id).map(|q| q.len()).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_admit_first_turn_immediately() {
        let controller = TurnAdmissionController::default();
        let req = TurnAdmissionRequest::new("th_1", TurnPriority::UserInteractive, "Hello");

        let decision = controller.submit_turn(req);
        match decision {
            TurnAdmissionDecision::Admitted { lease_id } => {
                assert!(!lease_id.is_nil());
                assert!(controller.is_thread_active("th_1"));
                assert_eq!(controller.active_turn_count(), 1);
            }
            _ => panic!("Expected turn to be admitted immediately"),
        }
    }

    #[test]
    fn test_queue_second_turn_on_same_thread() {
        let controller = TurnAdmissionController::default();

        let req1 = TurnAdmissionRequest::new("th_busy", TurnPriority::UserInteractive, "Task 1");
        let d1 = controller.submit_turn(req1);
        let lease1 = match d1 {
            TurnAdmissionDecision::Admitted { lease_id } => lease_id,
            _ => panic!("Expected first to be admitted"),
        };

        let req2 = TurnAdmissionRequest::new("th_busy", TurnPriority::BackgroundTask, "Task 2");
        let d2 = controller.submit_turn(req2);
        match d2 {
            TurnAdmissionDecision::Queued { queue_position, .. } => {
                assert_eq!(queue_position, 1);
                assert_eq!(controller.queued_turn_count("th_busy"), 1);
            }
            _ => panic!("Expected second to be queued"),
        }

        // Complete turn 1 -> automatically admits turn 2!
        let next = controller.complete_turn("th_busy", lease1);
        assert!(next.is_some());
        let (lease2, admitted_req) = next.unwrap();
        assert_ne!(lease1, lease2);
        assert_eq!(admitted_req.user_prompt, "Task 2");
        assert_eq!(controller.queued_turn_count("th_busy"), 0);
        assert!(controller.is_thread_active("th_busy"));

        // Complete turn 2 -> thread is now idle
        let empty = controller.complete_turn("th_busy", lease2);
        assert!(empty.is_none());
        assert!(!controller.is_thread_active("th_busy"));
    }

    #[test]
    fn test_steer_priority_routes_to_active() {
        let controller = TurnAdmissionController::default();

        let req1 =
            TurnAdmissionRequest::new("th_steer", TurnPriority::UserInteractive, "Long turn");
        let d1 = controller.submit_turn(req1);
        let lease1 = match d1 {
            TurnAdmissionDecision::Admitted { lease_id } => lease_id,
            _ => panic!("Expected admit"),
        };

        let steer_req = TurnAdmissionRequest::new(
            "th_steer",
            TurnPriority::AutonomousSteer,
            "Also check tests",
        );
        let d2 = controller.submit_turn(steer_req);
        match d2 {
            TurnAdmissionDecision::SteeredIntoActiveTurn { active_lease_id } => {
                assert_eq!(active_lease_id, lease1);
            }
            _ => panic!("Expected steer into active turn"),
        }
    }

    #[test]
    fn test_queue_overflow_rejected() {
        let config = TurnAdmissionConfig {
            max_global_active_turns: 5,
            max_queued_per_thread: 2,
        };
        let controller = TurnAdmissionController::new(config);

        controller.submit_turn(TurnAdmissionRequest::new(
            "th_q",
            TurnPriority::UserInteractive,
            "1",
        ));
        controller.submit_turn(TurnAdmissionRequest::new(
            "th_q",
            TurnPriority::UserInteractive,
            "2",
        ));
        controller.submit_turn(TurnAdmissionRequest::new(
            "th_q",
            TurnPriority::UserInteractive,
            "3",
        ));

        // 4th request breaches max_queued_per_thread = 2
        let d4 = controller.submit_turn(TurnAdmissionRequest::new(
            "th_q",
            TurnPriority::UserInteractive,
            "4",
        ));
        match d4 {
            TurnAdmissionDecision::RejectedBusy { reason } => {
                assert!(reason.contains("capacity limit reached"));
            }
            _ => panic!("Expected rejected busy"),
        }
    }
}

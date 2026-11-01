//! Causal critical-path and direct-tool timing attribution engine.
//!
//! Provides depth-bounded persistent DAG tracking of turn intervals across:
//! - Model Inference
//! - Automated Review (ExecPolicy / Guardian)
//! - User Wait (Interactive confirmation / approvals)
//! - Tool Execution (Code mode cells, subprocesses, native tools)
//! - Overhead (IPC, JSON serialization, process spawning)
//!
//! Computes execution critical paths across asynchronous subagent DAGs and profiles tool latency bottlenecks.
//! with memory safety, non-recursive node destruction, and causal branch joining.

use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Maximum depth of causal dependency history before collapsing into unknown work.
pub const MAX_PATH_DEPTH: usize = 16_384;

/// Category attributed to an observed segment of dependency work.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CausalWorkKind {
    /// Model inference work on the selected dependency path.
    Inference,
    /// Automatic review work, such as reviewing a tool call before execution.
    AutoReview,
    /// Time awaiting a human answer or approval on the selected dependency path.
    UserWait,
    /// Tool execution work, including work after a dependency join resumes.
    Tools,
    /// Time within the observed turn interval not covered by a recorded work segment.
    Overhead,
    /// Work whose category is unavailable, including history collapsed at the depth limit.
    Unknown,
}

/// Persistent immutable observed dependency path.
/// Forks share their prefix; a join retains the dependency that actually released the waiting operation.
#[derive(Clone, Default)]
pub struct CausalCriticalPath(Option<Arc<Node>>);

impl std::fmt::Debug for CausalCriticalPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CausalCriticalPath")
            .field("depth", &self.0.as_ref().map_or(0, |node| node.depth))
            .field("end", &self.end())
            .finish()
    }
}

impl PartialEq for CausalCriticalPath {
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (Some(left), Some(right)) => Arc::ptr_eq(left, right),
            (None, None) => true,
            (Some(_), None) | (None, Some(_)) => false,
        }
    }
}

impl Eq for CausalCriticalPath {}

struct Node {
    previous: CausalCriticalPath,
    kind: CausalWorkKind,
    start: Instant,
    end: Instant,
    origin: Instant,
    depth: usize,
}

/// Exclusive time attributed to each work category within the requested interval.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CausalPathDurations {
    pub inference: Duration,
    pub auto_review: Duration,
    pub user_wait: Duration,
    pub tools: Duration,
    pub overhead: Duration,
    pub unknown: Duration,
}

impl CausalPathDurations {
    fn record(&mut self, kind: CausalWorkKind, duration: Duration) {
        let total = match kind {
            CausalWorkKind::Inference => &mut self.inference,
            CausalWorkKind::AutoReview => &mut self.auto_review,
            CausalWorkKind::UserWait => &mut self.user_wait,
            CausalWorkKind::Tools => &mut self.tools,
            CausalWorkKind::Overhead => &mut self.overhead,
            CausalWorkKind::Unknown => &mut self.unknown,
        };
        *total = total.saturating_add(duration);
    }

    /// Sum of all categorized durations.
    pub fn total(&self) -> Duration {
        self.inference
            .saturating_add(self.auto_review)
            .saturating_add(self.user_wait)
            .saturating_add(self.tools)
            .saturating_add(self.overhead)
            .saturating_add(self.unknown)
    }
}

impl CausalCriticalPath {
    /// End of the last recorded segment, or `None` for an empty path.
    pub fn end(&self) -> Option<Instant> {
        self.0.as_ref().map(|node| node.end)
    }

    /// Retain the later-finishing path, choosing `self` on a tie.
    pub fn latest(self, other: Self) -> Self {
        if other.end() > self.end() {
            other
        } else {
            self
        }
    }

    /// Append work after the existing path ends, clipping overlap and ignoring empty segments.
    /// Exceeding the history limit collapses the retained interval into unknown work.
    pub fn work(self, start: Instant, end: Instant, kind: CausalWorkKind) -> Self {
        let start = self.end().map_or(start, |previous| previous.max(start));
        if start >= end {
            return self;
        }
        let origin = self.0.as_ref().map_or(start, |node| node.origin);
        let previous_depth = self.0.as_ref().map_or(0, |node| node.depth);
        let depth = previous_depth + 1;
        if depth > MAX_PATH_DEPTH {
            return Self::default().work(origin, end, CausalWorkKind::Unknown);
        }
        Self(Some(Arc::new(Node {
            previous: self,
            kind,
            start,
            end,
            origin,
            depth,
        })))
    }

    /// Select the full branch that released this join, carrying its common causal prefix.
    pub fn join(self, start: Instant, end: Instant, dependency: Self) -> Self {
        let path = self.latest(dependency);
        let resumed_at = path.end().unwrap_or(start).max(start);
        path.work(resumed_at, end, CausalWorkKind::Tools)
    }

    /// Attribute the requested interval exclusively, counting unrecorded gaps as overhead.
    pub fn durations(&self, start: Instant, end: Instant) -> CausalPathDurations {
        let mut totals = CausalPathDurations::default();
        let mut path = self;
        let mut end = end;
        while let Some(node) = path.0.as_ref() {
            if end <= start {
                break;
            }
            let segment_end = end.min(node.end);
            let segment_start = start.max(node.start);
            if end > node.end {
                totals.record(
                    CausalWorkKind::Overhead,
                    end.duration_since(start.max(node.end)),
                );
            }
            if segment_start < segment_end {
                totals.record(node.kind, segment_end.duration_since(segment_start));
            }
            end = end.min(node.start);
            path = &node.previous;
        }
        if start < end {
            totals.record(CausalWorkKind::Overhead, end.duration_since(start));
        }
        totals
    }
}

// Custom iterative Drop prevents call-stack overflow on deep causal paths.
impl Drop for Node {
    fn drop(&mut self) {
        let mut previous = self.previous.0.take();
        while let Some(node) = previous {
            let Some(mut node) = Arc::into_inner(node) else {
                break;
            };
            previous = node.previous.0.take();
        }
    }
}

/// Completion time and dependency history for one direct tool invocation.
#[derive(Clone, Debug)]
pub struct CausalCompletedWork {
    pub ended_at: Instant,
    pub path: Option<CausalCriticalPath>,
}

/// Tracks the last-finishing invocation in a parallel batch.
#[derive(Clone, Default)]
pub struct CausalToolBatchTiming(Arc<Mutex<BatchState>>);

#[derive(Default)]
struct BatchState {
    pending: usize,
    latest: Option<CausalCompletedWork>,
}

impl CausalToolBatchTiming {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a newly started parallel invocation.
    pub fn started(&self) {
        let mut state = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.pending += 1;
    }

    /// Record completed invocation work.
    pub fn record(&self, work: CausalCompletedWork) {
        let mut state = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.pending > 0 {
            state.pending -= 1;
        }
        if state
            .latest
            .as_ref()
            .is_none_or(|previous| previous.ended_at < work.ended_at)
        {
            state.latest = Some(work);
        }
    }

    /// Returns the batch result only when all invocations in the batch are finished.
    pub fn completed(&self) -> Option<CausalCompletedWork> {
        let state = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (state.pending == 0).then(|| state.latest.clone()).flatten()
    }

    /// Returns the number of currently active invocations.
    pub fn pending_count(&self) -> usize {
        let state = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_replaces_overlapping_work_with_the_full_winning_branch() {
        let turn_start = Instant::now();
        let time_after_turn_start = |ms| turn_start + Duration::from_millis(ms);

        let shared_initial_path = CausalCriticalPath::default().work(
            time_after_turn_start(0),
            time_after_turn_start(100),
            CausalWorkKind::Inference,
        );

        let reviewed_tool_path = shared_initial_path
            .clone()
            .work(
                time_after_turn_start(100),
                time_after_turn_start(300),
                CausalWorkKind::AutoReview,
            )
            .work(
                time_after_turn_start(300),
                time_after_turn_start(400),
                CausalWorkKind::Tools,
            );

        let parallel_tool_path = shared_initial_path.clone().work(
            time_after_turn_start(100),
            time_after_turn_start(700),
            CausalWorkKind::Tools,
        );

        let critical_path = shared_initial_path
            .work(
                time_after_turn_start(100),
                time_after_turn_start(200),
                CausalWorkKind::Inference,
            )
            .join(
                time_after_turn_start(200),
                time_after_turn_start(700),
                reviewed_tool_path.latest(parallel_tool_path),
            );

        assert_eq!(
            critical_path.durations(time_after_turn_start(0), time_after_turn_start(700)),
            CausalPathDurations {
                inference: Duration::from_millis(100),
                tools: Duration::from_millis(600),
                ..CausalPathDurations::default()
            }
        );
    }

    #[test]
    fn nested_serial_dependencies_preserve_exclusive_categories() {
        let turn_start = Instant::now();
        let time_after_turn_start = |ms| turn_start + Duration::from_millis(ms);

        let shared_initial_path = CausalCriticalPath::default().work(
            time_after_turn_start(0),
            time_after_turn_start(100),
            CausalWorkKind::Inference,
        );

        let child_path = shared_initial_path
            .clone()
            .work(
                time_after_turn_start(100),
                time_after_turn_start(300),
                CausalWorkKind::AutoReview,
            )
            .work(
                time_after_turn_start(300),
                time_after_turn_start(400),
                CausalWorkKind::Tools,
            )
            .work(
                time_after_turn_start(400),
                time_after_turn_start(500),
                CausalWorkKind::Inference,
            );

        let critical_path = shared_initial_path
            .work(
                time_after_turn_start(100),
                time_after_turn_start(200),
                CausalWorkKind::Inference,
            )
            .join(
                time_after_turn_start(200),
                time_after_turn_start(600),
                child_path,
            );

        assert_eq!(
            critical_path.durations(time_after_turn_start(0), time_after_turn_start(600)),
            CausalPathDurations {
                inference: Duration::from_millis(200),
                auto_review: Duration::from_millis(200),
                tools: Duration::from_millis(200),
                ..CausalPathDurations::default()
            }
        );
    }

    #[test]
    fn equal_finish_times_select_one_path_without_double_counting() {
        let turn_start = Instant::now();
        let turn_end = turn_start + Duration::from_millis(100);

        let reviewed_tool_path =
            CausalCriticalPath::default().work(turn_start, turn_end, CausalWorkKind::AutoReview);
        let inference_path =
            CausalCriticalPath::default().work(turn_start, turn_end, CausalWorkKind::Inference);

        assert_eq!(
            reviewed_tool_path
                .clone()
                .latest(inference_path)
                .durations(turn_start, turn_end),
            reviewed_tool_path.durations(turn_start, turn_end)
        );
    }

    #[test]
    fn overlapping_work_only_attributes_time_after_existing_path() {
        let turn_start = Instant::now();
        let time_after = |ms| turn_start + Duration::from_millis(ms);

        let critical_path = CausalCriticalPath::default()
            .work(time_after(0), time_after(10), CausalWorkKind::Inference)
            .work(time_after(5), time_after(20), CausalWorkKind::Tools);

        assert_eq!(
            critical_path.durations(time_after(0), time_after(20)),
            CausalPathDurations {
                inference: Duration::from_millis(10),
                tools: Duration::from_millis(10),
                ..CausalPathDurations::default()
            }
        );
    }

    #[test]
    fn test_batch_timing_coalescing() {
        let timing = CausalToolBatchTiming::new();
        timing.started();
        timing.started();
        assert_eq!(timing.pending_count(), 2);
        assert!(timing.completed().is_none());

        let t1 = Instant::now();
        timing.record(CausalCompletedWork {
            ended_at: t1,
            path: None,
        });
        assert_eq!(timing.pending_count(), 1);
        assert!(timing.completed().is_none());

        let t2 = t1 + Duration::from_millis(50);
        timing.record(CausalCompletedWork {
            ended_at: t2,
            path: None,
        });
        assert_eq!(timing.pending_count(), 0);

        let completed = timing.completed().expect("batch must be complete");
        assert_eq!(completed.ended_at, t2);
    }
}

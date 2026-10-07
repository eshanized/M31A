//! Dual-track Critical Path Method (CPM) engine (D-11, DAG-05).
//!
//! Provides baseline and live schedule projection, early/late time computation,
//! total slack (float), critical path detection, and schedule drift analysis.

use crate::ids::TaskId;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Default estimated duration for tasks when unspecified (300 seconds / 5 minutes).
/// ALGORITHMIC constant: a CPM scheduling-estimate default, not operator
/// policy. Operational timeouts are governed by `TimeoutPolicy` (canonical
/// config); this only fills an absent duration estimate for critical-path
/// projection math.
pub const DEFAULT_TASK_DURATION_SECS: u64 = 300;

/// Schedule timing information and critical path identification from CPM analysis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CriticalPathInfo {
    /// Tasks lying on the critical path (zero total float), in topological order.
    pub critical_tasks: Vec<TaskId>,
    /// Total duration of the longest path in the graph.
    pub total_projected_duration_secs: u64,
    /// Early start time for each task from project epoch 0.
    pub task_early_start: BTreeMap<TaskId, u64>,
    /// Early finish time for each task.
    pub task_early_finish: BTreeMap<TaskId, u64>,
    /// Late start time for each task without delaying the overall project duration.
    pub task_late_start: BTreeMap<TaskId, u64>,
    /// Late finish time for each task.
    pub task_late_finish: BTreeMap<TaskId, u64>,
    /// Total float / slack for each task (late start - early start).
    pub task_slack: BTreeMap<TaskId, u64>,
}

/// Schedule drift analysis comparing immutable baseline against live dynamic projections.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduleDrift {
    pub baseline_duration_secs: u64,
    pub live_duration_secs: u64,
    /// Difference: live_duration_secs - baseline_duration_secs (positive indicates delay).
    pub drift_secs: i64,
}

/// Calculate schedule drift between baseline and live CPM projections.
pub fn calculate_drift(baseline: &CriticalPathInfo, live: &CriticalPathInfo) -> ScheduleDrift {
    let baseline_duration = baseline.total_projected_duration_secs;
    let live_duration = live.total_projected_duration_secs;
    let drift_secs = live_duration as i64 - baseline_duration as i64;

    ScheduleDrift {
        baseline_duration_secs: baseline_duration,
        live_duration_secs: live_duration,
        drift_secs,
    }
}

/// Calculate Critical Path Method (CPM) metrics over a directed acyclic graph.
///
/// Executes standard dual-pass CPM algorithm:
/// 1. Forward pass: computes early start (ES) and early finish (EF) times, and project duration.
/// 2. Backward pass: computes late start (LS) and late finish (LF) times.
/// 3. Float & Critical Path: calculates total slack (LS - ES) and tags tasks with zero slack.
pub fn calculate_cpm(
    topological_order: &[TaskId],
    prerequisites_of: &BTreeMap<TaskId, BTreeSet<TaskId>>,
    dependents_of: &BTreeMap<TaskId, BTreeSet<TaskId>>,
    durations: &BTreeMap<TaskId, u64>,
) -> CriticalPathInfo {
    let mut task_early_start = BTreeMap::new();
    let mut task_early_finish = BTreeMap::new();
    let mut task_late_start = BTreeMap::new();
    let mut task_late_finish = BTreeMap::new();
    let mut task_slack = BTreeMap::new();

    if topological_order.is_empty() {
        return CriticalPathInfo {
            critical_tasks: Vec::new(),
            total_projected_duration_secs: 0,
            task_early_start,
            task_early_finish,
            task_late_start,
            task_late_finish,
            task_slack,
        };
    }

    // 1. Forward pass: compute Early Start (ES) and Early Finish (EF)
    for &task_id in topological_order {
        let duration = durations
            .get(&task_id)
            .copied()
            .unwrap_or(DEFAULT_TASK_DURATION_SECS);

        let es = match prerequisites_of.get(&task_id) {
            Some(prereqs) if !prereqs.is_empty() => prereqs
                .iter()
                .map(|p| task_early_finish.get(p).copied().unwrap_or(0))
                .max()
                .unwrap_or(0),
            _ => 0,
        };

        let ef = es.saturating_add(duration);
        task_early_start.insert(task_id, es);
        task_early_finish.insert(task_id, ef);
    }

    let total_projected_duration_secs = task_early_finish.values().copied().max().unwrap_or(0);

    // 2. Backward pass: compute Late Finish (LF) and Late Start (LS)
    for &task_id in topological_order.iter().rev() {
        let duration = durations
            .get(&task_id)
            .copied()
            .unwrap_or(DEFAULT_TASK_DURATION_SECS);

        let lf = match dependents_of.get(&task_id) {
            Some(deps) if !deps.is_empty() => deps
                .iter()
                .map(|d| {
                    task_late_start
                        .get(d)
                        .copied()
                        .unwrap_or(total_projected_duration_secs)
                })
                .min()
                .unwrap_or(total_projected_duration_secs),
            _ => total_projected_duration_secs,
        };

        let ls = lf.saturating_sub(duration);
        task_late_finish.insert(task_id, lf);
        task_late_start.insert(task_id, ls);
    }

    // 3. Compute Total Slack (LS - ES) and identify Critical Tasks
    let mut critical_tasks = Vec::new();
    for &task_id in topological_order {
        let es = task_early_start.get(&task_id).copied().unwrap_or(0);
        let ls = task_late_start.get(&task_id).copied().unwrap_or(0);
        let slack = ls.saturating_sub(es);
        task_slack.insert(task_id, slack);

        if slack == 0 {
            critical_tasks.push(task_id);
        }
    }

    CriticalPathInfo {
        critical_tasks,
        total_projected_duration_secs,
        task_early_start,
        task_early_finish,
        task_late_start,
        task_late_finish,
        task_slack,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_cpm() {
        let t1 = TaskId::new();
        let t2 = TaskId::new();
        let t3 = TaskId::new();

        let topo = vec![t1, t2, t3];

        let mut prereqs: BTreeMap<TaskId, BTreeSet<TaskId>> = BTreeMap::new();
        prereqs.entry(t1).or_default();
        prereqs.entry(t2).or_default().insert(t1);
        prereqs.entry(t3).or_default().insert(t2);

        let mut deps: BTreeMap<TaskId, BTreeSet<TaskId>> = BTreeMap::new();
        deps.entry(t1).or_default().insert(t2);
        deps.entry(t2).or_default().insert(t3);
        deps.entry(t3).or_default();

        let mut durations = BTreeMap::new();
        durations.insert(t1, 10);
        durations.insert(t2, 20);
        durations.insert(t3, 30);

        let cpm = calculate_cpm(&topo, &prereqs, &deps, &durations);

        assert_eq!(cpm.total_projected_duration_secs, 60);
        assert_eq!(cpm.critical_tasks, vec![t1, t2, t3]);

        assert_eq!(cpm.task_early_start.get(&t1), Some(&0));
        assert_eq!(cpm.task_early_finish.get(&t1), Some(&10));
        assert_eq!(cpm.task_early_start.get(&t2), Some(&10));
        assert_eq!(cpm.task_early_finish.get(&t2), Some(&30));
        assert_eq!(cpm.task_early_start.get(&t3), Some(&30));
        assert_eq!(cpm.task_early_finish.get(&t3), Some(&60));

        assert_eq!(cpm.task_slack.get(&t1), Some(&0));
        assert_eq!(cpm.task_slack.get(&t2), Some(&0));
        assert_eq!(cpm.task_slack.get(&t3), Some(&0));
    }

    #[test]
    fn test_diamond_cpm_slack() {
        let t1 = TaskId::new();
        let t2 = TaskId::new(); // fast path
        let t3 = TaskId::new(); // slow path
        let t4 = TaskId::new();

        let topo = vec![t1, t2, t3, t4];

        let mut prereqs: BTreeMap<TaskId, BTreeSet<TaskId>> = BTreeMap::new();
        prereqs.entry(t1).or_default();
        prereqs.entry(t2).or_default().insert(t1);
        prereqs.entry(t3).or_default().insert(t1);
        prereqs.entry(t4).or_default().extend([t2, t3]);

        let mut deps: BTreeMap<TaskId, BTreeSet<TaskId>> = BTreeMap::new();
        deps.entry(t1).or_default().extend([t2, t3]);
        deps.entry(t2).or_default().insert(t4);
        deps.entry(t3).or_default().insert(t4);
        deps.entry(t4).or_default();

        let mut durations = BTreeMap::new();
        durations.insert(t1, 100);
        durations.insert(t2, 10); // fast branch: 10s
        durations.insert(t3, 50); // slow branch: 50s
        durations.insert(t4, 20);

        let cpm = calculate_cpm(&topo, &prereqs, &deps, &durations);

        // Project duration: 100 (t1) + 50 (t3) + 20 (t4) = 170
        assert_eq!(cpm.total_projected_duration_secs, 170);

        // Critical tasks should be T1 -> T3 -> T4
        assert_eq!(cpm.critical_tasks, vec![t1, t3, t4]);

        // T2 slack: LS(t2) = 150 - 10 = 140, ES(t2) = 100, Slack = 40
        assert_eq!(cpm.task_slack.get(&t2), Some(&40));
        assert_eq!(cpm.task_slack.get(&t1), Some(&0));
        assert_eq!(cpm.task_slack.get(&t3), Some(&0));
        assert_eq!(cpm.task_slack.get(&t4), Some(&0));
    }

    #[test]
    fn test_schedule_drift() {
        let baseline = CriticalPathInfo {
            critical_tasks: Vec::new(),
            total_projected_duration_secs: 100,
            task_early_start: BTreeMap::new(),
            task_early_finish: BTreeMap::new(),
            task_late_start: BTreeMap::new(),
            task_late_finish: BTreeMap::new(),
            task_slack: BTreeMap::new(),
        };

        let live = CriticalPathInfo {
            critical_tasks: Vec::new(),
            total_projected_duration_secs: 135,
            task_early_start: BTreeMap::new(),
            task_early_finish: BTreeMap::new(),
            task_late_start: BTreeMap::new(),
            task_late_finish: BTreeMap::new(),
            task_slack: BTreeMap::new(),
        };

        let drift = calculate_drift(&baseline, &live);
        assert_eq!(drift.baseline_duration_secs, 100);
        assert_eq!(drift.live_duration_secs, 135);
        assert_eq!(drift.drift_secs, 35);
    }
}

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

use crate::ids::{MissionId, TaskGraphId, TaskId};

/// Aggregate counts of tasks in various states across the task graph.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueStatistics {
    pub pending_count: usize,
    pub ready_count: usize,
    pub running_count: usize,
    pub blocked_count: usize,
    pub completed_count: usize,
    pub failed_count: usize,
    pub cancelled_count: usize,
    pub skipped_count: usize,
    pub needs_review_count: usize,
}

/// Read-only atomic snapshot projection of scheduler runtime state for TUI/telemetry (D-12).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchedulerSnapshot {
    pub mission_id: MissionId,
    pub graph_id: TaskGraphId,
    pub revision: u32,
    pub active_workers: usize,
    pub max_workers: usize,
    pub queue_stats: QueueStatistics,
    pub ready_task_ids: Vec<TaskId>,
    pub running_task_ids: Vec<TaskId>,
    pub blocked_task_ids: Vec<(TaskId, String)>,
    pub active_leases: Vec<(Uuid, TaskId, String, String)>,
    pub wave_tiers: BTreeMap<usize, Vec<TaskId>>,
    pub critical_path: Vec<TaskId>,
    pub projected_duration_secs: u64,
    pub schedule_drift_secs: i64,
    pub timestamp: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduler_snapshot_serde_roundtrip() {
        let snapshot = SchedulerSnapshot {
            mission_id: MissionId::new(),
            graph_id: TaskGraphId::new(),
            revision: 1,
            active_workers: 2,
            max_workers: 8,
            queue_stats: QueueStatistics {
                ready_count: 2,
                running_count: 2,
                ..Default::default()
            },
            ready_task_ids: vec![TaskId::new(), TaskId::new()],
            running_task_ids: vec![TaskId::new(), TaskId::new()],
            blocked_task_ids: Vec::new(),
            active_leases: Vec::new(),
            wave_tiers: BTreeMap::new(),
            critical_path: Vec::new(),
            projected_duration_secs: 120,
            schedule_drift_secs: 5,
            timestamp: Utc::now(),
        };

        let json = serde_json::to_string(&snapshot).unwrap();
        let deserialized: SchedulerSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(snapshot, deserialized);
    }
}

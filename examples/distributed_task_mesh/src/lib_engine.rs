//! Task engine coordinator for Distributed Task Mesh.
//!
//! Integrates write-ahead logging, scheduler queues, and task dispatching into a unified local engine.

use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::broadcast;
use tracing::info;

use crate::cluster::ClusterMembership;
use crate::metrics::TaskMeshMetrics;
use crate::scheduler::WorkStealingScheduler;
use crate::types::{MeshEvent, NodeId, TaskDefinition, TaskId, TaskStatus};
use crate::wal::{WalRecord, WriteAheadLog};

pub struct TaskMeshNode {
    pub node_id: NodeId,
    pub scheduler: Arc<WorkStealingScheduler>,
    pub cluster: Arc<ClusterMembership>,
    pub wal: Arc<WriteAheadLog>,
    pub metrics: Arc<TaskMeshMetrics>,
    task_states: Arc<Mutex<HashMap<TaskId, TaskStatus>>>,
    event_bus: broadcast::Sender<MeshEvent>,
}

impl TaskMeshNode {
    pub fn new(node_id: NodeId, wal_path: impl AsRef<Path>) -> Result<Self> {
        let (tx, _) = broadcast::channel(1024);
        let wal = Arc::new(WriteAheadLog::open(wal_path)?);

        let node = Self {
            node_id: node_id.clone(),
            scheduler: Arc::new(WorkStealingScheduler::new()),
            cluster: Arc::new(ClusterMembership::new(node_id, Duration::from_secs(3))),
            wal,
            metrics: Arc::new(TaskMeshMetrics::new()),
            task_states: Arc::new(Mutex::new(HashMap::new())),
            event_bus: tx,
        };

        // Recover state from WAL if existing records exist
        node.recover_from_wal()?;

        Ok(node)
    }

    pub fn submit(&self, task: TaskDefinition) -> Result<()> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        // 1. Commit to WAL before in-memory queueing
        self.wal.append(&WalRecord::CreateTask {
            id: task.id,
            name: task.name.clone(),
            payload: task.payload.clone(),
            priority: task.priority as u8,
            created_at: now,
        })?;

        // 2. Track in-memory state
        self.task_states
            .lock()
            .unwrap()
            .insert(task.id, TaskStatus::Pending);

        // 3. Schedule onto worker queue
        let assigned_to = self.scheduler.schedule(task.clone());
        self.metrics.record_submitted();

        let _ = self.event_bus.send(MeshEvent::TaskSubmitted(task));
        info!(assigned_worker = %assigned_to, "Task submitted and queued");

        Ok(())
    }

    pub fn complete_task(
        &self,
        task_id: TaskId,
        result: serde_json::Value,
        duration_ms: u64,
    ) -> Result<()> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let status = TaskStatus::Completed {
            result: result.clone(),
            duration_ms,
        };

        self.wal.append(&WalRecord::UpdateStatus {
            id: task_id,
            status: status.clone(),
            timestamp: now,
        })?;

        self.task_states.lock().unwrap().insert(task_id, status);
        self.metrics.record_completed(duration_ms);

        let _ = self.event_bus.send(MeshEvent::TaskCompleted {
            task_id,
            result,
            duration_ms,
        });

        Ok(())
    }

    pub fn fail_task(&self, task_id: TaskId, reason: String) -> Result<()> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let status = TaskStatus::Failed {
            reason: reason.clone(),
            retry_count: 1,
        };

        self.wal.append(&WalRecord::UpdateStatus {
            id: task_id,
            status: status.clone(),
            timestamp: now,
        })?;

        self.task_states.lock().unwrap().insert(task_id, status);
        self.metrics.record_failed();

        let _ = self
            .event_bus
            .send(MeshEvent::TaskFailed { task_id, reason });

        Ok(())
    }

    pub fn cancel_task(&self, task_id: TaskId, reason: String) -> Result<()> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let status = TaskStatus::Failed {
            reason: format!("Cancelled: {reason}"),
            retry_count: 0,
        };

        self.wal.append(&WalRecord::UpdateStatus {
            id: task_id,
            status: status.clone(),
            timestamp: now,
        })?;

        self.task_states.lock().unwrap().insert(task_id, status);
        self.metrics.record_failed();

        let _ = self
            .event_bus
            .send(MeshEvent::TaskFailed { task_id, reason });

        Ok(())
    }

    pub fn get_status(&self, task_id: &TaskId) -> Option<TaskStatus> {
        self.task_states.lock().unwrap().get(task_id).cloned()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<MeshEvent> {
        self.event_bus.subscribe()
    }

    fn recover_from_wal(&self) -> Result<()> {
        let records = self.wal.replay()?;
        let mut states = self.task_states.lock().unwrap();

        for record in records {
            match record {
                WalRecord::CreateTask { id, .. } => {
                    states.entry(id).or_insert(TaskStatus::Pending);
                }
                WalRecord::UpdateStatus { id, status, .. } => {
                    states.insert(id, status);
                }
                WalRecord::Checkpoint { .. } => {}
            }
        }

        Ok(())
    }
}

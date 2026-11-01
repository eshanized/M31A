//! Task scheduler and priority queue for Distributed Task Mesh.
//!
//! Implements priority-weighted FIFO task scheduling with dependency tracking and retry management.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tracing::debug;

use crate::types::{NodeId, Priority, TaskDefinition};

pub struct WorkerQueue {
    pub worker_id: NodeId,
    /// High-priority queue
    pub high: Mutex<VecDeque<TaskDefinition>>,
    /// Normal and low-priority queue
    pub normal: Mutex<VecDeque<TaskDefinition>>,
}

impl WorkerQueue {
    pub fn new(worker_id: NodeId) -> Self {
        Self {
            worker_id,
            high: Mutex::new(VecDeque::new()),
            normal: Mutex::new(VecDeque::new()),
        }
    }

    pub fn push(&self, task: TaskDefinition) {
        if matches!(task.priority, Priority::High | Priority::Critical) {
            self.high.lock().unwrap().push_back(task);
        } else {
            self.normal.lock().unwrap().push_back(task);
        }
    }

    /// Pops from local worker queue (LIFO for cache-locality on local thread).
    pub fn pop_local(&self) -> Option<TaskDefinition> {
        let mut high = self.high.lock().unwrap();
        if let Some(task) = high.pop_back() {
            return Some(task);
        }
        let mut normal = self.normal.lock().unwrap();
        normal.pop_back()
    }

    /// Steals from remote worker queue (FIFO / pop_front to avoid contention with worker's pop_back).
    pub fn steal(&self) -> Option<TaskDefinition> {
        let mut high = self.high.lock().unwrap();
        if let Some(task) = high.pop_front() {
            return Some(task);
        }
        let mut normal = self.normal.lock().unwrap();
        normal.pop_front()
    }

    pub fn len(&self) -> usize {
        self.high.lock().unwrap().len() + self.normal.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

pub struct WorkStealingScheduler {
    workers: Arc<Mutex<HashMap<NodeId, Arc<WorkerQueue>>>>,
    worker_list: Arc<Mutex<Vec<NodeId>>>,
    stolen_tasks: AtomicU64,
}

impl WorkStealingScheduler {
    pub fn new() -> Self {
        Self {
            workers: Arc::new(Mutex::new(HashMap::new())),
            worker_list: Arc::new(Mutex::new(Vec::new())),
            stolen_tasks: AtomicU64::new(0),
        }
    }

    pub fn register_worker(&self, worker_id: NodeId) -> Arc<WorkerQueue> {
        let mut workers = self.workers.lock().unwrap();
        let queue = Arc::new(WorkerQueue::new(worker_id.clone()));
        workers.insert(worker_id.clone(), Arc::clone(&queue));
        self.worker_list.lock().unwrap().push(worker_id);
        queue
    }

    /// Schedules task onto least loaded worker queue.
    pub fn schedule(&self, task: TaskDefinition) -> NodeId {
        let workers = self.workers.lock().unwrap();
        let best_worker = workers
            .values()
            .min_by_key(|q| q.len())
            .expect("Scheduler must have at least one registered worker");

        let target_id = best_worker.worker_id.clone();
        best_worker.push(task);
        target_id
    }

    /// Retrieves next task for a worker: tries local queue first, then work-steals from peers.
    pub fn next_task_or_steal(&self, worker_id: &NodeId) -> Option<(TaskDefinition, bool)> {
        let workers = self.workers.lock().unwrap();
        if let Some(local_queue) = workers.get(worker_id)
            && let Some(task) = local_queue.pop_local()
        {
            return Some((task, false));
        }

        // Steal from other workers with tasks available
        let list = self.worker_list.lock().unwrap();
        for peer_id in list.iter() {
            if peer_id == worker_id {
                continue;
            }
            if let Some(peer_queue) = workers.get(peer_id)
                && let Some(stolen) = peer_queue.steal()
            {
                self.stolen_tasks.fetch_add(1, Ordering::Relaxed);
                debug!(worker = %worker_id, victim = %peer_id, task_id = %stolen.id, "Work-stealing succeeded");
                return Some((stolen, true));
            }
        }

        None
    }

    pub fn total_pending_tasks(&self) -> usize {
        let workers = self.workers.lock().unwrap();
        workers.values().map(|q| q.len()).sum()
    }

    pub fn stolen_count(&self) -> u64 {
        self.stolen_tasks.load(Ordering::Relaxed)
    }
}

impl Default for WorkStealingScheduler {
    fn default() -> Self {
        Self::new()
    }
}

use async_trait::async_trait;
use chrono::Utc;
use sqlx::SqlitePool;
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;

use crate::dag::cpm::{CriticalPathInfo, calculate_cpm};
use crate::dag::graph::TaskGraph;
use crate::dag::materializer::TaskGraphMaterializer;
use crate::dag::reconciler::{ReconciliationResult, TaskGraphReconciler};
use crate::events::EventBus;
use crate::ids::{AgentId, MissionId, TaskGraphId, TaskId};
use crate::kernel::plan::CandidatePlan;
use crate::kernel::seams::scheduler::{ReadyWorkResponse, SchedulerError, WorkItem, WorkScheduler};
use crate::persistence::sqlite::repositories::{SqliteTaskGraphRepository, TaskGraphRepository};
use crate::scheduler::concurrency::{ConcurrencyLimiter, ConcurrencyLimits};
use crate::scheduler::events::{
    emit_critical_path_recalculated, emit_graph_materialized, emit_task_blocked, emit_wave_tier,
};
use crate::scheduler::queue::{PriorityDispatchQueue, QueueTaskEntry};
use crate::scheduler::resources::{LockMode, ResourceKey, ResourceLeaseGuard, ResourceManager};
use crate::scheduler::snapshot::{QueueStatistics, SchedulerSnapshot};
use crate::state::task::{BlockingReason, TaskResult};
use crate::state_machine::TaskState;
use crate::state_machine::agent::AgentRole;
use crate::state_machine::task::{TaskEvent, transition_task};

fn compute_cpm_for_graph(graph: &TaskGraph) -> CriticalPathInfo {
    let topological_order = graph.topological_order();
    let mut durations = BTreeMap::new();
    for (&id, task) in &graph.tasks {
        let dur = if task.estimates.max_duration_secs > 0 {
            task.estimates.max_duration_secs
        } else {
            300
        };
        durations.insert(id, dur);
    }
    calculate_cpm(
        &topological_order,
        &graph.prerequisites_of,
        &graph.dependents_of,
        &durations,
    )
}

type TaskResourceMap = Arc<Mutex<BTreeMap<TaskId, Vec<(ResourceKey, LockMode)>>>>;

/// Production Task DAG Scheduler Engine implementing the WorkScheduler seam (DAG-01 through DAG-06).
pub struct SchedulerEngine {
    pool: SqlitePool,
    task_graph_repo: Arc<SqliteTaskGraphRepository>,
    materializer: Arc<TaskGraphMaterializer>,
    reconciler: Arc<TaskGraphReconciler>,
    resource_manager: Arc<ResourceManager>,
    concurrency_limiter: Arc<Mutex<ConcurrencyLimiter>>,
    dispatch_queue: Arc<Mutex<PriorityDispatchQueue>>,
    active_graph: Arc<RwLock<Option<TaskGraph>>>,
    task_roles: Arc<Mutex<BTreeMap<TaskId, AgentRole>>>,
    task_resources: TaskResourceMap,
    held_guards: Arc<Mutex<BTreeMap<TaskId, ResourceLeaseGuard>>>,
    cancellation_tokens: Arc<Mutex<BTreeMap<TaskId, CancellationToken>>>,
    limits: ConcurrencyLimits,
    event_bus: Option<Arc<dyn EventBus>>,
    sequence_counter: AtomicU64,
}

impl SchedulerEngine {
    pub fn new(
        pool: SqlitePool,
        resource_manager: Arc<ResourceManager>,
        limits: ConcurrencyLimits,
        event_bus: Option<Arc<dyn EventBus>>,
    ) -> Self {
        let task_graph_repo = Arc::new(SqliteTaskGraphRepository::new(pool.clone()));
        let materializer = Arc::new(TaskGraphMaterializer::new(pool.clone()));
        let reconciler = Arc::new(TaskGraphReconciler::new(pool.clone()));

        Self {
            pool,
            task_graph_repo,
            materializer,
            reconciler,
            resource_manager,
            concurrency_limiter: Arc::new(Mutex::new(ConcurrencyLimiter::new())),
            dispatch_queue: Arc::new(Mutex::new(PriorityDispatchQueue::default())),
            active_graph: Arc::new(RwLock::new(None)),
            task_roles: Arc::new(Mutex::new(BTreeMap::new())),
            task_resources: Arc::new(Mutex::new(BTreeMap::new())),
            held_guards: Arc::new(Mutex::new(BTreeMap::new())),
            cancellation_tokens: Arc::new(Mutex::new(BTreeMap::new())),
            limits,
            event_bus,
            sequence_counter: AtomicU64::new(1),
        }
    }

    pub fn resource_manager(&self) -> &Arc<ResourceManager> {
        &self.resource_manager
    }

    pub async fn set_task_resources(
        &self,
        task_id: TaskId,
        resources: Vec<(ResourceKey, LockMode)>,
    ) {
        self.task_resources.lock().await.insert(task_id, resources);
    }

    pub async fn get_task_resources(&self, task_id: TaskId) -> Vec<(ResourceKey, LockMode)> {
        self.task_resources
            .lock()
            .await
            .get(&task_id)
            .cloned()
            .unwrap_or_default()
    }

    pub async fn ensure_active_graph_loaded(
        &self,
        mission_id: MissionId,
    ) -> Result<(), SchedulerError> {
        let mut active = self.active_graph.write().await;
        if active.as_ref().map(|g| g.mission_id) != Some(mission_id) {
            let loaded = self
                .task_graph_repo
                .get_active_graph(mission_id)
                .await
                .map_err(|e| SchedulerError::Failed(e.to_string()))?;
            *active = loaded;
        }
        Ok(())
    }

    /// Materialize a plan in the headless/workflow lane
    /// ([`crate::dag::materializer::MaterializationLane::HeadlessWorkflow`]).
    /// It materializes machine-generated plans owned by the calling lane
    /// (workflow definition acceptance, controller stage gating). Governed
    /// interactive execution MUST use
    /// `TaskGraphMaterializer::materialize_authorized` (runner ReadyToExecute
    /// arm, TUI bridge) and MUST NOT call this method.
    pub async fn materialize_plan(
        &self,
        mission_id: MissionId,
        plan: &CandidatePlan,
    ) -> Result<TaskGraphId, SchedulerError> {
        let graph = self
            .materializer
            .materialize(mission_id, plan)
            .await
            .map_err(|e| SchedulerError::Failed(e.to_string()))?;

        let graph_id = graph.id;
        let rev = graph.revision;
        let count = graph.tasks.len();

        let mut active = self.active_graph.write().await;
        *active = Some(graph.clone());
        drop(active);

        if let Some(ref bus) = self.event_bus {
            let seq = self.sequence_counter.fetch_add(1, Ordering::SeqCst);
            let _ = emit_graph_materialized(
                bus,
                seq,
                mission_id,
                graph_id,
                rev,
                count,
                graph.task_summaries(),
            )
            .await;
            for (&tier_idx, task_ids) in &graph.wave_tiers {
                let seq = self.sequence_counter.fetch_add(1, Ordering::SeqCst);
                let _ = emit_wave_tier(bus, seq, mission_id, graph_id, tier_idx, task_ids.clone())
                    .await;
            }
        }

        Ok(graph_id)
    }

    pub async fn reconcile_plan(
        &self,
        mission_id: MissionId,
        new_plan: &CandidatePlan,
    ) -> Result<(TaskGraphId, ReconciliationResult), SchedulerError> {
        self.ensure_active_graph_loaded(mission_id).await?;
        let old_graph = {
            let active = self.active_graph.read().await;
            active
                .clone()
                .ok_or_else(|| SchedulerError::Failed("No active graph to reconcile".into()))?
        };

        let (new_graph, summary) = self
            .reconciler
            .reconcile_with_summary(mission_id, &old_graph, new_plan)
            .await
            .map_err(|e| SchedulerError::Failed(e.to_string()))?;

        for cancel_id in &summary.running_tasks_to_cancel {
            let _ = self.cancel_task(*cancel_id).await;
        }

        let graph_id = new_graph.id;
        let rev = new_graph.revision;
        let count = new_graph.tasks.len();

        let mut active = self.active_graph.write().await;
        *active = Some(new_graph.clone());
        drop(active);

        if let Some(ref bus) = self.event_bus {
            let seq = self.sequence_counter.fetch_add(1, Ordering::SeqCst);
            let _ = emit_graph_materialized(
                bus,
                seq,
                mission_id,
                graph_id,
                rev,
                count,
                new_graph.task_summaries(),
            )
            .await;
        }

        Ok((graph_id, summary))
    }

    fn task_repo(&self) -> crate::persistence::sqlite::repositories::SqliteTaskRepository {
        crate::persistence::sqlite::repositories::SqliteTaskRepository::new(self.pool.clone())
    }

    pub async fn mark_task_completed(
        &self,
        task_id: TaskId,
        result: TaskResult,
    ) -> Result<(), SchedulerError> {
        let mut active_guard = self.active_graph.write().await;
        let graph = active_guard
            .as_mut()
            .ok_or_else(|| SchedulerError::Failed("No active graph".into()))?;

        let task = graph
            .tasks
            .get_mut(&task_id)
            .ok_or_else(|| SchedulerError::Failed(format!("Task {} not found", task_id)))?;

        let new_state = transition_task(task.status, TaskEvent::Complete)
            .map_err(|e| SchedulerError::Failed(e.to_string()))?;

        let now = Utc::now();
        task.status = new_state;
        task.result = Some(result.clone());
        task.completed_at = Some(now);

        // Release leases, concurrency reservation, and cancellation token
        self.held_guards.lock().await.remove(&task_id);
        if let Some(role) = self.task_roles.lock().await.remove(&task_id) {
            self.concurrency_limiter.lock().await.release(role);
        }
        self.cancellation_tokens.lock().await.remove(&task_id);

        self.task_repo()
            .mark_succeeded(task_id, &result)
            .await
            .map_err(|e| SchedulerError::Failed(e.to_string()))?;

        // Evaluate downstream dependents
        let mut newly_ready = Vec::new();
        if let Some(dependents) = graph.dependents_of.get(&task_id).cloned() {
            for dep_id in dependents {
                if graph.is_dependency_satisfied(dep_id) {
                    newly_ready.push(dep_id);
                }
            }
        }

        for dep_id in newly_ready {
            if let Some(dep_task) = graph.tasks.get_mut(&dep_id)
                && dep_task.status == TaskState::Pending
            {
                dep_task.status = TaskState::Ready;
                let _ = self.task_repo().mark_ready(dep_id).await;
            }
        }

        Ok(())
    }

    pub async fn mark_task_failed(
        &self,
        task_id: TaskId,
        error_summary: String,
        retryable: bool,
    ) -> Result<(), SchedulerError> {
        let mut active_guard = self.active_graph.write().await;
        let graph = active_guard
            .as_mut()
            .ok_or_else(|| SchedulerError::Failed("No active graph".into()))?;

        let task = graph
            .tasks
            .get_mut(&task_id)
            .ok_or_else(|| SchedulerError::Failed(format!("Task {} not found", task_id)))?;

        // Release held leases and worker slot
        self.held_guards.lock().await.remove(&task_id);
        if let Some(role) = self.task_roles.lock().await.remove(&task_id) {
            self.concurrency_limiter.lock().await.release(role);
        }
        self.cancellation_tokens.lock().await.remove(&task_id);

        let now = Utc::now();

        if retryable && task.can_retry() {
            // Retryable failure within retry envelope (D-07)
            task.record_retry();
            task.status = TaskState::Ready;

            self.task_repo()
                .mark_retry(task_id, task.retry_count)
                .await
                .map_err(|e| SchedulerError::Failed(e.to_string()))?;
        } else {
            // Non-recoverable or retries exhausted (D-05)
            task.status = TaskState::Failed;
            task.completed_at = Some(now);

            self.task_repo()
                .mark_failed(task_id, now)
                .await
                .map_err(|e| SchedulerError::Failed(e.to_string()))?;

            // Downstream failure invalidation across transitive closure (D-05)
            let mut queue = VecDeque::new();
            let mut visited = BTreeSet::new();
            queue.push_back(task_id);

            while let Some(current) = queue.pop_front() {
                if let Some(dependents) = graph.dependents_of.get(&current) {
                    for &dep in dependents {
                        if visited.insert(dep)
                            && let Some(dep_task) = graph.tasks.get_mut(&dep)
                            && !dep_task.status.is_terminal()
                        {
                            let reason = BlockingReason::PrerequisiteFailed {
                                failed_task_id: task_id,
                                error_summary: error_summary.clone(),
                            };
                            dep_task.status = TaskState::Blocked;
                            dep_task.blocking_reason = Some(reason.clone());

                            let _ = self.task_repo().mark_blocked(dep, &reason).await;

                            if let Some(ref bus) = self.event_bus {
                                let seq = self.sequence_counter.fetch_add(1, Ordering::SeqCst);
                                let _ = emit_task_blocked(
                                    bus,
                                    seq,
                                    graph.mission_id,
                                    dep,
                                    format!("Prerequisite {} failed: {}", task_id, error_summary),
                                )
                                .await;
                            }

                            queue.push_back(dep);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub async fn mark_task_needs_review(&self, task_id: TaskId) -> Result<(), SchedulerError> {
        let mut active_guard = self.active_graph.write().await;
        let graph = active_guard
            .as_mut()
            .ok_or_else(|| SchedulerError::Failed("No active graph".into()))?;

        let task = graph
            .tasks
            .get_mut(&task_id)
            .ok_or_else(|| SchedulerError::Failed(format!("Task {} not found", task_id)))?;

        let new_state = transition_task(task.status, TaskEvent::RequestReview)
            .map_err(|e| SchedulerError::Failed(e.to_string()))?;

        task.status = new_state;

        self.task_repo()
            .mark_needs_review(task_id)
            .await
            .map_err(|e| SchedulerError::Failed(e.to_string()))?;

        Ok(())
    }

    pub async fn review_task(
        &self,
        task_id: TaskId,
        approved: bool,
        reviewer: String,
    ) -> Result<(), SchedulerError> {
        if approved {
            self.mark_task_completed(
                task_id,
                TaskResult {
                    summary: format!("Approved by {}", reviewer),
                    output_artifacts: Vec::new(),
                    metadata: HashMap::new(),
                },
            )
            .await
        } else {
            self.mark_task_failed(task_id, format!("Rejected by {}", reviewer), true)
                .await
        }
    }

    pub async fn cancel_task(&self, task_id: TaskId) -> Result<(), SchedulerError> {
        if let Some(token) = self.cancellation_tokens.lock().await.remove(&task_id) {
            token.cancel();
        }

        self.held_guards.lock().await.remove(&task_id);
        if let Some(role) = self.task_roles.lock().await.remove(&task_id) {
            self.concurrency_limiter.lock().await.release(role);
        }

        let mut active_guard = self.active_graph.write().await;
        let graph = active_guard
            .as_mut()
            .ok_or_else(|| SchedulerError::Failed("No active graph".into()))?;

        if let Some(task) = graph.tasks.get_mut(&task_id)
            && !task.status.is_terminal()
        {
            task.status = TaskState::Cancelled;
            task.completed_at = Some(Utc::now());

            let _ = self.task_repo().mark_cancelled(task_id, Utc::now()).await;

            // Transitive dependents become blocked
            if let Some(dependents) = graph.dependents_of.get(&task_id) {
                for &dep in dependents {
                    if let Some(dep_task) = graph.tasks.get_mut(&dep)
                        && !dep_task.status.is_terminal()
                    {
                        let reason = BlockingReason::PrerequisiteFailed {
                            failed_task_id: task_id,
                            error_summary: "Prerequisite task cancelled".into(),
                        };
                        dep_task.status = TaskState::Blocked;
                        dep_task.blocking_reason = Some(reason.clone());
                        let _ = self.task_repo().mark_blocked(dep, &reason).await;
                    }
                }
            }
        }

        Ok(())
    }

    pub async fn cancel_mission(&self, _mission_id: MissionId) -> Result<(), SchedulerError> {
        let tokens = std::mem::take(&mut *self.cancellation_tokens.lock().await);
        for t in tokens.into_values() {
            t.cancel();
        }

        self.held_guards.lock().await.clear();
        *self.concurrency_limiter.lock().await = ConcurrencyLimiter::new();
        self.task_roles.lock().await.clear();

        let mut active_guard = self.active_graph.write().await;
        if let Some(ref mut graph) = *active_guard {
            for task in graph.tasks.values_mut() {
                if !task.status.is_terminal() {
                    task.status = TaskState::Cancelled;
                    task.completed_at = Some(Utc::now());
                    let _ = self.task_repo().mark_cancelled(task.id, Utc::now()).await;
                }
            }
        }

        Ok(())
    }

    pub async fn get_snapshot(
        &self,
        mission_id: MissionId,
    ) -> Result<SchedulerSnapshot, SchedulerError> {
        self.ensure_active_graph_loaded(mission_id).await?;
        let active_guard = self.active_graph.read().await;
        let graph = active_guard
            .as_ref()
            .ok_or_else(|| SchedulerError::Failed("No active graph".into()))?;

        let mut stats = QueueStatistics::default();
        let mut ready_task_ids = Vec::new();
        let mut running_task_ids = Vec::new();
        let mut blocked_task_ids = Vec::new();

        for task in graph.tasks.values() {
            match task.status {
                TaskState::Pending => stats.pending_count += 1,
                TaskState::Ready => {
                    stats.ready_count += 1;
                    ready_task_ids.push(task.id);
                }
                TaskState::Running => {
                    stats.running_count += 1;
                    running_task_ids.push(task.id);
                }
                TaskState::Blocked => {
                    stats.blocked_count += 1;
                    let reason = task
                        .blocking_reason
                        .as_ref()
                        .map(|r| format!("{:?}", r))
                        .unwrap_or_else(|| "Blocked".into());
                    blocked_task_ids.push((task.id, reason));
                }
                TaskState::Succeeded => stats.completed_count += 1,
                TaskState::Failed => stats.failed_count += 1,
                TaskState::Cancelled => stats.cancelled_count += 1,
                TaskState::Skipped => stats.skipped_count += 1,
                TaskState::NeedsReview => stats.needs_review_count += 1,
            }
        }

        let cpm = compute_cpm_for_graph(graph);
        let active_workers = self
            .concurrency_limiter
            .lock()
            .await
            .active_global_workers();

        let mut active_leases = Vec::new();
        for task_id in &running_task_ids {
            let leases = self.resource_manager.get_task_leases(*task_id);
            for l in leases {
                active_leases.push((
                    *l.lease_id.as_uuid(),
                    l.task_id,
                    l.key.canonical_id,
                    format!("{:?}", l.mode),
                ));
            }
        }

        if let Some(ref bus) = self.event_bus {
            let seq = self.sequence_counter.fetch_add(1, Ordering::SeqCst);
            let _ = emit_critical_path_recalculated(
                bus,
                seq,
                mission_id,
                graph.id,
                cpm.critical_tasks.clone(),
                cpm.total_projected_duration_secs,
            )
            .await;
        }

        Ok(SchedulerSnapshot {
            mission_id,
            graph_id: graph.id,
            revision: graph.revision,
            active_workers,
            max_workers: self.limits.max_global_workers,
            queue_stats: stats,
            ready_task_ids,
            running_task_ids,
            blocked_task_ids,
            active_leases,
            wave_tiers: graph.wave_tiers.clone(),
            critical_path: cpm.critical_tasks,
            projected_duration_secs: cpm.total_projected_duration_secs,
            schedule_drift_secs: 0,
            timestamp: Utc::now(),
        })
    }
}

#[async_trait]
impl WorkScheduler for SchedulerEngine {
    async fn find_ready_work(
        &self,
        mission_id: MissionId,
    ) -> Result<ReadyWorkResponse, SchedulerError> {
        self.ensure_active_graph_loaded(mission_id).await?;

        let mut active_guard = self.active_graph.write().await;
        let graph = match active_guard.as_mut() {
            Some(g) => g,
            None => {
                return Ok(ReadyWorkResponse {
                    ready_tasks: Vec::new(),
                    blocked_tasks_count: 0,
                });
            }
        };

        // 1. Advance Pending tasks whose hard prerequisites all Succeeded to Ready
        let mut newly_ready = Vec::new();
        for (task_id, task) in &graph.tasks {
            if task.status == TaskState::Pending && graph.is_dependency_satisfied(*task_id) {
                newly_ready.push(*task_id);
            }
        }

        for nid in newly_ready {
            if let Some(t) = graph.tasks.get_mut(&nid) {
                t.status = TaskState::Ready;
                let _ = self.task_repo().mark_ready(nid).await;
            }
        }

        // 2. Collect ready tasks and blocked tasks
        let ready_tasks: Vec<_> = graph
            .tasks
            .values()
            .filter(|t| t.status == TaskState::Ready)
            .cloned()
            .collect();
        let blocked_tasks_count = graph
            .tasks
            .values()
            .filter(|t| t.status == TaskState::Blocked)
            .count();

        if ready_tasks.is_empty() {
            return Ok(ReadyWorkResponse {
                ready_tasks: Vec::new(),
                blocked_tasks_count,
            });
        }

        // 3. Compute CPM critical path
        let cpm = compute_cpm_for_graph(graph);
        let critical_tasks_set: BTreeSet<TaskId> = cpm.critical_tasks.iter().copied().collect();
        drop(active_guard);

        // 4. Populate dispatch queue
        let mut queue = self.dispatch_queue.lock().await;
        for task in &ready_tasks {
            if queue.get(task.id).is_none() {
                let on_cp = critical_tasks_set.contains(&task.id);
                let reqs = self.get_task_resources(task.id).await;
                queue.push(QueueTaskEntry::new(
                    task.id,
                    task.priority,
                    on_cp,
                    reqs,
                    task.role.clone(),
                ));
            }
        }

        // 5. Select feasible tasks with queue bypass (D-02, D-10)
        let limiter = self.concurrency_limiter.lock().await;
        let mut dispatched = Vec::new();

        while dispatched.len() + limiter.active_global_workers() < self.limits.max_global_workers {
            let selected = queue.pop_highest_feasible(|entry| {
                let can_role = limiter
                    .can_reserve(entry.role.clone(), &self.limits)
                    .is_ok();
                let can_res = self
                    .resource_manager
                    .check_availability(&entry.required_resources, entry.task_id)
                    .is_ok();
                can_role && can_res
            });

            if let Some(entry) = selected {
                if let Some(t) = ready_tasks.iter().find(|t| t.id == entry.task_id) {
                    let mut req_caps: Vec<String> =
                        t.capabilities.iter().map(|c| c.id.clone()).collect();
                    let role_tag = format!("role:{}", t.role);
                    if !req_caps.contains(&role_tag) {
                        req_caps.push(role_tag);
                    }
                    dispatched.push(WorkItem {
                        task_id: entry.task_id,
                        title: t.title.clone(),
                        estimated_tokens: t.estimates.max_tokens,
                        required_capabilities: req_caps,
                        description: t.description.clone(),
                        completion_criteria: t.completion_criteria.clone(),
                        requirement_keys: t.requirement_keys.clone(),
                        assumptions: t.assumptions.clone(),
                        verification: Some(t.verification.clone()),
                        prompt_ref: t.prompt_ref.clone(),
                    });
                }
            } else {
                break;
            }
        }

        // Record bypass aging for any ready tasks that were not dispatched
        for task in &ready_tasks {
            if !dispatched.iter().any(|d| d.task_id == task.id) {
                queue.record_bypass(task.id);
            }
        }

        Ok(ReadyWorkResponse {
            ready_tasks: dispatched,
            blocked_tasks_count,
        })
    }

    async fn is_work_complete(&self, mission_id: MissionId) -> Result<bool, SchedulerError> {
        self.ensure_active_graph_loaded(mission_id).await?;
        let active = self.active_graph.read().await;
        match active.as_ref() {
            Some(g) => {
                let all_terminal = g.tasks.values().all(|t| t.status.is_terminal());
                Ok(all_terminal && !g.tasks.is_empty())
            }
            None => Ok(false),
        }
    }

    async fn mark_task_started(
        &self,
        task_id: TaskId,
        _agent_id: AgentId,
    ) -> Result<(), SchedulerError> {
        let (mission_id, role) = {
            let active_guard = self.active_graph.read().await;
            let graph = active_guard
                .as_ref()
                .ok_or_else(|| SchedulerError::Failed("No active graph".into()))?;

            let task = graph
                .tasks
                .get(&task_id)
                .ok_or_else(|| SchedulerError::Failed(format!("Task {} not found", task_id)))?;

            transition_task(task.status, TaskEvent::Start)
                .map_err(|e| SchedulerError::Failed(e.to_string()))?;

            (graph.mission_id, task.role.clone())
        };

        // 1. Reserve concurrency slot (D-10)
        self.concurrency_limiter
            .lock()
            .await
            .try_reserve(role.clone(), &self.limits)
            .map_err(|e| SchedulerError::Failed(e.to_string()))?;
        self.task_roles.lock().await.insert(task_id, role.clone());

        // 2. Acquire resource locks (D-01, D-04)
        let reqs = self.get_task_resources(task_id).await;
        let guard = match self.resource_manager.acquire(task_id, mission_id, reqs) {
            Ok(g) => g,
            Err(e) => {
                self.concurrency_limiter.lock().await.release(role);
                self.task_roles.lock().await.remove(&task_id);
                return Err(SchedulerError::Failed(e.to_string()));
            }
        };
        self.held_guards.lock().await.insert(task_id, guard);

        // 3. Register cancellation token (D-06, Law 8)
        self.cancellation_tokens
            .lock()
            .await
            .insert(task_id, CancellationToken::new());

        // 4. Update task state and started_at
        let now = Utc::now();
        {
            let mut active_guard = self.active_graph.write().await;
            let graph = active_guard
                .as_mut()
                .ok_or_else(|| SchedulerError::Failed("No active graph".into()))?;

            let task = graph
                .tasks
                .get_mut(&task_id)
                .ok_or_else(|| SchedulerError::Failed(format!("Task {} not found", task_id)))?;

            task.status = TaskState::Running;
            task.started_at = Some(now);
        }

        self.task_repo()
            .mark_running(task_id, now)
            .await
            .map_err(|e| SchedulerError::Failed(e.to_string()))?;

        // 5. Remove from dispatch queue
        self.dispatch_queue.lock().await.remove(task_id);

        Ok(())
    }

    async fn materialize_plan(
        &self,
        mission_id: MissionId,
        plan: &CandidatePlan,
    ) -> Result<TaskGraphId, SchedulerError> {
        Self::materialize_plan(self, mission_id, plan).await
    }

    async fn reconcile_plan(
        &self,
        mission_id: MissionId,
        plan: &CandidatePlan,
    ) -> Result<TaskGraphId, SchedulerError> {
        Self::reconcile_plan(self, mission_id, plan)
            .await
            .map(|(id, _)| id)
    }

    async fn mark_task_completed(
        &self,
        task_id: TaskId,
        result: Option<TaskResult>,
    ) -> Result<(), SchedulerError> {
        let res = result.unwrap_or_else(|| TaskResult::new("Task completed"));
        Self::mark_task_completed(self, task_id, res).await
    }

    async fn mark_task_failed(
        &self,
        task_id: TaskId,
        error_message: String,
        is_recoverable: bool,
    ) -> Result<(), SchedulerError> {
        Self::mark_task_failed(self, task_id, error_message, is_recoverable).await
    }
}

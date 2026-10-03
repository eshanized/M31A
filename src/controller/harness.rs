use async_trait::async_trait;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;

use crate::controller::dependencies::ControllerDependencies;
use crate::controller::{AutonomyController, LoopStage, StageOutcome};
use crate::events::bus::{BroadcastEventBus, EventBus};
use crate::ids::{AgentId, JobId, MissionId, TaskId};
use crate::kernel::seams::*;
use crate::state::intake::AutonomyMode;

#[derive(Default)]
pub struct MockPlanService {
    pub valid_plan: AtomicBool,
    pub calls: Mutex<Vec<String>>,
}

#[async_trait]
impl PlanService for MockPlanService {
    async fn has_valid_plan(&self, _mission_id: MissionId) -> Result<bool, PlanError> {
        self.calls.lock().unwrap().push("has_valid_plan".into());
        Ok(self.valid_plan.load(Ordering::SeqCst))
    }

    async fn generate_initial_plan(&self, _req: PlanRequest) -> Result<PlanResponse, PlanError> {
        self.calls
            .lock()
            .unwrap()
            .push("generate_initial_plan".into());
        self.valid_plan.store(true, Ordering::SeqCst);
        Ok(PlanResponse {
            plan_id: "plan-1".into(),
            task_count: 1,
            candidate_plan: crate::kernel::plan::CandidatePlan::new(
                "plan-1",
                "Execute Mission",
                vec![],
            ),
        })
    }

    async fn replan(&self, req: ReplanRequest) -> Result<ReplanResponse, PlanError> {
        self.calls.lock().unwrap().push("replan".into());
        Ok(ReplanResponse {
            new_plan_id: "plan-replan-1".into(),
            modified_tasks: vec![req.failed_task_id],
            candidate_plan: None,
        })
    }
}

pub struct MockWorkScheduler {
    pub ready_items: Mutex<Vec<WorkItem>>,
    pub started_tasks: Mutex<Vec<(TaskId, AgentId)>>,
    pub is_complete: AtomicBool,
    pub calls: Mutex<Vec<String>>,
}

impl Default for MockWorkScheduler {
    fn default() -> Self {
        Self {
            ready_items: Mutex::new(vec![WorkItem {
                task_id: TaskId::new(),
                title: "Default Task".into(),
                estimated_tokens: 500,
                required_capabilities: vec!["code".into()],
                description: None,
                completion_criteria: Vec::new(),
                requirement_keys: Vec::new(),
                assumptions: Vec::new(),
                verification: None,
            
                prompt_ref: None,}]),
            started_tasks: Mutex::new(Vec::new()),
            is_complete: AtomicBool::new(false),
            calls: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl WorkScheduler for MockWorkScheduler {
    async fn find_ready_work(
        &self,
        _mission_id: MissionId,
    ) -> Result<ReadyWorkResponse, SchedulerError> {
        self.calls.lock().unwrap().push("find_ready_work".into());
        let items = self.ready_items.lock().unwrap().clone();
        Ok(ReadyWorkResponse {
            ready_tasks: items,
            blocked_tasks_count: 0,
        })
    }

    async fn is_work_complete(&self, _mission_id: MissionId) -> Result<bool, SchedulerError> {
        self.calls.lock().unwrap().push("is_work_complete".into());
        Ok(self.is_complete.load(Ordering::SeqCst))
    }

    async fn mark_task_started(
        &self,
        task_id: TaskId,
        agent_id: AgentId,
    ) -> Result<(), SchedulerError> {
        self.calls.lock().unwrap().push("mark_task_started".into());
        self.started_tasks.lock().unwrap().push((task_id, agent_id));
        Ok(())
    }
}

pub struct MockPolicyGate {
    pub decision: Mutex<PolicyDecision>,
    pub evaluations: Mutex<Vec<PolicyEvaluationRequest>>,
}

impl Default for MockPolicyGate {
    fn default() -> Self {
        Self {
            decision: Mutex::new(PolicyDecision::Allow),
            evaluations: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl PolicyGate for MockPolicyGate {
    async fn evaluate(&self, req: PolicyEvaluationRequest) -> Result<PolicyDecision, PolicyError> {
        self.evaluations.lock().unwrap().push(req);
        Ok(*self.decision.lock().unwrap())
    }
}

pub struct MockContextCompiler {
    pub compilations: Mutex<Vec<ContextCompilationRequest>>,
}

impl Default for MockContextCompiler {
    fn default() -> Self {
        Self {
            compilations: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl ContextCompiler for MockContextCompiler {
    async fn compile_context(
        &self,
        req: ContextCompilationRequest,
    ) -> Result<CompiledContext, ContextError> {
        self.compilations.lock().unwrap().push(req);
        Ok(CompiledContext {
            context_id: "ctx-1".into(),
            token_count: 500,
            system_prompt: "You are an assistant".into(),
            manifest: None,
            messages: Vec::new(),
        
            prompt_provenance: None,})
    }
}

pub struct MockWorkerDispatcher {
    pub allocated_workers: Mutex<Vec<(TaskId, AgentId)>>,
    pub dispatched: Mutex<Vec<WorkExecutionRequest>>,
    pub last_handle: Mutex<Option<WorkExecutionHandle>>,
    pub result: Mutex<Option<WorkExecutionResult>>,
}

impl Default for MockWorkerDispatcher {
    fn default() -> Self {
        Self {
            allocated_workers: Mutex::new(Vec::new()),
            dispatched: Mutex::new(Vec::new()),
            last_handle: Mutex::new(None),
            result: Mutex::new(None),
        }
    }
}

#[async_trait]
impl WorkerDispatcher for MockWorkerDispatcher {
    async fn allocate_worker(
        &self,
        task_id: TaskId,
        _mission_id: MissionId,
        _capabilities: &[String],
    ) -> Result<AgentId, ExecutionError> {
        let agent_id = AgentId::new();
        self.allocated_workers
            .lock()
            .unwrap()
            .push((task_id, agent_id));
        Ok(agent_id)
    }

    async fn dispatch_work(
        &self,
        req: WorkExecutionRequest,
    ) -> Result<WorkExecutionHandle, ExecutionError> {
        self.dispatched.lock().unwrap().push(req.clone());
        let handle = WorkExecutionHandle {
            job_id: JobId::new(),
            task_id: req.task_id,
            agent_id: req.agent_id,
        };
        *self.last_handle.lock().unwrap() = Some(handle.clone());
        Ok(handle)
    }

    async fn collect_result(
        &self,
        handle: &WorkExecutionHandle,
    ) -> Result<WorkExecutionResult, ExecutionError> {
        if let Some(res) = self.result.lock().unwrap().clone() {
            Ok(res)
        } else {
            Ok(WorkExecutionResult {
                task_id: handle.task_id,
                success: true,
                output: "execution successful".into(),
                error_detail: None,
                token_usage: None,
            })
        }
    }
}

pub struct MockVerificationEngine {
    pub task_outcome: Mutex<VerificationOutcome>,
    pub gate_outcome: Mutex<CompletionGateOutcome>,
    pub verified_tasks: Mutex<Vec<TaskVerificationRequest>>,
}

impl Default for MockVerificationEngine {
    fn default() -> Self {
        Self {
            task_outcome: Mutex::new(VerificationOutcome::Passed),
            gate_outcome: Mutex::new(CompletionGateOutcome::Satisfied),
            verified_tasks: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl VerificationEngine for MockVerificationEngine {
    async fn verify_task(
        &self,
        req: TaskVerificationRequest,
    ) -> Result<VerificationOutcome, VerificationError> {
        self.verified_tasks.lock().unwrap().push(req);
        Ok(self.task_outcome.lock().unwrap().clone())
    }

    async fn verify_completion_gate(
        &self,
        _mission_id: MissionId,
    ) -> Result<CompletionGateOutcome, VerificationError> {
        Ok(self.gate_outcome.lock().unwrap().clone())
    }
}

pub struct MockRecoveryEngine {
    pub classification: Mutex<FailureClassification>,
    pub action: Mutex<RecoveryAction>,
    pub classified: Mutex<Vec<FailureClassificationRequest>>,
}

impl Default for MockRecoveryEngine {
    fn default() -> Self {
        Self {
            classification: Mutex::new(FailureClassification::Transient),
            action: Mutex::new(RecoveryAction::Retry { delay_ms: 100 }),
            classified: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl RecoveryEngine for MockRecoveryEngine {
    async fn classify_failure(
        &self,
        req: FailureClassificationRequest,
    ) -> Result<FailureClassification, RecoveryError> {
        self.classified.lock().unwrap().push(req);
        Ok(*self.classification.lock().unwrap())
    }

    async fn determine_recovery(
        &self,
        _req: RecoveryStrategyRequest,
    ) -> Result<RecoveryAction, RecoveryError> {
        Ok(self.action.lock().unwrap().clone())
    }
}

#[derive(Default)]
pub struct MockEscalationChannel {
    pub requests: Mutex<Vec<EscalationRequest>>,
    pub response: Mutex<Option<EscalationResponse>>,
}

#[async_trait]
impl EscalationChannel for MockEscalationChannel {
    async fn request_escalation(
        &self,
        req: EscalationRequest,
    ) -> Result<EscalationRequestId, EscalationError> {
        let id = req.id;
        self.requests.lock().unwrap().push(req);
        Ok(id)
    }

    async fn check_response(
        &self,
        _id: EscalationRequestId,
    ) -> Result<Option<EscalationResponse>, EscalationError> {
        Ok(self.response.lock().unwrap().clone())
    }
}

/// Hand-written deterministic mock test harness for Autonomy Controller seams (D-07).
pub struct MockControllerHarness {
    pub planner: Arc<MockPlanService>,
    pub scheduler: Arc<MockWorkScheduler>,
    pub policy: Arc<MockPolicyGate>,
    pub context: Arc<MockContextCompiler>,
    pub dispatcher: Arc<MockWorkerDispatcher>,
    pub verifier: Arc<MockVerificationEngine>,
    pub recovery: Arc<MockRecoveryEngine>,
    pub escalation: Arc<MockEscalationChannel>,
    pub event_bus: Arc<dyn EventBus>,
}

impl Default for MockControllerHarness {
    fn default() -> Self {
        Self::new()
    }
}

impl MockControllerHarness {
    pub fn new() -> Self {
        let planner = Arc::new(MockPlanService::default());
        planner.valid_plan.store(true, Ordering::SeqCst);

        Self {
            planner,
            scheduler: Arc::new(MockWorkScheduler::default()),
            policy: Arc::new(MockPolicyGate::default()),
            context: Arc::new(MockContextCompiler::default()),
            dispatcher: Arc::new(MockWorkerDispatcher::default()),
            verifier: Arc::new(MockVerificationEngine::default()),
            recovery: Arc::new(MockRecoveryEngine::default()),
            escalation: Arc::new(MockEscalationChannel::default()),
            event_bus: Arc::new(BroadcastEventBus::new(2048)),
        }
    }

    pub fn with_ready_work(self, items: Vec<WorkItem>) -> Self {
        *self.scheduler.ready_items.lock().unwrap() = items;
        self
    }

    pub fn with_no_work(self) -> Self {
        *self.scheduler.ready_items.lock().unwrap() = Vec::new();
        self
    }

    pub fn with_policy_decision(self, decision: PolicyDecision) -> Self {
        *self.policy.decision.lock().unwrap() = decision;
        self
    }

    pub fn with_verification_outcome(self, outcome: VerificationOutcome) -> Self {
        *self.verifier.task_outcome.lock().unwrap() = outcome;
        self
    }

    pub fn with_failure_classification(self, class: FailureClassification) -> Self {
        *self.recovery.classification.lock().unwrap() = class;
        self
    }

    pub fn dependencies(&self) -> ControllerDependencies {
        ControllerDependencies::new(
            self.planner.clone(),
            self.scheduler.clone(),
            self.policy.clone(),
            self.context.clone(),
            self.dispatcher.clone(),
            self.verifier.clone(),
            self.recovery.clone(),
            self.escalation.clone(),
        )
    }

    pub fn create_controller(
        &self,
        mission_id: MissionId,
        mode: AutonomyMode,
        cancellation_token: CancellationToken,
    ) -> AutonomyController {
        AutonomyController::new(
            mission_id,
            mode,
            self.dependencies(),
            self.event_bus.clone(),
            cancellation_token,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_controller_harness_happy_path() {
        let harness = MockControllerHarness::new();
        let mission_id = MissionId::new();
        let cancel = CancellationToken::new();
        let mut controller =
            harness.create_controller(mission_id, AutonomyMode::Autonomous, cancel);

        assert_eq!(controller.progress.cycle, 0);
        let outcome = controller.tick().await.unwrap();
        assert_eq!(outcome, StageOutcome::Advance(LoopStage::Observe));
        assert_eq!(controller.progress.cycle, 1);

        // Verify seam invocations
        assert!(
            harness
                .planner
                .calls
                .lock()
                .unwrap()
                .contains(&"has_valid_plan".to_string())
        );
        assert!(
            harness
                .scheduler
                .calls
                .lock()
                .unwrap()
                .contains(&"find_ready_work".to_string())
        );
        assert_eq!(harness.policy.evaluations.lock().unwrap().len(), 1);
        assert_eq!(harness.dispatcher.dispatched.lock().unwrap().len(), 1);
        assert_eq!(harness.verifier.verified_tasks.lock().unwrap().len(), 1);
    }
}

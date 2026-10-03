//! Phase 03: Autonomy Controller Contracts E2E / Integration Tests
//!
//! Verifies closed-loop lifecycle execution, multi-cycle completion, cooperative cancellation,
//! layered budget enforcement, sliding-window loop detection, fixed safety precedence,
//! cycle-boundary mode transitions, fail-safe unattended policy, and escalation timeouts.

use async_trait::async_trait;
use chrono::{Duration as ChronoDuration, Utc};
use futures::StreamExt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

use m31a::controller::AutonomyController;
use m31a::controller::budget_tracker::BudgetKind;
use m31a::controller::dependencies::ControllerDependencies;
use m31a::controller::error::ControllerError;
use m31a::controller::halting::{ControllerHaltReason, HaltEvaluation};
use m31a::controller::loop_detector::{LoopDetector, LoopSignature};
use m31a::controller::progress::LoopStage;
use m31a::controller::stage::StageOutcome;
use m31a::events::bus::{BroadcastEventBus, EventBus, EventFilter};
use m31a::events::types::EventType;
use m31a::ids::{AgentId, JobId, MissionId, TaskId};
use m31a::kernel::seams::*;
use m31a::state::budget::ResourceBudget;
use m31a::state::intake::AutonomyMode;

// --- Test Mocks for Seam Dependencies ---

#[derive(Default)]
struct TestPlanService {
    has_plan: AtomicBool,
    initial_plan_called: AtomicBool,
    replan_called: AtomicBool,
}

#[async_trait]
impl PlanService for TestPlanService {
    async fn has_valid_plan(&self, _mission_id: MissionId) -> Result<bool, PlanError> {
        Ok(self.has_plan.load(Ordering::SeqCst))
    }

    async fn generate_initial_plan(&self, _req: PlanRequest) -> Result<PlanResponse, PlanError> {
        self.initial_plan_called.store(true, Ordering::SeqCst);
        self.has_plan.store(true, Ordering::SeqCst);
        Ok(PlanResponse {
            plan_id: "plan-test-1".into(),
            task_count: 1,
            candidate_plan: m31a::kernel::plan::CandidatePlan::new(
                "plan-test-1",
                "Execute Mission",
                vec![],
            ),
        })
    }

    async fn replan(&self, req: ReplanRequest) -> Result<ReplanResponse, PlanError> {
        self.replan_called.store(true, Ordering::SeqCst);
        Ok(ReplanResponse {
            new_plan_id: "plan-replan-1".into(),
            modified_tasks: vec![req.failed_task_id],
            candidate_plan: None,
        })
    }
}

struct TestWorkScheduler {
    tasks: Mutex<Vec<WorkItem>>,
    is_complete: AtomicBool,
}

impl TestWorkScheduler {
    fn new(tasks: Vec<WorkItem>, is_complete: bool) -> Self {
        Self {
            tasks: Mutex::new(tasks),
            is_complete: AtomicBool::new(is_complete),
        }
    }
}

#[async_trait]
impl WorkScheduler for TestWorkScheduler {
    async fn find_ready_work(
        &self,
        _mission_id: MissionId,
    ) -> Result<ReadyWorkResponse, SchedulerError> {
        // Read semantics: peeking must be non-destructive. Tasks leave the
        // ready set only when leased via mark_task_started, mirroring the
        // production SchedulerEngine. (A pop-on-read mock would corrupt any
        // controller flow that legitimately polls scheduler state.)
        let guard = self.tasks.lock().unwrap();
        Ok(ReadyWorkResponse {
            ready_tasks: guard.clone(),
            blocked_tasks_count: 0,
        })
    }

    async fn is_work_complete(&self, _mission_id: MissionId) -> Result<bool, SchedulerError> {
        Ok(self.is_complete.load(Ordering::SeqCst))
    }

    async fn mark_task_started(
        &self,
        task_id: TaskId,
        _agent_id: AgentId,
    ) -> Result<(), SchedulerError> {
        let mut guard = self.tasks.lock().unwrap();
        guard.retain(|t| t.task_id != task_id);
        Ok(())
    }
}

struct TestPolicyGate {
    decision: Mutex<PolicyDecision>,
}

impl TestPolicyGate {
    fn new(decision: PolicyDecision) -> Self {
        Self {
            decision: Mutex::new(decision),
        }
    }
}

#[async_trait]
impl PolicyGate for TestPolicyGate {
    async fn evaluate(&self, _req: PolicyEvaluationRequest) -> Result<PolicyDecision, PolicyError> {
        Ok(*self.decision.lock().unwrap())
    }
}

#[derive(Default)]
struct TestContextCompiler;

#[async_trait]
impl ContextCompiler for TestContextCompiler {
    async fn compile_context(
        &self,
        _req: ContextCompilationRequest,
    ) -> Result<CompiledContext, ContextError> {
        Ok(CompiledContext {
            context_id: "ctx-test-1".into(),
            token_count: 500,
            system_prompt: "test context".into(),
            manifest: None,
            messages: Vec::new(),
        
            prompt_provenance: None,})
    }
}

struct TestWorkerDispatcher {
    execution_success: AtomicBool,
}

impl TestWorkerDispatcher {
    fn new(execution_success: bool) -> Self {
        Self {
            execution_success: AtomicBool::new(execution_success),
        }
    }
}

#[async_trait]
impl WorkerDispatcher for TestWorkerDispatcher {
    async fn allocate_worker(
        &self,
        _task_id: TaskId,
        _mission_id: MissionId,
        _capabilities: &[String],
    ) -> Result<AgentId, ExecutionError> {
        Ok(AgentId::new())
    }

    async fn dispatch_work(
        &self,
        req: WorkExecutionRequest,
    ) -> Result<WorkExecutionHandle, ExecutionError> {
        Ok(WorkExecutionHandle {
            job_id: JobId::new(),
            task_id: req.task_id,
            agent_id: req.agent_id,
        })
    }

    async fn collect_result(
        &self,
        handle: &WorkExecutionHandle,
    ) -> Result<WorkExecutionResult, ExecutionError> {
        let success = self.execution_success.load(Ordering::SeqCst);
        Ok(WorkExecutionResult {
            task_id: handle.task_id,
            success,
            output: if success {
                "success output".into()
            } else {
                "failure output".into()
            },
            error_detail: if success {
                None
            } else {
                Some("Task failed deterministically".into())
            },
            token_usage: None,
        })
    }
}

struct TestVerificationEngine {
    task_passed: AtomicBool,
    gate_satisfied: AtomicBool,
}

impl TestVerificationEngine {
    fn new(task_passed: bool, gate_satisfied: bool) -> Self {
        Self {
            task_passed: AtomicBool::new(task_passed),
            gate_satisfied: AtomicBool::new(gate_satisfied),
        }
    }
}

#[async_trait]
impl VerificationEngine for TestVerificationEngine {
    async fn verify_task(
        &self,
        _req: TaskVerificationRequest,
    ) -> Result<VerificationOutcome, VerificationError> {
        if self.task_passed.load(Ordering::SeqCst) {
            Ok(VerificationOutcome::Passed)
        } else {
            Ok(VerificationOutcome::Failed {
                reason: "task verification failed".into(),
            })
        }
    }

    async fn verify_completion_gate(
        &self,
        _mission_id: MissionId,
    ) -> Result<CompletionGateOutcome, VerificationError> {
        if self.gate_satisfied.load(Ordering::SeqCst) {
            Ok(CompletionGateOutcome::Satisfied)
        } else {
            Ok(CompletionGateOutcome::Deficient {
                violations: vec!["Remaining tasks pending".into()],
            })
        }
    }
}

struct TestRecoveryEngine {
    classification: Mutex<FailureClassification>,
    action: Mutex<RecoveryAction>,
}

impl TestRecoveryEngine {
    fn new(classification: FailureClassification, action: RecoveryAction) -> Self {
        Self {
            classification: Mutex::new(classification),
            action: Mutex::new(action),
        }
    }
}

#[async_trait]
impl RecoveryEngine for TestRecoveryEngine {
    async fn classify_failure(
        &self,
        _req: FailureClassificationRequest,
    ) -> Result<FailureClassification, RecoveryError> {
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
struct TestEscalationChannel {
    requested: AtomicBool,
}

#[async_trait]
impl EscalationChannel for TestEscalationChannel {
    async fn request_escalation(
        &self,
        _req: EscalationRequest,
    ) -> Result<EscalationRequestId, EscalationError> {
        self.requested.store(true, Ordering::SeqCst);
        Ok(EscalationRequestId::new())
    }

    async fn check_response(
        &self,
        _id: EscalationRequestId,
    ) -> Result<Option<EscalationResponse>, EscalationError> {
        Ok(None)
    }
}

// --- Helper to assemble ControllerDependencies ---
#[allow(clippy::too_many_arguments)]
fn create_test_dependencies(
    planner: Arc<dyn PlanService>,
    scheduler: Arc<dyn WorkScheduler>,
    policy: Arc<dyn PolicyGate>,
    context: Arc<dyn ContextCompiler>,
    dispatcher: Arc<dyn WorkerDispatcher>,
    verifier: Arc<dyn VerificationEngine>,
    recovery: Arc<dyn RecoveryEngine>,
    escalation: Arc<dyn EscalationChannel>,
) -> ControllerDependencies {
    ControllerDependencies::new(
        planner, scheduler, policy, context, dispatcher, verifier, recovery, escalation,
    )
}

// =========================================================================
// Integration Test Scenarios (9 E2E Scenarios)
// =========================================================================

/// Scenario 1: Full 12-stage cycle execution from Observe to Checkpoint with event broadcasting.
#[tokio::test]
async fn test_e2e_full_lifecycle_single_cycle_to_checkpoint() {
    // Arrange
    let event_bus = Arc::new(BroadcastEventBus::new(64));
    let mut sub = event_bus.subscribe(EventFilter::all()).await;

    let planner = Arc::new(TestPlanService {
        has_plan: AtomicBool::new(true),
        ..Default::default()
    });
    let task = WorkItem {
        task_id: TaskId::new(),
        title: "E2E Unit Task".into(),
        estimated_tokens: 100,
        required_capabilities: vec!["code_write".into()],
        description: None,
        completion_criteria: Vec::new(),
        requirement_keys: Vec::new(),
        assumptions: Vec::new(),
        verification: None,
    
        prompt_ref: None,};
    let scheduler = Arc::new(TestWorkScheduler::new(vec![task], false));
    let policy = Arc::new(TestPolicyGate::new(PolicyDecision::Allow));
    let context = Arc::new(TestContextCompiler);
    let dispatcher = Arc::new(TestWorkerDispatcher::new(true));
    let verifier = Arc::new(TestVerificationEngine::new(true, false));
    let recovery = Arc::new(TestRecoveryEngine::new(
        FailureClassification::Transient,
        RecoveryAction::Retry { delay_ms: 0 },
    ));
    let escalation = Arc::new(TestEscalationChannel::default());

    let deps = create_test_dependencies(
        planner, scheduler, policy, context, dispatcher, verifier, recovery, escalation,
    );

    let mission_id = MissionId::new();
    let cancel = CancellationToken::new();
    let mut controller = AutonomyController::new(
        mission_id,
        AutonomyMode::Autonomous,
        deps,
        event_bus.clone(),
        cancel,
    );

    // Act: execute a single bounded tick (covers all 12 stages to Checkpoint -> Observe)
    let outcome = controller.tick().await.expect("tick should succeed");

    // Assert: cycle advanced from 0 to 1 and reset to Observe
    assert_eq!(outcome, StageOutcome::Advance(LoopStage::Observe));
    assert_eq!(controller.progress.cycle, 1);
    assert_eq!(controller.progress.current_stage, LoopStage::Observe);

    // Verify events were published across event bus
    let mut event_names = Vec::new();
    while let Ok(Some(Ok(env))) = tokio::time::timeout(Duration::from_millis(30), sub.next()).await
    {
        event_names.push(env.event_type.name().to_string());
    }

    assert!(event_names.contains(&"ControllerCycleStarted".to_string()));
    assert!(event_names.contains(&"ControllerStageTransitioned".to_string()));
}

/// Scenario 2: Multi-cycle execution reaching completion gate and halting with MissionCompleted.
#[tokio::test]
async fn test_e2e_multi_cycle_mission_completion() {
    // Arrange
    let event_bus = Arc::new(BroadcastEventBus::new(64));
    let mut sub = event_bus.subscribe(EventFilter::all()).await;

    let planner = Arc::new(TestPlanService {
        has_plan: AtomicBool::new(true),
        ..Default::default()
    });
    // Cycle 0: one task; Cycle 1: empty work with is_complete = true
    let task = WorkItem {
        task_id: TaskId::new(),
        title: "Only Task".into(),
        estimated_tokens: 150,
        required_capabilities: vec![],
        description: None,
        completion_criteria: Vec::new(),
        requirement_keys: Vec::new(),
        assumptions: Vec::new(),
        verification: None,
    
        prompt_ref: None,};
    let scheduler = Arc::new(TestWorkScheduler::new(vec![task], true));
    let policy = Arc::new(TestPolicyGate::new(PolicyDecision::Allow));
    let context = Arc::new(TestContextCompiler);
    let dispatcher = Arc::new(TestWorkerDispatcher::new(true));
    // Verification: task passes, and when checked with no active task, completion gate is satisfied
    let verifier = Arc::new(TestVerificationEngine::new(true, true));
    let recovery = Arc::new(TestRecoveryEngine::new(
        FailureClassification::Transient,
        RecoveryAction::Retry { delay_ms: 0 },
    ));
    let escalation = Arc::new(TestEscalationChannel::default());

    let deps = create_test_dependencies(
        planner, scheduler, policy, context, dispatcher, verifier, recovery, escalation,
    );

    let mission_id = MissionId::new();
    let cancel = CancellationToken::new();
    let mut controller = AutonomyController::new(
        mission_id,
        AutonomyMode::Autonomous,
        deps,
        event_bus.clone(),
        cancel,
    );

    // Act: run controller loop to terminal outcome
    let halt_reason = controller
        .run()
        .await
        .expect("controller run should finish");

    // Assert: controller halts with MissionCompleted
    assert_eq!(halt_reason, ControllerHaltReason::MissionCompleted);

    // Verify ControllerHalted event was published
    let mut halted_event_found = false;
    while let Ok(Some(Ok(env))) = tokio::time::timeout(Duration::from_millis(30), sub.next()).await
    {
        if env.event_type.name() == "ControllerHalted" {
            halted_event_found = true;
            break;
        }
    }
    assert!(
        halted_event_found,
        "ControllerHalted event should be emitted on completion"
    );
}

/// Scenario 3: Cooperative cancellation via CancellationToken interrupting active run() loop.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_e2e_cancellation_cooperative_halt() {
    // Arrange
    let event_bus = Arc::new(BroadcastEventBus::new(64));
    let planner = Arc::new(TestPlanService {
        has_plan: AtomicBool::new(true),
        ..Default::default()
    });
    // Scheduler yields (no work ready and work not complete)
    let scheduler = Arc::new(TestWorkScheduler::new(vec![], false));
    let policy = Arc::new(TestPolicyGate::new(PolicyDecision::Allow));
    let context = Arc::new(TestContextCompiler);
    let dispatcher = Arc::new(TestWorkerDispatcher::new(true));
    let verifier = Arc::new(TestVerificationEngine::new(true, false));
    let recovery = Arc::new(TestRecoveryEngine::new(
        FailureClassification::Transient,
        RecoveryAction::Retry { delay_ms: 10 },
    ));
    let escalation = Arc::new(TestEscalationChannel::default());

    let deps = create_test_dependencies(
        planner, scheduler, policy, context, dispatcher, verifier, recovery, escalation,
    );

    let mission_id = MissionId::new();
    let cancel = CancellationToken::new();
    let mut controller = AutonomyController::new(
        mission_id,
        AutonomyMode::Autonomous,
        deps,
        event_bus,
        cancel.clone(),
    );

    // Spawn async task to cancel after a brief pause
    let cancel_clone = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(15)).await;
        cancel_clone.cancel();
    });

    // Act
    let halt_reason = controller
        .run()
        .await
        .expect("run should handle cancellation");

    // Assert
    assert_eq!(halt_reason, ControllerHaltReason::Cancelled);
}

/// Scenario 4: Layered budget enforcement halts at Observe (wall-clock) and ValidatePolicyAndResources (tokens).
#[tokio::test]
async fn test_e2e_budget_exhaustion_wall_clock_and_tokens() {
    let event_bus = Arc::new(BroadcastEventBus::new(64));
    let planner = Arc::new(TestPlanService {
        has_plan: AtomicBool::new(true),
        ..Default::default()
    });
    let task = WorkItem {
        task_id: TaskId::new(),
        title: "Expensive Task".into(),
        estimated_tokens: 5_000,
        required_capabilities: vec![],
        description: None,
        completion_criteria: Vec::new(),
        requirement_keys: Vec::new(),
        assumptions: Vec::new(),
        verification: None,
    
        prompt_ref: None,};
    let scheduler = Arc::new(TestWorkScheduler::new(vec![task], false));
    let policy = Arc::new(TestPolicyGate::new(PolicyDecision::Allow));
    let context = Arc::new(TestContextCompiler);
    let dispatcher = Arc::new(TestWorkerDispatcher::new(true));
    let verifier = Arc::new(TestVerificationEngine::new(true, false));
    let recovery = Arc::new(TestRecoveryEngine::new(
        FailureClassification::Transient,
        RecoveryAction::Retry { delay_ms: 0 },
    ));
    let escalation = Arc::new(TestEscalationChannel::default());

    let deps = create_test_dependencies(
        planner, scheduler, policy, context, dispatcher, verifier, recovery, escalation,
    );

    let mission_id = MissionId::new();

    // 4a: Wall-clock budget exceeded at Observe
    let limits_clock = ResourceBudget {
        max_wall_clock_seconds: Some(10),
        ..Default::default()
    };
    let mut ctrl_clock = AutonomyController::with_budget(
        mission_id,
        AutonomyMode::Autonomous,
        deps.clone(),
        limits_clock,
        None,
        event_bus.clone(),
        CancellationToken::new(),
    );
    ctrl_clock.budget_tracker.record_elapsed_seconds(10);
    let outcome = ctrl_clock.step().await.unwrap();
    assert_eq!(
        outcome,
        StageOutcome::Halt(ControllerHaltReason::BudgetExhausted {
            kind: BudgetKind::WallClock,
        })
    );

    // 4b: Tokens budget exceeded at ValidatePolicyAndResources
    let limits_tokens = ResourceBudget {
        max_tokens: Some(2_000),
        ..Default::default()
    };
    let mut ctrl_tokens = AutonomyController::with_budget(
        mission_id,
        AutonomyMode::Autonomous,
        deps,
        limits_tokens,
        None,
        event_bus,
        CancellationToken::new(),
    );
    // Step from Observe -> IdentifyReadyWork
    ctrl_tokens.step().await.unwrap();
    // Step from IdentifyReadyWork -> ValidatePolicyAndResources
    ctrl_tokens.step().await.unwrap();
    // ValidatePolicyAndResources checks 5_000 tokens against limit 2_000 -> Halts
    let outcome_tokens = ctrl_tokens.step().await.unwrap();
    assert_eq!(
        outcome_tokens,
        StageOutcome::Halt(ControllerHaltReason::BudgetExhausted {
            kind: BudgetKind::Tokens,
        })
    );
}

/// Scenario 5: Sliding-window loop detector trips on repeating failure cycles without progress.
#[tokio::test]
async fn test_e2e_sliding_window_loop_detection() {
    let task_id = TaskId::new();

    // LoopDetector with capacity 20 and threshold 3
    let mut detector = LoopDetector::new(20, 3);

    let sig = LoopSignature {
        task_id: Some(task_id),
        failure_class: "Transient".into(),
        recovery_strategy: "determine_recovery".into(),
        stage: LoopStage::ClassifyFailure,
        progress_fingerprint: 0,
    };

    assert!(!detector.record(sig.clone()));
    assert!(!detector.record(sig.clone()));
    // 3rd repetition with same fingerprint trips!
    assert!(detector.record(sig.clone()));

    // Progress advancement resets loop count
    let mut detector_progress = LoopDetector::new(20, 3);
    assert!(!detector_progress.record(sig.clone()));
    assert!(!detector_progress.record(sig.clone()));
    let sig_advanced = LoopSignature {
        progress_fingerprint: 1, // Progress made!
        ..sig
    };
    assert!(!detector_progress.record(sig_advanced));
}

/// Scenario 6: Fixed safety precedence in halting resolver (FatalError > Cancelled > ... > MissionCompleted).
#[tokio::test]
async fn test_e2e_fixed_safety_precedence_in_controller() {
    let dummy_sig = LoopSignature {
        task_id: None,
        failure_class: "err".into(),
        recovery_strategy: "retry".into(),
        stage: LoopStage::Observe,
        progress_fingerprint: 0,
    };

    // Arrange: multiple simultaneous conditions
    let conditions = vec![
        ControllerHaltReason::MissionCompleted,
        ControllerHaltReason::Blocked,
        ControllerHaltReason::BudgetExhausted {
            kind: BudgetKind::CostUsd,
        },
        ControllerHaltReason::Cancelled,
        ControllerHaltReason::FatalError {
            failure_class: "SecurityViolation".into(),
        },
        ControllerHaltReason::LoopDetected {
            signature: dummy_sig,
        },
    ];

    // Act
    let eval = HaltEvaluation::new(conditions.clone()).expect("evaluation should succeed");

    // Assert: FatalError (rank 0) strictly wins over Cancelled (rank 1) and all others
    assert_eq!(
        eval.primary,
        ControllerHaltReason::FatalError {
            failure_class: "SecurityViolation".into(),
        }
    );
    assert_eq!(eval.contributing_conditions.len(), 6);
}

/// Scenario 7: Dynamic autonomy mode transitions strictly enforced at LoopStage::Observe cycle boundary.
#[tokio::test]
async fn test_e2e_set_autonomy_mode_cycle_boundary_enforcement() {
    let event_bus = Arc::new(BroadcastEventBus::new(64));
    let mut sub = event_bus.subscribe(EventFilter::all()).await;

    let planner = Arc::new(TestPlanService {
        has_plan: AtomicBool::new(true),
        ..Default::default()
    });
    let task = WorkItem {
        task_id: TaskId::new(),
        title: "Mode Task".into(),
        estimated_tokens: 100,
        required_capabilities: vec![],
        description: None,
        completion_criteria: Vec::new(),
        requirement_keys: Vec::new(),
        assumptions: Vec::new(),
        verification: None,
    
        prompt_ref: None,};
    let scheduler = Arc::new(TestWorkScheduler::new(vec![task], false));
    let policy = Arc::new(TestPolicyGate::new(PolicyDecision::Allow));
    let context = Arc::new(TestContextCompiler);
    let dispatcher = Arc::new(TestWorkerDispatcher::new(true));
    let verifier = Arc::new(TestVerificationEngine::new(true, false));
    let recovery = Arc::new(TestRecoveryEngine::new(
        FailureClassification::Transient,
        RecoveryAction::Retry { delay_ms: 0 },
    ));
    let escalation = Arc::new(TestEscalationChannel::default());

    let deps = create_test_dependencies(
        planner, scheduler, policy, context, dispatcher, verifier, recovery, escalation,
    );

    let mission_id = MissionId::new();
    let mut controller = AutonomyController::new(
        mission_id,
        AutonomyMode::Safe,
        deps,
        event_bus,
        CancellationToken::new(),
    );

    // 7a: Allowed at Observe boundary
    assert_eq!(controller.progress.current_stage, LoopStage::Observe);
    assert!(
        controller
            .set_autonomy_mode(AutonomyMode::Unattended)
            .await
            .is_ok()
    );
    assert_eq!(controller.mode, AutonomyMode::Unattended);

    // Verify ControllerDecisionModeChanged was emitted
    let mut mode_changed_event = false;
    while let Ok(Some(Ok(env))) = tokio::time::timeout(Duration::from_millis(30), sub.next()).await
    {
        if let EventType::ControllerDecisionModeChanged { from, to, .. } = env.event_type {
            assert_eq!(from, AutonomyMode::Safe);
            assert_eq!(to, AutonomyMode::Unattended);
            mode_changed_event = true;
            break;
        }
    }
    assert!(mode_changed_event);

    // Advance to IdentifyReadyWork
    controller.step().await.unwrap();
    assert_eq!(
        controller.progress.current_stage,
        LoopStage::IdentifyReadyWork
    );

    // 7b: Rejected mid-cycle
    let err = controller.set_autonomy_mode(AutonomyMode::Autonomous).await;
    assert!(matches!(
        err,
        Err(ControllerError::InvalidStageTransition { .. })
    ));
    assert_eq!(controller.mode, AutonomyMode::Unattended);
}

/// Scenario 8: Unattended mode converts unresolved ASK to DENY or Escalate, NEVER implicit Allow.
#[tokio::test]
async fn test_e2e_unattended_mode_escalation_and_deny() {
    // When escalation channel is available: converts ASK to Escalate
    let action_with_channel = resolve_decision(
        AutonomyMode::Unattended,
        PolicyDecision::Ask,
        "destructive delete",
        true,
    );
    assert_eq!(
        action_with_channel,
        ResolvedAction::Escalate {
            reason: "destructive delete".into(),
        }
    );

    // When NO escalation channel is available: converts ASK to Deny
    let action_without_channel = resolve_decision(
        AutonomyMode::Unattended,
        PolicyDecision::Ask,
        "destructive delete",
        false,
    );
    assert_eq!(
        action_without_channel,
        ResolvedAction::Deny {
            reason: "Unattended mode converts unresolved ASK to DENY".into(),
        }
    );

    // Invariant: Unattended on Ask NEVER evaluates to Proceed
    assert_ne!(action_with_channel, ResolvedAction::Proceed);
    assert_ne!(action_without_channel, ResolvedAction::Proceed);
}

/// Scenario 9: Operator escalation timeout generating EscalationTimedOut and effective DENY.
#[tokio::test]
async fn test_e2e_escalation_timeout_failsafe_deny() {
    let start = Utc::now();
    let req = EscalationRequest {
        id: EscalationRequestId::new(),
        mission_id: MissionId::new(),
        task_id: Some(TaskId::new()),
        reason: "Root filesystem modification requested".into(),
        timeout_seconds: Some(30),
    };

    let pending = PendingEscalation::new(req.clone(), start);

    // Before timeout (29 seconds elapsed) -> false
    assert!(!pending.evaluate_timeout(start + ChronoDuration::seconds(29)));

    // At/after timeout (30 seconds elapsed) -> true
    assert!(pending.evaluate_timeout(start + ChronoDuration::seconds(30)));
    assert!(pending.evaluate_timeout(start + ChronoDuration::seconds(60)));

    // Resolving timeout produces effective DENY + EscalationTimedOut event
    let (decision, event) = pending.resolve_timeout();
    assert_eq!(decision, PolicyDecision::Deny);
    assert_eq!(
        event,
        EventType::EscalationTimedOut {
            request_id: req.id.to_string(),
            mission_id: req.mission_id,
        }
    );
}

#[tokio::test]
async fn test_e2e_controller_persistence_and_durable_checkpoint_restart() {
    use m31a::persistence::sqlite::initialize_database;
    use m31a::persistence::sqlite::repositories::SqliteMissionRepository;
    use m31a::persistence::sqlite::transaction::SqliteTransactionManager;
    use m31a::state::Mission;
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let db_path = dir.path().join("controller_persistence.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let mission_id = MissionId::new();
    let mission = Mission::new(mission_id, "Build M31A Core".to_string());
    let mission_repo = Arc::new(SqliteMissionRepository::new(pool.clone()));
    mission_repo.insert(&mission).await.unwrap();
    let tx_manager = Arc::new(SqliteTransactionManager::new(pool.clone()));

    let task_id = TaskId::new();
    let item = WorkItem {
        task_id,
        title: "Task 1".into(),
        estimated_tokens: 100,
        required_capabilities: vec!["fs.read".into()],
        description: None,
        completion_criteria: Vec::new(),
        requirement_keys: Vec::new(),
        assumptions: Vec::new(),
        verification: None,
    
        prompt_ref: None,};

    // Insert task into SQLite tasks table so UpdateState can update it
    let now_str = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO tasks (id, mission_id, title, status, created_at, updated_at) VALUES (?, ?, ?, 'ready', ?, ?)"
    )
    .bind(task_id.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind("Task 1")
    .bind(&now_str)
    .bind(&now_str)
    .execute(&pool)
    .await
    .unwrap();

    let scheduler = Arc::new(TestWorkScheduler::new(vec![item], false));
    let planner = Arc::new(TestPlanService::default());
    let policy = Arc::new(TestPolicyGate::new(PolicyDecision::Allow));
    let context = Arc::new(TestContextCompiler);
    let dispatcher = Arc::new(TestWorkerDispatcher::new(true));
    let verifier = Arc::new(TestVerificationEngine::new(true, true));
    let recovery = Arc::new(TestRecoveryEngine::new(
        FailureClassification::Transient,
        RecoveryAction::AbortMission {
            reason: "err".into(),
        },
    ));
    let escalation = Arc::new(TestEscalationChannel::default());

    let deps = ControllerDependencies::new(
        planner, scheduler, policy, context, dispatcher, verifier, recovery, escalation,
    )
    .with_persistence(tx_manager.clone(), mission_repo.clone());

    let bus = Arc::new(BroadcastEventBus::new(100));
    let token = CancellationToken::new();

    let mut controller = AutonomyController::new(
        mission_id,
        AutonomyMode::Safe,
        deps.clone(),
        bus.clone(),
        token.clone(),
    );

    // 1. Run cycle 1 until Checkpoint
    let outcome = controller.tick().await.unwrap();
    assert_eq!(outcome, StageOutcome::Advance(LoopStage::Observe));
    assert_eq!(controller.progress.cycle, 1);

    // 2. Verify durable checkpoint was persisted to SQLite (PST-01, PST-06, FINDING-05)
    let checkpoints: Vec<(String, i64)> =
        sqlx::query_as("SELECT stage, cycle FROM checkpoints WHERE mission_id = ?")
            .bind(mission_id.as_bytes().as_slice())
            .fetch_all(&pool)
            .await
            .unwrap();

    assert_eq!(
        checkpoints.len(),
        1,
        "exactly one checkpoint must be written"
    );
    assert_eq!(checkpoints[0].1, 1, "checkpoint cycle must match cycle 1");

    // 3. Verify task status was updated in SQLite
    let task_status: (String,) = sqlx::query_as("SELECT status FROM tasks WHERE id = ?")
        .bind(task_id.as_bytes().as_slice())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(task_status.0, "running");

    // 4. Simulate a process crash and recovery by instantiating a fresh controller
    let mut recovered_controller =
        AutonomyController::new(mission_id, AutonomyMode::Safe, deps, bus, token);
    assert_eq!(recovered_controller.progress.cycle, 0);

    let restored = recovered_controller
        .restore_from_checkpoint()
        .await
        .unwrap();
    assert!(restored, "must successfully restore from SQLite checkpoint");
    assert_eq!(
        recovered_controller.progress.cycle, 1,
        "recovered controller must resume at cycle 1 without re-running completed work"
    );
}

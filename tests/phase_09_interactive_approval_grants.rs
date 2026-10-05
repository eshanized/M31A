//! Phase 9: Durable Policy Auditing, Interactive Approval Coordinator & Session Grants Tests (POL-01, POL-03, POL-05, D-04, D-09..D-12).

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use tempfile::tempdir;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use m31a::agent::runner::ActionRequest;
use m31a::capability::family::CapabilityFamily;
use m31a::capability::instance::CapabilityInstance;
use m31a::capability::permissions::CapabilityPermissions;
use m31a::capability::registry::CapabilityRegistry;
use m31a::ids::{AgentId, ApprovalRequestId, MissionId, TaskId, ToolCallId};
use m31a::kernel::seams::policy::PolicyDecisionContract;
use m31a::kernel::seams::policy::{PolicyDecision, PolicyEvaluationRequest, PolicyGate};
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::pipeline::runner::ToolPipelineRunner;
use m31a::policy::approval::channel::{ApprovalChannel, ApprovalError};
use m31a::policy::approval::coordinator::ApprovalCoordinator;
use m31a::policy::approval::explanation::ApprovalExplanationPacket;
use m31a::policy::approval::grant::{PolicyGrant, PolicyGrantStore};
use m31a::policy::approval::{ApprovalAction, ApprovalRequest, ApprovalResolutionScope};
use m31a::policy::audit::DurablePolicyAuditor;
use m31a::policy::effective::PolicyDecisionRecord;
use m31a::policy::layers::PolicyLayer;
use m31a::state::intake::AutonomyMode;
use m31a::tools::definition::{ResourceLimits, ToolExecutionContext, TypedTool};
use m31a::tools::error::ToolError as InternalToolError;
use m31a::tools::registry::ToolRegistry;
use m31a::tools::risk::RiskClass;

// ============================================================================
// Mock Channel & Tools
// ============================================================================

#[derive(Default, Clone)]
struct MockApprovalChannel {
    notified: Arc<Mutex<Vec<ApprovalExplanationPacket>>>,
}

#[async_trait]
impl ApprovalChannel for MockApprovalChannel {
    async fn notify_request(
        &self,
        packet: &ApprovalExplanationPacket,
    ) -> Result<(), ApprovalError> {
        let mut list = self.notified.lock().await;
        list.push(packet.clone());
        Ok(())
    }

    async fn poll_response(
        &self,
        _id: ApprovalRequestId,
    ) -> Result<Option<ApprovalAction>, ApprovalError> {
        Ok(None)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct EchoInput {
    pub message: String,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct EchoOutput {
    pub output: String,
}

struct EchoTool {
    counter: Arc<AtomicU32>,
}

#[async_trait]
impl TypedTool for EchoTool {
    type Input = EchoInput;
    type Output = EchoOutput;

    fn id(&self) -> &str {
        "echo_tool"
    }

    fn description(&self) -> &str {
        "Test echo tool"
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Filesystem]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::LowRiskMutation
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(10, 1024)
    }

    async fn execute(
        &self,
        _ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, InternalToolError> {
        self.counter.fetch_add(1, Ordering::SeqCst);
        Ok(EchoOutput {
            output: input.message,
        })
    }
}

struct AskPolicyGate;

#[async_trait]
impl PolicyGate for AskPolicyGate {
    async fn evaluate(
        &self,
        _req: PolicyEvaluationRequest,
    ) -> Result<PolicyDecision, m31a::kernel::seams::policy::PolicyError> {
        Ok(PolicyDecision::Ask)
    }

    async fn evaluate_record(
        &self,
        _req: PolicyEvaluationRequest,
    ) -> Result<
        (PolicyDecision, Option<PolicyDecisionContract>),
        m31a::kernel::seams::policy::PolicyError,
    > {
        let rec = PolicyDecisionRecord {
            decision: PolicyDecision::Ask,
            matched_rule_id: Some("ask-rule".to_string()),
            matched_layer: Some(PolicyLayer::Workspace),
            precedence_rank: Some(PolicyLayer::Workspace.precedence_rank()),
            authority_source: PolicyLayer::Workspace.authority_source().to_string(),
            explanation: "Mutating operations require confirmation".to_string(),
            policy_version_or_hash: "hash123".to_string(),
        };
        Ok((PolicyDecision::Ask, Some(rec.into())))
    }
}

// ============================================================================
// Helper: Seed foreign key dependencies for SQLite
// ============================================================================

async fn seed_mission(pool: &sqlx::SqlitePool, mission_id: MissionId) {
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?)"
    )
    .bind(mission_id.as_bytes().as_slice())
    .bind("Testing policy auditing")
    .bind("planning")
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_agent(pool: &sqlx::SqlitePool, mission_id: MissionId, agent_id: AgentId) {
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO agents (id, mission_id, role, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)"
    )
    .bind(agent_id.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind("implementer")
    .bind("active")
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_task(pool: &sqlx::SqlitePool, mission_id: MissionId, task_id: TaskId) {
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO tasks (id, mission_id, title, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)"
    )
    .bind(task_id.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind("Test Task")
    .bind("running")
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_approval_request(
    pool: &sqlx::SqlitePool,
    request_id: m31a::ids::ApprovalRequestId,
    mission_id: MissionId,
    task_id: TaskId,
    policy_hash: &str,
) {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO approval_requests (id, mission_id, task_id, tool_call_id, tool_or_capability, normalized_args_json, redacted_args_json, affected_resources, risk_classification, policy_hash, reason, resolution_state, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(request_id.as_bytes().to_vec())
    .bind(mission_id.as_bytes().as_slice())
    .bind(task_id.as_bytes().as_slice())
    .bind("call-1")
    .bind("write_file")
    .bind("{}")
    .bind("{}")
    .bind("[]")
    .bind("low")
    .bind(policy_hash)
    .bind("test")
    .bind("allowed")
    .bind(&now)
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_mission_and_task(
    pool: &sqlx::SqlitePool,
    mission_id: MissionId,
    task_id: TaskId,
    agent_id: AgentId,
) {
    seed_mission(pool, mission_id).await;
    seed_agent(pool, mission_id, agent_id).await;
    seed_task(pool, mission_id, task_id).await;
}

// ============================================================================
// Task 09-02-01: Durable Pre-Execution Policy Audit Store
// ============================================================================

#[tokio::test]
async fn test_durable_decision_audit() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("audit_test.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let mission_id = MissionId::new();
    let task_id = TaskId::new();
    let agent_id = AgentId::new();
    seed_mission_and_task(&pool, mission_id, task_id, agent_id).await;

    let auditor = DurablePolicyAuditor::new();
    let record = PolicyDecisionRecord {
        decision: PolicyDecision::Allow,
        matched_rule_id: Some("rule-allow-fs".to_string()),
        matched_layer: Some(PolicyLayer::Workspace),
        precedence_rank: Some(3),
        authority_source: "workspace policy".to_string(),
        explanation: "Allowed within workspace".to_string(),
        policy_version_or_hash: "sha256-active-hash-1234".to_string(),
    };

    let args = serde_json::json!({
        "path": "src/main.rs",
        "content": "fn main() {}"
    });

    // 1. Record decision
    let eval_id = auditor
        .record_domain_decision(
            &pool,
            mission_id,
            Some(task_id),
            Some(agent_id),
            Some("call-1"),
            "write_file",
            &record,
            &args,
            "src/main.rs",
        )
        .await
        .expect("record_decision should succeed");

    // 2. Fetch recorded decision by ID
    let fetched = auditor
        .get_decision(&pool, eval_id)
        .await
        .expect("get_decision should succeed")
        .expect("record must exist");

    assert_eq!(fetched.id, eval_id);
    assert_eq!(fetched.mission_id, mission_id);
    assert_eq!(fetched.task_id, Some(task_id));
    assert_eq!(fetched.agent_id, Some(agent_id));
    assert_eq!(fetched.tool_or_capability, "write_file");
    assert_eq!(fetched.matched_rule_id.as_deref(), Some("rule-allow-fs"));
    assert_eq!(fetched.matched_layer, "workspace");
    assert_eq!(fetched.precedence_rank, 3);
    assert_eq!(fetched.decision, "allow");
    assert_eq!(fetched.resource_scope, "src/main.rs");
    assert!(!fetched.normalized_args_hash.is_empty());

    // 3. List decisions for mission
    let list = auditor
        .list_decisions_for_mission(&pool, mission_id)
        .await
        .unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, eval_id);
}

// ============================================================================
// Task 09-02-02: Concurrent ApprovalCoordinator & Unattended Fail-Closed
// ============================================================================

#[tokio::test]
async fn test_approval_coordinator_concurrency() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("coord_test.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let channel = Arc::new(MockApprovalChannel::default());
    let coordinator = ApprovalCoordinator::new(Some(pool.clone()), Some(channel.clone()));

    let mission_id = MissionId::new();
    let task_1 = TaskId::new();
    let task_2 = TaskId::new();
    let agent_id = AgentId::new();
    seed_mission(&pool, mission_id).await;
    seed_agent(&pool, mission_id, agent_id).await;
    seed_task(&pool, mission_id, task_1).await;
    seed_task(&pool, mission_id, task_2).await;

    // 1. Unattended mode strictly converts ASK to DENY without waiting
    let unattended_req = ApprovalRequest::new(
        mission_id,
        Some(task_1),
        Some(agent_id),
        ToolCallId::new(),
        "run_command",
        serde_json::json!({ "command": "deploy.sh" }),
        vec!["network".to_string()],
        RiskClass::HighRiskMutation,
        Some("rule-net".to_string()),
        "hash-1",
        "Deployment requires approval",
    );

    let unattended_res = coordinator
        .request_approval(
            unattended_req,
            AutonomyMode::Unattended,
            Duration::from_secs(5),
        )
        .await
        .unwrap();

    assert_eq!(
        unattended_res,
        ApprovalAction::Deny {
            reason: "Unattended mode strictly converts unresolved ASK to DENY".to_string()
        }
    );

    // 2. Concurrent async waiters: task_1 and task_2 are requested in parallel
    let req_1 = ApprovalRequest::new(
        mission_id,
        Some(task_1),
        Some(agent_id),
        ToolCallId::new(),
        "write_file",
        serde_json::json!({ "path": "config.toml" }),
        vec!["config.toml".to_string()],
        RiskClass::HighRiskMutation,
        Some("rule-cfg".to_string()),
        "hash-1",
        "Config update",
    );
    let id_1 = req_1.id;

    let req_2 = ApprovalRequest::new(
        mission_id,
        Some(task_2),
        Some(agent_id),
        ToolCallId::new(),
        "delete_file",
        serde_json::json!({ "path": "old.log" }),
        vec!["old.log".to_string()],
        RiskClass::LowRiskMutation,
        Some("rule-del".to_string()),
        "hash-1",
        "Log cleanup",
    );
    let id_2 = req_2.id;

    let coord_clone = coordinator.clone();
    let handle_1 = tokio::spawn(async move {
        coord_clone
            .request_approval(req_1, AutonomyMode::Autonomous, Duration::from_secs(5))
            .await
    });

    let coord_clone2 = coordinator.clone();
    let handle_2 = tokio::spawn(async move {
        coord_clone2
            .request_approval(req_2, AutonomyMode::Autonomous, Duration::from_secs(5))
            .await
    });

    // Give requests time to register in coordinator
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Operator resolves task 1 with AllowOnce and task 2 with Deny
    coordinator
        .resolve_request(id_1, ApprovalAction::AllowOnce, "operator-alice")
        .await
        .unwrap();

    coordinator
        .resolve_request(
            id_2,
            ApprovalAction::Deny {
                reason: "Do not delete logs".to_string(),
            },
            "operator-bob",
        )
        .await
        .unwrap();

    let res_1 = handle_1.await.unwrap().unwrap();
    let res_2 = handle_2.await.unwrap().unwrap();

    assert_eq!(res_1, ApprovalAction::AllowOnce);
    assert_eq!(
        res_2,
        ApprovalAction::Deny {
            reason: "Do not delete logs".to_string()
        }
    );

    // 3. Task cancellation cascades: pending approvals for task_1 get cancelled
    let req_3 = ApprovalRequest::new(
        mission_id,
        Some(task_1),
        Some(agent_id),
        ToolCallId::new(),
        "shell",
        serde_json::json!({ "command": "make test" }),
        vec![],
        RiskClass::ProcessExecution,
        None,
        "hash-1",
        "Run tests",
    );

    let coord_clone3 = coordinator.clone();
    let handle_3 = tokio::spawn(async move {
        coord_clone3
            .request_approval(req_3, AutonomyMode::Autonomous, Duration::from_secs(5))
            .await
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Task 1 gets cancelled
    coordinator.cancel_task_approvals(task_1).await;

    let res_3 = handle_3.await.unwrap().unwrap();
    assert_eq!(
        res_3,
        ApprovalAction::DenyAndCancelTask {
            reason: "Task was cancelled".to_string()
        }
    );
}

// ============================================================================
// Task 09-02-03: SQLite policy_grants Store & Pipeline Stage 8 Integration
// ============================================================================

#[tokio::test]
async fn test_session_grants_lifecycle() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("grants_test.db");
    let pool = initialize_database(&db_path).await.unwrap();
    let workspace = temp.path().to_path_buf();

    let mission_id = MissionId::new();
    let task_id = TaskId::new();
    let agent_id = AgentId::new();
    seed_mission_and_task(&pool, mission_id, task_id, agent_id).await;

    let grant_store = PolicyGrantStore::new();
    let policy_hash = "sha256-active-policy-999";

    // 1. Create a task-scoped grant for write_file on ./src/**.
    // Canonical provenance: the grant names the approval request that
    // created it (unattributed rows never authorize).
    let approval_request_id = m31a::ids::ApprovalRequestId::new();
    seed_approval_request(&pool, approval_request_id, mission_id, task_id, policy_hash).await;
    let grant = PolicyGrant::new(
        mission_id,
        Some(task_id),
        ApprovalResolutionScope::Task,
        "write_file",
        "./src/**",
        None,
        Some(approval_request_id),
        policy_hash,
        None,
    );
    grant_store.create_grant(&pool, &grant).await.unwrap();

    // 2. Finding applicable grants
    let target_inside = workspace.join("src/utils.rs");
    std::fs::create_dir_all(target_inside.parent().unwrap()).unwrap();
    std::fs::write(&target_inside, "// utils").unwrap();

    let matches = grant_store
        .find_applicable_grants(
            &pool,
            mission_id,
            Some(task_id),
            "write_file",
            Some(&target_inside),
            &workspace,
            &serde_json::json!({}),
            policy_hash,
        )
        .await
        .unwrap();

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].tool_or_capability, "write_file");

    // 3. Different task does not match task-scoped grant
    let other_task = TaskId::new();
    let no_match_task = grant_store
        .find_applicable_grants(
            &pool,
            mission_id,
            Some(other_task),
            "write_file",
            Some(&target_inside),
            &workspace,
            &serde_json::json!({}),
            policy_hash,
        )
        .await
        .unwrap();
    assert!(no_match_task.is_empty());

    // 4. Different policy hash invalidates grant
    let no_match_hash = grant_store
        .find_applicable_grants(
            &pool,
            mission_id,
            Some(task_id),
            "write_file",
            Some(&target_inside),
            &workspace,
            &serde_json::json!({}),
            "modified-policy-hash",
        )
        .await
        .unwrap();
    assert!(no_match_hash.is_empty());

    // 5. Invalidate task grants on completion
    let revoked_count = grant_store
        .invalidate_task_grants(&pool, task_id)
        .await
        .unwrap();
    assert_eq!(revoked_count, 1);

    let post_revocation = grant_store
        .find_applicable_grants(
            &pool,
            mission_id,
            Some(task_id),
            "write_file",
            Some(&target_inside),
            &workspace,
            &serde_json::json!({}),
            policy_hash,
        )
        .await
        .unwrap();
    assert!(post_revocation.is_empty());
}

#[tokio::test]
async fn test_pipeline_approval_coordinator_integration() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("pipeline_approval.db");
    let pool = initialize_database(&db_path).await.unwrap();
    let workspace = temp.path().to_path_buf();

    let mission_id = MissionId::new();
    let task_id = TaskId::new();
    let agent_id = AgentId::new();
    seed_mission_and_task(&pool, mission_id, task_id, agent_id).await;

    let registry = Arc::new(CapabilityRegistry::new());
    registry.register_instance(CapabilityInstance::new(
        "fs.test",
        "Test Filesystem",
        "1.0.0",
        CapabilityFamily::Filesystem,
        "test_fs",
        CapabilityPermissions::full_access(),
    ));

    let counter = Arc::new(AtomicU32::new(0));
    let mut tool_reg = ToolRegistry::new_default(Arc::clone(&registry));
    tool_reg.register(EchoTool {
        counter: Arc::clone(&counter),
    });
    let tool_registry = Arc::new(tool_reg);

    let channel = Arc::new(MockApprovalChannel::default());
    let coordinator = Arc::new(ApprovalCoordinator::new(
        Some(pool.clone()),
        Some(channel.clone()),
    ));

    let runner = ToolPipelineRunner::new(tool_registry)
        .with_db_pool(pool.clone())
        .with_approval_coordinator(Arc::clone(&coordinator));

    let token = CancellationToken::new();
    let context = ToolExecutionContext::new(registry, workspace.clone(), token)
        .with_mission_id(mission_id)
        .with_task_id(task_id)
        .with_agent_id(agent_id);

    let policy_gate = AskPolicyGate;

    let action_req = ActionRequest {
        id: "act-ask-1".to_string(),
        tool_name: "echo_tool".to_string(),
        parameters: serde_json::json!({
            "message": "critical mutation",
            "path": "test.txt"
        }),
    };

    // Spawn execution in background
    let runner_clone = runner.clone();
    let ctx_clone = context.clone();
    let exec_handle = tokio::spawn(async move {
        runner_clone
            .execute_action(
                &action_req,
                &ctx_clone,
                &policy_gate,
                AutonomyMode::Autonomous,
            )
            .await
    });

    // Wait for channel to receive request
    let mut request_id = None;
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(20)).await;
        let list = channel.notified.lock().await;
        if let Some(packet) = list.first() {
            request_id = Some(packet.request_id);
            break;
        }
    }

    let req_id = request_id.expect("approval channel must receive explanation packet");

    // Operator grants permission for task
    coordinator
        .resolve_request(req_id, ApprovalAction::AllowForTask, "admin-operator")
        .await
        .unwrap();

    let action_res = exec_handle.await.unwrap();
    assert!(
        action_res.success,
        "Action should succeed after approval: {:?}",
        action_res.error
    );
    assert_eq!(counter.load(Ordering::SeqCst), 1);

    // Verify policy_decisions table recorded the evaluation
    let auditor = DurablePolicyAuditor::new();
    let audit_records = auditor
        .list_decisions_for_mission(&pool, mission_id)
        .await
        .unwrap();
    assert_eq!(audit_records.len(), 1);
    assert_eq!(audit_records[0].tool_or_capability, "echo_tool");
    assert_eq!(audit_records[0].decision, "ask");

    // Verify persistent grant was recorded in policy_grants. The grant
    // binds the approved arguments, so the lookup uses those exact args
    // (a generic empty-args query must NOT match an arg-bound grant).
    let grants = PolicyGrantStore::new()
        .find_applicable_grants(
            &pool,
            mission_id,
            Some(task_id),
            "echo_tool",
            None,
            &workspace,
            &serde_json::json!({
                "message": "critical mutation",
                "path": "test.txt",
                "target": "test.txt"
            }),
            "hash123",
        )
        .await
        .unwrap();
    assert_eq!(grants.len(), 1);
    assert_eq!(grants[0].scope_type, ApprovalResolutionScope::Task);
}

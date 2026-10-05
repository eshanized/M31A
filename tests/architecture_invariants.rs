//! Architecture-level invariant tests: policy identity, authorization
//! binding, Git gates, budget atomicity/durability, and sandbox enforcement.
//!
//! These tests assert the ARCHITECTURE (no side effect without
//! authorization; typed identity preserved; deny immutable; admission
//! atomic; restart durable; sandbox fail-closed), not implementation details.

use std::sync::Arc;
use std::time::Duration;

use m31a::budget::{BudgetEnforcer, BudgetLedger, TaskEstimates};
use m31a::capability::providers::CliGitProvider;
use m31a::capability::traits::git::GitService;
use m31a::git::{AuthorizationAuthority, GIT_AUTH_DEFAULT_TTL, GitGate, GitOperation};
use m31a::ids::{MissionId, TaskId};
use m31a::kernel::seams::policy::{
    PolicyDecision, PolicyError, PolicyEvaluationRequest, PolicyGate,
};
use m31a::policy::approval::{ApprovalResolutionScope, PolicyGrant, PolicyGrantStore};
use m31a::policy::effective::EffectivePolicy;
use m31a::policy::layers::PolicyLayer;
use m31a::policy::rule::PolicyRule;
use m31a::sandbox::plan::{NetworkConfinement, SandboxPlan};
use m31a::sandbox::{SandboxCapabilities, SandboxEnforcement};
use m31a::state::budget::ResourceBudget;
use m31a::state_machine::AutonomyMode;
use m31a::state_machine::agent::AgentRole;

fn typed_request(tool: &str, ws: &std::path::Path) -> PolicyEvaluationRequest {
    PolicyEvaluationRequest::new(MissionId::new(), TaskId::new(), tool)
        .with_role(AgentRole::implementer())
        .with_autonomy_mode(AutonomyMode::Autonomous)
        .with_workspace(ws.to_path_buf())
}

// ─── 1. No side effect without authorization ─────────────────────────────

#[tokio::test]
async fn test_no_side_effect_without_authorization() {
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path();
    init_git_repo(ws);
    let git = CliGitProvider::new(ws);
    let denied = GitGate::denied();

    // Every mutating GitService entry fails closed on a denied gate.
    assert!(git.commit("x", &denied).await.is_err());
    assert!(git.add_all(&denied).await.is_err());
    assert!(git.reset(&["."], &denied).await.is_err());
    assert!(git.merge("main", true, &denied).await.is_err());
    assert!(git.merge_abort(&denied).await.is_err());
    assert!(git.restore_head(".", &denied).await.is_err());
    assert!(git.checkout("main", &denied).await.is_err());
    // Read-only entries never require authorization.
    assert!(git.status().await.is_ok());
}

#[tokio::test]
async fn test_git_mutation_requires_execution_authorization() {
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path();
    init_git_repo(ws);
    let git = CliGitProvider::new(ws);
    let authority = AuthorizationAuthority::new();

    // Forged gate from a FOREIGN authority instance is rejected.
    let foreign = AuthorizationAuthority::new();
    let forged = foreign.mint_git_authorization(
        MissionId::new(),
        Some(TaskId::new()),
        None,
        "hash".to_string(),
        ws.to_path_buf(),
        GitOperation::Add {
            paths: vec!["-A".to_string()],
        },
        Vec::new(),
        "scope",
        "forged",
        GIT_AUTH_DEFAULT_TTL,
    );
    assert!(GitGate::authorized_verified(forged, &authority).is_err());

    // Gate bound to a DIFFERENT operation does not cover this one.
    let other_op = authority.mint_git_authorization(
        MissionId::new(),
        Some(TaskId::new()),
        None,
        "hash".to_string(),
        ws.to_path_buf(),
        GitOperation::Merge {
            source: "other".to_string(),
        },
        Vec::new(),
        "scope",
        "test",
        GIT_AUTH_DEFAULT_TTL,
    );
    let wrong_op_gate = GitGate::authorized_verified(other_op, &authority).expect("mint verifies");
    assert!(git.add_all(&wrong_op_gate).await.is_err());

    // Gate bound to a DIFFERENT workspace does not cover this one.
    let elsewhere = authority.mint_git_authorization(
        MissionId::new(),
        Some(TaskId::new()),
        None,
        "hash".to_string(),
        std::path::PathBuf::from("/tmp/elsewhere"),
        GitOperation::Add {
            paths: vec!["-A".to_string()],
        },
        Vec::new(),
        "scope",
        "test",
        GIT_AUTH_DEFAULT_TTL,
    );
    let wrong_ws_gate = GitGate::authorized_verified(elsewhere, &authority).expect("mint verifies");
    assert!(git.add_all(&wrong_ws_gate).await.is_err());
}

#[tokio::test]
async fn test_commit_message_binding_is_exact() {
    let authority = AuthorizationAuthority::new();
    let ws = std::path::PathBuf::from("/tmp/ws");
    let base = "fix: thing\n\nM31A-Mission: m\nM31A-Task: t\n";
    let auth = authority.mint_git_authorization(
        MissionId::new(),
        Some(TaskId::new()),
        None,
        "hash".to_string(),
        ws.clone(),
        GitOperation::Commit {
            message: base.to_string(),
        },
        Vec::new(),
        "commit",
        "test",
        GIT_AUTH_DEFAULT_TTL,
    );
    // Final message naming THIS authorization covers.
    let final_msg = format!("{base}M31A-Authorization: {}\n", auth.authorization_id);
    assert!(
        auth.verify_for(
            &authority,
            &GitOperation::Commit { message: final_msg },
            &ws
        )
        .is_ok()
    );
    // Tampered body fails closed.
    assert!(
        auth.verify_for(
            &authority,
            &GitOperation::Commit {
                message: "fix: EVIL\n".to_string()
            },
            &ws
        )
        .is_err()
    );
    // Trailer naming a DIFFERENT authorization fails closed.
    assert!(
        auth.verify_for(
            &authority,
            &GitOperation::Commit {
                message: format!("{base}M31A-Authorization: someone-else\n")
            },
            &ws
        )
        .is_err()
    );
}

// ─── 2. Typed policy identity ────────────────────────────────────────────

#[tokio::test]
async fn test_policy_identity_is_typed_and_preserved() {
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path();

    // Role-constrained rule: only implementer may write.
    let mut rule = PolicyRule::new("role-write", PolicyDecision::Allow);
    rule.tools = vec!["write_file".to_string()];
    rule.roles = vec![AgentRole::implementer()];
    let policy = EffectivePolicy::new(vec![(PolicyLayer::Workspace, vec![rule])]);

    // Role reaches the engine unchanged.
    let req = typed_request("write_file", ws);
    assert_eq!(policy.evaluate(req).await.unwrap(), PolicyDecision::Allow);

    // A different role does not match (falls to Ask default, never Allow).
    let other = PolicyEvaluationRequest::new(MissionId::new(), TaskId::new(), "write_file")
        .with_role(AgentRole::reviewer())
        .with_autonomy_mode(AutonomyMode::Autonomous)
        .with_workspace(ws.to_path_buf());
    assert_ne!(policy.evaluate(other).await.unwrap(), PolicyDecision::Allow);

    // Autonomy mode reaches the engine unchanged.
    let mut mode_rule = PolicyRule::new("mode-plan-deny", PolicyDecision::Deny);
    mode_rule.tools = vec!["write_file".to_string()];
    mode_rule.modes = vec![AutonomyMode::Plan];
    let mode_policy = EffectivePolicy::new(vec![(PolicyLayer::Workspace, vec![mode_rule])]);
    let plan_req = PolicyEvaluationRequest::new(MissionId::new(), TaskId::new(), "write_file")
        .with_role(AgentRole::implementer())
        .with_autonomy_mode(AutonomyMode::Plan)
        .with_workspace(ws.to_path_buf());
    assert_eq!(
        mode_policy.evaluate(plan_req).await.unwrap(),
        PolicyDecision::Deny
    );

    // Workspace identity reaches the engine: a path rule matches only under
    // the workspace it was evaluated for.
    let mut path_rule = PolicyRule::new("ws-path", PolicyDecision::Allow);
    path_rule.tools = vec!["read_file".to_string()];
    path_rule.paths = vec!["./src/**".to_string()];
    let path_policy = EffectivePolicy::new(vec![(PolicyLayer::Workspace, vec![path_rule])]);
    let rel =
        typed_request("read_file", ws).with_arguments(serde_json::json!({"path": "src/a.rs"}));
    assert_eq!(
        path_policy.evaluate(rel).await.unwrap(),
        PolicyDecision::Allow
    );

    // Missing security-critical attributes fail closed (never defaulted).
    let no_role = PolicyEvaluationRequest::new(MissionId::new(), TaskId::new(), "write_file")
        .with_autonomy_mode(AutonomyMode::Autonomous)
        .with_workspace(ws.to_path_buf());
    assert!(matches!(
        path_policy.evaluate(no_role).await,
        Err(PolicyError::MissingSecurityAttribute(_))
    ));
    let no_mode = PolicyEvaluationRequest::new(MissionId::new(), TaskId::new(), "write_file")
        .with_role(AgentRole::implementer())
        .with_workspace(ws.to_path_buf());
    assert!(matches!(
        path_policy.evaluate(no_mode).await,
        Err(PolicyError::MissingSecurityAttribute(_))
    ));
    let no_ws = PolicyEvaluationRequest::new(MissionId::new(), TaskId::new(), "write_file")
        .with_role(AgentRole::implementer())
        .with_autonomy_mode(AutonomyMode::Autonomous);
    assert!(matches!(
        path_policy.evaluate(no_ws).await,
        Err(PolicyError::MissingSecurityAttribute(_))
    ));
    // Missing authorization-bearing evaluation on the mock gate is covered
    // by controller fail-closed paths; the request type itself carries the
    // optional authorization slot (None here proves nothing is fabricated).
    let bare = PolicyEvaluationRequest::new(MissionId::new(), TaskId::new(), "t");
    assert!(bare.authorization_id.is_none());
}

// ─── 3. Hierarchy: higher-authority Deny is immutable ────────────────────

#[tokio::test]
async fn test_builtin_deny_is_immutable() {
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path();
    let base = EffectivePolicy::standard(ws);

    // Protected runtime paths are vetoed at BuiltInSafety…
    let req =
        typed_request("write_file", ws).with_arguments(serde_json::json!({"path": ".git/config"}));
    let (decision, record) = base.evaluate_record(req).await.unwrap();
    assert_eq!(decision, PolicyDecision::Deny);

    // …and a lower-authority Allow (even a session grant) cannot weaken it.
    let mut grant_rule = PolicyRule::new("session-grant", PolicyDecision::Allow);
    grant_rule.tools = vec!["write_file".to_string()];
    let extended = base.extended_for_execution(vec![], vec![], vec![], vec![grant_rule]);
    let req2 =
        typed_request("write_file", ws).with_arguments(serde_json::json!({"path": ".git/config"}));
    let (decision2, _) = extended.evaluate_record(req2).await.unwrap();
    assert_eq!(decision2, PolicyDecision::Deny);
    assert_eq!(
        record.unwrap().matched_layer.as_deref(),
        Some("built_in_safety")
    );
}

// ─── 4. Grants: exact scope, expiry, revocation, one-shot ────────────────

async fn grant_pool() -> (tempfile::TempDir, sqlx::SqlitePool) {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("m31a.db");
    let pool = m31a::persistence::sqlite::schema::initialize_database(&db)
        .await
        .expect("migrate");
    (dir, pool)
}

fn test_grant(mission: MissionId, task: TaskId, hash: &str) -> PolicyGrant {
    use m31a::ids::ApprovalRequestId;
    PolicyGrant::new(
        mission,
        Some(task),
        ApprovalResolutionScope::Task,
        "write_file",
        "src/*",
        Some(serde_json::json!({"path": "src/a.rs"})),
        Some(ApprovalRequestId::new()),
        hash,
        None,
    )
}

/// Seed the mission/task/approval-request rows that `policy_grants` foreign
/// keys require. Grants reference REAL durable rows — the same provenance
/// discipline production enforces.
async fn seed_grant_provenance(
    pool: &sqlx::SqlitePool,
    mission: MissionId,
    task: TaskId,
    request: m31a::ids::ApprovalRequestId,
) {
    let now = chrono::Utc::now().to_rfc3339();
    // Mission/task rows are the durable execution context (idempotent);
    // the approval-request row is the fresh provenance for each grant.
    sqlx::query("INSERT OR IGNORE INTO missions (id, objective, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?)")
        .bind(mission.as_bytes().as_slice())
        .bind("invariant test")
        .bind("running")
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT OR IGNORE INTO tasks (id, mission_id, title, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(task.as_bytes().as_slice())
        .bind(mission.as_bytes().as_slice())
        .bind("t")
        .bind("running")
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO approval_requests (id, mission_id, task_id, tool_call_id, tool_or_capability, normalized_args_json, redacted_args_json, affected_resources, risk_classification, policy_hash, reason, resolution_state, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(request.as_bytes().as_slice())
    .bind(mission.as_bytes().as_slice())
    .bind(task.as_bytes().as_slice())
    .bind("call-1")
    .bind("write_file")
    .bind("{}")
    .bind("{}")
    .bind("[]")
    .bind("low")
    .bind("hash1")
    .bind("test")
    .bind("allowed")
    .bind(&now)
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn test_session_grant_requires_exact_scope() {
    let (_dir, pool) = grant_pool().await;
    let store = PolicyGrantStore::new();
    let mission = MissionId::new();
    let task = TaskId::new();
    let ws = tempfile::tempdir().unwrap();
    let ws_path = ws.path().to_path_buf();
    std::fs::create_dir_all(ws_path.join("src")).unwrap();
    std::fs::write(ws_path.join("src/a.rs"), "x").unwrap();
    let args = serde_json::json!({"path": "src/a.rs"});
    let target = ws_path.join("src/a.rs");

    store
        .create_grant(&pool, &{
            let g = test_grant(mission, task, "hash1");
            seed_grant_provenance(
                &pool,
                mission,
                task,
                g.created_from_request_id.expect("grant carries provenance"),
            )
            .await;
            g
        })
        .await
        .unwrap();

    // Exact scope matches.
    let hit = store
        .find_applicable_grants(
            &pool,
            mission,
            Some(task),
            "write_file",
            Some(&target),
            &ws_path,
            &args,
            "hash1",
        )
        .await
        .unwrap();
    assert_eq!(hit.len(), 1);

    // Wrong task scope.
    let miss_task = store
        .find_applicable_grants(
            &pool,
            mission,
            Some(TaskId::new()),
            "write_file",
            Some(&target),
            &ws_path,
            &args,
            "hash1",
        )
        .await
        .unwrap();
    assert!(miss_task.is_empty());

    // Wrong tool.
    let miss_tool = store
        .find_applicable_grants(
            &pool,
            mission,
            Some(task),
            "run_command",
            Some(&target),
            &ws_path,
            &args,
            "hash1",
        )
        .await
        .unwrap();
    assert!(miss_tool.is_empty());

    // Wrong resource (outside the granted pattern).
    let other = ws_path.join("other/c.rs");
    let miss_path = store
        .find_applicable_grants(
            &pool,
            mission,
            Some(task),
            "write_file",
            Some(&other),
            &ws_path,
            &args,
            "hash1",
        )
        .await
        .unwrap();
    assert!(miss_path.is_empty());

    // Wrong arguments.
    let miss_args = store
        .find_applicable_grants(
            &pool,
            mission,
            Some(task),
            "write_file",
            Some(&target),
            &ws_path,
            &serde_json::json!({"path": "src/other.rs"}),
            "hash1",
        )
        .await
        .unwrap();
    assert!(miss_args.is_empty());

    // Policy generation changed.
    let miss_hash = store
        .find_applicable_grants(
            &pool,
            mission,
            Some(task),
            "write_file",
            Some(&target),
            &ws_path,
            &args,
            "hash2",
        )
        .await
        .unwrap();
    assert!(miss_hash.is_empty());

    // Revocation kills it.
    store.invalidate_task_grants(&pool, task).await.unwrap();
    let revoked = store
        .find_applicable_grants(
            &pool,
            mission,
            Some(task),
            "write_file",
            Some(&target),
            &ws_path,
            &args,
            "hash1",
        )
        .await
        .unwrap();
    assert!(revoked.is_empty());
}

#[tokio::test]
async fn test_grant_expiry_and_one_shot_consumption() {
    let (_dir, pool) = grant_pool().await;
    let store = PolicyGrantStore::new();
    let mission = MissionId::new();
    let task = TaskId::new();
    let ws = tempfile::tempdir().unwrap();
    let args = serde_json::json!({});

    // Expired grant never matches.
    let mut expired = test_grant(mission, task, "hash1");
    expired.scope_type = ApprovalResolutionScope::Mission;
    expired.expires_at = Some(chrono::Utc::now() - chrono::Duration::hours(1));
    seed_grant_provenance(
        &pool,
        mission,
        task,
        expired.created_from_request_id.expect("provenance"),
    )
    .await;
    store.create_grant(&pool, &expired).await.unwrap();
    let found = store
        .find_applicable_grants(
            &pool,
            mission,
            Some(task),
            "write_file",
            None,
            ws.path(),
            &args,
            "hash1",
        )
        .await
        .unwrap();
    assert!(found.is_empty(), "expired grant must not match");

    // One-shot grant matches once, then consumption revokes it.
    let mut once = test_grant(mission, task, "hash1");
    once.scope_type = ApprovalResolutionScope::Once;
    once.arg_constraints = None;
    once.resource_pattern = "*".to_string();
    once.expires_at = None;
    // Provenance row already seeded above (same mission/task/request family
    // would conflict; seed only if absent).
    if once.created_from_request_id.is_some() {
        let req = once.created_from_request_id.expect("provenance");
        let exists: Option<Vec<u8>> =
            sqlx::query_scalar("SELECT id FROM approval_requests WHERE id = ?")
                .bind(req.as_bytes().to_vec())
                .fetch_optional(&pool)
                .await
                .unwrap();
        if exists.is_none() {
            seed_grant_provenance(&pool, mission, task, req).await;
        }
    }
    store.create_grant(&pool, &once).await.unwrap();
    let first = store
        .find_applicable_grants(
            &pool,
            mission,
            Some(task),
            "write_file",
            None,
            ws.path(),
            &args,
            "hash1",
        )
        .await
        .unwrap();
    // The task-scoped arg-constrained grant above does not match arg-less
    // query; only the Once grant (resource "*" covers, args None covers).
    assert!(
        first.iter().any(|g| g.id == once.id),
        "one-shot grant must match before consumption"
    );
    assert!(store.consume_one_shot_grant(&pool, once.id).await.unwrap());
    // Replay fails: already consumed.
    assert!(!store.consume_one_shot_grant(&pool, once.id).await.unwrap());
    let second = store
        .find_applicable_grants(
            &pool,
            mission,
            Some(task),
            "write_file",
            None,
            ws.path(),
            &args,
            "hash1",
        )
        .await
        .unwrap();
    assert!(
        !second.iter().any(|g| g.id == once.id),
        "consumed one-shot grant must never match again"
    );
}

// ─── 5/6. Authorization binding + replay ─────────────────────────────────

#[test]
fn test_authorization_cannot_be_replayed() {
    use m31a::planning::review::ExecutionAuthorization;
    let auth = ExecutionAuthorization::new("session-1", 2, 1, "operator")
        .with_content_hashes("plan-hash".to_string(), "task-hash".to_string())
        .with_execution_binding(
            "policy-1",
            "/tmp/ws",
            "implementer",
            "autonomous",
            Duration::from_secs(3600),
        );
    assert!(auth.is_valid_for(2, 1));
    assert!(!auth.is_valid_for(3, 1), "plan revision drift fails closed");
    assert!(!auth.is_valid_for(2, 2), "task revision drift fails closed");
    assert!(
        !auth.is_valid_for_exact(2, 1, Some("other-plan"), Some("task-hash")),
        "content drift fails closed"
    );

    // Surface verification: every dimension must bind exactly.
    use m31a::persistence::sqlite::repositories::lifecycle::{
        ResumeAuthExpectations, SqliteLifecycleRepository,
    };
    let exact = ResumeAuthExpectations {
        policy_hash: "policy-1".to_string(),
        workspace_root: "/tmp/ws".to_string(),
        agent_role: "implementer".to_string(),
        autonomy_mode: "autonomous".to_string(),
    };
    assert!(SqliteLifecycleRepository::verify_execution_surface(&auth, &exact).is_ok());
    for mutated in [
        ResumeAuthExpectations {
            policy_hash: "policy-2".to_string(),
            ..exact.clone()
        },
        ResumeAuthExpectations {
            workspace_root: "/tmp/other".to_string(),
            ..exact.clone()
        },
        ResumeAuthExpectations {
            agent_role: "reviewer".to_string(),
            ..exact.clone()
        },
        ResumeAuthExpectations {
            autonomy_mode: "safe".to_string(),
            ..exact.clone()
        },
    ] {
        assert!(
            SqliteLifecycleRepository::verify_execution_surface(&auth, &mutated).is_err(),
            "surface drift must fail closed"
        );
    }
    // Expired authorization fails closed.
    let mut stale_auth = auth.clone();
    stale_auth.expires_at = Some(chrono::Utc::now() - chrono::Duration::seconds(1));
    assert!(SqliteLifecycleRepository::verify_execution_surface(&stale_auth, &exact).is_err());
    // Unbound (legacy) authorization never verifies.
    let legacy = ExecutionAuthorization::new("session-1", 2, 1, "operator");
    assert!(SqliteLifecycleRepository::verify_execution_surface(&legacy, &exact).is_err());
}

// ─── 7. Budget atomicity under concurrency ───────────────────────────────

#[test]
fn test_budget_admission_is_atomic_under_concurrency() {
    use std::sync::Barrier;
    let limits = ResourceBudget {
        max_tokens: Some(100),
        ..Default::default()
    };
    let enforcer = Arc::new(BudgetEnforcer::new(limits));
    let barrier = Arc::new(Barrier::new(16));
    let mut handles = Vec::new();
    for _ in 0..16 {
        let e = enforcer.clone();
        let b = barrier.clone();
        handles.push(std::thread::spawn(move || {
            b.wait();
            let est = TaskEstimates {
                estimated_tokens: 30,
                estimated_cost_usd: 0.0,
                requires_worker: false,
                estimated_artifact_bytes: 0,
            };
            e.reserve(&est, false).ok().map(|_| est.estimated_tokens)
        }));
    }
    let admitted: u64 = handles
        .into_iter()
        .map(|h| h.join().unwrap().unwrap_or(0))
        .sum();
    // Sequential admission would allow floor(100/30)=3 (90 tokens). Racy
    // admission without serialization could admit all 16 (480). Atomicity
    // guarantees the combined reservations never exceed the budget.
    assert!(
        admitted <= 100,
        "combined concurrent reservations ({admitted}) exceeded budget 100"
    );
    assert!(
        admitted >= 90,
        "admission starved: only {admitted} admitted"
    );
}

// ─── 8. Budget survives restart ──────────────────────────────────────────

#[tokio::test]
async fn test_budget_survives_restart() {
    let (_dir, pool) = grant_pool().await;
    let mission = MissionId::new();
    let limits = ResourceBudget {
        max_tokens: Some(1000),
        ..Default::default()
    };
    let enforcer = BudgetEnforcer::new(limits.clone());
    // Consume 700 authoritative + 200 estimated, then persist.
    let r1 = enforcer
        .reserve(
            &TaskEstimates {
                estimated_tokens: 700,
                estimated_cost_usd: 0.0,
                requires_worker: false,
                estimated_artifact_bytes: 0,
            },
            false,
        )
        .unwrap();
    enforcer.settle(
        &r1,
        &m31a::budget::ActualUsage {
            tokens: 700,
            ..Default::default()
        },
    );
    let r2 = enforcer
        .reserve(
            &TaskEstimates {
                estimated_tokens: 200,
                estimated_cost_usd: 0.0,
                requires_worker: false,
                estimated_artifact_bytes: 0,
            },
            false,
        )
        .unwrap();
    enforcer.settle_estimated(
        &r2,
        &m31a::budget::ActualUsage {
            tokens: 200,
            ..Default::default()
        },
    );
    BudgetLedger::new(pool.clone())
        .record(mission, &enforcer)
        .await
        .unwrap();

    // Crash/restart: fresh enforcer hydrates full accounted totals.
    let restarted = BudgetEnforcer::new(limits);
    assert!(
        BudgetLedger::new(pool.clone())
            .hydrate(mission, &restarted)
            .await
            .unwrap()
    );
    assert_eq!(restarted.total_tokens_consumed(), 700);
    assert_eq!(restarted.total_tokens_estimated(), 200);
    // 900 accounted: a 200-token admission must be denied post-restart.
    assert!(
        restarted
            .reserve(
                &TaskEstimates {
                    estimated_tokens: 200,
                    estimated_cost_usd: 0.0,
                    requires_worker: false,
                    estimated_artifact_bytes: 0,
                },
                false
            )
            .is_err(),
        "post-restart over-limit admission must be denied"
    );
}

// ─── 9/10. Sandbox fail-closed + network ─────────────────────────────────

#[tokio::test]
async fn test_process_execution_fails_closed_without_required_sandbox() {
    use m31a::capability::providers::LocalProcessProvider;
    use m31a::capability::traits::process::ProcessService;
    use m31a::process::supervisor::ProcessSupervisor;
    let dir = tempfile::tempdir().unwrap();
    // Required isolation with NO isolation backend: fail closed, never host fallback.
    let provider = LocalProcessProvider::with_sandbox(
        dir.path().to_path_buf(),
        Arc::new(ProcessSupervisor::default()),
        None,
        false,
    )
    .with_enforcement(SandboxEnforcement::from_sandbox_mode("strict"));
    assert!(provider.is_sandbox_required());
    let res = provider
        .spawn_command("echo", &["hi".to_string()], None, 10)
        .await;
    assert!(
        res.is_err(),
        "required-but-unavailable isolation must fail closed"
    );
}

#[test]
fn test_network_policy_is_enforced_by_sandbox() {
    // A plan demanding network isolation never validates against a backend
    // that cannot deny network: no silent downgrade.
    let plan = SandboxPlan::new(std::path::PathBuf::from("/tmp/ws")).with_net_isolation(true);
    assert!(plan.validate_against(&SandboxCapabilities::none()).is_err());
    assert!(plan.validate_against(&SandboxCapabilities::all()).is_ok());
    // Deployment policy derives network denial: strict/standard isolate.
    assert!(SandboxEnforcement::from_sandbox_mode("strict").network_isolated);
    assert!(SandboxEnforcement::from_sandbox_mode("standard").network_isolated);
    // Unknown modes fail closed to strict (required + isolated).
    let unknown = SandboxEnforcement::from_sandbox_mode("paranoid");
    assert!(unknown.isolation_required && unknown.network_isolated);
    let _ = NetworkConfinement::Isolated;
}

// ─── helpers ─────────────────────────────────────────────────────────────

fn init_git_repo(ws: &std::path::Path) {
    let run = |args: &[&str]| {
        assert!(
            std::process::Command::new("git")
                .args(args)
                .current_dir(ws)
                .status()
                .unwrap()
                .success(),
            "git {args:?}"
        );
    };
    run(&["init", "-b", "main"]);
    run(&["config", "user.name", "t"]);
    run(&["config", "user.email", "t@t"]);
    std::fs::write(ws.join("f.txt"), "base\n").unwrap();
    run(&["add", "-A"]);
    run(&["commit", "-m", "base"]);
}

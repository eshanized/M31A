//! Architecture regression tests: single authoritative runtime dependency graph.
//!
//! Each test enforces one wiring invariant from
//! `docs/audits/WIRING-REMEDIATION-v0.1.1.md`. These tests guard the
//! remediation against drift back into split-brain wiring: if production code
//! reintroduces a second registry, a fabricated approval, a stale derived
//! engine, or a fake-success path, the corresponding test fails.

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use m31a::agent::engine::{AgentEngine, AgentTurnOutcome};
use m31a::agent::model_policy::{ModelCaller, ModelProposal, TestModelCaller};
use m31a::agent::profile::AgentProfile;
use m31a::ids::{AgentId, MissionId, SessionId, TaskId};
use m31a::interaction::session::SqliteSessionRepository;
use m31a::kernel::seams::policy::{PolicyDecision, PolicyEvaluationRequest, PolicyGate};
use m31a::model::types::ModelToolCall;
use m31a::runtime::AppRuntime;
use m31a::state_machine::agent::AgentRole;

/// Counting policy gate: the single-evaluation probe (Invariant 12).
struct CountingGate {
    count: Arc<AtomicUsize>,
    decision: PolicyDecision,
}

#[async_trait::async_trait]
impl PolicyGate for CountingGate {
    async fn evaluate(
        &self,
        _req: PolicyEvaluationRequest,
    ) -> Result<PolicyDecision, m31a::kernel::seams::policy::PolicyError> {
        self.count.fetch_add(1, Ordering::SeqCst);
        Ok(self.decision)
    }
}

async fn setup_runtime() -> (
    tempfile::TempDir,
    Arc<AppRuntime>,
    SessionId,
    SqliteSessionRepository,
) {
    let tmp_root = std::path::PathBuf::from("target/tmp");
    let _ = std::fs::create_dir_all(&tmp_root);
    let dir = tempfile::Builder::new()
        .prefix("m31a-arch-")
        .tempdir_in(&tmp_root)
        .expect("tempdir");
    let ws = dir.path();
    let git_init = std::process::Command::new("git")
        .args(["init", "--template=", "-b", "main"])
        .current_dir(ws)
        .status()
        .expect("git init failed");
    assert!(git_init.success());
    tokio::fs::create_dir_all(ws.join("src")).await.unwrap();
    tokio::fs::write(
        ws.join("src/lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 { a + b }\n",
    )
    .await
    .unwrap();

    let runtime = Arc::new(AppRuntime::new(ws.to_path_buf()).await.expect("runtime"));
    let session_repo = SqliteSessionRepository::new(runtime.pool().clone());
    let session = session_repo.create_session(ws).await.unwrap();
    (dir, runtime, session.id, session_repo)
}

/// Build an engine bound to the runtime-shared registries with an injected
/// policy gate and model caller (the supported seam-override path: shared
/// authorities, replaced decision/model seams only).
fn bound_test_engine(
    runtime: &Arc<AppRuntime>,
    session_id: SessionId,
    session_repo: SqliteSessionRepository,
    caller: Arc<dyn ModelCaller>,
    gate: Arc<dyn PolicyGate>,
    wired_pipeline: bool,
) -> AgentEngine {
    let pipeline_runner = if wired_pipeline {
        Arc::new(
            m31a::pipeline::runner::ToolPipelineRunner::new(runtime.tool_registry().clone())
                .with_db_pool(runtime.pool().clone())
                .with_approval_coordinator(runtime.approval_coordinator().clone()),
        )
    } else {
        Arc::new(m31a::pipeline::runner::ToolPipelineRunner::new(
            runtime.tool_registry().clone(),
        ))
    };
    let completion_gate = Arc::new(m31a::verification::gate::EvidenceCompletionGate::new(
        runtime.pool().clone(),
        runtime.artifact_store().clone(),
        runtime.workspace_root(),
    ));
    let context_compiler = Arc::new(
        m31a::context::compiler::ProductionContextCompiler::new()
            .with_workspace_root(runtime.workspace_root().to_path_buf()),
    );
    AgentEngine::new(
        session_id,
        runtime.workspace_root().to_path_buf(),
        session_repo,
        caller,
        runtime.tool_registry().clone(),
        pipeline_runner,
        gate,
        runtime.approval_coordinator().clone(),
        completion_gate,
        context_compiler,
        Some(runtime.event_bus().clone()),
        runtime.capability_registry().clone(),
    )
    .with_scope_repos(
        m31a::persistence::sqlite::repositories::SqliteMissionRepository::new(
            runtime.pool().clone(),
        ),
        m31a::persistence::sqlite::repositories::SqliteTaskRepository::new(runtime.pool().clone()),
    )
}

// ── Invariant 1: one capability authority ────────────────────────────────────

#[tokio::test]
async fn invariant_1_engine_observes_runtime_capability_registry() {
    let (_dir, runtime, session_id, _) = setup_runtime().await;
    let engine = runtime.create_agent_engine(session_id);
    assert!(
        Arc::ptr_eq(engine.capability_registry(), runtime.capability_registry()),
        "AgentEngine capability registry must BE the runtime registry (same Arc)"
    );
}

// ── Invariant 2: one tool authority ──────────────────────────────────────────

#[tokio::test]
async fn invariant_2_engine_observes_runtime_tool_registry() {
    let (_dir, runtime, session_id, _) = setup_runtime().await;
    let engine = runtime.create_agent_engine(session_id);
    assert!(
        Arc::ptr_eq(engine.tool_registry(), runtime.tool_registry()),
        "AgentEngine tool registry must BE the runtime registry (same Arc)"
    );
    let schemas = runtime.authorities().model_tool_schemas();
    assert!(
        !schemas.is_empty(),
        "model-visible schemas must derive from the shared registries"
    );
    // The execution context observes the same registry the schemas derive from.
    let ctx = engine.build_execution_context();
    assert!(
        Arc::ptr_eq(&ctx.capability_registry, runtime.capability_registry()),
        "execution context must carry the runtime capability registry"
    );
}

// ── Invariant 3: same policy identity ────────────────────────────────────────

#[tokio::test]
async fn invariant_3_execution_context_carries_bound_identities() {
    let (_dir, runtime, session_id, _) = setup_runtime().await;
    let mission = MissionId::new();
    let task = TaskId::new();
    let agent = AgentId::new();
    let engine = runtime
        .create_agent_engine(session_id)
        .with_role_authority(AgentRole::planner())
        .bind_execution_identity(Some(mission), Some(task), Some(agent));
    let ctx = engine.build_execution_context();
    assert_eq!(ctx.mission_id, Some(mission));
    assert_eq!(ctx.task_id, Some(task));
    assert_eq!(ctx.agent_id, Some(agent));
    let expected_envelope = AgentProfile::built_in(AgentRole::planner()).capability_policy;
    assert_eq!(
        ctx.role_envelope,
        Some(expected_envelope),
        "role envelope must be the TYPED planner envelope, not prompt text"
    );
}

// ── Invariant 4: real approval IDs ───────────────────────────────────────────

#[tokio::test]
async fn invariant_4_approval_ids_are_real_coordinator_requests() {
    let (_dir, runtime, session_id, session_repo) = setup_runtime().await;
    let caller = Arc::new(TestModelCaller::from_proposals(vec![Ok(
        ModelProposal::ToolCalls {
            calls: vec![ModelToolCall::new(
                "read_file",
                serde_json::json!({ "path": "src/lib.rs" }),
            )],
        },
    )]));
    let gate: Arc<dyn PolicyGate> = Arc::new(CountingGate {
        count: Arc::new(AtomicUsize::new(0)),
        decision: PolicyDecision::Ask,
    });
    let mut engine = bound_test_engine(&runtime, session_id, session_repo, caller, gate, true)
        .with_autonomy_mode(m31a::state::intake::AutonomyMode::Autonomous);

    let coordinator = runtime.approval_coordinator().clone();
    let mut handle = tokio::spawn(async move { engine.step(Some("inspect")).await });

    let pending = tokio::time::timeout(std::time::Duration::from_secs(15), async {
        loop {
            let ids = coordinator.pending_request_ids().await;
            if let Some(id) = ids.first().copied() {
                return id;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("Ask must register a REAL coordinator request");

    // An unknown (fabricated) ID cannot be resolved: the coordinator owns truth.
    let fake = m31a::ids::ApprovalRequestId::new();
    assert!(
        coordinator
            .resolve_request(fake, m31a::policy::approval::ApprovalAction::AllowOnce, "t")
            .await
            .is_err(),
        "fabricated approval IDs must NOT resolve"
    );

    coordinator
        .resolve_request(
            pending,
            m31a::policy::approval::ApprovalAction::AllowOnce,
            "test-operator",
        )
        .await
        .expect("real request resolves");

    let outcome = tokio::time::timeout(std::time::Duration::from_secs(30), &mut handle)
        .await
        .expect("approved turn completes")
        .expect("join")
        .expect("step ok");
    assert!(
        matches!(outcome, AgentTurnOutcome::ToolResults { .. }),
        "approved action executes, got {outcome:?}"
    );
}

// ── Invariant 5: no stale engine ─────────────────────────────────────────────

#[tokio::test]
async fn invariant_5_reconfiguration_changes_authority_generation() {
    let (_dir, runtime, session_id, _) = setup_runtime().await;
    let before = runtime.authorities().clone();
    let policy_before = runtime.policy().clone();
    let reconfigured = (*runtime).clone().with_config(runtime.config().clone());
    // Atomic generation change: new authority set, rebuilt policy.
    assert!(
        !Arc::ptr_eq(&before, &reconfigured.authorities().clone()),
        "with_config must install a new authority generation"
    );
    assert!(
        !Arc::ptr_eq(&policy_before, reconfigured.policy()),
        "with_config must rebuild the policy authority"
    );
    // Engines derive per generation: a fresh engine observes the new policy.
    let _engine = reconfigured.create_agent_engine(session_id);
}

#[tokio::test]
async fn invariant_5_runtime_rebind_invalidates_derived_engine() {
    let (_dir, runtime, _, _) = setup_runtime().await;
    let (_dir2, runtime2, _, _) = setup_runtime().await;
    let mut runner = m31a::interaction::InteractiveSessionRunner::new(runtime);
    runner.rebind_runtime(runtime2.clone());
    assert!(
        !runner.has_active_engine(),
        "rebind must invalidate the derived engine"
    );
    assert_eq!(
        runner.workspace_root(),
        runtime2.workspace_root(),
        "rebind must attach the new runtime"
    );
}

// ── Invariant 6: no stale model catalog ──────────────────────────────────────

#[tokio::test]
async fn invariant_6_catalog_replacement_rebinds_model_caller() {
    let (_dir, runtime, _, _) = setup_runtime().await;
    let runtime = (*runtime)
        .clone()
        .with_model_provider(Arc::new(m31a::model::provider::MockProvider::new()));
    let caller = runtime.model_caller().expect("caller with provider");
    let before = caller
        .bound_catalog_snapshot()
        .await
        .expect("caller bound to a catalog");
    assert_ne!(before.provider, "marker-b");

    let rebound = runtime.with_model_catalog(m31a::model::catalog::ModelCatalog::new("marker-b"));
    let after = rebound
        .model_caller()
        .expect("caller survives catalog replacement")
        .bound_catalog_snapshot()
        .await
        .expect("rebuilt caller bound to a catalog");
    assert_eq!(
        after.provider, "marker-b",
        "model caller must observe the CURRENT catalog lock, never a stale one"
    );
    let current = rebound.model_catalog().await;
    assert_eq!(current.provider, "marker-b");
}

// ── Invariant 7: no fake success ─────────────────────────────────────────────

#[tokio::test]
async fn invariant_7_runtime_failure_is_never_fake_success() {
    use m31a::cli::{CliDispatcher, RuntimeCommand};
    let dispatcher = CliDispatcher::new();
    let run = dispatcher
        .dispatch(RuntimeCommand::RunMission {
            prompt: "do work".to_string(),
            profile: None,
            wait_for_approval: false,
        })
        .await;
    assert!(
        matches!(run, Err(m31a::cli::CliError::ExecutionFailed(_))),
        "RunMission without a runtime must fail, got {run:?}"
    );
}

// ── Invariant 8: no ambient provider authority ───────────────────────────────

static ENV_SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn invariant_8_credential_resolution_is_channel_bound() {
    let _guard = ENV_SERIAL.lock().unwrap();
    let dir = tempfile::tempdir().expect("tempdir");
    let ws = dir.path();
    let prod_file = ws.join(".m31a").join("credentials.json");
    std::fs::create_dir_all(prod_file.parent().unwrap()).unwrap();
    std::fs::write(&prod_file, r#"{"nvidia_nim": "file-key"}"#).unwrap();
    // SAFETY: serialized by ENV_SERIAL within this binary (separate test
    // binaries are separate processes); no other thread touches these vars.
    // Original values are restored before returning.
    let saved_nvidia = std::env::var("NVIDIA_API_KEY").ok();
    let saved_compat = std::env::var("API_KEY_NVIDIA").ok();
    unsafe {
        std::env::set_var("NVIDIA_API_KEY", "env-key");
        std::env::remove_var("API_KEY_NVIDIA");
    }

    // Production channel: channel file wins over environment.
    let res = m31a::runtime_authorities::resolve_runtime_credentials(
        ws,
        m31a::deployment::DeploymentChannel::Production,
    );
    assert_eq!(res.api_key.as_deref(), Some("file-key"));
    assert!(matches!(
        res.source,
        m31a::runtime_authorities::CredentialSource::ChannelFile(_)
    ));

    // Development channel: NEVER reads the production file (isolation).
    let dev = m31a::runtime_authorities::resolve_runtime_credentials(
        ws,
        m31a::deployment::DeploymentChannel::Development,
    );
    assert_eq!(dev.api_key.as_deref(), Some("env-key"));
    assert!(matches!(
        dev.source,
        m31a::runtime_authorities::CredentialSource::Environment("NVIDIA_API_KEY")
    ));

    // SAFETY: see above (ENV_SERIAL held). Restore pre-test values so no
    // other test in this binary can observe the manipulation.
    unsafe {
        match saved_nvidia {
            Some(v) => std::env::set_var("NVIDIA_API_KEY", v),
            None => std::env::remove_var("NVIDIA_API_KEY"),
        }
        match saved_compat {
            Some(v) => std::env::set_var("API_KEY_NVIDIA", v),
            None => std::env::remove_var("API_KEY_NVIDIA"),
        }
    }
}

// ── Invariant 9: channel isolation ───────────────────────────────────────────

#[test]
fn invariant_9_development_and_production_storage_are_isolated() {
    use m31a::deployment::{DeploymentChannel, DeploymentPaths};
    let ws = std::path::Path::new("/tmp/ws");
    let prod = DeploymentChannel::Production;
    let dev = DeploymentChannel::Development;
    assert_ne!(
        DeploymentPaths::project_db_path(ws, prod),
        DeploymentPaths::project_db_path(ws, dev)
    );
    assert_ne!(
        DeploymentPaths::project_credentials_file(ws, prod),
        DeploymentPaths::project_credentials_file(ws, dev)
    );
    assert_ne!(
        DeploymentPaths::project_artifacts_dir(ws, prod),
        DeploymentPaths::project_artifacts_dir(ws, dev)
    );
    assert_ne!(
        DeploymentPaths::project_telemetry_dir(ws, prod),
        DeploymentPaths::project_telemetry_dir(ws, dev)
    );
    assert_ne!(
        DeploymentPaths::project_staging_dir(ws, prod),
        DeploymentPaths::project_staging_dir(ws, dev)
    );
    assert_ne!(
        DeploymentPaths::project_state_dir(ws, prod),
        DeploymentPaths::project_state_dir(ws, dev)
    );
    // Production keeps legacy filenames (backward compatible).
    assert_eq!(
        DeploymentPaths::project_db_path(ws, prod),
        ws.join(".m31a").join("m31a.db")
    );
    assert_eq!(
        DeploymentPaths::project_credentials_file(ws, prod),
        ws.join(".m31a").join("credentials.json")
    );
    assert_eq!(
        DeploymentPaths::project_artifacts_dir(ws, prod),
        ws.join(".m31a").join("artifacts")
    );
}

// ── Invariant 10: one composition root ───────────────────────────────────────

#[tokio::test]
async fn invariant_10_derived_services_share_runtime_authorities() {
    let (_dir, runtime, session_id, _) = setup_runtime().await;
    // Authority set is stable: repeated access yields the same generation.
    assert!(Arc::ptr_eq(runtime.authorities(), runtime.authorities()));
    // Controller bundle mirrors the runtime instances (no second authorities).
    let deps = runtime.dependencies();
    assert!(
        Arc::as_ptr(runtime.policy()) as *const () == Arc::as_ptr(deps.policy()) as *const (),
        "controller policy must BE the runtime policy (same allocation)"
    );
    assert!(Arc::ptr_eq(
        runtime.approval_coordinator(),
        deps.approval_coordinator().expect("coordinator wired")
    ));
    assert!(Arc::ptr_eq(
        runtime.budget_enforcer(),
        deps.budget_enforcer().expect("budget wired")
    ));
    // Engines derive from the same generation.
    let engine = runtime.create_agent_engine(session_id);
    assert!(Arc::ptr_eq(
        engine.capability_registry(),
        runtime.capability_registry()
    ));
}

// ── Invariant 11: role is typed ──────────────────────────────────────────────

#[tokio::test]
async fn invariant_11_role_change_rebinds_execution_authority() {
    let (_dir, runtime, session_id, session_repo) = setup_runtime().await;
    let caller = Arc::new(TestModelCaller::from_proposals(vec![]));
    let gate: Arc<dyn PolicyGate> = Arc::new(CountingGate {
        count: Arc::new(AtomicUsize::new(0)),
        decision: PolicyDecision::Allow,
    });
    let base = bound_test_engine(&runtime, session_id, session_repo, caller, gate, false);
    let implementer_ctx = base.build_execution_context();
    let planner_engine = runtime
        .create_agent_engine(session_id)
        .with_role_authority(AgentRole::planner());
    assert_eq!(*planner_engine.active_role(), AgentRole::planner());
    let planner_ctx = planner_engine.build_execution_context();
    let expected = AgentProfile::built_in(AgentRole::planner()).capability_policy;
    assert_eq!(planner_ctx.role_envelope, Some(expected));
    assert_ne!(
        planner_ctx.role_envelope, implementer_ctx.role_envelope,
        "changing role must change execution authority state"
    );
    assert_eq!(
        planner_engine.invocation_context().role,
        AgentRole::planner()
    );
}

// ── Invariant 12: policy is singular ─────────────────────────────────────────

#[tokio::test]
async fn invariant_12_single_policy_evaluation_per_action() {
    let (_dir, runtime, session_id, session_repo) = setup_runtime().await;
    let caller = Arc::new(TestModelCaller::from_proposals(vec![Ok(
        ModelProposal::ToolCalls {
            calls: vec![ModelToolCall::new(
                "read_file",
                serde_json::json!({ "path": "src/lib.rs" }),
            )],
        },
    )]));
    let count = Arc::new(AtomicUsize::new(0));
    let gate: Arc<dyn PolicyGate> = Arc::new(CountingGate {
        count: count.clone(),
        decision: PolicyDecision::Allow,
    });
    let mut engine = bound_test_engine(&runtime, session_id, session_repo, caller, gate, false);
    let outcome = engine.step(Some("inspect")).await.expect("step");
    assert!(
        matches!(outcome, AgentTurnOutcome::ToolResults { .. }),
        "allowed tool executes, got {outcome:?}"
    );
    assert_eq!(
        count.load(Ordering::SeqCst),
        1,
        "exactly ONE policy evaluation per action: the pipeline's (no engine precheck)"
    );
}

// ── Static regression guards ─────────────────────────────────────────────────

fn read_src(rel: &str) -> String {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(root.join(rel)).expect("read src file")
}

/// Production construction sites are allowlisted per file; every other
/// production file MUST consume shared authorities instead of building them.
#[test]
fn static_no_split_brain_construction_in_downstream_code() {
    // CapabilityRegistry::production: canonical root (runtime), legacy
    // standalone dispatcher ctor, legacy controller branch, definition.
    // cli/dispatch.rs retains EXACTLY ONE site: the standalone
    // `CliDispatcher::production` fallback, always superseded by
    // `with_runtime` on executing paths (asserted by count, not presence).
    for rel in [
        "src/agent/engine.rs",
        "src/interaction/runner.rs",
        "src/interaction/commands.rs",
        "src/pipeline/runner.rs",
        "src/tui/runtime_bridge.rs",
        "src/planning/service.rs",
        "src/planning/review.rs",
        "src/controller/mod.rs",
    ] {
        let src = read_src(rel);
        assert!(
            !src.contains("CapabilityRegistry::production("),
            "{rel} must not construct a capability registry (consume the shared authority)"
        );
    }
    let dispatch = read_src("src/cli/dispatch.rs");
    assert_eq!(
        dispatch
            .match_indices("CapabilityRegistry::production(")
            .count(),
        1,
        "cli/dispatch.rs may retain exactly one standalone-ctor registry site"
    );
    // ToolRegistry::new_default outside the canonical roots is a fork.
    for rel in [
        "src/agent/engine.rs",
        "src/cli/dispatch.rs",
        "src/interaction/commands.rs",
        "src/tui/runtime_bridge.rs",
        "src/pipeline/runner.rs",
    ] {
        let src = read_src(rel);
        assert!(
            !src.contains("ToolRegistry::new_default("),
            "{rel} must not construct a tool registry (consume the shared authority)"
        );
    }
    // Model callers are composed at the root or the legacy dispatcher ctor only.
    for rel in [
        "src/agent/engine.rs",
        "src/cli/dispatch.rs",
        "src/interaction/runner.rs",
        "src/controller/dependencies.rs",
        "src/controller/mod.rs",
    ] {
        let src = read_src(rel);
        assert!(
            !src.contains("RoutedModelCaller::new("),
            "{rel} must not construct a model caller (consume the shared authority)"
        );
    }
    // Context compilers: canonical root, legacy controller branch, and the
    // WorkerRunner offline default only.
    for rel in [
        "src/agent/engine.rs",
        "src/cli/dispatch.rs",
        "src/interaction/runner.rs",
        "src/tui/runtime_bridge.rs",
        "src/controller/mod.rs",
    ] {
        let src = read_src(rel);
        assert!(
            !src.contains("ProductionContextCompiler::new("),
            "{rel} must not construct a context compiler (consume the shared authority)"
        );
    }
}

/// Partial dependency mutation is forbidden on the interactive path: runtime
/// swaps MUST funnel through `rebind_runtime` (which invalidates the engine).
/// The single legitimate assignment site is `rebind_runtime` itself.
#[test]
fn static_no_direct_runtime_swap_in_interaction_runner() {
    let src = read_src("src/interaction/runner.rs");
    let swaps = src.match_indices("self.runtime = ").count();
    assert_eq!(
        swaps, 1,
        "runner must swap runtimes only via rebind_runtime (engine invalidation); found {swaps} direct swaps"
    );
    assert!(
        src.contains("pub fn rebind_runtime("),
        "rebind_runtime funnel must exist"
    );
}

/// The engine default autonomy must be fail-closed; binding is explicit.
#[test]
fn static_engine_autonomy_is_explicit() {
    let src = read_src("src/agent/engine.rs");
    assert!(
        !src.contains("AutonomyMode::Autonomous"),
        "AgentEngine must not hardcode Autonomous execution"
    );
}

/// Prompt-text role inference must not drive tool visibility in the
/// production caller; the single legacy heuristic lives in
/// `runtime_authorities` for `Unspecified` compatibility only.
#[test]
fn static_no_prompt_sniffing_in_production_caller() {
    let authorities = read_src("src/runtime_authorities.rs");
    assert!(
        authorities.contains("pub fn is_legacy_structured_prompt"),
        "the single legacy heuristic must live in runtime_authorities"
    );
    let policy = read_src("src/agent/model_policy.rs");
    // Only the test-double lifecycle caller (explicitly test-only) may still
    // match prompt markers; the production routed caller must not.
    let sniff_sites = policy.match_indices("ROLE: Discovery Analyst").count()
        + policy.match_indices("ROLE: Lead Planner").count();
    assert!(
        sniff_sites <= 2,
        "production model caller must not sniff prompt text for tool visibility"
    );
}

/// `build_fallback` must never appear on runtime execution paths: invalid
/// configuration surfaces as an error there.
#[test]
fn static_no_config_fallback_on_runtime_paths() {
    let src = read_src("src/runtime.rs");
    assert!(
        !src.contains("build_fallback"),
        "AppRuntime must load configuration strictly (invalid != absent)"
    );
}

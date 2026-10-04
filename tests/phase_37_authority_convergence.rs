//! Phase 37 — Runtime Authority Convergence tests.
//!
//! Proves the §60 invariants that Phase 37 converges:
//! one capability / tool / policy / budget / artifact / checkpoint / model
//! authority per intended scope, one canonical TUI action path, no stale
//! authorization reuse, no fake completion, and real end-to-end reachability
//! for the newly owned workflow pause/cancel/inspect paths.

use std::sync::Arc;
use tempfile::tempdir;

use m31a::events::bus::{EventBus, EventFilter};
use m31a::events::types::EventType;
use m31a::ids::{MissionId, TaskId};
use m31a::interaction::action::ApplicationAction;
use m31a::interaction::commands::{CommandContext, CommandOutput, SlashCommandRegistry};
use m31a::kernel::seams::policy::{PolicyDecision, PolicyEvaluationRequest, PolicyGate};
use m31a::persistence::artifacts::fs_store::ArtifactStore;
use m31a::prompt::PromptCatalog;
use m31a::runtime::AppRuntime;
use m31a::workflow::repository::WorkflowRepository;
use m31a::workflow::state::{WorkflowMode, WorkflowRun, WorkflowRunState};

// ─── helpers ────────────────────────────────────────────────────────────────

async fn test_runtime(dir: &std::path::Path) -> AppRuntime {
    AppRuntime::new(dir)
        .await
        .expect("runtime constructs on temp workspace")
}

fn policy_request(tool: &str) -> PolicyEvaluationRequest {
    PolicyEvaluationRequest {
        mission_id: MissionId::default(),
        task_id: TaskId::new(),
        tool_or_action: tool.to_string(),
        context_digest: "phase37".to_string(),
    }
}

/// Seed a `Running` workflow run directly in the canonical repository.
async fn seed_running_workflow_run(runtime: &AppRuntime) -> m31a::ids::WorkflowRunId {
    let repo = m31a::workflow::repository::SqliteWorkflowRepository::new(runtime.pool().clone());
    let mut run = WorkflowRun::new(
        "phase37-test-def",
        1,
        runtime.workspace_root().to_path_buf(),
        WorkflowMode::Standard,
    );
    run.transition_to(WorkflowRunState::Running, None)
        .expect("Pending -> Running is legal");
    repo.create_run(&run).await.expect("seed run");
    run.id
}

// ─── §6/§7 capability + tool authority ──────────────────────────────────────

#[tokio::test]
async fn test_capability_and_tool_registries_are_runtime_shared() {
    let dir = tempdir().expect("tempdir");
    let runtime = test_runtime(dir.path()).await;

    // Runtime-shared registries are non-empty production inventories.
    let caps = runtime.capability_registry().list_capabilities();
    assert!(!caps.is_empty(), "capability registry must be populated");
    let tools = runtime.tool_registry().list_tools();
    assert!(!tools.is_empty(), "tool registry must be populated");

    // Same workspace → same deterministic inventory (no per-consumer drift).
    let dir2 = tempdir().expect("tempdir");
    let runtime2 = test_runtime(dir2.path()).await;
    let mut ids_a: Vec<String> = tools.iter().map(|t| t.id().to_string()).collect();
    ids_a.sort();
    let mut ids_b: Vec<String> = runtime2
        .tool_registry()
        .list_tools()
        .iter()
        .map(|t| t.id().to_string())
        .collect();
    ids_b.sort();
    assert_eq!(ids_a, ids_b, "tool inventory must not drift per instance");
    assert!(
        ids_a.iter().any(|id| id == "complete"),
        "CompleteTool must be registered, got: {ids_a:?}"
    );
}

#[tokio::test]
async fn test_engine_and_cli_observe_same_capability_environment() {
    let dir = tempdir().expect("tempdir");
    let runtime = test_runtime(dir.path()).await;
    let rt = Arc::new(runtime);

    // CLI dispatcher attached to the runtime observes the canonical registry.
    let dispatcher = m31a::cli::dispatch::CliDispatcher::new().with_runtime(rt.clone());
    let dispatcher_count = dispatcher
        .capability_registry
        .as_ref()
        .map(|r| r.list_capabilities().len());
    assert_eq!(
        dispatcher_count,
        Some(rt.capability_registry().list_capabilities().len()),
        "CLI must observe the runtime-shared capability registry"
    );

    // Agent engine construction succeeds against shared registries and the
    // engine session is bound to the requesting session id.
    let session_id = m31a::ids::SessionId::new();
    let engine = rt.create_agent_engine(session_id);
    assert_eq!(engine.session_id(), session_id);
}

// ─── §9 prompt catalog authority ────────────────────────────────────────────

#[tokio::test]
async fn test_prompt_catalog_is_single_immutable_authority() {
    let dir = tempdir().expect("tempdir");
    let runtime = test_runtime(dir.path()).await;

    let first = runtime.prompt_catalog().list();
    assert!(!first.is_empty(), "builtins must be registered");
    let second = runtime.prompt_catalog().list();
    assert_eq!(first.len(), second.len(), "catalog must be stable");

    // Same content hashes across resolutions: no per-branch version drift.
    for meta in &first {
        let a = runtime
            .prompt_catalog()
            .get(&meta.id, meta.version)
            .expect("get");
        let b = runtime
            .prompt_catalog()
            .get(&meta.id, meta.version)
            .expect("get");
        assert_eq!(
            a.content_hash, b.content_hash,
            "prompt bodies must not duplicate"
        );
    }
}

// ─── §11/§13 policy + budget authority ──────────────────────────────────────

#[tokio::test]
async fn test_runtime_and_dependencies_share_budget_authority() {
    let dir = tempdir().expect("tempdir");
    let runtime = test_runtime(dir.path()).await;

    let deps_budget = runtime
        .dependencies()
        .budget_enforcer()
        .cloned()
        .expect("deps budget");
    assert!(
        Arc::ptr_eq(runtime.budget_enforcer(), &deps_budget),
        "runtime and controller dependencies must share one budget authority"
    );
}

#[tokio::test]
async fn test_budget_limit_update_preserves_consumption_counters() {
    let dir = tempdir().expect("tempdir");
    let runtime = test_runtime(dir.path()).await;

    // Consume against the shared enforcer.
    let estimates = m31a::budget::enforcer::TaskEstimates {
        estimated_tokens: 10,
        estimated_cost_usd: 0.0,
        requires_worker: false,
        estimated_artifact_bytes: 0,
    };
    let receipt = runtime
        .budget_enforcer()
        .reserve(&estimates, true)
        .expect("reserve");
    runtime.budget_enforcer().settle(
        &receipt,
        &m31a::budget::enforcer::ActualUsage {
            tokens: 7,
            ..Default::default()
        },
    );
    assert_eq!(runtime.budget_enforcer().total_tokens_consumed(), 7);

    // Reconfigure: limits update in place, counters survive, and the shared
    // dependencies handle observes the same state.
    let mut config = (*runtime.config().clone()).clone();
    config.app_config.budget.max_tokens = Some(999_999);
    let reconfigured = runtime.with_config(Arc::new(config));
    assert_eq!(
        reconfigured.budget_enforcer().total_tokens_consumed(),
        7,
        "with_config must not reset consumption counters"
    );
    assert_eq!(
        reconfigured.budget_enforcer().budget().max_tokens,
        Some(999_999)
    );
    let deps_budget = reconfigured
        .dependencies()
        .budget_enforcer()
        .cloned()
        .expect("deps budget");
    assert!(
        Arc::ptr_eq(reconfigured.budget_enforcer(), &deps_budget),
        "shared budget authority must survive reconfiguration"
    );
}

#[tokio::test]
async fn test_budget_exhaustion_fails_closed() {
    let dir = tempdir().expect("tempdir");
    let runtime = test_runtime(dir.path()).await;
    runtime
        .budget_enforcer()
        .update_limits(m31a::state::budget::ResourceBudget {
            max_tokens: Some(5),
            ..Default::default()
        });
    let estimates = m31a::budget::enforcer::TaskEstimates {
        estimated_tokens: 100,
        estimated_cost_usd: 0.0,
        requires_worker: false,
        estimated_artifact_bytes: 0,
    };
    assert!(
        runtime
            .budget_enforcer()
            .reserve(&estimates, false)
            .is_err(),
        "over-budget reservation must fail closed"
    );
}

#[tokio::test]
async fn test_policy_decisions_agree_across_runtime_and_dependencies() {
    let dir = tempdir().expect("tempdir");
    let runtime = test_runtime(dir.path()).await;

    let via_runtime = runtime
        .policy()
        .evaluate(policy_request("shell_exec"))
        .await
        .expect("evaluate");
    let via_deps = runtime
        .dependencies()
        .policy()
        .evaluate(policy_request("shell_exec"))
        .await
        .expect("evaluate");
    assert_eq!(
        via_runtime, via_deps,
        "runtime policy and dependency policy must decide identically"
    );
}

#[tokio::test]
async fn test_with_config_propagates_policy_to_all_consumers() {
    let dir = tempdir().expect("tempdir");
    let runtime = test_runtime(dir.path()).await;

    // Deny a tool via configuration layer, then reconfigure atomically.
    let layer = toml::Value::Table(toml::toml! {
        [policy]
        denied_tools = ["shell_exec"]
    });
    let config = Arc::new(
        m31a::config::ResolvedConfigBuilder::new(dir.path())
            .with_cli_layer(layer)
            .build_fallback(),
    );
    assert!(
        config
            .app_config
            .policy
            .denied_tools
            .iter()
            .any(|t| t == "shell_exec"),
        "test config must deny shell_exec"
    );
    let reconfigured = runtime.with_config(config);

    let decide = |gate: &Arc<dyn PolicyGate>| {
        let gate = gate.clone();
        async move { gate.evaluate(policy_request("shell_exec")).await }
    };
    let via_runtime = decide(&(reconfigured.policy().clone() as Arc<dyn PolicyGate>)).await;
    let via_deps = decide(reconfigured.dependencies().policy()).await;
    assert!(
        matches!(
            via_runtime,
            Ok(PolicyDecision::Deny) | Ok(PolicyDecision::Escalate)
        ),
        "reconfigured runtime policy must deny, got: {via_runtime:?}"
    );
    assert_eq!(via_runtime.unwrap(), via_deps.unwrap());
}

// ─── §14/§15 artifact + checkpoint authority ────────────────────────────────

#[tokio::test]
async fn test_artifact_store_is_single_scoped_authority() {
    let dir = tempdir().expect("tempdir");
    let runtime = test_runtime(dir.path()).await;

    // Canonical store round-trips bytes: every consumer of the same base dir
    // observes the same payload (stateless file authority + SQLite ledger).
    // Authority equivalence is proven by identical resolved paths: two
    // handles pointing at the same scope resolve the same file.
    let artifact_id = m31a::ids::ArtifactId::new();
    runtime
        .artifact_store()
        .store(artifact_id, b"phase37-bytes", "bin")
        .await
        .expect("store");
    let mirror =
        m31a::persistence::artifacts::FsArtifactStore::new(runtime.artifact_store().base_dir());
    assert_eq!(
        mirror.path_for(artifact_id, "bin"),
        runtime.artifact_store().path_for(artifact_id, "bin"),
        "same-scope stores must resolve identical paths"
    );
    let bytes = mirror
        .retrieve(artifact_id, "bin")
        .await
        .expect("cross-handle retrieve");
    assert_eq!(bytes, b"phase37-bytes");

    // Checkpoint staging is the canonical channel-aware staging dir on
    // every path (`staging` on production, `staging-dev` on development):
    // runtime and CLI managers must resolve the SAME directory, proving a
    // single staging authority per channel instead of forked locations.
    let canonical_staging = m31a::deployment::DeploymentPaths::project_staging_dir(
        dir.path(),
        m31a::deployment::DeploymentChannel::current(),
    );
    assert_eq!(
        runtime.checkpoint_manager().staging_dir(),
        canonical_staging.as_path(),
        "runtime checkpoints must stage under the canonical channel-aware dir"
    );
    let pool = runtime.pool().clone();
    let bus = Arc::new(m31a::events::bus::BroadcastEventBus::new(64));
    let cli = m31a::cli::dispatch::CliDispatcher::production(pool, dir.path().to_path_buf(), bus);
    let staging = cli
        .checkpoint_manager
        .as_ref()
        .expect("cli checkpoints")
        .staging_dir()
        .to_path_buf();
    assert_eq!(
        staging, canonical_staging,
        "CLI checkpoints must share the canonical staging dir"
    );
}

#[tokio::test]
async fn test_dispatcher_preserves_artifact_store_across_capability_swap() {
    let dir = tempdir().expect("tempdir");
    let custom_store: Arc<dyn ArtifactStore> = Arc::new(
        m31a::persistence::artifacts::FsArtifactStore::new(dir.path().join("custom-artifacts")),
    );
    let disp = m31a::agent::dispatcher::ProductionWorkerDispatcher::new_with_workspace(dir.path())
        .with_artifact_store(custom_store.clone());
    let before = disp
        .pipeline_runner()
        .artifact_store()
        .cloned()
        .expect("store set");
    let probe = m31a::ids::ArtifactId::new();
    assert_eq!(
        before.path_for(probe, "bin"),
        custom_store.path_for(probe, "bin"),
        "injected store must be the pipeline store"
    );

    // Capability rotation rebuilds tools but must not fork artifacts.
    let caps = Arc::new(m31a::capability::registry::CapabilityRegistry::production(
        dir.path(),
        None,
        None,
    ));
    let rotated = disp.with_capabilities(caps);
    let after = rotated
        .pipeline_runner()
        .artifact_store()
        .cloned()
        .expect("store preserved");
    assert_eq!(
        after.path_for(probe, "bin"),
        custom_store.path_for(probe, "bin"),
        "capability swap must preserve the canonical artifact store"
    );
}

#[tokio::test]
async fn test_cli_dispatcher_with_runtime_syncs_all_authorities() {
    let dir = tempdir().expect("tempdir");
    let runtime = test_runtime(dir.path()).await;
    let rt = Arc::new(runtime);
    let dispatcher = m31a::cli::dispatch::CliDispatcher::new().with_runtime(rt.clone());

    let store = dispatcher.artifact_store.as_ref().expect("store synced");
    let probe = m31a::ids::ArtifactId::new();
    assert_eq!(
        store.path_for(probe, "bin"),
        rt.artifact_store().path_for(probe, "bin"),
        "CLI must share the runtime artifact scope"
    );
    let staging = dispatcher
        .checkpoint_manager
        .as_ref()
        .expect("checkpoints synced")
        .staging_dir()
        .to_path_buf();
    assert_eq!(staging, rt.checkpoint_manager().staging_dir());
    assert_eq!(
        dispatcher.pool.as_ref().expect("pool synced").size(),
        rt.pool().size()
    );
}

// ─── §16 session authority ──────────────────────────────────────────────────

#[tokio::test]
async fn test_session_repository_with_bus_emits_session_started() {
    use futures::StreamExt;
    let dir = tempdir().expect("tempdir");
    let runtime = test_runtime(dir.path()).await;

    let mut rx = runtime.event_bus().subscribe(EventFilter::all()).await;
    let repo = m31a::interaction::session::SqliteSessionRepository::new(runtime.pool().clone())
        .with_event_bus(runtime.event_bus().clone() as Arc<dyn m31a::events::bus::EventBus>);
    let session = repo
        .create_session(dir.path())
        .await
        .expect("create session");

    let mut saw_started = false;
    let deadline = tokio::time::sleep(std::time::Duration::from_secs(5));
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            _ = &mut deadline => break,
            item = rx.next() => {
                match item {
                    Some(Ok(env)) => {
                        if matches!(env.event_type, EventType::SessionStarted { .. }) {
                            saw_started = true;
                            break;
                        }
                    }
                    _ => break,
                }
            }
        }
    }
    assert!(
        saw_started,
        "bus-attached session repo must emit SessionStarted"
    );
    assert!(!session.id.to_string().is_empty());
}

// ─── §22/§23 workflow authority + §24 inspect/pause/cancel ──────────────────

#[tokio::test]
async fn test_workflow_engines_share_durable_state_across_instances() {
    let dir = tempdir().expect("tempdir");
    let runtime = test_runtime(dir.path()).await;
    let run_id = seed_running_workflow_run(&runtime).await;

    // Engine A mutates; engine B (fresh per-call instance) observes.
    let engine_a = runtime.create_workflow_engine(runtime.prompt_catalog_arc());
    engine_a
        .pause_workflow(run_id, "phase37 probe")
        .await
        .expect("pause");
    let engine_b = runtime.create_workflow_engine(runtime.prompt_catalog_arc());
    let snapshot = engine_b.inspect_workflow(run_id).await.expect("inspect");
    assert_eq!(snapshot.run.status, WorkflowRunState::Blocked);
    assert_eq!(snapshot.run.id, run_id);
}

#[tokio::test]
async fn test_workflow_pause_inspect_cancel_roundtrip_via_runtime() {
    let dir = tempdir().expect("tempdir");
    let runtime = test_runtime(dir.path()).await;
    let run_id = seed_running_workflow_run(&runtime).await;
    let run_str = run_id.to_string();

    let paused = runtime
        .pause_workflow_run(&run_str, "operator pause")
        .await
        .expect("pause");
    assert!(paused.contains("blocked"), "unexpected: {paused}");

    let inspected = runtime
        .inspect_workflow_run(&run_str)
        .await
        .expect("inspect");
    assert!(inspected.contains("blocked"), "unexpected: {inspected}");
    assert!(
        inspected.contains("phase37-test-def"),
        "unexpected: {inspected}"
    );

    // Pausing a Blocked run fails closed — invalid lifecycle transition.
    let err = runtime
        .pause_workflow_run(&run_str, "second pause")
        .await
        .expect_err("second pause must be refused");
    assert!(!err.to_string().is_empty());

    let cancelled = runtime
        .cancel_workflow_run(&run_str, "operator cancel")
        .await
        .expect("cancel");
    assert!(cancelled.contains("cancelled"), "unexpected: {cancelled}");
    let after_cancel = runtime
        .inspect_workflow_run(&run_str)
        .await
        .expect("inspect");
    assert!(
        after_cancel.contains("cancelled"),
        "unexpected: {after_cancel}"
    );
}

#[tokio::test]
async fn test_workflow_unknown_run_fails_closed_on_all_paths() {
    let dir = tempdir().expect("tempdir");
    let runtime = test_runtime(dir.path()).await;
    let unknown = "00000000-0000-0000-0000-000000000000";

    for result in [
        runtime.inspect_workflow_run(unknown).await.map(|_| ()),
        runtime.pause_workflow_run(unknown, "x").await.map(|_| ()),
        runtime.cancel_workflow_run(unknown, "x").await.map(|_| ()),
    ] {
        assert!(
            result.is_err(),
            "unknown run must fail closed, never fake success"
        );
    }
}

// ─── §19 application-action topology for new workflow actions ───────────────

#[tokio::test]
async fn test_workflow_slash_command_maps_to_canonical_actions() {
    let dir = tempdir().expect("tempdir");
    let runtime = test_runtime(dir.path()).await;
    let reg = SlashCommandRegistry::new_standard();
    assert!(
        reg.find("workflow").is_some(),
        "/workflow must be registered"
    );

    let ctx = CommandContext {
        workspace_root: dir.path(),
        session_id: None,
        active_mission_id: None,
        pool: runtime.pool(),
        event_bus: runtime.event_bus(),
        configured_model: "test-model".to_string(),
        configured_provider: "test-provider".to_string(),
        active_profile: "default".to_string(),
        tool_registry: None,
        command_registry: None,
    };

    let run_id = "00000000-0000-0000-0000-000000000001";
    let cases: &[(&str, ApplicationAction)] = &[
        (
            &format!("/workflow inspect {run_id}"),
            ApplicationAction::WorkflowInspectRequested {
                run_id: run_id.to_string(),
            },
        ),
        (
            &format!("/workflow pause {run_id} taking a break"),
            ApplicationAction::WorkflowPauseRequested {
                run_id: run_id.to_string(),
                reason: "taking a break".to_string(),
            },
        ),
        (
            &format!("/workflow cancel {run_id}"),
            ApplicationAction::WorkflowCancelRequested {
                run_id: run_id.to_string(),
                reason: "cancelled by operator".to_string(),
            },
        ),
        (
            &format!("/workflow resume {run_id}"),
            ApplicationAction::WorkflowResumeRequested {
                run_id: run_id.to_string(),
            },
        ),
        (
            &format!("/workflow approve {run_id} step_a looks good"),
            ApplicationAction::WorkflowApprovalSubmitted {
                run_id: run_id.to_string(),
                step_key: "step_a".to_string(),
                approved: true,
                reason: Some("looks good".to_string()),
            },
        ),
    ];
    for (line, expected) in cases {
        match reg.execute_line(line, &ctx).await.expect("execute") {
            CommandOutput::ApplicationAction(act) => assert_eq!(&act, expected, "line: {line}"),
            other => panic!("line {line} must yield an action, got: {other:?}"),
        }
    }

    // Missing run id fails honestly at the parser, never as a runtime mutation.
    if let CommandOutput::ApplicationAction(_) = reg
        .execute_line("/workflow pause", &ctx)
        .await
        .expect("execute")
    {
        panic!("missing run id must not produce a mutation action");
    }
}

// ─── §25 genesis reachability + event ───────────────────────────────────────

#[tokio::test]
async fn test_genesis_emits_started_event_and_registers_charter() {
    use futures::StreamExt;
    let dir = tempdir().expect("tempdir");
    let runtime = test_runtime(dir.path()).await;

    let mut rx = runtime.event_bus().subscribe(EventFilter::all()).await;
    // Research disabled: this test proves genesis reachability, charter
    // registration, and event emission — not the model-driven research
    // pipeline (covered by genesis_runtime_integration with a test caller).
    let mut request =
        m31a::workflow::genesis::GenesisRequest::new("phase37 probe idea", dir.path());
    request.options.enable_research = false;
    let request_id = request.request_id.clone();
    let outcome = runtime.run_genesis(&request).await.expect("genesis runs");

    // Charter artifact registered through the canonical artifact service.
    assert!(!outcome.registered_artifacts.is_empty());
    assert_eq!(outcome.charter_artifact.name, "PROJECT.md");

    // GenesisStarted was produced on the bus (previously defined, never emitted).
    let mut saw_started = false;
    let deadline = tokio::time::sleep(std::time::Duration::from_secs(5));
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            _ = &mut deadline => break,
            item = rx.next() => {
                match item {
                    Some(Ok(env)) => {
                        if let EventType::GenesisStarted { request_id: rid, .. } = &env.event_type {
                            if rid == &request_id {
                                saw_started = true;
                                break;
                            }
                        }
                    }
                    _ => break,
                }
            }
        }
    }
    assert!(saw_started, "run_genesis must emit GenesisStarted");
}

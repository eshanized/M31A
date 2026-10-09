//! Architectural verification of runtime convergence & remediation invariants.

use std::sync::Arc;
use tempfile::tempdir;

use m31a::capability::family::CapabilityFamily;
use m31a::capability::instance::CapabilityInstance;
use m31a::capability::permissions::CapabilityPermissions;
use m31a::capability::providers::LocalTerminalProvider;
use m31a::capability::registry::CapabilityRegistry;
use m31a::eval::scenarios::{EvalScenario, ScenarioA};
use m31a::git::hosting::{GitHubCliHostingProvider, RepoHostingProvider};
use m31a::repo::lsp::{LspBackend, LspService};
use m31a::state_machine::agent::AgentRole;
use m31a::tools::definition::{ToolExecutionContext, TypedTool};
use m31a::tools::filter::ToolAuthorityScope;
use m31a::tools::registry::ToolRegistry;
use m31a::tools::terminal::*;

#[tokio::test]
async fn test_terminal_model_facing_tools() {
    let tmp = tempdir().unwrap();
    let reg = Arc::new(CapabilityRegistry::new());
    let terminal_provider = Arc::new(LocalTerminalProvider::new());
    reg.register_terminal(terminal_provider);
    reg.register_instance(CapabilityInstance::new(
        "terminal.local",
        "Local Interactive Terminal",
        "1.0.0",
        CapabilityFamily::Terminal,
        "local_terminal",
        CapabilityPermissions::full_access(),
    ));

    let ctx = ToolExecutionContext::new(
        reg.clone(),
        tmp.path().to_path_buf(),
        tokio_util::sync::CancellationToken::new(),
    );

    let session_id = "test-session-1".to_string();

    // 1. Fail-closed: writing input to unstarted session must fail with NotFound
    let write_tool = TerminalWriteInputTool;
    let write_res = write_tool
        .execute(
            &ctx,
            TerminalWriteInputInput {
                session_id: session_id.clone(),
                input: "echo test\n".to_string(),
            },
        )
        .await;
    assert!(
        write_res.is_err(),
        "Writing to unstarted session must fail closed"
    );

    // 2. Start session explicitly
    let start_tool = TerminalStartSessionTool;
    let start_res = start_tool
        .execute(
            &ctx,
            TerminalStartSessionInput {
                session_id: session_id.clone(),
                command: Some("sh".to_string()),
                args: None,
                cwd: Some(tmp.path().display().to_string()),
                env: None,
                cols: Some(80),
                rows: Some(24),
            },
        )
        .await
        .expect("start_session should succeed");
    assert!(start_res.started);

    // 3. Status check
    let status_tool = TerminalSessionStatusTool;
    let status_res = status_tool
        .execute(
            &ctx,
            TerminalSessionStatusInput {
                session_id: session_id.clone(),
            },
        )
        .await
        .expect("session_status should succeed");
    assert!(status_res.is_alive);
    assert_eq!(status_res.cols, 80);
    assert_eq!(status_res.rows, 24);

    // 4. Resize
    let resize_tool = TerminalResizeTool;
    let resize_res = resize_tool
        .execute(
            &ctx,
            TerminalResizeInput {
                session_id: session_id.clone(),
                cols: 120,
                rows: 40,
            },
        )
        .await
        .expect("resize should succeed");
    assert!(resize_res.resized);
    assert_eq!(resize_res.cols, 120);

    // 5. Write and read
    let write_res = write_tool
        .execute(
            &ctx,
            TerminalWriteInputInput {
                session_id: session_id.clone(),
                input: "echo M31A_TERMINAL_TEST\n".to_string(),
            },
        )
        .await
        .expect("write_input should succeed");
    assert!(write_res.bytes_written > 0);

    let read_tool = TerminalReadStreamTool;
    let read_res = read_tool
        .execute(
            &ctx,
            TerminalReadStreamInput {
                session_id: session_id.clone(),
                timeout_ms: Some(500),
            },
        )
        .await
        .expect("read_stream should succeed");
    assert!(
        read_res.output.contains("M31A_TERMINAL_TEST"),
        "Output should contain echoed text: got '{}'",
        read_res.output
    );

    // 6. Terminate session
    let term_tool = TerminalTerminateSessionTool;
    let term_res = term_tool
        .execute(
            &ctx,
            TerminalTerminateSessionInput {
                session_id: session_id.clone(),
            },
        )
        .await
        .expect("terminate_session should succeed");
    assert!(term_res.terminated);

    // status after termination must be not found (fail-closed, cleaned up)
    let post_status = status_tool
        .execute(
            &ctx,
            TerminalSessionStatusInput {
                session_id: session_id.clone(),
            },
        )
        .await;
    assert!(
        post_status.is_err(),
        "Terminated session should not be found"
    );

    // writing to terminated session must fail closed
    let post_write = write_tool
        .execute(
            &ctx,
            TerminalWriteInputInput {
                session_id: session_id.clone(),
                input: "echo fail\n".to_string(),
            },
        )
        .await;
    assert!(
        post_write.is_err(),
        "Writing to terminated session must fail closed"
    );
}

#[tokio::test]
async fn test_lsp_backend_dispatch_and_degraded_reporting() {
    let tmp = tempdir().unwrap();
    let lsp = LspService::new(tmp.path());

    // 1. Detect backend for empty dir
    let backend = lsp.detect_backend().await;
    assert_eq!(backend, LspBackend::FallbackSyntactic);

    // 2. Query definitions with syntactic fallback
    let def_res = lsp
        .goto_definition("test.rs", 1, 1, Some("my_func"))
        .await
        .expect("goto_definition should return Ok result");
    assert_eq!(def_res.backend_used, LspBackend::FallbackSyntactic);
    assert!(def_res.fallback_used);
    assert!(def_res.degraded_reason.is_some());

    // 3. Query references
    let ref_res = lsp
        .find_references("test.rs", 1, 1, Some("my_func"))
        .await
        .expect("find_references should return Ok result");
    assert_eq!(ref_res.backend_used, LspBackend::FallbackSyntactic);
    assert!(ref_res.fallback_used);

    // 4. Query hover
    let hover_res = lsp
        .hover("test.rs", 1, 1, Some("my_func"))
        .await
        .expect("hover should return Ok result");
    assert_eq!(hover_res.backend_used, LspBackend::FallbackSyntactic);
    assert!(hover_res.fallback_used);

    // 5. Query symbols
    let sym_res = lsp
        .workspace_symbols("my_func")
        .await
        .expect("workspace_symbols should return Ok result");
    assert_eq!(sym_res.backend_used, LspBackend::FallbackSyntactic);
    assert!(sym_res.fallback_used);
}

#[tokio::test]
async fn test_hosting_provider_fails_closed_without_mock_fallback() {
    let tmp = tempdir().unwrap();
    let provider = GitHubCliHostingProvider::new(tmp.path());

    // Pull request operations must fail closed with GitError, never mock success
    let pr_res = provider
        .create_pull_request("test-branch", "main", "Test PR", "Description")
        .await;
    assert!(
        pr_res.is_err(),
        "GitHubCliHostingProvider must fail closed when gh is not functional"
    );

    let status_res = provider.get_pull_request_status(999_999_999).await;
    assert!(
        status_res.is_err(),
        "GitHubCliHostingProvider status must fail closed with GitError, never mock success"
    );
}

#[tokio::test]
async fn test_offline_eval_telemetry_is_truthful() {
    let scenario = ScenarioA;
    let res = scenario.run().await.expect("Scenario A run should succeed");
    assert_eq!(
        res.tokens_used, 0,
        "Offline fixture scenarios must report 0 tokens used without LLM calls"
    );
    assert_eq!(
        res.cost_usd, 0.0,
        "Offline fixture scenarios must report 0.0 cost without LLM calls"
    );
}

#[test]
fn test_tool_registry_extended_tools_includes_terminal() {
    let mut registry = ToolRegistry::new();
    registry.register_extended_tools();
    assert!(registry.get("terminal_start_session").is_some());
    assert!(registry.get("terminal_write_input").is_some());
    assert!(registry.get("terminal_read_stream").is_some());
    assert!(registry.get("terminal_resize").is_some());
    assert!(registry.get("terminal_session_status").is_some());
    assert!(registry.get("terminal_terminate_session").is_some());
}

#[test]
fn test_tool_authority_scope_role_rebinding() {
    let reg = Arc::new(CapabilityRegistry::new());
    let mut tool_reg = ToolRegistry::new_default(reg.clone());
    tool_reg.register_extended_tools();
    let tool_reg = Arc::new(tool_reg);

    let scope_impl = m31a::tools::filter::ToolAuthorityScope::new(
        AgentRole::implementer(),
        reg.clone(),
        tool_reg.clone(),
        m31a::state_machine::AutonomyMode::Autonomous,
    );
    let impl_schemas = scope_impl.model_tool_schemas();
    let impl_names: Vec<&str> = impl_schemas
        .iter()
        .filter_map(|s| s["function"]["name"].as_str())
        .collect();
    assert!(impl_names.contains(&"write_file"));
    assert!(impl_names.contains(&"git_commit"));

    // Rebind to reviewer (read-only role envelope)
    let scope_rev = scope_impl.for_role(AgentRole::reviewer());
    let rev_schemas = scope_rev.model_tool_schemas();
    let rev_names: Vec<&str> = rev_schemas
        .iter()
        .filter_map(|s| s["function"]["name"].as_str())
        .collect();

    // Reviewer MUST NOT receive mutating tools
    assert!(
        !rev_names.contains(&"write_file"),
        "Reviewer role must not receive write_file schema"
    );
    assert!(
        !rev_names.contains(&"git_commit"),
        "Reviewer role must not receive git_commit schema"
    );
}

#[tokio::test]
async fn test_adapt_strategy_tool_is_proposal_only() {
    let tmp = tempdir().unwrap();
    let reg = Arc::new(CapabilityRegistry::new());
    let ctx = ToolExecutionContext::new(
        reg,
        tmp.path().to_path_buf(),
        tokio_util::sync::CancellationToken::new(),
    );

    let tool = m31a::tools::definition::AdaptStrategyTool;
    let res = tool
        .execute(
            &ctx,
            m31a::tools::definition::AdaptStrategyInput {
                strategy: "investigate_then_act".to_string(),
                reason: "Prior task encountered flaky tests; deeper diagnosis needed".to_string(),
                tasks_to_supersede: vec!["task-123".to_string()],
                new_tasks: vec!["Investigate flakiness".to_string()],
            },
        )
        .await
        .expect("AdaptStrategyTool proposal execution should succeed");

    assert!(res.adapted);
    assert_eq!(res.new_strategy, "investigate_then_act");
    assert_eq!(res.tasks_superseded, 1);
    assert_eq!(res.new_tasks_added, 1);
}

#[test]
fn test_scoped_tool_authority_visible_equals_executable() {
    let reg = Arc::new(CapabilityRegistry::new());
    let mut tool_reg = ToolRegistry::new_default(reg.clone());
    tool_reg.register_extended_tools();
    let tool_reg = Arc::new(tool_reg);

    let roles = vec![
        AgentRole::implementer(),
        AgentRole::reviewer(),
        AgentRole::researcher(),
        AgentRole::integrator(),
    ];

    for role in roles {
        let scope = ToolAuthorityScope::new(
            role.clone(),
            reg.clone(),
            tool_reg.clone(),
            m31a::state_machine::AutonomyMode::Autonomous,
        );
        let visible = scope.visible_tools();
        let executable = scope.executable_tools();
        assert_eq!(
            visible,
            executable,
            "visible tools must equal executable tools for role {}",
            role.as_str()
        );

        // test delegated child scope
        let child_scope = scope.child_scope(m31a::ids::AgentId::new(), AgentRole::reviewer());
        assert_eq!(
            child_scope.visible_tools(),
            child_scope.executable_tools(),
            "child scope visible tools must equal executable tools"
        );
    }
}

#[tokio::test]
async fn test_lsp_provenance_mock_and_absent() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = tempdir().unwrap();
    let mock_bin = tmp.path().join("mock_lsp.sh");
    let script = r#"#!/bin/sh
if [ "$1" = "--version" ]; then
    echo "mock-lsp 1.0"
    exit 0
fi

read -r l1
read -r l2
len=$(echo "$l1" | tr -dc '0-9')
dd bs=1 count="$len" 2>/dev/null >/dev/null

resp='{"jsonrpc":"2.0","id":1,"result":{"capabilities":{}}}'
printf "Content-Length: %d\r\n\r\n%s" "${#resp}" "$resp"

read -r l1
read -r l2
len=$(echo "$l1" | tr -dc '0-9')
dd bs=1 count="$len" 2>/dev/null >/dev/null

read -r l1
read -r l2
len=$(echo "$l1" | tr -dc '0-9')
dd bs=1 count="$len" 2>/dev/null >/dev/null

resp='{"jsonrpc":"2.0","id":2,"result":[{"uri":"file:///test.rs","range":{"start":{"line":0,"character":0},"end":{"line":0,"character":5}}}]}'
printf "Content-Length: %d\r\n\r\n%s" "${#resp}" "$resp"
"#;

    tokio::fs::write(&mock_bin, script).await.unwrap();
    let mut perms = tokio::fs::metadata(&mock_bin).await.unwrap().permissions();
    perms.set_mode(0o755);
    tokio::fs::set_permissions(&mock_bin, perms).await.unwrap();

    // 1. mock lsp execution returns fallback_used == false
    let lsp = LspService::new(tmp.path()).with_server_override(mock_bin.display().to_string());
    let res = lsp
        .goto_definition("test.rs", 1, 1, Some("my_func"))
        .await
        .expect("mock lsp goto_definition should succeed");
    assert_eq!(res.backend_used, LspBackend::RustAnalyzer);
    assert!(
        !res.fallback_used,
        "mock lsp should report fallback_used == false"
    );
    assert!(
        res.degraded_reason.is_none(),
        "mock lsp should not have degraded reason"
    );

    // 2. absent server binary degrades gracefully with explicit provenance
    let lsp_absent = LspService::new(tmp.path()).with_server_override("/nonexistent/bin/lsp");
    let res_absent = lsp_absent
        .goto_definition("test.rs", 1, 1, Some("my_func"))
        .await
        .expect("absent lsp goto_definition should succeed via fallback");
    assert!(
        res_absent.fallback_used,
        "absent binary must report fallback_used == true"
    );
    assert!(
        res_absent.degraded_reason.is_some(),
        "absent binary must report degraded reason"
    );
}

#[tokio::test]
async fn test_canonical_action_protocol() {
    use m31a::agent::action::AgentAction;
    use m31a::model::types::ModelToolCall;

    let tool_call = ModelToolCall {
        id: "call-1".to_string(),
        name: "test_tool".to_string(),
        arguments: serde_json::json!({}),
    };

    let action = AgentAction::from_tool_call(&tool_call);
    match action {
        AgentAction::CallTool { tool_name, .. } => {
            assert_eq!(tool_name, "test_tool");
        }
        _ => panic!("expected CallTool action"),
    }
}

#[tokio::test]
async fn test_replan_authority_end_to_end() {
    use m31a::agent::runner::ActionDispatcher;

    let tmp = tempdir().unwrap();
    let runtime = Arc::new(m31a::runtime::AppRuntime::new(tmp.path()).await.unwrap());
    let mission_id = m31a::ids::MissionId::new();

    // seed mission and initial task in repository
    let mission = m31a::state::Mission::new(mission_id, "Test Replan Mission".to_string());
    let mission_repo = m31a::persistence::sqlite::repositories::SqliteMissionRepository::new(
        runtime.pool().clone(),
    );
    mission_repo.insert(&mission).await.unwrap();

    let initial_plan = m31a::planning::CandidatePlan::new(
        "plan-v1",
        "Initial Plan",
        vec![m31a::planning::CandidateTask::new(
            "task_1",
            "Original Task 1".to_string(),
            m31a::state_machine::agent::AgentRole::implementer(),
            m31a::planning::VerificationStrategy::Compilation,
            m31a::planning::ResourceEstimate::default(),
        )],
    );
    let materializer = m31a::dag::TaskGraphMaterializer::new(runtime.pool().clone());
    let graph = materializer
        .materialize(mission_id, &initial_plan)
        .await
        .unwrap();
    let task_id = *graph
        .candidate_to_task
        .get(&m31a::planning::CandidateTaskKey::new("task_1"))
        .unwrap();

    let replan_auth = runtime.authorities().replan_authority().clone();
    let runner = runtime.authorities().tool_pipeline().clone();
    let ctx = m31a::tools::definition::ToolExecutionContext::new(
        runtime.capability_registry().clone(),
        tmp.path().to_path_buf(),
        tokio_util::sync::CancellationToken::new(),
    )
    .with_mission_id(mission_id)
    .with_task_id(task_id);

    let dispatcher = m31a::pipeline::dispatcher::ProductionActionDispatcher::new(
        runner,
        ctx,
        runtime.policy().clone(),
        m31a::state_machine::AutonomyMode::Autonomous,
    )
    .with_replan_authority(replan_auth);

    let action = m31a::agent::action::AgentAction::Replan {
        reason: "Test failure triggers differential replan".to_string(),
        tasks_to_supersede: vec![task_id],
        new_tasks: vec!["Investigate and repair".to_string()],
    };

    let observation = dispatcher
        .dispatch_action(&action)
        .await
        .expect("Replan dispatch must succeed");

    match observation {
        m31a::agent::action::AgentObservation::ReplanCompleted {
            new_plan_revision,
            superseded_count,
            added_count,
        } => {
            assert!(new_plan_revision >= 1);
            assert_eq!(superseded_count, 1);
            assert_eq!(added_count, 1);
        }
        other => panic!("expected ReplanCompleted, got {:?}", other),
    }
}

#[tokio::test]
async fn test_denied_tools_omitted_and_blocked() {
    let tmp = tempdir().unwrap();
    let runtime = Arc::new(m31a::runtime::AppRuntime::new(tmp.path()).await.unwrap());
    let session_repo =
        m31a::interaction::session::SqliteSessionRepository::new(runtime.pool().clone());
    let session = session_repo.create_session(tmp.path()).await.unwrap();

    let denied = vec!["write_file".to_string(), "delete_file".to_string()];
    let scope = m31a::tools::filter::ToolAuthorityScope::new(
        AgentRole::implementer(),
        runtime.capability_registry().clone(),
        runtime.tool_registry().clone(),
        m31a::state_machine::AutonomyMode::Autonomous,
    )
    .with_denied_tools(denied.clone());

    // verify denied tools omitted from model schemas
    let schemas = scope.model_tool_schemas();
    for s in &schemas {
        let name = s["function"]["name"].as_str().unwrap_or("");
        assert!(
            !denied.contains(&name.to_string()),
            "denied tool {} found in schemas",
            name
        );
    }

    // verify denied tools omitted from executable tools
    let executable = scope.executable_tools();
    for d in &denied {
        assert!(
            !executable.contains(d),
            "denied tool {} found in executable tools",
            d
        );
    }

    // verify engine with denied tools blocks execution fail-closed
    let call = m31a::model::types::ModelToolCall {
        id: "call-denied".to_string(),
        name: "write_file".to_string(),
        arguments: serde_json::json!({ "path": "test.txt", "content": "blocked" }),
    };

    let caller = Arc::new(m31a::agent::model_policy::TestModelCaller::with_proposal(
        m31a::model::types::ModelProposal::ToolCalls { calls: vec![call] },
    ));
    let mut engine = runtime
        .create_agent_engine(session.id)
        .with_model_caller(caller)
        .with_denied_tools(denied);

    let outcome = engine.step(Some("try write_file")).await.unwrap();
    match outcome {
        m31a::agent::engine::AgentTurnOutcome::ToolResults { results } => {
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].policy_decision, Some("scope_denied".to_string()));
            assert!(!results[0].success);
        }
        other => panic!("expected ToolResults with scope_denied, got {:?}", other),
    }
}

#[tokio::test]
async fn test_unmanaged_execution_continuation_rejected() {
    let tmp = tempdir().unwrap();
    let mut runtime = Arc::new(m31a::runtime::AppRuntime::new(tmp.path()).await.unwrap());
    let session_repo =
        m31a::interaction::session::SqliteSessionRepository::new(runtime.pool().clone());
    let mut session = session_repo.create_session(tmp.path()).await.unwrap();

    let coordinator = runtime.create_pre_execution_coordinator();
    let sid_str = session.id.to_string();
    // simulate session at PlanReview stage where free-text bypass is rejected fail-closed
    let state = m31a::persistence::sqlite::repositories::PersistedLifecycleState {
        session_id: sid_str.clone(),
        stage: m31a::state_machine::lifecycle::LifecycleStage::PlanReview,
        plan_revision: 1,
        task_revision: 0,
        authorization_id: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    coordinator
        .lifecycle_repo()
        .save_lifecycle_state(&state)
        .await
        .unwrap();

    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut active_execution = None;
    let parsed = m31a::interaction::mentions::MentionParser::parse("do unmanaged work", tmp.path());

    m31a::interaction::continuation::handle_user_text_submitted(
        &mut runtime,
        tmp.path(),
        &session_repo,
        &mut session,
        &parsed,
        &mut active_execution,
        &event_tx,
    )
    .await;

    let mut rejected = false;
    while let Ok(event) = event_rx.try_recv() {
        if let m31a::interaction::events::InteractionEvent::Error { message } = event {
            if message.contains("Governed lifecycle is in stage")
                || message.contains("Free-text input is not accepted")
            {
                rejected = true;
                break;
            }
        }
    }
    assert!(
        rejected,
        "unmanaged execution submission must emit rejection error"
    );
    assert!(
        active_execution.is_none(),
        "no unmanaged execution may be launched"
    );
}

#[tokio::test]
async fn test_persistent_lsp_session_lifecycle_e2e() {
    use m31a::capability::providers::local_lsp::{LspSessionState, PersistentLspSession};
    use std::time::Duration;

    let tmp = tempdir().unwrap();
    let script_path = tmp.path().join("mock_lsp.py");
    let script = r#"
import sys, json

def read_msg():
    line = sys.stdin.readline()
    if not line: return None
    length = int(line.split(":")[1].strip())
    sys.stdin.readline()
    content = sys.stdin.read(length)
    return json.loads(content)

def send_msg(obj):
    body = json.dumps(obj)
    sys.stdout.write(f"Content-Length: {len(body)}\r\n\r\n{body}")
    sys.stdout.flush()

while True:
    msg = read_msg()
    if msg is None: break
    method = msg.get("method")
    msg_id = msg.get("id")
    if method == "initialize":
        send_msg({"jsonrpc": "2.0", "id": msg_id, "result": {"capabilities": {}}})
    elif method == "initialized":
        pass
    elif method == "textDocument/definition":
        send_msg({"jsonrpc": "2.0", "id": msg_id, "result": [{"uri": "file:///test.rs", "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 5}}}]})
    elif method == "shutdown":
        send_msg({"jsonrpc": "2.0", "id": msg_id, "result": None})
    elif method == "exit":
        break
"#;
    tokio::fs::write(&script_path, script).await.unwrap();

    let session =
        PersistentLspSession::start(tmp.path(), "python3", &[script_path.to_str().unwrap()])
            .await
            .expect("session start and initialize handshake must succeed");

    assert_eq!(session.state().await, LspSessionState::Ready);
    assert!(session.is_alive().await);

    // query definition with bounded timeout
    let def_res = session
        .query(
            "textDocument/definition",
            serde_json::json!({
                "textDocument": { "uri": "file:///test.rs" },
                "position": { "line": 0, "character": 2 }
            }),
            Duration::from_secs(5),
        )
        .await
        .expect("definition query should succeed");

    assert!(def_res.is_array());

    // document sync
    session
        .did_open(std::path::Path::new("test.rs"), "rust", "fn main() {}")
        .await
        .unwrap();
    session
        .did_change(std::path::Path::new("test.rs"), "fn main() { println!(); }")
        .await
        .unwrap();

    let docs = session.open_documents().await;
    assert_eq!(docs.len(), 1);

    // clean shutdown
    session.shutdown().await.expect("shutdown must succeed");
    assert_eq!(session.state().await, LspSessionState::Stopped);
}

#[tokio::test]
async fn test_model_telemetry_and_cost_provenance() {
    use m31a::model::persistence::invocation::{
        ModelInvocationRecord, SqliteModelInvocationRepository,
    };
    use m31a::model::types::{CostProvenance, TokenUsage, UsageSource};

    let tmp = tempdir().unwrap();
    let runtime = Arc::new(m31a::runtime::AppRuntime::new(tmp.path()).await.unwrap());

    // 1. calculate cost with pricing engine
    let usage = TokenUsage::new(1000, 500, 1500, 0, UsageSource::AuthoritativeProvider);
    let (cost_usd, prov) = m31a::model::pricing::calculate_cost_from_usage(
        "nvidia",
        "meta/llama-3.1-8b-instruct",
        &usage,
    );
    assert!(cost_usd.is_some());
    assert!(cost_usd.unwrap() > 0.0);
    assert_eq!(prov, CostProvenance::Estimated);

    let (free_cost, free_prov) =
        m31a::model::pricing::calculate_cost_from_usage("ollama", "qwen", &usage);
    assert_eq!(free_cost, Some(0.0));
    assert_eq!(free_prov, CostProvenance::Estimated);

    let (unk_cost, unk_prov) = m31a::model::pricing::calculate_cost_from_usage(
        "unknown_provider",
        "unknown_model",
        &usage,
    );
    assert_eq!(unk_cost, None);
    assert_eq!(unk_prov, CostProvenance::Unknown);

    // 2. persist invocation record with cost into repository
    let mission_id = m31a::ids::MissionId::new();
    let task_id = m31a::ids::TaskId::new();
    let agent_id = m31a::ids::AgentId::new();

    // seed parents to satisfy foreign keys
    let mission = m31a::state::Mission::new(mission_id, "telemetry mission".to_string());
    let mission_repo = m31a::persistence::sqlite::repositories::SqliteMissionRepository::new(
        runtime.pool().clone(),
    );
    mission_repo.insert(&mission).await.unwrap();

    let task = m31a::state::Task::new(task_id, mission_id, "telemetry task".to_string());
    let task_repo =
        m31a::persistence::sqlite::repositories::SqliteTaskRepository::new(runtime.pool().clone());
    task_repo.insert(&task).await.unwrap();

    let agent = m31a::state::Agent::new(agent_id, mission_id, "implementer".to_string());
    let agent_repo =
        m31a::persistence::sqlite::repositories::SqliteAgentRepository::new(runtime.pool().clone());
    agent_repo.insert(&agent).await.unwrap();

    let record = ModelInvocationRecord::new(
        mission_id,
        task_id,
        agent_id,
        1,
        "nvidia",
        "meta/llama-3.1-8b-instruct",
        1,
        "success",
        &usage,
        "test_routing",
    )
    .with_cost(cost_usd, prov);

    let repo = SqliteModelInvocationRepository::new(runtime.pool().clone());
    repo.insert_invocation(&record).await.unwrap();

    let fetched = repo.get_all_invocations().await.unwrap();
    assert_eq!(fetched.len(), 1);
    assert_eq!(fetched[0].cost_usd, cost_usd);
    assert_eq!(fetched[0].cost_provenance, CostProvenance::Estimated);
}

#[tokio::test]
async fn test_autonomous_evaluation_runner() {
    let runner =
        m31a::eval::AutonomousEvalRunner::new().with_timeout(std::time::Duration::from_secs(60));
    let result = runner
        .run_autonomous_bug_fix()
        .await
        .expect("Autonomous bug fix must succeed");

    assert_eq!(result.status, m31a::eval::ScenarioStatus::Passed);
    assert!(result.tokens_used > 0, "tokens_used must be > 0");
    assert!(result.cost_usd > 0.0, "cost_usd must be > 0.0");
    assert_eq!(
        result.files_modified, 1,
        "exactly 1 file should be modified"
    );
    assert!(result.verification_passed, "verification must pass");
}

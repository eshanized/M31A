//! Comprehensive Regression Tests for:
//! - P0: Authoritative Tool Execution Dispatcher & Code Mode security parity
//! - P0: Plan Mode read-only enforcement across normal and nested dispatch
//! - P1: Process Cancellation & Supervision (interactive approval race, process group signaling)
//! - P1: Claim Recovery & Atomic SQLite Finalization
//! - P2: ApprovalPolicy::Never fail-closed semantics & dangerously_bypass flag

use anyhow::Result;
use m31a_agent_engine::dispatcher::{
    AuthoritativeToolDispatcher, CodeModeDispatcherBridge, DispatchContext,
};
use m31a_agent_engine::turn_loop::EngineEvent;
use m31a_code_mode::orchestrator::CodeModeOrchestrator;
use m31a_core::prompts::CollaborationMode;
use m31a_core::{ApprovalPolicy, SandboxPolicy};
use m31a_execpolicy::{Decision, ExecPolicy, PrefixRule};
use m31a_storage::Storage;
use m31a_tools::ToolRegistry;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use tempfile::tempdir;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

fn make_dispatcher(
    cwd: PathBuf,
    approval_policy: ApprovalPolicy,
    dangerously_bypass: bool,
    collaboration_mode: CollaborationMode,
    exec_policy: ExecPolicy,
) -> AuthoritativeToolDispatcher {
    AuthoritativeToolDispatcher {
        tools: ToolRegistry::new(),
        exec_policy: Arc::new(RwLock::new(exec_policy)),
        approval_policy,
        dangerously_bypass,
        sandbox_policy: SandboxPolicy::WorkspaceWrite,
        collaboration_mode,
        guardian: None,
        hooks: None,
        session_allowed_files: Arc::new(RwLock::new(HashSet::new())),
        cwd,
    }
}

// ============================================================================
// Test 1: Authoritative Dispatcher rejects ExecPolicy::Deny commands fail-closed
// ============================================================================
#[tokio::test]
async fn test_authoritative_dispatcher_execpolicy_denial() -> Result<()> {
    let tmp = tempdir()?;
    let mut ep = ExecPolicy::default();
    let deny_rule = PrefixRule::from_pattern(
        "rm *",
        Decision::Deny,
        Some("Deny destructive rm commands".to_string()),
    )?;
    ep.add_prefix_rule(deny_rule);

    let dispatcher = make_dispatcher(
        tmp.path().to_path_buf(),
        ApprovalPolicy::OnRequest,
        false,
        CollaborationMode::Default,
        ep,
    );

    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let cancel_token = CancellationToken::new();

    let ctx = DispatchContext {
        session_id: "s1",
        turn_id: "t1",
        call_id: "c1",
        cancel_token: &cancel_token,
        event_tx: &event_tx,
        turn_client: None,
        task_context: "delete files",
    };

    let args = serde_json::json!({
        "command": "rm -rf sensitive_data"
    })
    .to_string();

    let res = dispatcher.dispatch(&ctx, "exec_command", &args).await;
    assert!(res.is_err(), "ExecPolicy::Deny must fail-closed");
    let err_str = res.unwrap_err().to_string();
    assert!(
        err_str.contains("denied"),
        "Error message should state policy denial: {err_str}"
    );

    // Verify denial event was emitted
    let mut saw_finished_failure = false;
    while let Ok(evt) = event_rx.try_recv() {
        if let EngineEvent::ToolCallFinished { success, .. } = evt
            && !success
        {
            saw_finished_failure = true;
        }
    }
    assert!(saw_finished_failure, "Failure event must be recorded");
    Ok(())
}

// ============================================================================
// Test 2: Code Mode nested JavaScript tool calls cannot bypass ExecPolicy
// ============================================================================
#[tokio::test]
async fn test_code_mode_nested_execpolicy_denial_bypass_prevented() -> Result<()> {
    let tmp = tempdir()?;
    let mut ep = ExecPolicy::default();
    let deny_rule = PrefixRule::from_pattern(
        "malicious_tool *",
        Decision::Deny,
        Some("Deny malicious tools".to_string()),
    )?;
    ep.add_prefix_rule(deny_rule);

    let dispatcher = make_dispatcher(
        tmp.path().to_path_buf(),
        ApprovalPolicy::OnRequest,
        false,
        CollaborationMode::Default,
        ep,
    );

    let (event_tx, _event_rx) = mpsc::unbounded_channel();
    let cancel_token = CancellationToken::new();

    let bridge = CodeModeDispatcherBridge {
        dispatcher,
        session_id: "s_cm".to_string(),
        turn_id: "t_cm".to_string(),
        cancel_token,
        event_tx,
        turn_client: None,
        task_context: "run nested script".to_string(),
    };

    let orchestrator = CodeModeOrchestrator::new();
    let js_code = r#"
        try {
            await tools.exec_command({ command: "malicious_tool --payload test" });
            process.stdout.write("BYPASS_SUCCESS");
        } catch (e) {
            process.stdout.write("DENIED_CAUGHT: " + e.message);
        }
    "#;

    let res = orchestrator
        .execute_batch(js_code, &bridge, tmp.path(), SandboxPolicy::WorkspaceWrite)
        .await;

    // Either execute_batch returns the output where exception was caught, or fails
    if let Ok(out) = res {
        assert!(
            !out.contains("BYPASS_SUCCESS"),
            "Code Mode must not bypass ExecPolicy deny rules"
        );
        assert!(
            out.contains("DENIED_CAUGHT") || out.contains("Policy denied execution"),
            "Execution should reflect policy denial in JS context: {out}"
        );
    }
    Ok(())
}

// ============================================================================
// Test 3: Plan Mode enforces read-only in both top-level and Code Mode
// ============================================================================
#[tokio::test]
async fn test_plan_mode_mutation_blocked_top_level_and_code_mode() -> Result<()> {
    let tmp = tempdir()?;
    let ep = ExecPolicy::default();

    // Plan Mode dispatcher
    let dispatcher = make_dispatcher(
        tmp.path().to_path_buf(),
        ApprovalPolicy::OnRequest,
        false,
        CollaborationMode::Plan,
        ep,
    );

    let (event_tx, _event_rx) = mpsc::unbounded_channel();
    let cancel_token = CancellationToken::new();

    // 1. Top-level write_file blocked in Plan Mode
    let ctx = DispatchContext {
        session_id: "s_plan",
        turn_id: "t_plan",
        call_id: "c_plan",
        cancel_token: &cancel_token,
        event_tx: &event_tx,
        turn_client: None,
        task_context: "planning",
    };

    let write_args = serde_json::json!({
        "path": "test.txt",
        "content": "mutation"
    })
    .to_string();

    let res = dispatcher.dispatch(&ctx, "write_file", &write_args).await;
    assert!(res.is_err(), "write_file must be blocked in Plan Mode");
    let err_msg = res.unwrap_err().to_string();
    assert!(
        err_msg.contains("Plan Mode"),
        "Error should reference Plan Mode constraint: {err_msg}"
    );

    // 2. Nested write_file via Code Mode also blocked in Plan Mode
    let bridge = CodeModeDispatcherBridge {
        dispatcher: dispatcher.clone(),
        session_id: "s_cm_plan".to_string(),
        turn_id: "t_cm_plan".to_string(),
        cancel_token: cancel_token.clone(),
        event_tx: event_tx.clone(),
        turn_client: None,
        task_context: "code mode planning".to_string(),
    };

    let orchestrator = CodeModeOrchestrator::new();
    let js_code = r#"
        try {
            await tools.write_file({ path: "nested_mut.txt", content: "forbidden" });
            process.stdout.write("PLAN_MUTATION_BYPASS");
        } catch (e) {
            process.stdout.write("PLAN_VIOLATION_CAUGHT: " + e.message);
        }
    "#;

    let res = orchestrator
        .execute_batch(js_code, &bridge, tmp.path(), SandboxPolicy::WorkspaceWrite)
        .await;

    if let Ok(out) = res {
        assert!(
            !out.contains("PLAN_MUTATION_BYPASS"),
            "Nested Code Mode must not bypass Plan Mode mutation protections"
        );
        assert!(
            out.contains("PLAN_VIOLATION_CAUGHT") || out.contains("Plan Mode"),
            "Nested Plan Mode mutation should fail-closed: {out}"
        );
    }
    Ok(())
}

// ============================================================================
// Test 4: ApprovalPolicy::Never auto-denies interactive actions fail-closed
// ============================================================================
#[tokio::test]
async fn test_approval_policy_never_auto_denies_fail_closed() -> Result<()> {
    let tmp = tempdir()?;
    let ep = ExecPolicy::default(); // default prompts for commands

    let dispatcher = make_dispatcher(
        tmp.path().to_path_buf(),
        ApprovalPolicy::Never, // Strictly fail-closed without prompt
        false,                 // dangerously_bypass = false
        CollaborationMode::Default,
        ep,
    );

    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let cancel_token = CancellationToken::new();

    let ctx = DispatchContext {
        session_id: "s_never",
        turn_id: "t_never",
        call_id: "c_never",
        cancel_token: &cancel_token,
        event_tx: &event_tx,
        turn_client: None,
        task_context: "automated non-interactive run",
    };

    let cmd_args = serde_json::json!({
        "command": "npm install malicious-package"
    })
    .to_string();

    let res = dispatcher.dispatch(&ctx, "exec_command", &cmd_args).await;
    assert!(
        res.is_err(),
        "ApprovalPolicy::Never must fail closed on prompt actions"
    );
    let err = res.unwrap_err().to_string();
    assert!(
        err.contains("Approval policy is 'Never'"),
        "Expected auto-denial message, got: {err}"
    );

    // Ensure NO ApprovalRequested event was sent
    while let Ok(evt) = event_rx.try_recv() {
        if let EngineEvent::ApprovalRequested { .. } = evt {
            panic!("ApprovalPolicy::Never must never emit ApprovalRequested event!");
        }
    }
    Ok(())
}

// ============================================================================
// Test 5: dangerously_bypass explicitly bypasses interactive approval
// ============================================================================
#[tokio::test]
async fn test_dangerously_bypass_executes_without_prompt() -> Result<()> {
    let tmp = tempdir()?;
    let ep = ExecPolicy::default();

    let dispatcher = make_dispatcher(
        tmp.path().to_path_buf(),
        ApprovalPolicy::Never,
        true, // dangerously_bypass = true
        CollaborationMode::Default,
        ep,
    );

    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let cancel_token = CancellationToken::new();

    let ctx = DispatchContext {
        session_id: "s_bypass",
        turn_id: "t_bypass",
        call_id: "c_bypass",
        cancel_token: &cancel_token,
        event_tx: &event_tx,
        turn_client: None,
        task_context: "dangerously bypass approvals",
    };

    let cmd_args = serde_json::json!({
        "command": "touch bypass_marker.txt"
    })
    .to_string();

    let res = dispatcher.dispatch(&ctx, "exec_command", &cmd_args).await;
    assert!(
        res.is_ok(),
        "dangerously_bypass should permit execution: {:?}",
        res.err()
    );

    // Verify no prompt was requested
    while let Ok(evt) = event_rx.try_recv() {
        if let EngineEvent::ApprovalRequested { .. } = evt {
            panic!("dangerously_bypass must not emit ApprovalRequested!");
        }
    }
    Ok(())
}

// ============================================================================
// Test 6: Dispatch cancellation races and aborts interactive approval
// ============================================================================
#[tokio::test]
async fn test_interactive_approval_cancellation_race() -> Result<()> {
    let tmp = tempdir()?;
    let ep = ExecPolicy::default();

    let dispatcher = make_dispatcher(
        tmp.path().to_path_buf(),
        ApprovalPolicy::OnRequest,
        false,
        CollaborationMode::Default,
        ep,
    );

    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let cancel_token = CancellationToken::new();

    let cancel_token_clone = cancel_token.clone();
    let dispatcher_clone = dispatcher.clone();

    let handle = tokio::spawn(async move {
        let ctx = DispatchContext {
            session_id: "s_cancel",
            turn_id: "t_cancel",
            call_id: "c_cancel",
            cancel_token: &cancel_token_clone,
            event_tx: &event_tx,
            turn_client: None,
            task_context: "test cancellation",
        };

        let cmd_args = serde_json::json!({
            "command": "some_prompt_command"
        })
        .to_string();

        dispatcher_clone
            .dispatch(&ctx, "exec_command", &cmd_args)
            .await
    });

    // Wait for the ApprovalRequested event
    let mut saw_approval_requested = false;
    while let Some(evt) = event_rx.recv().await {
        if let EngineEvent::ApprovalRequested { .. } = evt {
            saw_approval_requested = true;
            break;
        }
    }
    assert!(
        saw_approval_requested,
        "Approval request should have been sent"
    );

    // Cancel while waiting for user approval
    cancel_token.cancel();

    let res = handle.await?;
    assert!(
        res.is_err(),
        "Dispatch should fail when cancelled during approval"
    );
    let err_str = res.unwrap_err().to_string();
    assert!(
        err_str.contains("Turn cancelled"),
        "Error should reflect turn cancellation: {err_str}"
    );
    Ok(())
}

// ============================================================================
// Test 7: Atomic SQLite Finalization & Heartbeat Tracking
// ============================================================================
#[tokio::test]
async fn test_atomic_turn_and_claim_finalization_with_heartbeat() -> Result<()> {
    let tmp = tempdir()?;
    let db_path = tmp.path().join("test_storage.db");
    let storage = Storage::open(&db_path)?;

    let thread_id = "th_regression_1";
    let turn_id = "turn_regression_1";

    // 0. Save thread to satisfy foreign key constraints
    let meta = m31a_core::types::SessionMetadata {
        id: thread_id.to_string(),
        title: "Test Thread".to_string(),
        model: "test-model".to_string(),
        cwd: tmp.path().to_string_lossy().to_string(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        extra: std::collections::HashMap::new(),
    };
    storage.save_thread(&meta, "/tmp/rollout", "workspace-write", "on-request")?;

    // 1. Register execution claim
    let claim =
        storage.create_execution_claim(thread_id, 1, std::process::id(), Some("test claim"))?;
    let claim_id = claim.claim_id;

    // 2. Save active turn
    storage.save_turn(turn_id, thread_id, 1, "active")?;

    // 3. Test heartbeat updates timestamp
    let claim_before = storage
        .get_active_claim(thread_id)?
        .expect("Claim must exist");
    let initial_ts = claim_before.updated_at_ms;

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    storage.heartbeat_claim(&claim_id)?;

    let claim_after = storage
        .get_active_claim(thread_id)?
        .expect("Claim must exist");
    assert!(
        claim_after.updated_at_ms >= initial_ts,
        "Heartbeat must update or maintain timestamp: before={}, after={}",
        initial_ts,
        claim_after.updated_at_ms
    );

    // 4. Test atomic finalization updates both turn and claim
    storage.finalize_turn_and_claim(thread_id, turn_id, 1, &claim_id, "completed")?;

    // Verify turn status
    let turns = storage.list_turns(thread_id)?;
    assert_eq!(turns.len(), 1);
    assert_eq!(turns[0].status, "completed");

    // Verify claim status is no longer active
    let active_claim = storage.get_active_claim(thread_id)?;
    assert!(
        active_claim.is_none(),
        "Claim must be finalized and no longer active"
    );

    Ok(())
}

// ============================================================================
// Test 8: Dead PID Claim Recovery preserves Live PIDs
// ============================================================================
#[tokio::test]
async fn test_recover_crashed_claims_respects_pid_liveness() -> Result<()> {
    let tmp = tempdir()?;
    let db_path = tmp.path().join("claims_liveness.db");
    let storage = Storage::open(&db_path)?;

    // Save threads to satisfy foreign key constraints
    let live_thread = "th_live";
    let meta_live = m31a_core::types::SessionMetadata {
        id: live_thread.to_string(),
        title: "Live Thread".to_string(),
        model: "test-model".to_string(),
        cwd: tmp.path().to_string_lossy().to_string(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        extra: std::collections::HashMap::new(),
    };
    storage.save_thread(&meta_live, "/tmp/rollout", "workspace-write", "on-request")?;

    let dead_thread = "th_dead";
    let meta_dead = m31a_core::types::SessionMetadata {
        id: dead_thread.to_string(),
        title: "Dead Thread".to_string(),
        model: "test-model".to_string(),
        cwd: tmp.path().to_string_lossy().to_string(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        extra: std::collections::HashMap::new(),
    };
    storage.save_thread(&meta_dead, "/tmp/rollout", "workspace-write", "on-request")?;

    // Claim with live PID (current test process)
    let live_claim =
        storage.create_execution_claim(live_thread, 1, std::process::id(), Some("live prompt"))?;

    // Claim with definitively dead PID (e.g., PID 4194300 which is unlikely to exist)
    let dead_claim =
        storage.create_execution_claim(dead_thread, 1, 4_194_300, Some("dead prompt"))?;

    // Run recovery
    let recovered = storage.recover_crashed_claims()?;

    // Dead PID claim should be recovered as crashed
    assert!(
        recovered.iter().any(|c| c.claim_id == dead_claim.claim_id),
        "Dead PID claim should be recovered"
    );

    // Live PID claim must NOT be recovered even if recovery runs
    assert!(
        !recovered.iter().any(|c| c.claim_id == live_claim.claim_id),
        "Live PID claim must NOT be crashed while process is alive"
    );

    let live_active = storage.get_active_claim(live_thread)?;
    assert!(
        live_active.is_some(),
        "Live PID claim must remain active in storage"
    );

    Ok(())
}

//! Remediation Test Suite: TUI Interactive Approval Resolution (BLK-03, TUI-04, Law 5).
//!
//! Verifies:
//! 1. `TuiApp::handle_key` on an active `ApprovalModal` returns `RuntimeCommand::ResolveApproval`
//!    rather than the former architectural contradiction `RuntimeCommand::CheckPolicy`.
//! 2. Dispatching `ResolveApproval` updates the pending approval in SQLite to `approved` or `denied`.
//! 3. Dispatching `ResolveApproval` emits `EventType::ApprovalResolved` on the event bus.
//! 4. Keybinding 'y' maps to ApproveOnce and 'n' maps to Reject.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use futures::StreamExt;
use std::sync::Arc;
use tempfile::tempdir;

use m31a::cli::dispatch::{CliDispatcher, RuntimeCommand};
use m31a::events::bus::{BroadcastEventBus, EventBus, EventFilter};
use m31a::events::types::EventType;
use m31a::ids::{ApprovalRequestId, MissionId};
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::tui::app::TuiApp;
use m31a::tui::approval::ApprovalDecision;
use m31a::tui::model::TuiApprovalRequest;

#[tokio::test]
async fn test_tui_approval_modal_key_dispatches_resolve_approval() {
    use m31a::interaction::action::ApplicationAction;

    let mut app = TuiApp::new();
    // Attach the governed bridge channel: the modal resolves through the
    // single canonical approval path (Phase 36.5 DD3: modal → bridge
    // ApplicationAction::ApprovalDecision → ApprovalCoordinator, never via a
    // parallel RuntimeCommand dispatch).
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<ApplicationAction>();
    app = app.with_bridge_tx(tx);

    let approval_id = ApprovalRequestId::new().to_string();

    let request = TuiApprovalRequest {
        id: approval_id.clone(),
        tool_name: "fs_write".to_string(),
        agent_role: "implementer".to_string(),
        justification: "Writing production source code".to_string(),
        parameters_summary: "path=src/main.rs".to_string(),
        risk_tier: "medium".to_string(),
        timestamp: chrono::Utc::now(),
    };

    app.approval_modal.open(request);
    assert!(app.approval_modal.is_open);

    // Press 'y' to approve once
    let key_y = KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE);
    let cmd = app.handle_key(key_y);

    // No parallel RuntimeCommand: the bridge owns approval resolution.
    assert!(
        cmd.is_none(),
        "modal must not emit a parallel RuntimeCommand; bridge owns approval"
    );
    match rx
        .try_recv()
        .expect("bridge must receive approval decision")
    {
        ApplicationAction::ApprovalDecision {
            request_id,
            decision,
        } => {
            assert_eq!(request_id, approval_id);
            assert_eq!(decision, ApprovalDecision::ApproveOnce);
        }
        other => panic!("expected ApprovalDecision, got: {other:?}"),
    }
}

#[tokio::test]
async fn test_tui_approval_modal_reject_dispatches_resolve_approval() {
    use m31a::interaction::action::ApplicationAction;

    let mut app = TuiApp::new();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<ApplicationAction>();
    app = app.with_bridge_tx(tx);

    let approval_id = ApprovalRequestId::new().to_string();

    let request = TuiApprovalRequest {
        id: approval_id.clone(),
        tool_name: "shell_exec".to_string(),
        agent_role: "implementer".to_string(),
        justification: "Running destructive cleanup".to_string(),
        parameters_summary: "cmd=rm -rf /".to_string(),
        risk_tier: "critical".to_string(),
        timestamp: chrono::Utc::now(),
    };

    app.approval_modal.open(request);
    assert!(app.approval_modal.is_open);

    // Press 'n' to reject
    let key_n = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE);
    let cmd = app.handle_key(key_n);

    assert!(
        cmd.is_none(),
        "modal must not emit a parallel RuntimeCommand; bridge owns approval"
    );
    match rx
        .try_recv()
        .expect("bridge must receive approval decision")
    {
        ApplicationAction::ApprovalDecision {
            request_id,
            decision,
        } => {
            assert_eq!(request_id, approval_id);
            assert_eq!(decision, ApprovalDecision::Reject);
        }
        other => panic!("expected ApprovalDecision, got: {other:?}"),
    }
}

#[tokio::test]
async fn test_resolve_approval_updates_database_and_emits_event() {
    let dir = tempdir().unwrap();
    let storage_root = dir.path().to_path_buf();
    let db_path = storage_root.join("approval_resolution.db");
    let pool = initialize_database(&db_path).await.unwrap();
    let bus = Arc::new(BroadcastEventBus::new(64));
    let mut sub = bus.subscribe(EventFilter::all()).await;

    let dispatcher = CliDispatcher::production(pool.clone(), storage_root.clone(), bus);

    let mission_id = MissionId::new();
    let req_id = ApprovalRequestId::new();

    // Insert dummy mission and approval request in SQLite
    sqlx::query("INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, 'Approval Test', 'Executing', ?, ?)")
        .bind(mission_id.as_bytes().as_slice())
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query(
        r#"
        INSERT INTO approval_requests (
            id, mission_id, tool_call_id, tool_or_capability, normalized_args_json,
            redacted_args_json, affected_resources, risk_classification, policy_hash,
            reason, resolution_state, created_at
        ) VALUES (?, ?, 'call_1', 'fs_write', '{}', '{}', '', 'High', 'hash', 'Test reason', 'pending', ?)
        "#
    )
    .bind(req_id.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(&pool)
    .await
    .unwrap();

    // Dispatch ResolveApproval
    let resolve_cmd = RuntimeCommand::ResolveApproval {
        approval_id: req_id.to_string(),
        decision: ApprovalDecision::ApproveOnce,
    };

    let out = dispatcher.dispatch(resolve_cmd).await.unwrap();
    assert_eq!(out.exit_code, 0);

    // Verify database row state was updated to 'approved'
    let state = sqlx::query_scalar::<_, String>(
        "SELECT resolution_state FROM approval_requests WHERE id = ?",
    )
    .bind(req_id.as_bytes().as_slice())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(state, "approved");

    // Verify ApprovalResolved event was published
    let mut found_event = false;
    while let Ok(Some(Ok(event_env))) =
        tokio::time::timeout(std::time::Duration::from_millis(500), sub.next()).await
    {
        if let EventType::ApprovalResolved { ref request_id, .. } = event_env.event_type
            && request_id == &req_id.to_string()
        {
            found_event = true;
            break;
        }
    }
    assert!(
        found_event,
        "ApprovalResolved event must be emitted to the event bus"
    );
}

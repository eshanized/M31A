//! Regression tests for App Server Resource Validation and RPC Correctness.
//!
//! Validates method allowlisting, resource existence checks, thread-to-checkpoint correlation,
//! turn steering, and approval/question lifecycle in a single-user local-first runtime.

use m31a_app_server::protocol::{ActiveTurnHandle, AppServer, JsonRpcRequest};
use m31a_core::Config;
use m31a_storage::Storage;
use m31a_tools::ToolRegistry;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

async fn setup_app_server() -> (Arc<AppServer>, Storage, String, String) {
    let config = Arc::new(Config {
        model: "mock-testing-model".to_string(),
        model_provider: Some("mock".to_string()),
        ..Default::default()
    });
    let storage = Storage::open_in_memory().unwrap();
    let tools = Arc::new(ToolRegistry::new());
    let server = Arc::new(AppServer::new(config, storage.clone(), tools).unwrap());

    // Trust workspace for thread creation
    server
        .trust_store
        .lock()
        .unwrap()
        .trust(
            std::path::Path::new("."),
            m31a_trust::TrustLevel::SessionOnly,
        )
        .unwrap();

    // Thread A
    let res_a = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(1)),
            method: "thread/create".to_string(),
            params: Some(serde_json::json!({"cwd": "."})),
        })
        .await;
    let thread_a_id = res_a
        .result
        .unwrap()
        .get("id")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();

    // Thread B
    let res_b = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(2)),
            method: "thread/create".to_string(),
            params: Some(serde_json::json!({"cwd": "."})),
        })
        .await;
    let thread_b_id = res_b
        .result
        .unwrap()
        .get("id")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();

    (server, storage, thread_a_id, thread_b_id)
}

#[tokio::test]
async fn test_active_turn_interrupt_and_steer() {
    let (server, storage, _thread_a_id, thread_b_id) = setup_app_server().await;

    // Simulate active turn on Thread B
    storage
        .save_turn("turn_client_b_123", &thread_b_id, 1, "in_progress")
        .unwrap();
    let (steer_tx, mut steer_rx) = mpsc::unbounded_channel();
    let cancel_token = CancellationToken::new();
    let turn_b_id = "turn_client_b_123".to_string();

    server.active_turns.lock().unwrap().insert(
        turn_b_id.clone(),
        ActiveTurnHandle {
            thread_id: thread_b_id.clone(),
            turn_id: turn_b_id.clone(),
            steer_tx,
            cancel_token: cancel_token.clone(),
        },
    );

    // Steer active turn
    let req_steer = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!(10)),
        method: "turn/steer".to_string(),
        params: Some(serde_json::json!({
            "turn_id": turn_b_id,
            "message": "Focus on security",
        })),
    };
    let res_steer = server.handle_request(req_steer).await;
    assert!(res_steer.error.is_none());
    assert_eq!(steer_rx.recv().await.unwrap(), "Focus on security");

    // Interrupt active turn
    let req_interrupt = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!(11)),
        method: "turn/interrupt".to_string(),
        params: Some(serde_json::json!({
            "turn_id": turn_b_id,
        })),
    };
    let res_interrupt = server.handle_request(req_interrupt).await;
    assert!(res_interrupt.error.is_none());
    assert!(cancel_token.is_cancelled());
}

#[tokio::test]
async fn test_missing_identifiers_rejected_for_interrupt_and_steer() {
    let (server, _storage, _thread_a_id, thread_b_id) = setup_app_server().await;

    let (steer_tx, _steer_rx) = mpsc::unbounded_channel();
    let cancel_token = CancellationToken::new();
    let turn_b_id = "turn_client_b_456".to_string();

    server.active_turns.lock().unwrap().insert(
        turn_b_id.clone(),
        ActiveTurnHandle {
            thread_id: thread_b_id.clone(),
            turn_id: turn_b_id.clone(),
            steer_tx,
            cancel_token: cancel_token.clone(),
        },
    );

    // Empty interrupt params
    let req_interrupt_empty = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!(11)),
        method: "turn/interrupt".to_string(),
        params: Some(serde_json::json!({})),
    };
    let res = server.handle_request(req_interrupt_empty).await;
    assert!(
        res.error.is_some(),
        "Empty params for turn/interrupt must be rejected!"
    );
    assert!(
        !cancel_token.is_cancelled(),
        "Must not cancel arbitrary turn via fallback!"
    );

    // Empty steer params
    let req_steer_empty = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!(12)),
        method: "turn/steer".to_string(),
        params: Some(serde_json::json!({ "message": "Injected steering" })),
    };
    let res_steer = server.handle_request(req_steer_empty).await;
    assert!(
        res_steer.error.is_some(),
        "Empty target identifiers for turn/steer must be rejected!"
    );
}

#[tokio::test]
async fn test_conflicting_identifiers_rejected() {
    let (server, storage, thread_a_id, thread_b_id) = setup_app_server().await;

    storage
        .save_turn("turn_client_b_789", &thread_b_id, 1, "in_progress")
        .unwrap();
    let (steer_tx, _steer_rx) = mpsc::unbounded_channel();
    let cancel_token = CancellationToken::new();
    let turn_b_id = "turn_client_b_789".to_string();

    server.active_turns.lock().unwrap().insert(
        turn_b_id.clone(),
        ActiveTurnHandle {
            thread_id: thread_b_id.clone(),
            turn_id: turn_b_id.clone(),
            steer_tx,
            cancel_token: cancel_token.clone(),
        },
    );

    // Passes thread_a_id, but points turn_id to turn_b_id (which belongs to thread_b_id)
    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!(13)),
        method: "turn/interrupt".to_string(),
        params: Some(serde_json::json!({
            "thread_id": thread_a_id,
            "turn_id": turn_b_id,
        })),
    };

    let res = server.handle_request(req).await;
    assert!(
        res.error.is_some(),
        "Conflicting thread_id and turn_id must be rejected!"
    );
    assert!(!cancel_token.is_cancelled());
}

#[tokio::test]
async fn test_checkpoint_to_thread_resolution_and_existence() {
    let (server, _storage, _thread_a_id, thread_b_id) = setup_app_server().await;

    // Create a valid checkpoint
    let req_cp = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!(20)),
        method: "checkpoint/create".to_string(),
        params: Some(serde_json::json!({
            "thread_id": thread_b_id,
            "label": "Thread B Snapshot",
            "turn_index": 1,
            "modified_files": [],
        })),
    };
    let res_cp = server.handle_request(req_cp).await;
    assert!(res_cp.error.is_none());
    let cp_id = res_cp
        .result
        .unwrap()
        .get("id")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();

    // 1. Non-existent checkpoint fails closed
    let req_restore_nonexistent = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!(21)),
        method: "checkpoint/restore".to_string(),
        params: Some(serde_json::json!({
            "checkpoint_id": "checkpoint_does_not_exist",
        })),
    };
    let res_none = server.handle_request(req_restore_nonexistent).await;
    assert!(
        res_none.error.is_some(),
        "Non-existent checkpoint restore must fail closed"
    );

    // 2. Conflicting thread identifier fails closed
    let req_restore_conflict = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!(22)),
        method: "checkpoint/restore".to_string(),
        params: Some(serde_json::json!({
            "checkpoint_id": cp_id,
            "thread_id": "wrong_conflicting_thread_id",
        })),
    };
    let res_conflict = server.handle_request(req_restore_conflict).await;
    assert!(
        res_conflict.error.is_some(),
        "Conflicting thread identifier must fail closed"
    );

    // 3. Valid checkpoint restore succeeds
    let req_restore = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!(23)),
        method: "checkpoint/restore".to_string(),
        params: Some(serde_json::json!({
            "checkpoint_id": cp_id,
        })),
    };
    let res_restore = server.handle_request(req_restore).await;
    assert!(
        res_restore.error.is_none(),
        "Valid checkpoint restore should succeed"
    );
}

#[tokio::test]
async fn test_pending_approval_correlation_and_resolution() {
    let (server, _storage, _thread_a_id, thread_b_id) = setup_app_server().await;

    let (resp_tx, mut resp_rx) = oneshot::channel();
    let approval = m31a_agent_engine::PendingApproval {
        tool_call: m31a_core::types::ToolCall {
            id: "call_b_1".to_string(),
            call_type: "function".to_string(),
            function: m31a_core::types::FunctionCall {
                name: "exec_command".to_string(),
                arguments: "{\"command\": \"cat /etc/passwd\"}".to_string(),
            },
        },
        reason: "Requires confirmation".to_string(),
        command: Some("cat /etc/passwd".to_string()),
    };

    server.bridge_engine_event(
        "turn_b_active",
        &thread_b_id,
        m31a_agent_engine::EngineEvent::ApprovalRequested {
            approval,
            response_tx: resp_tx,
        },
    );

    // 1. Non-existent approval fails closed
    let req_approve_fake = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!(29)),
        method: "turn/approve".to_string(),
        params: Some(serde_json::json!({
            "approval_id": "nonexistent_approval_id",
            "decision": "allow",
        })),
    };
    let res_fake = server.handle_request(req_approve_fake).await;
    assert!(
        res_fake.error.is_some(),
        "Non-existent approval must fail closed"
    );

    // Find the approval ID
    let approval_id = server
        .pending_approvals
        .lock()
        .unwrap()
        .keys()
        .next()
        .unwrap()
        .clone();

    // 2. Real approval succeeds
    let req_approve = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!(30)),
        method: "turn/approve".to_string(),
        params: Some(serde_json::json!({
            "approval_id": approval_id,
            "decision": "allow",
        })),
    };

    let res = server.handle_request(req_approve).await;
    assert!(res.error.is_none(), "Valid approval must succeed");
    assert_eq!(
        resp_rx.try_recv().unwrap(),
        m31a_elicitation::ApprovalDecision::AllowOnce
    );
}

#[tokio::test]
async fn test_pending_question_correlation_and_resolution() {
    let (server, _storage, _thread_a_id, thread_b_id) = setup_app_server().await;

    let (q_tx, mut q_rx) = oneshot::channel();
    let question = m31a_elicitation::QuestionItem {
        id: "q_b_1".to_string(),
        question: "Should we proceed?".to_string(),
        options: vec!["Yes".to_string(), "No".to_string()],
        is_multi_select: false,
        selected_indices: std::collections::HashSet::new(),
        cursor_idx: 0,
        custom_input: String::new(),
        is_custom_focused: false,
    };

    server.bridge_engine_event(
        "turn_b_active",
        &thread_b_id,
        m31a_agent_engine::EngineEvent::QuestionRequested {
            question,
            response_tx: q_tx,
        },
    );

    // 1. Non-existent question fails closed
    let req_answer_fake = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!(39)),
        method: "turn/answer".to_string(),
        params: Some(serde_json::json!({
            "question_id": "nonexistent_question_id",
            "answers": ["Yes"],
        })),
    };
    let res_fake = server.handle_request(req_answer_fake).await;
    assert!(
        res_fake.error.is_some(),
        "Non-existent question must fail closed"
    );

    let question_id = server
        .pending_questions
        .lock()
        .unwrap()
        .keys()
        .next()
        .unwrap()
        .clone();

    // 2. Real question succeeds
    let req_answer = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!(40)),
        method: "turn/answer".to_string(),
        params: Some(serde_json::json!({
            "question_id": question_id,
            "answers": ["Yes"],
        })),
    };

    let res = server.handle_request(req_answer).await;
    assert!(res.error.is_none(), "Valid answer must succeed");
    assert_eq!(q_rx.try_recv().unwrap(), vec!["Yes".to_string()]);
}

#[tokio::test]
async fn test_unknown_rpc_method_fails_closed() {
    let (server, _storage, _thread_a_id, _thread_b_id) = setup_app_server().await;

    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!(99)),
        method: "unregistered/dangerous_method".to_string(),
        params: None,
    };

    let res = server.handle_request(req).await;
    assert!(res.error.is_some());
    let err = res.error.unwrap();
    assert_eq!(err.code, -32003);
    assert!(err.message.contains("Unknown or unclassified RPC method"));
}

//! Regression tests for P0 App Server Authorization defects.

use m31a_app_server::auth::AuthScope;
use m31a_app_server::protocol::{ActiveTurnHandle, AppServer, JsonRpcRequest};
use m31a_core::Config;
use m31a_storage::Storage;
use m31a_tools::ToolRegistry;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

async fn setup_app_server() -> (
    Arc<AppServer>,
    m31a_app_server::auth::ClientIdentity,
    m31a_app_server::auth::ClientIdentity,
    String,
    String,
) {
    let config = Arc::new(Config {
        model: "mock-testing-model".to_string(),
        model_provider: Some("mock".to_string()),
        ..Default::default()
    });
    let storage = Storage::open_in_memory().unwrap();
    let tools = Arc::new(ToolRegistry::new());
    let server = Arc::new(AppServer::new(config, storage, tools).unwrap());

    // Client A
    let token_a = server.auth_manager.generate_token(
        "client_a",
        "Client A",
        vec![AuthScope::Read, AuthScope::Write, AuthScope::Execute],
        None,
    );
    let identity_a = server.auth_manager.validate_token(&token_a).unwrap();

    // Client B
    let token_b = server.auth_manager.generate_token(
        "client_b",
        "Client B",
        vec![AuthScope::Read, AuthScope::Write, AuthScope::Execute],
        None,
    );
    let identity_b = server.auth_manager.validate_token(&token_b).unwrap();

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

    // Thread A owned by Client A
    let res_a = server
        .handle_request_with_identity(
            JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: Some(serde_json::json!(1)),
                method: "thread/create".to_string(),
                params: Some(serde_json::json!({"cwd": "."})),
            },
            Some(&identity_a),
        )
        .await;
    let thread_a_id = res_a
        .result
        .unwrap()
        .get("id")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();

    // Thread B owned by Client B
    let res_b = server
        .handle_request_with_identity(
            JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: Some(serde_json::json!(2)),
                method: "thread/create".to_string(),
                params: Some(serde_json::json!({"cwd": "."})),
            },
            Some(&identity_b),
        )
        .await;
    let thread_b_id = res_b
        .result
        .unwrap()
        .get("id")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();

    (server, identity_a, identity_b, thread_a_id, thread_b_id)
}

#[tokio::test]
async fn test_client_a_cannot_interrupt_client_b_turn() {
    let (server, identity_a, _identity_b, _thread_a_id, thread_b_id) = setup_app_server().await;

    // Simulate active turn on Thread B
    let (steer_tx, _steer_rx) = mpsc::unbounded_channel();
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

    // Client A attempts to interrupt Client B's turn
    let req_interrupt = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!(10)),
        method: "turn/interrupt".to_string(),
        params: Some(serde_json::json!({
            "turn_id": turn_b_id,
        })),
    };

    let res = server
        .handle_request_with_identity(req_interrupt, Some(&identity_a))
        .await;

    assert!(
        res.error.is_some(),
        "Client A must not be allowed to interrupt Client B's turn!"
    );
    assert!(
        !cancel_token.is_cancelled(),
        "Client B's turn token must not be cancelled by Client A!"
    );
}

#[tokio::test]
async fn test_missing_identifiers_rejected_for_interrupt_and_steer() {
    let (server, identity_a, _identity_b, _thread_a_id, thread_b_id) = setup_app_server().await;

    // Simulate active turn on Thread B
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

    // Empty interrupt params from Client A
    let req_interrupt_empty = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!(11)),
        method: "turn/interrupt".to_string(),
        params: Some(serde_json::json!({})),
    };
    let res = server
        .handle_request_with_identity(req_interrupt_empty, Some(&identity_a))
        .await;
    assert!(
        res.error.is_some(),
        "Empty params for turn/interrupt must be rejected!"
    );
    assert!(
        !cancel_token.is_cancelled(),
        "Must not cancel arbitrary turn via fallback!"
    );

    // Empty steer params from Client A
    let req_steer_empty = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!(12)),
        method: "turn/steer".to_string(),
        params: Some(serde_json::json!({ "message": "Injected steering" })),
    };
    let res_steer = server
        .handle_request_with_identity(req_steer_empty, Some(&identity_a))
        .await;
    assert!(
        res_steer.error.is_some(),
        "Empty target identifiers for turn/steer must be rejected!"
    );
}

#[tokio::test]
async fn test_conflicting_identifiers_rejected() {
    let (server, identity_a, _identity_b, thread_a_id, thread_b_id) = setup_app_server().await;

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

    // Client A passes their own thread_a_id, but points turn_id to turn_b_id!
    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!(13)),
        method: "turn/interrupt".to_string(),
        params: Some(serde_json::json!({
            "thread_id": thread_a_id,
            "turn_id": turn_b_id,
        })),
    };

    let res = server
        .handle_request_with_identity(req, Some(&identity_a))
        .await;
    assert!(
        res.error.is_some(),
        "Conflicting thread_id and turn_id must be rejected!"
    );
    assert!(!cancel_token.is_cancelled());
}

#[tokio::test]
async fn test_client_a_cannot_restore_client_b_checkpoint() {
    let (server, identity_a, identity_b, _thread_a_id, thread_b_id) = setup_app_server().await;

    // Client B creates a checkpoint
    let req_cp = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!(20)),
        method: "checkpoint/create".to_string(),
        params: Some(serde_json::json!({
            "thread_id": thread_b_id,
            "label": "Client B Snapshot",
            "turn_index": 1,
            "modified_files": [],
        })),
    };
    let res_cp = server
        .handle_request_with_identity(req_cp, Some(&identity_b))
        .await;
    assert!(res_cp.error.is_none());
    let cp_id = res_cp
        .result
        .unwrap()
        .get("id")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();

    // Client A attempts to restore Client B's checkpoint
    let req_restore = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!(21)),
        method: "checkpoint/restore".to_string(),
        params: Some(serde_json::json!({
            "checkpoint_id": cp_id,
        })),
    };
    let res_restore = server
        .handle_request_with_identity(req_restore, Some(&identity_a))
        .await;
    assert!(
        res_restore.error.is_some(),
        "Client A must not be allowed to restore Client B's checkpoint!"
    );
}

#[tokio::test]
async fn test_client_a_cannot_approve_client_b_pending_operation() {
    let (server, identity_a, _identity_b, _thread_a_id, thread_b_id) = setup_app_server().await;

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

    // Find the approval ID
    let approval_id = server
        .pending_approvals
        .lock()
        .unwrap()
        .keys()
        .next()
        .unwrap()
        .clone();

    // Client A attempts to approve Client B's approval
    let req_approve = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!(30)),
        method: "turn/approve".to_string(),
        params: Some(serde_json::json!({
            "approval_id": approval_id,
            "decision": "allow",
        })),
    };

    let res = server
        .handle_request_with_identity(req_approve, Some(&identity_a))
        .await;
    assert!(
        res.error.is_some(),
        "Client A must not be allowed to approve Client B's pending approval!"
    );
    assert!(
        resp_rx.try_recv().is_err(),
        "Approval must not have been resolved by Client A!"
    );
}

#[tokio::test]
async fn test_client_a_cannot_answer_client_b_pending_question() {
    let (server, identity_a, _identity_b, _thread_a_id, thread_b_id) = setup_app_server().await;

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

    let question_id = server
        .pending_questions
        .lock()
        .unwrap()
        .keys()
        .next()
        .unwrap()
        .clone();

    // Client A attempts to answer Client B's question
    let req_answer = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!(40)),
        method: "turn/answer".to_string(),
        params: Some(serde_json::json!({
            "question_id": question_id,
            "answers": ["Yes"],
        })),
    };

    let res = server
        .handle_request_with_identity(req_answer, Some(&identity_a))
        .await;
    assert!(
        res.error.is_some(),
        "Client A must not be allowed to answer Client B's pending question!"
    );
    assert!(
        q_rx.try_recv().is_err(),
        "Question must not have been answered by Client A!"
    );
}

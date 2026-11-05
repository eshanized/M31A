//! Deterministic end-to-end integration tests for the M31A Agent Engine.
//!
//! Exercises reactive turn loop, streaming, and tool execution using local TCP wire server.

use anyhow::Result;
use m31a_agent_engine::{AgentEngine, EngineEvent};
use m31a_core::types::ChatMessage;
use m31a_core::{ApprovalPolicy, SandboxPolicy};
use m31a_execpolicy::ExecPolicy;
use m31a_provider::NvidiaNimClient;
use m31a_provider::wire_server::{LocalWireServer, WireServerResponse};
use m31a_storage::Storage;
use m31a_tools::ToolRegistry;
use tempfile::tempdir;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn test_engine_chat_and_reasoning_turn_over_wire_server() -> Result<()> {
    let tmp = tempdir()?;
    let workspace = tmp.path().to_path_buf();

    let server = LocalWireServer::start().await?;
    let mut resp = WireServerResponse::text("The result of 15 + 27 is 42.");
    resp.reasoning = Some("Computing 15 + 27: 15 + 20 = 35, 35 + 7 = 42.".to_string());
    server.enqueue_response(resp);

    let client = NvidiaNimClient::new(
        server.api_base(),
        m31a_core::resolve_stored_api_key()
            .unwrap_or_else(|| "nvapi-runtime-integration-token".to_string()),
        "nvidia/nemotron-3.5-lightning-30b-a3b",
    );
    let storage = Storage::open_in_memory()?;
    let registry = ToolRegistry::new();

    let mut engine = AgentEngine::new(
        client,
        registry,
        storage,
        ApprovalPolicy::Never,
        ExecPolicy::standard(),
        SandboxPolicy::WorkspaceWrite,
        workspace,
    );

    let history = vec![ChatMessage::user(
        "What is 15 + 27? Answer with just the number.",
    )];
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let (_steer_tx, steer_rx) = mpsc::unbounded_channel();
    let cancel = CancellationToken::new();

    let result = engine
        .run_turn("session-wire-1", history, event_tx, steer_rx, cancel, 3)
        .await;

    assert!(result.is_ok(), "Turn failed: {:?}", result.err());

    let mut response_text = String::new();
    let mut reasoning_text = String::new();
    while let Ok(ev) = event_rx.try_recv() {
        match ev {
            EngineEvent::ContentChunk(chunk) => response_text.push_str(&chunk),
            EngineEvent::ReasoningChunk(chunk) => reasoning_text.push_str(&chunk),
            _ => {}
        }
    }

    assert!(
        response_text.contains("42"),
        "Expected 42, got: {}",
        response_text
    );
    assert!(
        !reasoning_text.is_empty(),
        "Reasoning chunks should be recorded"
    );
    Ok(())
}

#[tokio::test]
async fn test_engine_tool_calling_file_creation_over_wire_server() -> Result<()> {
    let tmp = tempdir()?;
    let workspace = tmp.path().to_path_buf();

    let server = LocalWireServer::start().await?;
    // Turn 1: model returns write_file tool call
    let tool_args = serde_json::json!({
        "path": "test_output.txt",
        "content": "M31A Wire Server Verified",
        "replacement_reason": "Writing test file"
    });
    server.enqueue_response(
        WireServerResponse::text("").with_tool_call("write_file", &tool_args.to_string()),
    );
    // Turn 2: model concludes after tool execution
    server.enqueue_response(WireServerResponse::text("File created successfully."));

    let client = NvidiaNimClient::new(
        server.api_base(),
        m31a_core::resolve_stored_api_key()
            .unwrap_or_else(|| "nvapi-runtime-integration-token".to_string()),
        "nvidia/nemotron-3.5-lightning-30b-a3b",
    );
    let storage = Storage::open_in_memory()?;
    let registry = ToolRegistry::new();

    let mut engine = AgentEngine::new(
        client,
        registry,
        storage,
        ApprovalPolicy::Never,
        ExecPolicy::standard(),
        SandboxPolicy::WorkspaceWrite,
        workspace.clone(),
    );

    let prompt = "Use write_file to create test_output.txt";
    let history = vec![ChatMessage::user(prompt)];
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let (_steer_tx, steer_rx) = mpsc::unbounded_channel();
    let cancel = CancellationToken::new();

    let result = engine
        .run_turn(
            "session-wire-tool-1",
            history,
            event_tx,
            steer_rx,
            cancel,
            5,
        )
        .await;

    assert!(result.is_ok(), "Tool turn failed: {:?}", result.err());

    let file_path = workspace.join("test_output.txt");
    assert!(
        file_path.exists(),
        "test_output.txt must be created on disk"
    );
    let content = std::fs::read_to_string(&file_path)?;
    assert_eq!(content, "M31A Wire Server Verified");

    let mut saw_tool_start = false;
    let mut saw_tool_finish = false;
    let mut saw_turn_complete = false;

    while let Ok(ev) = event_rx.try_recv() {
        match ev {
            EngineEvent::ToolCallStarted { name, .. } if name == "write_file" => {
                saw_tool_start = true
            }
            EngineEvent::ToolCallFinished { .. } => saw_tool_finish = true,
            EngineEvent::TurnComplete { .. } => saw_turn_complete = true,
            _ => {}
        }
    }

    assert!(saw_tool_start, "Expected ToolCallStarted");
    assert!(saw_tool_finish, "Expected ToolCallFinished");
    assert!(saw_turn_complete, "Expected TurnComplete");
    Ok(())
}

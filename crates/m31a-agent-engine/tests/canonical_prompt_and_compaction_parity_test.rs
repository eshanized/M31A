//! Canonical System Prompt Architecture & P0 Parity Integration Tests.
//!
//! Validates:
//! - P0-A: System prompt preservation across normal, background, and merged compaction.
//! - P0-B: One canonical runtime prompt pipeline across agent execution and child agents.
//! - P0-C: Exact prompt-to-runtime capability parity (Codex patch spec, Code Mode exec/wait,
//!   permission escalation, subagent depth, worktree isolation).

use anyhow::Result;
use m31a_agent_engine::{AgentEngine, EngineEvent};
use m31a_core::context_compactor::CompactTokenBudget;
use m31a_core::prompts::{CollaborationMode, PromptRuntimeSnapshot, SystemPromptBuilder};
use m31a_core::types::ChatMessage;
use m31a_core::{ApprovalPolicy, SandboxPolicy};
use m31a_execpolicy::ExecPolicy;
use m31a_provider::NvidiaNimClient;
use m31a_provider::wire_server::{LocalWireServer, WireServerResponse};
use m31a_storage::Storage;
use m31a_tools::ToolRegistry;
use std::sync::Arc;
use tempfile::tempdir;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

fn extract_chat_request_body(requests: &[String]) -> Option<serde_json::Value> {
    for req in requests {
        if !req.contains("/chat/completions") {
            continue;
        }
        let Some((_headers, body)) = req.split_once("\r\n\r\n") else {
            continue;
        };
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(body.trim()) {
            return Some(val);
        }
    }
    None
}

#[tokio::test]
async fn test_provider_receives_authoritative_system_prompt_even_if_history_has_none() -> Result<()>
{
    let tmp = tempdir()?;
    let workspace = tmp.path().to_path_buf();
    let server = LocalWireServer::start().await?;

    let unique_prompt = "CANONICAL-TEST-DIRECTIVE-AUTH-991122";
    server.enqueue_response(WireServerResponse::text("Acknowledged instructions."));

    let client = NvidiaNimClient::new(
        server.api_base(),
        "local-wire-test-key",
        "meta/llama-3.3-70b-instruct",
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
    )
    .with_system_prompt(unique_prompt);

    // Call run_turn with ONLY a user message (no system message in history)
    let history = vec![ChatMessage::user("Perform work.")];
    let (event_tx, _event_rx) = mpsc::unbounded_channel();
    let (_steer_tx, steer_rx) = mpsc::unbounded_channel();
    let cancel = CancellationToken::new();

    let res = engine
        .run_turn(
            "session-prompt-test-1",
            history,
            event_tx,
            steer_rx,
            cancel,
            2,
        )
        .await;
    assert!(res.is_ok(), "turn failed: {:?}", res.err());

    // Verify what wire server received
    let requests = server.requests();
    let first_req = extract_chat_request_body(&requests).expect("chat completions request");
    let messages = first_req
        .get("messages")
        .and_then(|m| m.as_array())
        .expect("messages array");

    assert_eq!(messages[0]["role"], "system");
    assert_eq!(
        messages[0]["content"], unique_prompt,
        "Authoritative system instructions must be injected at head of provider request"
    );

    Ok(())
}

#[tokio::test]
async fn test_rebuilding_history_does_not_duplicate_system_prompt() -> Result<()> {
    let tmp = tempdir()?;
    let workspace = tmp.path().to_path_buf();
    let server = LocalWireServer::start().await?;

    let unique_prompt = "NON-DUPLICATE-DIRECTIVE-443322";
    server.enqueue_response(WireServerResponse::text("First response."));
    server.enqueue_response(WireServerResponse::text("Second response."));

    let client = NvidiaNimClient::new(
        server.api_base(),
        "local-wire-test-key",
        "meta/llama-3.3-70b-instruct",
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
    )
    .with_system_prompt(unique_prompt);

    // Pass history that already contains the system message
    let history = vec![
        ChatMessage::system(unique_prompt),
        ChatMessage::user("First query"),
    ];
    let (event_tx, _event_rx) = mpsc::unbounded_channel();
    let (_steer_tx, steer_rx) = mpsc::unbounded_channel();
    let cancel = CancellationToken::new();

    engine
        .run_turn("session-dedup-1", history, event_tx, steer_rx, cancel, 2)
        .await?;

    let requests = server.requests();
    let first_req = extract_chat_request_body(&requests).expect("chat completions request");
    let messages = first_req
        .get("messages")
        .and_then(|m| m.as_array())
        .expect("messages array");

    let sys_count = messages.iter().filter(|m| m["role"] == "system").count();
    assert_eq!(
        sys_count, 1,
        "There must be exactly one system prompt in the request, no duplicates"
    );

    Ok(())
}

#[tokio::test]
async fn test_compaction_preserves_unique_system_prompt_end_to_end() -> Result<()> {
    let tmp = tempdir()?;
    let workspace = tmp.path().to_path_buf();
    let server = LocalWireServer::start().await?;

    let unique_prompt = "END-TO-END-IMMUTABLE-PROMPT-#778811";
    server.enqueue_response(WireServerResponse::text("Response after compaction."));

    let client = NvidiaNimClient::new(
        server.api_base(),
        "local-wire-test-key",
        "meta/llama-3.3-70b-instruct",
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

    // Configure a small token budget so the heavy history triggers compaction
    engine.compactor = Arc::new(m31a_core::context_compactor::ContextCompactor::new(
        CompactTokenBudget {
            max_context_tokens: 1_000,
            trigger_ratio: 0.50, // triggers at 500 tokens
            target_ratio: 0.25,
            reserved_system_tokens: 150,
            max_single_tool_output_tokens: 30,
            ..Default::default()
        },
    ));
    engine.authoritative_system_prompt = Some(unique_prompt.to_string());

    // Build a history that exceeds 500 tokens
    let mut history = vec![ChatMessage::system(unique_prompt)];
    for i in 0..6 {
        history.push(ChatMessage::user(
            format!("Turn {} heavy context payload content string ", i).repeat(30),
        ));
        history.push(ChatMessage::assistant(
            Some(format!("Assistant turn {} response payload ", i).repeat(30)),
            None,
        ));
    }
    history.push(ChatMessage::user("Final prompt triggering compaction."));

    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let (_steer_tx, steer_rx) = mpsc::unbounded_channel();
    let cancel = CancellationToken::new();

    engine
        .run_turn(
            "session-compact-e2e",
            history,
            event_tx,
            steer_rx,
            cancel,
            2,
        )
        .await?;

    let mut saw_compacted_event = false;
    while let Ok(ev) = event_rx.try_recv() {
        if let EngineEvent::Compacted { .. } = ev {
            saw_compacted_event = true;
        }
    }
    assert!(
        saw_compacted_event,
        "Engine must have fired EngineEvent::Compacted"
    );

    // Verify the request sent to the provider retained the exact system prompt byte-for-byte!
    let requests = server.requests();
    let req = extract_chat_request_body(&requests).expect("chat completions request");
    let messages = req["messages"].as_array().unwrap();

    assert_eq!(messages[0]["role"], "system");
    assert_eq!(
        messages[0]["content"], unique_prompt,
        "System prompt must survive compaction byte-for-byte in the outgoing provider request"
    );

    // Verify summary is at index 1 and does not discard the system prompt
    assert_eq!(messages[1]["role"], "system");
    let summary_text = messages[1]["content"].as_str().unwrap();
    assert!(summary_text.starts_with(m31a_core::context_compactor::SUMMARY_PREFIX));

    Ok(())
}

#[test]
fn test_runtime_capability_parity_in_canonical_prompt() {
    let tmp = tempdir().unwrap();
    let snapshot = PromptRuntimeSnapshot::new(
        tmp.path(),
        "nvidia/nemotron-3.5-70b-instruct",
        "WorkspaceWrite",
        CollaborationMode::Default,
    );

    let prompt = SystemPromptBuilder::build_canonical(&snapshot);

    // 1. Patch specification parity
    assert!(prompt.contains("*** Begin Patch"));
    assert!(prompt.contains("*** Update File:"));
    assert!(prompt.contains("*** End Patch"));
    assert!(!prompt.contains("Lark grammar"));
    assert!(!prompt.contains("FREEFORM"));

    // 2. Code Mode parity
    assert!(prompt.contains("`exec`"));
    assert!(prompt.contains("`wait`"));
    assert!(!prompt.contains("code_mode_eval"));

    // 3. Permission tool parity
    assert!(prompt.contains("request_permissions"));
    assert!(prompt.contains("with_additional_permissions"));

    // 4. Subagent depth & worktree isolation
    assert!(prompt.contains("Max spawn depth is 3"));
    assert!(prompt.contains("isolate_worktree: true"));

    // 5. Instruction precedence & untrusted data
    assert!(prompt.contains("Instruction Precedence Hierarchy"));
    assert!(prompt.contains("Untrusted Data Content"));

    // 6. Engineering workflow layers
    assert!(prompt.contains("1. Understand"));
    assert!(prompt.contains("2. Plan"));
    assert!(prompt.contains("3. Implement"));
    assert!(prompt.contains("4. Verify"));
    assert!(prompt.contains("5. Conclude"));
}

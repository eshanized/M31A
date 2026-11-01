//! Live end-to-end integration tests for M31A AgentEngine using the real NVIDIA Nemotron 3 Ultra model.
//!
//! Target:
//! - Model: nvidia/nemotron-3-ultra-550b-a55b
//! - Provider: NVIDIA NIM (https://integrate.api.nvidia.com/v1)
//!
//! Gated by:
//! - Cargo feature: `live-nvidia-tests`
//! - Environment variable: `M31A_RUN_LIVE_NVIDIA_TESTS=1`

use m31a_agent_engine::{AgentEngine, EngineEvent};
use m31a_core::types::ChatMessage;
use m31a_core::{ApprovalPolicy, SandboxPolicy};
use m31a_execpolicy::ExecPolicy;
use m31a_provider::NvidiaNimClient;
use m31a_provider::live_test_support::{LiveTestConfig, NVIDIA_NEMOTRON_3_ULTRA};
use m31a_storage::Storage;
use m31a_tools::ToolRegistry;
use std::sync::Arc;
use tempfile::tempdir;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

fn make_isolated_tool_registry(
    temp_dir: &std::path::Path,
    workspace: &std::path::Path,
) -> ToolRegistry {
    let vault_dir = temp_dir.join("vault");
    std::fs::create_dir_all(&vault_dir).expect("create vault dir");
    let vault =
        Arc::new(m31a_core::LocalEncryptedVault::new(vault_dir).expect("isolated vault creation"));
    let mcp_manager = Arc::new(m31a_tools::McpManager::new(
        m31a_core::config::McpConfig::default(),
    ));
    let storage = Storage::open_in_memory().expect("in-memory storage open");

    let mut registry = ToolRegistry::new_isolated(
        Some(storage),
        vault,
        mcp_manager,
        vec![temp_dir.join("plugins")],
        workspace,
    );
    // Explicit positive allowlist: strictly read_file and write_file
    registry.set_allowed_tools(["read_file", "write_file"]);
    registry
}

#[tokio::test]
#[ignore = "Live NVIDIA NIM integration test requiring M31A_RUN_LIVE_NVIDIA_TESTS=1"]
async fn test_real_live_agent_engine_tool_calling_file_creation() {
    let cfg = LiveTestConfig::load();
    let tmp = tempdir().expect("tempdir");
    let workspace = tmp.path().join("workspace");
    std::fs::create_dir_all(&workspace).expect("create workspace");

    let client = NvidiaNimClient::new(cfg.api_base, &cfg.api_key, cfg.model)
        .with_max_tokens(Some(1024))
        .with_reasoning_budget(Some(cfg.agent_reasoning_budget))
        .with_reasoning_effort(Some(cfg.reasoning_effort.to_string()))
        .with_temperature(Some(cfg.temperature))
        .with_top_p(Some(cfg.top_p));

    assert_eq!(client.model(), NVIDIA_NEMOTRON_3_ULTRA);

    let storage = Storage::open_in_memory().expect("storage open");
    let registry = make_isolated_tool_registry(tmp.path(), &workspace);

    let mut engine = AgentEngine::new(
        client,
        registry,
        storage,
        ApprovalPolicy::Never,
        ExecPolicy::standard(),
        SandboxPolicy::WorkspaceWrite,
        workspace.clone(),
    );

    let prompt = "Use the write_file tool to write a file at path 'live_test.txt' with content 'M31A Real Provider Verified'.";
    let history = vec![ChatMessage::user(prompt)];
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let (_steer_tx, steer_rx) = mpsc::unbounded_channel();
    let cancel = CancellationToken::new();

    let start = std::time::Instant::now();
    let result = engine
        .run_turn(
            "session-real-tool-1",
            history,
            event_tx,
            steer_rx,
            cancel,
            5,
        )
        .await;

    let elapsed = start.elapsed();
    println!(
        "Live AgentEngine write_file turn completed in {} ms",
        elapsed.as_millis()
    );

    assert!(
        result.is_ok(),
        "Real tool calling turn failed: {:?}",
        result.err()
    );

    // 1. Verify result on disk independently of the model's final conversational message
    let file_path = workspace.join("live_test.txt");
    assert!(
        file_path.exists(),
        "live_test.txt must be created on disk by real tool invocation"
    );
    let file_content = std::fs::read_to_string(&file_path).unwrap();
    assert!(
        file_content.contains("M31A Real Provider Verified"),
        "File content mismatch: {file_content}"
    );

    // 2. Assert real engine events were emitted
    let mut saw_tool_started = false;
    let mut saw_tool_finished = false;
    let mut saw_turn_complete = false;

    while let Ok(ev) = event_rx.try_recv() {
        match ev {
            EngineEvent::ToolCallStarted { name, .. } => {
                if name == "write_file" {
                    saw_tool_started = true;
                }
            }
            EngineEvent::ToolCallFinished { .. } => {
                saw_tool_finished = true;
            }
            EngineEvent::TurnComplete { .. } => {
                saw_turn_complete = true;
            }
            _ => {}
        }
    }

    assert!(
        saw_tool_started,
        "Must observe ToolCallStarted for write_file"
    );
    assert!(saw_tool_finished, "Must observe ToolCallFinished");
    assert!(saw_turn_complete, "Must observe TurnComplete");
}

#[tokio::test]
#[ignore = "Live NVIDIA NIM integration test requiring M31A_RUN_LIVE_NVIDIA_TESTS=1"]
async fn test_real_live_agent_engine_file_read_and_reason() {
    let cfg = LiveTestConfig::load();
    let tmp = tempdir().expect("tempdir");
    let workspace = tmp.path().join("workspace");
    std::fs::create_dir_all(&workspace).expect("create workspace");

    // Create synthetic file in temporary workspace
    let question_file = workspace.join("secret_data.txt");
    std::fs::write(
        &question_file,
        "PROJECT_CODENAME: ANDROMEDA_PRIME\nKEY_ID: 98765\n",
    )
    .expect("write synthetic file");

    let client = NvidiaNimClient::new(cfg.api_base, &cfg.api_key, cfg.model)
        .with_max_tokens(Some(1024))
        .with_reasoning_budget(Some(cfg.agent_reasoning_budget))
        .with_reasoning_effort(Some(cfg.reasoning_effort.to_string()))
        .with_temperature(Some(cfg.temperature))
        .with_top_p(Some(cfg.top_p));

    let storage = Storage::open_in_memory().expect("storage open");
    let registry = make_isolated_tool_registry(tmp.path(), &workspace);

    let mut engine = AgentEngine::new(
        client,
        registry,
        storage,
        ApprovalPolicy::Never,
        ExecPolicy::standard(),
        SandboxPolicy::WorkspaceWrite,
        workspace.clone(),
    );

    let prompt = "Read the file 'secret_data.txt' using the read_file tool and report what the PROJECT_CODENAME is.";
    let history = vec![ChatMessage::user(prompt)];
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let (_steer_tx, steer_rx) = mpsc::unbounded_channel();
    let cancel = CancellationToken::new();

    let start = std::time::Instant::now();
    let result = engine
        .run_turn(
            "session-real-read-1",
            history,
            event_tx,
            steer_rx,
            cancel,
            5,
        )
        .await;

    let elapsed = start.elapsed();
    println!(
        "Live AgentEngine read_file turn completed in {} ms",
        elapsed.as_millis()
    );

    assert!(
        result.is_ok(),
        "Real read tool turn failed: {:?}",
        result.err()
    );

    let mut response_text = String::new();
    let mut saw_read_tool = false;

    while let Ok(ev) = event_rx.try_recv() {
        match ev {
            EngineEvent::ToolCallStarted { name, .. } => {
                if name == "read_file" {
                    saw_read_tool = true;
                }
            }
            EngineEvent::ContentChunk(chunk) => {
                response_text.push_str(&chunk);
            }
            _ => {}
        }
    }

    println!("Live response for read test: {}", response_text);
    assert!(
        saw_read_tool,
        "Agent must invoke read_file tool to inspect the synthetic file"
    );
    assert!(
        response_text.contains("ANDROMEDA_PRIME"),
        "Response must contain secret codename read from disk: {}",
        response_text
    );
}

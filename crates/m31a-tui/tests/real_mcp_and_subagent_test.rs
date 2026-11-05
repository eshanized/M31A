//! Real MCP and Subagent Integration Verification (`real_mcp_and_subagent_test.rs`).
//!
//! Verifies Gate C and Gate E requirements:
//! 1. Real local MCP server process lifecycle with live tool execution and dynamic registry sync.
//! 2. Real subagent lifecycle querying EngineAgentRuntime and reflecting in bottom pane tabs.

use m31a_core::Config;
use m31a_core::config::McpServerConfig;
use m31a_tools::agent_runtime::AgentSpawnRequest;
use m31a_tui::App;
use std::collections::HashMap;
use tempfile::tempdir;

const PYTHON_MCP_SERVER: &str = r#"
import sys, json

def send(obj):
    line = json.dumps(obj)
    sys.stdout.write(line + "\n")
    sys.stdout.flush()

for line in sys.stdin:
    if not line.strip():
        continue
    try:
        msg = json.loads(line)
    except Exception:
        continue

    method = msg.get("method")
    msg_id = msg.get("id")

    if method == "initialize":
        send({
            "jsonrpc": "2.0",
            "id": msg_id,
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "test-calc-server", "version": "1.0.0"}
            }
        })
    elif method == "notifications/initialized":
        pass
    elif method == "tools/list":
        send({
            "jsonrpc": "2.0",
            "id": msg_id,
            "result": {
                "tools": [{
                    "name": "add",
                    "description": "Add two numbers",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "a": {"type": "number"},
                            "b": {"type": "number"}
                        },
                        "required": ["a", "b"]
                    }
                }]
            }
        })
    elif method == "tools/call":
        params = msg.get("params", {})
        args = params.get("arguments", {})
        res = args.get("a", 0) + args.get("b", 0)
        send({
            "jsonrpc": "2.0",
            "id": msg_id,
            "result": {
                "content": [{"type": "text", "text": str(res)}],
                "isError": False
            }
        })
"#;

#[tokio::test(flavor = "multi_thread")]
async fn test_real_mcp_server_roundtrip_and_tool_registration() {
    let dir = tempdir().unwrap();
    let server_script = dir.path().join("server.py");
    std::fs::write(&server_script, PYTHON_MCP_SERVER).unwrap();

    let mut mcp_servers = HashMap::new();
    mcp_servers.insert(
        "calc".to_string(),
        McpServerConfig {
            command: "python3".to_string(),
            args: vec![server_script.to_string_lossy().to_string()],
            transport: Some("stdio".to_string()),
            ..Default::default()
        },
    );

    let config = Config {
        model: "meta/llama-3.3-70b-instruct".to_string(),
        api_key: None,
        mcp_servers,
        ..Default::default()
    };

    let mut app = App::new_test(config, dir.path()).expect("create app");

    // 1. Initial state: server not connected, dynamic tool not present
    assert!(!app.mcp_manager.is_server_connected_sync("calc"));
    assert_eq!(app.mcp_manager.get_server_tools_sync("calc").len(), 0);

    // 2. Connect to local MCP server via toggle_mcp_server_sync
    app.toggle_mcp_server_sync("calc");

    // Wait for async spawn & connection
    for _ in 0..60 {
        if app.mcp_manager.is_server_connected_sync("calc") {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    // Verify connected
    assert!(app.mcp_manager.is_server_connected_sync("calc"));
    let tools = app.mcp_manager.get_server_tools_sync("calc");
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0], "add");

    // 3. Verify tool exists in ToolRegistry and is callable
    let has_tool = app
        .tools
        .definitions()
        .iter()
        .any(|d| d.function.name == "mcp__calc__add");
    assert!(
        has_tool,
        "Tool mcp__calc__add must be registered in ToolRegistry"
    );

    let result = app
        .mcp_manager
        .execute_tool("mcp__calc__add", r#"{"a": 15, "b": 27}"#)
        .await
        .expect("execute tool");
    assert_eq!(result.trim(), "42");

    // 4. Disconnect from local MCP server via toggle_mcp_server_sync
    app.toggle_mcp_server_sync("calc");

    for _ in 0..60 {
        if !app.mcp_manager.is_server_connected_sync("calc") {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    // Verify disconnected
    assert!(!app.mcp_manager.is_server_connected_sync("calc"));
    assert_eq!(app.mcp_manager.get_server_tools_sync("calc").len(), 0);

    // Verify tool unregistered from ToolRegistry
    let has_tool_after = app
        .tools
        .definitions()
        .iter()
        .any(|d| d.function.name == "mcp__calc__add");
    assert!(
        !has_tool_after,
        "Tool mcp__calc__add must be unregistered from ToolRegistry"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_real_subagent_lifecycle_and_tab_reflection() {
    let dir = tempdir().unwrap();
    let config = Config {
        model: "meta/llama-3.3-70b-instruct".to_string(),
        api_key: None,
        ..Default::default()
    };

    let mut app = App::new_test(config, dir.path()).expect("create app");

    // Authorize workspace trust
    app.trust_store
        .lock()
        .unwrap()
        .trust(&app.cwd, m31a_trust::TrustLevel::SessionOnly)
        .expect("trust workspace");

    // Spawn a child subagent using the canonical AgentRuntime
    let req = AgentSpawnRequest {
        task_name: "Code Analysis Task".to_string(),
        prompt: "Analyze dependencies".to_string(),
        role_name: "analyst".to_string(),
        model_override: None,
        depth: 1,
        max_depth: 3,
        isolate_worktree: false,
    };

    let spawned = app
        .agent_runtime
        .spawn_agent(req)
        .await
        .expect("spawn subagent");
    let child_id = spawned.agent_id.clone();

    // Query live status from EngineAgentRuntime
    let status_opt = app.engine_runtime.get_status_sync(&child_id);
    assert!(status_opt.is_some(), "Subagent status must be queryable");

    // Record subagent in storage so app detects it
    app.storage
        .upsert_spawn_edge(&app.session_id, &child_id, "Code Analyst", "open", Some(1))
        .expect("save subagent");

    // Update status badges on bottom pane
    app.update_status_badges();

    // Verify subagent tab appears in bottom pane
    assert_eq!(app.bottom_pane.subagent_tabs.len(), 1);
    assert_eq!(app.bottom_pane.subagent_tabs[0].thread_id, child_id);
    assert_eq!(app.bottom_pane.subagent_tabs[0].role, "Code Analyst");

    // Terminate subagent
    let term_res = app.terminate_subagent(&child_id, "Code Analyst");
    assert!(term_res.is_ok(), "Subagent termination should succeed");

    // Wait for async termination to complete
    let mut post_status = None;
    for _ in 0..50 {
        post_status = app.engine_runtime.get_status_sync(&child_id);
        if matches!(
            post_status.as_deref(),
            Some("cancelled") | Some("closed") | Some("terminated")
        ) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    assert!(
        matches!(
            post_status.as_deref(),
            Some("cancelled") | Some("closed") | Some("terminated")
        ),
        "Subagent status should reflect termination, got {:?}",
        post_status
    );
}

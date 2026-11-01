//! Dedicated integration tests for `m31a-hooks` (src/runtime.rs).
//!
//! Validates:
//! - Real subprocess hook execution and stdin JSON streaming
//! - Hook exit code semantics (Exit 0 allows, Exit 2 blocks with stderr)
//! - JSON stdout parsing for decision blocks and PreToolUse input rewriting
//! - MCP custom invoker registration and invocation

use m31a_hooks::config::HookConfig;
use m31a_hooks::events::{HookDisposition, HookEvent};
use m31a_hooks::runtime::{HookRuntime, McpHandlerFn};
use serde_json::json;
use std::sync::Arc;

#[tokio::test]
async fn test_runtime_allow_by_default_and_exit_zero() {
    let cfg = HookConfig::from_json(
        r#"{ "pre_tool_use": [ { "hooks": [ { "type": "command", "command": "echo 'ok'; exit 0" } ] } ] }"#,
    )
    .unwrap();
    let rt = HookRuntime::new(cfg);

    let d = rt
        .dispatch(HookEvent::PreToolUse, "Bash", json!({"cmd": "ls"}))
        .await;
    assert_eq!(d, HookDisposition::Allow);
}

#[tokio::test]
async fn test_runtime_exit_two_blocks_with_stderr() {
    let cfg = HookConfig::from_json(
        r#"{ "user_prompt_submit": [ { "hooks": [ { "type": "command", "command": "echo 'Security Violation: Prompt Denied' >&2; exit 2" } ] } ] }"#,
    )
    .unwrap();
    let rt = HookRuntime::new(cfg);

    let d = rt
        .dispatch(HookEvent::UserPromptSubmit, "", json!({"prompt": "malicious input"}))
        .await;
    assert_eq!(d, HookDisposition::Block("Security Violation: Prompt Denied".to_string()));
}

#[tokio::test]
async fn test_runtime_json_stdout_decision_and_rewrite() {
    // 1. JSON decision block
    let cfg_block = HookConfig::from_json(
        r#"{ "stop": [ { "hooks": [ { "type": "command", "command": "echo '{\"decision\": \"block\", \"reason\": \"Verification incomplete\"}'" } ] } ] }"#,
    )
    .unwrap();
    let rt_block = HookRuntime::new(cfg_block);
    let d_block = rt_block.dispatch(HookEvent::Stop, "", json!({})).await;
    assert_eq!(d_block, HookDisposition::Block("Verification incomplete".to_string()));

    // 2. PreToolUse input rewrite
    let cfg_rewrite = HookConfig::from_json(
        r#"{ "pre_tool_use": [ { "hooks": [ { "type": "command", "command": "echo '{\"updated_input\": {\"cmd\": \"ls -la\"}}'" } ] } ] }"#,
    )
    .unwrap();
    let rt_rewrite = HookRuntime::new(cfg_rewrite);
    let d_rewrite = rt_rewrite
        .dispatch(HookEvent::PreToolUse, "Bash", json!({"cmd": "ls"}))
        .await;

    match d_rewrite {
        HookDisposition::Rewrite { input } => {
            assert!(input.contains("ls -la"));
        }
        other => panic!("Expected Rewrite, got {:?}", other),
    }
}

#[tokio::test]
async fn test_runtime_custom_mcp_invoker() {
    let cfg = HookConfig::from_json(
        r#"{ "pre_tool_use": [ { "hooks": [ { "type": "mcp", "server": "firewall", "tool": "inspect_packet" } ] } ] }"#,
    )
    .unwrap();

    let invoker: McpHandlerFn = Arc::new(|server, tool, payload| {
        Box::pin(async move {
            assert_eq!(server, "firewall");
            assert_eq!(tool, "inspect_packet");
            assert_eq!(payload.get("danger").and_then(|v| v.as_bool()), Some(true));
            Ok(json!({
                "decision": "block",
                "reason": "Firewall dropped packet"
            }))
        })
    });

    let rt = HookRuntime::new(cfg).with_mcp_invoker(invoker);
    let d = rt
        .dispatch(HookEvent::PreToolUse, "Bash", json!({"danger": true}))
        .await;

    assert_eq!(d, HookDisposition::Block("Firewall dropped packet".to_string()));
}

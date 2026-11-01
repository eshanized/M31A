//! Integration tests for lifecycle hook execution and event propagation.
//!
//! Tests command execution, JSON payload piping, and disposition enforcement across lifecycle seams.

use m31a_hooks::{HookConfig, HookDisposition, HookEvent, HookRuntime};
use serde_json::json;

#[tokio::test]
async fn test_real_hook_execution_rewrite_input() {
    let config_json = r#"{
        "PreToolUse": [
            {
                "matcher": "read_file",
                "hooks": [
                    {
                        "type": "command",
                        "command": "cat - >/dev/null && echo '{\"updated_input\": {\"path\": \"rewritten.txt\"}}'"
                    }
                ]
            }
        ]
    }"#;

    let config = HookConfig::from_json(config_json).expect("parse hook config");
    let runtime = HookRuntime::new(config);

    let disp = runtime
        .dispatch(
            HookEvent::PreToolUse,
            "read_file",
            json!({ "path": "original.txt" }),
        )
        .await;

    match disp {
        HookDisposition::Rewrite { input } => {
            let parsed: serde_json::Value = serde_json::from_str(&input).unwrap();
            assert_eq!(parsed["path"], "rewritten.txt");
        }
        other => panic!("Expected Rewrite, got {:?}", other),
    }
}

#[tokio::test]
async fn test_real_hook_execution_exit_2_blocks() {
    let config_json = r#"{
        "PreToolUse": [
            {
                "matcher": "rm_tool",
                "hooks": [
                    {
                        "type": "command",
                        "command": "echo 'Direct removal is forbidden' >&2; exit 2"
                    }
                ]
            }
        ]
    }"#;

    let config = HookConfig::from_json(config_json).expect("parse hook config");
    let runtime = HookRuntime::new(config);

    let disp = runtime
        .dispatch(
            HookEvent::PreToolUse,
            "rm_tool",
            json!({ "target": "/etc" }),
        )
        .await;

    match disp {
        HookDisposition::Block(reason) => {
            assert!(reason.contains("Direct removal is forbidden"));
        }
        other => panic!("Expected Block, got {:?}", other),
    }
}

#[tokio::test]
async fn test_real_hook_execution_json_block() {
    let config_json = r#"{
        "UserPromptSubmit": [
            {
                "hooks": [
                    {
                        "type": "command",
                        "command": "echo '{\"decision\": \"block\", \"reason\": \"Prompt violates safety policy\"}'"
                    }
                ]
            }
        ]
    }"#;

    let config = HookConfig::from_json(config_json).expect("parse hook config");
    let runtime = HookRuntime::new(config);

    let disp = runtime
        .dispatch(
            HookEvent::UserPromptSubmit,
            "",
            json!({ "prompt": "malicious prompt" }),
        )
        .await;

    match disp {
        HookDisposition::Block(reason) => {
            assert!(reason.contains("Prompt violates safety policy"));
        }
        other => panic!("Expected Block, got {:?}", other),
    }
}

#[tokio::test]
async fn test_real_hook_timeout_handling() {
    let config_json = r#"{
        "PostToolUse": [
            {
                "hooks": [
                    {
                        "type": "command",
                        "command": "sleep 10",
                        "timeout_seconds": 1
                    }
                ]
            }
        ]
    }"#;

    let config = HookConfig::from_json(config_json).expect("parse hook config");
    let runtime = HookRuntime::new(config);

    // Timeout should fail gracefully without blocking execution
    let disp = runtime
        .dispatch(
            HookEvent::PostToolUse,
            "exec_command",
            json!({ "result": "ok" }),
        )
        .await;

    assert_eq!(disp, HookDisposition::Allow);
}

#[tokio::test]
async fn test_matcher_regex_filtering() {
    let config_json = r#"{
        "PreToolUse": [
            {
                "matcher": "Bash|apply_patch",
                "hooks": [
                    {
                        "type": "command",
                        "command": "echo '{\"decision\": \"block\", \"reason\": \"matched\"}'"
                    }
                ]
            }
        ]
    }"#;

    let config = HookConfig::from_json(config_json).expect("parse hook config");
    let runtime = HookRuntime::new(config);

    // Matches Bash -> Block
    let d1 = runtime
        .dispatch(HookEvent::PreToolUse, "Bash", json!({}))
        .await;
    assert!(matches!(d1, HookDisposition::Block(_)));

    // Matches apply_patch -> Block
    let d2 = runtime
        .dispatch(HookEvent::PreToolUse, "apply_patch", json!({}))
        .await;
    assert!(matches!(d2, HookDisposition::Block(_)));

    // Does NOT match read_file -> Allow
    let d3 = runtime
        .dispatch(HookEvent::PreToolUse, "read_file", json!({}))
        .await;
    assert_eq!(d3, HookDisposition::Allow);
}

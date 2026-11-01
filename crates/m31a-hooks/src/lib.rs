//! Lifecycle hook dispatcher executing configured commands before and after tools, turns, and phase transitions.
//!
//! 14 lifecycle events; handlers are shell commands (MCP handlers parsed but
//! deferred) configured from Claude-style `hooks.json`. Upstream semantics
//! adopted: PreToolUse rewrites or blocks, PostToolUse gates result
//! acceptance, Stop blocks continuation, exit 2 blocks with a reason.

pub mod config;
pub mod events;
pub mod lifecycle;
pub mod runtime;

pub use config::{HandlerKind, HookConfig, HookHandler, HookRule};
pub use events::{HookDisposition, HookEvent, SessionStartSource};
pub use lifecycle::{
    CanonicalPhasePoint, OnErrorPolicy, PhaseDispatchResult, PhaseHookAction, PhaseHookRegistry,
    PhaseHookRule,
};
pub use runtime::{HookRuntime, McpHandlerFn};

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_config_parses_camel_and_snake_keys() {
        let text = r#"{
            "PreToolUse": [
                { "matcher": "Bash|apply_patch",
                  "hooks": [ { "type": "command", "command": "echo hi", "timeout_seconds": 5 } ] }
            ],
            "post_tool_use": [
                { "hooks": [ { "type": "command", "command": "echo ok" } ] }
            ]
        }"#;
        let cfg = HookConfig::from_json(text).unwrap();
        assert_eq!(cfg.rules.len(), 2);
        // Find by event: JSON object key order is not preserved (HashMap),
        // so rules are sorted deterministically rather than declaration order.
        let pre = cfg
            .rules
            .iter()
            .find(|r| r.event == HookEvent::PreToolUse)
            .unwrap();
        let post = cfg
            .rules
            .iter()
            .find(|r| r.event == HookEvent::PostToolUse)
            .unwrap();
        assert_eq!(pre.matcher.as_deref(), Some("Bash|apply_patch"));
        assert_eq!(pre.handlers[0].timeout_seconds, 5);
        assert!(post.matcher.is_none());
    }

    #[test]
    fn test_config_rejects_unknown_event_and_handler_type() {
        assert!(HookConfig::from_json(r#"{"Nope": []}"#).is_err());
        assert!(
            HookConfig::from_json(r#"{"stop": [ { "hooks": [ { "type": "wat" } ] } ]}"#).is_err()
        );
    }

    #[test]
    fn test_matcher_evaluation() {
        let cfg = HookConfig::from_json(
            r#"{ "pre_tool_use": [
                { "matcher": "Bash", "hooks": [ { "type": "command", "command": "x" } ] },
                { "matcher": "", "hooks": [ { "type": "command", "command": "y" } ] }
            ] }"#,
        )
        .unwrap();

        let matched = cfg.matched_rules(HookEvent::PreToolUse, "Bash");
        assert_eq!(matched.len(), 2); // exact + empty matcher
        let matched = cfg.matched_rules(HookEvent::PreToolUse, "read_file");
        assert_eq!(matched.len(), 1); // only the empty matcher
        let matched = cfg.matched_rules(HookEvent::Stop, "anything");
        assert_eq!(matched.len(), 0);
    }

    #[tokio::test]
    async fn test_no_hooks_allows() {
        let rt = HookRuntime::new(HookConfig::empty());
        let d = rt.dispatch(HookEvent::PreToolUse, "Bash", json!({})).await;
        assert_eq!(d, HookDisposition::Allow);
    }

    #[tokio::test]
    async fn test_stdin_payload_reaches_handler() {
        // Handler reads stdin; blocks if the payload's tool is not "Bash".
        let cfg = HookConfig::from_json(
            r#"{ "pre_tool_use": [ { "hooks": [ { "type": "command",
                "command": "grep -q '\"tool\":\"Bash\"' && exit 0 || exit 2" } ] } ] }"#,
        )
        .unwrap();
        let rt = HookRuntime::new(cfg);

        let d = rt
            .dispatch(
                HookEvent::PreToolUse,
                "Bash",
                json!({ "tool": "Bash", "input": {} }),
            )
            .await;
        assert_eq!(d, HookDisposition::Allow);

        let d = rt
            .dispatch(
                HookEvent::PreToolUse,
                "read_file",
                json!({ "tool": "read_file", "input": {} }),
            )
            .await;
        match d {
            HookDisposition::Block(reason) => assert!(!reason.is_empty()),
            other => panic!("expected Block, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_decision_json_block_and_rewrite() {
        // JSON decision block.
        let cfg = HookConfig::from_json(
            r#"{ "stop": [ { "hooks": [ { "type": "command",
                "command": "echo '{\"decision\": \"block\", \"reason\": \"keep going\"}'" } ] } ] }"#,
        )
        .unwrap();
        let rt = HookRuntime::new(cfg);
        let d = rt.dispatch(HookEvent::Stop, "", json!({})).await;
        assert_eq!(d, HookDisposition::Block("keep going".to_string()));

        // updated_input rewrite.
        let cfg = HookConfig::from_json(
            r#"{ "pre_tool_use": [ { "hooks": [ { "type": "command",
                "command": "echo '{\"updated_input\": {\"command\": \"ls -la\"}}'" } ] } ] }"#,
        )
        .unwrap();
        let rt = HookRuntime::new(cfg);
        let d = rt
            .dispatch(HookEvent::PreToolUse, "Bash", json!({ "command": "rm" }))
            .await;
        assert_eq!(
            d,
            HookDisposition::Rewrite {
                input: r#"{"command":"ls -la"}"#.to_string()
            }
        );
    }

    #[tokio::test]
    async fn test_exit_two_blocks_with_stderr_reason() {
        let cfg = HookConfig::from_json(
            r#"{ "user_prompt_submit": [ { "hooks": [ { "type": "command",
                "command": "echo 'denied' >&2; exit 2" } ] } ] }"#,
        )
        .unwrap();
        let rt = HookRuntime::new(cfg);
        let d = rt
            .dispatch(HookEvent::UserPromptSubmit, "", json!({ "prompt": "hi" }))
            .await;
        assert_eq!(d, HookDisposition::Block("denied".to_string()));
    }

    #[tokio::test]
    async fn test_timeout_is_non_fatal() {
        let cfg = HookConfig::from_json(
            r#"{ "post_tool_use": [ { "hooks": [ { "type": "command",
                "command": "sleep 5", "timeout_seconds": 1 } ] } ] }"#,
        )
        .unwrap();
        let rt = HookRuntime::new(cfg);
        let d = rt.dispatch(HookEvent::PostToolUse, "Bash", json!({})).await;
        assert_eq!(d, HookDisposition::Allow); // timeout treated as non-blocking
    }

    #[tokio::test]
    async fn test_mcp_handler_skipped_with_warning() {
        let cfg = HookConfig::from_json(
            r#"{ "stop": [ { "hooks": [ { "type": "mcp", "server": "non_existent_server_xyz", "tool": "t" } ] } ] }"#,
        )
        .unwrap();
        let rt = HookRuntime::new(cfg);
        let d = rt.dispatch(HookEvent::Stop, "", json!({})).await;
        assert_eq!(d, HookDisposition::Allow);
    }

    #[tokio::test]
    async fn test_mcp_handler_with_custom_invoker_block() {
        let cfg = HookConfig::from_json(
            r#"{ "pre_tool_use": [ { "hooks": [ { "type": "mcp", "server": "sec_server", "tool": "audit_tool" } ] } ] }"#,
        )
        .unwrap();

        let invoker: McpHandlerFn = std::sync::Arc::new(|server, tool, _payload| {
            Box::pin(async move {
                assert_eq!(server, "sec_server");
                assert_eq!(tool, "audit_tool");
                Ok(json!({
                    "decision": "block",
                    "reason": "Security policy violation"
                }))
            })
        });

        let rt = HookRuntime::new(cfg).with_mcp_invoker(invoker);
        let d = rt
            .dispatch(HookEvent::PreToolUse, "Bash", json!({"cmd": "rm -rf /"}))
            .await;
        assert_eq!(
            d,
            HookDisposition::Block("Security policy violation".to_string())
        );
    }

    #[test]
    fn test_discover_missing_files_is_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = HookConfig::discover(tmp.path());
        assert!(cfg.rules.is_empty());
    }
}

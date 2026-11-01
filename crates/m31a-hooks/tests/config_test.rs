//! Dedicated integration tests for `m31a-hooks` (src/config.rs).
//!
//! Validates:
//! - JSON configuration parsing with both CamelCase and snake_case keys
//! - HandlerKind classification (`Command` and `Mcp`)
//! - Subject matcher regex evaluation (exact, regex patterns, empty matchers)
//! - Validation and rejection of invalid event keys or handler types
//! - Workspace hook file discovery

use m31a_hooks::config::{HandlerKind, HookConfig};
use m31a_hooks::events::HookEvent;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_hook_config_empty() {
    let empty = HookConfig::empty();
    assert!(empty.rules.is_empty());
    assert!(empty.matched_rules(HookEvent::PreToolUse, "Bash").is_empty());
}

#[test]
fn test_parse_camel_case_and_snake_case_keys() {
    let json_content = r#"{
        "PreToolUse": [
            {
                "matcher": "Bash|apply_patch",
                "hooks": [
                    { "type": "command", "command": "./guard.sh", "timeout_seconds": 15 }
                ]
            }
        ],
        "post_tool_use": [
            {
                "hooks": [
                    { "type": "command", "command": "./audit.sh" }
                ]
            }
        ],
        "session_start": [
            {
                "hooks": [
                    { "type": "mcp", "server": "telemetry", "tool": "log_event" }
                ]
            }
        ]
    }"#;

    let config = HookConfig::from_json(json_content).expect("Parse hooks JSON");
    assert_eq!(config.rules.len(), 3);

    let pre_rules = config.matched_rules(HookEvent::PreToolUse, "Bash");
    assert_eq!(pre_rules.len(), 1);
    assert_eq!(pre_rules[0].handlers[0].timeout_seconds, 15);
    assert_eq!(pre_rules[0].handlers[0].kind, HandlerKind::Command);

    let post_rules = config.matched_rules(HookEvent::PostToolUse, "any_tool");
    assert_eq!(post_rules.len(), 1);

    let session_rules = config.matched_rules(HookEvent::SessionStart, "startup");
    assert_eq!(session_rules.len(), 1);
    assert_eq!(
        session_rules[0].handlers[0].kind,
        HandlerKind::Mcp {
            server: "telemetry".to_string(),
            tool: "log_event".to_string()
        }
    );
}

#[test]
fn test_matcher_regex_matching_semantics() {
    let json_content = r#"{
        "pre_tool_use": [
            { "matcher": "^git_.*", "hooks": [ { "type": "command", "command": "git_guard.sh" } ] },
            { "matcher": "Bash", "hooks": [ { "type": "command", "command": "bash_guard.sh" } ] },
            { "matcher": "", "hooks": [ { "type": "command", "command": "global_guard.sh" } ] }
        ]
    }"#;

    let config = HookConfig::from_json(json_content).unwrap();

    // "git_commit" matches git_* and global (empty matcher)
    let git_matches = config.matched_rules(HookEvent::PreToolUse, "git_commit");
    assert_eq!(git_matches.len(), 2);

    // "Bash" matches Bash and global
    let bash_matches = config.matched_rules(HookEvent::PreToolUse, "Bash");
    assert_eq!(bash_matches.len(), 2);

    // "read_file" matches only global
    let read_matches = config.matched_rules(HookEvent::PreToolUse, "read_file");
    assert_eq!(read_matches.len(), 1);

    // Stop event has no rules configured
    let stop_matches = config.matched_rules(HookEvent::Stop, "");
    assert!(stop_matches.is_empty());
}

#[test]
fn test_parse_rejections() {
    // Unknown event name
    assert!(HookConfig::from_json(r#"{"InvalidEventName": []}"#).is_err());

    // Unknown handler type
    assert!(HookConfig::from_json(r#"{"stop": [ { "hooks": [ { "type": "alien" } ] } ]}"#).is_err());
}

#[test]
fn test_hook_discovery_in_workspace() {
    let temp = tempdir().unwrap();
    let root = temp.path();

    // No hooks file initially
    let cfg_empty = HookConfig::discover(root);
    assert!(cfg_empty.rules.is_empty());

    // Create .m31a/hooks.json
    let m31a_dir = root.join(".m31a");
    fs::create_dir_all(&m31a_dir).unwrap();
    fs::write(
        m31a_dir.join("hooks.json"),
        br#"{ "stop": [ { "hooks": [ { "type": "command", "command": "echo stopped" } ] } ] }"#,
    )
    .unwrap();

    let cfg_discovered = HookConfig::discover(root);
    assert_eq!(cfg_discovered.rules.len(), 1);
    assert_eq!(cfg_discovered.rules[0].event, HookEvent::Stop);
}

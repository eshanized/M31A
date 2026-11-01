//! Dedicated integration tests for `m31a-hooks` (src/events.rs).
//!
//! Validates:
//! - All 14 HookEvent variants, config keys, and subject field bindings
//! - SessionStartSource enum variants and serialization
//! - HookDisposition variants (Allow, Block, Rewrite)

use m31a_hooks::events::{HookDisposition, HookEvent, SessionStartSource};

#[test]
fn test_all_14_hook_events_config_keys() {
    let events = [
        (HookEvent::PreModelRequest, "pre_model_request"),
        (HookEvent::PostModelRequest, "post_model_request"),
        (HookEvent::PreToolUse, "pre_tool_use"),
        (HookEvent::PermissionRequest, "permission_request"),
        (HookEvent::PostToolUse, "post_tool_use"),
        (HookEvent::PreCompact, "pre_compact"),
        (HookEvent::PostCompact, "post_compact"),
        (HookEvent::SessionStart, "session_start"),
        (HookEvent::SessionEnd, "session_end"),
        (HookEvent::UserPromptSubmit, "user_prompt_submit"),
        (HookEvent::SubagentStart, "subagent_start"),
        (HookEvent::SubagentStop, "subagent_stop"),
        (HookEvent::Stop, "stop"),
        (HookEvent::Interrupt, "interrupt"),
    ];

    assert_eq!(events.len(), 14, "Must cover all 14 lifecycle events");
    for (event, expected_key) in events {
        assert_eq!(event.config_key(), expected_key);
    }
}

#[test]
fn test_matcher_subject_field_bindings() {
    // Tool-based events match against the "tool" name
    assert_eq!(HookEvent::PreToolUse.matcher_subject_field(), "tool");
    assert_eq!(HookEvent::PostToolUse.matcher_subject_field(), "tool");
    assert_eq!(HookEvent::PermissionRequest.matcher_subject_field(), "tool");

    // Other events match against generic "event"
    assert_eq!(HookEvent::PreModelRequest.matcher_subject_field(), "event");
    assert_eq!(HookEvent::Stop.matcher_subject_field(), "event");
    assert_eq!(HookEvent::SessionStart.matcher_subject_field(), "event");
}

#[test]
fn test_session_start_sources() {
    let sources = [
        SessionStartSource::Startup,
        SessionStartSource::Resume,
        SessionStartSource::Clear,
        SessionStartSource::Compact,
        SessionStartSource::Fork,
    ];

    for s in sources {
        let json = serde_json::to_string(&s).unwrap();
        let deserialized: SessionStartSource = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, s);
    }
}

#[test]
fn test_hook_disposition_variants() {
    let allow = HookDisposition::Allow;
    assert_eq!(allow, HookDisposition::Allow);

    let block = HookDisposition::Block("Execution restricted".to_string());
    assert_eq!(block, HookDisposition::Block("Execution restricted".to_string()));

    let rewrite = HookDisposition::Rewrite {
        input: "{\"safe\": true}".to_string(),
    };
    assert!(matches!(rewrite, HookDisposition::Rewrite { .. }));
}

//! Dedicated integration tests for `m31a-hooks` (src/lib.rs).
//!
//! Validates:
//! - Root re-exports across config, events, lifecycle, and runtime modules
//! - Cohesive lifecycle: config discovery -> runtime dispatch -> phase hook orchestration

use m31a_hooks::{
    CanonicalPhasePoint, HookConfig, HookDisposition, HookEvent, HookRuntime, OnErrorPolicy,
    PhaseDispatchResult, PhaseHookAction, PhaseHookRegistry, PhaseHookRule, SessionStartSource,
};
use serde_json::json;
use std::collections::HashMap;
use tempfile::tempdir;

#[test]
fn test_root_reexports() {
    assert_eq!(HookEvent::PreToolUse.config_key(), "pre_tool_use");
    assert_eq!(CanonicalPhasePoint::DiscussPre.as_str(), "discuss:pre");
    assert_eq!(SessionStartSource::Startup, SessionStartSource::Startup);
    assert_eq!(OnErrorPolicy::Halt, OnErrorPolicy::Halt);

    let empty_cfg = HookConfig::empty();
    assert!(empty_cfg.rules.is_empty());
}

#[tokio::test]
async fn test_full_hooks_lifecycle_integration() {
    let temp = tempdir().unwrap();
    let root = temp.path();

    // 1. Configure and run HookRuntime for tool guard
    let json_cfg = r#"{
        "pre_tool_use": [
            { "matcher": "safe_.*", "hooks": [ { "type": "command", "command": "echo 'ok'; exit 0" } ] }
        ]
    }"#;
    let cfg = HookConfig::from_json(json_cfg).unwrap();
    let runtime = HookRuntime::new(cfg);

    let allow_res = runtime
        .dispatch(HookEvent::PreToolUse, "safe_tool", json!({}))
        .await;
    assert_eq!(allow_res, HookDisposition::Allow);

    // 2. Dispatch through Canonical Phase Registry
    let mut phase_registry = PhaseHookRegistry::new();
    phase_registry.register(PhaseHookRule {
        id: "phase-ship-check".to_string(),
        point: CanonicalPhasePoint::ShipPre,
        action: PhaseHookAction::Command("git branch".to_string()),
        produces: vec!["RELEASE.md".to_string()],
        consumes: vec![],
        when: None,
        on_error: OnErrorPolicy::Warn,
    });

    let context = HashMap::new();
    let phase_res: PhaseDispatchResult =
        phase_registry.dispatch(CanonicalPhasePoint::ShipPre, root, &context);

    assert!(!phase_res.halted);
    assert_eq!(phase_res.point, CanonicalPhasePoint::ShipPre);
    assert_eq!(phase_res.executed_hooks, 1);
    assert_eq!(phase_res.artifacts_produced, vec!["RELEASE.md"]);
}

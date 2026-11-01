//! Dedicated integration tests for `m31a-hooks` (src/lifecycle.rs).
//!
//! Validates:
//! - Canonical 12-point phase loop taxonomy and string parsing
//! - OnErrorPolicy directives (Halt, Warn, Skip)
//! - Default GSD phase hook registrations
//! - Phase dispatch execution, consumed artifact verification, and when conditions

use m31a_hooks::lifecycle::{
    CanonicalPhasePoint, OnErrorPolicy, PhaseHookAction, PhaseHookRegistry, PhaseHookRule,
};
use std::collections::HashMap;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_all_12_canonical_phase_points() {
    let all = CanonicalPhasePoint::all();
    assert_eq!(
        all.len(),
        12,
        "Must contain exactly 12 canonical phase points"
    );

    let expected_names = [
        "discuss:pre",
        "discuss:post",
        "plan:pre",
        "plan:post",
        "execute:pre",
        "execute:wave:pre",
        "execute:wave:post",
        "execute:post",
        "verify:pre",
        "verify:post",
        "ship:pre",
        "ship:post",
    ];

    for (point, name) in all.iter().zip(expected_names.iter()) {
        assert_eq!(point.as_str(), *name);
        assert_eq!(CanonicalPhasePoint::from_str(name), Some(*point));
    }

    assert_eq!(CanonicalPhasePoint::from_str("invalid:point"), None);
}

#[test]
fn test_gsd_defaults_registry() {
    let reg = PhaseHookRegistry::with_gsd_defaults();

    let discuss_post = reg.resolve_hooks(CanonicalPhasePoint::DiscussPost);
    assert_eq!(discuss_post.len(), 1);
    assert_eq!(discuss_post[0].produces, vec!["CONTEXT.md".to_string()]);

    let plan_post = reg.resolve_hooks(CanonicalPhasePoint::PlanPost);
    assert_eq!(plan_post.len(), 1);
    assert_eq!(plan_post[0].consumes, vec!["CONTEXT.md".to_string()]);
    assert_eq!(plan_post[0].produces, vec!["PLAN.md".to_string()]);

    let wave_post = reg.resolve_hooks(CanonicalPhasePoint::ExecuteWavePost);
    assert_eq!(wave_post.len(), 1);
}

#[test]
fn test_phase_hook_dispatch_missing_consumed_artifact_halts() {
    let temp = tempdir().unwrap();
    let root = temp.path();

    let mut reg = PhaseHookRegistry::new();
    reg.register(PhaseHookRule {
        id: "plan-rule".to_string(),
        point: CanonicalPhasePoint::PlanPost,
        action: PhaseHookAction::Command("echo planning".to_string()),
        produces: vec!["PLAN.md".to_string()],
        consumes: vec!["CONTEXT.md".to_string()], // Missing!
        when: None,
        on_error: OnErrorPolicy::Halt,
    });

    let context = HashMap::new();
    let result = reg.dispatch(CanonicalPhasePoint::PlanPost, root, &context);

    assert!(
        result.halted,
        "Missing consumed artifact with Halt policy must halt"
    );
    assert!(result.halt_reason.unwrap().contains("CONTEXT.md"));
}

#[test]
fn test_phase_hook_dispatch_success_with_existing_artifact() {
    let temp = tempdir().unwrap();
    let root = temp.path();

    // Create required consumed artifact
    fs::write(root.join("CONTEXT.md"), b"# Context").unwrap();

    let mut reg = PhaseHookRegistry::new();
    reg.register(PhaseHookRule {
        id: "plan-rule".to_string(),
        point: CanonicalPhasePoint::PlanPost,
        action: PhaseHookAction::Command("echo planning".to_string()),
        produces: vec!["PLAN.md".to_string()],
        consumes: vec!["CONTEXT.md".to_string()],
        when: None,
        on_error: OnErrorPolicy::Halt,
    });

    let context = HashMap::new();
    let result = reg.dispatch(CanonicalPhasePoint::PlanPost, root, &context);

    assert!(!result.halted);
    assert_eq!(result.executed_hooks, 1);
    assert_eq!(result.artifacts_produced, vec!["PLAN.md"]);
}

#[test]
fn test_phase_hook_when_condition_skipped() {
    let temp = tempdir().unwrap();
    let root = temp.path();

    let mut reg = PhaseHookRegistry::new();
    reg.register(PhaseHookRule {
        id: "optional-check".to_string(),
        point: CanonicalPhasePoint::VerifyPre,
        action: PhaseHookAction::Command("echo audit".to_string()),
        produces: vec![],
        consumes: vec![],
        when: Some("flags.enable_audit".to_string()),
        on_error: OnErrorPolicy::Halt,
    });

    let mut context = HashMap::new();
    context.insert("flags.enable_audit".to_string(), "false".to_string());

    let result = reg.dispatch(CanonicalPhasePoint::VerifyPre, root, &context);
    assert_eq!(
        result.executed_hooks, 0,
        "When condition = false must skip execution"
    );
}

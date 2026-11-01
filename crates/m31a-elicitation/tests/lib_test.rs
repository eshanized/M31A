//! Dedicated integration tests for `m31a-elicitation` (src/lib.rs).
//!
//! Validates:
//! - Root re-exports: `ApprovalDecision`, `ApprovalKind`, `ApprovalState`, `QuestionItem`
//! - Cohesive workflow chaining elicitation questions with downstream approval states

use m31a_elicitation::{ApprovalDecision, ApprovalKind, ApprovalState, QuestionItem};

#[test]
fn test_root_reexports_availability() {
    let question = QuestionItem::new(
        "Run test suite?",
        vec!["Yes".to_string(), "No".to_string()],
        false,
    );
    assert_eq!(question.options.len(), 2);

    let state = ApprovalState::new(ApprovalKind::ExecCommand {
        command: "cargo test".to_string(),
        risk_level: "low".to_string(),
        suggested_pattern: "cargo test*".to_string(),
    });
    assert_eq!(state.selected_option, 0);

    let decision = state.decide();
    assert_eq!(decision, ApprovalDecision::AllowOnce);
    assert!(decision.is_allowed());
}

#[test]
fn test_elicitation_to_approval_lifecycle() {
    // 1. User answers an elicitation question about action risk
    let mut q = QuestionItem::new(
        "Choose authorization tier",
        vec![
            "Read Only".to_string(),
            "Execute In Sandbox".to_string(),
            "Unrestricted".to_string(),
        ],
        false,
    );
    q.move_down(); // Move to Execute In Sandbox
    q.toggle_selection();
    let choices = q.submit();
    assert_eq!(choices, vec!["Execute In Sandbox".to_string()]);

    // 2. An approval item is generated
    let mut approval = ApprovalState::new(ApprovalKind::SpawnAgent {
        role: "sandboxed-executor".to_string(),
        instructions: format!("Configured tier: {}", choices[0]),
    });

    // Advance to AlwaysAllow
    approval.select_next();
    let decision = approval.decide();
    assert_eq!(
        decision,
        ApprovalDecision::AlwaysAllowPattern("agent:sandboxed-executor".to_string())
    );
    assert!(decision.is_allowed());
    assert_eq!(decision.denial_feedback(), None);
}

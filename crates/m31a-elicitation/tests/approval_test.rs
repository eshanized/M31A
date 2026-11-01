//! Dedicated integration tests for `m31a-elicitation` (src/approval.rs).
//!
//! Validates:
//! - Initial state construction and buffer initialization across all `ApprovalKind` variants
//! - Cursor navigation limits (`select_next` / `select_prev`) and editing isolation
//! - Decision synthesis across all option indices (AllowOnce, AlwaysAllowPattern, Deny, EditCommand)
//! - `ApprovalDecision` predicate methods (`is_allowed`, `denial_feedback`)
//! - `From<bool>` conversions and Serde JSON serialization fidelity

use m31a_elicitation::approval::{ApprovalDecision, ApprovalKind, ApprovalState};

#[test]
fn test_approval_state_initialization() {
    let cmd_kind = ApprovalKind::ExecCommand {
        command: "cargo check --all".to_string(),
        risk_level: "low".to_string(),
        suggested_pattern: "cargo check *".to_string(),
    };
    let state_cmd = ApprovalState::new(cmd_kind);
    assert!(!state_cmd.id.is_empty());
    assert_eq!(state_cmd.selected_option, 0);
    assert_eq!(state_cmd.edit_buffer, "cargo check --all");
    assert!(!state_cmd.is_editing);

    let write_kind = ApprovalKind::WriteFile {
        file_path: "secret.env".to_string(),
        bytes_count: 2048,
    };
    let state_write = ApprovalState::new(write_kind);
    assert_eq!(state_write.edit_buffer, "");
    assert_eq!(state_write.selected_option, 0);
}

#[test]
fn test_approval_navigation_and_bounds() {
    let mut state = ApprovalState::new(ApprovalKind::ExecCommand {
        command: "rm -rf target".to_string(),
        risk_level: "high".to_string(),
        suggested_pattern: "rm target".to_string(),
    });

    assert_eq!(state.selected_option, 0);
    state.select_prev();
    assert_eq!(state.selected_option, 0, "Prev at 0 should remain at 0");

    state.select_next();
    assert_eq!(state.selected_option, 1);
    state.select_next();
    assert_eq!(state.selected_option, 2);
    state.select_next();
    assert_eq!(state.selected_option, 3);
    state.select_next();
    assert_eq!(state.selected_option, 3, "Next at 3 should clamp to 3");

    // When editing is active, cursor movement is locked
    state.is_editing = true;
    state.select_prev();
    assert_eq!(state.selected_option, 3, "Movement locked while editing");
    state.select_next();
    assert_eq!(state.selected_option, 3, "Movement locked while editing");
}

#[test]
fn test_decisions_for_all_approval_kinds() {
    // 1. ExecCommand
    let mut s1 = ApprovalState::new(ApprovalKind::ExecCommand {
        command: "ls -la".to_string(),
        risk_level: "safe".to_string(),
        suggested_pattern: "ls *".to_string(),
    });
    s1.selected_option = 0;
    assert_eq!(s1.decide(), ApprovalDecision::AllowOnce);

    s1.selected_option = 1;
    assert_eq!(
        s1.decide(),
        ApprovalDecision::AlwaysAllowPattern("ls *".to_string())
    );

    s1.selected_option = 2;
    assert_eq!(
        s1.decide(),
        ApprovalDecision::Deny {
            reason: "Operation denied by user.".to_string()
        }
    );

    s1.selected_option = 3;
    s1.edit_buffer = "ls -lh".to_string();
    assert_eq!(
        s1.decide(),
        ApprovalDecision::EditCommand {
            new_command: "ls -lh".to_string()
        }
    );

    s1.selected_option = 4;
    assert_eq!(
        s1.decide(),
        ApprovalDecision::Deny {
            reason: "Cancelled".to_string()
        }
    );

    // 2. ApplyPatch pattern format
    let mut s2 = ApprovalState::new(ApprovalKind::ApplyPatch {
        file_path: "src/main.rs".to_string(),
        diff_preview: "+++ diff".to_string(),
        lines_added: 5,
        lines_deleted: 2,
    });
    s2.selected_option = 1;
    assert_eq!(
        s2.decide(),
        ApprovalDecision::AlwaysAllowPattern("patch:src/main.rs".to_string())
    );

    // 3. WriteFile pattern format
    let mut s3 = ApprovalState::new(ApprovalKind::WriteFile {
        file_path: "Cargo.lock".to_string(),
        bytes_count: 1024,
    });
    s3.selected_option = 1;
    assert_eq!(
        s3.decide(),
        ApprovalDecision::AlwaysAllowPattern("write:Cargo.lock".to_string())
    );

    // 4. SpawnAgent pattern format
    let mut s4 = ApprovalState::new(ApprovalKind::SpawnAgent {
        role: "security-auditor".to_string(),
        instructions: "scan code".to_string(),
    });
    s4.selected_option = 1;
    assert_eq!(
        s4.decide(),
        ApprovalDecision::AlwaysAllowPattern("agent:security-auditor".to_string())
    );
}

#[test]
fn test_approval_decision_predicates_and_conversions() {
    let allow = ApprovalDecision::AllowOnce;
    assert!(allow.is_allowed());
    assert_eq!(allow.denial_feedback(), None);

    let allow_file = ApprovalDecision::AllowFile {
        path: "/tmp/test".to_string(),
    };
    assert!(allow_file.is_allowed());
    assert_eq!(allow_file.denial_feedback(), None);

    let always = ApprovalDecision::AlwaysAllowPattern("cargo *".to_string());
    assert!(always.is_allowed());
    assert_eq!(always.denial_feedback(), None);

    let edit = ApprovalDecision::EditCommand {
        new_command: "git status".to_string(),
    };
    assert!(edit.is_allowed());
    assert_eq!(edit.denial_feedback(), None);

    let deny = ApprovalDecision::Deny {
        reason: "Suspicious command".to_string(),
    };
    assert!(!deny.is_allowed());
    assert_eq!(deny.denial_feedback(), Some("Suspicious command"));

    // From<bool>
    let from_true: ApprovalDecision = true.into();
    assert_eq!(from_true, ApprovalDecision::AllowOnce);
    let from_false: ApprovalDecision = false.into();
    assert_eq!(
        from_false,
        ApprovalDecision::Deny {
            reason: "Operation denied by user.".to_string()
        }
    );
}

#[test]
fn test_serialization_roundtrip() {
    let decision = ApprovalDecision::AlwaysAllowPattern("cargo test *".to_string());
    let json = serde_json::to_string(&decision).expect("Serialize decision");
    let deserialized: ApprovalDecision = serde_json::from_str(&json).expect("Deserialize decision");
    assert_eq!(decision, deserialized);
}

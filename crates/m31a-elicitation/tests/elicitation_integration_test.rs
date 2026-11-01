//! Integration tests for user elicitation and interactive approval flows.
//!
//! Exercises multiple-choice prompts, risk assessment overlays, and user decision routing.

use m31a_elicitation::{ApprovalDecision, ApprovalKind, ApprovalState, QuestionItem};

#[test]
fn test_approval_patterns_for_all_kinds() {
    // 1. ExecCommand
    let mut s1 = ApprovalState::new(ApprovalKind::ExecCommand {
        command: "cargo check".into(),
        risk_level: "low".into(),
        suggested_pattern: "cargo *".into(),
    });
    s1.selected_option = 1;
    assert_eq!(
        s1.decide(),
        ApprovalDecision::AlwaysAllowPattern("cargo *".into())
    );

    // 2. ApplyPatch
    let mut s2 = ApprovalState::new(ApprovalKind::ApplyPatch {
        file_path: "src/main.rs".into(),
        diff_preview: "+let x = 1;".into(),
        lines_added: 1,
        lines_deleted: 0,
    });
    s2.selected_option = 1;
    assert_eq!(
        s2.decide(),
        ApprovalDecision::AlwaysAllowPattern("patch:src/main.rs".into())
    );

    // 3. WriteFile
    let mut s3 = ApprovalState::new(ApprovalKind::WriteFile {
        file_path: "config.toml".into(),
        bytes_count: 512,
    });
    s3.selected_option = 1;
    assert_eq!(
        s3.decide(),
        ApprovalDecision::AlwaysAllowPattern("write:config.toml".into())
    );

    // 4. SpawnAgent
    let mut s4 = ApprovalState::new(ApprovalKind::SpawnAgent {
        role: "architect".into(),
        instructions: "design system".into(),
    });
    s4.selected_option = 1;
    assert_eq!(
        s4.decide(),
        ApprovalDecision::AlwaysAllowPattern("agent:architect".into())
    );
}

#[test]
fn test_approval_navigation_and_decisions() {
    let mut state = ApprovalState::new(ApprovalKind::ExecCommand {
        command: "rm -rf target".into(),
        risk_level: "high".into(),
        suggested_pattern: "rm *".into(),
    });

    // Default option 0: AllowOnce
    assert_eq!(state.selected_option, 0);
    assert_eq!(state.decide(), ApprovalDecision::AllowOnce);

    // Navigate down to 2 (Deny)
    state.select_next();
    state.select_next();
    assert_eq!(state.selected_option, 2);
    match state.decide() {
        ApprovalDecision::Deny { reason } => assert!(reason.contains("denied by user")),
        other => panic!("Expected Deny, got {:?}", other),
    }

    // Navigate to 3 (EditCommand)
    state.select_next();
    assert_eq!(state.selected_option, 3);
    state.edit_buffer = "cargo clean".into();
    assert_eq!(
        state.decide(),
        ApprovalDecision::EditCommand {
            new_command: "cargo clean".into()
        }
    );

    // Boundary: cannot navigate beyond 3
    state.select_next();
    assert_eq!(state.selected_option, 3);

    // Navigate back to 0
    state.select_prev();
    state.select_prev();
    state.select_prev();
    state.select_prev(); // boundary check
    assert_eq!(state.selected_option, 0);
}

#[test]
fn test_question_multi_select_and_ordering() {
    let mut q = QuestionItem::new(
        "Select frameworks",
        vec![
            "React".into(),
            "Vue".into(),
            "Svelte".into(),
            "Angular".into(),
        ],
        true, // multi-select
    );

    // Select index 0 (React)
    q.cursor_idx = 0;
    q.toggle_selection();

    // Select index 2 (Svelte)
    q.cursor_idx = 2;
    q.toggle_selection();

    // Select index 1 (Vue)
    q.cursor_idx = 1;
    q.toggle_selection();

    // Submit should return sorted by index (React, Vue, Svelte)
    let answers = q.submit();
    assert_eq!(answers, vec!["React", "Vue", "Svelte"]);

    // Untoggle index 1 (Vue)
    q.cursor_idx = 1;
    q.toggle_selection();
    let answers_after = q.submit();
    assert_eq!(answers_after, vec!["React", "Svelte"]);
}

#[test]
fn test_question_custom_input_and_navigation() {
    let mut q = QuestionItem::new(
        "Choose option",
        vec!["Option A".into(), "Option B".into()],
        false,
    );

    assert_eq!(q.cursor_idx, 0);
    q.move_down();
    assert_eq!(q.cursor_idx, 1);
    assert!(!q.is_custom_focused);

    // Move down past last option -> custom focus
    q.move_down();
    assert!(q.is_custom_focused);

    // Provide custom input
    q.custom_input = "Custom Choice".into();
    let submitted = q.submit();
    assert!(submitted.contains(&"Custom Choice".to_string()));

    // Move back up -> returns to last option
    q.move_up();
    assert!(!q.is_custom_focused);
    assert_eq!(q.cursor_idx, 1);
}

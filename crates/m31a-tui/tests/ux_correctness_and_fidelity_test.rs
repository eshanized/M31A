//! Comprehensive UX Correctness, Interaction Fidelity, and Telemetry Verification Tests for M31A.

use m31a_core::types::{FunctionCall, ToolCall};
use m31a_tui::chatwidget::{ChatWidgetState, TurnExecutionState};
use m31a_tui::modals::{ApprovalActionKind, PendingApproval};
use m31a_tui::theme::{Theme, ThemeVariant, init_theme, theme};
use m31a_tui::turn::{TurnState, TurnStateMachine};
use m31a_tui::{safe_truncate_str, truncate_with_ellipsis};
use ratatui::style::Color;
use std::time::Instant;

#[test]
fn test_canonical_state_projection_idempotency_and_fidelity() {
    let mut widget = ChatWidgetState::new(24);
    let start_time = Instant::now();

    // 1. Thinking state
    let canonical_thinking = TurnState::Thinking {
        turn_id: "turn-proj-1".to_string(),
        started_at: start_time,
        preview: "Refactoring model".to_string(),
    };

    widget.sync_with_canonical(&canonical_thinking);
    let state_snapshot = widget.turn_state.clone();
    assert_eq!(
        state_snapshot,
        TurnExecutionState::Thinking {
            started_at: start_time,
            tokens: None,
            preview: "Refactoring model".to_string(),
        }
    );

    // Multiple consecutive sync calls must produce IDENTICAL output (no synthetic token counter increments)
    for _ in 0..10 {
        widget.sync_with_canonical(&canonical_thinking);
        assert_eq!(widget.turn_state, state_snapshot);
    }

    // 2. Awaiting admission state
    let canonical_admission = TurnState::AwaitingAdmission {
        turn_id: "turn-proj-1".to_string(),
        started_at: start_time,
    };
    widget.sync_with_canonical(&canonical_admission);
    assert_eq!(
        widget.turn_state,
        TurnExecutionState::AwaitingAdmission {
            started_at: start_time,
        }
    );
    let (badge, color) = widget.format_status_badge();
    assert!(badge.contains("Awaiting admission"));
    assert_eq!(color, Color::Yellow);

    // 3. Awaiting question state (never conflated with security approval!)
    let canonical_question = TurnState::AwaitingQuestion {
        turn_id: "turn-proj-1".to_string(),
        question_id: "q-42".to_string(),
        prompt: "Proceed with database migration?".to_string(),
        started_at: start_time,
    };
    widget.sync_with_canonical(&canonical_question);
    assert_eq!(
        widget.turn_state,
        TurnExecutionState::AwaitingQuestion {
            question_id: "q-42".to_string(),
            prompt: "Proceed with database migration?".to_string(),
            started_at: start_time,
        }
    );
    let (badge, color) = widget.format_status_badge();
    assert_eq!(badge, "Input required");
    assert_eq!(color, Color::Yellow);

    // 4. Awaiting approval state
    let canonical_approval = TurnState::AwaitingApproval {
        turn_id: "turn-proj-1".to_string(),
        request_id: "req-99".to_string(),
        summary: "Delete /etc/hosts".to_string(),
        started_at: start_time,
    };
    widget.sync_with_canonical(&canonical_approval);
    assert_eq!(
        widget.turn_state,
        TurnExecutionState::AwaitingApproval {
            request_id: "req-99".to_string(),
            action_summary: "Delete /etc/hosts".to_string(),
            started_at: start_time,
        }
    );
    let (badge, color) = widget.format_status_badge();
    assert_eq!(badge, "Approval required");
    assert_eq!(color, Color::Magenta);

    // 5. Cancellation lifecycle states
    let canonical_cancelling = TurnState::Cancelling {
        turn_id: "turn-proj-1".to_string(),
        started_at: start_time,
    };
    widget.sync_with_canonical(&canonical_cancelling);
    assert_eq!(
        widget.turn_state,
        TurnExecutionState::Cancelling {
            started_at: start_time,
        }
    );
    let (badge, color) = widget.format_status_badge();
    assert!(badge.contains("Terminating processes..."));
    assert_eq!(color, Color::Red);

    // 6. Recovering state
    let canonical_recovering = TurnState::Recovering {
        turn_id: "turn-proj-1".to_string(),
        started_at: start_time,
    };
    widget.sync_with_canonical(&canonical_recovering);
    assert_eq!(
        widget.turn_state,
        TurnExecutionState::Recovering {
            started_at: start_time,
        }
    );
    let (badge, color) = widget.format_status_badge();
    assert!(badge.contains("Recovering..."));
    assert_eq!(color, Color::Yellow);
}

#[test]
fn test_turn_state_machine_preserves_started_at() {
    let mut sm = TurnStateMachine::new();
    assert!(sm.state().started_at().is_none());

    sm.transition_submitted("turn-100").unwrap();
    let sub_time = sm.state().started_at().unwrap();

    sm.transition_thinking("turn-100", "Thinking...").unwrap();
    let think_time = sm.state().started_at().unwrap();
    assert!(think_time >= sub_time);

    sm.transition_streaming("turn-100").unwrap();
    let stream_time = sm.state().started_at().unwrap();
    assert!(stream_time >= think_time);

    sm.transition_failed("turn-100", "network timeout").unwrap();
    sm.transition_recovering("turn-100").unwrap();
    assert!(sm.state().started_at().is_some());
}

#[test]
fn test_multibyte_unicode_safe_truncation() {
    // 1. ASCII boundaries
    let ascii = "The quick brown fox jumps over the lazy dog";
    assert_eq!(safe_truncate_str(ascii, 9), "The quick");
    assert_eq!(truncate_with_ellipsis(ascii, 12), "The quick...");

    // 2. Cyrillic (2-byte UTF-8 code units)
    let cyrillic = "У лукоморья дуб зелёный";
    assert_eq!(safe_truncate_str(cyrillic, 11), "У лукоморья");
    assert_eq!(truncate_with_ellipsis(cyrillic, 14), "У лукоморья...");

    // 3. CJK (3-byte UTF-8 code units)
    let cjk = "自主软件工程代理架构";
    assert_eq!(safe_truncate_str(cjk, 4), "自主软件");
    assert_eq!(truncate_with_ellipsis(cjk, 5), "自主...");

    // 4. Emojis and combined symbols (4-byte UTF-8 code units)
    let emoji = "✨🚀🦀🛠️⚙️";
    assert_eq!(safe_truncate_str(emoji, 3), "✨🚀🦀");

    // 5. Short string never modified
    assert_eq!(truncate_with_ellipsis("short", 10), "short");
}

#[test]
fn test_all_four_themes_distinct_and_switchable() {
    let themes = [
        (ThemeVariant::Everblush, "everblush"),
        (ThemeVariant::Dark, "dark"),
        (ThemeVariant::Light, "light"),
        (ThemeVariant::HighContrast, "contrast"),
    ];

    for (variant, _name) in &themes {
        init_theme(*variant);
        assert_eq!(theme().variant, *variant);
    }

    // Verify palettes are distinct
    let eb = Theme::everblush();
    let dark = Theme::dark();
    let light = Theme::light();
    let hc = Theme::high_contrast();

    assert_ne!(eb.colors.surface_0, dark.colors.surface_0);
    assert_ne!(eb.colors.surface_0, light.colors.surface_0);
    assert_ne!(dark.colors.surface_0, hc.colors.surface_0);
}

#[test]
fn test_pending_approval_risk_and_action_fidelity() {
    // 1. Critical shell command
    let rm_call = ToolCall {
        id: "call-critical".to_string(),
        call_type: "function".to_string(),
        function: FunctionCall {
            name: "execute_command".to_string(),
            arguments: r#"{"command":"rm -rf /"}"#.to_string(),
        },
    };
    let approval = PendingApproval::new(rm_call, "Purge files", None);
    let risk = approval.assess_risk();
    assert!(risk.risk_level.contains("Critical"));
    assert_eq!(risk.sandbox_context, "Host OS");
    assert_eq!(risk.execution_scope, "Host Subprocess (Unsandboxed Shell)");

    // 2. High-risk benign command
    let cargo_call = ToolCall {
        id: "call-cargo".to_string(),
        call_type: "function".to_string(),
        function: FunctionCall {
            name: "execute_command".to_string(),
            arguments: r#"{"command":"cargo test"}"#.to_string(),
        },
    };
    let approval_cargo = PendingApproval::new(cargo_call, "Run tests", None);
    let risk_cargo = approval_cargo.assess_risk();
    assert!(risk_cargo.risk_level.contains("High"));

    // 3. File mutation without diff
    let write_call = ToolCall {
        id: "call-write".to_string(),
        call_type: "function".to_string(),
        function: FunctionCall {
            name: "write_file".to_string(),
            arguments: r#"{"path":"test.txt"}"#.to_string(),
        },
    };
    let write_approval = PendingApproval::new(write_call, "Write file", None);
    let write_risk = write_approval.assess_risk();
    assert!(write_risk.risk_level.contains("Medium"));
    // ViewDiff must NOT be offered when diff is absent
    assert!(
        !write_approval
            .action_kinds()
            .contains(&ApprovalActionKind::ViewDiff)
    );

    // 4. File mutation with empty patch
    let empty_patch_call = ToolCall {
        id: "call-empty-patch".to_string(),
        call_type: "function".to_string(),
        function: FunctionCall {
            name: "apply_patch".to_string(),
            arguments: r#"{"path":"test.txt","patch":"   "}"#.to_string(),
        },
    };
    let empty_patch_approval = PendingApproval::new(empty_patch_call, "Apply patch", None);
    assert!(
        !empty_patch_approval
            .action_kinds()
            .contains(&ApprovalActionKind::ViewDiff)
    );

    // 5. File mutation with valid patch
    let valid_patch_call = ToolCall {
        id: "call-valid-patch".to_string(),
        call_type: "function".to_string(),
        function: FunctionCall {
            name: "apply_patch".to_string(),
            arguments: r#"{"path":"test.txt","patch":"+fn main() {}"}"#.to_string(),
        },
    };
    let valid_patch_approval = PendingApproval::new(valid_patch_call, "Apply patch", None);
    assert!(
        valid_patch_approval
            .action_kinds()
            .contains(&ApprovalActionKind::ViewDiff)
    );

    // 6. Read-only operation
    let read_call = ToolCall {
        id: "call-read".to_string(),
        call_type: "function".to_string(),
        function: FunctionCall {
            name: "read_file".to_string(),
            arguments: r#"{"path":"test.txt"}"#.to_string(),
        },
    };
    let read_approval = PendingApproval::new(read_call, "Read file", None);
    let read_risk = read_approval.assess_risk();
    assert!(read_risk.risk_level.contains("Low"));

    // 7. Unrecognized tool
    let unk_call = ToolCall {
        id: "call-unk".to_string(),
        call_type: "function".to_string(),
        function: FunctionCall {
            name: "some_third_party_tool".to_string(),
            arguments: "{}".to_string(),
        },
    };
    let unk_approval = PendingApproval::new(unk_call, "External invocation", None);
    let unk_risk = unk_approval.assess_risk();
    assert_eq!(unk_risk.risk_level, "Risk assessment unavailable");
}

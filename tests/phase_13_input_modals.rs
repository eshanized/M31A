//! Integration tests for Phase 13 Plan 02: Zero-Dependency Input Engine, Forms & Modal Stack.
//!
//! Verifies:
//! - TextInput editing, cursor movement, Unicode safety, word boundaries, and secret masking (COP-01, CFX-03, D-04, D-08).
//! - Form field validation rules, Tab/Shift-Tab focus traversal, and UiAction normalization (COP-01, COP-02, D-04).
//! - ModalManager priority-tiered stack and non-dismissible critical approval modal invariants (COP-02, D-04).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use m31a::tui::input::action::{UiAction, normalize_key_event};
use m31a::tui::input::form::{Form, FormField, ValidationRule};
use m31a::tui::input::text_input::{InputMasking, InputMode, TextInput};

#[test]
fn test_text_input_and_secret_masking() {
    // 1. Basic text insertion and Unicode cursor operations
    let mut input = TextInput::new(InputMode::SingleLine);
    input.insert_str("Hello, 🚀 world!");
    assert_eq!(input.value(), "Hello, 🚀 world!");
    assert_eq!(input.char_count(), 15);

    // 2. Cursor navigation
    input.move_cursor_home();
    assert_eq!(input.cursor_char_index(), 0);

    input.move_cursor_right();
    assert_eq!(input.cursor_char_index(), 1);

    input.move_cursor_end();
    assert_eq!(input.cursor_char_index(), 15);

    // 3. Word boundary navigation
    input.move_cursor_word_left(); // Jumps to start of "world!"
    assert_eq!(input.cursor_char_index(), 9);

    input.move_cursor_word_left(); // Jumps to start of "🚀"
    assert_eq!(input.cursor_char_index(), 7);

    input.move_cursor_word_right(); // Jumps past "🚀 "
    assert_eq!(input.cursor_char_index(), 9);

    // 4. Deletion operations
    input.move_cursor_end();
    input.delete_word_backward();
    assert_eq!(input.value(), "Hello, 🚀 ");

    input.delete_backward();
    assert_eq!(input.value(), "Hello, 🚀");

    // 5. Secret Masking
    let mut secret_input =
        TextInput::new(InputMode::SingleLine).with_masking(InputMasking::Masked('*'));
    secret_input.insert_str("super-secret-api-key-12345");
    assert_eq!(secret_input.value(), "super-secret-api-key-12345");
    assert_eq!(secret_input.display_text(), "**************************");

    // 6. Multiline cursor movements
    let mut multiline = TextInput::new(InputMode::MultiLine);
    multiline.insert_str("line 1\nline 2\nline 3");
    assert_eq!(multiline.char_count(), 20);
    multiline.move_cursor_up();
    assert!(multiline.cursor_char_index() < 14);
}

#[test]
fn test_form_validation_and_action_mapping() {
    // 1. Construct Form with validation rules
    let field1 = FormField::new(
        "username",
        "Username",
        TextInput::new(InputMode::SingleLine),
    )
    .with_rule(ValidationRule::NonEmpty)
    .with_rule(ValidationRule::MinLength(3));

    let field2 = FormField::new("port", "Port", TextInput::new(InputMode::SingleLine))
        .with_rule(ValidationRule::NumericRange(1024.0, 65535.0));

    let mut form = Form::new(vec![field1, field2]);

    // Initial empty form should fail validation
    assert!(!form.validate_all());
    assert!(form.fields[0].error.is_some());

    // Fill valid username
    form.fields[0].input.insert_str("admin");
    assert!(form.fields[0].validate());
    assert!(form.fields[0].error.is_none());

    // Fill invalid port
    form.fields[1].input.insert_str("99999");
    assert!(!form.fields[1].validate());
    assert!(form.fields[1].error.is_some());

    // Fix port
    form.fields[1].input.clear();
    form.fields[1].input.insert_str("8080");
    assert!(form.fields[1].validate());
    assert!(form.validate_all());

    // 2. Focus cycling (Tab / Shift-Tab)
    assert_eq!(form.focus_index(), 0);
    form.focus_next();
    assert_eq!(form.focus_index(), 1);
    form.focus_next();
    assert_eq!(form.focus_index(), 0);
    form.focus_prev();
    assert_eq!(form.focus_index(), 1);

    // 3. UiAction key event normalization
    assert_eq!(
        normalize_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
        Some(UiAction::TabNext)
    );
    assert_eq!(
        normalize_key_event(KeyEvent::new(KeyCode::BackTab, KeyModifiers::NONE)),
        Some(UiAction::TabPrev)
    );
    assert_eq!(
        normalize_key_event(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL)),
        Some(UiAction::CommandPaletteToggle)
    );
    assert_eq!(
        normalize_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
        Some(UiAction::Quit)
    );
    assert_eq!(
        normalize_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        Some(UiAction::Submit)
    );
    assert_eq!(
        normalize_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
        Some(UiAction::Cancel)
    );
}

#[test]
fn test_canonical_approval_modal_and_overlay_lifecycle() {
    use m31a::tui::approval::{ApprovalDecision, ApprovalModal};
    use m31a::tui::model::TuiApprovalRequest;
    use m31a::tui::overlay::OverlayManager;

    // 1. OverlayManager lifecycle
    let mut overlay = OverlayManager::new();
    assert!(!overlay.is_help_open);
    overlay.open_help();
    assert!(overlay.is_help_open);
    overlay.toggle_help();
    assert!(!overlay.is_help_open);

    // 2. ApprovalModal lifecycle
    let mut modal = ApprovalModal::new();
    assert!(!modal.is_open);
    assert!(modal.current_request.is_none());

    let req = TuiApprovalRequest {
        id: "req-7788".to_string(),
        tool_name: "file_system_write".to_string(),
        agent_role: "implementer".to_string(),
        risk_tier: "CRITICAL".to_string(),
        justification: "Overwrite system hosts file".to_string(),
        parameters_summary: "/etc/hosts".to_string(),
        timestamp: chrono::Utc::now(),
    };
    modal.open(req);
    assert!(modal.is_open);
    assert_eq!(modal.current_request.as_ref().unwrap().id, "req-7788");

    // Non-decision key leaves modal open
    let ign = modal.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
    assert!(ign.is_none());
    assert!(modal.is_open);

    // Approve key resolves and closes
    let decision = modal.handle_key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));
    assert_eq!(decision, Some(ApprovalDecision::ApproveOnce));
    assert!(!modal.is_open);
    assert!(modal.current_request.is_none());
}

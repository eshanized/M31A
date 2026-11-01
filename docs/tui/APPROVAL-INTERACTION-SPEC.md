# M31A — TUI Approval Interaction Specification

**Status:** AUTHORITATIVE SPECIFICATION  
**Target:** `crates/m31a-tui/src/modals.rs`, `crates/m31a-tui/src/mouse.rs`, `crates/m31a-tui/src/renderer.rs`  
**Classification:** TUI Systems Interaction & UX Specification  

---

## 1. Executive Summary

Autonomous execution requires an unambiguous, secure, and resilient user approval system. 
Prior to this specification:
1. Multiple disparate approval widgets coexisted with slightly varying options.
2. Modal dimensions for rendering were hardcoded independently of mouse hit-testing routines.
3. Backdrop clicks outside the modal could accidentally pop or dismiss a critical pending approval, leaving the engine stranded.
4. Single-keystroke action shortcuts did not dynamically correspond to the available options (e.g., prefix rule approval when no command existed).

This specification establishes **exactly one authoritative approval interface** with unified geometry, dynamic single-keystroke action bindings, and fail-safe mouse interaction.

---

## 2. Authoritative Data Model & Action Kinds

Defined in `crates/m31a-tui/src/modals.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalActionKind {
    AllowOnce,
    AllowRule,
    AllowFile,
    ViewDiff,
    DeclineWithFeedback,
    Cancel,
}
```

The options displayed and allowed for any pending approval are generated dynamically by `PendingApproval::action_kinds(&self)`:

| Action Kind | Hotkey | Availability Condition | Action Taken |
|---|---|---|---|
| `AllowOnce` | `y` | Always available | Executes tool call once; permission not persisted. |
| `AllowRule` | `a` | Only when `approval.suggested_prefix()` exists | Adds persistent rule allowlist for the command prefix. |
| `AllowFile` | `s` | Only when `approval.target_file_path()` exists | Adds persistent rule allowlist for the target file path. |
| `ViewDiff` | `v` | Only when diff content is available | Opens `QuickDiffPreview` modal over current approval. |
| `DeclineWithFeedback` | `n` | Always available | Opens `DeclineFeedbackInput` modal to guide model recovery. |
| `Cancel` | `Esc` | Always available | Rejects tool call without additional feedback. |

---

## 3. Unified Modal Geometry Contract

To prevent hit-testing discrepancies between where buttons are drawn on screen and where mouse clicks register, `ModalState` provides an authoritative geometry method:

```rust
impl ModalState {
    pub fn dimensions(&self) -> (u16, u16) {
        match self {
            ModalState::ApprovalDialog { .. } => (72, 56),
            ModalState::DeclineFeedbackInput { .. } => (70, 40),
            ModalState::ModelPicker { .. } => (72, 70),
            // ...
        }
    }

    pub fn calculate_rect(&self, area: Rect) -> Rect {
        let (pct_x, pct_y) = self.dimensions();
        centered_rect(pct_x, pct_y, area)
    }
}
```

Both `render_approval_dialog` (in `renderer.rs`) and `handle_left_click_down` (in `mouse.rs`) call `modal.calculate_rect(screen_area)`. Neither function contains independent percentage values.

---

## 4. Backdrop Click Safety & Mouse Dispatch

Backdrop clicks (clicking the shaded terminal region outside the modal box) have explicit safety guarantees:

```rust
// crates/m31a-tui/src/mouse.rs
if !modal_rect.contains(crossterm::event::Position::new(col, row)) {
    // DO NOT pop or orphan critical security modals on backdrop click!
    if matches!(modal, ModalState::ApprovalDialog { .. } | ModalState::DeclineFeedbackInput { .. }) {
        app.push_modal(modal);
        return true;
    }
    // Non-security modals may be dismissed
    return true;
}
```

### Action Button Hit Testing
Action buttons are laid out horizontally at `y = modal_rect.bottom() - 3`.
The hit test dynamically iterates over `approval.action_kinds()`, calculating each button's width `[label]` + spacing. Clicking a button immediately dispatches the corresponding approval decision without requiring keyboard interaction.

---

## 5. Single-Keystroke & Keyboard Dispatch

Handled in `crates/m31a-tui/src/terminal.rs`:
- Pressing `y` immediately triggers `AllowOnce`.
- Pressing `a` (if `AllowRule` is present) approves and stores the rule.
- Pressing `s` (if `AllowFile` is present) approves and stores the file exception.
- Pressing `v` (if `ViewDiff` is present) transitions to the diff preview.
- Pressing `n` opens the decline feedback composer.
- Pressing `Esc` cancels the operation safely.
- Arrow keys (`Left`/`Right`/`Tab`) navigate focus across buttons; `Enter` activates the focused option.

---

## 6. Regression Testing

Verified by `crates/m31a-tui/tests/tui_regression_test.rs::test_approval_dialog_actions_and_modal_geometry`:
- Confirms action kind ordering and hotkey mappings.
- Confirms `calculate_rect` geometry consistency.
- Confirms dynamic generation of `AllowRule` based on command presence.

# `m31a-elicitation`

> Interactive preference elicitation, structured multi-choice question prompts, and human-in-the-loop approval widgets for M31A.

---

## Why Does This Crate Exist?

When an autonomous agent encounters ambiguous user requirements or multiple valid architectural paths (e.g., choosing between PostgreSQL vs SQLite, or selecting REST vs gRPC), guessing often leads to wasted tokens and unneeded refactors.

Instead of hallucinating or stopping abruptly, the agent should ask targeted, interactive questions.

`m31a-elicitation` provides the interactive user-inquiry machinery:
- Structures questions as single-choice or multi-choice options with keyboard navigation.
- Allows the user to select from suggested options or type a custom input.
- Provides modal approval widgets (`ApprovalDialogState`) for human confirmation before mutating files or executing external commands.

---

## Core Concepts & How It Works

### 1. Structured Inquiries (`QuestionItem`)
Agents invoke the `request_user_input` tool, generating a structured payload with:
- The question text.
- Recommended options list.
- Single-select vs multi-select flags.
- Option to provide freeform custom input.

`QuestionItem` tracks cursor positions, active selections, and custom text buffers, making it trivial to render inside Ratatui TUI screens or serialize over JSON-RPC to IDE extensions.

### 2. Human-in-the-Loop Approvals (`ApprovalDialogState`)
When the approval policy is set to `on-request`, mutating tool operations trigger an approval dialog. The user can:
- `allow`: Execute once.
- `allow_always`: Whitelist this command/tool pattern for the rest of the session.
- `deny`: Cancel execution and pass a denial reason back to the agent.
- `edit`: Modify command arguments before execution.

---

## Key Modules & Types

| Module | Key Structs / Enums | Purpose |
| :--- | :--- | :--- |
| [`src/questions.rs`](src/questions.rs) | `QuestionItem`, `QuestionState` | Manages interactive multi-choice and freeform text inquiries. |
| [`src/approval.rs`](src/approval.rs) | `ApprovalDialogState`, `ApprovalDecision` | Manages tool approval modals, keybindings, and decisions. |

---

## Practical Usage Example

```rust
use m31a_elicitation::questions::QuestionItem;

fn main() {
    // 1. Create a multi-choice question for architecture clarification
    let mut question = QuestionItem::new(
        "Which serialization format do you prefer for IPC?",
        vec!["Protocol Buffers".into(), "FlatBuffers".into(), "JSON / BSON".into()],
        /* is_multi_select = */ false,
    );

    // 2. Simulate user navigating downward and selecting "FlatBuffers"
    question.move_down();
    question.toggle_selection();

    let answers = question.get_selected_options();
    println!("User selected: {:?}", answers);
}
```

---

## Testing & Verification

```bash
cargo test -p m31a-elicitation
```

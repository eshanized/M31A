# M31A TUI — State Machines Specification

This document defines the formal state machines governing the M31A terminal user interface, including legal transitions, invariants, and failure recovery.

---

## 1. Canonical Turn State Machine (`turn::TurnStateMachine`)

The turn lifecycle represents the single source of truth for all turn execution. No visual component or background thread may invent turn status independently.

```mermaid
stateDiagram-v2
    [*] --> Idle

    Idle --> Submitted: submit_prompt()
    Submitted --> AwaitingAdmission: engine queued
    AwaitingAdmission --> Thinking: engine active
    Thinking --> Streaming: first token chunk
    Thinking --> ExecutingTool: tool invocation
    Streaming --> ExecutingTool: tool call emitted
    ExecutingTool --> AwaitingApproval: security policy requires confirmation
    AwaitingApproval --> ExecutingTool: user approves (AllowOnce / AlwaysAllow)
    AwaitingApproval --> Failed: user denies
    ExecutingTool --> Streaming: tool output fed back to LLM
    Streaming --> AwaitingUserQuestion: clarification requested
    AwaitingUserQuestion --> Streaming: user submits answer
    Streaming --> Compacting: context window limit reached
    Compacting --> Streaming: compaction completed
    Streaming --> Completed: finish_reason stop / end of turn
    ExecutingTool --> Completed: all tool calls settled

    Thinking --> CancellationRequested: Ctrl+C
    Streaming --> CancellationRequested: Ctrl+C
    ExecutingTool --> CancellationRequested: Ctrl+C
    AwaitingApproval --> CancellationRequested: Ctrl+C

    CancellationRequested --> Cancelling: engine acknowledges
    Cancelling --> Idle: subprocesses reaped & epoch bumped

    Thinking --> Failed: network/provider error
    Streaming --> Failed: connection dropped
    ExecutingTool --> Failed: tool fatal panic
    Failed --> Recovering: error recovery prompt generated
    Recovering --> Idle: reset to safe state
    Completed --> Idle: next prompt ready
```

### Transition Legality Table

| From State | Allowed Target States | Illegal Target States (Rejected with Error) |
|---|---|---|
| `Idle` | `Submitted` | `Streaming`, `ExecutingTool`, `Completed`, `Cancelling` |
| `Submitted` | `AwaitingAdmission`, `Thinking`, `CancellationRequested`, `Failed` | `Completed`, `Idle` (without cancel) |
| `AwaitingAdmission` | `Thinking`, `CancellationRequested`, `Failed` | `Streaming`, `Completed` |
| `Thinking` | `Streaming`, `ExecutingTool`, `CancellationRequested`, `Failed` | `Idle`, `Completed` |
| `Streaming` | `ExecutingTool`, `AwaitingUserQuestion`, `Compacting`, `Completed`, `CancellationRequested`, `Failed` | `Idle`, `Submitted` |
| `ExecutingTool` | `Streaming`, `AwaitingApproval`, `Completed`, `CancellationRequested`, `Failed` | `Idle`, `Thinking` |
| `AwaitingApproval` | `ExecutingTool`, `Failed`, `CancellationRequested` | `Completed`, `Idle` |
| `AwaitingUserQuestion`| `Streaming`, `CancellationRequested`, `Failed` | `Completed`, `Idle` |
| `Compacting` | `Streaming`, `CancellationRequested`, `Failed` | `Idle` |
| `CancellationRequested`| `Cancelling`, `Failed` | `Streaming`, `Completed`, `Idle` |
| `Cancelling` | `Idle`, `Failed` | `Streaming`, `Thinking`, `Completed` |
| `Completed` | `Idle` | `Streaming`, `ExecutingTool` |
| `Failed` | `Recovering`, `Idle` | `Streaming`, `Completed` |
| `Recovering` | `Idle` | `Streaming`, `Completed` |

---

## 2. Composer & Input State Machine (`composer::ChatComposer`)

```mermaid
stateDiagram-v2
    [*] --> NormalInput

    NormalInput --> NormalInput: on_char() / on_backspace() / on_delete()
    NormalInput --> SlashCommandPopup: buffer starts with '/'
    SlashCommandPopup --> SlashCommandPopup: arrow up/down navigates matches
    SlashCommandPopup --> NormalInput: Tab / Enter selects command, or Esc cancels

    NormalInput --> FileMentionPopup: buffer contains '@'
    FileMentionPopup --> NormalInput: Tab / Enter inserts path, or Esc cancels

    NormalInput --> SkillMentionPopup: buffer contains '$'
    SkillMentionPopup --> NormalInput: Tab / Enter inserts skill, or Esc cancels

    NormalInput --> MultilineEditing: Shift+Enter / Alt+Enter inserts newline
    MultilineEditing --> MultilineEditing: cursor moves across lines, viewport scrolls
    MultilineEditing --> Submitted: Enter on last line (or Ctrl+Enter submit)

    NormalInput --> HistoryBrowsing: Up Arrow on empty buffer
    HistoryBrowsing --> HistoryBrowsing: Up / Down arrows traverse snapshots
    HistoryBrowsing --> NormalInput: Typing or Down Arrow past newest entry

    NormalInput --> UndoSnapshotRestored: Ctrl+Z
    UndoSnapshotRestored --> RedoSnapshotRestored: Ctrl+Y
```

### Invariants:
1. `cursor` is always positioned at a valid UTF-8 boundary and grapheme cluster offset.
2. `viewport_offset` never exceeds the maximum rendered lines in the composer area.
3. Every mutating keystroke pushes a snapshot to `undo_stack` and clears `redo_stack`.

---

## 3. Modal Stack Lifecycle Machine (`modals::ModalStack`)

```mermaid
stateDiagram-v2
    [*] --> NoModalActive: App initialized

    NoModalActive --> ModalActive: push_modal(M1)
    ModalActive --> NestedModalActive: push_modal(M2)
    NestedModalActive --> DeeplyNestedModal: push_modal(M3)

    DeeplyNestedModal --> NestedModalActive: pop_modal() [Esc / Close M3]
    NestedModalActive --> ModalActive: pop_modal() [Esc / Close M2]
    ModalActive --> NoModalActive: pop_modal() [Esc / Close M1]

    NestedModalActive --> NoModalActive: clear_modals() [Hard Reset]
```

### Invariants:
1. The currently rendered modal is always `modal_stack.last()`.
2. Popping a modal immediately restores keyboard and mouse focus to the previous modal on the stack.
3. When the stack reaches depth 0, main chat and composer view receive input focus.

---

## 4. Subagent Hierarchy & Tab Machine

```mermaid
stateDiagram-v2
    [*] --> RootSessionView: Active thread = Main Session (Tab 0)

    RootSessionView --> SubagentView: switch_subagent_view(child_thread_id)
    Note over SubagentView: Transcript displays child messages only.<br/>Status badges highlight child role.

    SubagentView --> SubagentView: Alt+Left / Alt+Right cycles active subagent tabs
    SubagentView --> RootSessionView: switch_subagent_view(None) or click Tab 0

    SubagentView --> SubagentTerminated: terminate_subagent(child_thread_id)
    SubagentTerminated --> RootSessionView: tab removed, thread cleaned up, return to root
```

---

## 5. Deferred Interrupt & Notification Buffer Machine

```mermaid
stateDiagram-v2
    [*] --> IdleState: Turn completed

    IdleState --> QueuedState: Notification arrives during Streaming / ExecutingTool
    QueuedState --> QueuedState: Buffer multiple notifications without interrupting UI
    QueuedState --> FlushedState: Turn transitions to Completed or Idle
    FlushedState --> IdleState: All buffered notices rendered sequentially in transcript
```

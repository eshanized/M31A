# M31A TUI — User Interaction & Event Dispatch Matrix

This matrix documents every user action, keyboard shortcut, mouse gesture, and terminal event against current application state, specifying the resulting state transitions, real backend effects, and failure recovery behaviors.

---

## 1. Global Keyboard Shortcuts

| Input Key | Current State | Target State | Real Backend Invoked | Observable UI Result | Recovery / Edge Case Behavior |
|---|---|---|---|---|---|
| `Ctrl+C` | `turn_state.is_active()` | `CancellationRequested` -> `Cancelling` -> `Idle` | `tokio_util::sync::CancellationToken::cancel()`, kill subprocesses | Cancels running LLM stream or tool execution; renders warning notice. | Session epoch is bumped; in-flight delayed events safely discarded. |
| `Ctrl+C` | `turn_state == Idle`, composer non-empty | `Idle` | None | Clears composer text buffer and resets cursor to 0. | Undo history retains cleared draft if needed. |
| `Ctrl+C` | `turn_state == Idle`, composer empty | `Idle` (1st press) / `Exit` (2nd press within 2s) | `TerminalSessionGuard::restore()` | 1st press: Notice "Press Ctrl+C again within 2 seconds to quit". 2nd press: Exits process. | Timer resets if gap exceeds 2.0s; avoids accidental quit. |
| `Ctrl+Q` | Any (no modal) | Clean Shutdown | `TerminalSessionGuard::restore()` | Restores terminal attributes and exits immediately. | Drops resources, flushes persistent SQLite storage. |
| `Ctrl+M` / `Alt+M` / `F2` | Main View | `ModalState::ModelPicker` | `ProviderBuilder::catalog()` | Opens model selector dialog listing frontier NIM models. | Alt+M and F2 provide fallback for terminals where Ctrl+M sends carriage return. |
| `F1` / `Ctrl+H` | Main View | `ModalState::Help` | None | Displays comprehensive keybinding cheat-sheet and command list. | Esc dismisses and returns focus to composer. |
| `Ctrl+S` | Main View | `ModalState::Settings` | `Config::load()` | Displays interactive settings dialog for approval, sandbox, and theme. | Changes immediately persist to `config.toml`. |
| `Ctrl+P` | Main View | `ModalState::CommandPalette` | None | Opens fuzzy command palette for slash commands. | Fuzzy match against all registered commands. |
| `Ctrl+Y` | Main View | Main View | `arboard::Clipboard`, OSC 52 | Copies last code block from transcript to OS clipboard or emits OSC 52 ANSI escape. | Truthfully informs user if OS clipboard succeeded or OSC 52 was emitted. |
| `Ctrl+D` | Main View | `ModalState::InteractiveDiff` | `git diff HEAD` | Renders scrollable side-by-side or unified git diff viewer. | Displays "Working tree clean" notice if no diffs exist. |
| `Ctrl+W` | Main View | `ModalState::WorktreeSelector` | `WorktreeManager::list_worktrees()` | Opens worktree manager dialog. | Re-evaluates workspace trust upon worktree switch. |
| `Ctrl+R` | Main View | `ModalState::ResumePicker` | `Storage::list_threads()` | Displays previous sessions stored in SQLite database. | Cancels current turn prior to resuming target session. |
| `Ctrl+L` | Main View | Main View | None | Clears visible transcript and resets scroll offset to bottom. | SQLite session history remains intact; only UI display cleared. |
| `Ctrl+T` | Main View | Main View | None | Toggles expanded/collapsed state for all tool calls in transcript. | Re-measures transcript cell heights. |
| `Ctrl+K` | Main View | `ModalState::CriticalPath` | `DirectToolTimings::completed_work()` | Displays execution latency breakdown and critical path timings. | Safe fallback if no prior turn exists. |
| `Ctrl+E` | Main View | `ModalState::ActiveLocks` | `ExecutionLockManager::active_locks()` | Displays active filesystem and execution lock leases. | Helps diagnose concurrent background task deadlocks. |
| `Tab` | Main View (Input Normal) | Collaboration Mode toggled | `Config::save()` | Toggles collaboration mode between `BUILD` and `PLAN`. | Updates header mode badge instantly. |
| `BackTab` | Main View (Input Normal) | Collaboration Mode toggled | `Config::save()` | Toggles collaboration mode between `BUILD` and `PLAN`. | Supported across modern extended terminal protocols. |
| `Alt+Left` / `Alt+Right` | Main View (Subagent tabs present) | Active tab changed | `Storage::list_items()` | Cycles active subagent tab left/right and updates transcript items. | Tab 0 always routes back to Main Session view. |

---

## 2. Composer & Input Navigation

| Key | Mode / State | Target State | Real Backend Invoked | Observable UI Result |
|---|---|---|---|---|
| `Enter` | Empty buffer | No action | None | Cursor stays in place. |
| `Enter` | Buffer starts with `!<cmd>` | `Idle` | `tokio::process::Command` | Asynchronously executes instant shell command; outputs stdout/stderr to transcript. |
| `Enter` | Buffer starts with `/<cmd>` | Action executed | Slash command dispatcher | Executes slash command (e.g. `/model`, `/trust`, `/agents`). |
| `Enter` | Regular text prompt (`turn_state == Idle`) | `turn_state == Submitted` | `AgentEngine::run_turn` | Spawns background turn task; switches spinner to active. |
| `Ctrl+Enter` | Active turn (`Streaming` or `ExecutingTool`) | Prompt queued | In-memory queue | Queues prompt into `queued_prompts`; auto-runs upon turn settle. |
| `Shift+Enter` / `Alt+Enter` | Input Normal | Multiline editing | None | Inserts literal `\n`; expands composer box vertically up to max rows. |
| `Backspace` | Any input position | Text deleted | `unicode_segmentation` | Deletes preceding grapheme cluster safely without splitting UTF-8 byte sequences. |
| `Delete` | Any input position | Text deleted | `unicode_segmentation` | Deletes following grapheme cluster safely. |
| `Left` / `Right` | Any input position | Cursor moved | `unicode_segmentation` | Moves cursor by exactly one grapheme cluster. |
| `Home` / `End` | Single-line input | Cursor to start/end | None | Jumps cursor to index 0 or buffer length. |
| `Up` / `Down` | Multiline input | Cursor up/down line | `unicode_width` | Moves cursor up/down preserving target visual display column. |
| `Up` / `Down` | Single-line empty buffer | History snapshot | None | Navigates through previous prompt input history. |
| `PageUp` / `PageDown` | Main View | Scrolled transcript | Virtualizer | Scrolls transcript view up/down by 8 lines. |

---

## 3. Modal Key Routing (When Modal Is Active)

| Key | Active Modal | Transition / Backend Action |
|---|---|---|
| `Esc` | Any modal | Pops topmost modal; restores previous modal on `ModalStack` or main view. |
| `Up` / `Down` | List / Picker modals | Moves selection index up/down with boundary clamping. |
| `Enter` | `ModelPicker` | Selects highlighted model, persists to `config.toml`, rebuilds provider client. |
| `Enter` | `WorktreeSelector` | Switches working directory to selected worktree; re-evaluates trust. |
| `Enter` | `SelectPicker` | Applies chosen item action; pops modal. |
| `y` / `Y` | `ApprovalDialog` | Sends `ApprovalDecision::AllowOnce` via oneshot channel to unblock tool execution. |
| `a` / `A` | `ApprovalDialog` | Synthesizes prefix rule in `ExecPolicy` and sends `ApprovalDecision::AlwaysAllowPattern`. |
| `s` / `S` | `ApprovalDialog` | Synthesizes file rule in policy and sends `ApprovalDecision::AllowFile`. |
| `v` / `V` | `ApprovalDialog` | Pushes child `ModalState::QuickDiffPreview` to inspect candidate diff before deciding. |
| `n` / `N` | `ApprovalDialog` | Pushes child `ModalState::DeclineFeedbackInput` to capture decline reason; Enter submits Deny. |
| `Esc` | `ApprovalDialog` | Cancels operation; dispatches `ApprovalDecision::Deny` with cancellation note. |
| `Space` | `ExperimentalFeatures` | Toggles feature boolean and immediately persists configuration. |
| `Char('k')` | `SubagentsModal` | Terminates selected child subagent; cleans up thread tab and process resources. |
| `Enter` | `SubagentsModal` | Drills down into selected subagent; loads child transcript in subagent tab view. |

---

## 4. Mouse Interactions

| Mouse Event | Target Widget | Action & State Transition |
|---|---|---|
| `MouseClick` (Left) | Header Mode Badge (`BUILD`/`PLAN`) | Toggles collaboration mode between `BUILD` and `PLAN`. |
| `MouseClick` (Left) | Subagent Tab Bar | Switches view to clicked subagent thread or Main Session. |
| `MouseClick` (Left) | Modal Backdrop | Dismisses topmost modal (`pop_modal`). |
| `MouseClick` (Middle) | Anywhere when modal open | Dismisses topmost modal (`pop_modal`). |
| `ScrollUp` / `ScrollDown` | Transcript Area | Scrolls transcript history up/down. |
| `ScrollUp` / `ScrollDown` | Diff Viewer Modal | Scrolls diff viewer buffer up/down. |
| `MouseClick` (Left) | Tool Call Header | Toggles collapsed/expanded state for the clicked tool call cell. |

---

## 5. Terminal Life Events

| Event | Handling | Outcome |
|---|---|---|
| `Event::Resize(cols, rows)` | Virtualizer recalculates viewport | Next draw dynamically lays out bottom pane, status bar, and transcript cells. |
| `Event::Paste(text)` | `handle_paste_event` | Short pastes inserted at cursor; pastes > 10 lines collapsed into markdown code block. |
| `Event::FocusGained` | `app.is_focused = true` | Restores normal cursor blinking and live telemetry updates. |
| `Event::FocusLost` | `app.is_focused = false` | Dims cursor and suppresses non-critical UI updates. |

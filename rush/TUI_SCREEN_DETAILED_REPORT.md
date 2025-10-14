# M31A TUI Screen Detailed Report

> **Generated:** 2026-06-07
> **Scope:** All TUI screens, components, routing, and wiring in `internal/tui/`

---

## Table of Contents

1. [Screen Inventory](#screen-inventory)
2. [Screen-by-Screen Analysis](#screen-by-screen-analysis)
3. [Routing & Navigation Map](#routing--navigation-map)
4. [Components Reference](#components-reference)
5. [Issues & Broken State](#issues--broken-state)
6. [Missing Implementations](#missing-implementations)

---

## Screen Inventory

### Enum Definition (`internal/tui/types.go:14-35`)

| Enum Value | Constant | Implemented | File |
|---|---|---|---|
| 0 | `ScreenFirstRun` | ✅ Yes | `firstrun.go` |
| 1 | `ScreenREPL` | ✅ Yes | `repl.go`, `repl_view.go`, `repl_stream.go`, `repl_thinking.go`, `repl_commands.go`, `repl_quickactions.go` |
| 2 | `ScreenModelSelector` | ✅ Yes | `modelselector.go`, `modelselector_view.go`, `modelselector_list.go` |
| 3 | `ScreenSettings` | ✅ Yes | `settings.go` |
| 4 | `ScreenResume` | ✅ Yes | `resume.go` |
| 5 | `ScreenPermission` | ✅ Yes | `components/permission.go` (rendered by `app_view.go`) |
| 6 | `ScreenPlan` | ✅ Yes | `plan.go` |
| 7 | `ScreenExecute` | ✅ Yes | `execute.go` |
| 8 | `ScreenVerify` | ✅ Yes | `verify.go` |
| 9 | `ScreenShip` | ✅ Yes | `ship.go` |
| 10 | `ScreenDiff` | ✅ Yes | `diff.go` |
| 11 | `ScreenLedger` | ❌ **NOT IMPLEMENTED** | — |
| 12 | `ScreenRollback` | ❌ **NOT IMPLEMENTED** | — |
| 13 | `ScreenGoalInput` | ✅ Yes | `goalinput.go` |
| 14 | `ScreenDiscuss` | ❌ **NOT IMPLEMENTED** (streams via REPL) | — |
| 15 | `ScreenMetrics` | ✅ Yes | `metrics.go` |

### Total: 16 screen types declared, 13 implemented, 3 missing

---

## Screen-by-Screen Analysis

### 1. ScreenFirstRun (0) — `firstrun.go`

**Purpose:** Initial setup wizard when no API key is configured.

**States:**
- `FirstRunWelcome` → `FirstRunProviderSelect` → `FirstRunKeyInput` → `FirstRunValidating` → `FirstRunKeychainPrompt` → `FirstRunComplete`

**Model:** `FirstRunModel` (804 lines)

**Key Fields:**
- `state FirstRunState` — wizard step
- `apiKeyInput textinput.Model` — API key input
- `providers []string` — selected providers
- `configPath string` — config file path
- `openrouterBaseURL`, `zenBaseURL`, `openrouterReferer`, `openrouterTitle` — provider URLs

**Entry:** `NewApp()` when `resolvedAPIKey == ""`

**Exit:** Transitions to `ScreenREPL` via `AppMsg{Screen: ScreenREPL}`

**Issues:**
- None critical. Well-structured state machine.

---

### 2. ScreenREPL (1) — `repl.go` + 5 supporting files

**Purpose:** Main chat interface. The central hub of the application.

**Model:** `ReplModel` (937 lines)

**Key Fields:**
- `messages []types.Message` — conversation history
- `viewport viewport.Model` — scrollable message area
- `textarea textarea.Model` — user input
- `spinner spinner.Model` — loading indicator
- `streaming bool` — actively streaming LLM response
- `thinking bool` — in thinking/reasoning mode
- `thinkingBlocks map[int]*components.ThinkingBlock` — collapsible thinking segments
- `toolCards map[int]*components.ToolCard` — tool execution cards
- `activeQuestion *QuestionRequestMsg` — AskUserQuestion modal state
- `fallbackBanner string` — provider fallback notification
- `sidebarWidth int` — sidebar reserved space
- `sessionID string` — current session ID
- `registry *provider.Registry` — LLM provider registry
- `activeProvider string`, `activeModel *types.ModelInfo` — current model
- `dispatcher *tools.Dispatcher` — tool execution dispatcher
- `cmdRegistry *CommandRegistry` — slash command registry
- `frecentHistory *FrecentHistory` — command history

**Sub-files:**
| File | Lines | Purpose |
|---|---|---|
| `repl.go` | 937 | Struct, Init, Update, SetProvider, AddMessage, etc. |
| `repl_view.go` | — | View() rendering: header, messages, input area |
| `repl_stream.go` | — | Streaming message assembly, token-by-token rendering |
| `repl_thinking.go` | — | Thinking block management and rendering |
| `repl_commands.go` | — | Slash command processing |
| `repl_quickactions.go` | — | Quick action keybindings |

**Entry:** `NewApp()` when API key exists

**Exit:** Transitions to Plan/Execute/Verify/Ship/Settings/ModelSelector/Resume/Diff via `AppMsg`

**Issues:**
- Streaming chunks are buffered in `pendingStreamChunks` during non-discuss workflow phases — could cause stale data if workflow state is inconsistent
- Question handling (AskUserQuestion) is integrated directly into REPL rather than having its own screen

---

### 3. ScreenSettings (3) — `settings.go`

**Purpose:** Configuration editor with 6 tabs.

**Model:** `SettingsModel` (1103 lines)

**Tabs:**
| Tab | Name | Fields |
|---|---|---|
| 0 | General | `ui.theme`, `ui.compact_mode`, `ui.show_token_usage`, `ui.show_cost_estimate`, `ui.max_iterations` |
| 1 | Provider | `provider.default`, `provider.auto_fallback`, `provider.openrouter.api_key`, `provider.zen.api_key` |
| 2 | Model | `model.default`, `model.context_warning_threshold`, `model.show_thinking_by_default`, `model.auto_collapse_tools`, `model.auto_arbitrage`, `model.arbitrage_threshold` |
| 3 | Permissions | `permissions.default_mode`, `permissions.timeout_seconds` |
| 4 | Features | `features.autodream_enabled`, `features.subagent_enabled`, `features.auto_backup`, `features.resume_on_startup` |
| 5 | Ledger | `ledger.enabled`, `ledger.max_entries` |

**Entry:** `/settings` slash command

**Exit:** `Esc` or `ctrl+c` returns to previous screen

**Issues:**
- API key fields are stored in config file (last resort fallback) — keychain storage is offered but config always has the plaintext value as well
- No validation feedback for invalid numeric ranges

---

### 4. ScreenResume (4) — `resume.go`

**Purpose:** Session browser for resuming previous sessions.

**Model:** `ResumeModel` (861 lines)

**Key Features:**
- Session list with `bubbles/list`
- Fuzzy search filtering
- Session metadata: ID, phase, model, provider, timestamp, message count
- Corrupted session detection with `[!]` badge
- Delete with confirmation
- Active session indicator

**Entry:** `/resume` slash command, or `ResumeOnStartup` config

**Exit:** Select session → loads into `ScreenREPL` via `AppMsg{SessionID: id}`

**Issues:**
- None critical.

---

### 5. ScreenModelSelector (2) — `modelselector.go` + `modelselector_view.go` + `modelselector_list.go`

**Purpose:** Full-screen model picker with provider filter and fuzzy search.

**Model:** `ModelSelector` (325 lines)

**Key Features:**
- Provider filter cycles: All → OpenRouter → Zen (via `P` key)
- Fuzzy search over model name and ID
- Detail pane on `Tab` (description, architecture, pricing)
- Same model on two providers shown as separate entries
- Recent models tracking

**Entry:** `/models` slash command

**Exit:** Select model → `AppMsg{ModelSelected: &ModelSelectedMsg{...}}`; `Esc` → back to previous screen

**Issues:**
- `generateMockUsageData()` placeholder exists in `modelselector_view.go` — latency stats are mocked

---

### 6. ScreenPermission (5) — `components/permission.go`

**Purpose:** Modal overlay for dangerous tool execution approval.

**Component:** `components.PermissionModal`

**Key Features:**
- Centered overlay with tool name, command, risk badge
- Timeout countdown (default 300s)
- Keys: `Y`/`Enter` = approve, `N`/`Esc` = deny, `A` = approve and remember
- Auto-deny on timeout
- Queued permission requests when modal already active

**Entry:** `PermissionRequestMsg` from tool dispatcher

**Exit:** Permission response → returns to `prevScreen`

**Issues:**
- Permission queue works but could stall if dispatcher channel is full

---

### 7. ScreenPlan (6) — `plan.go`

**Purpose:** Task plan review screen after Plan phase.

**Model:** `PlanModel` (589 lines)

**Key Features:**
- Task list with status indicators
- Cost/time/model panel
- Per-task arbitrage suggestion
- Diff preview overlay for predicted files
- Dependency graph view (Tab toggle)
- Keys: `A` = accept, `E` = edit, `R` = retry, `D` = diff preview, `Tab` = dependency graph, `O` = optimize

**Entry:** `PlanReadyMsg` from workflow engine → `handlePlanReady()`

**Exit:** Accept → `AppMsg{Screen: ScreenExecute}`; `Esc` → back to REPL

**Issues:**
- BUG-05: arbitrage in Plan was previously broken — now fixed with `ApplyArbitrage()` method
- Cost estimation is placeholder (`fmt.Sprintf("%d tasks", len(tasks))`) — no real cost model

---

### 8. ScreenExecute (7) — `execute.go`

**Purpose:** Task execution progress tracker.

**Model:** `ExecuteModel` (515 lines)

**Key Features:**
- Task list with status: `[x]` done, `[>]` running, `[ ]` queued, `[ ]` blocked
- Progress bar in header: `N of M complete X%`
- Live tool cards for current task
- Keys: `P` = pause, `R` = resume, `S` = skip
- Metrics: tokens, cost, tool calls

**Entry:** `PhaseResultMsg{Phase: PhaseExecute}` → `handlePhaseExecute()`

**Exit:** All tasks done → auto-transitions to Verify via workflow engine

**Issues:**
- `paused` field exists but pause/resume is not fully wired to the workflow engine
- `transitioning` and `transitionSec` fields suggest countdown animation was planned but not completed

---

### 9. ScreenVerify (8) — `verify.go`

**Purpose:** Verification results display with self-heal support.

**Model:** `VerifyModel` (388 lines)

**Key Features:**
- Pass/fail checklist per task
- `[H]` self-heal button
- `[S]` skip button
- Self-heal confirmation dialog
- Results map: `map[int]workflow.VerificationResult`
- Heal callback via `SetHealFunc()`

**Entry:** `PhaseResultMsg{Phase: PhaseVerify}` → `handlePhaseVerify()`

**Exit:** All tasks pass/skipped → auto-transitions to Ship

**Issues:**
- `healFunc` callback calls `m.workflowEngine.HealTask(taskID)` but the return value is not used to trigger re-verification
- Self-heal confirmation UX could be confusing (double confirm pattern)

---

### 10. ScreenShip (9) — `ship.go`

**Purpose:** Session completion summary.

**Model:** `ShipModel` (427 lines)

**Key Features:**
- Summary banner: task count, commit log, diff stats
- Duration, token usage, cost
- Keys: `O` = open in browser, `N` = new session, `R` = return to REPL
- New session confirmation dialog

**Entry:** `PhaseResultMsg{Phase: PhaseShip}` → `handlePhaseShip()`

**Exit:** `N` → creates fresh session; `R` → returns to `ScreenREPL`

**Issues:**
- `O` key (open in browser) implementation depends on detecting dev server URL — may not work for all projects

---

### 11. ScreenDiff (10) — `diff.go`

**Purpose:** Git diff viewer with syntax highlighting.

**Model:** `DiffModel` (322 lines)

**Key Features:**
- Parsed diff lines with types: Context, Added, Deleted, Header, Hunk
- Color-coded line types
- Keyboard scrolling (j/k, arrows, PgUp/PgDn)
- `Tab` toggle between unified and split view (partially implemented)
- `Esc` closes and returns to previous screen

**Entry:** `DiffScreenMsg` from various commands

**Exit:** `DiffCloseMsg` or `Esc` → returns to `prevScreen`

**Issues:**
- Split view toggle is mentioned in comments but the implementation only shows unified view
- No file tree sidebar for multi-file diffs

---

### 12. ScreenGoalInput (13) — `goalinput.go`

**Purpose:** Full-screen goal entry for `/workflow start`.

**Model:** `GoalInputModel` (221 lines)

**Key Features:**
- Large centered textarea with brand border
- Recent goals quick-select (from session history)
- Search/filter recent goals
- Tab switch between textarea and recent goals
- `Enter` confirms goal and starts workflow
- `Esc` cancels and returns to REPL

**Entry:** `/workflow start` command or slash command routing

**Exit:** Confirm → transitions to `ScreenREPL` and starts `PhaseDiscuss` workflow

**Issues:**
- Goal input → workflow start skips `PhaseInitialize` and goes directly to `PhaseDiscuss` — may miss project type detection

---

### 13. ScreenMetrics (15) — `metrics.go`

**Purpose:** Session analytics dashboard ("Flight Data").

**Model:** `MetricsModel` (333 lines)

**Key Features:**
- Aggregate stats: total sessions, tokens, cost, tool calls
- Per-model usage breakdown
- Daily usage chart (last 14 days)
- Zero API calls — reads local session data only
- `Esc` returns to previous screen

**Entry:** `/metrics` slash command

**Exit:** `Esc` → back to previous screen

**Issues:**
- No interactive filtering or drill-down
- Daily usage chart is text-based (no sparkline or bar chart)

---

### 14. ScreenDiscuss (14) — **NOT IMPLEMENTED**

**Status:** Declared in enum but has no dedicated screen.

**Current behavior:** Discuss phase Q&A flows through `ScreenREPL` using:
- `askNextDiscussQuestion()` method on `AppState`
- `DiscussAnswerTimeoutMsg` for 5-minute timeout
- Inline question rendering in REPL messages

**Impact:** The discuss phase reuses the REPL screen, which means:
- No dedicated Q&A UI with progress indicator
- Questions are mixed with regular chat messages
- No visual separation between discuss questions and normal conversation

---

### 15. ScreenLedger (11) — **NOT IMPLEMENTED**

**Status:** Declared in enum but no implementation file exists.

**Expected behavior (from spec):**
- Browse the learning journal (`~/.m31a/LEDGER.md`)
- Filterable viewer
- Per-session entry display

**Impact:** The `/ledger` slash command has no screen to navigate to.

---

### 16. ScreenRollback (12) — **NOT IMPLEMENTED**

**Status:** Declared in enum but no implementation file exists.

**Expected behavior (from spec):**
- Interactive commit list with `[HEAD]` marker
- `Enter` = rollback, `D` = view diff
- Backup branch creation before rollback
- Soft/hard reset options

**Impact:** The `/rollback` slash command has no screen to navigate to.

---

## Routing & Navigation Map

### Screen Transitions (`app_update.go` + `app_update_workflow.go`)

```
ScreenFirstRun
  └─ [setup complete] ──► ScreenREPL

ScreenREPL
  ├─ [ctrl+p] ──► CommandPalette (overlay, not a screen)
  ├─ [/settings] ──► ScreenSettings
  ├─ [/resume] ──► ScreenResume
  ├─ [/models] ──► ScreenModelSelector
  ├─ [/workflow <goal>] ──► ScreenREPL (starts workflow, stays on REPL)
  ├─ [/plan <goal>] ──► (via RunPhaseCmd, stays on REPL initially)
  ├─ [/metrics] ──► ScreenMetrics
  ├─ [workflow phase complete] ──► ScreenPlan/ScreenExecute/ScreenVerify/ScreenShip
  ├─ [DiffScreenMsg] ──► ScreenDiff
  └─ [PermissionRequestMsg] ──► ScreenPermission

ScreenSettings
  └─ [Esc/ctrl+c] ──► ScreenREPL

ScreenResume
  └─ [select session] ──► ScreenREPL

ScreenModelSelector
  ├─ [Esc] ──► prevScreen
  └─ [select model] ──► prevScreen

ScreenPlan
  ├─ [A/accept] ──► ScreenExecute (via RunPhaseCmd)
  ├─ [Esc] ──► ScreenREPL
  └─ [D/diff] ──► ScreenDiff

ScreenExecute
  ├─ [all tasks done] ──► ScreenVerify (via workflow engine)
  └─ [Esc] ──► ScreenREPL

ScreenVerify
  ├─ [all pass] ──► ScreenShip (via workflow engine)
  └─ [Esc] ──► ScreenREPL

ScreenShip
  ├─ [N/new session] ──► ScreenREPL (fresh session)
  ├─ [R/return] ──► ScreenREPL
  └─ [O/open] ──► browser launch

ScreenDiff
  └─ [Esc/DiffCloseMsg] ──► prevScreen

ScreenMetrics
  └─ [Esc] ──► prevScreen

ScreenGoalInput
  ├─ [Enter/confirm] ──► ScreenREPL (starts Discuss workflow)
  └─ [Esc/cancel] ──► ScreenREPL

ScreenPermission
  ├─ [Y/A/timeout] ──► prevScreen (permission response sent)
  └─ [N/E/ctrl+c] ──► prevScreen (permission denied)
```

### Workflow Phase → Screen Mapping

| Workflow Phase | Screen | Entry Point |
|---|---|---|
| `PhaseInitialize` | `ScreenREPL` (stays) | `RunPhaseCmd()` |
| `PhaseDiscuss` | `ScreenREPL` (inline Q&A) | `handlePhaseDiscuss()` |
| `PhasePlan` | `ScreenPlan` | `handlePhasePlan()` |
| `PhaseExecute` | `ScreenExecute` | `handlePhaseExecute()` |
| `PhaseVerify` | `ScreenVerify` | `handlePhaseVerify()` |
| `PhaseShip` | `ScreenShip` | `handlePhaseShip()` |
| `PhaseIdle` | `ScreenREPL` | `handlePhaseResult()` |

---

## Components Reference

All components live in `internal/tui/components/`:

| Component | File | Purpose |
|---|---|---|
| `PermissionModal` | `permission.go` | Tool execution approval overlay |
| `MessageRenderer` | `message.go` | Markdown message rendering with Glamour |
| `ThinkingBlock` | `thinking.go` | Collapsible thinking/reasoning segments |
| `ToolCard` | `toolcard.go` | Tool execution status cards |
| `QuestionModel` | `question.go` | AskUserQuestion inline form |
| `ProgressBar` | `progress.go` | Progress bar component |
| `Badge` | `badge.go` | Status/label badges |
| `FilterChips` | `filterchips.go` | Filter chip toggles |
| `StatRow` | `statrow.go` | Key-value stat display |
| `MetricCard` | `metriccard.go` | Metric display card |
| `BashRenderer` | `bash_renderer.go` | Bash tool output renderer |
| `FileRenderers` | `file_renderers.go` | FileRead/FileWrite/Glob/Grep renderers |
| `SpecialRenderers` | `special_renderers.go` | Special tool renderers |
| `ToolRenderers` | `toolrenderers.go` | Unified tool renderer dispatch |
| `Starfield` | `starfield.go` | Decorative background animation |
| `Sparkline` | `sparkline.go` | Sparkline chart component |

### Supporting TUI Files

| File | Purpose |
|---|---|
| `header.go` | REPL header with brand, provider badge, model name, context bar |
| `statusbar.go` | Bottom status bar with operation, streaming indicator, keyboard hints |
| `sidebar.go` | Git file status sidebar (ctrl+b toggle) |
| `cmdpalette.go` | Command palette overlay (ctrl+p) |
| `commands.go` | Command registry and all slash command definitions |
| `commands_core.go` | Core commands: /help, /clear, /status, /quit |
| `commands_config.go` | Config commands: /theme, /config |
| `commands_session.go` | Session commands: /save, /load, /fork, /prev, /next |
| `commands_workflow.go` | Workflow commands: /workflow, /plan, /execute, /verify, /ship |
| `commands_git.go` | Git commands: /git, /diff, /log |
| `commands_ai.go` | AI commands: /optimize, /compress, /memory |
| `keybindings.go` | Key binding definitions and leader key support |
| `keybindings_screens.go` | Per-screen key binding overrides |
| `streaming.go` | Stream chunk processing and message assembly |
| `health.go` | Health check ticker and async check commands |
| `cache_refresh.go` | Model cache refresh ticker |
| `truncate.go` | Text truncation utilities |
| `backup.go` | Session backup utilities |
| `history.go` | Frecent command history |
| `providerbadge.go` | Provider badge rendering (OR/ZEN) |
| `theme/` | Theme manager and color definitions |

---

## Issues & Broken State

### Critical Issues

| ID | Severity | Screen | Description |
|---|---|---|---|
| **MISS-01** | HIGH | ScreenLedger | Screen enum declared (11) but no implementation. `/ledger` command has no screen to navigate to. |
| **MISS-02** | HIGH | ScreenRollback | Screen enum declared (12) but no implementation. `/rollback` command has no screen to navigate to. |
| **MISS-03** | MEDIUM | ScreenDiscuss | Screen enum declared (14) but no dedicated screen. Discuss Q&A flows through REPL without visual separation or progress tracking. |

### Moderate Issues

| ID | Severity | Screen | Description |
|---|---|---|---|
| **WIR-01** | MEDIUM | ScreenPlan | Cost estimation is placeholder: `fmt.Sprintf("%d tasks", len(tasks))`. No real cost model integration. |
| **WIR-02** | MEDIUM | ScreenExecute | `paused` field exists but pause/resume not wired to workflow engine. `P`/`R` keys have no effect. |
| **WIR-03** | MEDIUM | ScreenVerify | `healFunc` calls `HealTask()` but return value ignored — re-verification not triggered. |
| **WIR-04** | LOW | ScreenDiff | Split view toggle mentioned in comments but only unified view implemented. |
| **WIR-05** | LOW | ScreenModelSelector | `generateMockUsageData()` placeholder — latency stats are fabricated. |
| **WIR-06** | LOW | ScreenGoalInput | Goal input → workflow start skips `PhaseInitialize` (project type detection). |
| **WIR-07** | LOW | ScreenMetrics | Daily usage is text-only — no sparkline or bar chart visualization. |

### Minor Issues

| ID | Severity | Screen | Description |
|---|---|---|---|
| MIN-01 | LOW | All | `defaultSidebarWidth = 120` is hardcoded — should be configurable. |
| MIN-02 | LOW | ScreenREPL | `frecentHistory` max size is not configurable. |
| MIN-03 | LOW | ScreenPermission | Permission queue could stall if dispatcher channel is full. |

---

## Missing Implementations

### 1. ScreenLedger (Screen = 11)

**Required for:** `/ledger` slash command

**Expected features:**
- Browse `~/.m31a/LEDGER.md` entries
- Filter by project type, goal keywords, date
- Per-session detail view
- Aggregate stats display

**Implementation notes:**
- `pkg/ledger/` already exists with `Ledger`, `Entry`, `Stats`, `Query` types
- Screen needs: `LedgerModel` struct, `View()`, `Update()`, list rendering

### 2. ScreenRollback (Screen = 12)

**Required for:** `/rollback` slash command

**Expected features:**
- Interactive commit list from `git log`
- `[HEAD]` marker on current commit
- Backup branch creation before rollback
- Soft/hard reset options
- Diff view per commit

**Implementation notes:**
- `pkg/rollback/` already exists with `Rollback`, `SessionCommits`, `SoftReset`, `HardReset` types
- Screen needs: `RollbackModel` struct, commit list, diff preview, confirmation dialogs

### 3. ScreenDiscuss (Screen = 14)

**Required for:** Dedicated discuss phase Q&A flow

**Expected features:**
- Progress indicator (question N of M)
- Dedicated Q&A layout (no chat history clutter)
- Timer display for 5-minute timeout
- Skip/answer buttons

**Implementation notes:**
- Currently streams through REPL via `askNextDiscussQuestion()` and `DiscussAnswerTimeoutMsg`
- Would need: `DiscussModel` struct, question display, answer input, progress bar

---

## File Index

### Core TUI Files (77 files in `internal/tui/`)

| File | Lines (approx) | Purpose |
|---|---|---|
| `app.go` | 786 | AppState struct, NewApp, Init, RunPhaseCmd |
| `app_update.go` | 1064 | Main Update() with message routing |
| `app_view.go` | 163 | Main View() with screen dispatch |
| `app_update_workflow.go` | 628 | Phase result handlers, permission, discuss |
| `app_workflow.go` | — | Workflow state persistence |
| `types.go` | 184 | Screen enum, message types |
| `repl.go` | 937 | REPL model and update logic |
| `repl_view.go` | — | REPL rendering |
| `repl_stream.go` | — | Streaming message assembly |
| `repl_thinking.go` | — | Thinking block management |
| `repl_commands.go` | — | Slash command processing |
| `repl_quickactions.go` | — | Quick action keybindings |
| `firstrun.go` | 804 | First-run setup wizard |
| `settings.go` | 1103 | Settings screen (6 tabs) |
| `resume.go` | 861 | Session browser |
| `modelselector.go` | 325 | Model selector logic |
| `modelselector_view.go` | — | Model selector rendering |
| `modelselector_list.go` | — | Model list item rendering |
| `plan.go` | 589 | Plan review screen |
| `execute.go` | 515 | Task execution tracker |
| `verify.go` | 388 | Verification results screen |
| `ship.go` | 427 | Session completion summary |
| `diff.go` | 322 | Git diff viewer |
| `metrics.go` | 333 | Session analytics dashboard |
| `goalinput.go` | 221 | Full-screen goal entry |
| `header.go` | 242 | REPL header rendering |
| `statusbar.go` | 166 | Bottom status bar |
| `sidebar.go` | 241 | Git file status sidebar |
| `cmdpalette.go` | — | Command palette overlay |
| `commands.go` | — | Command registry |
| `commands_core.go` | — | Core commands |
| `commands_config.go` | — | Config commands |
| `commands_session.go` | — | Session commands |
| `commands_workflow.go` | — | Workflow commands |
| `commands_git.go` | — | Git commands |
| `commands_ai.go` | — | AI commands |
| `keybindings.go` | — | Key binding definitions |
| `keybindings_screens.go` | — | Per-screen key bindings |
| `streaming.go` | — | Stream processing |
| `health.go` | — | Health check ticker |
| `cache_refresh.go` | — | Cache refresh ticker |
| `backup.go` | — | Session backup |
| `history.go` | — | Frecent history |
| `providerbadge.go` | — | Provider badge rendering |
| `truncate.go` | — | Text truncation |
| `theme/` | — | Theme manager + colors |

### Component Files (23 files in `internal/tui/components/`)

| File | Lines (approx) | Purpose |
|---|---|---|
| `permission.go` | — | Permission modal |
| `message.go` | — | Message renderer |
| `thinking.go` | — | Thinking blocks |
| `toolcard.go` | — | Tool cards |
| `question.go` | — | AskUserQuestion form |
| `progress.go` | — | Progress bar |
| `badge.go` | — | Status badges |
| `filterchips.go` | — | Filter chip toggles |
| `statrow.go` | — | Stat row display |
| `metriccard.go` | — | Metric card |
| `bash_renderer.go` | — | Bash output renderer |
| `file_renderers.go` | — | File tool renderers |
| `special_renderers.go` | — | Special renderers |
| `toolrenderers.go` | — | Tool renderer dispatch |
| `starfield.go` | — | Background animation |
| `sparkline.go` | — | Sparkline chart |

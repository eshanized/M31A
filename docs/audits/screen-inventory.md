# TUI Screen Inventory Audit

**Date:** 2026-07-11  
**Purpose:** Prioritize migration to new Screen interface + Router architecture  
**Source:** `internal/tui/app_state.go` (sub-model fields), `internal/tui/tuitypes/tuitypes.go` (Screen enum)

---

## Screen Inventory Table

| Screen | Model File(s) | Size (lines) | Test Coverage | Fix Commits (6mo) | Cross-Screen Coupling |
|--------|--------------|--------------|---------------|-------------------|----------------------|
| **REPL** (ScreenREPL) | `repl_model.go`, `repl_view.go`, `repl_stream.go`, `repl_state.go` | 212 + 780 + 300+ | `repl_state_extra_test.go`: 10 tests | 0 | **HIGH** — AppState handlers directly mutate `replModel` fields (streaming, thinking, messages, toolCards, lastUsage, lastCost). Sidebar calls `StartTokenBurn()`, `AddToolCallStart()`, `CompleteToolCall()`. Used by `chatHistoryModel.SetMessages()`, `autoDream`. |
| **Sidebar** (persistent) | `sidebar_model.go` | 1621 | `sidebar_extra_test.go`: 22, `sidebar_fixes_test.go`: 4 | 1 (`fix: add bounds check`) | **HIGH** — Central hub. Called from 15+ AppState handlers. Tracks git status, todos, token usage, context pressure, execution metrics. Receives events from workflow, tools, REPL. |
| **Plan** (ScreenPlan) | `plan_model.go` | 385 | `plan_extra_test.go`: 11 tests | 1 | Medium — Receives tasks from `executeModel.tasks` (line 353), reads `workflowEngine.PlanContent()`/`PlanVersion()`. Updates sidebar todo items. |
| **Settings** (ScreenSettings) | `settings_model.go`, `config_model.go`, `config_model_view.go` | 860 + 412 = 1272 | `settings_model_extra_test.go`: 55, `config_model_extra_test.go`: 47 | 1 | Medium — Reads/writes `AppState.config`, `AppState.keychain`, `AppState.registry`. `ConfigModel` and `SettingsModel` share config state. |
| **Execute** (ScreenExecute) | `execute_model.go` | 336 | None specific | 0 | **HIGH** — Tightly coupled to `PlanModel.tasks` (copied at line 353, 359). Mutated by 10+ workflow handlers (`app_handlers_workflow.go`). Sidebar reads `executeModel.tasks`. |
| **Verify** (ScreenVerify) | `verify_model.go` | 337 | `verify_extra_test.go`: 5 tests | 0 | Medium — `StartHealing()`/`StopHealing()` called from workflow handlers. `SetManualSteps()` from phase result. |
| **Ship** (ScreenShip) | `ship_model.go` | 204 | None specific | 0 | Low — `SetDemonstration()` from phase result. |
| **Runtime** (ScreenRuntimeCheck) | `runtime_model.go`, `runtime_view.go` | 168 | `handler_runtime_test.go`: 5 tests | 0 | Low — `SetSummary()` from workflow msg. |
| **Discuss** (ScreenDiscuss) | `discuss_model.go` | 202 | None specific | 1 | Low — `SetTimeout()`/`SetDimensions()` from nav. |
| **Home** (ScreenHome) | `home_model.go` | 248 | None specific | 0 | Low — Receives `cmdRegistry`, `config` at init. |
| **Help** (ScreenHelp) | `help_model.go` | 404 | None specific | 0 | Low — Receives `keyRegistry` at init. |
| **Resume** (ScreenResume) | `resume_model.go` | 212 | `resume_extra_test.go`: 5, `resume_fixes_test.go`: 3 | 0 | Medium — `sessionDetailModel.SetSession()` called from `resumeModel` cursor (line 445-449). |
| **SessionDetail** (ScreenSessionDetail) | `sessiondetail_model.go` | 122 | None specific | 0 | Medium — `SetSession()` called from resume screen navigation. |
| **Ledger** (ScreenLedger) | `ledger_model.go` | 156 | None specific | 0 | Low — `LoadEntries()` called on nav. |
| **Rollback** (ScreenRollback) | `rollback_model.go` | 288 | `rollback_extra_test.go`: 4 tests | 0 | Low — `LoadCommits()` called on nav. |
| **Metrics** (ScreenMetrics) | `metrics_model.go` | 248 | None specific | 0 | Low — `LoadStatsCmd()` called on nav. |
| **FileExplorer** (ScreenFileExplorer) | `fileexplorer_model.go` | 142 | None specific | 0 | Low — `SetRoot()` called on nav. |
| **ToolDetail** (ScreenToolDetail) | `tooldetail_model.go` | 92 | None specific | 0 | Low — `SetContent()` called from sidebar click handler. |
| **GhostPicker** (ScreenGhostPicker) | `ghostpicker_model.go` | 193 | None specific | 0 | Low — `SetDimensions()` only. |
| **GhostOutput** (ScreenGhostOutput) | `ghostoutput_model.go` | 178 | None specific | 0 | Low — `SetResult()` from sidebar handler. |
| **ConfirmQuit** (ScreenConfirmQuit) | `confirmquit_model.go` | 85 | None specific | 0 | None — Pure modal. |
| **ChatHistory** (ScreenChatHistory) | `chathistory_model.go` | 147 | None specific | 0 | **Medium** — `SetMessages(m.replModel.Messages())` at view time (line 780). |
| **CommandPalette** (ScreenCommandPalette) | `commandpalette_model.go`, `cmdpalette.go` | 524 | None specific | 0 | Low — Standalone. |
| **ModelSelector** (ScreenModelSelector) | `modelselector_model.go`, `modelselector_view.go` | 293 | None specific | 0 | Low — Standalone. |
| **FirstRun** (ScreenFirstRun) | `firstrun_model.go` | 780 | `firstrun_extra_test.go`: 12 tests | 0 | Low — Wizard flow, writes to config/keychain. |
| **GoalInput** (ScreenGoalInput) | `goalinput_model.go` | 159 | None specific | 0 | Low — Standalone. |
| **Config** (ScreenConfig) | `config_model.go`, `config_model_view.go` | 412 | `config_model_extra_test.go`: 47 tests | 1 | Medium — Shares `AppState.config` with Settings. |
| **Bisect** (ScreenBisect) | `bisect_model.go` | 247 | None specific | 1 | Low — `SetCommits()` from workflow handler. |
| **Dashboard** (ScreenDashboard) | `dashboard_model.go` | 230 | None specific | 0 | Low — `SetWorkflowState()` from nav/phase result. |
| **Notifications** (ScreenNotifications) | `notification_model.go` | 82 | None specific | 0 | Low — `AddNotification()` from `addToastCmd()`. |
| **PhaseModelPicker** (ScreenPhaseModelPicker) | `phasemodelpicker.go`, `phasemodelpicker_view.go` | 337 | `phasemodelpicker_extra_test.go`: 7 tests | 0 | Low — Standalone. |
| **Subagents** (ScreenSubagents) | `subagents_model.go` | 317 | None specific | 0 | Low — `ApplyEvent()` from subagent handler. |
| **Diff** (ScreenDiff) | `diff_model.go`, `diff_view.go` | 127 | None specific | 0 | Low — Standalone viewer. |
| **Permission** (ScreenPermission) | Modal in `app.go`/`components/permission.go` | N/A (modal) | `permission_test.go`, `permission_desc_test.go` | 0 | N/A — Overlay, not a full screen. |

---

## Test Coverage Summary

| Coverage Level | Screens |
|----------------|---------|
| **Good** (≥10 tests) | Settings/Config (102), Plan (11), Sidebar (26), REPL state (10) |
| **Minimal** (1-9 tests) | FirstRun (12), Resume (8), PhaseModelPicker (7), Rollback (4), Bisect (0 but has handler test) |
| **None** (0 model-specific tests) | Execute, Verify, Ship, Runtime, Discuss, Home, Help, Ledger, Metrics, FileExplorer, ToolDetail, GhostPicker, GhostOutput, ConfirmQuit, ChatHistory, CommandPalette, ModelSelector, GoalInput, Dashboard, Notifications, Subagents, Diff |

---

## Cross-Screen Coupling Details

### High Coupling (Migration Risk)

1. **REPL ↔ Sidebar** — Bidirectional. REPL pushes streaming state, tool calls, messages; Sidebar reads token burn, context pressure, git status.
2. **Execute ↔ Plan** — Execute copies `PlanModel.tasks` (lines 353, 359 in `app_update_phase.go`). Plan reads `executeModel.tasks` for sidebar init.
3. **Execute ↔ Workflow Handlers** — 10+ direct mutations in `app_handlers_workflow.go` (`SetCurrentTask`, `UpdateTaskStatus`, `AppendLiveOutput`, `UpdateTaskStatus`).
4. **ChatHistory → REPL** — `SetMessages(m.replModel.Messages())` at render time (view layer coupling).
5. **Sidebar → Everything** — Central event sink for token usage, tool calls, todo updates, git status, narrative state.

### Medium Coupling

- **Settings ↔ Config** — Both mutate `AppState.config`; `ConfigModel` rebuilds sections on view.
- **Resume ↔ SessionDetail** — Resume passes selected session to detail screen.
- **Dashboard ↔ Workflow** — Reads `workflowPhase`, `workflowGoal`, `activeProvider` from AppState.

### Low Coupling (Good Migration Candidates)

- Home, Help, Ledger, Rollback, Metrics, FileExplorer, ToolDetail, GhostPicker, GhostOutput, ConfirmQuit, CommandPalette, ModelSelector, FirstRun, GoalInput, Bisect, Dashboard, Notifications, PhaseModelPicker, Subagents, Diff

---

## Recommended Migration Order

### Phase 1: Low Risk / High Value (Migrate First)
| Screen | Rationale |
|--------|-----------|
| **ConfirmQuit** | Tiny (85 lines), zero coupling, pure modal. Perfect pilot for new `Screen` interface. |
| **Help** | Self-contained (404 lines), only needs `keyRegistry` at init. No workflow coupling. |
| **Home** | Landing screen (248 lines), only needs `cmdRegistry` + `config`. Clean boundary. |
| **GoalInput** | Standalone (159 lines), single purpose. |
| **FileExplorer** | Simple (142 lines), only `SetRoot()` + `SetDimensions()`. |
| **GhostPicker / GhostOutput** | Pair (193+178 lines), isolated feature. |
| **PhaseModelPicker** | Standalone (337 lines), 7 tests, clean dual-model selection UI. |

### Phase 2: Medium Risk / Good Test Coverage
| Screen | Rationale |
|--------|-----------|
| **Settings / Config** | Large (1272 lines combined) but **excellent test coverage** (102 tests). Config mutation is well-tested. Good candidate once interface boundary is defined. |
| **Plan** | Medium size (385 lines), 11 tests. Coupling to Execute is one-way (Plan reads Execute's tasks only for sidebar). |
| **Discuss** | Small (202 lines), simple Q&A flow. One fix commit. |
| **Bisect** | 247 lines, one fix. Clean `SetCommits()` boundary. |
| **Dashboard** | 230 lines, reads workflow state passively. |

### Phase 3: High Risk / Migrate Last
| Screen | Rationale |
|--------|-----------|
| **REPL** | **Largest** (~1300+ lines across files), **highest coupling** (streaming, tools, messages, sidebar, autoDream, chatHistory). Core of the app. |
| **Sidebar** | **Largest single file** (1621 lines), central event hub. Touches everything. |
| **Execute** | Tight Plan coupling, 10+ workflow handler mutations. Complex live output rendering. |
| **Verify** | Healing integration, spinner state, manual steps. |
| **ChatHistory** | View-time coupling to REPL messages (architectural smell). |

---

## Architectural Notes for New Screen Interface

1. **Current pattern**: Each screen model has `Update(msg) (Model, Cmd)` and `View() string` but is invoked via giant switch statements in `app_routing.go` (Update) and `app_view.go` (View).

2. **Proposed Screen interface**:
   ```go
   type Screen interface {
       Init() Cmd
       Update(msg tea.Msg) (Screen, Cmd)
       View() string
       SetDimensions(w, h int)
       SetTheme(theme.Theme)
   }
   ```

3. **Router** would hold `map[ScreenID]Screen` and delegate, eliminating the 35-case switches.

4. **Shared state** (workflow engine, config, sidebar) stays in `AppState`; screens receive only what they need via `SetDimensions`, `SetTheme`, and explicit data setters (`SetTasks`, `SetPlanContent`, etc.).

5. **Migration strategy**: Extract one screen at a time behind the interface, update `AppState` to hold `Screen` instead of concrete `*PlanModel`, etc., keep `AppState.Update/View` as thin delegators.
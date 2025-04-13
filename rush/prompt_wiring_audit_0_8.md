# Wiring Audit Prompt: Phase 0 through Phase 8

**Write this prompt to:** `rush/prompt_wiring_audit_0_8.md`

---

## Context

M31A is a Go terminal AI coding assistant with a six-phase workflow engine (Initialize → Discuss → Plan → Execute → Verify → Ship), a Bubble Tea TUI with 10 screens, a provider abstraction (OpenRouter + Zen), a command registry with 16+ slash commands, and a streaming token-by-token rendering pipeline.

After completing all 8 implementation phases and fixing 12 audit deviations, the next step is a **wiring-focused audit** — not checking whether deliverables exist, but whether they are **connected to each other**.

## What "Wiring Issues" Are

A wiring issue exists when:

1. **Message defined but never emitted** — a `tea.Msg` type has a handler but no production code constructs it
2. **Registry built but never instantiated** — a registry/manager exists but is never added to `AppState`
3. **Screen fully implemented but unreachable** — a screen model exists but no code transitions to it
4. **Function tested but never called in production** — a function only runs in `_test.go` files
5. **Incompatible connection patterns** — two subsystems use different communication patterns and can't talk
6. **Handler exists but input never routed** — a message is handled locally but never reaches the parent
7. **Inline code bypasses proper system** — hardcoded string comparisons skip a proper registry

## The Prompt

You are performing a **wiring-focused audit** of the M31A codebase at `/home/snigdha/Desktop/Helix/M31A`. Unlike a general audit that checks deliverables exist, this audit finds **broken connections** between components.

### Source of Truth

- Primary: `adrenaline/ROADMAP.md` — phase deliverables
- Secondary: `AGENTS.md` — architectural rules
- Tertiary: `internal/tui/types.go` — all message types that should flow through the event loop

### Part 1: Message Bus Wiring

For every `tea.Msg` type in `internal/tui/`, trace its full lifecycle: **defined → emitted → handled → response**.

Check these specific messages:
- `AppMsg` — emitted by first-run, resume, model selector; handled by AppState
- `HealthCheckTickMsg` — emitted by ticker; handled by AppState
- `RefreshCacheMsg` — emitted by ticker; handled by AppState
- `PermissionRequestMsg` — emitted by permissionListenerCmd; handled by AppState
- `PermissionResponseMsg` — emitted inline in AppState; handled by AppState
- `FallbackEventMsg` — defined in types.go, handled in AppState and ReplModel; **verify emission path exists**
- `ThinkingToggleMsg` — emitted by ReplModel (T key); handled by ReplModel
- `ModelSelectedMsg` — emitted by ModelSelector; handled by AppState
- `SettingsSavedMsg` — emitted by SettingsModel; **verify it reaches AppState**
- `StreamMsg`, `StreamDoneMsg`, `StreamErrorMsg` — emitted by StartStreamCmd; **verify StartStreamCmd is called in production**
- `ErrorMsg` — emitted by streaming goroutine and settings; handled by AppState

For each message, report: ✅ (fully wired), ⚠️ (partially wired — defined + handled but never emitted, or emitted but handler is no-op), or ❌ (broken — defined but never emitted AND never handled).

### Part 2: Screen Transition Wiring

Map the complete screen transition graph. For each of the 10 screen constants in `types.go`, find **what code sets `m.screen = ScreenX`**.

Check:
- `ScreenFirstRun` → `ScreenREPL` (via firstRunModel + AppMsg)
- `ScreenREPL` → `ScreenSettings` (inline `/settings` string check)
- `ScreenREPL` → `ScreenResume` (inline `/resume` string check)
- `ScreenREPL` → `ScreenModelSelector` (inline `/models` string check)
- `ScreenModelSelector` → `m.prevScreen` (Esc key + ModelSelectedMsg)
- `ScreenPermission` → `m.prevScreen` (PermissionResponseMsg)
- `ScreenPlan` — **who transitions to it?**
- `ScreenExecute` — **who transitions to it?**
- `ScreenVerify` — **who transitions to it?**
- `ScreenShip` — **who transitions to it?**

Report all unreachable screens and screens whose View() renders only placeholder text.

### Part 3: Command Registry Wiring

The `CommandRegistry` in `internal/tui/commands.go` registers 16+ slash commands. Trace whether it is actually used at runtime.

Check:
- Is `CommandRegistry` instantiated in `AppState` or anywhere outside tests?
- Is `registry.Execute()` called in `app.go` or `repl.go`?
- Which commands are handled by inline string comparisons instead of the registry?
- For each of these commands, verify whether they actually work:
  - `/help`, `/clear`, `/status`, `/model`, `/provider`, `/reset`, `/quit`
  - `/undo`, `/compress`, `/ledger`, `/rollback`, `/sessions`, `/goal`
  - `/phase`, `/config`, `/fallback`

Report: commands handled inline vs. through registry vs. fully dead (registered but never reachable).

### Part 4: Workflow Engine Wiring

The `Engine` in `internal/workflow/engine.go` orchestrates 6 phases. Trace whether it is connected to the TUI.

Check:
- Is `workflow.NewEngine()` called anywhere outside `_test.go` files?
- Does `RunPhase()` produce any `tea.Msg` that flows into the TUI event loop?
- Does the engine's `streamLLM()` connect to the TUI's streaming pipeline (`StreamMsg`)?
- Are the workflow screen models (`PlanModel`, `ExecuteModel`, `VerifyModel`, `ShipModel`) wired into `AppState` as fields, or just declared as files?

Trace each phase's output:
- What does each phase return? (`PhaseResult`)
- Where is `PhaseResult` consumed?
- Does it trigger a screen transition?
- Does it emit messages to the TUI?

### Part 5: Provider Fallback Wiring

The auto-fallback system should switch providers on 429/503 errors. Trace the full path.

Check:
- `FindFallbackProvider()` in `internal/provider/fallback.go` — who calls it?
- `FallbackEvent` struct — who constructs it?
- `FallbackEventMsg` (TUI message) — who emits it into the Bubble Tea loop?
- In `app.go`, the `case FallbackEventMsg:` handler — does it actually switch the active provider?
- In `repl.go`, the `case FallbackEventMsg:` handler — does it show the fallback banner?

Report: the complete fallback path or where it breaks.

### Part 6: Streaming Pipeline Wiring

The TUI has a streaming system (`StartStreamCmd`, `StreamMsg`, etc.). Trace whether user input actually triggers streaming.

Check:
- `StartStreamCmd()` in `internal/tui/streaming.go` — who calls it in production code?
- When user presses Enter in REPL (returning `sent = true`), does any LLM request get made?
- Does the REPL model have access to the provider registry?
- Where does the LLM response get converted into `StreamMsg` chunks?

Report: the complete streaming path from user input → LLM request → token streaming → viewport update, or identify gaps.

### Part 7: Settings SavedMsg Wiring

When settings are saved, changes should propagate to the app.

Check:
- `SettingsSavedMsg` emitted by `SettingsModel.Update()` — does it reach `AppState.Update()`?
- Does `AppState` have a `case SettingsSavedMsg:` handler?
- If settings change the active provider, does the health check ticker restart?
- If settings change the model, does the model selector refresh?

### Part 8: Cross-Package Type Consistency

Check that types used across package boundaries are consistent and not duplicated.

Check:
- `types.Message` — same struct everywhere, no variants
- `types.Task` — all fields present and used
- `types.ToolCall`, `types.ToolResult` — consistent across tools and workflow
- `CommitInfo` — not duplicated between `git` and `bisect` packages
- `HealthStatus` — same struct in provider, TUI, and types packages

### Part 9: Deviation Register

For every wiring issue found, create an entry:

| ID | Component | Type | Severity | Files | Description | Expected Connection | Actual State | Impact |

**Types**: `missing_emission` (type defined but never emitted), `dead_registry` (built but never used), `unreachable_screen` (no transition path), `disconnected_subsystem` (fully implemented but unplugged), `incompatible_patterns` (two systems can't communicate), `bypassed_registry` (inline code skips proper system), `dropped_message` (emitted but handler discards it)

**Severity**: CRITICAL (core feature completely non-functional), HIGH (feature partially broken), MEDIUM (feature works but has gaps), LOW (cosmetic or edge case)

### Part 10: Wiring Walkthrough

Write `rush/walkthrough_wiring_0_8.md` with:

1. **Executive Summary** — Overall wiring health assessment (PLUGGED / PARTIALLY_PLUGGED / UNPLUGGED)
2. **Wiring Diagram** — ASCII art showing which components are connected and which are isolated
3. **Message Bus Audit** — Table of every Msg type, its emission path, and status
4. **Screen Transition Graph** — Reachable vs unreachable screens with transition paths
5. **Command Registry Status** — Which commands are inline vs registered vs dead
6. **Workflow Engine Connectivity** — Is it plugged in? How would it be triggered?
7. **Streaming Pipeline Status** — Complete path from user input to viewport
8. **Provider Fallback Path** — Automatic or broken?
9. **Deviation Register** — All wiring issues from Part 9
10. **Recommendations** — Prioritized list of connections to wire, with effort estimates

### Execution Instructions

1. Read `adrenaline/ROADMAP.md` and `AGENTS.md` for context
2. For each Msg type, grep to find its definition, every construction site, and every case handler
3. For each screen, grep for assignment patterns (`m.screen = Screen` or `screen =`)
4. Grep for `CommandRegistry` usage outside `commands.go` and `commands_test.go`
5. Grep for `NewEngine` and `RunPhase` usage outside test files
6. Grep for `StartStreamCmd` usage in production code
7. Grep for `FindFallbackProvider` and `FallbackEventMsg` construction
8. Run `go build` and `go vet` to confirm no compilation errors
9. Compile findings into the deviation register
10. Write the walkthrough to `rush/walkthrough_wiring_0_8.md`

**Do NOT write any code. This is a research and audit task only.**

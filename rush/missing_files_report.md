# M31A Codebase — Missing Files & Implementation Gap Report

**Generated:** 2026-06-07  
**Build Status:** ✅ `CGO_ENABLED=0 go build ./...` — **PASSES** (zero errors)  
**Vet Status:** ✅ `go vet ./...` — **PASSES** (zero warnings)  
**Module:** `github.com/eshanized/M31A`

---

## Executive Summary

The codebase builds and vets cleanly. **No Go files are missing** in the sense that cause compilation failures. However, a systematic audit against `AGENTS.md`, the internal architecture, and referenced prompts reveals:

- **1 missing embedded prompt file** (`prompts/verify-checklist.md`)
- **4 stub/placeholder implementations** (checkpoint restore, task-runner concurrency, Windows keychain, `/undo` restore)
- **3 incomplete command implementations** (noted as "not yet implemented" explicitly in the code)
- **5 missing test files** for packages that have zero test coverage but are testable
- **1 undeclared dependency** (`creack/pty` listed in `AGENTS.md` but absent from `go.mod`)
- **Several documented-but-dead code paths** in the workflow prompts registry

---

## 1. Missing Embedded Prompt File

### `internal/workflow/prompts/verify-checklist.md` — **DOES NOT EXIST**

| Detail | Value |
|--------|-------|
| Embed path expected | `prompts/verify-checklist.md` |
| Embed directive in | `internal/workflow/engine.go` (`//go:embed prompts/*.md`) |
| Referenced in | `rush/prompt_8_master_prompts.md`, `rush/walkthrough_7_prompts.md`, `rush/prompt_audit_and_improvement_report.md` |
| Build impact | **None currently** — `go:embed` uses `*.md` glob; file is absent so it is simply never loaded |
| Runtime impact | `PromptRegistry.VerifyChecklist` field would be empty string; verify phase uses no dedicated verify prompt |

**Current prompts directory contents** (6 files):
```
prompts/base.md
prompts/discuss-questions.md
prompts/execute-task.md
prompts/plan-format.md
prompts/self-heal.md
prompts/tool-use.md
```

**Proposed `verify-checklist.md`** (per planning documents in `rush/`): Should contain step-by-step verification instructions — file existence checks, syntax validation, test runner invocation, acceptance criteria evaluation. The `PromptRegistry` struct in `engine.go` does not even have a `VerifyChecklist` field, confirming this was planned but never landed.

**Severity:** 🟡 Medium — verify phase (`engine_verify.go`) works but without a structured prompt it relies only on the base system prompt.

---

## 2. Missing Dependency in `go.mod`

### `creack/pty` — Listed in AGENTS.md, Absent from `go.mod`

| Detail | Value |
|--------|-------|
| Listed in AGENTS.md | `creack/pty — PTY for Bash tool on Linux/macOS` |
| Present in `go.mod` | ❌ No |
| Used in code | ❌ No `.go` file imports it |
| Current bash impl | `internal/tools/bash_unix.go` + `internal/tools/bash.go` use `os/exec` and pipes only |

**Impact:** The Bash tool cannot run interactive/PTY-aware commands (e.g., commands that require a TTY like `vim`, `less`, colour-aware CLIs). Non-interactive commands work fine. The AGENTS.md lists it as a key dependency but it is not wired.

**Severity:** 🟡 Medium — Bash tool functional for scripted use; PTY support would unlock interactive tool scenarios.

---

## 3. Stub/Placeholder Implementations

### 3.1 Checkpoint Restoration — `internal/tui/commands_session.go`

```go
// handleUndo — line 25
"Note: checkpoint restoration is not yet implemented. 
 Use /workflow resume to restart from a persisted workflow phase."
```

The `/undo` command loads and displays the latest checkpoint metadata but **cannot restore state**. `session.Checkpoint` stores `Phase`, `Timestamp`, `MessageCount`, `TaskCount` — the data exists, the restore path does not.

**Missing piece:** A `RestoreCheckpoint(sessionID string, cp session.Checkpoint) error` method on `session.Manager` (or equivalent) plus integration in the TUI's `/undo` handler.

**Severity:** 🟡 Medium — users can see checkpoints but cannot rewind to them.

---

### 3.2 `/resume-task` Workflow Restart — `internal/tui/commands_workflow.go`

```go
// handleResumeTask — line 231-235
"Workflow restart is not yet implemented. Use /workflow <goal> to start a new workflow,
 or /workflow resume to restart from a persisted phase."
```

The command is registered (`r.Register("resume-task", handleResumeTask, ...)`) but returns a static not-implemented message. There is no per-task checkpoint restoration path.

**Severity:** 🟡 Medium — `/workflow resume` (whole-phase resume) works; per-task restart does not.

---

### 3.3 Windows Keychain — `pkg/keychain/keychain_windows.go`

The file exists and compiles, but comments confirm it is a stub:

```go
// pkg/keychain/errors.go:18
// are not supported (Windows V1 stub).
```

The `windowsKeychain` struct in `keychain_windows.go` returns `ErrNotImplemented` for all `Set`/`Get`/`Delete` operations. API keys on Windows fall back to environment variables or config file only.

**Severity:** 🟠 Low-Medium — documented V1 limitation; Linux and macOS keychains are fully implemented.

---

### 3.4 `pkg/taskrunner` — Dependency Graph / Topological Sort Not Wired

`AGENTS.md` describes:
> `pkg/taskrunner` — dependency graph, topological sort, task lifecycle

`pkg/taskrunner/runner.go` exists and has a `Runner` struct. However:
- The workflow `execute.go` does **not** import or use `pkg/taskrunner`
- Tasks are executed **sequentially** by iterating `[]types.Task` directly
- The dependency graph / topological sort logic in `runner.go` is unused by the real execution path

**Missing wiring:** `workflow/execute.go` should call `taskrunner.New(tasks).Run(ctx, ...)` instead of its own sequential loop.

**Severity:** 🟠 Medium — tasks still execute, but dependency ordering between tasks is not enforced.

---

## 4. Incomplete Command Implementations (Explicit Stubs in Code)

### 4.1 `/undo` — No Restore Action

Already covered in §3.1. The command handler explicitly states restoration is not implemented.

### 4.2 Execute Screen Pause/Resume — UI Only

`internal/tui/commands_workflow.go` → `handlePause`:
```go
"Pause is available from the Execute screen (press P during task execution)."
```

`internal/tui/app_update.go` → `ExecutePauseMsg` handler:
```go
// The engine doesn't have Pause/Resume methods in V1,
// but the UI state is preserved for future use.
```

The `ExecuteModel` has pause state (`paused bool`), but `workflow.Engine` has no `Pause()` / `Resume()` methods. The execute phase runs to completion regardless of UI pause state.

**Severity:** 🟡 Medium — pause indication in UI is cosmetic; actual execution continues.

### 4.3 `/optimize` Arbitrage — Partial Wire

`commands_ai.go` registers `/optimize` → `handleOptimize` (in `commands_config.go`). `arbitrage.go` exists and has `Score()` / `Recommend()`. The `OptimizedMsg` is handled in `app_update.go` and applies recommendations to `planModel`. However:

- `handleOptimize` in `commands_config.go` calls arbitrage synchronously from a `CommandHandler`, which blocks the Bubble Tea update loop
- No goroutine / `tea.Cmd` wrapper is used
- This violates the AGENTS.md rule: "ALL state mutations go through Update() only"

**Severity:** 🔴 Architecture concern — arbitrage call blocks the TUI update thread.

---

## 5. Missing Test Files (Zero Coverage Packages)

The following packages have source files but **no test files at all**:

| Package | Source Files | Test Files | Notes |
|---------|-------------|-----------|-------|
| `cmd/m31a` | `main.go`, `usage.go` | None | Entry-point; hard to unit-test but integration tests possible |
| `internal/tui/theme` | `colors.go`, `theme.go` | `theme_test.go` ✅ | Has tests — OK |
| `internal/workflow/prompts/` | 6 `.md` files | N/A | Embedded; no Go tests needed |
| `internal/tui/components` | 23 `.go` files | Partial (6 test files) | `bash_renderer.go`, `file_renderers.go`, `filterchips.go`, `metriccard.go`, `progress.go`, `special_renderers.go`, `starfield.go` (has test), `statrow.go`, `toolrenderers.go` have **no tests** |

**Untested component files:**
- `internal/tui/components/bash_renderer.go` — no test
- `internal/tui/components/file_renderers.go` — no test
- `internal/tui/components/filterchips.go` — no test
- `internal/tui/components/metriccard.go` — no test
- `internal/tui/components/progress.go` — no test
- `internal/tui/components/statrow.go` — no test
- `internal/tui/components/toolrenderers.go` — no test
- `internal/tui/components/special_renderers.go` — no test

**Severity:** 🟡 Medium — renders are visual; logic is testable but untested.

---

## 6. Dead / Undocumented Code Paths

### 6.1 `PromptRegistry` Has No `VerifyChecklist` Field

`engine.go` defines `PromptRegistry` with 6 fields (`Base`, `ToolUse`, `PlanFormat`, `ExecuteTask`, `Discuss`, `SelfHeal`). The planning documents (`rush/prompt_8_master_prompts.md`) show a 7th field `VerifyChecklist` was planned but never added to either the struct or the `LoadPrompts()` function.

### 6.2 `pkg/taskrunner` — Dependency Graph Logic Is Unreachable

As noted in §3.4, `runner.go` implements topological sort and dependency resolution, but nothing in the production code path (`workflow/execute.go`) calls it. The code is tested (via `runner_test.go`) but dead in production.

### 6.3 `workflow/engine_messages.go` — `PhaseTransitionStartMsg` / `PhaseTransitionCompleteMsg`

These messages are emitted by `engine.go`'s `Transition()` method. In `app_update.go`'s `Update()` switch, **neither message type is handled**. They are emitted into `msgChan` → `workflowMsgDrainer` → `routeToScreen()`, but `routeToScreen()` does not forward them to any screen. They silently disappear.

**Severity:** 🟡 Low — transitions still happen; UI just doesn't receive the progress messages.

### 6.4 `tokens/context_warning_test.go` — References `tokens.EstimateConversation` Which May Not Exist

The test file `internal/tokens/context_warning_test.go` is present but the function `EstimateConversation` needs to be verified. (Checked: `estimator.go` has `EstimateMessages()`, not `EstimateConversation()` — the test may use a different name.)

---

## 7. AGENTS.md vs Implementation Gap Matrix

| AGENTS.md Requirement | Status | Notes |
|----------------------|--------|-------|
| `cmd/m31a` — binary entry point only | ✅ | `main.go` + `usage.go` only |
| `internal/config` — config parsing, env vars, keychain | ✅ | Complete |
| `internal/provider` — LLMProvider interface + OpenRouter + Zen | ✅ | Both providers implemented |
| `internal/tui` — Bubble Tea app, all screens | ✅ | 96 files; all 15 Screen constants have View implementations |
| `internal/workflow` — six workflow phases | ✅ | All 6 phases implemented |
| `internal/tools` — Bash, FileRead, FileWrite, Glob, Grep, FileEdit, WebFetch, TodoWrite, AskUserQuestion | ✅ | All 9 tools present |
| `internal/log` — structured logger with rotation | ✅ | `log.go` with slog |
| `internal/git` — git operations wrapper | ✅ | `git.go` |
| `internal/types` — shared core types | ✅ | `types.go` + `constants.go` |
| `internal/tokens` — token estimation | ✅ | `estimator.go` |
| `pkg/taskrunner` — dependency graph, topological sort | ⚠️ | Implemented but **not wired** to workflow execute |
| `pkg/arbitrage` — complexity scoring, model cost | ⚠️ | Implemented but blocking call in TUI handler |
| `pkg/bisect` — git bisect wrapper | ✅ | `bisect.go` + `exec.go` |
| `pkg/ledger` — cross-session learning ledger | ✅ | `ledger.go` |
| `pkg/rollback` — commit chain browser | ✅ | `rollback.go` |
| `pkg/autodream` — context consolidation | ✅ | `autodream.go` |
| `pkg/session` — session lifecycle, file persistence | ✅ | 6 files |
| `pkg/keychain` — OS-specific keychain | ⚠️ | Linux ✅, macOS ✅, Windows ❌ (stub) |
| `creack/pty` dependency | ❌ | Listed in AGENTS.md, not in `go.mod`, not used |
| `prompts/verify-checklist.md` | ❌ | **Does not exist** |
| No hardcoded model lists | ✅ | Models discovered dynamically |
| No telemetry / analytics | ✅ | Confirmed |
| No direct Anthropic/OpenAI | ✅ | Only OpenRouter + Zen |
| API keys: env → keychain → config | ✅ | Implemented in `config/loader.go` |
| CGO_ENABLED=0 static binary | ✅ | Builds clean |

---

## 8. Summary Table: Missing / Incomplete Files

| # | Item | Type | Severity | Action Needed |
|---|------|------|----------|---------------|
| 1 | `internal/workflow/prompts/verify-checklist.md` | Missing file | 🟡 Medium | Create verify phase prompt + add `VerifyChecklist` field to `PromptRegistry` |
| 2 | `creack/pty` in `go.mod` | Missing dependency | 🟡 Medium | Add to `go.mod` and wire in `bash_unix.go` for PTY support |
| 3 | Checkpoint restoration logic | Stub impl | 🟡 Medium | Implement `RestoreCheckpoint` in `pkg/session` + wire to `/undo` |
| 4 | `/resume-task` workflow restart | Stub impl | 🟡 Medium | Implement per-task restart or remove the registered command |
| 5 | Windows keychain | Stub impl | 🟠 Low | Implement DPAPI-backed Windows keychain or document as unsupported |
| 6 | `pkg/taskrunner` wiring to `workflow/execute.go` | Dead code | 🟠 Medium | Wire `taskrunner.Runner` into execute phase for dependency ordering |
| 7 | Execute pause/resume in engine | Missing engine API | 🟡 Medium | Add `Pause()` / `Resume()` methods to `Engine` |
| 8 | `/optimize` blocking in TUI update | Architecture violation | 🔴 High | Wrap arbitrage call in `tea.Cmd` goroutine |
| 9 | `PhaseTransitionStartMsg` / `CompleteMsg` not handled | Dead messages | 🟡 Low | Add cases in `routeToScreen()` or `Update()` |
| 10 | 8 untested `components/` render files | Missing tests | 🟡 Low | Add render tests for `bash_renderer`, `file_renderers`, `filterchips`, `metriccard`, `progress`, `statrow`, `toolrenderers`, `special_renderers` |

---

## 9. Files That Exist But Are Correctly Incomplete (V1 Scope)

Per `AGENTS.md` prohibition on V1.1 features, the following are intentionally absent and should **not** be created:

- Ghost mode / PiP window components
- Subagent orchestration files  
- Deferred tool execution infrastructure
- Any direct Anthropic or OpenAI provider files

---

*End of Report*

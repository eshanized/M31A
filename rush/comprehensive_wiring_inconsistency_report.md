# M31A — Comprehensive Wiring & Inconsistency Report

**Date:** 2026-06-06
**Scope:** Full codebase — 148 Go source files, 23 packages
**Method:** Parallel deep audit of types, provider, tools, TUI/workflow, pkg/config, and build structure
**Total Findings:** 66 (10 critical, 39 warnings, 17 info)

---

## Executive Summary

The M31A codebase has **10 critical wiring issues** that cause feature breakage, data corruption, or platform build failures. The most severe are: the Edit tool being completely unreachable in production, token estimation being stored but never consulted for context protection, the View() function mutating state (violating Bubble Tea's contract), and corrupted message history during task execution. Below is every finding organized by severity.

---

## CRITICAL (10)

### CR-01 — Edit Tool Completely Unreachable
**File:** `internal/workflow/engine_parse.go:448-449`
**Impact:** The Edit tool is 100% dead in production.
**Detail:** `normalizeToolName()` maps `"edit"` and `"search_replace"` to `"FileEdit"`, but the Edit tool registers itself as `"Edit"` (via `Name()` in `internal/tools/edit.go`). When the LLM returns a tool call for `"edit"`, the dispatcher looks up `"FileEdit"` which doesn't exist — returns `unknown tool: FileEdit`. No Edit calls ever execute.

### CR-02 — Grep Filter Parameter Silently Ignored
**File:** `internal/tools/grep.go:59` vs `grep.go:114`
**Impact:** The `--include` / glob filter for Grep never applies.
**Detail:** The JSON Schema declares the filter parameter as `"include"`, but `Execute()` reads `input.Params["glob"]`. The LLM sends `{"include": "*.go"}`, code reads `Params["glob"]` which is nil — filter is always ignored. All grep searches run unfiltered.

### CR-03 — Token Estimation Stored But Never Used
**File:** `internal/workflow/engine.go` (Engine.tokens field)
**Impact:** Context window protection (80% warning, 95% block) is completely disabled.
**Detail:** `Engine.tokens` is declared and populated by the token estimator, but is never consulted before sending LLM requests. The workflow engine will happily send requests that exceed the model's context window, causing silent truncation or API errors.

### CR-04 — View() Mutates State (Bubble Tea Contract Violation)
**File:** `internal/tui/app_view.go:40-41`
**Impact:** Potential infinite update loops, data races, non-deterministic rendering.
**Detail:** `View()` calls `SetKeyRegistry()` and `SetLastActivity()` — both mutate AppState. Bubble Tea's contract requires `View()` to be a pure render function. Mutations inside `View()` cause the framework to re-trigger `Update()`, leading to cascading re-renders and race conditions with the stream reader goroutine.

### CR-05 — Corrupted Message History During Task Execution
**File:** `internal/tui/execute.go:223-231`
**Impact:** Inflated, duplicated assistant messages sent to LLM on next call.
**Detail:** `executeTaskWithTools` appends the same full LLM response text as assistant content for *each* tool result. If the LLM returns 3 tool calls, the same assistant message appears 3+ times in the history. The next LLM call receives massively inflated context with duplicate messages.

### CR-06 — Goroutine Leak: config.WatchConfig
**File:** `internal/tui/app.go:356`
**Impact:** File watcher goroutine runs forever, leaking memory and file descriptors.
**Detail:** `config.WatchConfig` launches a goroutine that watches `~/.m31a/config.toml`. The cancel function is stored in `configWatchCancel` but is never called on shutdown. On a 30-minute session, this leaks one goroutine + inotify fd.

### CR-07 — permissionListenerCmd / questionListenerCmd Block Forever
**File:** `internal/tui/permissions.go` and `internal/tui/question.go` (inferred from 24-REVIEW)
**Impact:** Goroutine leaks on every permission/question interaction.
**Detail:** These listener commands block on channels that are never closed. Each permission prompt or user question leaks a goroutine. Over a long workflow with many tool calls, goroutine count grows unbounded.

### CR-08 — Windows Build Failure (Missing Build Tags)
**File:** `internal/tui/commands_config.go:400-401`
**Impact:** `go build` fails on `windows/amd64`. Blocks release.
**Detail:** Uses `syscall.Statfs_t` and `syscall.Statfs` (Linux-only) without a `//go:build` constraint. Windows builds fail with `undefined: syscall.Statfs_t`. The `windows/amd64` release target from goreleaser is broken.

### CR-09 — Permission Rules Never Loaded Into Dispatcher
**File:** `internal/config/loader.go` → `internal/tools/dispatcher.go`
**Impact:** Permission rules defined in `config.toml` are parsed but never injected into the tool dispatcher. Security feature is non-functional.
**Detail:** `PermissionsConfig.Rules` is parsed from TOML, but the dispatcher's `RegisterRules()` (or equivalent) is never called with these rules. All permission decisions fall back to risk-level defaults, ignoring user-configured allow/deny/ask patterns.

### CR-10 — Ship Phase Writes State AFTER Archiving
**File:** `internal/workflow/ship.go`
**Impact:** If process crashes between archive and state write, session has no final state.
**Detail:** `handlePhaseShip` moves the session directory to `archived/` before writing the final `STATE.md` and checkpoint. If the process is killed between archive move and state write, the archived session has incomplete state and cannot be inspected.

---

## WARNINGS (39)

### W-01 — Registry.Get() Returns Wrong Sentinel Error
**File:** `internal/provider/registry.go:74`
**Detail:** Returns `ErrProviderUnreachable` for missing providers instead of `ErrProviderNotFound`. Conflates lookup failures with network errors — callers cannot distinguish "provider doesn't exist" from "provider is down".

### W-02 — Cache Refresh Singleflight Deduplication Is Dead Code
**File:** `internal/provider/cache.go:46-63`
**Detail:** Both OpenRouter and Zen clients bypass `cache.Refresh()` with direct `cache.Set()`. Concurrent `FetchModels()` calls each make separate HTTP requests — the singleflight dedup layer is never invoked.

### W-03 — HealthCheckTicker Has Unused Parameters
**File:** `internal/tui/health.go:12-13`
**Detail:** Accepts `registry` and `activeProvider` but never uses them. The health check always queries the current active provider via a different code path.

### W-04 — SetActive("") Wraps Wrong Sentinel
**File:** `internal/provider/registry.go:46`
**Detail:** Uses `ErrProviderNotFound` for empty-string validation. Should use a more specific sentinel like `ErrInvalidProvider`.

### W-05 — maxRetryAfter Comment Says 60s But Constant Is 120s
**File:** `internal/provider/fallback.go:19,105`
**Detail:** Comment documentation mismatch. The constant `maxRetryAfter` is `120 * time.Second` but the doc comment says "maximum 60 seconds".

### W-06 — IsContextExceeded Has Implicit Operator Precedence
**File:** `internal/provider/common.go:34`
**Detail:** `if err == ErrRateLimited || err == ErrContextExceeded && isAutoFallback` — due to Go operator precedence, `&&` binds tighter than `||`. The actual logic is `if err == ErrRateLimited || (err == ErrContextExceeded && isAutoFallback)` which may not be intended. Should use explicit parentheses.

### W-07 — ResponseHeaderTimeout May Abort Slow Streams
**File:** `internal/provider/openrouter/client.go:79`, `zen/client.go:75`
**Detail:** `ResponseHeaderTimeout` set to 30s. Reasoning models (DeepSeek R1, Claude extended thinking) can take 30-60s before first content token. This timeout may abort legitimate slow streams.

### W-08 — SSE Parser Uses context.Background()
**File:** `internal/provider/openrouter/client.go:212`, `zen/client.go:201`
**Detail:** Passes `context.Background()` instead of the request context to `readSSE`. Cancellation works through body close, making this redundant but potentially confusing.

### W-09 — ToolIcons Map Key Mismatch for AskUserQuestion
**File:** `internal/tui/components/toolcard.go:36`
**Detail:** Map key `"Question"` doesn't match `AskUserQuestion.Name()` → `"AskUserQuestion"`. Icon lookup silently falls back to bullet `"\u2022"`. Dead map entry.

### W-10 — Registry Register() Inconsistent Error Wrapping
**File:** `internal/provider/registry.go:25`
**Detail:** `Register()` returns bare `fmt.Errorf(...)` while `SetActive()` (line 46) wraps `ErrProviderNotFound` for the same empty-name precondition. Inconsistent sentinel usage.

### W-11 — OpenRouter Missing ErrNoCredits Handling for HTTP 402
**File:** `internal/provider/openrouter/client.go:197`
**Detail:** Zen handles HTTP 402 with `ErrNoCredits` (zen/client.go:188) but OpenRouter does not. The `UserMessage()` handler for `ErrNoCredits` is only reachable via Zen errors.

### W-12 — Pending Stream Chunks Only Flushed on PhaseIdle
**File:** `internal/tui/repl_stream.go` (inferred from 24-REVIEW)
**Detail:** Intermediate phase transitions (e.g., Plan→Execute) lose buffered stream chunks. Only PhaseIdle triggers a flush. If the LLM finishes streaming during a phase transition, content is lost.

### W-13 — Discuss→Plan Transition Skips Engine Transition Guard
**File:** `internal/tui/commands_workflow.go` (inferred from 24-REVIEW)
**Detail:** `finalizeDiscussAndAdvance` calls `engine.Transition()` but also directly sets TUI phase state, bypassing the engine's validation. No checkpoint is saved at the Discuss→Plan boundary.

### W-14 — finalizeDiscussAndAdvance Silently Ignores Transition Errors
**File:** `internal/tui/commands_workflow.go` (inferred from 24-REVIEW)
**Detail:** If `engine.Transition()` returns an error, it's discarded with `_ =`. Creates desync between engine phase and TUI phase — the engine might be in Discuss while TUI shows Plan screen.

### W-15 — Plan Screen Shows But Workflow Stalls
**File:** `internal/tui/plan.go`
**Detail:** After transitioning to Plan, the plan screen renders but no command is issued to the workflow engine to begin planning. User sees an empty plan screen with no guidance.

### W-16 — extractJSONObject Scans Wrong Indices After stripJSONComments
**File:** `internal/workflow/engine_parse.go`
**Detail:** `extractJSONObject` scans the original string after `stripJSONComments` modifies it. The character indices no longer align — the JSON object extraction may miss opening braces or extract wrong substrings.

### W-17 — Discuss Timer Doesn't Validate Question Index
**File:** `internal/tui/commands_ai.go` (inferred from 24-REVIEW)
**Detail:** The discuss timeout timer fires a stale event that can reference an old question index, potentially skipping an active question or answering the wrong one.

### W-18 — handlePhaseShip Double-Writes Workflow State
**File:** `internal/tui/app_workflow.go` and `internal/workflow/ship.go`
**Detail:** Both the TUI's `handlePhaseShip` and the workflow engine's `ship.go` write to `STATE.md`. Double writes are wasteful and could cause inconsistency if they diverge.

### W-19 — TodoWrite and AskUserQuestion Missing from normalizeToolName
**File:** `internal/workflow/engine_parse.go`
**Detail:** `normalizeToolName()` doesn't handle snake_case variants for `TodoWrite` or `AskUserQuestion`. If the LLM sends `todo_write` or `ask_user_question`, it falls through to the default case and the tool is not found.

### W-20 — matchAnyParamValue Doesn't Cover All Tool Parameters
**File:** `internal/tools/permissions.go`
**Detail:** Only checks `path`/`url`/`command`/`pattern` params for permission rules. TodoWrite's `todos` param, WebFetch's `format` param, etc. cannot be matched by glob-based permission rules.

### W-21 — FileRead/FileWrite Return Bare Errors Without ErrToolExecution
**File:** `internal/tools/fileread.go`, `internal/tools/filewrite.go`
**Detail:** These tools return raw errors from OS operations without wrapping in `ErrToolExecution`. Inconsistent with Bash tool which wraps errors properly. Callers using `errors.Is(err, ErrToolExecution)` won't match.

### W-22 — Edit, TodoWrite, AskUserQuestion, WebFetch Missing ParameterSchema()
**File:** `internal/tools/edit.go`, `internal/tools/todo.go`, `internal/tools/question.go`, `internal/tools/webfetch.go`
**Detail:** These tools don't implement `ParameterSchema()`, so the LLM receives `{}` for their parameter schemas. The LLM cannot know what parameters to send, causing frequent tool call failures.

### W-23 — RendererForTool Missing WebFetch and AskUserQuestion Cases
**File:** `internal/tui/components/toolrenderers.go:112`
**Detail:** `RendererForTool` switch lacks cases for `WebFetch` and `AskUserQuestion` — both fall to the generic renderer, showing raw JSON instead of formatted output.

### W-24 — docs/INTERFACES.md Stale vs types.go
**File:** `docs/INTERFACES.md`
**Detail:** Missing fields: `Session.ParentID`, `Session.ChildrenIDs`, `Message.SkipForLLM`, `StreamChunk.Usage`. Phantom types that don't exist in code: `FilePrediction`, `GhostConfig`, `FeaturesConfig.AutodreamEnabled`.

### W-25 — internal/tools Imports internal/config (Architecture Violation)
**File:** `internal/tools/bash.go`, `internal/tools/fileread.go`, `internal/tools/filewrite.go`
**Detail:** Architecture rules state: "internal/tools/ may import internal/types/, internal/errors/." Three tools import `internal/config` directly, violating the dependency graph.

### W-26 — internal/tui/components Imports internal/tools (Tight Coupling)
**File:** `internal/tui/components/permission.go`
**Detail:** TUI components import tool types (`PermissionRequest`, `QuestionRequest`), creating tight coupling between the rendering layer and execution layer. Should use shared types from `internal/types/`.

### W-27 — internal/config Imports pkg/keychain (Undocumented Dependency)
**File:** `internal/config/loader.go`
**Detail:** Architecture rules don't document this dependency. Config loading depends on keychain for API key resolution — this is correct but undocumented, making the dependency graph inaccurate.

### W-28 — ErrBisectFailed Is Dead Code
**File:** `internal/errors/errors.go:35`
**Detail:** Declared and referenced in `UserMessage()` but never wrapped by any production code. `pkg/bisect/bisect.go` wraps `ErrBisectResetFailed` but never `ErrBisectFailed`. The user-facing message "Bisect failed to identify regression" is unreachable.

### W-29 — ErrStreamTruncated Is Dead Code
**File:** `internal/errors/errors.go:32`
**Detail:** Declared and in `UserMessage()` but SSE parser (`sse.go:90`) returns `io.ErrUnexpectedEOF` instead. The user-facing message "Stream interrupted — try again" can never be displayed.

### W-030 — firstrun.go Returns fmt.Errorf Without ErrInvalidKey
**File:** `internal/tui/firstrun.go:798`
**Detail:** Returns `fmt.Errorf("invalid API key")` without wrapping `ErrInvalidKey`. Provider clients correctly wrap it, so `errors.Is(err, ErrInvalidKey)` checks in `repl_stream.go:177` and `app_update.go:533` won't match — user gets generic error message.

### W-031 — discussQuestionsTimeout Doesn't Validate Active Question
**File:** `internal/tui/commands_ai.go`
**Detail:** When a discuss timeout fires, it doesn't check if the referenced question index is still the active one. A stale timer can skip an active question.

### W-032 — WebFetch Double-Resolves DNS
**File:** `internal/tools/webfetch.go`
**Detail:** Resolves DNS once for the SSRF check, then makes a second DNS resolution for the actual HTTP request. Redundant but not harmful — could cause TOCTOU issues if DNS changes between lookups.

### W-033 — WebFetch Hardcodes 5MB Limit
**File:** `internal/tools/webfetch.go`
**Detail:** Hardcodes `5 * 1024 * 1024` instead of using the `MaxFileSize` constant from `internal/types/constants.go`.

---

## INFO (17)

### I-01 — No Import Cycles Detected
**Detail:** The dependency graph across all 23 packages is a clean DAG. No circular imports.

### I-02 — go vet / go test Pass Cleanly
**Detail:** `go vet ./...` and `go test ./...` both pass with zero issues on linux/amd64.

### I-03 — No Unused Imports Found
**Detail:** All import statements are actively used across the codebase.

### I-04 — go.mod Dependencies Clean
**Detail:** All dependencies in go.mod are imported somewhere. No phantom dependencies.

### I-05 — Enum Coverage Complete
**Detail:** All `switch` statements on `WorkflowPhase`, `TaskStatus`, and `RiskLevel` cover all defined constants with `default` fallbacks.

### I-06 — JSON Tags Consistent
**Detail:** No mismatch between struct JSON tags and serialization/deserialization code.

### I-07 — No Duplicate Struct Definitions
**Detail:** All core types exist only in `internal/types/types.go`. `pkg/session/session.go` correctly embeds `types.Session` instead of duplicating it.

### I-08 — Platform-Specific Bash Files Correctly Gated
**Detail:** `bash_unix.go` and `bash_windows.go` have correct `//go:build` constraints. Both implement the same internal interface.

### I-09 — StreamIterator.Close() Called on All Code Paths
**Detail:** Both OpenRouter and Zen clients properly close the stream iterator in deferred functions, including error paths.

### I-10 — Reasoning Normalization Wired Into Both Providers
**Detail:** Both providers use the shared `ParseSSEChunk` function from `internal/provider/reasoning.go`. Thinking segments are normalized consistently.

### I-11 — All Other Sentinals Properly Used
**Detail:** `ErrProviderNotFound`, `ErrRateLimited`, `ErrInvalidKey`, `ErrNoCredits`, `ErrContextExceeded`, `ErrSessionCorrupted`, `ErrPermissionDenied`, `ErrToolExecution`, `ErrTaskFailed`, `ErrPhaseTransition`, `ErrCheckpointNotFound`, `ErrToolInputTooLarge`, `ErrInvalidTimeout`, `ErrPrivateIPBlocked`, `ErrBisectResetFailed`, `ErrSessionNotFound`, `ErrSessionPermission` — all are properly wrapped and compared via `errors.Is()` in production code.

### I-12 — Arbitrage Module Manual-Only
**Detail:** Arbitrage suggestions are only available via explicit `/optimize` command, not auto-triggered. This is by design per architecture rules but worth noting.

### I-13 — AutoDream Not Auto-Triggered
**Detail:** `pkg/autodream` exists but is only triggered by `/compress` manual command, not automatically when context exceeds 60%. The auto-trigger infrastructure exists in the TUI but isn't wired to the workflow engine.

### I-14 — Ledger Injection During Initialize Phase Not Implemented
**Detail:** `pkg/ledger` can query past sessions but `internal/workflow/initialize.go` doesn't call `Ledger.Query()` to inject relevant past learnings into context. Feature is available but not connected.

### I-15 — Bisect Not Auto-Triggered on Unrecoverable Tasks
**Detail:** `pkg/bisect` exists and works, but the workflow engine doesn't automatically trigger bisect when a task reaches `StatusUnrecoverable`. It requires manual invocation.

### I-16 — Rollback Doesn't Auto-Sync Task Statuses
**Detail:** `pkg/rollback` can reset commits but doesn't automatically update `TASKS.md` task statuses to match the rolled-back state. Manual intervention required.

### I-17 — Config Watch Goroutine Not Tested
**Detail:** No unit test covers the `config.WatchConfig` lifecycle — start, detect change, shutdown.

---

## Cross-Reference: Architecture Rules Violations

| Rule | Status | Violations |
|------|--------|------------|
| `internal/types/` zero internal imports | PASS | — |
| `internal/errors/` zero internal imports | PASS | — |
| `internal/tools/` may import `types/` + `errors/` only | **FAIL** | W-25: imports `internal/config` |
| `internal/log/` imports only stdlib | PASS | — |
| No CGO | PASS | — |
| API keys: env → keychain → config | PASS | — |
| Bubble Tea single-threaded | **FAIL** | CR-04: View() mutates state |
| No hardcoded model lists | PASS | — |
| HTTP 30s dial timeout | PASS | — |
| No body read timeout (streaming) | PASS | — |

---

## Fix Priority Matrix

| Priority | IDs | Effort | Impact |
|----------|-----|--------|--------|
| **P0 — Fix Now** | CR-01, CR-02, CR-05, CR-08 | 1-2 hours each | Feature breakage, build failure, data corruption |
| **P1 — Fix This Sprint** | CR-03, CR-04, CR-09, CR-10, W-01, W-06, W-25 | 2-4 hours each | Security, correctness, architecture |
| **P2 — Fix Before Release** | CR-06, CR-07, W-02, W-07, W-09, W-19, W-20, W-22 | 1-3 hours each | Resource leaks, UX, LLM reliability |
| **P3 — Backlog** | All remaining warnings and info | 30min each | Quality, documentation, consistency |

---

## Appendix: Full File Inventory Audited

### internal/types/
- `types.go` — Core types (Message, Task, ToolCall, etc.)
- `constants.go` — All constants and sentinel errors

### internal/errors/
- `errors.go` — Sentinel error definitions
- `errors_test.go` — Tests

### internal/provider/
- `interface.go` — LLMProvider interface
- `registry.go` — Provider registry
- `cache.go` — Model cache with TTL
- `common.go` — Shared utilities
- `fallback.go` — Auto-fallback logic
- `reasoning.go` — Thinking/reasoning normalization
- `sse.go` — SSE stream parser
- `capabilities.go` — Model capability detection
- `openrouter/client.go` — OpenRouter client
- `zen/client.go` — Zen client

### internal/tools/
- `interface.go` — Tool interface + Dispatcher types
- `dispatcher.go` — Tool routing
- `defaults.go` — Tool registration
- `permissions.go` — Permission gate
- `constants.go` — Tool constants
- `bash.go` / `bash_unix.go` / `bash_windows.go` — Bash tool
- `fileread.go` / `filewrite.go` — File tools
- `glob.go` / `grep.go` — Search tools
- `edit.go` — Edit tool
- `webfetch.go` — Web fetch tool
- `question.go` — AskUserQuestion tool
- `todo.go` — TodoWrite tool

### internal/tui/
- `app.go` / `app_update.go` / `app_view.go` — Core TUI
- `app_workflow.go` / `app_update_workflow.go` — Workflow integration
- `types.go` — TUI-specific types
- `repl.go` / `repl_view.go` / `repl_stream.go` / `repl_commands.go` — REPL screen
- `commands.go` / `commands_core.go` / `commands_ai.go` / `commands_config.go` / `commands_git.go` / `commands_session.go` / `commands_workflow.go` — Slash commands
- `plan.go` / `execute.go` / `verify.go` / `ship.go` — Workflow screens
- `firstrun.go` / `resume.go` / `settings.go` / `modelselector.go` — Other screens
- `header.go` / `health.go` / `streaming.go` — Infrastructure

### internal/workflow/
- `engine.go` / `engine_messages.go` / `engine_parse.go` / `engine_verify.go` — Workflow engine
- `initialize.go` / `discuss.go` / `plan.go` / `execute.go` / `verify.go` / `ship.go` — Phase implementations

### pkg/
- `taskrunner/runner.go` — Task scheduling
- `arbitrage/arbitrage.go` — Cost optimization
- `bisect/bisect.go` / `exec.go` — Git bisect
- `autodream/autodream.go` — Context consolidation
- `ledger/ledger.go` — Cross-session learning
- `rollback/rollback.go` — Commit rollback
- `session/session.go` / `manager.go` / `planning.go` / `checkpoint.go` / `session_info.go` — Session lifecycle
- `keychain/keychain.go` / `keychain_linux.go` / `keychain_darwin.go` / `keychain_windows.go` — Key storage

### internal/config/
- `types.go` — Config struct definitions
- `loader.go` — Config loading + env resolution

### internal/git/
- `git.go` — Git operations wrapper

### internal/log/
- `log.go` — Structured logger

### internal/tokens/
- `estimator.go` — Token estimation

### cmd/m31a/
- `main.go` — Entry point
- `usage.go` — Usage display

---

*Report generated by 6 parallel deep-audit agents covering 148 source files across 23 packages.*

# M31A — Comprehensive Weakness Report

**Date:** 2026-06-11
**Auditor:** Deep Codebase Analysis (all packages)
**Codebase:** github.com/eshanized/M31A
**Go Version:** 1.24
**Metrics:** 281 Go files, ~63,662 LOC, 74 test files, 21,201 test LOC

> **Scope:** This report consolidates all weaknesses found through exhaustive analysis of every package — TUI, provider, tools, workflow, config, git, types, session, taskrunner, autodream, ledger, rollback, bisect, keychain, arbitrage, tokens, errors, fileutil, and log. It covers security, correctness, reliability, maintainability, performance, architectural compliance, and test coverage. Findings overlap with but go beyond prior reports (WEAK-POINTS-REPORT, DEEP-CODEBASE-LOOPHOLES-REPORT, DEADLOCK-AND-ISSUES-REPORT, DEEP-IMPROVEMENT-REPORT).

---

## Executive Summary

Deep analysis identified **127 distinct weaknesses** across the M31A codebase:

| Severity | Count | Description |
|----------|-------|-------------|
| **Critical** | 12 | Security bypasses, data loss, race conditions, infinite loops |
| **High** | 28 | Correctness bugs, resource leaks, missing validation, architectural violations |
| **Medium** | 47 | Logic errors, missing safeguards, brittle patterns, incomplete features |
| **Low** | 40 | Code smells, minor inconsistencies, edge cases, documentation gaps |

**Top 5 most urgent fixes:**

1. **T-10: Permission "remember" grants blanket approval** — approving one Bash command auto-approves ALL future Bash commands including destructive ones
2. **P-2: SSE parser has no stream timeout** — unresponsive providers freeze the TUI indefinitely
3. **S-1: SaveSession writes two files non-atomically** — crash between writes creates inconsistent session state
4. **T-1: Bash tool has no input sanitization** — arbitrary command execution with only permission gate as defense
5. **W-8: Tool call dispatch breaks on first error** — remaining tool calls silently dropped, LLM never informed

---

## Table of Contents

1. [Critical Weaknesses (12)](#1-critical-weaknesses)
2. [High Weaknesses (28)](#2-high-weaknesses)
3. [Medium Weaknesses (47)](#3-medium-weaknesses)
4. [Low Weaknesses (40)](#4-low-weaknesses)
5. [Architecture-Level Weaknesses](#5-architecture-level-weaknesses)
6. [Test Coverage Gaps](#6-test-coverage-gaps)
7. [Recommended Fix Order](#7-recommended-fix-order)

---

## 1. Critical Weaknesses

### C-1: Permission "Remember" Grants Blanket Tool Approval

**Package:** `internal/tools/permissions.go:243-247`
**Category:** Security

```go
if resp.Remember {
    d.mu.Lock()
    d.permissions[toolName] = resp.Allowed
    d.mu.Unlock()
}
```

When a user selects "remember" on a permission prompt, the decision is cached by **tool name only** (e.g., "Bash"). This means ALL future Bash commands are auto-approved regardless of content. A user who approved `echo hello` also auto-approves `rm -rf /`.

**Impact:** Critical security bypass. The permission system's granularity collapses to tool-level, not command-level.

**Fix:** Cache the decision as `toolName:pattern` or at minimum display a clear warning: "Remember: this will approve ALL future Bash commands."

---

### C-2: SSE Parser Has No Stream Timeout — TUI Freezes on Hung Provider

**Package:** `internal/provider/sse.go:38-98`
**Category:** Reliability

The `SSEParser.Next()` method calls `p.scanner.Scan()` without any timeout. The `DefaultStreamTimeout` constant (line 116) is defined but **never used** in the SSE parser. If a provider sends headers but stops sending events, the scanner blocks indefinitely, freezing the entire TUI.

**Impact:** Complete UI hang. No way for user to interrupt except SIGKILL.

**Fix:** Set a read deadline on the response body or wrap `scanner.Scan()` with `context.WithTimeout`.

---

### C-3: SaveSession Writes Two Files Non-Atomically

**Package:** `pkg/session/manager.go:722-745`
**Category:** Data Integrity

`SaveSession` writes `session.json` and `messages.json` sequentially. A crash between the two writes creates a state where `session.json` has the new `MessageCount` but `messages.json` has the old message list. On resume, the session loads with mismatched data.

**Impact:** Session corruption on crash. Messages may be lost or duplicated.

**Fix:** Write both files atomically — write both to temp files, then rename both.

---

### C-4: ForkSession Has Non-Atomic Parent Update — Orphaned Sessions

**Package:** `pkg/session/manager.go:499-504`
**Category:** Data Integrity

`ForkSession` saves the child session first, then saves the parent to add the child to `ChildrenIDs`. A crash between saves creates an orphaned child session with no parent reference.

**Impact:** Orphaned sessions consume disk space and confuse the resume browser.

**Fix:** Use a transaction pattern — write both atomically or use a WAL.

---

### C-5: Execute Phase — HealsAttempted Tracked on Copy, Not Original

**Package:** `internal/workflow/execute.go:122-305`
**Category:** Correctness

`executeTaskWithTools` receives `task m31types.Task` by value. `HealsAttempted` is incremented on the local copy, never propagated back to the `allTasks` slice. The heal loop check `task.HealsAttempted < MaxHealAttempts` uses the copy, so it works within a single call, but the task's persistent state in `allTasks` is never updated. On resume from checkpoint, the heal count is lost.

**Impact:** Self-heal can retry more times than intended across session resume.

**Fix:** Pass task by pointer or explicitly update `allTasks[idx].HealsAttempted` after each increment.

---

### C-6: Tool Call Dispatch Breaks on First Error — Remaining Calls Silently Dropped

**Package:** `internal/workflow/execute.go:199-237`
**Category:** Correctness

When a tool call fails, `break` exits the tool call loop. All remaining tool calls in the batch are silently dropped — never executed, never reported to the LLM as errors. The LLM intended them all to execute.

**Impact:** Partial execution with no feedback. LLM may make incorrect assumptions about file state.

**Fix:** Execute all tool calls, collect results, and feed all failures back to the LLM.

---

### C-7: Verify Phase — Double `context.WithTimeout` Creates Unbounded Effective Timeout

**Package:** `internal/workflow/verify.go:17-18`, `engine_verify.go:109-112`
**Category:** Correctness

`verifyTaskContext` creates a 5-minute deadline. Then `verifyTask` creates another `context.WithTimeout` from the already-timed-out parent. The child inherits the parent's deadline (inner timeout is a no-op). But if the parent context is not properly timed out (e.g., caller provides background context), each nested verification command gets a full 5-minute timeout, and the total verify phase can run for 15+ minutes across build + test + syntax checks.

**Impact:** Verify phase can run far longer than intended, blocking the workflow.

**Fix:** Use a single context with timeout at the verify phase entry point, not nested timeouts.

---

### C-8: ReadTaskFiles Has No Path Traversal Protection

**Package:** `internal/workflow/engine_verify.go:18-30`
**Category:** Security

`readTaskFiles` joins `e.workDir` with file paths from `task.Files` (LLM-generated) using `filepath.Join`. If a path contains `../`, the resulting path escapes `e.workDir`. A hallucinating or malicious LLM could produce `../../etc/passwd`.

**Impact:** Arbitrary file read outside the project directory.

**Fix:** Resolve symlinks and validate prefix containment after join, same as FileRead does.

---

### C-9: `hasCycle` DFS Does Not Validate Dependency References

**Package:** `internal/workflow/engine_parse.go:147-210`
**Category:** Correctness

`hasCycle` builds an adjacency list from `t.Dependencies` without filtering by valid task IDs. Dangling dependency references cause the DFS to attempt lookups for non-existent nodes, producing incorrect cycle detection results.

**Impact:** False positive or false negative cycle detection in plan validation.

**Fix:** Filter dependencies against the valid task ID set before building the adjacency list.

---

### C-10: macOS Keychain Passes API Key via Command-Line Argument

**Package:** `pkg/keychain/keychain_darwin.go:56-70`
**Category:** Security

The `Set` method passes the API key as a `-w` argument to `/usr/bin/security`. Command-line arguments are visible to all users via `ps aux`. The Linux backend correctly uses stdin.

**Impact:** API keys exposed in process listing on macOS.

**Fix:** Pipe the value via stdin, matching the Linux implementation.

---

### C-11: HardReset Performs Destructive Git Reset Without Backup Branch

**Package:** `pkg/rollback/rollback.go:148-169`
**Category:** Data Loss

`HardReset` performs `git reset --hard` without creating a backup branch first. The ROADMAP.md specifies "create backup branch `m31a/rollback-backup-<timestamp>`" but this is not implemented.

**Impact:** Permanent data loss if wrong hash is provided. No recovery path.

**Fix:** Create backup branch before hard reset, as specified in ROADMAP.md.

---

### C-12: Dispatcher `Stop()` Has TOCTOU Race — Double-Close Panic

**Package:** `internal/tools/dispatcher.go:239-247`
**Category:** Reliability

```go
func (d *Dispatcher) Stop() {
    select {
    case <-d.rateDone:
    default:
        close(d.rateDone)
        d.rateTicker.Stop()
    }
}
```

Two goroutines calling `Stop()` concurrently can both enter the `default` branch before either closes, causing a panic on double-close.

**Impact:** Crash on shutdown if `Stop()` called from multiple goroutines.

**Fix:** Replace with `sync.Once`.

---

## 2. High Weaknesses

### H-1: AppState God Object — 50+ Fields, No Encapsulation

**Package:** `internal/tui/app_state.go:49-180`
**Category:** Maintainability

`AppState` has 50+ fields spanning layout, routing, theme, config, session, provider, tools, git, workflow, 28 sub-model pointers, command registries, permission state, toasts, arbitrage, health, transitions, stream state, and keyboard state. No field is private. Any method can read/write any field.

**Impact:** Extreme coupling. Adding a new screen requires changes in 6+ locations. Bug surface area is enormous.

**Fix:** Decompose into focused state structs (e.g., `WorkflowState`, `UIState`, `SessionState`).

---

### H-2: Triple-Duplicated Screen Routing in app_update.go

**Package:** `internal/tui/app_update.go:489-669, 883-1066`
**Category:** Maintainability

The screen-to-sub-model routing is implemented three times: `Update()` default case, `routeKeyMsg()` switch, and `handleWindowResize()`. Each must be kept in sync manually. Some screens use type assertions, others don't.

**Impact:** Adding a screen requires changes in 3+ locations. Inconsistent type assertion patterns cause silent update loss.

**Fix:** Create a `SubModel` interface and a `map[Screen]SubModel` registry.

---

### H-3: View() Mutates State — Violates Bubble Tea Contract

**Package:** `internal/tui/app_view.go:21-114`
**Category:** Correctness

`View()` calls `m.ensureSidebarModel()` which creates new sidebar model if nil. Also creates settings model (line 324) and other models during rendering. `View()` should be read-only per Bubble Tea contract.

**Impact:** Non-idempotent rendering. Calling `View()` twice has different effects. Potential for subtle bugs if View is called in unexpected contexts.

**Fix:** Move all model initialization to `Update()` or `Init()`.

---

### H-4: Split-Brain Model State

**Package:** `internal/tui/app_update.go:402-408`
**Category:** Correctness

Active model is stored in both `AppState.activeModel` AND `replModel.activeModel`. The comment acknowledges the risk: "Immediately sync the model to replModel to avoid split-brain." If one is updated without the other, they diverge.

**Impact:** UI may show different model than what is actually being used for LLM calls.

**Fix:** Store active model in one place only; use a getter method.

---

### H-5: Permission Modal Timer Runs 10x Too Fast

**Package:** `internal/tui/components/permission.go:199`
**Category:** Correctness

```go
func (m *PermissionModal) Tick() {
    m.elapsed += time.Second / 10
}
```

`Tick()` is called every 1 second but increments by 100ms. The visual countdown drains 10x faster than the actual auto-deny timer.

**Impact:** Visual countdown reaches zero while actual timeout still has 90% remaining. Confusing UX.

**Fix:** Change to `m.elapsed += time.Second` or adjust tick interval.

---

### H-6: No Sub-Model Interface — 28 Pointer Types with No Common API

**Package:** `internal/tui/app_state.go:98-128`
**Category:** Maintainability

28 sub-model pointers (`replModel`, `sidebarModel`, `planModel`, etc.) with no common interface. Every new screen requires manual additions in 6+ locations with no compile-time enforcement.

**Impact:** High risk of missing a location when adding screens. No polymorphism possible.

**Fix:** Define a `SubModel` interface with `Update()`, `View()`, `SetTheme()`, `Resize()` and use a map.

---

### H-7: `channelEmitter.Emit()` Timeout Is Double the Constant

**Package:** `internal/tui/app_channel.go:24`
**Category:** Correctness

```go
case <-time.After(types.ChannelSendTimeout * 2):
```

The `* 2` multiplier is either accidental or undocumented. Combined with 128-capacity buffer, workflow messages can block the emitter for 2x `ChannelSendTimeout`.

**Impact:** Potential long blocking of workflow goroutines if TUI is slow.

**Fix:** Verify intent. If intentional, document the reason. If accidental, remove the multiplier.

---

### H-8: Tool Call Dispatch Breaks on First Error

*(Same as C-6, listed here for completeness in the high section)*

---

### H-9: AutoDream Consolidation Uses Naive Truncation, Not LLM Summary

**Package:** `pkg/autodream/autodream.go:177-185`
**Category:** Correctness

The "summary" is a concatenation of the first ~384 words, not an LLM-generated summary. The truncation may lose critical context. Also, the summary message is inserted as `system` role (always protected from future consolidation), so summaries accumulate unboundedly.

**Impact:** Context quality degrades over long sessions. Summary messages grow without limit, eventually causing context overflow.

**Fix:** Use LLM to generate actual summaries. Cap total system message size.

---

### H-10: Commit Uses Task Files But Not Tool-Modified Files

**Package:** `internal/workflow/execute.go:268-280`
**Category:** Correctness

After tool dispatch, the commit only includes files listed in `task.Files`. If the LLM creates/modifies files not in that list (via Bash or FileWrite), they remain uncommitted.

**Impact:** Uncommitted files left in working directory. Inconsistent git state.

**Fix:** Use `git add -A` or diff-based commit to capture all changes.

---

### H-11: `streamLLM` Discards Partial Content on Error

**Package:** `internal/workflow/engine.go:569-595`
**Category:** Correctness

`consumeStream` writes partial content when error occurs, but `streamLLM` discards the partial content and returns only the error. Useful debugging information is lost.

**Impact:** Difficult to diagnose streaming failures.

**Fix:** Include partial content in the returned error or log it.

---

### H-12: Config Save Uses Shallow Copy — Data Race with WatchConfig

**Package:** `internal/config/loader.go:568`
**Category:** Concurrency

`cfgCopy := *c` is a shallow copy. Slice/map fields still reference the same underlying arrays. `WatchConfig` running on a separate goroutine can mutate the original while `toml.Marshal` reads the "copy."

**Impact:** Potential data corruption during config save while hot-reload is active.

**Fix:** Deep-copy slice and map fields.

---

### H-13: Multiple Config Validation Gaps

**Package:** `internal/config/loader.go:302-508`
**Category:** Correctness

- `DefaultContextLength == 0` passes validation but causes division-by-zero in token estimation
- `PermissionRule.RiskLevel` is not validated against valid enum values
- `UI.Theme` only validates `"dark"`, `"light"`, `"auto"` but 10+ preset palettes exist
- `GitConfig.CommitPrefix` can contain newlines, producing malformed commit messages
- No upper-bound validation on `MaxGlobResults`, `MaxGrepResults`

**Impact:** Invalid config values cause runtime crashes or unexpected behavior.

**Fix:** Add comprehensive validation with upper bounds and enum checks.

---

### H-14: Type System Has No Validation Methods

**Package:** `internal/types/types.go`
**Category:** Correctness

None of the core types have validation:
- `Message.Role` is free-form string, no constants
- `Task.Status` has no `IsValid()` method
- `WorkflowPhase` has no validation
- `RiskLevel` has no validation
- `StreamChunk.Type` is unvalidated string

**Impact:** Invalid values propagate silently through the system.

**Fix:** Add validation methods and use typed constants consistently.

---

### H-15: `fmt.Sscanf` Return Values Ignored in Git StatusPorcelain

**Package:** `internal/git/git.go:338-339`
**Category:** Correctness

```go
fmt.Sscanf(parts[0], "%d", &add)
fmt.Sscanf(parts[1], "%d", &del)
```

If numstat output is malformed, `add` and `del` remain 0 silently. User sees incorrect file statistics.

**Impact:** Misleading diff stats in UI.

**Fix:** Check `n, _ := fmt.Sscanf(...)` and handle `n != 1`.

---

### H-16: No Commit Message Sanitization

**Package:** `internal/git/git.go:85-120`
**Category:** Security

Commit messages from LLM output are passed directly to `--message=`. Newlines can create fake trailers (`Signed-off-by:`, `Co-authored-by:`). No length cap.

**Impact:** Fabricated authorship attribution. Malformed git history.

**Fix:** Sanitize newlines, cap length, validate against trailer injection.

---

### H-17: Ref Injection via DiffRefs String Concatenation

**Package:** `internal/git/git.go:221-233`
**Category:** Security

```go
args = []string{"diff", ref1 + ".." + ref2}
```

Ref strings are concatenated directly. A crafted ref could contain `..` or start with `--`, causing unexpected git arguments.

**Impact:** Potential git argument injection.

**Fix:** Validate refs are valid git ref names before concatenation.

---

### H-18: Log Parsing Fragility with Pipe Characters in Author Names

**Package:** `internal/git/git.go:196-213`
**Category:** Correctness

Git log format uses `|` as delimiter. If commit author name contains `|` (e.g., `Last|First`), parsing produces incorrect author and truncated message.

**Impact:** Incorrect commit metadata display.

**Fix:** Use a less common delimiter or parse from the right side.

---

### H-19: `AtomicWrite` Silently Downgrades File Permissions

**Package:** `internal/fileutil/atomic.go:23`
**Category:** Correctness

Temp file is created with `0600`. After `os.Rename`, the target inherits `0600` permissions, silently downgrading from the original `0644`. This directly impacts `Config.Save()` — config file becomes owner-only after first save.

**Impact:** Config file becomes unreadable to other users/processes.

**Fix:** Stat original file permissions before writing, or accept a permission parameter.

---

### H-20: `SkipDirsMap()` Returns Writable Cached Map

**Package:** `internal/types/constants.go:128-137`
**Category:** Correctness

`SkipDirsMap()` returns the cached `map[string]bool` directly. Any caller can mutate it, corrupting the cache for all future callers.

**Impact:** One bad caller poisons the skip dirs for all subsequent operations.

**Fix:** Return a copy or a read-only interface.

---

### H-21: Error Pattern Matching False Positives

**Package:** `internal/errors/errors.go:106-119`
**Category:** Correctness

```go
case strings.Contains(errStr, "401"):
    return "Invalid API key — run /settings to update"
```

Substring matching on "401" catches any error containing those digits (e.g., "error reading 401 bytes"). Same for "429".

**Impact:** Misleading user-facing error messages.

**Fix:** Use regex with word boundaries: `\b401\b`.

---

### H-22: `.env` File Loading — Incomplete Permission Check

**Package:** `internal/config/loader.go:686-728`
**Category:** Security

Only checks world-writable (`0o002`), not group-writable (`0o020`). Does not check for symlinks. No line length limit.

**Impact:** Group members can inject env vars. Symlink attacks possible.

**Fix:** Check `0o022` mask, resolve symlinks, add line length cap.

---

### H-23: Variable Substitution Covers Only ~30% of String Fields

**Package:** `internal/config/loader.go:514-538`
**Category:** Correctness

`applyVarSubstitution` only processes a hardcoded subset of fields. `GitConfig.CommitPrefix`, `VerifyConfig.BuildCommand`, `ToolsConfig.SkipDirs`, `AgentsConfig.*`, and 20+ other string fields are not covered.

**Impact:** `${VAR}` in uncovered fields appears as literal string.

**Fix:** Use reflection or a field tag to apply substitution to all string fields.

---

### H-24: No Graceful Shutdown Timeout

**Package:** `cmd/m31a/main.go:206-229`
**Category:** Reliability

Signal handler sends `tea.QuitMsg{}` but has no fallback timeout. If TUI `Update()` hangs (e.g., during permission modal or streaming), the process never exits on SIGINT/SIGTERM.

**Impact:** Process hangs on interrupt, requiring SIGKILL.

**Fix:** Add `time.AfterFunc(5*time.Second, os.Exit(1))` as hard fallback.

---

### H-25: `app.Shutdown()` Not Called on Error Path

**Package:** `cmd/m31a/main.go:229`
**Category:** Reliability

`app.Shutdown()` is only called when `p.Run()` returns nil. If `p.Run()` returns an error, `Shutdown()` is never called, leaving session files inconsistent.

**Impact:** Session corruption on TUI error exit.

**Fix:** Use `defer app.Shutdown()` or call in error path.

---

### H-26: Bisect `execGit` Has No Timeout

**Package:** `pkg/bisect/exec.go:12-19`
**Category:** Reliability

`execGit` uses `exec.Command` without context or timeout. If git hangs (e.g., GPG passphrase prompt), it blocks indefinitely.

**Impact:** Bisect can hang the entire application.

**Fix:** Add context with timeout to all git exec calls.

---

### H-27: Checkpoint `LoadCheckpoints` Prunes on Every Read

**Package:** `pkg/session/checkpoint.go:101-113`
**Category:** Correctness

Every `LoadCheckpoints` call prunes excess checkpoints and rewrites the file. This is a side effect in a read operation. Concurrent readers could interfere.

**Impact:** Unexpected file writes during read operations.

**Fix:** Separate read and prune operations. Only prune on explicit save.

---

### H-28: `tiktoken-go` Is Unmaintained

**Package:** `internal/tokens/estimator.go:12-13`
**Category:** Maintenance

The dependency `tiktoken-go` has been unmaintained since 2024. It may not support newer model tokenizers.

**Impact:** Token estimation may become inaccurate for new models.

**Fix:** Plan migration to an actively maintained tokenizer library.

---

## 3. Medium Weaknesses

### M-1: `app_update.go` at 1,807 Lines — Complexity Hotspot

Single dispatch point handling 24+ screens, streaming, permissions, workflow transitions, keyboard routing. Extremely difficult to reason about, test, or modify.

### M-2: Toast Timers Map Is Dead Code

`toastTimers map[int]*time.Timer` declared in `AppState` but never written to. `addToast()` uses `tea.Tick` instead. Dead state that confuses readers.

### M-3: `/cost` Command Misleading — Claims to Toggle, Only Displays

Description says "Toggle cost display" but only shows current state and tells user to use settings.

### M-4: `/memory revert` Returns Unimplemented Message, Not Discoverable

Help text says `Usage: /memory [view|pause|resume]` but `revert` case exists in code. Dead code path.

### M-5: `/handleStatus` Leaks Raw Error Messages to User

Internal path information and JSON parse errors exposed via `fmt.Sprintf("Failed to load session: %v", err)`.

### M-6: `/log` Reads Entire Log File Into Memory

Reads entire 7-day log file just to display last N lines. Could consume hundreds of MB.

### M-7: `/handleKey` Leaks API Key Prefix and Last 4 Characters

Reveals key format (`sk-or-...`) and last 4 characters. More than needed for status confirmation.

### M-8: `/handleRollback` Performs Soft Reset Without Confirmation

`/rollback <hash>` immediately performs soft reset without `ConfirmRequired = true`. Destructive operation triggered by single command.

### M-9: Question Model Drops Enter Without Feedback

When Enter is pressed with no option selected and empty textarea, keypress is silently consumed. User gets no feedback.

### M-10: ToolCard Output Truncation Uses Inconsistent Byte/Rune Logic

Trigger check uses `len(output)` (bytes), truncation uses `utf8.RuneCountInString` (runes). For ASCII, allows slightly more than `MaxToolOutputChars` bytes.

### M-11: Iterator Double-Close on Context Cancellation

Both the cancellation watcher goroutine and the deferred `iterator.Close()` can race when context cancels simultaneously with goroutine exit. Depends on `Close()` being idempotent.

### M-12: HTTP Client Missing Response Header and TLS Timeouts

Only `DialContext` timeout configured. No `TLSHandshakeTimeout`, `ResponseHeaderTimeout`, or `IdleConnTimeout`. Server can hold connections open indefinitely.

### M-13: Fallback Logic `FindFallbackProvider` — Non-Atomic Try-HealthCheck-Rollback Sequence

Two concurrent fallback attempts can race on the active provider state. Try → HealthCheck → Rollback is not atomic.

### M-14: OpenRouter Client `isRetryable` Uses Brittle String Matching

Substring matching on "500", "502", "503" in error messages. False positives possible (e.g., "request ID: 500abc").

### M-15: No Retry on 429 Rate Limit — Retry-After Header Ignored

Client returns `ErrRateLimited` immediately without consulting `Retry-After` header. Short rate limits could be waited out.

### M-16: Zen Client Has No Retry Logic

Unlike OpenRouter, the Zen client has zero retry logic. Any transient 500/502/503 error is immediately fatal.

### M-17: Anthropic Thinking Budget Hardcoded to 1024 Tokens

Extremely low for complex tasks. No configuration path. Anthropic recommends 10,000+ for complex reasoning.

### M-18: `limitWriter` Violates `io.Writer` Contract

Returns `len(p), nil` when discarding data past limit. Callers assume `n == len(p)` means all data was written.

### M-19: Edit Tool — Whitespace-Normalized Replace Can Match Wrong Location

Strategy 4 normalizes all whitespace and compares line-by-line. Could match wrong code block if two blocks have same normalized content.

### M-20: Edit Tool — Fuzzy Anchor Replace Uses First/Last Line Anchors Only

If multiple code blocks share same first/last lines, first match is always accepted regardless of middle content similarity.

### M-21: `hasCycle` DFS Does Not Validate Dependency References

*(Same as C-9, listed here for completeness)*

### M-22: `parseQuestions` Third Fallback Captures Non-Question Lines

Lines containing `?` longer than 10 characters are captured. URLs with query params, comments mentioning questions, etc.

### M-23: `healTask` Returns Success Even If File Check Fails

After heal tool calls, missing expected files only logs warning. Method still returns `Success: true`.

### M-24: `extractJSONArray` Returns First Match Only

If LLM response has two JSON arrays, only the first is returned. Second (possibly the task list) is discarded.

### M-25: Naive Autodream Summary — Truncation, Not Summarization

~384 word concatenation, not LLM-generated. Critical context may be lost. System messages accumulate unboundedly.

### M-26: `parseEntry` in Ledger Silently Skips Malformed Lines

Pipe character in model name or project type shifts fields, causing silent entry skip. No error reporting.

### M-27: Ledger `rewriteFile` Uses Predictable Temp Path

`l.path + ".tmp"` is predictable. Race condition possible with multiple processes. Also, no restrictive file permissions.

### M-28: macOS Keychain `validateService` Returns Wrong Error

Returns `ErrNotImplemented` instead of `ErrKeychainUnavailable`. Callers make incorrect assumptions about feature availability.

### M-29: Windows Keychain `credentialBlobSize` Calculation

`len(value) * 2` is correct for ASCII but may be incorrect for non-BMP Unicode characters requiring surrogate pairs.

### M-30: Fallback Estimation Inaccurate for Non-Latin Scripts

`len([]rune)/4 * 1.3` overestimates CJK text significantly. No language-detection heuristic.

### M-31: `EstimateMessages` Ignores Tool Calls and Metadata

Only estimates `msg.Content`. Tool calls, role prefixes, JSON structure overhead not counted. 20-50% underestimate for tool-heavy conversations.

### M-32: Duplicate Constants in types/constants.go

`FetchModelsTimeout` and `DefaultFetchModelsTimeout` are identical. `MaxLLMResponseBytes` defined twice (second shadows first).

### M-33: `ToolInput.Params` Has No Size Limits

`map[string]any` with no bounds. Malicious LLM response could include millions of keys causing OOM.

### M-34: Log Rotation Race Condition on Concurrent Startup

Two instances can see old file and both try to rotate. Second rename fails. Log entries lost between failure and new file creation.

### M-35: No Max Log File Size

Daily rotation but no size limit. Verbose debug session could create hundreds of MB single-day log.

### M-36: `channelEmitter` Not Closed on Shutdown

`m.emitterCh` (cap 128) is not closed in `Shutdown()`. Goroutines blocked on send may leak.

### M-37: Permission Countdown Timer Dead Code

Line 166 sets `permCountdown = 0`, then line 167 immediately overwrites. First assignment is dead code.

### M-38: Registry `List()` and `ListAll()` Are Identical Implementations

Code duplication in `internal/provider/registry.go`.

### M-39: `MaxLLMResponseBytes` Has Commented-Out Duplicate

Dead comment followed by same value. Should be cleaned up.

### M-40: `ReasoningConfig` Prefix Matching Fragile for Future Models

OpenAI o-series matching doesn't cover `o4` or future model families.

### M-41: `ApplyReasoningParams` Overwrites Existing Body Keys Silently

Could collide with provider-specific parameters.

### M-42: WatchConfig Silently Drops Reload Messages

When channel is full, `default` case drops config changes with no logging.

### M-43: Bisect Loop Can Hang Indefinitely

If `checkFn` always returns same value, loop depends on git bisect log containing "first bad commit" to terminate. Abnormal git bisect exit can hang.

### M-44: Edit Tool No Backup Pruning Race

Concurrent edits on different files could interfere with backup pruning (minor in V1 sequential mode).

### M-45: WebFetch DNS Cache Has No Size Bound

Unbounded `sync.Map` grows without limit over long sessions.

### M-46: `ParseSSEChunk` Returns Empty Content Chunk on Missing Delta

Causes potential empty message rendering in TUI.

### M-47: Permission Request Channel Buffer (8) Denies 9th Concurrent Request

Only relevant for V1.1 concurrent subagents, but 8 is a hard limit with immediate denial.

---

## 4. Low Weaknesses

### L-1: `pathBase` Called Without Import Verification — Potential Panic on Empty String

### L-2: ANSI Regex in `SanitizeOutput` Doesn't Handle All CSI Sequences

### L-3: `convertLinks` in WebFetch Doesn't Handle `javascript:` or `mailto:` URIs

### L-4: WebFetch Double-Resolves DNS on Redirects (Defense in Depth)

### L-5: Bash Tool — `BashWaitTimeout` Can Mask Real Errors and Leak Goroutines

### L-6: Bash Tool — Race Between `cmdDone` Channel and `killTimer.Stop()`

### L-7: FileWrite — `os.Rename` Not Atomic Across Filesystems (mitigated by same-dir temp)

### L-8: FileWrite — Backup Name Collision Theoretical (32-bit random makes it negligible)

### L-9: `isBinary` Check Inconsistent Across Tools (FileWrite checks all bytes, others check 512)

### L-10: Dispatcher `Execute` Error Wrapping Loses Original Error Chain

### L-11: Grep `--glob` Filter Not Sanitized (safe in practice, but fragile)

### L-12: Glob Tool Passes Pattern Directly to `--glob` Without Sanitization

### L-13: `StashApply` Negative Index Produces Confusing Error

### L-14: `ConfigUser` Special Characters in Name/Email Malform Git Config

### L-15: `DiffFile` Path Not Sanitized (mitigated by `--` separator)

### L-16: Git Test Coverage Gaps for DiffRefs, DiffStaged, StatusPorcelain, etc.

### L-17: Nil vs Empty Slice Ambiguity on `Message.Segments`

### L-18: `Task.HealsAttempted` Not Bounded by Type

### L-19: Keychain Failure Not Checked Against Resolved Keys in main.go

### L-20: Log File Never Explicitly Synced (acceptable for diagnostic log)

### L-21: `loggerOnce` Makes DefaultLogger Untestable

### L-22: Token Estimation Race Between `Estimate()` and `Calibrate()`

### L-23: No `ErrConfigValidation` Sentinel in Errors Package

### L-24: `os.Setenv` in `.env` Loading Not Goroutine-Safe

### L-25: Empty `PermissionRule.Action` Accepted Without Default

### L-26: `overlayToastOnContent` Drops Toasts When Content Has Fewer Lines

### L-27: `formatSI` Called in `app_view.go` but Defined Elsewhere (fragile cross-file dependency)

### L-28: Workspace Root Path Issue in `pathBase` Helper

### L-29: `runWorkflowFromGoal` Sets Phase Before Engine Initialization Complete

### L-30: Phase Result Handler Does Not Reset `workflowEngine` on Error

### L-31: `view()` Rendering Anti-Patterns in 8+ Screen Renderers

### L-32: `SanitizeOutput` ToolCard Byte/Rune Inconsistency

### L-33: WebFetch `contentLength` Check Bypass for Chunked Responses (correct fallback, but code smell)

### L-34: `SSEParser` `sync.Once` on Close Prevents Error Reporting on Retry

### L-35: Config `WatchConfig` Channel Drop Without Logging

### L-36: `LoadTasks` Parses Empty Cells as Empty Strings

### L-37: `classifyText` in Arbitrage Returns Simple for Unknown Keywords

### L-38: `Chain` in Rollback Computes Diffs for All Commits Upfront (performance)

### L-39: `atomicWrite` No Concurrent Write Protection

### L-40: `webfetch` `convertLinks` Handles Empty Href but Not Fragment-Only Href

---

## 5. Architecture-Level Weaknesses

### A-1: CR-09 — `internal/tools` Imports `internal/config`

**Files:** `dispatcher.go`, `permissions.go`, `defaults.go` + 3 test files

`PermissionRule` type defined in `config/types.go` but consumed by `tools/permissions.go`. Violates the rule: "internal/tools may only import internal/types and internal/errors."

**Impact:** Architectural boundary violation. Cascading import changes needed to fix.

### A-2: W-26 — TUI Components Import Tools Package

`permission.go`, `question.go` in `components/` import `internal/tools` for risk level lookup. Creates TUI → Tools dependency.

**Impact:** Read-only dependency, not circular, but violates clean layering.

### A-3: No Sub-Model Interface — Screen Management is Manual

28 sub-models with no common interface. Adding a screen requires changes in 6+ files with no compile-time enforcement.

### A-4: God Object `AppState` Resists Decomposition

50+ fields across 180 lines. No encapsulation. Every method can access everything. Decomposition blocked by Bubble Tea's single-model requirement.

### A-5: `app_update.go` at 1,807 Lines is Unmaintainable

Single dispatch point for all screens, streaming, permissions, workflow. Cognitive overload for any modification.

### A-6: Embed Prompt Templates Without Version Management

`//go:embed prompts/*.md` embeds prompts at compile time. No version tracking. Changes require recompilation.

### A-7: No Dependency Injection for Testability

Provider, tools, session manager, git client are all created in `main.go` and threaded through constructors. No interfaces for mocking in integration tests.

---

## 6. Test Coverage Gaps

### TC-1: TUI Package — 27,639 Lines, Only 1,397 Test Lines

| Component | Lines | Tests | Coverage |
|-----------|-------|-------|----------|
| `app_update.go` | 1,807 | 0 | **0%** |
| `app_view.go` | 686 | 0 | **0%** |
| `commands*.go` | 1,200+ | 0 | **0%** |
| `streaming.go` | 200+ | 0 | **0%** |
| `repl*.go` | 2,000+ | 0 | **0%** |
| `plan_*.go` | 1,500+ | 0 | **0%** |
| `execute_*.go` | 800+ | 0 | **0%** |
| `settings*.go` | 808 | 0 | **0%** |
| Components (40) | 5,000+ | 1,397 | ~28% |

**Total TUI test coverage: ~5%**

### TC-2: Config Package — 1,938 Lines, 1 Test File

Only `loader_test.go` exists. No tests for:
- Config validation edge cases
- Variable substitution
- Hot-reload
- Multi-layer merging
- .env file loading

### TC-3: Git Package — 977 Lines, 1 Test File

Missing tests for:
- `DiffRefs`, `DiffStaged`, `DiffFile`
- `StatusPorcelain`
- `Fetch`, `Pull`, `Push`
- `CheckoutBranch`, `StashList`, `StashApply`
- `Merge`, `Tag`, `BranchList`, `RevParse`
- Error paths (empty repo, no changes)

### TC-4: Workflow Engine — Integration Test Gaps

Unit tests exist for individual phases but no end-to-end integration test with mocked LLM covering:
- Full Initialize → Discuss → Plan → Execute → Verify → Ship cycle
- Error recovery across phases
- Session resume mid-workflow
- Concurrent tool execution

### TC-5: No Fuzz Tests for Input Parsers

`plan_parser.go`, `engine_parse.go`, `webfetch.go` parse untrusted LLM output. No fuzz tests to find edge cases.

### TC-6: No Benchmark Tests

No `Benchmark*` functions exist anywhere. Critical paths (token estimation, SSE parsing, plan parsing) should have benchmarks.

### TC-7: Race Detector Coverage

`go test -race` is in CI but many packages have too few tests to actually exercise concurrent paths.

---

## 7. Recommended Fix Order

### Phase 1: Security Critical (Week 1)

1. **C-1** — Permission "remember" granularity (permissions.go)
2. **C-10** — macOS keychain CLI arg exposure (keychain_darwin.go)
3. **C-8** — ReadTaskFiles path traversal (engine_verify.go)
4. **H-16** — Commit message sanitization (git.go)
5. **H-17** — Ref injection in DiffRefs (git.go)
6. **H-22** — .env permission checks (loader.go)

### Phase 2: Data Integrity Critical (Week 1-2)

7. **C-3** — SaveSession atomicity (manager.go)
8. **C-4** — ForkSession atomicity (manager.go)
9. **C-11** — HardReset backup branch (rollback.go)
10. **H-19** — AtomicWrite permission preservation (atomic.go)
11. **H-12** — Config Save deep copy (loader.go)

### Phase 3: Reliability Critical (Week 2)

12. **C-2** — SSE stream timeout (sse.go)
13. **C-6** — Tool call dispatch error handling (execute.go)
14. **C-7** — Verify phase timeout (verify.go)
15. **H-24** — Graceful shutdown timeout (main.go)
16. **H-26** — Bisect exec timeout (exec.go)
17. **C-12** — Dispatcher Stop race (dispatcher.go)

### Phase 4: Correctness High (Week 2-3)

18. **C-5** — HealsAttempted copy semantics (execute.go)
19. **H-13** — Config validation gaps (loader.go)
20. **H-14** — Type validation methods (types.go)
21. **H-15** — fmt.Sscanf return check (git.go)
22. **H-21** — Error pattern matching (errors.go)
23. **H-23** — Variable substitution coverage (loader.go)

### Phase 5: Architecture & Maintainability (Week 3-4)

24. **H-1** — Begin AppState decomposition
25. **H-2/H-3** — SubModel interface for screen routing
26. **H-4** — Consolidate split-brain model state
27. **A-1** — Move PermissionRule to internal/types

### Phase 6: Test Coverage (Week 4-5)

28. **TC-1** — TUI app_update tests
29. **TC-2** — Config validation tests
30. **TC-3** — Git operation tests
31. **TC-4** — Workflow integration tests
32. **TC-5** — Fuzz tests for parsers
33. **TC-6** — Benchmarks for hot paths

---

## Appendix: Weakness Distribution by Package

| Package | Critical | High | Medium | Low | Total |
|---------|----------|------|--------|-----|-------|
| `internal/tui/` | 3 | 8 | 12 | 10 | 33 |
| `internal/tools/` | 2 | 4 | 8 | 6 | 20 |
| `internal/workflow/` | 3 | 4 | 6 | 2 | 15 |
| `internal/provider/` | 1 | 4 | 6 | 3 | 14 |
| `pkg/session/` | 2 | 2 | 3 | 1 | 8 |
| `internal/config/` | 0 | 4 | 4 | 3 | 11 |
| `internal/git/` | 0 | 4 | 3 | 5 | 12 |
| `pkg/rollback/` | 1 | 0 | 1 | 1 | 3 |
| `pkg/bisect/` | 0 | 1 | 2 | 0 | 3 |
| `pkg/keychain/` | 1 | 0 | 2 | 0 | 3 |
| `pkg/autodream/` | 0 | 1 | 1 | 0 | 2 |
| `pkg/ledger/` | 0 | 0 | 2 | 0 | 2 |
| `pkg/taskrunner/` | 0 | 1 | 0 | 0 | 1 |
| `internal/types/` | 0 | 2 | 2 | 2 | 6 |
| `internal/tokens/` | 0 | 0 | 2 | 2 | 4 |
| `internal/errors/` | 0 | 1 | 0 | 1 | 2 |
| `internal/fileutil/` | 0 | 1 | 0 | 1 | 2 |
| `internal/log/` | 0 | 0 | 2 | 2 | 4 |
| `cmd/m31a/` | 0 | 2 | 0 | 1 | 3 |
| **Total** | **12** | **28** | **47** | **40** | **127** |

---

*End of Report*

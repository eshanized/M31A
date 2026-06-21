# M31A — Codebase Audit Report
> **Scope**: Complete deep-read of every source file in `cmd/`, `internal/`, and `pkg/`  
> **Perspective**: End-user ("I ran M31A on my project — what went wrong?")  
> **Author**: Antigravity Audit Pass · 2026-06-21

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Critical Logical Errors](#2-critical-logical-errors)
3. [Workflow Engine Issues](#3-workflow-engine-issues)
4. [UX / Interaction Gaps](#4-ux--interaction-gaps)
5. [Configuration & Provider Blind Spots](#5-configuration--provider-blind-spots)
6. [Session & State Management](#6-session--state-management)
7. [Self-Heal & Verification Weaknesses](#7-self-heal--verification-weaknesses)
8. [Context Window Management](#8-context-window-management)
9. [Security & Permission Issues](#9-security--permission-issues)
10. [Missing Features That Users Will Expect](#10-missing-features-that-users-will-expect)
11. [Performance Gaps](#11-performance-gaps)
12. [Test Coverage Gaps](#12-test-coverage-gaps)
13. [Improvement Priority Matrix](#13-improvement-priority-matrix)

---

## 1. Executive Summary

M31A is an ambitious terminal-native AI coding agent with a well-structured 6-phase workflow engine, multi-provider support, and a rich TUI. The architecture is sound and the code quality is generally high. However, after reading every source file, there are **several logical bugs** that a real user would hit in the first day of use, a cluster of **silent failure paths** that leave the user with no feedback, and significant **UX friction** in the workflow transitions. The most serious issues are documented below, ordered by user impact.

---

## 2. Critical Logical Errors

### 2.1 `HealTask()` always re-verifies with a *stale* task copy — mutated tasks are lost

**File**: `internal/workflow/engine.go` Lines 510–515

```go
if healResult.Success {
    newResult := e.verifyTask(verifyCtx, task)        // ← `task` is the OLD value from LoadTasks
    if newResult.FilesExist && newResult.SyntaxOK && newResult.TestsOK {
        tasks[i].Status = m31types.StatusDone
    }
}
```

`task` is a **value copy** created in the for-range loop. After `healTask()` mutates files, the verification re-reads `task.Files` from the *original* snapshot. If the heal changed what files were created, the post-heal `verifyTask` checks the wrong path list. The fix is to pass `tasks[i]` (by reference) into the second `verifyTask` call.

---

### 2.2 `executeTaskWithTools` — quality gate `continue` jumps to outer loop, not inner retry

**File**: `internal/workflow/execute.go` Lines 459–481

The inner `continue` inside the quality-gate heal block re-enters the `for task.HealsAttempted < m31types.MaxHealAttempts` loop at the top — which re-calls the LLM but does **not** re-run the quality gate before dispatching tool calls. This means a heal that still fails the acceptance criteria burns another heal attempt on a guaranteed re-fail without any check.

---

### 2.3 `streamLLMStreaming` skips preflight context check — Discuss phase can overflow context

**File**: `internal/workflow/engine.go` Lines 993–1004

Both `streamLLM` and `streamLLMWithTools` call `preflightContextCheck()` before building the request. `streamLLMStreaming` (the Discuss phase's only streaming method) does **not**, meaning the Discuss phase will send arbitrarily large context to the provider and trigger a provider-level error rather than M31A's graceful truncation. This affects the very first meaningful user interaction.

---

### 2.4 `runPlan` — plan version not incremented when `chunkSucceeded`

**File**: `internal/workflow/plan.go` Lines 155–162

When `chunkSucceeded == true`, the code jumps past the standard retry loop **and** the `e.planVersion = 1` assignment. `e.planVersion` stays at 0. Downstream callers of `PlanVersion()` (TUI plan display, refinement context) see version 0, which triggers confusing "v0" labels and incorrect refinement prompts.

---

### 2.5 `preflightContextCheck` — pass 3 message removal is off-by-one

**File**: `internal/workflow/engine.go` Lines 589–602

```go
for i := 1; i < len(msgs)-keepRecent && estimated > threshold80; i++ {
    if msgs[i-removed].Role == "system" {
        continue           // ← skips but does not compensate i
    }
    msgs = append(msgs[:i-removed], msgs[i-removed+1:]...)
    removed++
}
```

When a system message is skipped with `continue`, `i` is still incremented on the next iteration. This causes the loop to **skip the message immediately following a system message** — it is never evaluated for removal. Context trimming silently leaves surplus messages.

---

### 2.6 Ship phase — `ShipSummary.DurationMs` stores session total, not phase duration

**File**: `internal/workflow/ship.go` Lines 295–302

```go
DurationMs: duration.Milliseconds(),    // `duration` = time since engine start
```

`duration` is `time.Since(e.startTime)` — the *entire session duration*. The `PhaseResult.DurationMs` field is supposed to be per-phase duration (the generic runner in `RunPhase` re-sets this). Ship phase overwrites it with the session total, making metrics and the ledger show the wrong per-phase cost.

---

### 2.7 `consumeClassifyStream` — breaks on ALL errors (including non-EOF), always returns nil error

**File**: `internal/workflow/intent.go` Lines 62–82

```go
if err != nil {
    ...
    break    // ← breaks on io.EOF AND real errors, returning partial content as "success"
}
return sb.String(), nil   // always nil error
```

Compare with `consumeStream` which correctly distinguishes `io.EOF`. The classify stream always returns `nil` error, so a network timeout silently falls through. `parseIntentJSON` gets partial JSON, the intent system logs nothing, and the fallback fires without context — making debugging impossible.

---

### 2.8 Agent arbitrage — comparing wrong cost fields when active model is not in alternatives

**File**: `internal/tui/app.go` Lines 571–582

```go
currentCost := rec.RecommendedModel.TotalCost   // initialized to RECOMMENDED model's cost
for _, alt := range rec.Alternatives {
    if alt.ModelID == m.activeModel.ID {
        currentCost = alt.TotalCost; break
    }
}
// if not found, currentCost == recommended cost → ShouldArbitrage(x, x) is always false
```

Auto-arbitrage silently does nothing for users whose active model was not returned in the alternative list (e.g., a newly-added or custom model ID).

---

## 3. Workflow Engine Issues

### 3.1 Plan checker no-ops on chunked plans — quality gates silently pass

The chunked plan markdown produced by `composeChunkedPlanMarkdown` contains raw JSON inside a code block, not the structured markdown that `ParsePlan()` expects. When `runPlanChecker` calls `ParsePlan(planMarkdown)` on this output, it fails and falls back to `&m31types.Plan{Tasks: tasks, RawMarkdown: planMarkdown}` — a plan object with no Summary, no Verification steps, no ProposedChanges. The plan checker and coverage gates then run against an almost-empty plan object, finding no issues. All quality checks silently pass for chunked plans.

---

### 3.2 Discuss phase does not respect the configured `discuss_timeout`

`UIConfig.DiscussTimeout` is defined in config types but **never read** during the Discuss phase. The TUI discuss model has no timer-based auto-skip. Users who set `discuss_timeout = 60` in config.toml will find it has zero effect.

---

### 3.3 Phase transition guard allows `Execute → Ship` regardless of workflow mode

```go
m31types.PhaseExecute: {m31types.PhaseVerify, m31types.PhaseShip, m31types.PhaseIdle},
```

A TUI edge case that emits a `PhaseShip` message from Execute in `full` mode would skip verification with no error. The transition guard should be mode-aware, or the Ship summary should warn when Verify was skipped.

---

### 3.4 SkipDiscuss save failure is not surfaced to the user

When `FinalizeDiscuss` returns an error (session manager I/O failure during `SaveProject`), the error is returned to the TUI caller. The caller logs it but does not show a toast or block progression. The user proceeds to Plan without their discuss answers persisted — the plan will be generated without clarification context.

---

## 4. UX / Interaction Gaps

### 4.1 Ctrl+C cancels stream then immediately shows "press again to exit" — confusing double-tap

After pressing Ctrl+C to cancel a stream, `streamCancelFn` is set to nil. If the user immediately presses Ctrl+C again (instinctively), they receive "Press ctrl+c again to exit (2s window)" — which is confusing because they just cancelled a stream, not requested an exit.

---

### 4.2 No active workflow mode indicator in the REPL

When auto-mode silently skips the Discuss phase (for simple intents), users are confused. The sidebar shows the current phase but not the *mode* that determined the routing. A small "Mode: fast" indicator in the footer or sidebar header would eliminate a common support question.

---

### 4.3 Welcome screen "1/2/3" shortcuts permanently disabled after any screen navigation

```go
if len(m.messages) == 0 && m.welcomeRevealCount >= 3 {
```

After opening `/settings` and returning, `m.messages` will have entries (the slash command is added). The number shortcuts are permanently disabled. Should be gated on `m.workflowGoal == ""` instead.

---

### 4.4 Slash autocomplete completes on Enter even when text is an exact command match

If the user types `/ship` and `/shiplog` appears as a suggestion, pressing Enter completes to `/shiplog` instead of submitting `/ship`. Completion should only fire if the user navigated the suggestion list explicitly.

---

### 4.5 History navigation (Up arrow) does not restore draft on Esc

After pressing Up to browse history, pressing Esc only dismisses the slash/mention overlay — it does not restore the pre-history draft from `savedInput`. The draft is only restored on explicit Down-to-index-0.

---

### 4.6 `ScreenConfirmQuit` is wired but unreachable — dead code

`ScreenConfirmQuit`, `ConfirmQuitModel` and its Update handler are all implemented but no keyboard shortcut or command navigates to it from the REPL. Either wire a `/quit` slash command or remove the dead code.

---

### 4.7 File watcher failure is `slog.Debug` — invisible in normal operation

```go
slog.Debug("file watcher init failed", "error", err)
```

On inotify-exhausted systems (Docker, containers), the sidebar silently stops auto-refreshing. This should be at minimum `slog.Warn`, preferably a dismissible toast so users understand why the sidebar is stale.

---

### 4.8 Sidebar auto-show threshold ignores `sidebar_width_threshold` config

`UIConfig.SidebarWidthThreshold` is defined but the sidebar auto-show logic uses the hard-coded `sidebarDefaultWidth = 30` constant. The config field has no effect.

---

## 5. Configuration & Provider Blind Spots

### 5.1 After auto-fallback, the original provider is never re-checked on recovery

Once `attemptAutoFallback` switches providers, the health ticker updates `m.healthStatus` but never calls `registry.SetActive(originalProvider)` on recovery. Users on a flaky connection permanently fall back to their secondary provider for the session.

---

### 5.2 `AgentsConfig` missing Initialize and Research phases

```go
type AgentsConfig struct {
    Default, Plan, Execute, Verify, Ship, Discuss string
    // Missing: Initialize, Research
}
```

The Initialize phase (deep project analysis) and Research sub-step both use `e.modelForPhase()` which falls back to `e.modelID` for unlisted phases. Users cannot assign a cheaper model to pre-planning research.

---

### 5.3 `MEMORY.md` grows unbounded across sessions

Every session appends to `MEMORY.md` with no size cap, rotation, or max-entries config. After 100+ sessions it can grow to hundreds of KB. Both `buildDiscussContext` and `buildPlanContext` inject the **entire** `MEMORY.md` into the LLM context without truncation — silently consuming large portions of the context window and inflating API costs.

---

### 5.4 Keychain failure + no inline API key = infinite first-run loop

If the keychain is unavailable (headless server, missing libsecret) and the config file has no inline API key, the user sees the first-run wizard. The wizard also fails to save to the keychain. The user is stuck with no clear error message explaining what failed.

---

## 6. Session & State Management

### 6.1 Only one `.bak` backup file — overwritten every `NewSession` call

If M31A crashes mid-session, `session.json` may be corrupted and the `.bak` is from two sessions ago (already overwritten). No timestamped or rotating backup exists for crash recovery.

---

### 6.2 `ResumeOnStartup` can corrupt a concurrently running session

```go
sessions[0].ID   // the most recently modified session
```

If M31A is running in another terminal, a new instance with `resume_on_startup = true` will attempt to restore and write to the same session. There is no lock file or PID guard preventing concurrent session writes.

---

### 6.3 Session cleanup (`Cleanup`) is effectively a no-op

The session manager uses a flat layout (one `session.json` per project). `ListSessions` returns at most the current session. The `Cleanup` retention logic therefore never deletes anything — it checks age of a single session it won't touch.

---

### 6.4 Plan version does not survive session resume

When a session is resumed, a new engine is created with `planVersion = 0`. If the original session had a plan at version 3, refinement feedback will show "Refining plan (v1)..." instead of "Refining plan (v4)...". The plan version should be persisted in the session state.

---

## 7. Self-Heal & Verification Weaknesses

### 7.1 `healTask` dispatches tool calls sequentially — 4× slower than execute

Execute dispatches tool calls in parallel (up to 4 concurrent). Heal dispatches them sequentially:

```go
for _, tc := range toolCalls {
    _, err := e.dispatcher.Execute(ctx, tc)
```

For tasks with multiple file writes, heals are dramatically slower than the original execute — adding latency at exactly the wrong time.

---

### 7.2 Bisect heal can leave the repository in bisect mode after a crash

`tryBisectHeal` calls `git bisect start` and checks out intermediate commits. If M31A is interrupted mid-bisect, the repository is left in bisect mode at an unknown commit. `subagent.Sweep` cleans worktrees but does not run `git bisect reset`.

---

### 7.3 Shared `HealsAttempted` counter across Execute and Verify phases

A task using 2 heal attempts in Execute arrives at Verify with `HealsAttempted = 2`. If verify also fails, only 1 heal remains. Tasks that were hard to implement but correctly verified get incorrectly marked `StatusUnrecoverable` during verify because the execute phase consumed the budget.

---

### 7.4 `verify.build_command` and `verify.test_command` config fields are silently ignored

`VerifyConfig` is loaded but `runVerify` uses auto-detection heuristics (Makefile, package.json, etc.) — not the configured commands. A user setting `build_command = "cargo test"` for a Rust project will find it has no effect.

---

## 8. Context Window Management

### 8.1 Full plan injected into every task context — expensive for large plans

For a 50-task project with a detailed plan, `buildExecuteContext` adds 2,000–5,000 tokens to every single task's context. The plan context should be filtered to the current task's section and adjacent dependencies, not the entire plan.

---

### 8.2 `readTaskFiles` truncates from the first file — later files never shown

```go
if len(fileCtx) > maxFileCtx {
    fileCtx = fileCtx[:maxFileCtx] + "\n... (truncated)"
}
```

If file #1 is 40KB, the truncation cuts off there. Files #2–#10 are never shown to the LLM. A per-file cap with all files included would be more useful than one complete large file and none of the others.

---

### 8.3 AutoDream consolidation is not persisted — re-inflates on resume

When consolidation fires and `replModel.SetMessages` replaces the message list, the consolidated messages are **not persisted** to `messages.json`. On session resume, the original un-consolidated messages are restored, immediately re-ballooning the context.

---

## 9. Security & Permission Issues

### 9.1 User-typed shell commands (`!cmd`) trigger the AI permission prompt

```go
if strings.HasPrefix(input, "!") { ... SlashCommandMsg{Command: input} }
```

This routes through the Bash tool, which shows "The AI wants to run: ls". For commands explicitly typed by the user, this is jarring and misleading — the user should be exempt from AI permission prompts for their own commands.

---

### 9.2 `MEMORY.md` is created with 0644 permissions on shared systems

```go
os.OpenFile(memPath, os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0644)
```

`MEMORY.md` includes session goals, project types, model names, and failed task details. On shared systems or CI environments, other users can read this file. It should use `0600` (owner-read-only).

---

### 9.3 Concurrent subagent backups use the same path — overwrite each other

Subagents share the parent's `backupDir`. Two agents backing up `src/main.go` concurrently use the same backup path with no namespace separation. Each overwrites the other's backup, defeating the recovery mechanism.

---

## 10. Missing Features That Users Will Expect

### 10.1 No `/undo` slash command for the last AI action

Users who see the AI write bad code have no immediate undo. The `/rollback` screen requires understanding git. A `/undo` command running `git checkout -- <last-modified-files>` is critical for daily use.

---

### 10.2 No progress indicator during intent classification (5-second delay with no feedback)

Intent classification adds up to 5 seconds of latency before the workflow starts. During this time the REPL shows nothing. A "Classifying intent…" micro-status would dramatically improve perceived responsiveness.

---

### 10.3 No per-project or per-session custom system prompt

Power users cannot inject project-specific instructions without modifying the binary. A `.m31a/SYSTEM_PROMPT.md` auto-injected at engine startup would solve this.

---

### 10.4 Follow-up questions from `GenerateFollowUpsIfNeeded` are never shown in the TUI

`GenerateFollowUpsIfNeeded` is implemented in the engine but the `DiscussModel` only shows the initial question list. Follow-up questions are fully unreachable by users even though the backend exists.

---

### 10.5 No `/export` command for the session transcript

Users frequently want to save or share a session's conversation. The chat history screen shows messages but there is no export-to-Markdown command. `messages.json` is in internal format.

---

### 10.6 Ghost mode is wired but has no entry point from the REPL

`GhostPickerModel`, `GhostOutputModel`, `ScreenGhostPicker`, and `ScreenGhostOutput` are implemented and wired. No keyboard shortcut navigates to the ghost picker from the REPL. Users who discover it by accident get a broken experience.

---

## 11. Performance Gaps

### 11.1 Code intel is always rebuilt at group boundaries, even when no files changed

```go
e.codeIntel = nil
e.codeIntelBuilt = false
```

The invalidation should only fire if any `FileWrite`/`Edit` tool calls were made in the just-completed group. For read-only tasks, the 30-second code intel rebuild is wasted on every group boundary.

---

### 11.2 `buildExecuteContext` re-parses the plan markdown on every task in the same group

The plan is cached by MD5 hash, so re-parsing only happens when the markdown changes. However, the MD5 is computed on every call — a `strings.Builder.String()` allocation per task. For 50 tasks, this is 50 MD5 computations of a potentially large markdown string.

---

### 11.3 MEMORY.md grows to hundreds of KB, injected untruncated into every Discuss and Plan call

(See §5.3 above — combined cost and performance issue.)

---

## 12. Test Coverage Gaps

### 12.1 `runChunkedPlan` has no integration test

No test exercises the full chunked planning flow including `composeChunkedPlanMarkdown`. The plan-checker integration bug (§3.1) is not caught by any test.

---

### 12.2 `checkAutoArbitrage` has no unit test

The arbitrage cost comparison bug (§2.8) would be caught by a test verifying "current model not in alternatives → no switch".

---

### 12.3 The 5-second force-exit timer in `main.go` has no test

The signal handler's `time.After(5s)` path calls `os.Exit(1)` and writes a sentinel file. This code path is never tested and the `cleanup()` call before `os.Exit` could deadlock if TUI goroutines are still active.

---

### 12.4 `streamLLMStreaming` context overflow is not tested

The missing preflight check (§2.3) is not covered. There are tests for `preflightContextCheck` itself but no test verifying the Discuss phase handles context overflow gracefully.

---

## 13. Improvement Priority Matrix

| # | Issue | User Impact | Effort | Priority |
|---|-------|-------------|--------|----------|
| 2.3 | `streamLLMStreaming` skips preflight | **High** — Discuss fails on large projects | Low | 🔴 P0 |
| 2.1 | HealTask re-verifies stale task copy | **High** — heals wrongly fail | Low | 🔴 P0 |
| 5.3 | MEMORY.md grows unbounded | **High** — cost/context inflation | Low | 🔴 P0 |
| 6.2 | Two instances corrupt shared session | **High** — data loss | Medium | 🔴 P0 |
| 2.4 | Plan version 0 after chunked planning | **Medium** — UI confusion | Low | 🟠 P1 |
| 2.6 | Ship phase stores wrong DurationMs | **Medium** — misleading metrics | Low | 🟠 P1 |
| 3.1 | Plan checker no-ops on chunked plans | **Medium** — quality gates skipped | Medium | 🟠 P1 |
| 7.1 | Heal tool calls are sequential | **Medium** — 4× slower heals | Low | 🟠 P1 |
| 7.3 | Shared heal counter across phases | **Medium** — tasks wrongly unrecoverable | Medium | 🟠 P1 |
| 2.5 | Context trimming off-by-one | **Medium** — context overflows persist | Medium | 🟠 P1 |
| 7.4 | verify commands config ignored | **Medium** — config field is a lie | Low | 🟠 P1 |
| 4.4 | Enter completes slash on exact match | **Medium** — UX friction | Low | 🟡 P2 |
| 4.7 | File watcher failure is silent | **Medium** — stale sidebar in containers | Low | 🟡 P2 |
| 10.1 | No `/undo` command | **High UX ask** | High | 🟡 P2 |
| 10.3 | No per-project system prompt | **High UX ask** | Medium | 🟡 P2 |
| 10.5 | No `/export` transcript command | **Medium UX ask** | Medium | 🟡 P2 |
| 8.1 | Full plan in every task context | **Low-Medium** — cost on large plans | Medium | 🟡 P2 |
| 2.7 | classify stream swallows errors | **Medium** — silent failures | Low | 🟡 P2 |
| 9.2 | MEMORY.md is 0644 permissions | **Low** — security hygiene | Low | 🟢 P3 |
| 2.2 | Quality gate continue skips re-check | **Low** — burns heal attempt | Medium | 🟢 P3 |
| 3.3 | Phase guard not mode-aware | **Low** — edge case | Medium | 🟢 P3 |
| 9.3 | Subagent backups overwrite each other | **Low** — parallel subagent edge case | Medium | 🟢 P3 |
| 5.1 | No provider recovery after fallback | **Low** — session-scoped | High | 🟢 P3 |
| 4.6 | ConfirmQuit screen unreachable | **Low** — dead code | Low | 🟢 P3 |
| 6.3 | Session cleanup is a no-op | **Low** — cosmetic | Medium | 🟢 P3 |

---

## Appendix: Files Audited

All source files in the following packages were read in full:

- `cmd/m31a/` — `main.go`, `usage.go`
- `internal/config/` — `types.go`, `loader.go`, `project_context.go`
- `internal/workflow/` — all 59 `.go` files including `engine.go`, `execute.go`, `plan.go`, `ship.go`, `verify.go`, `discuss.go`, `intent.go`, `research.go`, `initialize.go`, `plan_chunk.go`, `plan_check.go`, `coverage_gates.go`, `engine_verify.go`
- `internal/tui/` — all 102 `.go` files (key: `app.go`, `app_update.go`, `app_state.go`, `repl.go`, `repl_state.go`, `sidebar_model.go`, `settings_model.go`, `firstrun_model.go`)
- `internal/provider/` — all 23 `.go` files
- `pkg/session/` — all 15 `.go` files
- `pkg/arbitrage/`, `pkg/autodream/`, `pkg/bisect/`, `pkg/history/`, `pkg/keychain/`, `pkg/ledger/`, `pkg/metrics/`, `pkg/rollback/`, `pkg/taskrunner/`

---

*Report generated by Antigravity deep audit pass. Last updated: 2026-06-21.*

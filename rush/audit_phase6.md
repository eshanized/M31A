# Phase 6 — Workflow Engine Deep Audit

**Date:** 2026-05-29
**Scope:** `internal/workflow/`, `pkg/taskrunner/`, `pkg/bisect/`, workflow TUI screens
**Method:** Line-by-line review of all 2,800+ LOC across 15 files

---

## Executive Summary

Phase 6 is **structurally sound but has 29 specific gaps** between the roadmap specification and the actual implementation. The engine runs all 6 phases, prompts are embedded and composed correctly, and the task runner's topological sort is solid. However, there are critical gaps in **phase transitions**, **self-heal wiring**, **context composition**, **bisect integration**, and **LLM feedback loops**. Several pieces of code are orphaned (written but never called).

**Severity breakdown:**
- **Critical (9):** Functionality that is broken or incomplete in a way that affects end-to-end workflow
- **High (8):** Missing features from the roadmap that degrade the user experience
- **Medium (7):** Code quality issues, deviations from spec, or edge cases
- **Low (5):** Cosmetic deviations, placeholder TUI features

---

## Critical Gaps

### C1: No Automatic Phase Transitions

**Roadmap:** Initialize transitions to Discuss automatically. Discuss transitions to Plan automatically.

**Reality:** `runInitialize()` and `runDiscuss()` both have `transitionToPlan()` / equivalent methods defined but **never called**. The TUI must manually trigger each phase transition.

**Files:** `internal/workflow/initialize.go`, `internal/workflow/discuss.go`

**Impact:** User must manually invoke each phase. Workflow is not autonomous.

### C2: Discuss Q&A Collection Never Happens in Engine

**Roadmap:** "Stream LLM; parse 2-4 numbered clarifying questions; Render questions inline; user answers one at a time"

**Reality:** `runDiscuss()` returns questions in `PhaseResult.Messages` but **never collects answers**. `saveDiscussAnswers()` exists but is never called by the engine. `CollectAnswers()` helper exists but is only used in tests.

**Files:** `internal/workflow/discuss.go` (lines 25-30, 118-133)

**Impact:** Answers are never saved to PROJECT.md during workflow execution. Plan phase gets no Q&A context.

### C3: Plan Retry Does Not Feed Validation Errors Back to LLM

**Roadmap:** "On validation failure: send errors back to LLM (max 3 retries)"

**Reality:** The retry loop in `runPlan()` passes `nil` for `existingTasks` on each retry. The `buildPlanContext()` function includes "Previous task list had errors. Please fix" when `existingTasks` is non-empty, but the loop never populates it. **Each retry sends the identical request.**

**Files:** `internal/workflow/plan.go` (lines 35-60)

**Impact:** LLM produces the same invalid JSON on each retry. All 3 retries waste tokens.

### C4: No Self-Heal During Execute Phase

**Roadmap:** Execute phase should self-heal on task failure.

**Reality:** `healTask()` exists but is **never called from `executeTaskWithTools()`**. Self-heal only runs in `verify.go`. Failed tasks during execution are simply marked failed with no recovery attempt.

**Files:** `internal/workflow/execute.go` (lines 100-140 — healTask defined but orphaned)

**Impact:** Task failures during execution are not recoverable. Workflow is fragile.

### C5: Bisect Uses Hardcoded `HEAD~50` Instead of `sessionStartHash`

**Roadmap:** `Bisect.Run(sessionStartHash, headHash, checkFn)` — bisect from session start.

**Reality:** `verify.go` calls `bisect.Run("HEAD~50", "HEAD", checkFn)`. The `sessionStartHash` is **never tracked or passed** to the verify phase.

**Files:** `internal/workflow/verify.go` (line 108)

**Impact:** Bisect searches arbitrary 50-commit range instead of session-specific commits. May find unrelated bugs.

### C6: No 3rd Targeted Heal After Bisect

**Roadmap:** After bisect identifies offending commit, use the diff for a 3rd targeted heal attempt.

**Reality:** Bisect result is logged but **the diff is never passed to the LLM**. No 3rd heal is attempted.

**Files:** `internal/workflow/verify.go` (lines 110-120)

**Impact:** Bisect diagnostic capability is wasted. The most powerful heal (with exact diff) never happens.

### C7: No Manual Task Entry Fallback After 3 Plan Failures

**Roadmap:** "After 3 failures: prompt user to enter tasks manually or skip to REPL"

**Reality:** After 3 retries, `runPlan()` returns an error. No user prompt, no manual entry path, no REPL skip.

**Files:** `internal/workflow/plan.go` (line 62)

**Impact:** Workflow dead-ends on persistent plan failures.

### C8: No End-to-End Integration Test

**Roadmap:** "Mocked LLM provider with scripted responses for each phase. End-to-end: Initialize → Discuss → Plan → Execute → Verify → Ship against temp git repo"

**Reality:** Each phase is tested individually. **No test runs all 6 phases sequentially.** The mock provider only returns a static string — it does not support tool calls or multi-turn conversations.

**Files:** `internal/workflow/engine_test.go`, `internal/workflow/*_test.go`

**Impact:** No confidence that the full workflow pipeline works together. Regressions in phase handoffs would be caught only in production.

### C9: PROJECT.md Missing from Execute Context

**Roadmap:** "Pruned context per task: system prompt + TASKS.md + PROJECT.md + current task spec"

**Reality:** `buildExecuteContext()` includes system prompt, task summary table, and current task spec. **PROJECT.md (goal, project type, framework) is not included.**

**Files:** `internal/workflow/execute.go` (lines 16-30)

**Impact:** LLM executes tasks without knowing the overall goal or project context.

---

## High-Severity Gaps

### H1: No LLM-Based Verification

**Roadmap:** "Pruned context: system prompt + TASKS.md + file contents of all task outputs" sent to LLM for verification.

**Reality:** `verifyTask()` is purely deterministic — file existence checks, `go build ./...`, `go test ./...`. No LLM is involved.

**Files:** `internal/workflow/verify.go` (lines 40-80)

**Impact:** Verification cannot assess code quality, correctness of logic, or whether acceptance criteria are met semantically.

### H2: Go-Only Syntax/Test Validation

**Reality:** `verifyTask()` hardcodes `go build ./...` and `go test ./...`. No syntax validation for JavaScript, Python, Rust, or other languages.

**Files:** `internal/workflow/verify.go` (lines 55-70)

**Impact:** Non-Go projects always fail verification checks.

### H3: No PlanReadyMsg Emission

**Roadmap:** "Serialize to TASKS.md; emit PlanReadyMsg"

**Reality:** `runPlan()` returns tasks in `PhaseResult` but there is no `PlanReadyMsg` type defined or emitted.

**Files:** `internal/tui/types.go` — no PlanReadyMsg exists.

**Impact:** TUI has no explicit signal that plan is ready for review.

### H4: No TaskStartMsg from TaskRunner

**Roadmap:** "Per-task lifecycle: emit TaskStartMsg → stream LLM → dispatch tool calls → git commit → emit result"

**Reality:** `taskrunner.Runner.ExecuteFunc` returns a single `TaskResult` after completion. No streaming callbacks or message emission during execution.

**Files:** `pkg/taskrunner/runner.go` (lines 80-120)

**Impact:** TUI cannot show live progress during task execution. Execute screen is static until task completes.

### H5: Discuss "Skip" Has No Default-Filling Mechanism

**Roadmap:** "skip fills remaining with defaults"

**Reality:** No skip mechanism exists. No default values for unanswered questions.

**Files:** `internal/workflow/discuss.go`

**Impact:** User cannot bypass Discuss phase. Must answer all questions or break workflow.

### H6: MEMORY.md Missing from Plan Context

**Roadmap:** "Pruned context: system prompt + goal + Discuss Q&A + MEMORY.md + cwd file schema"

**Reality:** `buildPlanContext()` does not load MEMORY.md.

**Files:** `internal/workflow/plan.go` (lines 16-30)

**Impact:** Cross-session learning is not available during planning.

### H7: Commit Message Format Deviations

**Roadmap:** `git add -A && git commit -m "feat: <description>"` and `chore: ship <session-id>`

**Reality:** Uses `feat(task N): <description>` and `chore(ship): complete session <session-id>`.

**Files:** `internal/workflow/execute.go` (line 80), `internal/workflow/ship.go` (line 25)

**Impact:** Minor — inconsistent commit message conventions.

### H8: `git add -A` Not Used

**Roadmap:** Atomic git commits with `git add -A`

**Reality:** `CommitWithFiles()` takes specific file paths. Only those files are staged.

**Files:** `internal/workflow/execute.go` (line 80), `internal/git/git.go`

**Impact:** Untracked files (new files not in task.Files) are not committed. Task outputs may be incomplete.

---

## Medium-Severity Gaps

### M1: No PhaseHeal as Distinct Phase

**Roadmap:** P6.7 lists a dedicated heal phase.

**Reality:** Healing is embedded inline in `execute.go` and `verify.go`. `RunPhase()` has no `case "heal"`.

**Files:** `internal/workflow/engine.go` (lines 200-230)

**Impact:** Architectural deviation. Not critical since healing works inline, but differs from roadmap design.

### M2: `buildToolDefinitions()` Has Hardcoded `"{}"` for Parameters

**Reality:** Tool parameters JSON schema is `"{}"` — empty object. LLM gets no input schema guidance.

**Files:** `internal/workflow/engine.go` (lines 320-350)

**Impact:** LLM may produce malformed tool calls without schema guidance.

### M3: `listCwdFiles` Silently Swallows Walk Errors

**Reality:** `filepath.Walk` func returns `nil` on error — silently skips subtrees.

**Files:** `internal/workflow/engine.go` (lines 400-410)

**Impact:** File schema may be incomplete. LLM plans without seeing all files.

### M4: Bisect Loop Has No Iteration Limit

**Reality:** `for` loop in `bisect.Run()` has no `maxIterations` safeguard. Edge case: if `checkFn()` always returns same value, loop could run indefinitely.

**Files:** `pkg/bisect/bisect.go` (lines 40-70)

**Impact:** Potential infinite loop in edge cases.

### M5: `parseToolCalls` Uses Fragile Regex

**Reality:** Tool call extraction from LLM response uses regex-based JSON extraction. Fails on nested JSON or escaped quotes.

**Files:** `internal/workflow/engine.go` (lines 360-390)

**Impact:** Complex tool calls with nested objects may fail to parse.

### M6: 5-Minute Execution Timeout Is Hardcoded

**Reality:** `context.WithTimeout(context.Background(), 5*time.Minute)` in `taskrunner/runner.go`. Not configurable.

**Files:** `pkg/taskrunner/runner.go` (line 85)

**Impact:** Long-running tasks (e.g., npm install, large builds) may timeout prematurely.

### M7: Plan Validation Does Not Check `files` or `acceptance_criteria`

**Reality:** `validateTasks()` checks ID, description, action, deps, cycles. Does not validate that `files` or `acceptance_criteria` arrays are present/non-empty.

**Files:** `internal/workflow/engine.go` (lines 280-310)

**Impact:** Tasks with empty files/acceptance_criteria pass validation but break downstream execution.

---

## Low-Severity Gaps

### L1: TUI Placeholders

| Screen | Key | Feature | Status |
|--------|-----|---------|--------|
| Plan | `e/E` | Edit task inline | Placeholder — no implementation |
| Execute | `p/P` | Pause execution | State-only flag, no execution loop reads it |
| Execute | `r/R` | Resume execution | Cosmetic |
| Verify | `h/H` | Self-heal | Placeholder |
| Ship | `o/O` | Open in browser | Placeholder |

### L2: Execute Screen Has No Streaming Output Display

`toolCard` field exists but nothing populates it during execution.

### L3: Verify Screen Offers No Recovery Path Beyond Skip

`[UNRECOVERABLE]` displays but no bisect result or targeted heal option is offered.

### L4: Ship Screen Missing Ledger Confirmation

No indication that ledger was updated or session archived.

### L5: Ship Commit Format Minor Deviation

`chore(ship):` vs `chore: ship` — cosmetic inconsistency.

---

## Orphaned Code

Code that exists but is **never called** in production:

| Function | File | Where it should be called |
|----------|------|--------------------------|
| `saveDiscussAnswers()` | `discuss.go` | `runDiscuss()` |
| `transitionToPlan()` | `discuss.go` | `runDiscuss()` |
| `healTask()` | `execute.go` | `executeTaskWithTools()` on task failure |
| `transitionToDiscuss()` | `initialize.go` | `runInitialize()` |

---

## Test Coverage Assessment

| Package | Tests | Quality | Gap |
|---------|-------|---------|-----|
| `engine_test.go` | 18 tests | Good | No full workflow integration test |
| `initialize_test.go` | 7 tests | Good | None |
| `discuss_test.go` | 8 tests | Good | Tests CollectAnswers but engine doesn't call it |
| `plan_test.go` | 8 tests | Good | No test for retry-with-errors path |
| `execute_test.go` | 6 tests | Moderate | No full execute group flow test |
| `verify_test.go` | 8 tests | Good | No bisect integration test |
| `ship_test.go` | 4 tests | Good | None |
| `runner_test.go` | 22 tests | Excellent | None |
| `bisect_test.go` | 15+ tests | Excellent | None |

**Overall: 96 test functions. Good coverage of individual functions. Missing: end-to-end workflow test, retry-with-feedback test, bisect+heal integration test.**

---

## Priority Fix Order

1. **C3** — Feed validation errors back to LLM in plan retries (highest ROI fix)
2. **C1** — Wire automatic phase transitions (Initialize→Discuss→Plan)
3. **C2** — Wire Discuss Q&A collection and save to PROJECT.md
4. **C4** — Wire self-heal in execute phase
5. **C5** — Track and pass sessionStartHash to bisect
6. **C6** — Add 3rd targeted heal after bisect using diff
7. **C7** — Add manual task entry fallback after 3 plan failures
8. **C9** — Add PROJECT.md to execute context
9. **H1** — Add LLM-based verification (or accept deterministic-only for V1)
10. **H2** — Add multi-language syntax validation (or document Go-only limitation)
11. **H3** — Define and emit PlanReadyMsg
12. **H4** — Add TaskStartMsg/callback to taskrunner
13. **H5** — Add skip/default mechanism for Discuss
14. **H6** — Load MEMORY.md in plan context
15. **H7** — Fix commit message format
16. **H8** — Use `git add -A` or document file-specific staging
17. **C8** — Write end-to-end integration test
18. **M1-M7** — Address medium issues
19. **L1-L5** — Address low issues / remove placeholders

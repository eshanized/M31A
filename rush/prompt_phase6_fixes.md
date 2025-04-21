# Fix Phase 6 — Workflow Engine Critical & High Gaps

## Context

The Phase 6 deep audit (`rush/audit_phase6.md`) identified **29 gaps** between the roadmap specification and the actual implementation. This prompt fixes all **Critical (9)** and **High (8)** gaps. Medium and Low gaps are deferred.

**Key finding:** The workflow engine's structure is solid — all 6 phases exist, prompts are embedded and composed correctly, task runner topological sort works, bisect implementation is thorough. The gaps are primarily **wiring issues**: functions that exist but are never called, context fields that are missing, and feedback loops that are broken.

---

## Fix 1: Wire Automatic Phase Transitions (C1)

### Problem
`runInitialize()` and `runDiscuss()` do not auto-transition to the next phase. The TUI must manually trigger each transition.

### Files to Read
- `internal/workflow/initialize.go` — `runInitialize()` function
- `internal/workflow/discuss.go` — `runDiscuss()` function, `transitionToPlan()` method
- `internal/workflow/engine.go` — `Transition()` method, `RunPhase()` switch

### What to Do
1. After `runInitialize()` completes successfully, call `e.Transition("discuss")` within the same method
2. After `runDiscuss()` completes successfully (questions answered and saved), call `e.Transition("plan")` within the same method
3. Ensure `Transition()` updates STATE.md and checkpoint.json before returning
4. The `PhaseResult` from Initialize should indicate `NextPhase: "discuss"`, and from Discuss should indicate `NextPhase: "plan"`
5. Update `runInitialize()` tests to assert the phase transition occurred
6. Update `runDiscuss()` tests to assert the phase transition occurred

### Expected Changes
- `internal/workflow/initialize.go` — add `e.Transition("discuss")` at end of successful run
- `internal/workflow/discuss.go` — add `e.Transition("plan")` at end of successful run (after answers saved)
- `internal/workflow/engine.go` — ensure `PhaseResult` includes `NextPhase` field
- Tests updated

---

## Fix 2: Wire Discuss Q&A Collection (C2, H5)

### Problem
`runDiscuss()` returns questions but never collects answers. `saveDiscussAnswers()` and `CollectAnswers()` are orphaned.

### Files to Read
- `internal/workflow/discuss.go` — `runDiscuss()`, `saveDiscussAnswers()`, `CollectAnswers()`
- `internal/workflow/engine.go` — `PhaseResult` struct, `streamLLM()` method
- `internal/tui/repl.go` — how user input is captured (for understanding the collection pattern)

### What to Do
1. Modify `runDiscuss()` to include an answer collection mechanism. Since the engine is TUI-agnostic, the engine should:
   - Return questions in `PhaseResult` with a flag indicating answers are needed
   - Provide a `SubmitAnswer(questionIndex, answer)` method on the engine that the TUI calls
   - After all answers are submitted (or skip), call `saveDiscussAnswers()` to persist to PROJECT.md
2. Add `DiscussState` to the engine: `questions []string`, `answers map[int]string`, `answeredCount int`
3. Add `SubmitDiscussAnswer(index int, answer string) error` method
4. Add `SkipDiscuss()` method that fills defaults (empty strings or "Not specified")
5. After all answers collected or skipped, `runDiscuss()` saves and auto-transitions to Plan (Fix 1)
6. Update tests to exercise the answer collection flow

### Expected Changes
- `internal/workflow/engine.go` — add `DiscussState` struct and `discussState` field to Engine
- `internal/workflow/discuss.go` — add `SubmitDiscussAnswer()`, `SkipDiscuss()`, wire `saveDiscussAnswers()` in `runDiscuss()`
- `internal/workflow/discuss_test.go` — test answer collection and skip
- `internal/tui/app.go` or `internal/tui/repl.go` — wire `SubmitDiscussAnswer()` calls when user answers questions

---

## Fix 3: Feed Validation Errors Back to LLM in Plan Retries (C3)

### Problem
Plan retry loop sends the identical request each time. Validation errors are never communicated to the LLM.

### Files to Read
- `internal/workflow/plan.go` — `runPlan()` retry loop (lines 35-62)
- `internal/workflow/engine.go` — `buildPlanContext()` function, `validateTasks()` function

### What to Do
1. In the retry loop, capture validation errors from `validateTasks()`
2. Pass the validated (but invalid) tasks AND the error messages to `buildPlanContext()` on retry
3. Modify `buildPlanContext()` to include a "Previous attempt errors: <error list>" section when `existingTasks` is non-empty
4. Include the previous (invalid) JSON in the context so the LLM can see what it produced and what was wrong
5. Add a test that verifies: invalid JSON → retry with errors → valid JSON on second attempt

### Expected Changes
- `internal/workflow/plan.go` — modify retry loop to pass `invalidTasks` and `validationErrors` to `buildPlanContext()`
- `internal/workflow/engine.go` — modify `buildPlanContext()` to accept and include error feedback
- `internal/workflow/plan_test.go` — add test for retry-with-error-feedback path

---

## Fix 4: Wire Self-Heal in Execute Phase (C4)

### Problem
`healTask()` exists in `execute.go` but is never called from `executeTaskWithTools()`. Failed tasks are not recoverable during execution.

### Files to Read
- `internal/workflow/execute.go` — `executeTaskWithTools()`, `healTask()`
- `internal/workflow/verify.go` — how self-heal is triggered there (for pattern reference)
- `internal/workflow/engine.go` — `MaxHealAttempts` constant

### What to Do
1. In `executeTaskWithTools()`, after a task fails (tool execution error or LLM error), call `healTask()` with:
   - The task spec
   - The error output
   - Current file state (read affected files)
2. `healTask()` attempts to fix the task via LLM
3. If heal succeeds (task completes after fix), mark task as done
4. If heal fails and `HealsAttempted < MaxHealAttempts`, retry heal
5. After `MaxHealAttempts` exceeded, mark `StatusUnrecoverable`
6. Add test for execute-phase self-heal

### Expected Changes
- `internal/workflow/execute.go` — add heal loop in `executeTaskWithTools()` after task failure
- `internal/workflow/execute_test.go` — add test for execute-phase self-heal flow

---

## Fix 5: Track and Pass sessionStartHash to Bisect (C5)

### Problem
Bisect uses hardcoded `HEAD~50` instead of the commit hash at session start.

### Files to Read
- `internal/workflow/verify.go` — where bisect is called (line 108)
- `internal/workflow/engine.go` — Engine struct, `NewEngine()`, `startTime` field
- `internal/git/git.go` — `HeadHash()` method

### What to Do
1. In `NewEngine()`, capture the current git HEAD hash before the session starts: `e.sessionStartHash = e.git.HeadHash()`
2. If not a git repo or HEAD doesn't exist, use a fallback (empty string — bisect skips)
3. Pass `e.sessionStartHash` to `bisect.Run()` instead of `"HEAD~50"`
4. Add `sessionStartHash` field to Engine struct
5. Add test that verifies sessionStartHash is captured and passed

### Expected Changes
- `internal/workflow/engine.go` — add `sessionStartHash` field, capture in `NewEngine()`
- `internal/workflow/verify.go` — pass `e.sessionStartHash` to `bisect.Run()`
- `internal/workflow/verify_test.go` — test with sessionStartHash

---

## Fix 6: Add 3rd Targeted Heal After Bisect (C6)

### Problem
After bisect identifies the offending commit, the diff is logged but never used for a targeted heal.

### Files to Read
- `internal/workflow/verify.go` — where bisect result is handled (lines 110-120)
- `internal/workflow/engine.go` — `healTask()` method

### What to Do
1. After bisect returns `BisectResult{OffendingCommit, Diff}`, construct a targeted heal prompt that includes:
   - The offending commit hash and message
   - The diff of the offending commit
   - The current task spec
   - The failure output
2. Call `healTask()` with this enriched context as a 3rd targeted heal attempt
3. If this heal succeeds, mark task as done
4. If it fails, mark `StatusUnrecoverable`
5. Add test for bisect + targeted heal flow

### Expected Changes
- `internal/workflow/verify.go` — add targeted heal call after bisect succeeds
- `internal/workflow/verify_test.go` — add test for bisect → targeted heal flow

---

## Fix 7: Add Manual Task Entry Fallback After 3 Plan Failures (C7)

### Problem
After 3 plan retries, the engine returns an error with no user fallback.

### Files to Read
- `internal/workflow/plan.go` — end of retry loop (line 62)
- `internal/workflow/engine.go` — `PhaseResult` struct

### What to Do
1. After 3 failed retries, instead of returning an error, return a `PhaseResult` with:
   - `Status: "plan_failed"`
   - `ErrorMessage: "Could not generate valid task list after 3 attempts"`
   - `RequiresManualInput: true` (new field)
2. The TUI can then prompt the user to enter tasks manually or skip to REPL
3. Add a `SubmitManualTasks(tasks []Task)` method for manual entry
4. If user skips, return empty task list and transition to Execute (which will immediately complete)
5. Add test for the fallback path

### Expected Changes
- `internal/workflow/engine.go` — add `RequiresManualInput` field to `PhaseResult`
- `internal/workflow/plan.go` — return structured failure instead of error after 3 retries
- `internal/workflow/plan_test.go` — test the fallback path
- TUI update needed separately (deferred to TUI wiring)

---

## Fix 8: Add PROJECT.md to Execute Context (C9)

### Problem
`buildExecuteContext()` does not include PROJECT.md (goal, project type, framework).

### Files to Read
- `internal/workflow/execute.go` — `buildExecuteContext()` function
- `internal/workflow/discuss.go` — `buildDiscussContext()` for pattern reference (how it loads PROJECT.md)
- `pkg/session/planning.go` — `LoadProject()` method

### What to Do
1. In `buildExecuteContext()`, load PROJECT.md via `e.sessionMgr.LoadProject()` or read from disk
2. Include goal, project type, and framework in the context
3. Format: "## Project Context\nGoal: <goal>\nType: <type>\nFramework: <framework>"
4. Add test that verifies PROJECT.md is included in execute context

### Expected Changes
- `internal/workflow/execute.go` — add PROJECT.md loading and inclusion in `buildExecuteContext()`
- `internal/workflow/execute_test.go` — test context includes project info

---

## Fix 9: Define and Emit PlanReadyMsg (H3)

### Problem
No `PlanReadyMsg` type exists. TUI has no explicit signal that plan is ready for review.

### Files to Read
- `internal/tui/types.go` — existing message types
- `internal/workflow/plan.go` — where plan completes

### What to Do
1. Add `PlanReadyMsg struct { Tasks []types.Task; CostEstimate string; TimeEstimate string }` to `internal/tui/types.go`
2. In the TUI handler for plan phase completion (in `AppState.Update()` when handling `PhaseResultMsg` for plan phase), emit `PlanReadyMsg` with the task list
3. Add handler for `PlanReadyMsg` that transitions to `ScreenPlan`
4. Add test for PlanReadyMsg handling

### Expected Changes
- `internal/tui/types.go` — add `PlanReadyMsg`
- `internal/tui/app.go` — emit and handle `PlanReadyMsg` in plan phase completion handler

---

## Fix 10: Add TaskStartMsg/Callback to TaskRunner (H4)

### Problem
TaskRunner has no streaming callbacks. TUI cannot show live progress.

### Files to Read
- `pkg/taskrunner/runner.go` — `Runner` struct, `ExecuteGroup()`, `Run()` methods
- `internal/tui/types.go` — existing message types

### What to Do
1. Add `OnTaskStart func(task types.Task)` and `OnTaskUpdate func(task types.Task, status string)` callback fields to `Runner` struct
2. In `ExecuteGroup()`, call `OnTaskStart(task)` before executing, and `OnTaskUpdate(task, "streaming")` during execution
3. In the TUI, when creating the runner, set these callbacks to emit `tea.Msg` via a channel
4. Since Bubble Tea is single-threaded, callbacks must send to a channel that is drained in `Update()`
5. Add `TaskStartMsg` and `TaskUpdateMsg` to `types.go`
6. Add test for callback firing

### Expected Changes
- `pkg/taskrunner/runner.go` — add callback fields, call them in `ExecuteGroup()`
- `internal/tui/types.go` — add `TaskStartMsg`, `TaskUpdateMsg`
- `internal/tui/app.go` — set callbacks when creating runner, drain messages in `Update()`
- `pkg/taskrunner/runner_test.go` — test callbacks fire

---

## Fix 11: Add MEMORY.md to Plan Context (H6)

### Problem
`buildPlanContext()` does not load MEMORY.md.

### Files to Read
- `internal/workflow/plan.go` — `buildPlanContext()` function
- `internal/workflow/discuss.go` — how it loads MEMORY.md (for pattern)

### What to Do
1. In `buildPlanContext()`, attempt to read `~/.m31a/MEMORY.md` or `cwd/MEMORY.md`
2. If exists, include in context: "## Cross-Session Memory\n<memory content>"
3. If not exists, omit section
4. Add test

### Expected Changes
- `internal/workflow/plan.go` — add MEMORY.md loading in `buildPlanContext()`
- `internal/workflow/plan_test.go` — test with and without MEMORY.md

---

## Fix 12: Fix Commit Message Format (H7, H8)

### Problem
Commit messages don't match roadmap spec. `git add -A` not used.

### Files to Read
- `internal/workflow/execute.go` — commit call in `executeTaskWithTools()`
- `internal/workflow/ship.go` — commit call
- `internal/git/git.go` — `CommitWithFiles()`, `Commit()`, `AddAll()` methods

### What to Do
1. In `execute.go`, change commit message from `feat(task N): <description>` to `feat: <description>`
2. Before commit, call `e.git.AddAll()` (implement if missing) instead of staging specific files
3. In `ship.go`, change commit message from `chore(ship): complete session <session-id>` to `chore: ship <session-id>`
4. Update tests that check commit message format
5. Add `AddAll()` method to `internal/git/git.go` if not present

### Expected Changes
- `internal/workflow/execute.go` — fix commit message, add `AddAll()` call
- `internal/workflow/ship.go` — fix commit message
- `internal/git/git.go` — add `AddAll()` method if missing
- Tests updated

---

## Fix 13: Write End-to-End Integration Test (C8)

### Problem
No test runs all 6 phases sequentially.

### Files to Read
- `internal/workflow/engine_test.go` — existing test patterns, mock provider
- All `*_test.go` files — understand test setup patterns

### What to Do
1. Create `internal/workflow/integration_test.go`
2. Create a multi-turn mock provider that returns different responses based on the phase:
   - Initialize: returns empty (no LLM needed)
   - Discuss: returns "1. What framework?\n2. What's the scale?"
   - Plan: returns valid JSON task array
   - Execute: returns tool calls + completion
   - Verify: returns verification results
   - Ship: returns empty
3. Create a temp git repo for the test
4. Run: Initialize → Discuss (submit answers) → Plan → Execute → Verify → Ship
5. Assert:
   - All planning files written (PROJECT.md, TASKS.md, STATE.md)
   - Git commits produced
   - Session archived
   - Ledger entry appended
   - No errors at any phase
6. Run with `-race` flag

### Expected Deliverables
- `internal/workflow/integration_test.go` — 1 comprehensive integration test

---

## Implementation Order

Execute fixes in this order (dependencies respected):

1. **Fix 12** — Fix commit message format (quick, no dependencies)
2. **Fix 11** — Add MEMORY.md to plan context (quick)
3. **Fix 8** — Add PROJECT.md to execute context (quick)
4. **Fix 3** — Feed validation errors back to LLM (fixes broken retry loop)
5. **Fix 5** — Track sessionStartHash (needed for Fix 6)
6. **Fix 9** — Define PlanReadyMsg (needed for TUI wiring later)
7. **Fix 2** — Wire Discuss Q&A collection (needed for Fix 1)
8. **Fix 1** — Wire automatic phase transitions (depends on Fix 2)
9. **Fix 4** — Wire self-heal in execute phase
10. **Fix 6** — Add 3rd targeted heal after bisect (depends on Fix 5)
11. **Fix 7** — Add manual task entry fallback
12. **Fix 10** — Add TaskStartMsg to taskrunner
13. **Fix 13** — Write end-to-end integration test (depends on all above)

---

## Verification Commands

After all fixes:

```bash
cd /home/snigdha/Desktop/Helix/M31A

# 1. Build
go mod tidy
CGO_ENABLED=0 go build -o m31a ./cmd/m31a

# 2. Vet
go vet ./...

# 3. Test with race detector
go test -race -count=1 ./...

# 4. Run integration test specifically
go test -race -v -run TestFullWorkflow ./internal/workflow/

# 5. Coverage
go test -race -cover -coverprofile=coverage_phase6.out ./internal/workflow/ ./pkg/taskrunner/ ./pkg/bisect/
go tool cover -func=coverage_phase6.out | tail -5

# 6. Verify no orphaned code
# (Search for functions that are defined but never called)
```

---

## Constraints

- Follow `AGENTS.md` rules strictly
- Bubble Tea single-threaded — all state mutations through `Update()` only
- No CGO
- No telemetry
- No V1.1 features
- Keep changes minimal — fix what's broken, don't redesign
- Do NOT fix Medium or Low gaps — those are deferred
- All existing tests must still pass (no regressions)

---

## Expected Deliverables

1. **Modified files:**
   - `internal/workflow/engine.go` — DiscussState, sessionStartHash, PlanReadyMsg, error feedback in plan context
   - `internal/workflow/initialize.go` — auto-transition to Discuss
   - `internal/workflow/discuss.go` — Q&A collection, auto-transition to Plan
   - `internal/workflow/plan.go` — error feedback in retries, MEMORY.md, manual fallback
   - `internal/workflow/execute.go` — self-heal loop, PROJECT.md in context, commit message fix, git add -A
   - `internal/workflow/verify.go` — sessionStartHash for bisect, targeted heal after bisect
   - `internal/workflow/ship.go` — commit message fix
   - `pkg/taskrunner/runner.go` — TaskStartMsg/TaskUpdateMsg callbacks
   - `internal/git/git.go` — AddAll() method
   - `internal/tui/types.go` — PlanReadyMsg, TaskStartMsg, TaskUpdateMsg
   - `internal/tui/app.go` — message handlers, callback wiring

2. **New files:**
   - `internal/workflow/integration_test.go` — end-to-end integration test

3. **Updated test files:**
   - All `*_test.go` files in `internal/workflow/` and `pkg/taskrunner/` — tests for new functionality

4. **No new dependencies** beyond what's already in go.mod

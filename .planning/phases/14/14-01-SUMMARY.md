---
phase: 14-tui-core-wiring-fixes
plan: 01
subsystem: tui
tags: [d-01, discuss, workflow, deadlock-fix, tui-engine-wiring]
dependency_graph:
  requires: []
  provides:
    - Discuss Q&A flow with timeout-safe advance to Plan
    - WorkflowEngine interface for test injection
  affects:
    - internal/tui/app.go (PhaseDiscuss handler)
    - internal/tui/types.go (new DiscussAnswerTimeoutMsg)
    - internal/tui/commands.go (CommandContext uses interface)
    - internal/workflow/engine.go (DiscussState getter)
tech-stack:
  added: []
  patterns:
    - workflowEngineInterface in tui package for mock injection
    - Tea.Batch for question + timer emission
    - 5-minute time.NewTimer per question with tea.Batch dispatch
key-files:
  created:
    - internal/tui/discuss_questions_test.go (63 lines)
  modified:
    - internal/tui/app.go (+148 / -17)
    - internal/tui/types.go (+7)
    - internal/tui/commands.go (-2 / +1)
    - internal/tui/app_test.go (+249)
    - internal/workflow/engine.go (+8)
decisions:
  - Introduced workflowEngineInterface in tui package so tests can inject
    a mock engine without spinning up a real provider
  - Reordered commits: Task 4 (DiscussState getter) was committed before
    Task 2 to keep the build green at every step (Task 2 references the
    new getter)
  - TestApp_DiscussNoAnswers_AutoAdvancesToPlan verifies the engine was
    wired by checking mock.SetMsgEmitter call count (called eagerly by
    RunPhaseCmd) rather than executing the cmd, which is fragile
metrics:
  duration_seconds: 757
  completed_date: 2026-06-01
  tasks_completed: 5
  files_changed: 6
---

# Phase 14 Plan 01: Discuss Phase Q&A Wiring Summary

Wired the Discuss phase Q&A flow end-to-end so the workflow no longer
deadlocks when the LLM returns clarifying questions. The TUI now
collects answers via the same `QuestionRequestMsg`/`QuestionResponseMsg`
channel used by the `AskUserQuestion` tool, with a 5-minute per-question
timeout that auto-skips and advances to Plan.

## One-Line Summary

Discuss Q&A deadlock fix: emit inline questions, route answers to
`engine.SubmitDiscussAnswer`, finalize-and-advance on completion, or
`SkipDiscuss` on 5-minute timeout.

## Tasks Completed

| # | Name | Commit | Files |
|---|------|--------|-------|
| 1 | Add DiscussAnswerTimeoutMsg and AppState Q&A fields | 7b59e43 | app.go, types.go |
| 4 | Expose Engine.DiscussState getter | fbac67a | workflow/engine.go |
| 2 | Wire PhaseDiscuss Q&A flow | e162d01 | app.go, commands.go |
| 3 | Route QuestionResponseMsg to engine | a044683 | app.go |
| 5 | Add tests | 97be28b | app_test.go, discuss_questions_test.go |

(Note: Task 4 was committed before Task 2 to keep the build green at
every step. Both produce a passing build individually.)

## Key Files Changed

- `internal/tui/app.go` (+148 / -17) — new `workflowEngineInterface`,
  `pendingDiscussAnswers`/`currentDiscussIndex`/`discussQuestionCount`/
  `discussAnswerTimeout` fields, `resetDiscussQA`/`askNextDiscussQuestion`/
  `finalizeDiscussAndAdvance`/`skipDiscussAndAdvance` methods, new
  `PhaseDiscuss` case branch, new `case DiscussAnswerTimeoutMsg` handler,
  updated `QuestionResponseMsg` routing.
- `internal/tui/types.go` (+7) — new `DiscussAnswerTimeoutMsg` type.
- `internal/tui/commands.go` (-2 / +1) — `CommandContext.WorkflowEngine`
  now uses the interface.
- `internal/workflow/engine.go` (+8) — public `DiscussState()` getter
  returning a value copy.
- `internal/tui/app_test.go` (+249) — new `mockWorkflowEngine` test
  helper, `newTestAppForDiscuss` factory, 7 new test cases.
- `internal/tui/discuss_questions_test.go` (+63) — new file with 3
  ReplModel/`DiscussAnswerTimeoutMsg` tests.

## Acceptance Criteria

- [x] `AppState` has `pendingDiscussAnswers`, `currentDiscussIndex`,
      `discussQuestionCount`, `discussAnswerTimeout` fields
- [x] `PhaseDiscuss` handler initializes Q&A state and calls
      `askNextDiscussQuestion` when `NeedsAnswers: true`
- [x] `askNextDiscussQuestion` emits `QuestionRequestMsg` for current
      question and starts 5-min `time.NewTimer`
- [x] `QuestionResponseMsg` routes to
      `engine.SubmitDiscussAnswer(idx, ans)` when
      `pendingDiscussAnswers != nil`
- [x] After all answers collected, `engine.FinalizeDiscuss()` and
      `RunPhaseCmd(m, PhasePlan, goal)` are called
- [x] `DiscussAnswerTimeoutMsg` after 5 minutes calls
      `engine.SkipDiscuss()` and advances to Plan
- [x] `*Engine.DiscussState()` getter returns a value copy of
      `e.discussState`
- [x] All new tests pass; all existing tests still pass
- [x] `go build`, `go vet`, `go test -race` all pass
- [x] The Discuss phase no longer deadlocks — workflow advances within
      5 minutes max

## Test Results

```
go test -count=1 -race ./internal/tui/... -run "TestApp_Discuss|TestApp_QuestionResponse|TestApp_PhaseResultMsg_Discuss|TestApp_ResetDiscussQA|TestReplModel_ShowQuestion|TestDiscussAnswerTimeoutMsg"
ok  	github.com/eshanized/M31A/internal/tui	1.117s
```

```
go test -count=1 -race ./...   # full regression
ok  	github.com/eshanized/M31A/internal/config	1.031s
ok  	github.com/eshanized/M31A/internal/git	1.210s
ok  	github.com/eshanized/M31A/internal/log	1.027s
ok  	github.com/eshanized/M31A/internal/provider	1.045s
ok  	github.com/eshanized/M31A/internal/provider/openrouter	3.540s
ok  	github.com/eshanized/M31A/internal/provider/zen	9.069s
ok  	github.com/eshanized/M31A/internal/tokens	6.618s
ok  	github.com/eshanized/M31A/internal/tools	2.196s
ok  	github.com/eshanized/M31A/internal/tui	1.879s
ok  	github.com/eshanized/M31A/internal/tui/components	1.139s
ok  	github.com/eshanized/M31A/internal/tui/theme	1.012s
ok  	github.com/eshanized/M31A/internal/workflow	1.672s
ok  	github.com/eshanized/M31A/pkg/arbitrage	1.008s
ok  	github.com/eshanized/M31A/pkg/autodream	1.012s
ok  	github.com/eshanized/M31A/pkg/bisect	1.464s
ok  	github.com/eshanized/M31A/pkg/keychain	1.014s
ok  	github.com/eshanized/M31A/pkg/ledger	1.025s
ok  	github.com/eshanized/M31A/pkg/rollback	2.206s
ok  	github.com/eshanized/M31A/pkg/session	1.055s
ok  	github.com/eshanized/M31A/pkg/taskrunner	1.015s
```

```
CGO_ENABLED=0 go build -o /dev/null ./cmd/m31a   # OK
go vet ./...                                      # clean
```

## Deviations from Plan

### Auto-approved Adjustments

**1. Introduced `workflowEngineInterface` in the tui package** —
The plan's `mockWorkflowEngine` was defined as a struct that implements
the engine methods, but `AppState.workflowEngine` was a `*workflow.Engine`
struct pointer. To make the mock assignable to the field, a small
interface capturing the methods the TUI uses was introduced; the real
`*workflow.Engine` satisfies it. The `CommandContext.WorkflowEngine`
field was switched to the same interface for consistency. This is a
small, non-behavioral change that adds testability.

**2. Reordered commits** — Task 4 (DiscussState getter) was committed
before Task 2 (PhaseDiscuss handler) because Task 2 calls
`m.workflowEngine.DiscussState()`. Committing in plan order would have
left an intermediate non-building state.

**3. `TestApp_DiscussNoAnswers_AutoAdvancesToPlan` uses
`mockEngine.SetMsgEmitter` to verify the auto-advance path instead of
executing the returned cmd** — Executing a `tea.Batch` cmd is
asynchronous and racy with the test goroutine; checking the eagerly
called `SetMsgEmitter` (the first thing `RunPhaseCmd` does after
binding the engine) is a more deterministic signal that the engine
was wired. The actual `RunPhase` call is tested in the workflow
package's own tests.

**4. `ReplModel_ShowQuestion_*` tests use a fully-initialized
`ReplModel`** — `ShowQuestion` calls `m.textarea.Focus()`, which
panics on a zero-value ReplModel. Tests construct a ReplModel via
`NewReplModel(th)` to get a valid textarea/viewport.

## Auth Gates

None — no API keys, no external service interactions.

## Known Stubs

None — all paths wire to real implementations or the mock.

## Threat Flags

| Flag | File | Description |
|------|------|-------------|
| threat_flag: trust-boundary | internal/tui/app.go | New interface in tui package (`workflowEngineInterface`) widens the surface that can be plugged into the field. The interface only includes the methods the TUI actually calls, so no privilege escalation possible. |

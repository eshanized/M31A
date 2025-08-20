# Phase 21 Context: TUI Logical Errors and Connectivity Fixes

## Goal
Fix all logical errors, state management issues, and component connectivity problems identified in `rush/tui_logical_errors_and_connectivity_report.md`.

## Reference Material
- **Target Report:** `rush/tui_logical_errors_and_connectivity_report.md`
- **Related Phase:** Phase 20 (Internal Wiring and Logic Fixes) — addresses different issues from `rush/internal_wiring_and_logic_report.md`

## Scope

This phase addresses 11 categories of TUI issues across ~6,500+ lines of TUI code:

### Critical Issues (Wave 1)
1. **State Synchronization Failures** — workflowRunning/currentPhase inconsistency, sessionID propagation, model/provider state divergence
2. **Message Flow Breaks** — StreamMsg during phase transitions, PermissionRequestMsg timing, QuestionRequestMsg coordination
3. **Component Connectivity Gaps** — PlanModel stale data, ExecuteModel missing streaming, VerifyModel empty results, ShipModel incomplete summary
4. **Workflow Phase Transition Issues** — Initialize→Discuss cleanup, Discuss→Plan error handling, Plan→Execute timing, Execute→Verify completion checks, Verify→Ship empty results

### High Priority Issues (Wave 2)
5. **Error Handling Gaps** — StreamErrorMsg during workflow, provider unreachable errors, context exceeded cleanup
6. **Timer and Goroutine Management** — Discuss timeout leaks, health check ticker lifecycle, stream goroutine edge cases
7. **Memory Management Issues** — Message history unbounded growth, thinking blocks cache churn, tool cards cache churn

### Medium Priority Issues (Wave 3)
8. **UI Rendering Inconsistencies** — Header cache staleness, sidebar width propagation, theme change propagation
9. **Configuration Hot-Reload Issues** — Partial config updates, theme config changes, permission config changes
10. **Input Handling Edge Cases** — Slash during streaming, rapid key presses, Ctrl+C during permission modal

### Low Priority Issues (Wave 4)
11. **Accessibility and Usability Issues** — Color contrast, keyboard navigation, screen reader compatibility

## Key Files to Modify

| File | Issues Addressed |
|------|-----------------|
| `internal/tui/app.go` | State sync, timer lifecycle, memory |
| `internal/tui/app_update.go` | Message flow, error handling, config reload |
| `internal/tui/app_update_workflow.go` | Phase transitions, component connectivity |
| `internal/tui/app_workflow.go` | Timer management, phase transitions |
| `internal/tui/repl.go` | State sync, input handling, memory |
| `internal/tui/repl_stream.go` | Message flow, memory |
| `internal/tui/streaming.go` | Timer lifecycle, stream goroutine |
| `internal/tui/plan.go` | Component connectivity |
| `internal/tui/execute.go` | Component connectivity |
| `internal/tui/verify.go` | Component connectivity |
| `internal/tui/ship.go` | Component connectivity |
| `internal/tui/header.go` | UI rendering |
| `internal/tui/components/message.go` | UI rendering, theme propagation |
| `internal/tui/components/toolcard.go` | UI rendering, theme propagation |

## Locked Decisions

1. **No architecture refactor** — This phase fixes bugs, not architectural issues (AppState refactor deferred to v1.1)
2. **Preserve existing interfaces** — All fixes must maintain backward compatibility
3. **Test every fix** — Each critical/high fix gets a regression test
4. **Phase 20 independence** — This phase does not depend on Phase 20 completion

## Verification

After all fixes:
- `go build ./...` passes
- `go vet ./...` zero errors
- `go test -race ./...` passes
- Manual workflow test: Initialize → Discuss → Plan → Execute → Verify → Ship

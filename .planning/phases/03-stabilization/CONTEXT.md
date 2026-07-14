# Phase 3: Stabilization — Context

**Gathered:** 2026-07-14
**Status:** Ready for planning
**Source:** HIGH_PRIORITY_VERIFICATION_REPORT.md

<domain>
## Phase Boundary

This phase resolves ALL blockers discovered during verification of Phases 1 and 2. The objective is production stability — NOT new features. Every issue in the verification report must be investigated and resolved.

Priority order:
1. Critical regressions (REGR-1 to REGR-4)
2. Critical security issues (NEW-1, NEW-2)
3. High severity correctness issues (NEW-3, NEW-4)
4. Lint (5 issues)
5. Vet (1 issue)
6. Missing tests
7. Minor cleanup (NEW-5 through NEW-8, dead code, Elm architecture)

</domain>

<decisions>
## Implementation Decisions

### Fix Duplicate Task Rendering (REGR-1)
- Remove second loop in `renderTasks()` (lines 347-354)
- Consolidate all task rendering into single loop with status-based branching
- Pending tasks get `○` badge with faint/dimmed style

### Fix Data Race in Pause/Resume (REGR-2)
- Capture `e.resumeCh` into local variable under `pauseMu.Lock()`
- Use local variable in select, not `e.resumeCh` directly
- Follows same pattern as `skipCh`, `cancelCh`, `groupCh`

### Fix Double Permission Load (REGR-3)
- Remove second `NewPersistentPermissions()` instantiation (lines 40-46)
- Keep first load via `d.persistentPerms.Load(workDir)` (lines 16-21)
- Dispatcher constructor already initializes `persistentPerms`

### Fix ModeAuto Termination (REGR-4)
- Add explicit `ModeAuto` handling in `nextPhaseForMode()`
- `ModeAuto` follows full workflow path (no phase skipping)
- Same behavior as default path in each case block

### Fix Git Argument Injection (NEW-1)
- Create `validateGitArgs(operation, args)` function
- Per-operation allowlists for safe flags
- Reject unknown/dangerous flags before command construction

### Fix workdir Sandbox Bypass (NEW-2)
- Validate workdir is within `t.workDir` using path prefix check
- Clean path with `filepath.Clean()` before validation
- Reject absolute paths outside project directory

### Fix Relative workdir Resolution (NEW-3)
- Resolve relative workdir against `t.workDir` using `filepath.Join()`
- Ensure `cmd.Dir` is always absolute path

### Fix extractCommitMessage Truncation (NEW-4)
- Collect ALL tokens after `-m` flag, not just first
- Join with spaces to preserve full message

### Fix Lint Issues (LINT-1 through LINT-5)
- Fix copylocks in emitter_stress_test.go (pointer receiver)
- Fix ineffassign in phase_transition_model.go (remove unused `w`)
- Fix staticcheck QF1012 in git.go (use `fmt.Fprintf`)

### Fix Corrupt JSON Handling (NEW-6)
- Log error when `json.Unmarshal` fails in `Save()`
- Back up corrupt file before overwriting
- Return error to caller

### Fix Workflow Mode Race (NEW-7)
- Protect all reads of `e.workflowMode` with `e.mu.RLock()`
- Ensure `SetWorkflowMode` uses `e.mu.Lock()` for writes

### Add esc/q Confirmation (NEW-8)
- Show confirmation dialog when execution is active
- Offer Pause, Cancel, Stay options
- Prevent accidental navigation away from running execution

### Fix Dead Code
- Implement `descriptionKeywords()` to extract keywords from task context
- Complete `isConfigFile` allowlist with missing extensions

### Fix Validation Error Color (NEW-5)
- Change validation error color from green to red
- Match `theme.Error` color used elsewhere in TUI

### Fix ConfigModel.View() Violation
- Move state mutations from `View()` to `Update()`
- `View()` becomes pure function (read-only)

### Add Test Coverage
- Git tool tests (arg validation, commit message, operations)
- Bash workdir tests (validation, rejection, resolution)
- Persistent permissions tests (corrupt JSON, project isolation)
- Engine race tests (concurrent pause/resume)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Verification Report
- `HIGH_PRIORITY_VERIFICATION_REPORT.md` — Single source of truth for all issues

### Source Files
- `internal/tui/execute_model.go` — Duplicate rendering (REGR-1)
- `internal/workflow/engine.go` — Data race (REGR-2), workflow mode race (NEW-7)
- `internal/tools/defaults.go` — Double permission load (REGR-3)
- `internal/tui/app_update_phase.go` — ModeAuto termination (REGR-4)
- `internal/tools/git.go` — Argument injection (NEW-1), commit message truncation (NEW-4)
- `internal/tools/bash.go` — Sandbox bypass (NEW-2), workdir resolution (NEW-3)
- `internal/tools/persistent_permissions.go` — Corrupt JSON (NEW-6)
- `internal/workflow/engine_verify.go` — Dead code
- `internal/tui/config_model.go` — Validation color (NEW-5), Elm violation

### Project Guidelines
- `AGENTS.md` — Build requirements, architecture, code style

</canonical_refs>

<specifics>
## Specific Ideas

### Quality Gates (Post-Phase)
- `make check` must pass (fmt → tidy → vet → lint → test)
- `make lint` must pass (golangci-lint, 5m timeout)
- `make test` must pass (race-enabled tests with coverage)
- `go build ./...` must succeed

### Testing Requirements
Every bug fix must include:
- Unit tests
- Regression tests
- Concurrency tests where applicable
- Security tests where applicable
- Edge-case tests

### Completion Criteria
Continue until every blocker from the verification report is either:
- Fixed
- Verified already fixed
- Rejected with technical justification

Produce STABILIZATION_REPORT.md with:
- Fixed Regressions
- Fixed Security Issues
- Fixed Correctness Issues
- Tests Added
- Lint Status
- Vet Status
- Race Status
- Remaining Risks
- Production Readiness Score
- Recommendation

End with exactly one of:
- READY FOR MEDIUM PRIORITY
- ADDITIONAL STABILIZATION REQUIRED

</specifics>

<deferred>
## Deferred Ideas

- Numeric range validation for settings/config editors (low priority)
- Cursor navigation for skip/cancel in paused state (UI polish)
- Full Elm architecture compliance for all models (non-blocking)

None of these block production readiness.

</deferred>

---

*Phase: 03-stabilization*
*Context gathered: 2026-07-14 via HIGH_PRIORITY_VERIFICATION_REPORT.md*

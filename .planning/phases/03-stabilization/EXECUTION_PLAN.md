# Phase 3: Stabilization — Execution Plan

**Created:** 2026-07-14
**Status:** Ready for Execution
**Phase:** 03-stabilization
**Total Tasks:** 16 across 4 waves

---

## Pre-Execution Checklist

Before starting execution, ensure:

1. **Clean working tree** — Commit or stash uncommitted changes:
   - `internal/tools/git.go` (6 lines changed)
   - `internal/tui/phase_transition_model.go` (14 lines changed)
   - `internal/workflow/engine_verify.go` (18 lines changed)
   - `.planning/STATE.md` (35 lines changed)

2. **Phase directory exists** — `.planning/phases/03-stabilization/` ✓

3. **No SUMMARY.md exists** — Phase 3 is not started ✓

4. **Dependencies met** — Phase 1 and Phase 2 have plans ✓

---

## Wave Structure

### Wave 1: Critical Regressions + Critical Security (Tasks 1-4)
**Parallelization:** TRUE (no file overlaps)
**Estimated time:** 15-25 minutes

| Task | Objective | Files Modified |
|------|-----------|----------------|
| 1 | Fix Duplicate Task Rendering (REGR-1) | `internal/tui/execute_model.go` |
| 2 | Fix Data Race in Pause/Resume (REGR-2) | `internal/workflow/engine.go` |
| 3 | Fix Double Persistent Permissions Load (REGR-3) | `internal/tools/defaults.go` |
| 4 | Fix ModeAuto Silent Termination (REGR-4) | `internal/tui/app_update_phase.go` |

**Wave 1 Acceptance Criteria:**
- `renderTasks()` has exactly ONE loop
- `consumeSkipOrCancel()` captures `resumeCh` under mutex
- `DefaultDispatcher` loads permissions exactly once
- `ModeAuto` follows full workflow path
- `go build ./...` succeeds
- `go test ./... -count=1 -short` passes

---

### Wave 2: Critical Security + High Correctness (Tasks 5-8)
**Parallelization:** TRUE (no file overlaps)
**Estimated time:** 15-25 minutes

| Task | Objective | Files Modified |
|------|-----------|----------------|
| 5 | Add Argument Validation to Git Tool (NEW-1) | `internal/tools/git.go` |
| 6 | Fix workdir Sandbox Bypass (NEW-2) | `internal/tools/bash.go` |
| 7 | Fix Relative workdir Path Resolution (NEW-3) | `internal/tools/bash.go` |
| 8 | Fix extractCommitMessage Truncation (NEW-4) | `internal/tools/git.go` |

**⚠️ INTRA-WAVE OVERLAP DETECTED:**
- Tasks 5 and 8 both modify `internal/tools/git.go`
- Tasks 6 and 7 both modify `internal/tools/bash.go`

**Resolution:** Execute Wave 2 sequentially (not parallel) to avoid file conflicts.

**Wave 2 Acceptance Criteria:**
- `validateGitArgs()` exists and rejects dangerous flags
- `workdir` is validated to be within project directory
- `workdir` is resolved against `t.workDir`
- `extractCommitMessage()` preserves full messages
- `go build ./...` succeeds
- `go test ./internal/tools/... -count=1` passes

---

### Wave 3: Lint + Vet + Medium Issues (Tasks 9-12)
**Parallelization:** TRUE (no file overlaps)
**Estimated time:** 10-15 minutes

| Task | Objective | Files Modified |
|------|-----------|----------------|
| 9 | Fix All Lint Issues (LINT-1 to LINT-5) | `emitter_stress_test.go`, `phase_transition_model.go`, `git.go` |
| 10 | Fix Persistent Permissions Corrupt JSON (NEW-6) | `persistent_permissions.go` |
| 11 | Fix Workflow Mode Race (NEW-7) | `engine.go` |
| 12 | Add esc/q Confirmation (NEW-8) | `execute_model.go` |

**Wave 3 Acceptance Criteria:**
- `golangci-lint run ./...` passes (zero issues)
- `go vet ./...` passes (zero issues)
- Corrupt JSON is backed up before overwrite
- `workflowMode` reads are protected by RLock
- `esc/q` shows confirmation dialog
- `go test ./... -count=1 -short` passes

---

### Wave 4: Dead Code + Cleanup + Tests (Tasks 13-16)
**Parallelization:** TRUE (no file overlaps)
**Estimated time:** 15-20 minutes

| Task | Objective | Files Modified |
|------|-----------|----------------|
| 13 | Fix Dead Code | `engine_verify.go` |
| 14 | Add Test Coverage | `git_test.go`, `bash_test.go`, `persistent_permissions_test.go`, `engine_test.go` |
| 15 | Fix Validation Error Color (NEW-5) | `config_model.go` or `settings_model.go` |
| 16 | Fix ConfigModel.View() Elm Violation | `config_model.go` |

**Wave 4 Acceptance Criteria:**
- `descriptionKeywords()` returns non-nil slice
- `isConfigFile()` recognizes all config extensions
- Test files exist with required test functions
- Validation errors display in red
- `ConfigModel.View()` contains zero state mutations
- `go test ./... -race -count=1` passes

---

## Execution Commands

### Start Execution

```bash
# Option 1: Execute all waves
/gsd-execute-phase 3

# Option 2: Execute specific wave
/gsd-execute-phase 3 --wave 1
/gsd-execute-phase 3 --wave 2
/gsd-exd-phase 3 --wave 3
/gsd-execute-phase 3 --wave 4

# Option 3: Interactive mode (for careful review)
/gsd-execute-phase 3 --interactive
```

### Monitor Progress

```bash
# Check STATE.md for current status
cat .planning/STATE.md

# Check for SUMMARY.md files
ls .planning/phases/03-stabilization/*-SUMMARY.md

# Check git log for phase commits
git log --oneline --grep="03-" | head -20
```

### Verify Completion

```bash
# Full verification suite
make check

# Lint only
make lint

# Tests with race detector
make test

# Build verification
go build ./...
```

---

## Risk Assessment

### High Risk Tasks
1. **Task 2 (REGR-2)** — Data race fix requires careful mutex handling
2. **Task 5 (NEW-1)** — Git argument validation must be comprehensive
3. **Task 6 (NEW-2)** — Sandbox bypass is security-critical

### Medium Risk Tasks
1. **Task 1 (REGR-1)** — Rendering consolidation must preserve all status styles
2. **Task 8 (NEW-4)** — Commit message parser must handle edge cases
3. **Task 11 (NEW-7)** — Race condition fix must not introduce deadlocks

### Low Risk Tasks
1. **Task 3 (REGR-3)** — Simple code removal
2. **Task 4 (REGR-4)** — ModeAuto handling follows existing patterns
3. **Tasks 9-12 (LINT/VET)** — Mechanical fixes

---

## Post-Execution Checklist

After all waves complete:

1. **Run full verification suite:**
   ```bash
   make check
   ```

2. **Check race detector:**
   ```bash
   go test ./... -race -count=1
   ```

3. **Manual verification:**
   - Execute view shows each task exactly once
   - Pause/resume works without deadlock
   - Git tool rejects dangerous flags
   - Bash workdir validates paths correctly
   - Permission modal shows countdown from start
   - Workflow progresses through all phases in ModeAuto

4. **Create STABILIZATION_REPORT.md** with:
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

5. **Update STATE.md** to mark Phase 3 as complete

6. **Update ROADMAP.md** to reflect Phase 3 completion

---

## Task Details

### Task 1: Fix Duplicate Task Rendering (REGR-1)

**File:** `internal/tui/execute_model.go`
**Lines:** 285-357

**Current Issue:**
- First loop (289-345) renders ALL tasks including pending
- Second loop (348-354) renders pending tasks again
- Every pending task appears twice

**Fix:**
- Remove second loop (lines 347-354)
- In first loop, after running-task block (line 324):
  - Add condition: `if task.Status == types.StatusPending || task.Status == ""`
  - Render with faint style and `○` badge
  - `continue`

**Acceptance Criteria:**
- `renderTasks()` contains exactly ONE loop
- Pending tasks render with `○` badge and faint style
- No task appears twice
- Completed tasks still show `✓`
- Failed tasks still show `✗`
- Running tasks still show spinner

---

### Task 2: Fix Data Race in Pause/Resume (REGR-2)

**File:** `internal/workflow/engine.go`
**Lines:** 270-289

**Current Issue:**
- `consumeSkipOrCancel()` reads `e.resumeCh` at line 278 without mutex
- `ResumeExecution()` closes/nils `e.resumeCh` under mutex
- Race condition between unlock (275) and select (277)

**Fix:**
- Inside mutex block (271-275), capture `resumeCh`:
  ```go
  resumeCh := e.resumeCh
  ```
- Use `resumeCh` in select at line 278 instead of `e.resumeCh`

**Acceptance Criteria:**
- `resumeCh` is captured under `pauseMu.Lock()`
- Select uses local variable
- `go vet ./internal/workflow/...` passes
- `go test ./internal/workflow/... -race -count=1` passes
- No deadlock on pause → resume

---

### Task 3: Fix Double Persistent Permissions Load (REGR-3)

**File:** `internal/tools/defaults.go`
**Lines:** 16-46

**Current Issue:**
- Lines 16-21: Load via `d.persistentPerms.Load(workDir)`
- Lines 40-46: Create new instance and load again
- Rules are duplicated

**Fix:**
- Remove lines 40-46 entirely
- Keep lines 16-21 (dispatcher's own persistentPerms)

**Acceptance Criteria:**
- Exactly ONE `persistentPerms.Load()` call
- No second `NewPersistentPermissions()` in `DefaultDispatcher`
- No duplicate rules
- `go test ./internal/tools/... -count=1` passes

---

### Task 4: Fix ModeAuto Silent Termination (REGR-4)

**File:** `internal/tui/app_update_phase.go`
**Lines:** 17-53

**Current Issue:**
- `ModeAuto` has no case in mode switch
- Falls through to `return types.PhaseIdle, false`
- Workflow terminates silently

**Fix:**
- Add `ModeAuto` handling in each case block
- `ModeAuto` follows full workflow path (no skipping)
- Same behavior as default path

**Acceptance Criteria:**
- `ModeAuto` is handled in all phase transitions
- `ModeAuto` does NOT skip phases
- Workflow progresses through all phases
- `go build ./...` succeeds

---

### Task 5: Add Argument Validation to Git Tool (NEW-1)

**File:** `internal/tools/git.go`

**Current Issue:**
- `args` parameter split with `strings.Fields()`
- Appended directly to git commands
- Dangerous flags like `--force`, `--no-index`, `-D` can be passed

**Fix:**
- Create `validateGitArgs(operation string, args []string) error`
- Per-operation allowlists:
  - `add`: `-f`, `-p`, `-v`, `-n`, `--dry-run`, `--verbose`
  - `commit`: `-m`, `-a`, `--amend`, `--no-edit`, `--allow-empty`
  - `diff`: `--stat`, `--name-only`, `--name-status`, `--cached`
  - `log`: `--oneline`, `--graph`, `--all`, `--stat`, `-n`
  - `branch`: `-d`, `-m`, `-M`, `-r`, `-a`, `--list`
  - `checkout`: `-b`, `-B`, `--orphan`
  - `stash`: `push`, `pop`, `apply`, `drop`, `list`, `show`
  - `status`: `--short`, `--branch`, `--porcelain`
- Call validation before command construction

**Acceptance Criteria:**
- `validateGitArgs()` exists
- `Execute()` calls validation
- `git checkout --force` is rejected
- `git diff --no-index /etc/passwd` is rejected
- `git commit -m "fix bug"` is accepted
- `go test ./internal/tools/... -count=1` passes

---

### Task 6: Fix workdir Sandbox Bypass (NEW-2)

**File:** `internal/tools/bash.go`
**Lines:** 122-125

**Current Issue:**
- User-supplied `workdir` passed to `applyBashSandbox()`
- Can add `/etc` to sandbox allowlist
- Neutralizes defense-in-depth

**Fix:**
- Clean path: `filepath.Clean(workdirRaw)`
- Validate within `t.workDir`: `strings.HasPrefix(cleaned, filepath.Clean(t.workDir))`
- Validate exists: `os.Stat(cleaned)`
- Reject if validation fails

**Acceptance Criteria:**
- `workdirRaw` is cleaned
- Path is validated to be within project
- `applyBashSandbox()` receives valid path
- `workdir="/etc"` returns error
- `go test ./internal/tools/... -count=1` passes

---

### Task 7: Fix Relative workdir Path Resolution (NEW-3)

**File:** `internal/tools/bash.go`
**Line:** 124

**Current Issue:**
- `cmd.Dir = workdirRaw` uses raw string
- Go resolves relative paths against process CWD
- Contradicts documentation

**Fix:**
- Check `filepath.IsAbs(cleaned)`
- If relative: `cmd.Dir = filepath.Join(t.workDir, cleaned)`
- If absolute: use directly (after validation)
- Ensure always absolute: `filepath.Abs()`

**Acceptance Criteria:**
- Relative paths resolve against `t.workDir`
- `filepath.IsAbs()` check determines resolution
- `cmd.Dir` is always absolute
- `workdir="subdir"` resolves to `t.workDir/subdir`
- `go build ./...` succeeds

---

### Task 8: Fix extractCommitMessage Truncation (NEW-4)

**File:** `internal/tools/git.go`
**Lines:** 527-554

**Current Issue:**
- `strings.Fields()` splits on spaces
- Only takes first token after `-m`
- `git commit -m fix bug` captures only `fix`

**Fix:**
- After finding `-m` at index `i`
- Collect ALL tokens: `fields[i+1:]`
- Join with spaces: `strings.Join(fields[i+1:], " ")`
- Strip surrounding quotes

**Acceptance Criteria:**
- `extractCommitMessage("-m fix bug")` returns `"fix bug"`
- `extractCommitMessage("-m 'fix bug'")` returns `"fix bug"`
- `extractCommitMessage("")` returns `""`
- `go test ./internal/tools/... -count=1` passes

---

### Task 9: Fix All Lint Issues (LINT-1 to LINT-5)

**Files:**
- `internal/tools/emitter_stress_test.go` (copylocks)
- `internal/tui/phase_transition_model.go` (ineffassign)
- `internal/tools/git.go` (staticcheck QF1012)

**LINT-1 (copylocks):**
- `emitter_stress_test.go:61` — `StreamingMockProvider` contains `sync.Mutex`
- Fix: Change to pointer receiver

**LINT-2 (ineffassign):**
- `phase_transition_model.go:127` — `w` assigned but never read
- Fix: Remove unused variable

**LINT-3, LINT-4, LINT-5 (staticcheck):**
- `git.go:485,487,491` — `WriteString(fmt.Sprintf(...))`
- Fix: Use `fmt.Fprintf(&sb, ...)`

**Acceptance Criteria:**
- `go vet ./...` passes
- `golangci-lint run ./...` passes
- No copylocks violations
- No ineffassign warnings
- No staticcheck warnings

---

### Task 10: Fix Persistent Permissions Corrupt JSON (NEW-6)

**File:** `internal/tools/persistent_permissions.go`
**Lines:** 66-71

**Current Issue:**
- `Save()` reads existing data
- Silently discards `json.Unmarshal` errors
- Overwrites corrupt file with only new rules

**Fix:**
- Log error when unmarshal fails
- Back up corrupt file: `permissions.json.corrupt.{timestamp}`
- Return error to caller

**Acceptance Criteria:**
- Error is logged on corrupt JSON
- Corrupt file is backed up
- Error is returned to caller
- Other projects' rules preserved
- `go test ./internal/tools/... -count=1` passes

---

### Task 11: Fix Workflow Mode Race (NEW-7)

**File:** `internal/workflow/engine.go`
**Line:** 876

**Current Issue:**
- `e.workflowMode` read without `RLock`
- `SetWorkflowMode` writes under `Lock`
- Data race between TUI and workflow goroutines

**Fix:**
- Protect all reads with `e.mu.RLock()`
- Ensure writes use `e.mu.Lock()`
- Check all other reads of `e.workflowMode`

**Acceptance Criteria:**
- All reads protected by `RLock`
- Writes use `Lock`
- `go vet ./...` passes
- `go test ./internal/workflow/... -race -count=1` passes

---

### Task 12: Add esc/q Confirmation (NEW-8)

**File:** `internal/tui/execute_model.go`
**Lines:** 199-202

**Current Issue:**
- `esc`/`q` during active execution navigates away
- No pause or confirmation
- Execution continues silently

**Fix:**
- Intercept `esc`/`q` when execution is active
- Show confirmation dialog
- Offer: Pause, Cancel, Stay
- Handle user choice

**Acceptance Criteria:**
- `esc`/`q` shows confirmation dialog
- Dialog offers Pause, Cancel, Stay
- Pause pauses execution
- Cancel cancels execution
- Stay remains on screen
- `go test ./internal/tui/... -count=1` passes

---

### Task 13: Fix Dead Code

**File:** `internal/workflow/engine_verify.go`

**13a. descriptionKeywords() (lines 85-89):**
- Always returns nil
- Fix: Extract capitalized words from task context
- Use simple heuristic: split on spaces, filter >3 chars, lowercase

**13b. isConfigFile allowlist (lines 506-517):**
- Incomplete list
- Add: `.env.example`, `.toml`, `.yaml`, `.yml`, `.json`, `.xml`, `.ini`, `.cfg`, `.conf`

**Acceptance Criteria:**
- `descriptionKeywords()` returns non-nil slice
- `isConfigFile()` recognizes all config extensions
- `go test ./internal/workflow/... -count=1` passes

---

### Task 14: Add Test Coverage

**New Test Files:**
- `internal/tools/git_test.go`
- `internal/tools/bash_test.go` (additions)
- `internal/tools/persistent_permissions_test.go`
- `internal/workflow/engine_test.go` (additions)

**Required Tests:**
1. `TestGit_ArgValidation` — validateGitArgs rejects dangerous flags
2. `TestGit_ExtractCommitMessage` — full message preservation
3. `TestGit_Operations` — basic operation tests
4. `TestBash_WorkdirValidation` — workdir within project accepted
5. `TestBash_WorkdirRejectAbsolute` — `/etc` rejected
6. `TestBash_WorkdirRelativeResolution` — relative paths resolve correctly
7. `TestPersistentPermissions_CorruptJSON` — corrupt file backed up
8. `TestPersistentPermissions_SavePreservesOtherProjects` — project isolation
9. `TestEngine_ConcurrentPauseResume` — no race under concurrency

**Acceptance Criteria:**
- All test files exist
- All test functions exist
- `go test ./internal/tools/... -race -count=1` passes
- `go test ./internal/workflow/... -race -count=1` passes
- Coverage for `git.go` > 60%
- Coverage for `bash.go` workdir paths > 70%

---

### Task 15: Fix Validation Error Color (NEW-5)

**File:** `internal/tui/config_model.go` or `internal/tui/settings_model.go`

**Current Issue:**
- Validation error messages display in green
- Should display in red (error color)

**Fix:**
- Find color assignment for validation errors
- Change from green to red
- Match `theme.Error` color

**Acceptance Criteria:**
- Validation errors display in red
- Color matches `theme.Error`
- `go test ./internal/tui/... -count=1` passes

---

### Task 16: Fix ConfigModel.View() Elm Violation

**File:** `internal/tui/config_model.go`

**Current Issue:**
- `View()` mutates viewport state
- Bubble Tea requires `View()` to be pure

**Fix:**
- Move state mutations from `View()` to `Update()`
- `View()` only reads and renders
- `renderFields()` does not modify model

**Acceptance Criteria:**
- `View()` contains zero state mutations
- `renderFields()` does not modify model
- All changes in `Update()`
- `go test ./internal/tui/... -count=1` passes
- TUI renders correctly (manual test)

---

## Quality Gates

### Pre-Execution
- [ ] Clean working tree (commit or stash changes)
- [ ] Phase directory exists
- [ ] No SUMMARY.md exists
- [ ] Dependencies met (Phase 1, Phase 2)

### Post-Wave 1
- [ ] `renderTasks()` has ONE loop
- [ ] `consumeSkipOrCancel()` captures `resumeCh` under mutex
- [ ] `DefaultDispatcher` loads permissions once
- [ ] `ModeAuto` follows full workflow
- [ ] `go build ./...` succeeds
- [ ] `go test ./... -count=1 -short` passes

### Post-Wave 2
- [ ] `validateGitArgs()` exists and works
- [ ] `workdir` is validated and resolved correctly
- [ ] `extractCommitMessage()` preserves full messages
- [ ] `go build ./...` succeeds
- [ ] `go test ./internal/tools/... -count=1` passes

### Post-Wave 3
- [ ] `golangci-lint run ./...` passes
- [ ] `go vet ./...` passes
- [ ] Corrupt JSON is backed up
- [ ] `workflowMode` reads are protected
- [ ] `esc/q` shows confirmation
- [ ] `go test ./... -count=1 -short` passes

### Post-Wave 4
- [ ] `descriptionKeywords()` returns keywords
- [ ] `isConfigFile()` recognizes all extensions
- [ ] All test files exist with required tests
- [ ] Validation errors display in red
- [ ] `ConfigModel.View()` is pure
- [ ] `go test ./... -race -count=1` passes

### Final Verification
- [ ] `make check` passes
- [ ] `make lint` passes
- [ ] `make test` passes
- [ ] `go build ./...` succeeds
- [ ] Manual verification complete
- [ ] STABILIZATION_REPORT.md created
- [ ] STATE.md updated
- [ ] ROADMAP.md updated

---

## Troubleshooting

### Common Issues

1. **Build fails after Wave 1:**
   - Check for syntax errors in modified files
   - Verify all imports are present
   - Run `goimports -w .`

2. **Tests fail after Wave 2:**
   - Check argument validation logic
   - Verify workdir path resolution
   - Check commit message parser edge cases

3. **Lint fails after Wave 3:**
   - Run `golangci-lint run ./...` to see specific issues
   - Fix copylocks with pointer receivers
   - Remove unused variables
   - Use `fmt.Fprintf` instead of `WriteString(fmt.Sprintf(...))`

4. **Race detector fails:**
   - Check mutex usage in engine.go
   - Verify all channel captures are under lock
   - Use `go test -race -count=1` to identify races

### Recovery Options

1. **Wave fails mid-execution:**
   - Check git status for partial commits
   - Review executor logs
   - Fix issues and re-run failed wave

2. **Verification fails:**
   - Review STABILIZATION_REPORT.md
   - Address remaining issues
   - Re-run verification

3. **Need to restart:**
   - `git stash` uncommitted changes
   - Re-run from failed wave
   - Or start from beginning with `--wave 1`

---

## Notes

- All tasks follow AGENTS.md conventions
- No new features allowed — only stabilization
- Every fix must include tests
- Documentation updates only if behavior changes
- No TODO comments or suppressed warnings
- Fix root causes, not symptoms

---

*Execution Plan created: 2026-07-14*
*Ready for execution via `/gsd-execute-phase 3`*

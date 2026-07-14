# HIGH_PRIORITY_VERIFICATION_REPORT.md

**Date:** 2026-07-14
**Reviewer:** Release Verification Lead
**Scope:** All Critical (C1-C4) and High Priority (H1-H10) fixes

---

## Executive Summary

All 14 issues (C1-C4, H1-H10) are **implemented** and **functional**. However, the verification uncovered **4 critical/blocker issues**, **5 high-severity issues**, and **8 medium-severity issues** that collectively prevent this codebase from being production-ready. The most severe problems are: data races in the pause/resume system, duplicate rendering in the execute view, double-loading of persistent permissions, and a sandbox bypass in the Bash tool's workdir parameter.

**Verdict: NOT ready to ship. Resolve remaining high priority issues first.**

---

## Verification Status

| Check | Status |
|-------|--------|
| Build (go build) | PASS (after `gofmt`) |
| go vet | FAIL — 1 copylocks violation |
| golangci-lint | FAIL — 5 issues (1 govet, 1 ineffassign, 3 staticcheck) |
| go test ./... -short | PASS — all packages pass |
| All 14 fixes implemented | YES |
| All 14 fixes correct | NO — 4 critical defects found |

---

## Verified Fixes

### C1: Bash Syntax Blocking (PASS)
- `$(`, `${`, and backtick removed from obfuscation blocklist
- `bash.go:473-486` confirmed correct

### C2: Installer URL (PASS)
- `install.sh:64` uses lowercase URL with `v` prefix matching goreleaser output

### C3: Headless Mode (PASS)
- `cmd/m31a/main.go:55-134` implements full `--goal` headless workflow execution

### C4: Permission Timeout (PASS)
- `permission.go:139-143` shows countdown from start of 10-minute period

### H1: Dedicated Git Tool (CONDITIONAL PASS)
- `git.go` — 554 lines, 8 operations (add, commit, diff, log, branch, checkout, stash, status)
- Registered in `defaults.go:130`
- **Issues found:** argument injection risk, `extractCommitMessage` truncates multi-word messages, no test coverage

### H2: Bash workdir Parameter (CONDITIONAL PASS)
- `bash.go:67,123-124` — optional workdir parameter added
- **Issues found:** sandbox bypass (CR-01), relative path resolution mismatch (CR-02), no validation

### H3: Settings Unification (CONDITIONAL PASS)
- `settings_model.go:611` — "Showing common options" note present
- Config includes nvidia in provider choices
- **Issues found:** provider ordering differs between views, color-detection bug shows validation errors in green

### H4: Input Validation (CONDITIONAL PASS)
- Toast notifications for validation errors confirmed
- **Issues found:** no range checks, password fields cannot be cleared, choice fields show no error for unrecognized values

### H5: Phase Transition Confirmation (PASS)
- `phase_transition_model.go` — 224 lines, correctly integrated for Discuss->Plan, Execute->Verify, Verify->Runtime
- Router, view, and message handling all wire up properly

### H6: Pause/Resume Execution (CONDITIONAL PASS)
- `execute_model.go:174-176` — pause toggle works
- **Issues found:** duplicate task rendering, esc/q navigates away without pausing, skip/cancel only works on current task

### H7: Verification Checks (CONDITIONAL PASS)
- `engine_verify.go:315-341` — configurable build_command, test_command, lint_command
- **Issues found:** `descriptionKeywords()` always returns nil (dead code), incomplete isConfigFile allowlist

### H8: Running Cost/Time Display (PASS)
- `/cost` command and `GetCostInfo()` confirmed in sidebar

### H9: Persistent Permission Saving (CONDITIONAL PASS)
- `persistent_permissions.go:55-88` — Save() writes to ~/.m31a/permissions.json
- **Issues found:** double-loading in defaults.go, corrupt JSON silently overwrites all projects

### H10: Workflow Mode Display (CONDITIONAL PASS)
- `app_update_commands.go:456` — shows classification result
- **Issues found:** `ModeAuto` silently terminates workflow, no `/mode` command for runtime switching

---

## Partial Fixes

None. All fixes are implemented. The issues are correctness/quality defects within the implementations.

---

## Regressions Found

### REGR-1: Duplicate Task Rendering (execute_model.go:289-354)
**Severity: CRITICAL**
`renderTasks()` has two loops. The first loop (lines 289-345) renders ALL non-done/non-failed tasks (including pending) with badges. The second loop (lines 348-354) renders pending tasks again with dimmed style. Every pending task appears twice in the viewport.

### REGR-2: Data Race in Pause/Resume (engine.go:270-289)
**Severity: CRITICAL**
`consumeSkipOrCancel` reads `e.resumeCh` at line 278 without the mutex. `ResumeExecution()` closes/nils `e.resumeCh` under the mutex. If resume happens between the unlock (line 275) and the select (line 277), `e.resumeCh` becomes nil and the goroutine deadlocks forever.

### REGR-3: Double Persistent Permissions Load (defaults.go:16-46)
**Severity: CRITICAL**
Persistent permissions are loaded twice during `DefaultDispatcher` initialization:
1. Lines 16-21: via `d.persistentPerms.Load(workDir)` (dispatcher already initialized `persistentPerms` in constructor)
2. Lines 40-46: via a new `PersistentPermissions()` instance, appending the same rules again

Every rule is duplicated in the rules slice, causing double-matching or double auto-approvals.

### REGR-4: ModeAuto Silent Termination (app_update_phase.go:17-53)
**Severity: HIGH**
`ModeAuto` (the default workflow mode) hits no case in the mode switch, silently terminating the workflow without executing any phase.

---

## New Issues Discovered

### NEW-1: Argument Injection in Git Tool (git.go)
**Severity: CRITICAL**
The `args` parameter from LLM input is split with `strings.Fields()` and appended directly to git commands. While `exec.CommandContext` prevents shell injection, an attacker could pass `--force` to checkout, `--no-index /etc/passwd` to diff, or `-D` to force-delete branches. Args should be validated against an allowlist of safe flags per operation.

### NEW-2: Sandbox Bypass via workdir (bash.go:122-125)
**Severity: CRITICAL**
The user-supplied `workdir` value is passed directly to `applyBashSandbox()`, which injects it into Landlock allowed paths (Linux) or sandbox-exec profile (macOS). A `workdir` of `/etc` would add that directory to the OS-level sandbox's allow list, neutralizing the defense-in-depth layer.

### NEW-3: Relative workdir Path Resolution Mismatch (bash.go:124)
**Severity: HIGH**
`cmd.Dir = workdirRaw` assigns the raw string. Go resolves relative `Dir` paths against the calling process's CWD, not `t.workDir` — contradicting the schema documentation which states "If relative, resolved from the default working directory."

### NEW-4: extractCommitMessage Truncation (git.go:527-554)
**Severity: HIGH**
`extractCommitMessage` silently truncates multi-word unquoted commit messages. `git commit -m fix bug` captures only `fix`. The parser splits on spaces and only takes the first token after `-m`.

### NEW-5: Validation Error Color Bug (settings_model.go / config_model.go)
**Severity: MEDIUM**
Input validation error messages display in green due to a color-detection bug. Users see green text for errors, which is UX-inverted.

### NEW-6: Persistent Permissions Corrupt JSON Overwrite (persistent_permissions.go:68-71)
**Severity: MEDIUM**
When `json.Unmarshal` fails on existing permissions file (corrupt JSON), the error is silently discarded. The subsequent `Save()` overwrites the file with only the new project's rules, permanently deleting all other projects' permissions.

### NEW-7: Workflow Mode Race (engine.go:876)
**Severity: MEDIUM**
`e.workflowMode` is read without RLock at line 876 while `SetWorkflowMode` writes under Lock. Potential data race between TUI and workflow goroutines.

### NEW-8: esc/q Exits Execution Without Pause (execute_model.go:199-202)
**Severity: MEDIUM**
Pressing `esc` or `q` during active execution navigates away without pausing or confirmation. Execution continues silently in the background.

---

## Architecture Concerns

1. **Bubble Tea Elm Architecture Violation:** `ConfigModel.View()` mutates viewport state (width, height, content, scroll offset) during rendering via `renderFields()`. View should be pure; all mutations belong in `Update()`.

2. **Pause/Resume Complexity:** The pause/resume system introduces channels (`resumeCh`, `skipTaskCh`, `cancelTaskCh`, `cancelGroupCh`) protected by `pauseMu`. The data race at `engine.go:278` shows this locking discipline is incomplete. The channel-based approach creates hard-to-test timing dependencies.

3. **Persistent Permissions Architecture:** The permissions system has two loading paths (`dispatcher.go:88` and `defaults.go:16-21` and `defaults.go:40-46`), creating the double-load bug. This suggests the initialization sequence was not designed holistically.

---

## Code Quality Review

### Lint Issues (5 total, blocking)
1. `emitter_stress_test.go:61` — copylocks: `StreamingMockProvider` contains `sync.Mutex`, passed by value
2. `phase_transition_model.go:127` — ineffassign: `w` assigned but never read
3. `git.go:485` — staticcheck QF1012: `WriteString(fmt.Sprintf(...))` should be `fmt.Fprintf`
4. `git.go:487` — staticcheck QF1012: same pattern
5. `git.go:491` — staticcheck QF1012: same pattern

### Dead Code
- `engine_verify.go:85-89` — `descriptionKeywords()` always returns nil, making smart truncation a no-op
- `engine_verify.go:506-517` — `isConfigFile` allowlist is incomplete (missing `.env.example`, `.toml`, `.yaml`)

### Missing Input Validation
- `bash.go` workdir — no path existence check, no path traversal prevention, no `filepath.Clean`
- `git.go` args — no flag allowlist per operation
- Settings numeric fields — no range checks (only format checks)

---

## Testing Review

### Test Results
- **All packages pass** `go test ./... -count=1 -short`
- **1 vet failure** in test code (copylocks in emitter_stress_test.go)

### Missing Test Coverage
| Feature | Test Coverage | Risk |
|---------|--------------|------|
| Git tool (git.go) | **0 tests** | HIGH — 554 lines, 8 operations, security-sensitive |
| Phase transition model | **0 tests** | MEDIUM — 224 lines, user-facing |
| Bash workdir parameter | **0 tests** | HIGH — security-sensitive, sandbox interaction |
| Persistent permissions Save() | **0 tests** | MEDIUM — file I/O, data integrity |
| Pause/Resume execution flow | **0 tests** | HIGH — concurrent, race-prone |
| Settings/Config input validation | **0 tests** | LOW — UI-only |

### Test Quality Issues
- `dispatcher_test.go:618-630` — duplicate assertions in `TestCheckPermission_ToolFilter`

---

## Documentation Review

- **AGENTS.md:** Accurate and current. Correctly documents build requirements, architecture, and code style.
- **TESTING.md:** Accurate with correct commands and coverage targets.
- **CHANGELOG.md:** v1.7.0 properly documents all changes.
- **DX_AUDIT.md:** Original audit findings are comprehensive and well-organized.
- **Phase plans:** All 10 H-issues have corresponding plan tasks with acceptance criteria.

No stale or misleading documentation found.

---

## Production Readiness

| Criterion | Status |
|-----------|--------|
| Compiles | YES |
| All tests pass | YES |
| Lint clean | NO — 5 issues |
| Vet clean | NO — 1 issue |
| No data races | NO — 2 confirmed |
| No security vulnerabilities | NO — 2 confirmed (arg injection, sandbox bypass) |
| No duplicate rendering | NO — 1 confirmed |
| No dead code | NO — 2 instances |
| Error handling complete | NO — silent discards, corrupt data overwrites |
| Input validation complete | NO — missing ranges, path validation, flag allowlists |

---

## Release Readiness Score: 4/10

**Rationale:** All 14 fixes are implemented and functional at a basic level. However, 4 critical defects (data race, duplicate rendering, double permission load, mode termination), 2 security issues (arg injection, sandbox bypass), and 5 high-severity issues make this unsuitable for production. The codebase would regress on multiple user-facing behaviors if shipped as-is.

---

## Remaining Blockers

### Must Fix Before Production (Critical)
1. **Fix duplicate task rendering** in `execute_model.go:289-354` — second loop re-renders pending tasks
2. **Fix data race in pause/resume** in `engine.go:270-289` — channel read without mutex
3. **Fix double persistent permissions load** in `defaults.go:16-46` — remove redundant load path
4. **Fix ModeAuto termination** in `app_update_phase.go:17-53` — handle default mode case

### Must Fix Before Production (High)
5. **Add arg validation to Git tool** — whitelist safe flags per operation
6. **Fix workdir sandbox bypass** — validate/sanitize workdir before passing to sandbox
7. **Fix relative workdir path resolution** — resolve against `t.workDir`, not process CWD
8. **Fix extractCommitMessage** — preserve full commit message, not just first word
9. **Fix `ineffassign` in phase_transition_model.go:127** — remove unused `w` variable
10. **Fix lint issues** — 3 staticcheck warnings in git.go, 1 copylocks in test

### Should Fix (Medium)
11. Fix validation error color (green instead of red)
12. Fix persistent permissions corrupt JSON handling
13. Fix workflow mode race condition
14. Add esc/q confirmation before exiting active execution

### Recommended Follow-ups
15. Add test coverage for Git tool, phase transitions, workdir, permissions, pause/resume
16. Fix `descriptionKeywords()` dead code in engine_verify.go
17. Fix ConfigModel.View() Elm architecture violation
18. Add numeric range validation to settings/config editors
19. Add cursor navigation for skip/cancel in paused state

---

## Recommended Next Phase

**Resolve Remaining High Priority Issues First**

The 4 critical blockers and 5 high-severity issues identified above must be resolved before moving to Medium Priority work. The data race, duplicate rendering, and double permission load are functional regressions that would ship as bugs. The security issues (argument injection, sandbox bypass) are exploitable via prompt injection.

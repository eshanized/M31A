---
wave: 1
depends_on: ["01-critical-issues", "02-high-priority-issues"]
files_modified:
  - internal/tui/execute_model.go
  - internal/workflow/engine.go
  - internal/tools/defaults.go
  - internal/tui/app_update_phase.go
  - internal/tools/git.go
  - internal/tools/bash.go
  - internal/tools/persistent_permissions.go
  - internal/workflow/engine_verify.go
  - internal/tui/config_model.go
  - internal/tui/settings_model.go
  - internal/tools/emitter_stress_test.go
  - internal/tui/phase_transition_model.go
autonomous: false
---

<planning_context>
**Phase:** 3
**Mode:** standard

<files_to_read>
- .planning/STATE.md (Project State)
- .planning/ROADMAP.md (Roadmap)
- HIGH_PRIORITY_VERIFICATION_REPORT.md (Requirements - single source of truth)
- .planning/phases/01-critical-issues/CONTEXT.md (USER DECISIONS from discuss-phase)
</files_to_read>

<agent_skills_planner>
- gsd-planner
</agent_skills_planner>

<review_incorporation_contract>
**If Mode is reviews:** REVIEWS.md is feedback input, not a hidden execution contract. /gsd-execute-phase primarily consumes PLAN.md plus the normal phase context, so every current actionable review finding must become visible in the relevant PLAN.md before planning can pass.

For each current actionable finding in REVIEWS.md, the planner MUST either:
- incorporate it into a PLAN.md task, `<action>`, `<acceptance_criteria>`, `<verify>`, `must_haves`, threat model, or artifact list; or
- explicitly document a deferral/rejection rationale in the relevant PLAN.md so the executor and reviewer can see the decision.

Historical findings already incorporated, explicitly deferred/rejected in PLAN.md, or marked fully resolved do not require new plan changes.
</review_incorporation_contract>

**Phase requirement IDs (every ID MUST appear in a plan's `requirements` field):** REGR-1, REGR-2, REGR-3, REGR-4, NEW-1, NEW-2, NEW-3, NEW-4, NEW-5, NEW-6, NEW-7, NEW-8, LINT-1, LINT-2, LINT-3, LINT-4, LINT-5, VET-1

**Project instructions:** Read ./AGENTS.md or ./.opencode/AGENTS.md if either exists — follow project-specific guidelines
**Project skills:** Check .claude/skills/ or .agents/skills/ directory (if either exists) — read SKILL.md files, plans should account for project skill rules

**MVP_MODE:** false
**WALKING_SKELETON:** false
**Granularity:** fine
</planning_context>

<downstream_consumer>
Output consumed by /gsd-execute-phase. Plans need:
- Frontmatter (wave, depends_on, files_modified, autonomous)
- Tasks in XML format with read_first and acceptance_criteria fields (MANDATORY on every task)
- Verification criteria
- must_haves for goal-backward verification
- "Artifacts this phase produces" section (MANDATORY) — list every symbol this phase creates: decorators, classes, functions, CLI flags, struct/dataclass fields, new file paths
</downstream_consumer>

<deep_work_rules>
## Anti-Shallow Execution Rules (MANDATORY)

Every task MUST include these fields — they are NOT optional:

1. **`<read_first>`** — Files the executor MUST read before touching anything. Always include:
   - The file being modified (so executor sees current state, not assumptions)
   - Any "source of truth" file referenced in CONTEXT.md (reference implementations, existing patterns, config files, schemas)
   - Any file whose patterns, signatures, types, or conventions must be replicated or respected

2. **`<acceptance_criteria>`** — Verifiable conditions that prove the task was done correctly. Rules:
   - Every criterion must be checkable as a source assertion, behavior assertion, test command, or CLI output
   - NEVER use subjective language ("looks correct", "properly configured", "consistent with")
   - Include exact strings, patterns, values, command outputs, or observable behavior where that is the right proof
   - Examples:
     - Code: `auth.py contains def verify_token(` / `test_auth.py exits 0`
     - Behavior: `POST /api/auth/login returns 200 + httpOnly JWT cookie for valid credentials`
     - Config: `.env.example contains DATABASE_URL=` / `Dockerfile contains HEALTHCHECK`
     - Docs: `README.md contains '## Installation'` / `API.md lists all endpoints`
     - Infra: `deploy.yml has rollback step` / `docker-compose.yml has healthcheck for db`

3. **`<action>`** — Must include CONCRETE values, not references. Rules:
   - NEVER say "align X with Y", "match X to Y", "update to be consistent" without specifying the exact target state
   - Include concrete identifiers and reference values: config keys, function signatures, SQL table names, class names, import paths, env vars, endpoint paths, etc.
   - If CONTEXT.md has a comparison table or expected values, copy only the target identifiers/values needed to remove ambiguity
   - Do not include full file contents, fenced code blocks, or complete implementations in `<action>`
   - The executor should understand the intended target state from `<action>` and use `<read_first>` files for current implementation details, patterns, and source-of-truth context

**Why this matters:** Executor agents work from the plan text. Vague instructions like "update the config to match production" produce shallow one-line changes. Concrete instructions like "add DATABASE_URL, set POOL_SIZE=20, add REDIS_URL, and read config/runtime.ts before editing" produce complete work without turning the planner into the executor.
</deep_work_rules>

<quality_gate>
- [ ] PLAN.md files created in phase directory
- [ ] Each plan has valid frontmatter
- [ ] Tasks are specific and actionable
- [ ] Every task has `<read_first>` with at least the file being modified
- [ ] Every task has `<acceptance_criteria>` with behavior, test-command, CLI, or source assertions
- [ ] Every `<action>` contains concrete identifiers without fenced code blocks or full implementations
- [ ] Dependencies correctly identified
- [ ] Waves assigned for parallel execution
- [ ] must_haves derived from phase goal
- [ ] Every PLAN.md includes an "Artifacts this phase produces" section listing symbols created by this phase (decorators, classes, functions, CLI flags, struct/dataclass fields, new file paths)
</quality_gate>

<!-- ══════════════════════════════════════════════════════════════════════════════
     WAVE 1: CRITICAL REGRESSSIONS + CRITICAL SECURITY (parallel)
     ══════════════════════════════════════════════════════════════════════════════ -->

<task>
  <id>1</id>
  <objective>Fix Duplicate Task Rendering in Execute View (REGR-1)</objective>
  <action>
    In `internal/tui/execute_model.go`, the `renderTasks()` method (lines 285-357) has two loops that both render pending tasks. The first loop (lines 289-345) iterates ALL tasks and renders pending tasks with a badge at line 317-324. The second loop (lines 348-354) renders pending tasks again with dimmed style. Remove the second loop entirely (lines 347-354). In the first loop, add a condition: after the running-task block (line 324), if `task.Status == types.StatusPending || task.Status == ""`, render with dimmed style (faint) and a `○` badge, then `continue`. This consolidates all task rendering into a single loop.
  </action>
  <read_first>
    internal/tui/execute_model.go
    internal/types/workflow.go (for StatusPending, StatusRunning, StatusDone, StatusFailed constants)
  </read_first>
  <acceptance_criteria>
    `renderTasks()` contains exactly ONE loop over `em.tasks`
    Pending tasks render with `○` badge and faint/dimmed style in the single loop
    No task appears twice in the rendered output
    Completed tasks still show `✓` badge
    Failed tasks still show `✗` badge
    Running tasks still show spinner
    `go build ./...` succeeds
    `go test ./internal/tui/... -count=1` passes
  </acceptance_criteria>
  <requirements>REGR-1</requirements>
</task>

<task>
  <id>2</id>
  <objective>Fix Data Race in Pause/Resume System (REGR-2)</objective>
  <action>
    In `internal/workflow/engine.go`, `consumeSkipOrCancel()` (lines 270-289) reads `e.resumeCh` at line 278 without holding `pauseMu`. Meanwhile, `ResumeExecution()` closes and nils `e.resumeCh` under the mutex. Fix by capturing `resumeCh` inside the mutex-protected block (lines 271-275), just like `skipCh`, `cancelCh`, and `groupCh` are already captured. Add `resumeCh := e.resumeCh` between lines 274-275 (inside the lock), then use `resumeCh` in the select at line 278 instead of `e.resumeCh`.
  </action>
  <read_first>
    internal/workflow/engine.go (lines 260-310 for consumeSkipOrCancel, and ResumeExecution method)
  </read_first>
  <acceptance_criteria>
    `consumeSkipOrCancel` captures `e.resumeCh` into a local variable under `pauseMu.Lock()`
    The select statement uses the local `resumeCh` variable, not `e.resumeCh`
    `go vet ./internal/workflow/...` reports no race conditions
    `go test ./internal/workflow/... -race -count=1` passes
    No deadlock when pause → resume sequence executes
  </acceptance_criteria>
  <requirements>REGR-2</requirements>
</task>

<task>
  <id>3</id>
  <objective>Fix Double Persistent Permissions Load (REGR-3)</objective>
  <action>
    In `internal/tools/defaults.go`, persistent permissions are loaded twice during `DefaultDispatcher` initialization. Lines 16-21 load via `d.persistentPerms.Load(workDir)` (the dispatcher already initialized `persistentPerms` in its constructor via `NewDispatcher`). Lines 40-46 create a new `PersistentPermissions()` instance and load again, appending duplicate rules. Remove the second load block entirely (lines 40-46). The first load at lines 16-21 is correct because it uses the dispatcher's own `persistentPerms` instance which is already initialized.
  </action>
  <read_first>
    internal/tools/defaults.go
    internal/tools/dispatcher.go (for NewDispatcher constructor and persistentPerms initialization)
  </read_first>
  <acceptance_criteria>
    `DefaultDispatcher` contains exactly ONE `persistentPerms.Load()` call (at lines 16-21)
    No second `NewPersistentPermissions()` instantiation in `DefaultDispatcher`
    `d.rules` does not contain duplicate entries after initialization
    `go build ./...` succeeds
    `go test ./internal/tools/... -count=1` passes
  </acceptance_criteria>
  <requirements>REGR-3</requirements>
</task>

<task>
  <id>4</id>
  <objective>Fix ModeAuto Silent Termination (REGR-4)</objective>
  <action>
    In `internal/tui/app_update_phase.go`, the `nextPhaseForMode()` function (lines 17-53) handles `ModeFast`, `ModeDirect`, but has no case for `ModeAuto` in several phase transitions. `ModeAuto` is the default workflow mode. When `ModeAuto` is active, the function falls through to the default `return types.PhaseIdle, false` at line 52, silently terminating the workflow. Add `ModeAuto` handling: in each `case` block, `ModeAuto` should behave the same as the default full workflow path (not skipping phases). For example, at `PhaseInitialize`, `ModeAuto` should return `PhaseDiscuss, true` (same as the default). At `PhaseExecute`, `ModeAuto` should return `PhaseVerify, true`. The simplest fix: add `case types.ModeAuto:` alongside the existing mode checks where it should follow the default path, or explicitly check `if mode == types.ModeAuto { /* use default path */ }` before the mode-specific shortcuts.
  </action>
  <read_first>
    internal/tui/app_update_phase.go
    internal/types/workflow.go (for ModeAuto constant and WorkflowMode type)
  </read_first>
  <acceptance_criteria>
    `nextPhaseForMode()` handles `ModeAuto` explicitly in all phase transition cases
    `ModeAuto` follows the full workflow path (Initialize→Discuss→Plan→Execute→Verify→Runtime→Ship)
    `ModeAuto` does NOT skip any phases (unlike ModeFast/ModeDirect)
    `go build ./...` succeeds
    Manual test: starting workflow with default mode progresses through all phases
  </acceptance_criteria>
  <requirements>REGR-4</requirements>
</task>

<!-- ══════════════════════════════════════════════════════════════════════════════
     WAVE 2: CRITICAL SECURITY + HIGH CORRECTNESS (parallel)
     ══════════════════════════════════════════════════════════════════════════════ -->

<task>
  <id>5</id>
  <objective>Add Argument Validation to Git Tool (NEW-1)</objective>
  <action>
    In `internal/tools/git.go`, the `args` parameter from LLM input is split with `strings.Fields()` and appended directly to git commands. While `exec.CommandContext` prevents shell injection, dangerous flags like `--force`, `--no-index`, `-D` can be passed. Add an `allowedArgs` map per operation that whitelists safe flags. For each operation:
    - `add`: allow `-f`, `-p`, `-v`, `-n`, `--dry-run`, `--verbose`
    - `commit`: allow `-m`, `-a`, `--amend`, `--no-edit`, `--allow-empty`, `--allow-empty-message`
    - `diff`: allow `--stat`, `--name-only`, `--name-status`, `--cached`, `--staged`
    - `log`: allow `--oneline`, `--graph`, `--all`, `--stat`, `-n`, `--pretty`
    - `branch`: allow `-d`, `-D`, `-m`, `-M`, `-r`, `-a`, `--list`
    - `checkout`: allow `-b`, `-B`, `--orphan`
    - `stash`: allow `push`, `pop`, `apply`, `drop`, `list`, `show`
    - `status`: allow `--short`, `--branch`, `--porcelain`
    
    Create a `validateGitArgs(operation string, args []string) error` function that checks each arg against the allowlist. Return an error if any arg is not in the allowlist. Call this function in `Execute()` before constructing the command.
  </action>
  <read_first>
    internal/tools/git.go
    internal/tools/bash.go (for dangerous-command pattern reference)
  </read_first>
  <acceptance_criteria>
    `validateGitArgs()` function exists and checks args against per-operation allowlists
    `Execute()` calls `validateGitArgs()` before constructing git commands
    `git checkout --force` is rejected (force flag not in checkout allowlist)
    `git diff --no-index /etc/passwd` is rejected (`--no-index` not in diff allowlist)
    `git branch -D main` is rejected (`-D` not in branch allowlist when used with branch name)
    `git commit -m "fix bug"` is accepted (`-m` in commit allowlist)
    `git log --oneline` is accepted
    `go build ./...` succeeds
    `go test ./internal/tools/... -count=1` passes
  </acceptance_criteria>
  <requirements>NEW-1</requirements>
</task>

<task>
  <id>6</id>
  <objective>Fix workdir Sandbox Bypass (NEW-2)</objective>
  <action>
    In `internal/tools/bash.go`, the user-supplied `workdir` value (line 123-124) is passed directly to `applyBashSandbox()` (line 132), which injects it into Landlock allowed paths (Linux) or sandbox-exec profile (macOS). A `workdir` of `/etc` would add that directory to the OS-level sandbox's allow list, neutralizing the defense-in-depth layer. Fix by:
    1. Before line 123, validate `workdirRaw` using `filepath.Clean()` to normalize the path
    2. Check that the cleaned path is within `t.workDir` (the default working directory) using `strings.HasPrefix(cleaned, filepath.Clean(t.workDir))` or by checking it's not an absolute path outside the project
    3. If validation fails, return an error: `"workdir must be within the project directory"`
    4. Also validate the path exists: `os.Stat(cleaned)` — if it doesn't exist, return an error
    5. Apply `filepath.Rel(t.workDir, cleaned)` to get a relative path, then resolve it against `t.workDir` to get the absolute path safely
  </action>
  <read_first>
    internal/tools/bash.go
    internal/tools/permission.go (for sandbox reference)
  </read_first>
  <acceptance_criteria>
    `workdirRaw` is cleaned with `filepath.Clean()` before use
    `workdir` value is validated to be within `t.workDir` (not an absolute path outside project)
    `workdir` value is validated to exist on filesystem
    `applyBashSandbox()` receives a path that is within the project directory
    `workdir="/etc"` returns an error, not added to sandbox allowlist
    `go build ./...` succeeds
    `go test ./internal/tools/... -count=1` passes
  </acceptance_criteria>
  <requirements>NEW-2</requirements>
</task>

<task>
  <id>7</id>
  <objective>Fix Relative workdir Path Resolution (NEW-3)</objective>
  <action>
    In `internal/tools/bash.go`, line 124 sets `cmd.Dir = workdirRaw` with the raw string. Go resolves relative `Dir` paths against the calling process's CWD, not `t.workDir` — contradicting the schema documentation which states "If relative, resolved from the default working directory." Fix by resolving relative workdir against `t.workDir`:
    1. After `filepath.Clean(workdirRaw)`, check if the path is absolute using `filepath.IsAbs()`
    2. If relative, join with `t.workDir`: `cmd.Dir = filepath.Join(t.workDir, cleaned)`
    3. If absolute, use as-is (after validation from task 6)
    4. Apply `filepath.Abs()` to ensure the final path is always absolute
  </action>
  <read_first>
    internal/tools/bash.go
  </read_first>
  <acceptance_criteria>
    Relative `workdir` values are resolved against `t.workDir`, not process CWD
    `filepath.IsAbs()` check determines whether to join with `t.workDir`
    `cmd.Dir` is always set to an absolute path
    `workdir="subdir"` resolves to `t.workDir/subdir`
    `workdir="/absolute/path"` uses the absolute path directly (after validation)
    `go build ./...` succeeds
  </acceptance_criteria>
  <requirements>NEW-3</requirements>
</task>

<task>
  <id>8</id>
  <objective>Fix extractCommitMessage Truncation (NEW-4)</objective>
  <action>
    In `internal/tools/git.go`, `extractCommitMessage()` (lines 527-554) splits on spaces with `strings.Fields()` and only takes the first token after `-m`. For `git commit -m fix bug`, it captures only `fix`. Fix by using a smarter parser:
    1. After finding the `-m` flag at index `i`, collect ALL remaining tokens from `fields[i+1:]` as the message
    2. Join them with spaces: `strings.Join(fields[i+1:], " ")`
    3. Strip surrounding quotes from the joined result if present
    4. This preserves the full commit message `fix bug` instead of just `fix`
  </action>
  <read_first>
    internal/tools/git.go
  </read_first>
  <acceptance_criteria>
    `extractCommitMessage("-m fix bug")` returns `"fix bug"` (not `"fix"`)
    `extractCommitMessage("-m 'fix bug'")` returns `"fix bug"`
    `extractCommitMessage("-m \"fix bug\"")` returns `"fix bug"`
    `extractCommitMessage("")` returns `""`
    `extractCommitMessage("fix bug")` returns `"fix bug"` (no -m flag)
    `go build ./...` succeeds
    `go test ./internal/tools/... -count=1` passes
  </acceptance_criteria>
  <requirements>NEW-4</requirements>
</task>

<!-- ══════════════════════════════════════════════════════════════════════════════
     WAVE 3: LINT + VET + MEDIUM ISSUES (parallel)
     ══════════════════════════════════════════════════════════════════════════════ -->

<task>
  <id>9</id>
  <objective>Fix All Lint Issues (LINT-1 through LINT-5)</objective>
  <action>
    Fix 5 golangci-lint issues:
    
    1. **LINT-1 (copylocks)**: `emitter_stress_test.go:61` — `StreamingMockProvider` contains `sync.Mutex`, passed by value. Change the function signature to accept `*StreamingMockProvider` (pointer) instead of `StreamingMockProvider` (value).
    
    2. **LINT-2 (ineffassign)**: `phase_transition_model.go:127` — `w` assigned but never read. Remove the unused `w` variable assignment or use it.
    
    3. **LINT-3, LINT-4, LINT-5 (staticcheck QF1012)**: `git.go:485,487,491` — `WriteString(fmt.Sprintf(...))` should be `fmt.Fprintf`. Change `sb.WriteString(fmt.Sprintf(...))` to `fmt.Fprintf(&sb, ...)` at all three locations.
  </action>
  <read_first>
    internal/tools/emitter_stress_test.go
    internal/tui/phase_transition_model.go
    internal/tools/git.go
  </read_first>
  <acceptance_criteria>
    `go vet ./...` passes with zero issues
    `golangci-lint run ./...` passes with zero issues (5-minute timeout)
    `emitter_stress_test.go` uses pointer receiver for `StreamingMockProvider`
    `phase_transition_model.go` has no unused variable `w`
    `git.go` uses `fmt.Fprintf(&sb, ...)` instead of `sb.WriteString(fmt.Sprintf(...))`
    `go test ./... -count=1 -short` passes
  </acceptance_criteria>
  <requirements>LINT-1, LINT-2, LINT-3, LINT-4, LINT-5</requirements>
</task>

<task>
  <id>10</id>
  <objective>Fix Persistent Permissions Corrupt JSON Handling (NEW-6)</objective>
  <action>
    In `internal/tools/persistent_permissions.go`, `Save()` (lines 66-71) reads existing data and silently discards `json.Unmarshal` errors. If the file contains corrupt JSON, the subsequent `Save()` overwrites it with only the new project's rules, permanently deleting all other projects' permissions. Fix by:
    1. In `Save()`, if `json.Unmarshal` fails, log the error and back up the corrupt file to `permissions.json.corrupt.{timestamp}`
    2. Return the error to the caller so it can be handled
    3. Optionally: prompt the user to confirm before overwriting corrupt data
  </action>
  <read_first>
    internal/tools/persistent_permissions.go
  </read_first>
  <acceptance_criteria>
    `Save()` logs an error when `json.Unmarshal` fails on existing file
    `Save()` backs up corrupt file before overwriting
    `Save()` returns an error when existing data is corrupt
    Other projects' rules are preserved even when one project's data is corrupt
    `go build ./...` succeeds
    `go test ./internal/tools/... -count=1` passes
  </acceptance_criteria>
  <requirements>NEW-6</requirements>
</task>

<task>
  <id>11</id>
  <objective>Fix Workflow Mode Race Condition (NEW-7)</objective>
  <action>
    In `internal/workflow/engine.go`, line 876 reads `e.workflowMode` without `RLock` while `SetWorkflowMode` writes under `Lock`. Fix by:
    1. In any method that reads `e.workflowMode`, use `e.mu.RLock()` before reading and `e.mu.RUnlock()` after
    2. Ensure `SetWorkflowMode` continues to use `e.mu.Lock()` for writes
    3. Check all other reads of `e.workflowMode` for the same issue
  </action>
  <read_first>
    internal/workflow/engine.go
  </read_first>
  <acceptance_criteria>
    All reads of `e.workflowMode` are protected by `e.mu.RLock()`
    `SetWorkflowMode` uses `e.mu.Lock()` for writes
    `go vet ./...` reports no race conditions
    `go test ./internal/workflow/... -race -count=1` passes
  </acceptance_criteria>
  <requirements>NEW-7</requirements>
</task>

<task>
  <id>12</id>
  <objective>Add esc/q Confirmation Before Exiting Active Execution (NEW-8)</objective>
  <action>
    In `internal/tui/execute_model.go`, pressing `esc` or `q` during active execution (lines 199-202) navigates away without pausing or confirmation. Execution continues silently in the background. Fix by:
    1. When execution is active (not paused, tasks running), intercept `esc`/`q` keys
    2. Show a confirmation dialog: "Execution is running. Pause first? [y/n/cancel]"
    3. If user confirms pause, pause execution before navigating
    4. If user confirms cancel, cancel execution before navigating
    5. If user cancels the dialog, stay on the execution screen
  </action>
  <read_first>
    internal/tui/execute_model.go
    internal/tui/app_model.go (for key handling patterns)
  </read_first>
  <acceptance_criteria>
    `esc`/`q` during active execution shows confirmation dialog
    Confirmation dialog offers Pause, Cancel, and Stay options
    Pause option pauses execution before navigating away
    Cancel option cancels execution before navigating away
    Stay option remains on execution screen
    `go build ./...` succeeds
    `go test ./internal/tui/... -count=1` passes
  </acceptance_criteria>
  <requirements>NEW-8</requirements>
</task>

<!-- ══════════════════════════════════════════════════════════════════════════════
     WAVE 4: DEAD CODE + CLEANUP + TESTS (parallel)
     ══════════════════════════════════════════════════════════════════════════════ -->

<task>
  <id>13</id>
  <objective>Fix Dead Code: descriptionKeywords() and isConfigFile Allowlist</objective>
  <action>
    In `internal/workflow/engine_verify.go`:
    1. `descriptionKeywords()` (lines 85-89) always returns nil, making smart truncation a no-op. Implement it to extract capitalized words and common code terms from the task description. Use a simple heuristic: split on spaces, filter words longer than 3 characters, convert to lowercase.
    2. `isConfigFile` allowlist (lines 506-517) is incomplete. Add missing extensions: `.env.example`, `.toml`, `.yaml`, `.yml`, `.json`, `.xml`, `.ini`, `.cfg`, `.conf`
  </action>
  <read_first>
    internal/workflow/engine_verify.go
  </read_first>
  <acceptance_criteria>
    `descriptionKeywords()` returns non-nil slice of keywords extracted from task context
    `isConfigFile()` recognizes `.env.example`, `.toml`, `.yaml`, `.yml`, `.json`, `.xml`, `.ini`, `.cfg`, `.conf`
    `go build ./...` succeeds
    `go test ./internal/workflow/... -count=1` passes
  </acceptance_criteria>
  <requirements>REGR-1</requirements>
</task>

<task>
  <id>14</id>
  <objective>Add Test Coverage for Critical Paths</objective>
  <action>
    Add tests for the most critical untested paths:
    
    1. **Git tool tests** (`internal/tools/git_test.go`):
       - `TestGit_ArgValidation`: verify `validateGitArgs` rejects dangerous flags
       - `TestGit_ExtractCommitMessage`: verify full message preservation
       - `TestGit_Operations`: basic operation tests for add, commit, diff, log, branch, status
    
    2. **Bash workdir tests** (`internal/tools/bash_test.go`):
       - `TestBash_WorkdirValidation`: verify workdir within project is accepted
       - `TestBash_WorkdirRejectAbsolute`: verify `/etc` is rejected
       - `TestBash_WorkdirRelativeResolution`: verify relative paths resolve against t.workDir
    
    3. **Persistent permissions tests** (`internal/tools/persistent_permissions_test.go`):
       - `TestPersistentPermissions_CorruptJSON`: verify corrupt file is backed up, not silently overwritten
       - `TestPersistentPermissions_SavePreservesOtherProjects`: verify saving for project A doesn't delete project B's rules
    
    4. **Engine race tests** (`internal/workflow/engine_test.go`):
       - `TestEngine_ConcurrentPauseResume`: verify no race under concurrent pause/resume
  </action>
  <read_first>
    internal/tools/git.go
    internal/tools/bash.go
    internal/tools/persistent_permissions.go
    internal/workflow/engine.go
    internal/tools/git_test.go (if exists)
    internal/tools/bash_test.go (if exists)
  </read_first>
  <acceptance_criteria>
    `internal/tools/git_test.go` exists with `TestGit_ArgValidation`, `TestGit_ExtractCommitMessage`, `TestGit_Operations`
    `internal/tools/bash_test.go` has `TestBash_WorkdirValidation`, `TestBash_WorkdirRejectAbsolute`, `TestBash_WorkdirRelativeResolution`
    `internal/tools/persistent_permissions_test.go` has `TestPersistentPermissions_CorruptJSON`, `TestPersistentPermissions_SavePreservesOtherProjects`
    `internal/workflow/engine_test.go` has `TestEngine_ConcurrentPauseResume`
    `go test ./internal/tools/... -race -count=1` passes
    `go test ./internal/workflow/... -race -count=1` passes
    Coverage for `git.go` > 60%
    Coverage for `bash.go` workdir paths > 70%
  </acceptance_criteria>
  <requirements>REGR-2, NEW-1, NEW-2, NEW-3</requirements>
</task>

<task>
  <id>15</id>
  <objective>Fix Validation Error Color Bug (NEW-5)</objective>
  <action>
    In `internal/tui/config_model.go` or `internal/tui/settings_model.go`, input validation error messages display in green due to a color-detection bug. Users see green text for errors, which is UX-inverted. Find the color assignment for validation errors and change it from green (success color) to red (error color). The error color should match `theme.Error` or the equivalent error style used elsewhere in the TUI.
  </action>
  <read_first>
    internal/tui/config_model.go
    internal/tui/settings_model.go
    internal/tui/theme.go (for Error color reference)
  </read_first>
  <acceptance_criteria>
    Validation error messages display in red, not green
    Error color matches `theme.Error` or equivalent error style
    `go build ./...` succeeds
    `go test ./internal/tui/... -count=1` passes
  </acceptance_criteria>
  <requirements>NEW-5</requirements>
</task>

<task>
  <id>16</id>
  <objective>Fix ConfigModel.View() Elm Architecture Violation</objective>
  <action>
    In `internal/tui/config_model.go`, `View()` mutates viewport state (width, height, content, scroll offset) during rendering via `renderFields()`. Bubble Tea's Elm architecture requires `View()` to be pure — all mutations belong in `Update()`. Fix by:
    1. Move any state mutations from `View()` to `Update()`
    2. `View()` should only read state and render, never modify it
    3. If `renderFields()` mutates state, move those mutations to the appropriate `Update()` handler
  </action>
  <read_first>
    internal/tui/config_model.go
    internal/tui/app_model.go (for Update pattern reference)
  </read_first>
  <acceptance_criteria>
    `ConfigModel.View()` contains zero state mutations
    `renderFields()` does not modify any model fields
    All state changes happen in `Update()` handlers
    `go build ./...` succeeds
    `go test ./internal/tui/... -count=1` passes
    TUI renders correctly (manual smoke test)
  </acceptance_criteria>
  <requirements>NEW-5</requirements>
</task>

<must_haves>
  <truths>
    <!-- Critical regressions fixed -->
    <truth>renderTasks() has exactly one loop — no duplicate task rendering</truth>
    <truth>consumeSkipOrCancel() captures resumeCh under mutex — no data race</truth>
    <truth>DefaultDispatcher loads persistent permissions exactly once</truth>
    <truth>ModeAuto follows full workflow path, does not silently terminate</truth>
    
    <!-- Critical security issues fixed -->
    <truth>Git tool validates args against per-operation allowlists</truth>
    <truth>Bash workdir is validated to be within project directory</truth>
    <truth>Bash workdir is resolved against t.workDir, not process CWD</truth>
    
    <!-- High correctness issues fixed -->
    <truth>extractCommitMessage preserves full multi-word commit messages</truth>
    <truth>All lint issues resolved (golangci-lint passes)</truth>
    <truth>All vet issues resolved (go vet passes)</truth>
    
    <!-- Quality gates -->
    <truth>Race detector passes on all packages</truth>
    <truth>No dead code remains (descriptionKeywords implemented, isConfigFile complete)</truth>
    <truth>Validation errors display in red, not green</truth>
    <truth>ConfigModel.View() is pure (no state mutations)</truth>
  </truths>
</must_haves>

<verification_criteria>
After all tasks complete, run:
1. `make check` — must pass (fmt → tidy → vet → lint → test)
2. `make lint` — must pass (golangci-lint, 5m timeout)
3. `make test` — must pass (race-enabled tests with coverage)
4. `go build ./...` — must succeed
5. Manual verification:
   - Execute view shows each task exactly once (no duplicates)
   - Pause/resume works without deadlock
   - Git tool rejects dangerous flags
   - Bash workdir validates paths correctly
   - Permission modal shows countdown from start
   - Workflow progresses through all phases in ModeAuto
</verification_criteria>

<artifacts>
  <!-- Artifacts this phase produces -->
  <symbol>
    <name>validateGitArgs</name>
    <type>function</type>
    <file>internal/tools/git.go</file>
    <description>Validates git arguments against per-operation allowlists</description>
  </symbol>
  <symbol>
    <name>TestGit_ArgValidation</name>
    <type>test</type>
    <file>internal/tools/git_test.go</file>
    <description>Tests for Git argument validation</description>
  </symbol>
  <symbol>
    <name>TestBash_WorkdirValidation</name>
    <type>test</type>
    <file>internal/tools/bash_test.go</file>
    <description>Tests for Bash workdir path validation</description>
  </symbol>
  <symbol>
    <name>TestPersistentPermissions_CorruptJSON</name>
    <type>test</type>
    <file>internal/tools/persistent_permissions_test.go</file>
    <description>Tests for corrupt JSON handling in permissions</description>
  </symbol>
  <symbol>
    <name>TestEngine_ConcurrentPauseResume</name>
    <type>test</type>
    <file>internal/workflow/engine_test.go</file>
    <description>Tests for concurrent pause/resume without races</description>
  </symbol>
</artifacts>

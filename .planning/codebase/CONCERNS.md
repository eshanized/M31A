<!-- refreshed: 2026-08-23 -->
# Codebase Concerns

**Analysis Date:** 2026-08-23

## Tech Debt

### State Authority & Ownership (Architectural Root Cause)

**Issue:** M31A has multiple partially overlapping sources of truth for the same engineering run. The workflow engine continuously translates between them through shared mutable state, filesystem persistence, prompt state, task state, and UI state.

**Files:**
- `internal/engine/workflow/engine.go` — 30+ fields, god object by dependency accumulation
- `internal/engine/workflow/workflow_state.go` — separate mutable state with its own mutex hierarchy
- `internal/engine/session/manager.go` — persists to `.m31a/session.json`, `messages.json`
- `internal/engine/workflow/engine_checkpoint.go` — checkpoint/recovery state
- `internal/ui/tui/app.go` — TUI state (MsgEmitter, screens, models)

**Impact:** State drift, context drift, workflow edge cases, recovery complexity, hard-to-test behavior, hard-to-predict agent behavior. Bugs become state convergence bugs rather than local code bugs.

**Fix approach:** Establish one authoritative runtime model (EngineeringRun) with explicit domain ownership. Move persistence to SQLite as authoritative store; TUI/artifacts become projections. See `docs/M31A_ARCHITECTURE_RESET.md` for full migration plan.

---

### Workflow Engine God Object

**Issue:** `Engine` holds 30+ fields mixing workflow orchestration, state management, LLM prompting, tool dispatch, Git, metrics, hooks, code-intel, compaction, context registry, session persistence, recovery, pause/cancel channels.

**Files:** `internal/engine/workflow/engine.go`

**Impact:** Violates single responsibility; coupling makes testing difficult; changes cascade across unrelated subsystems.

**Fix approach:** Extract domain services (TaskScheduler, AgentRuntime, ContextEngine, ModelGateway, ToolRuntime, VerificationService, RecoveryService) with explicit contracts.

---

### Inconsistent Path Validation

**Issue:** Some tools use `ResolveAndContainPath` but git commit and verify paths use raw `filepath.Join` without containment checks.

**Files:**
- `internal/engine/workflow/execute.go:648-668` — `git.CommitWithFiles(task.Files...)` uses LLM-provided paths directly
- `internal/engine/workflow/engine_verify.go:239-245` — `filepath.Join(e.workDir, f)` no validation
- `internal/tools/fileops/pathhelpers.go` — `ResolveAndContainPath` exists but not universally used

**Impact:** Path traversal via git commit (staging files outside workdir) or verification reads.

**Fix approach:** Central `ValidateTaskFiles(task.Files, workDir)` called at task creation (Plan phase) and before every use (Execute, Verify, Ship).

---

### Shell Variable Expansion Blocking Incomplete

**Issue:** `containsVariableExpansion()` regex `\$[A-Za-z_]` catches `$VAR` but misses `${VAR}`, `${VAR:-default}`, `$((arithmetic))`, `${#array}`.

**Files:** `internal/tools/exec/bash.go:438-453`

**Impact:** Potential command injection if LLM output includes expansions that evaluate to attacker-controlled values.

**Fix approach:** Use proper shell parser (`mvdan.cc/sh/v3/syntax`) or complete metacharacter deny-list (`$`, `` ` ``, `{`, `}`, `(`, `)`, `[`, `]`, `*`, `?`, `~`).

---

### TODO/FIXME/HACK Comments (100+ across codebase)

**Files:** Grep shows 100+ matches across multiple files. Notable:
- `pkg/extensions/registry.go:98` — `proc.SetWorkDir(".") // TODO: use project root`
- `internal/integrations/provider/external_provider.go:21` — same
- `internal/tools/external_tool.go:23` — same
- `internal/engine/tokens/estimator.go:400` — `// conversations and may allow requests that exceed the model's window (BUG-29)`

**Impact:** Technical debt accumulation; some mark known bugs (BUG-01 through BUG-29).

**Fix approach:** Address each BUG-XX reference; convert TODOs to tracked issues or implement fixes.

---

## Known Bugs

### P0: Git Commit() Commits Unrelated User Changes (M31A-AUDIT-001)

**Symptoms:** `Git.Commit()` calls `AddAll()` staging ALL modified, deleted, and untracked files in the worktree — including user changes unrelated to the agent's work.

**Files:** `internal/integrations/git/git.go:142-154`

**Trigger:** Any workflow phase that calls `Commit()` (Execute, Ship, self-heal)

**Workaround:** Use `Git.CommitWithFiles(paths...)` which stages only specified paths.

**Fix approach:** Remove or privatize `Commit()`. All callers must use `CommitWithFiles()`. Add integration test verifying unrelated changes are not committed.

---

### P0: 90% Verification Threshold Allows Failed Work to Pass (M31A-AUDIT-002)

**Symptoms:** `VerifySuccessThreshold = 0.90` means if 10% of tasks fail, verification still returns `Success: true`. Failed tasks logged as "warnings" but phase succeeds.

**Files:** `internal/engine/workflow/verify.go:21-22, 198-217`

**Trigger:** Verify phase when ≥10% of verified tasks fail

**Workaround:** None — core correctness guarantee violated.

**Fix approach:** Remove `VerifySuccessThreshold`. Make `allOK = false` if any task fails. If partial completion needed, add explicit config flag `allow_partial_verify` defaulting to false with user confirmation in TUI.

---

### P0: StateMachine.SetPhase() Bypasses All Validation (M31A-AUDIT-003)

**Symptoms:** `SetPhase()` directly assigns `currentPhase` without checking `validTransitions`. Resets `discussPlanCycles = 0`. Corrupted/malicious recovery file can set phase to `PhaseShip` from `PhaseIdle`, bypassing all intermediate phases and safety checks.

**Files:** `internal/engine/workflow/state_machine.go:108-116`

**Trigger:** Checkpoint restore (`engine_checkpoint.go:76, 115, 185`), recovery (`recovery.go:177`), rollback (`engine_checkpoint.go:185`)

**Workaround:** None — state corruption possible.

**Fix approach:** Add `SetPhaseWithValidation(phase, expectedFrom)` that validates transition. For checkpoint restore, store `fromPhase` in checkpoint and validate. Log structured warning when `SetPhase` bypasses validation.

---

### P1: Session Recovery Does Not Restore Task States (M31A-AUDIT-004)

**Symptoms:** `LoadCheckpointData()` restores `CurrentPhase`, `Goal`, `PlanVersion`, `Messages`, `Checkpoint` but NOT task execution state. Task states (Status, HealsAttempted) remain in session.json but not re-applied to task runner.

**Files:** `internal/engine/workflow/engine_checkpoint.go:52-84`, `internal/engine/session/manager.go:207-268`

**Trigger:** App crash/restart during Execute phase; `ResumeOnStartup: true`

**Workaround:** Manual re-execution or careful state inspection.

**Fix approach:** In `LoadCheckpointData()`, load tasks via `sessionMgr.LoadTasks()` and reinitialize `taskrunner.Runner` with restored task states. Preserve `HealsAttempted` and `Status`.

---

### P1: Workdir Escape in Bash Tool via Symlinks (M31A-AUDIT-005)

**Symptoms:** Bash tool `workdir` parameter validation can be bypassed with symlinks. Validation uses `filepath.Rel(cleanTWorkDir, absWorkdir)` but if `workdir` is a symlink to `/etc`, check operates on resolved path incorrectly.

**Files:** `internal/tools/exec/bash.go:138-162`

**Trigger:** LLM calls Bash with `workdir` pointing to a symlink outside project

**Workaround:** Avoid symlinks in project directory.

**Fix approach:** Resolve `t.workDir` to real path at construction. Validate `workdir` parameter against real project root using `filepath.EvalSymlinks` on both paths before comparison.

---

### P1: Headless Mode Bypasses Permission System Entirely (M31A-AUDIT-006)

**Symptoms:** `--goal` and `--prompt` headless modes execute tools without permission prompts. Dispatcher's `ensurePermission()` blocks forever on unbuffered channel or returns `ErrPermissionDenied` if buffer full.

**Files:** `cmd/m31a/main.go:186-247` (`runHeadlessWorkflow`, `runHeadless`)

**Trigger:** Running `m31a --goal "..."` or `m31a --prompt "..."`

**Workaround:** Use TUI mode for tool-using workflows.

**Fix approach:** Add `--permission-mode=auto|deny|allow` flag for headless mode. Default to `deny` for dangerous tools. Document headless mode as read-only/prompt-only unless explicitly configured.

---

### P1: LLM-Generated Task Files Used Without Path Validation (M31A-AUDIT-007)

**Symptoms:** Task `Files` field from LLM output used directly in git commits and file ops. While `FileWrite` uses `ResolveAndContainPath`, git commit and verify paths do not consistently validate containment.

**Files:** `internal/engine/workflow/execute.go:648-668`, `internal/engine/workflow/engine_verify.go:239-245`

**Trigger:** LLM generates task with `Files: ["../../etc/passwd"]` or similar

**Workaround:** None — supply chain risk from model output.

**Fix approach:** Add `ValidateTaskFiles(task.Files, workDir)` called at task creation and before every use (Execute, Verify, Ship). Reject tasks with invalid paths.

---

### P1: Command Injection Risk in Bash Tool — Incomplete Variable Expansion Check (M31A-AUDIT-008)

**Symptoms:** Regex misses `${VAR}`, `${VAR:-default}`, `$((arithmetic))`, `${#array}`. `$()` handled by `checkCommandChaining` but doesn't recursively parse nested `$()` inside arguments.

**Files:** `internal/tools/exec/bash.go:438-453`

**Trigger:** LLM crafts bash command with shell expansions

**Workaround:** None — potential command injection.

**Fix approach:** Use proper shell parser or complete metacharacter deny-list.

---

### P1: Recovery State Restore Does Not Validate Phase Consistency (M31A-AUDIT-009)

**Symptoms:** `Recover()` calls `stateMachine.SetPhase(state.CurrentPhase)` (bypassing validation) and restores plan/messages. Does NOT verify: tasks exist for restored phase, plan matches restored phase, git state matches checkpoint.

**Files:** `internal/engine/workflow/engine_checkpoint.go:92-123`

**Trigger:** App crash during Execute phase; restart with recovery file present

**Workaround:** Delete recovery file before restart.

**Fix approach:** Add session ID to recovery state. On `Recover()`, verify `state.SessionID == e.sessionID`. Validate restored phase has corresponding tasks in session manager.

---

### P2: Unbounded Emitter Channel Memory Growth (M31A-AUDIT-010)

**Symptoms:** `emitterCh` has fixed capacity `ChannelCap`. `drainAdaptiveCmd()` only switches to batch drain when `len(emitterCh) > ChannelCap/4`. If workflow produces messages faster than TUI consumes, channel fills and workflow goroutines block.

**Files:** `internal/ui/tui/app.go:596-625`, `internal/ui/tui/app.go:499`

**Trigger:** High-frequency tool calls during Execute phase

**Workaround:** None — workflow stalls, memory growth, potential deadlock.

**Fix approach:** Add `context.Context` with timeout to emitter sends. Implement backpressure: when channel > 75% full, signal workflow to pause tool dispatch.

---

### P2: Git Diff Validation Incomplete in Verify (M31A-AUDIT-011)

**Symptoms:** Verify phase only checks `git diff --name-only HEAD` (unstaged), missing staged/committed changes. Files committed during Execute won't appear in this diff.

**Files:** `internal/engine/workflow/engine_verify.go:248-271`

**Trigger:** Task modifies files that were already committed in Execute phase

**Workaround:** None — false warnings in verification.

**Fix approach:** Use `git diff --name-only <sessionStartHash>..HEAD` to capture all changes since session baseline.

---

### P2: Model Capability Detection by String Patterns Unreliable (M31A-AUDIT-012)

**Symptoms:** `provider.ParseModelCapabilities()` uses string matching on model IDs (e.g., "r1" → reasoning, "vision" → multimodal). Provider APIs return capability metadata but it's ignored.

**Files:** `internal/integrations/provider/capabilities.go` (referenced in openrouter/client.go:126, zen/client.go:113, nvidia/client.go:117)

**Trigger:** New model released with ID not matching existing patterns

**Workaround:** Manual model selection.

**Fix approach:** Extend provider model metadata to include capabilities. Update `ParseModelCapabilities` to use API data first, heuristics second.

---

### P2: Rollback Does Not Distinguish Agent vs User Changes (M31A-AUDIT-013)

**Symptoms:** `Rollback.SoftReset/HardReset` stash ALL uncommitted changes including user's. `stashIfDirty()` calls `git stash push` with no scoping. On `HardReset`, user's changes remain stashed (potentially lost if stash drops).

**Files:** `internal/engine/rollback/rollback.go:128-194`, `internal/engine/rollback/rollback.go:236-251`

**Trigger:** User triggers rollback via TUI or bisect identifies bad commit

**Workaround:** Commit user changes before rollback.

**Fix approach:** Track agent-modified files via session. On rollback, only reset those files (using `git checkout <commit> -- <files>`). Leave user's other changes untouched.

---

### P2: Bisect Modifies Git State Without Isolation (M31A-AUDIT-014)

**Symptoms:** `Bisect.Run()` runs `git bisect` directly on user's repo. If bisect is interrupted (crash, Ctrl+C), repo is left in bisect mode. `defer bisect reset` only runs on normal return, not on process kill.

**Files:** `internal/engine/bisect/bisect.go:61-152`

**Trigger:** Verify phase calls `tryBisectHeal()` which runs bisect

**Workaround:** Manual `git bisect reset` after crash.

**Fix approach:** Run bisect in a separate worktree (`git worktree add`). Or wrap in signal handler that forces `bisect reset`.

---

## Security Considerations

### API Keys Fall Back to Plaintext Config File If Keychain Fails (SEC-008)

**Risk:** When keychain is unavailable, API keys are persisted to config file (TOML) in plaintext with warning log only.

**Files:** `internal/core/config/loader.go:550-573`, `internal/core/config/loader.go:593-648`

**Current mitigation:** Keychain caching with 5-minute blacklist TTL; keys only fall back to file if keychain save fails.

**Recommendations:** 
- Add config option to disable plaintext fallback entirely (fail hard if keychain unavailable)
- Encrypt config file keys at rest (age/sops)
- Warn more prominently in TUI when keys stored in plaintext

---

### Bash Tool Symlink Escape (SEC-003)

**Risk:** Workdir validation bypassable via symlinks, allowing command execution outside project directory.

**Files:** `internal/tools/exec/bash.go:138-162`

**Current mitigation:** Path validation with `filepath.Rel` but doesn't resolve symlinks on both sides consistently.

**Recommendations:** Resolve both project root and target workdir with `filepath.EvalSymlinks` before comparison. Add integration test for symlink escape.

---

### Incomplete Shell Injection Blocking (SEC-004)

**Risk:** Variable expansion regex misses multiple shell metacharacter forms.

**Files:** `internal/tools/exec/bash.go:438-453`

**Current mitigation:** `checkCommandChaining` splits on `; & |` separators.

**Recommendations:** Use `mvdan.cc/sh/v3/syntax` parser for complete detection, or run commands with `bash -c` where expansions disabled.

---

### LLM-Controlled File Paths Unvalidated (SEC-005)

**Risk:** Task `Files` array from LLM used directly in git commit and verify without path containment validation.

**Files:** `internal/engine/workflow/execute.go:648-668`, `internal/engine/workflow/engine_verify.go:239-245`

**Current mitigation:** `FileWrite` tool uses `ResolveAndContainPath`.

**Recommendations:** Central validation at task creation and before every use. Reject invalid paths.

---

### Bisect Leaves Repo in Modified State on Crash (SEC-006)

**Risk:** `git bisect` modifies repo's bisect state; `defer` cleanup doesn't run on process kill.

**Files:** `internal/engine/bisect/bisect.go:67-73`

**Current mitigation:** `defer bisect reset` on normal return.

**Recommendations:** Run bisect in temporary worktree. Add signal handler for forced cleanup.

---

### Rollback Stashes User's Uncommitted Changes (SEC-007)

**Risk:** `stashIfDirty()` stashes ALL uncommitted changes. User's unrelated work mixed with agent's rollback. Potential loss on `HardReset`.

**Files:** `internal/engine/rollback/rollback.go:236-251`

**Current mitigation:** Stash popped on `SoftReset`, but not on `HardReset`.

**Recommendations:** Track agent-modified files; use `git checkout <commit> -- <files>` for selective revert.

---

## Performance Bottlenecks

### Emitter Channel Backpressure Missing

**Problem:** Workflow emitter channel can accumulate messages under sustained load. No backpressure signal to workflow engine.

**Files:** `internal/ui/tui/app.go:596-625`

**Cause:** Fixed capacity channel; `drainAdaptiveCmd()` only batches at 25% capacity; no mechanism to signal workflow to slow down.

**Improvement path:** Add bounded channel with backpressure or drop policy. Workflow should slow down or drop non-critical messages. Implement context with timeout on emitter sends.

---

### Token Estimation May Allow Context Overflow (BUG-29)

**Problem:** Token estimator uses EMA (exponential moving average) but may allow requests that exceed model's window.

**Files:** `internal/engine/tokens/estimator.go:400`

**Cause:** Estimation based on historical averages, not worst-case.

**Improvement path:** Add safety margin (e.g., 90% of context window). Validate against provider's actual `context_window` from model metadata.

---

### Large Engine Object & Lock Contention

**Problem:** `Engine` and `WorkflowState` have multiple mutexes with lock-ordering contracts. High contention during Execute phase with parallel tool execution.

**Files:** `internal/engine/workflow/engine.go`, `internal/engine/workflow/workflow_state.go`

**Cause:** God object with many responsibilities requiring synchronization.

**Improvement path:** Extract domain services to reduce lock scope. Use finer-grained locking per domain.

---

### Context Compaction Not Incremental

**Problem:** Compaction re-processes entire conversation history rather than incrementally.

**Files:** `internal/engine/compaction/compaction.go`

**Cause:** `Compact()` takes full message list each time.

**Improvement path:** Implement incremental compaction — only compact new messages since last compaction point.

---

## Fragile Areas

### Workflow Engine State Machine

**Files:** `internal/engine/workflow/state_machine.go`, `internal/engine/workflow/engine_checkpoint.go`, `internal/engine/workflow/recovery.go`

**Why fragile:** `SetPhase()` bypasses validation; checkpoint restore doesn't validate consistency; recovery can load stale state from different workflow.

**Safe modification:** Add validation to all phase transitions. Add session ID to recovery state. Write tests for invalid transition rejection.

**Test coverage gaps:** No tests for checkpoint restore with mismatched phase; no tests for recovery from corrupted state.

---

### Git Integration

**Files:** `internal/integrations/git/git.go`, `internal/engine/rollback/rollback.go`, `internal/engine/bisect/bisect.go`, `internal/engine/workflow/ship.go`

**Why fragile:** Multiple git operations with different safety profiles (`Commit` vs `CommitWithFiles`, rollback stashing all changes, bisect leaving repo in bisect mode). No unified git safety model.

**Safe modification:** Always use `CommitWithFiles(paths...)`. Track agent-modified files explicitly. Run bisect in isolated worktree.

**Test coverage gaps:** No test for `Commit()` vs `CommitWithFiles` safety; no test for rollback preserving user changes; no test for bisect crash recovery.

---

### Session Recovery & Checkpoint

**Files:** `internal/engine/workflow/engine_checkpoint.go`, `internal/engine/session/manager.go`, `internal/engine/workflow/recovery.go`

**Why fragile:** Checkpoint saves partial state (no tasks); recovery loads phase without validating task/plan consistency; session ID not in recovery state.

**Safe modification:** Include task state in checkpoint. Validate recovery state against current session. Add session ID to recovery.

**Test coverage gaps:** No test for task state restoration on resume; no test for cross-workflow recovery corruption; no test for recovery with missing tasks.

---

### Provider Capability Detection

**Files:** `internal/integrations/provider/capabilities.go`, `internal/integrations/provider/openrouter/client.go`, `internal/integrations/provider/zen/client.go`, `internal/integrations/provider/nvidia/client.go`

**Why fragile:** Heuristic string matching on model IDs; new models break capability detection; provider API metadata ignored.

**Safe modification:** Extend model metadata to include capabilities from API. Keep heuristics as fallback only.

**Test coverage gaps:** Zero test coverage for NVIDIA provider; no tests for capability detection with unknown models.

---

### Headless Mode Permission System

**Files:** `cmd/m31a/main.go:186-247`, `internal/tools/dispatcher.go`

**Why fragile:** Permission requests require TUI channel; no headless policy. Dispatcher blocks on unbuffered channel with no receiver.

**Safe modification:** Add `--permission-mode` flag. Default dangerous tools to deny in headless mode.

**Test coverage gaps:** No tests for headless mode permission handling.

---

## Scaling Limits

### Emitter Channel Capacity

**Current capacity:** Fixed `ChannelCap` (value not shown in grep but defined in `app.go`)

**Limit:** Under high-frequency tool calls (>100 ops/sec), channel fills, workflow goroutines block on send.

**Scaling path:** Implement backpressure protocol — when channel > 75% full, signal workflow to pause tool dispatch via context cancellation or rate limiter.

---

### Session File Size

**Current:** Flat files (`session.json`, `messages.json`) with atomic writes and file-size limits.

**Limit:** Large sessions (1000+ messages) cause slow reads/writes; file locking contention with concurrent access.

**Scaling path:** Move authoritative state to SQLite. Keep flat files as projections/artifacts only.

---

### Context Window Management

**Current:** Token estimation with EMA, automatic compaction when approaching limits.

**Limit:** Estimation may allow overflow (BUG-29); compaction re-processes entire history; no per-model context window from provider metadata.

**Scaling path:** Use provider API `context_window` field. Implement incremental compaction. Add safety margin (90%).

---

## Dependencies at Risk

### go-osc52/v2 (v2.0.1) — Clipboard

**Risk:** Clipboard dependency for OSC52 escape sequence. Used in TUI for copy/paste. If clipboard unavailable, copy operations may fail silently.

**Impact:** UX degradation (copy/paste broken). Not security-critical.

**Mitigation:** Graceful degradation; log warning.

---

### golang.org/x/sync v0.22.0

**Risk:** Singleflight, errgroup, semaphore usage. Standard library extended packages — generally stable but could have breaking changes in future versions.

**Impact:** Concurrency primitives used in dispatcher, provider cache, task runner.

**Mitigation:** Vendor or pin version. Monitor for security advisories.

---

### charm bracelet ecosystem (bubbletea, bubbles, lipgloss, glamour)

**Risk:** TUI framework. Active development; API changes between major versions could require migration.

**Impact:** Entire TUI layer depends on Bubble Tea architecture.

**Mitigation:** Pin versions in go.mod. Test upgrades in isolation.

---

### tiktoken-go v0.1.8

**Risk:** Token estimation for OpenAI-compatible models. If model tokenization changes, estimates become inaccurate.

**Impact:** Context overflow (BUG-29) or under-utilization.

**Mitigation:** Use provider API `context_window` where available. Add model-specific overrides.

---

### gotreesitter v0.20.5 (code intelligence)

**Risk:** Tree-sitter bindings for Go. CGo-free but depends on tree-sitter C library compiled to Go. Version mismatches can cause parse errors.

**Impact:** Code intelligence features (symbols, references, definitions) may break.

**Mitigation:** Test with multiple Go versions. Pin tree-sitter grammar versions.

---

## Missing Critical Features

### Explicit Partial Verification Configuration

**Problem:** Verify phase has implicit 90% threshold. No user-facing control for "allow partial success".

**Blocks:** Safety-critical workflows where any failure must block Ship.

**Fix:** Add config flag `allow_partial_verify` (default false) with TUI confirmation prompt.

---

### Headless Mode Permission Policy

**Problem:** Headless mode documented but non-functional for tool-using workflows.

**Blocks:** CI/CD integration, automation, scripting.

**Fix:** Add `--permission-mode=auto|deny|allow` flag with clear documentation.

---

### Recovery Session ID Validation

**Problem:** Recovery file from workflow A can be loaded during workflow B startup, restoring wrong phase.

**Blocks:** Reliable resume after crash in shared project directories.

**Fix:** Add session ID to recovery state; validate on load.

---

### Git Safety API Enforcement

**Problem:** `Commit()` (unsafe) exists alongside `CommitWithFiles()` (safe). No compile-time enforcement.

**Blocks:** Guaranteeing user data safety.

**Fix:** Remove or privatize `Commit()`. Lint rule to forbid `git.Commit()` calls.

---

### Provider Capability Metadata from API

**Problem:** Capabilities inferred from model ID strings; provider API metadata ignored.

**Blocks:** Reliable tool calling for new/unknown models.

**Fix:** Extend provider model metadata; update clients to use API capabilities first.

---

## Test Coverage Gaps

### NVIDIA Provider — 0% Coverage

**What's not tested:** `internal/integrations/provider/nvidia/client.go` — all logic including model fetching, capability detection, chat completion, streaming.

**Files:** `internal/integrations/provider/nvidia/client.go`

**Risk:** NVIDIA provider changes break silently; capability detection heuristics untested.

**Priority:** High

**Fix:** Add unit tests with mocked HTTP responses. Test model fetching, capability parsing, error handling.

---

### Workflow Prompts Package — 0% Coverage

**What's not tested:** `internal/engine/workflow/prompts/` — all prompt templates, rendering, model-specific composition.

**Files:** `internal/engine/workflow/prompts/` (entire package)

**Risk:** Prompt template changes break workflow phases silently.

**Priority:** High

**Fix:** Add tests for prompt template loading, rendering, variable substitution, model-specific variations.

---

### Git Commit Safety — No Tests

**What's not tested:** `Commit()` vs `CommitWithFiles()` safety — that unrelated changes are NOT committed.

**Files:** `internal/integrations/git/git.go`, `internal/engine/workflow/execute.go`, `internal/engine/workflow/ship.go`

**Risk:** P0 data loss bug (M31A-AUDIT-001) has no test coverage.

**Priority:** Critical

**Fix:** Add integration test: create repo with user's uncommitted changes, run workflow Execute phase, verify user's changes NOT committed.

---

### Verify Threshold Behavior — No Tests

**What's not tested:** 90% threshold allows failures to pass as success.

**Files:** `internal/engine/workflow/verify.go:21-217`

**Risk:** P0 correctness bug (M31A-AUDIT-002) has no test coverage.

**Priority:** Critical

**Fix:** Add test: create plan with 10 tasks, make 1 fail verification, assert `PhaseResult.Success == false`.

---

### State Machine Invalid Transition Rejection — No Tests

**What's not tested:** `SetPhase()` bypasses validation; invalid transitions allowed.

**Files:** `internal/engine/workflow/state_machine.go:108-116`

**Risk:** P0 state corruption bug (M31A-AUDIT-003) has no test coverage.

**Priority:** Critical

**Fix:** Add test: call `SetPhase(PhaseShip)` from `PhaseIdle`, assert error or structured warning logged.

---

### Headless Mode Permission Handling — No Tests

**What's not tested:** Headless mode permission flow (blocks or fails silently).

**Files:** `cmd/m31a/main.go:186-247`, `internal/tools/dispatcher.go`

**Risk:** P1 feature broken (M31A-AUDIT-006) has no test coverage.

**Priority:** High

**Fix:** Add test: run headless workflow with tool call, verify permission behavior (deny/allow based on flag).

---

### Bisect Crash Recovery — No Tests

**What's not tested:** Bisect leaves repo in bisect mode on process kill.

**Files:** `internal/engine/bisect/bisect.go:61-152`

**Risk:** P2 repo corruption (M31A-AUDIT-014) has no test coverage.

**Priority:** High

**Fix:** Add test: start bisect, kill process, verify repo not in bisect mode (or test worktree isolation).

---

### Symlink Escape in File Tools — No Tests

**What's not tested:** Bash tool workdir validation bypassable with symlinks.

**Files:** `internal/tools/exec/bash.go:138-162`

**Risk:** P1 filesystem escape (M31A-AUDIT-005) has no test coverage.

**Priority:** High

**Fix:** Add test: create symlink to outside directory, call Bash with workdir=symlink, verify command executes in project or fails validation.

---

### Task State Restoration on Resume — No Tests

**What's not tested:** Session recovery loads phase but not task states.

**Files:** `internal/engine/workflow/engine_checkpoint.go:52-84`

**Risk:** P1 reliability failure (M31A-AUDIT-004) has no test coverage.

**Priority:** High

**Fix:** Add test: run workflow to Execute with 3/5 tasks complete, kill process, restart with resume, verify task states restored.

---

### Cross-Workflow Recovery Corruption — No Tests

**What's not tested:** Recovery file from workflow A loads during workflow B.

**Files:** `internal/engine/workflow/engine_checkpoint.go:92-123`, `internal/engine/workflow/recovery.go`

**Risk:** P1 state corruption (M31A-AUDIT-009) has no test coverage.

**Priority:** High

**Fix:** Add test: run workflow A, kill, start workflow B in same dir, verify workflow B doesn't load A's recovery state.

---

### Model Capability Detection for Unknown Models — No Tests

**What's not tested:** `ParseModelCapabilities()` with model IDs not matching heuristics.

**Files:** `internal/integrations/provider/capabilities.go`

**Risk:** P2 reliability (M31A-AUDIT-012) has no test coverage.

**Priority:** Medium

**Fix:** Add tests: unknown model IDs, verify fallback behavior, verify API metadata takes precedence.

---

*Concerns audit: 2026-08-23*
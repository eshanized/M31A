# TEST_FAILURES.md — CI Run 30061804395 (Job `test`, ID 89384771805)

**Run:** https://github.com/eshanized/M31A/actions/runs/30061804395/job/89384771805
**Triggering commit:** `6a42c099` — "chore(ci): update Go setup action to v7 and upgrade x/sync module version"
**Date:** 2026-07-24

---

## 1. Full Failing-Test Inventory

The CI run failed with **59 distinct test failures** across **5 packages**, grouped into **6 root causes**. The annotations panel only showed ~9 pathspec lines and 2 session failures; the full log reveals far more.

### Package: `internal/core/config` (6 failures)

| # | Test | Line | Failure |
|---|------|------|---------|
| 1 | `TestMergeConfig_IntOverride` | merge_test.go:105 | expected 100, got 50 |
| 2 | `TestMergeConfig_FloatOverride` | merge_test.go:131 | expected 0.9, got 0.800000 |
| 3 | `TestMergeConfig_CompactionConfig` | merge_test.go:274 | expected 30000, got 20000 |
| 4 | `TestMergeConfig_NestedSubagentProfiles` | merge_test.go:351 | expected explore MaxTools 10, got 5 |
| 5 | `TestMergeConfig_TypeSafe_Int` | extra_test.go:158 | expected 100, got 50 |
| 6 | `TestMergeConfig_TypeSafe_Float` | extra_test.go:178 | expected 2.5, got 0.800000 |

### Package: `internal/engine/session` (3 failures)

| # | Test | Line | Failure |
|---|------|------|---------|
| 7 | `TestManager_RenameSession` | manager_extra_test.go:109 | Expected label 'my cool session', got "" |
| 8 | `TestManager_saveSessionAtomic` | manager_extra_test.go:536 | Expected label 'atomic test', got "" |
| 9 | `TestManager_saveSessionAtomic_ProducesValidJSON` | manager_extra_test.go:889 | Expected label 'test-label', got "" |

### Package: `internal/engine/workflow` (19 failures)

| # | Test | Line | Failure |
|---|------|------|---------|
| 10 | `TestRunPlan_WithRefinement` | engine_extra_test.go:1855 | invalid phase transition from initialize to plan |
| 11 | `TestRunVerify_SkipsNonDoneTasks` | engine_extra_test.go:2600 | invalid phase transition from initialize to verify |
| 12 | `TestEngineTransitionMultiplePhases` | engine_wiring_test.go:174 | invalid phase transition from initialize to initialize |
| 13 | `TestEngine_RunPlan_Success` | plan_test.go:29 | invalid phase transition from initialize to plan |
| 14 | `TestEngine_RunPlan_ParsesJSONFromMarkdown` | plan_test.go:58 | invalid phase transition from initialize to plan |
| 15 | `TestEngine_Plan_SavesTasks` | plan_test.go:198 | invalid phase transition from initialize to plan |
| 16 | `TestEngine_RunPlan_RetryWithErrorFeedback` | plan_test.go:227 | invalid phase transition from initialize to plan |
| 17 | `TestEngine_RunPlan_ManualFallback` | plan_test.go:260 | Expected non-nil result (nil from failed transition) |
| 18 | `TestEngine_RunShip` | ship_test.go:28 | invalid phase transition from initialize to ship |
| 19 | `TestEngine_RunShip_NoTasks` | ship_test.go:49 | invalid phase transition from initialize to ship |
| 20 | `TestEngine_RunShip_NoGit` | ship_test.go:105 | invalid phase transition from initialize to ship |
| 21 | `TestEngine_RunShip_WithFailedTasks` | ship_test.go:134 | Expected non-nil result (nil from failed transition) |
| 22 | `TestEngine_RunShip_WithSkippedTasks` | ship_test.go:161 | invalid phase transition from initialize to ship |
| 23 | `TestShip_SaveStateBeforeArchive` | ship_test.go:230 | invalid phase transition from initialize to ship |
| 24 | `TestEngine_RunVerify_NoTasks` | verify_test.go:20 | invalid phase transition from idle to verify |
| 25 | `TestEngine_RunVerify_WithDoneTasks` | verify_test.go:47 | invalid phase transition from initialize to verify |
| 26 | `TestEngine_RunVerify_MissingFile` | verify_test.go:72 | invalid phase transition from initialize to verify |
| 27 | `TestEngine_RunVerify_SkipsPendingTasks` | verify_test.go:173 | invalid phase transition from initialize to verify |
| 28 | `TestHandlePhase_InvalidPhaseName` | commands_all_test.go:429 | nil pointer dereference in session.LoadWorkflowState |

### Package: `internal/testutil/ci` (1 failure)

| # | Test | Line | Failure |
|---|------|------|---------|
| 29 | `TestIsCI/GITLAB_CI=true` | ci_test.go:73 | IsCI() = false, want true |

### Package: `internal/tools` (30 failures)

| # | Test | Subtest | Failure |
|---|------|---------|---------|
| 30 | `TestCheckDangerousCommand_Baseline` | rm -rf * | blocked=false, want true |
| 31 | | rm -rf ~ | blocked=false, want true |
| 32 | | shred | blocked=false, want true |
| 33 | | wipefs | blocked=false, want true |
| 34 | | nc -l | blocked=false, want true |
| 35 | | ncat -l | blocked=false, want true |
| 36 | | socat | blocked=false, want true |
| 37 | | >/dev/tcp | blocked=false, want true |
| 38 | | <_/dev/tcp | blocked=false, want true |
| 39 | | curl \| sh | blocked=false, want true |
| 40 | | wget \| bash | blocked=false, want true |
| 41 | | curl \| bash | blocked=false, want true |
| 42 | `TestCheckDangerousCommand_ExpandedBlocklist` | wipefs | blocked=false, want true |
| 43 | | shred | blocked=false, want true |
| 44 | | nc listener | blocked=false, want true |
| 45 | | ncat listener | blocked=false, want true |
| 46 | | socat | blocked=false, want true |
| 47 | `TestCheckDangerousCommand_ChainingDetection` | command_substitution_safe | blocked=true, want false |
| 48 | `TestCheckDangerousCommand_ObfuscationDetection` | variable_expansion | blocked=false, want true |
| 49 | `TestCheckDangerousCommand_CustomBlockedCommands` | partial_match_should_not_block | blocked=true, want false |
| 50 | `TestCheckDangerousCommand_CustomObfuscationPatterns` | partial_match_should_not_block | blocked=true, want false |
| 51 | `TestCheckDangerousCommand_LongCommand` | (top-level) | blocked=false, want true |
| 52 | `TestBash_CommandSubstitution` | (top-level) | command blocked: variable expansion |
| 53 | `TestAskUserQuestion_Timeout` | (top-level) | expected timeout error |

*(The `internal/tools` package also panicked from `TestAskUserQuestion_ChannelFull` causing a 30s timeout, which killed the entire package test run and swallowed additional subtest results. The above list includes only what completed before the timeout.)*

---

## 2. Root Causes, Grouped

### Root Cause A — Config merge `intField`/`float64Field` regression (6 tests)

**Hypothesis confirmed:** H3 variant — a source-code regression in a recent commit, not the CI chore itself.

**Commit:** `a28a011a` — "fix(01-03): token estimation and config bugs B16/B17/B18/B19"

The B18 fix changed `intField()` and `float64Field()` in `internal/core/config/merge.go` (lines 41-53) to use only `m.hasKey(key)`:

```go
func (m mergeHelper) intField(base, overlay *int, key string) {
    if m.hasKey(key) {
        *base = *overlay
    }
}
```

When `MergeConfig` is called with `nil` for `defined` (as all 6 failing tests do), `hasKey()` always returns `false`, so no int or float overlay value is ever applied. The fix dropped the pre-existing non-zero fallback (`*overlay != 0`) that all tests relied on.

**Evidence:** `boolField` (line 33) still has both conditions: `m.hasKey(key) || *overlay`. The `intField`/`float6Field` change is asymmetric with all other field types.

**Fix:** Add back the non-zero fallback:
```go
func (m mergeHelper) intField(base, overlay *int, key string) {
    if m.hasKey(key) || *overlay != 0 {
        *base = *overlay
    }
}
// Same for float64Field
```

---

### Root Cause B — Session manager `sessionMetadata` missing `Label` field (3 tests)

**Root cause:** `sessionMetadata` struct in `internal/engine/session/manager.go` (lines 336-349) does not include a `Label` field. `saveSessionAtomic()` builds this struct and writes it to `session.json`. Since `Label` is missing from the struct, it is silently dropped on every save, then reads back as `""`.

**Data flow:**
1. `RenameSession` sets `sess.Label = label` then calls `saveSessionAtomic(sess)`
2. `saveSessionAtomic` builds `sessionMetadata` (no `Label` field) and writes to disk
3. `LoadSession` reads from disk — `Label` is zero-value `""`

**Fix:** Add `Label string \`json:"label,omitempty"\`` to the `sessionMetadata` struct and add `Label: session.Label,` to the struct literal in `saveSessionAtomic`.

---

### Root Cause C — Workflow engine `RunPhase` transition enforcement (19 tests)

**Hypothesis confirmed:** Not H1, H2, or H3 — this is a state-machine enforcement change.

**Commit:** `c1e5dbda` — "fix(01-01): capture compaction return value and route RunPhase through Transition"

Changed `RunPhase` from `e.stateMachine.SetPhase(phase)` (unconditional) to `e.stateMachine.Transition(from, phase)` (validates against transition graph). The state machine in `internal/engine/workflow/state_machine.go` (lines 32-41) only allows:

- `Initialize` -> `Discuss`, `Execute`, `Idle`
- NOT `Initialize` -> `Plan`, `Verify`, `Ship`

All 18 transition tests were written for the old `SetPhase` behavior where each `RunPhase` was independent. After the commit, the transition graph blocks the illegal jumps the tests assume are valid.

**Sub-issue — TestHandlePhase_InvalidPhaseName:** Passes a zero-value `session.Manager{}` (nil internals) to `handlePhase`, which dereferences nil fields in `LoadWorkflowState`. This is a separate nil-safety issue in the test or implementation.

**Sub-issue — execute.go:571 lint error:** `messages = e.proactiveCompactCheck(messages)` is an ineffectual assignment because `messages` is re-declared with `:=` at the top of each heal-loop iteration.

**Fix:** Either update the tests to drive the state machine through legal transitions, or change `RunPhase` to allow direct phase jumps (for single-phase test usage) while still validating transitions in the normal workflow path.

---

### Root Cause D — TestIsCI race condition (1 test)

**Root cause:** Pre-existing design flaw. `ci_test.go` (lines 68-76) runs subtests with `t.Parallel()` but all mutate the same process-global environment variables (`os.Setenv`/`os.Unsetenv`) without synchronization. Goroutine scheduling determines which subtest's `os.Setenv` wins.

**Fix:** Remove `t.Parallel()` from the subtests, or use `t.Setenv()` (Go 1.17+) which automatically restores the env var after the test, avoiding cross-subtest contamination.

---

### Root Cause E — Bash security patterns lost in directory restructure (23 tests)

**Root cause:** Commit `b14d96ba` ("refactor(tui): complete architecture upgrade and directory restructuring") moved `internal/tools/bash.go` to `internal/tools/exec/bash.go`. The new file was created with the **original, minimal pattern lists** from before commit `f35077bd` which had expanded the blocklist. All expanded patterns were lost.

Additionally, `dangerousObfuscationPatterns` contains regex syntax (`echo.*|.*sh`) used with `strings.Contains` (literal matching), so those patterns never match anything. And `containsVariableExpansion` uses the regex string `$[A-Za-z_]` as a literal with `strings.Contains`, which never matches `$HOME` etc.

**Breakdown by sub-issue:**
- **Missing patterns** (Baseline + ExpandedBlocklist): `shred`, `wipefs`, `nc -l`, `ncat -l`, `socat`, `/dev/tcp`, `curl|sh`, `wget|bash`, `curl|bash`, `rm -rf *`, `rm -rf ~`
- **Regex-as-literal** (Baseline obfuscation + ObfuscationDetection): `dangerousObfuscationPatterns` use `.*` with `strings.Contains`
- **Regex-as-literal in detection** (ObfuscationDetection): `$[A-Za-z_]` used with `strings.Contains` instead of `regexp.MatchString`
- **Indiscriminate $() blocking** (ChainingDetection, TestBash_CommandSubstitution): blocks all `$()` instead of only dangerous inner commands
- **Substring vs exact match** (CustomBlockedCommands, CustomObfuscationPatterns): `strings.Contains` does substring matching; tests expect exact matching

**Fix:** Port the expanded pattern lists from `f35077bd` into `exec/bash.go`, fix the regex patterns to use `regexp.Compile`, and implement exact-prefix matching for custom blocklists.

---

### Root Cause F — TestAskUserQuestion_Timeout (1 test)

**Root cause:** The test at `extra_test.go:3616` expects `err != nil` on timeout, but `Execute` returns `(ToolResult{Error: "..."}, nil)` — the error is in the result struct, not the Go error return value. The check `err == nil` will always be true.

**Fix:** Assert on `result.Error != ""` instead of `err != nil`.

---

## 3. Post-Checkout `git exit 128` Failure

The `Post Run actions/checkout@v7` step failed with `git exit 128`. This is **unrelated** to the pathspec errors from tests. The annotations panel attributed these to pathspec errors, but they are actually the cleanup step failing because `checkout@v7`'s post-step runs `git config` commands to clean up credentials, and those commands can fail when the working directory's `.git` has been modified or when the checkout itself had issues.

The pathspec errors (`fatal: pathspec 'test.go' did not match any files`) come from the workflow engine's mock LLM returning `git add -- test.go` commands, and the tests' mock git implementations reporting pathspec errors. These are test-level events (logged as WARNs), not the post-checkout failure.

**These are two distinct symptoms:**
1. **Pathspec errors in tests:** Warn-level log lines from the workflow engine's mock tooling. They do not directly cause test failures; they are symptoms of the mock LLM returning file paths that don't exist in the test's temp git repo.
2. **Post-checkout git exit 128:** The `checkout@v7` cleanup step failed to run `git config --unset-all` or similar. This could be a shallow clone (`fetch-depth: 1`) combined with credential cleanup expecting a full clone. Check whether the CI workflow was changed recently to use `fetch-depth: 1`.

---

## 4. Regression Check

**Was this test passing before the CI chore commit?**

| Root Cause | Pre-existing? | Caused by CI chore? |
|------------|---------------|---------------------|
| A — Config merge | Yes, introduced by `a28a011a` | No — source code bug, not CI change |
| B — Session Label | Yes, introduced during restructuring | No |
| C — Workflow transitions | Yes, introduced by `c1e5dbda` | No |
| D — TestIsCI race | Yes, from test creation | No |
| E — Bash security patterns | Yes, introduced by `b14d96ba` | No |
| F — AskUserQuestion timeout | Yes, from test creation | No |

**The CI chore commit (`6a42c099`) only changed:**
- `.github/workflows/ci.yml` (setup-go v7, x/sync version bump)
- `go.mod` / `go.sum` (x/sync version)

**It touched zero source files and zero test files.** All 59 failures are pre-existing bugs introduced by earlier source-code commits. The CI chore merely exposed them — these tests were likely already failing locally or were masked by CI caching.

---

## 5. Recommended Fixes

### High Priority (functional bugs affecting runtime behavior)

1. **Config merge (Root Cause A):** Restore the non-zero fallback in `intField()` and `float64Field()` in `merge.go`. This is a real bug — any config merging without a `defined` map silently drops all int/float overrides.

2. **Session Label (Root Cause B):** Add `Label` field to `sessionMetadata` and propagate it in `saveSessionAtomic`. Session labels are user-visible and are being silently dropped.

### Medium Priority (test correctness)

3. **Workflow transitions (Root Cause C):** Update 18 tests to drive the state machine through legal transitions (Initialize -> Execute -> Verify -> Ship, etc.), or provide a test-only bypass for direct phase execution.

4. **Bash security patterns (Root Cause E):** Port the expanded blocklist from the original `bash.go` into `exec/bash.go`. Fix regex patterns to use `regexp.Compile`. Implement exact-prefix matching for custom blocklists. This is a real security gap — dangerous commands are not being blocked.

### Low Priority (flaky/minor)

5. **TestIsCI (Root Cause D):** Remove `t.Parallel()` from subtests or use `t.Setenv()`.

6. **TestAskUserQuestion_Timeout (Root Cause F):** Assert on `result.Error` instead of `err`.

### CI Improvements

7. **Post-checkout git exit 128:** Investigate whether `fetch-depth: 1` (shallow clone) combined with `persist-credentials: true` causes the cleanup step to fail. Consider bumping `actions/upload-artifact` from v4 to v5 to resolve the Node.js 20 deprecation warning.

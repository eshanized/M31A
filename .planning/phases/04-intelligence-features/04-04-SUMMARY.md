---
phase: 04-intelligence-features
plan: 04
subsystem: intelligence-cli
tags: [git-worktree, bisect, repro, regression-investigation, D-09, D-10, D-11]

# Dependency graph
requires:
  - phase: 04-intelligence-features
    plan: 01
    provides: Confidence enum, Evidence/EvidencePack/Citation contract, Event vocabulary, Intelligence error sentinels, [intelligence] TOML config
  - phase: 04-intelligence-features
    plan: 02
    provides: Git blame porcelain parser (ValidateRef), explain pipeline (CollectorDeps, Synthesizer, RenderText/RenderJSON)
provides:
  - internal/intelligence/investigate/WorktreeManager with Create/CheckoutTo/Remove/PruneOrphans/InstallSignalCleanup
  - internal/intelligence/investigate/ResolveReproCommand (flag > config > auto-detect) and ExecuteRepro (exec.CommandContext, no shell)
  - internal/intelligence/investigate/Candidates (rev-list --reverse truncated to maxCommits) and FindCulprit (window-bounded binary search)
  - Exported error sentinels: NotReproducibleInWindowError, IterationCapExceededError
  - InvestigateGitRunner narrow interface for testability
affects: [04-05-investigate-bisect-report, 04-06-deps-clients, 04-07-deps-verdict-cache]

# Actuals (#2632)
actuals:
  tokens: 35000
  tasks: 3
  commits: 5

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Temp worktree lifecycle with pid+timestamp naming, signal handlers (SIGINT/SIGTERM), orphan prune on startup"
    - "Strict repro command resolution chain: --repro flag > intelligence.repro_command config > auto-detect (go.mod -> go test ./..., package.json -> npm test --silent)"
    - "Window-bounded binary search over rev-list candidates with honest not-reproducible semantics (never silent widen)"
    - "Confirmation pass: culprit fails AND parent passes before verified attribution"

key-files:
  created:
    - internal/intelligence/investigate/worktree.go
    - internal/intelligence/investigate/worktree_test.go
    - internal/intelligence/investigate/repro.go
    - internal/intelligence/investigate/repro_test.go
    - internal/intelligence/investigate/bisect.go
    - internal/intelligence/investigate/bisect_test.go
  modified: []

key-decisions:
  - "Worktree temp directory uses pid + nanosecond timestamp (m31a-investigate-<pid>-<timestamp>) under os.TempDir() to prevent collision across runs"
  - "ValidateRef exported from worktree.go (mirroring internal/integrations/git/git.go) to gate all ref-taking git calls before exec"
  - "ExecuteRepro uses exec.CommandContext with strings.Fields arg-splitting — no shell ever spawned, output capped at 64KB"
  - "FindCulprit pre-checks: symptom MUST fail at HEAD (window end) and pass at baseline (window start); otherwise NotReproducibleInWindowError with evidence"
  - "Binary search converges to first-failing commit in candidate list; confirmation requires culprit=fail AND parent=pass (two independent observations)"

patterns-established:
  - "InvestigateGitRunner narrow interface mirrors legacy GitRunner for testability; tests inject mock runners for validation rejection tests"
  - "All ref validation before git exec via ValidateRef — hostile refs (shell metacharacters, path traversal, etc.) rejected pre-invocation"
  - "Worktree cleanup: defer Remove() on all return paths + signal handler for SIGINT/SIGTERM + startup PruneOrphans sweep"
  - "Error sentinel types with helper predicates (IsNotReproducibleInWindow, IsIterationCapExceeded) for callers"

requirements-completed: [REGRESS-01]

# Coverage metadata (#1602)
coverage:
  - id: D1
    description: "WorktreeManager lifecycle: Create/CheckoutTo/Remove/PruneOrphans with real git in temp dirs; hostile refs rejected pre-exec; orphan prune on startup proven"
    requirement: REGRESS-01
    verification:
      - kind: integration
        ref: "internal/intelligence/investigate/worktree_test.go#TestWorktreeManager_CreateCheckoutRemoveCycle"
        status: pass
      - kind: integration
        ref: "internal/intelligence/investigate/worktree_test.go#TestWorktreeManager_OrphanPruneOnStartup"
        status: pass
      - kind: unit
        ref: "internal/intelligence/investigate/worktree_test.go#TestWorktreeManager_ValidationRejectsHostileRefs"
        status: pass
      - kind: integration
        ref: "internal/intelligence/investigate/worktree_test.go#TestWorktreeManager_ParentRepoHEADUnchanged"
        status: pass
    human_judgment: false
  - id: D2
    description: "Repro resolution chain (flag > config > auto-detect Go/npm) and ExecuteRepro with exec.CommandContext, no shell, context cancellation, 64KB output bound"
    requirement: REGRESS-01
    verification:
      - kind: unit
        ref: "internal/intelligence/investigate/repro_test.go#TestResolveReproCommand_PrecedenceFlagOverConfigOverAuto"
        status: pass
      - kind: unit
        ref: "internal/intelligence/investigate/repro_test.go#TestExecuteRepro_PassFailRegex"
        status: pass
      - kind: unit
        ref: "internal/intelligence/investigate/repro_test.go#TestExecuteRepro_Cancellation"
        status: pass
      - kind: unit
        ref: "internal/intelligence/investigate/repro_test.go#TestExecuteRepro_OutputBounded"
        status: pass
    human_judgment: false
  - id: D3
    description: "Window-bounded binary search: Candidates from rev-list --reverse truncated to maxCommits; FindCulprit with pre-checks, confirmation pass, deterministic attribution"
    requirement: REGRESS-01
    verification:
      - kind: integration
        ref: "internal/intelligence/investigate/bisect_test.go#TestCandidates_FromRevList"
        status: pass
      - kind: integration
        ref: "internal/intelligence/investigate/bisect_test.go#TestCandidates_TruncatedToMaxCommits"
        status: pass
      - kind: integration
        ref: "internal/intelligence/investigate/bisect_test.go#TestFindCulprit_SeededCulprit"
        status: pass
      - kind: integration
        ref: "internal/intelligence/investigate/bisect_test.go#TestFindCulprit_NotReproducibleAtWindowStart"
        status: pass
      - kind: integration
        ref: "internal/intelligence/investigate/bisect_test.go#TestFindCulprit_DeterministicAcrossRuns"
        status: pass
      - kind: unit
        ref: "internal/intelligence/investigate/bisect_test.go#TestFindCulprit_IterationCap"
        status: pass
    human_judgment: false
  - id: D4
    description: "go vet clean and race detector clean for entire investigate package"
    requirement: REGRESS-01
    verification:
      - kind: other
        ref: "go vet ./internal/intelligence/investigate/"
        status: pass
      - kind: other
        ref: "go test -race ./internal/intelligence/investigate/"
        status: pass
    human_judgment: false

# Metrics
duration: 55 min
completed: 2026-09-05
status: complete
---

# Phase 04 Plan 04: Investigate Engine Core Summary

**Signal-safe temp-worktree bisect engine with deterministic repro resolution chain and window-bounded binary search — REGRESS-01 mechanics complete per D-09/D-10/D-11**

## Performance

- **Duration:** 55 min
- **Started:** 2026-09-05T14:30:00Z
- **Completed:** 2026-09-05T15:25:00Z
- **Tasks:** 3
- **Files modified:** 6 created

## Accomplishments

- Worktree isolation complete: detached temp worktrees created with `git worktree add --detach`, user checkout provably untouched, guaranteed cleanup via defer + signal handlers + startup orphan prune sweep
- Deterministic LLL-free check function with strict resolution order (flag > config > auto-detect) executes safely inside disposable worktree via `exec.CommandContext` with arg-splitting, no shell, context cancellation, 64KB output cap
- Bounded binary search over `git rev-list --reverse baseline..head` candidates truncated to `bisect_max_commits` (default 50), with honest "not reproducible in window" semantics (never silent widen), confirmation pass requiring culprit fails AND parent passes

## Task Commits

Each task was committed atomically:

1. **Task 1 (RED+GREEN): Temp worktree manager — create, checkout, remove, prune sweep, signal safety (D-10)** - `d990a7ac` (test), `3178929e` (feat - accessor additions)
2. **Task 2 (RED+GREEN): Repro resolution chain and check executor (D-09)** - `3e5b5c16` (feat)
3. **Task 3 (RED+GREEN): Window-bounded binary search over rev-list candidates (D-11)** - `3f1e930b` (feat)

_Plan metadata commit follows this summary._

## Files Created/Modified

- `internal/intelligence/investigate/worktree.go` - WorktreeManager with Create, CheckoutTo, Remove, PruneOrphans, InstallSignalCleanup, ValidateRef
- `internal/intelligence/investigate/worktree_test.go` - Lifecycle tests: hostile ref rejection, create/checkout/remove cycle, orphan prune, parent HEAD unchanged
- `internal/intelligence/investigate/repro.go` - ResolveReproCommand (flag>config>auto), ExecuteRepro (ReproResult with ExitCode/Output/Matched), ErrNoReproCommand
- `internal/intelligence/investigate/repro_test.go` - Precedence table (6 combos), pass/fail/regex, cancellation, output bounding
- `internal/intelligence/investigate/bisect.go` - Candidates, FindCulprit, BisectStep, NotReproducibleInWindowError, IterationCapExceededError, detection helpers
- `internal/intelligence/investigate/bisect_test.go` - Rev-list candidates, seeded culprit attribution, not-reproducible sentinel, determinism across runs, iteration cap

## Decisions Made

- Worktree temp directory naming: `m31a-investigate-<pid>-<nanosecond-timestamp>` under `os.TempDir()` — unique per run, prevents collision, enables orphan detection
- ValidateRef exported from investigate package (mirroring git.go) — all ref-taking calls gate through it before any git exec
- ExecuteRepro: `exec.CommandContext` with `strings.Fields` splitting — no shell, no metacharacter interpretation; combined output capped at 64KB via limitedWriter
- FindCulprit pre-checks are mandatory: symptom must FAIL at window end (badRef) and PASS at window start (baseline); violation returns NotReproducibleInWindowError with checked SHA and observed result — window never silently widened
- Confirmation pass requires TWO independent observations: culprit fails AND its parent passes — only then attribution = verified (D-12)
- Binary search operates on rev-list candidate set (topological order), not date range — handles merge-heavy history correctly (Pitfall 3)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Test helper function deduplication across test files**
- **Found during:** Task 3 test execution (redeclared seedRepoWithHistory, splitLines in worktree_test.go and bisect_test.go)
- **Issue:** Go prohibits duplicate function declarations in the same package; both test files needed shared test helpers
- **Fix:** Consolidated all test helpers (seedRepoWithHistory, splitLines, splitLinesNoEmpty, split, createGoMod) into worktree_test.go; bisect_test.go uses them via package visibility
- **Files modified:** internal/intelligence/investigate/worktree_test.go (added helpers), internal/intelligence/investigate/bisect_test.go (removed duplicates)
- **Verification:** All tests pass, no duplicate declarations
- **Committed in:** 3f1e930b (part of Task 3 commit)

**2. [Rule 3 - Blocking] WorktreeDir/WorkDir accessors needed for bisect integration**
- **Found during:** Task 3 implementation (checkFn needed to read marker.txt from worktree)
- **Issue:** WorktreeManager and GitRunner didn't expose their working directory paths, needed for bisect check function to access checked-out files
- **Fix:** Added `WorktreeDir()` method to WorktreeManager and `WorkDir()` method to GitRunner
- **Files modified:** internal/intelligence/investigate/worktree.go
- **Verification:** TestFindCulprit_SeededCulprit and TestFindCulprit_DeterministicAcrossRuns pass
- **Committed in:** 3178929e (separate commit for accessor additions)

---

**Total deviations:** 2 auto-fixed (both blocking)
**Impact on plan:** Both fixes were prerequisites for Task 3's integration tests to work; no scope creep beyond plan's acceptance criteria.

## Issues Encountered

- **Test helper deduplication:** Go package rules required consolidating shared test helpers into a single _test.go file. Resolved by moving all helpers to worktree_test.go.
- **Disk quota in /tmp:** Test environment tmpfs quota exceeded during git worktree operations. Workaround: set TMPDIR=/home/snigdha/.tmp for test runs. Not a code issue.
- **Pre-existing legacy package failures:** `make test-fast` fails on 4 legacy packages (engine/session, engine/taskrunner, integrations/ledger, tools/todo) — verified identical at HEAD before any changes; out of scope per known environment notes.

## User Setup Required

None - no external service configuration required. All functionality uses local git and stdlib.

## Next Phase Readiness

- All must-have truths hold: worktree isolation provable (grep confirms checkout via git.New(wtDir) not parent repo runner), resolution order enforced, window bounded with honest not-reproducible semantics, attribution deterministic (two-run test passes)
- REGRESS-01 machinery complete without presentation — plan 04-05 adds confirmation attribution, two-tier report, events, and CLI
- Downstream plans can import WorktreeManager, ResolveReproCommand/ExecuteRepro, Candidates/FindCulprit, InvestigateGitRunner, and error sentinels without modification
- Legacy internal/engine/bisect/bisect.go remains untouched and compiling (verify.go dependency intact)

---

*Phase: 04-intelligence-features*
*Completed: 2026-09-05*

## Self-Check: PASSED

- All 6 created files verified present on disk (`[ -f ]` checks passed).
- All 5 task commits verified in history: d990a7ac, 3e5b5c16, 3f1e930b, 3178929e.
- All acceptance criteria re-run and passing:
  - Task 1: 4/4 tests + hostile ref rejection (12 subtests) + orphan prune + parent HEAD unchanged + go vet + race detector
  - Task 2: 6/6 tests (precedence 6 combos, pass/fail/regex, cancellation, output bound) + go vet + race detector + zero shell invocations
  - Task 3: 9/9 tests (candidates 4, culprit 4, iteration cap) + go vet + race detector
- make test-fast green for scoped packages: investigate, git, core
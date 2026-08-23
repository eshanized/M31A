# M31A Audit Root Cause Analysis

## Root Cause Clusters

### ROOT-001: Unscoped Git Ownership

**Affected Findings:**
- M31A-AUDIT-001 (P0) - Git Commit() commits unrelated changes
- M31A-AUDIT-007 (P1) - LLM file paths unvalidated in git/verify
- M31A-AUDIT-013 (P2) - Rollback stashes user changes
- M31A-AUDIT-014 (P2) - Bisect leaves repo in modified state
- NEW-002 (P1) - Ship phase fallback to AddAll()

**Root Cause:**
The codebase treats Git as a full-worktree ownership tool. The `Git.Commit()` method calls `AddAll()` which stages ALL changes in the worktree. While production code mostly uses `CommitWithFiles()`, there are critical fallbacks:
- `ship.go:108` - Falls back to `AddAll()` if task file add fails
- `Rollback.stashIfDirty()` - Stashes ALL uncommitted changes without distinction
- `Bisect.Run()` - Runs directly on user's repo without isolation

**Why This Is Architectural:**
1. No concept of "agent-owned files" vs "user-owned files" in the Git abstraction
2. Git wrapper exposes `AddAll()` and `Commit()` as public API
3. No session-scoped file tracking to distinguish agent modifications from user's pre-existing changes
4. The "sensitive file check" in `Commit()` only blocks known secret patterns, not user source files

**Correct Boundary:**
```
┌─────────────────────────────────────────────────────────────┐
│                    USER'S WORKTREE                           │
│  ┌──────────────────────┐  ┌─────────────────────────────┐  │
│  │   USER'S CHANGES     │  │      AGENT'S FILES          │  │
│  │   (invisible to      │  │   (tracked in session,      │  │
│  │    agent)            │  │    committed via             │  │
│  │                      │  │    CommitWithFiles)         │  │
│  └──────────────────────┘  └─────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```
- Agent should ONLY see/modify files it created or was explicitly tasked with
- User's unstaged changes, untracked files, and staged changes must be preserved
- Git operations must be scoped to agent's file list
- Bisect/rollback must run in isolated worktree

---

### ROOT-002: State Machine Validation Bypass

**Affected Findings:**
- M31A-AUDIT-003 (P0) - SetPhase() bypasses transition validation
- M31A-AUDIT-009 (P1) - Recovery restores phase without consistency validation
- NEW-001 (P1) - Session resume fails due to state machine sync bug

**Root Cause:**
`StateMachine.SetPhase()` (state_machine.go:107-116) exists for checkpoint/restore but performs ZERO validation:
```go
func (sm *StateMachine) SetPhase(phase m31types.WorkflowPhase) {
    sm.mu.Lock()
    defer sm.mu.Unlock()
    sm.currentPhase = phase  // Direct assignment, NO validation
    sm.discussPlanCycles = 0
    sm.history = append(sm.history, phase)
    ...
}
```

**Four production callers bypass validation:**
1. `engine_checkpoint.go:76` - `LoadCheckpointData()` from session checkpoints
2. `engine_checkpoint.go:115` - `Recover()` from recovery.json
3. `engine_checkpoint.go:185` - `RollbackCurrentPhase()` to previous phase
4. `recovery.go:177` - `RollbackToLastCheckpoint()`

**Why This Is Architectural:**
The state machine is treated as a data store (setter/getter) rather than a state machine (validated transitions). The design assumes checkpoint data is always valid, but:
- Recovery file can be stale (from previous workflow)
- Recovery file can be corrupted
- Session ID not validated on restore
- Task state not validated against restored phase

**Correct Boundary:**
```go
// Validated setter for checkpoint restore
func (sm *StateMachine) RestorePhase(phase m31types.WorkflowPhase, expectedFrom m31types.WorkflowPhase) error {
    sm.mu.Lock()
    defer sm.mu.Unlock()
    
    // Validate the restored phase is reachable
    if !sm.isValidRestore(expectedFrom, phase) {
        return fmt.Errorf("invalid phase restore: %s -> %s", expectedFrom, phase)
    }
    
    sm.currentPhase = phase
    sm.history = append(sm.history, phase)
    ...
}
```
Checkpoint/restore must validate phase against valid transitions AND validate session identity.

---

### ROOT-003: Verification Threshold Semantics

**Affected Findings:**
- M31A-AUDIT-002 (P0) - 90% threshold allows failed work to pass

**Root Cause:**
`VerifySuccessThreshold = 0.90` at verify.go:21 is a product decision that treats partial failure as success:
```go
// verify.go:203-217
if passRate >= VerifySuccessThreshold {
    allOK = true  // OVERRIDES even with failedTasks > 0
    if len(failedTasks) > 0 {
        e.logger.Info("verification passed with warnings", ...)
    }
}
```

**Why This Is Architectural:**
This is not a bug but a product design decision that:
- Allows Ship with known-broken tasks (up to 10% failure rate)
- No user consent required for shipping broken code
- "Failed" tasks include test failures, syntax errors, missing files
- Downstream Ship phase proceeds without knowing verification had failures

**Correct Boundary:**
Verification must be all-or-nothing by default:
```go
// Option 1: Remove threshold entirely
allOK = len(failedTasks) == 0

// Option 2: Explicit opt-in with user confirmation
if cfg.Features.AllowPartialVerify && passRate >= threshold {
    // Require explicit user confirmation in TUI before Ship
}
```

---

### ROOT-004: Permission System TUI-Coupled

**Affected Findings:**
- M31A-AUDIT-006 (P1) - Headless mode bypasses permissions

**Root Cause:**
The `Dispatcher` permission flow requires a TUI listener:
```go
// dispatcher.go:494-530
func (d *Dispatcher) sendAndWaitForPermission(ctx, req, toolName) error {
    respCh := make(chan PermissionResponse, 1)
    d.pendingResponses.Store(req.ID, respCh)
    d.requestCh <- req  // Blocks if no listener (buffer=8)
    select {
    case resp = <-respCh:  // Waits for TUI response
    case <-timeoutCtx.Done():
        return ErrPermissionDenied
    }
}
```

**Why This Is Architectural:**
The permission system was designed exclusively for interactive TUI use. The dispatcher has no concept of "execution context" (interactive vs headless vs CI). The permission policy (allow/deny/ask) is static config, not context-aware.

**Correct Boundary:**
```go
type PermissionPolicy interface {
    Decide(ctx context.Context, tool string, risk RiskLevel, input ToolInput) PermissionDecision
}

// Implementations:
// - InteractivePolicy: sends to TUI, waits for user
// - HeadlessDenyPolicy: denies all dangerous tools
// - HeadlessAllowPolicy: allows all (with audit log)
// - CI/CD Policy: uses predefined rules
```
Dispatcher should accept a `PermissionPolicy` at construction, not hardcode TUI channel.

---

### ROOT-005: Recovery Incomplete - Task State Not Restored

**Affected Findings:**
- M31A-AUDIT-004 (P1) - Session recovery loses task state
- NEW-001 (P1) - Session resume fails (state machine sync)
- NEW-003 (P1) - Verify phase doesn't load checkpoint data

**Root Cause:**
Checkpoint/recovery saves partial state:
```go
// engine_checkpoint.go:27-50
func (e *Engine) SaveCheckpointData(goal string) {
    cp := &CheckpointData{
        Phase:       e.stateMachine.CurrentPhase(),
        Goal:        goal,
        PlanVersion: e.state.PlanVersion(),
        Decisions:   e.SnapshotDecisions(),
        Timestamp:   time.Now(),
    }
    // MISSING: Tasks, HealsAttempted, Task Status
}
```

On resume:
1. TUI loads session (has WorkflowPhase, Tasks in session.json)
2. Creates NEW engine (state machine at PhaseIdle)
3. TUI calls `RunPhaseCmd(sessionPhase)` → engine tries `Transition(PhaseIdle, sessionPhase)` → FAILS

**Why This Is Architectural:**
Task state is treated as "derived" (recomputed from plan) but actually has independent lifecycle:
- HealsAttempted counts
- Status (Done/Failed/Skipped/Unrecoverable)
- Acceptance criteria results
- These are NOT derivable from plan alone

**Correct Boundary:**
Checkpoint must include complete workflow state:
```go
type CheckpointData struct {
    Phase          WorkflowPhase
    Goal           string
    PlanVersion    int
    Tasks          []Task        // COMPLETE task state
    HealsAttempted map[int]int   // Per-task heal counts
    Messages       []Message
    Decisions      []DecisionReceipt
    SessionID      string        // For cross-session validation
    ...
}
```
On restore: `LoadCheckpointData()` must reinitialize TaskRunner with restored tasks.

---

### ROOT-006: Path Validation Inconsistent

**Affected Findings:**
- M31A-AUDIT-005 (P1) - Bash workdir symlink escape
- M31A-AUDIT-007 (P1) - LLM file paths unvalidated

**Root Cause:**
No centralized path validation layer. Each tool handles paths differently:
- `FileWrite` uses `ResolveAndContainPath()` (fileops/pathhelpers.go:15) ✓
- `Bash.Execute()` validates workdir with `filepath.Rel()` but NO symlink resolution ✗
- `verifyTask()` uses `filepath.Join()` without containment check ✗
- `CommitWithFiles()` passes paths directly to `git add --` ✗

**Why This Is Architectural:**
Path validation is scattered across tools. The LLM controls `task.Files` which flows through multiple systems without a single validation gate.

**Correct Boundary:**
```
                    ┌─────────────────────┐
                    │  Central Path Gate  │
                    │  ValidateTaskFiles()│
                    └──────────┬──────────┘
                               │
        ┌──────────────────────┼──────────────────────┐
        ▼                      ▼                      ▼
   ┌─────────┐           ┌─────────┐            ┌─────────┐
   │ FileOps │           │   Git   │            │  Bash   │
   │         │           │         │            │         │
   │ Trust   │           │ Trust   │            │ Trust   │
   │ validated│          │ validated│           │ validated│
   │ paths   │           │ paths   │           │ paths   │
   └─────────┘           └─────────┘            └─────────┘
```
All LLM-controlled paths validated ONCE at task creation (Plan phase) and before every use.

---

### ROOT-007: Shell Safety Heuristics Incomplete

**Affected Findings:**
- M31A-AUDIT-008 (P2) - Incomplete shell variable expansion blocking

**Root Cause:**
String-based parsing cannot capture shell grammar:
```go
// bash.go:438-453
func containsVariableExpansion(cmd string) bool {
    varExpansionRe := regexp.MustCompile(`\$[A-Za-z_]`)
    if varExpansionRe.MatchString(cmd) { return true }
    if strings.Contains(cmd, "${") { return true }
    if strings.Contains(cmd, "$((") { return true }
    return false
}
```

`checkCommandChaining()` splits on `; & |` but misses:
- `echo $(id)` - command substitution inside argument
- `cmd arg $(id) arg2` - nested in arguments
- Bash parameter expansions: `${VAR:-default}`, `${#VAR}`, `${VAR%pattern}`

**Why This Is Architectural:**
Regex/string parsing cannot fully capture POSIX shell grammar. The shell has context-sensitive parsing (quotes, escapes, nested expansions).

**Correct Boundary:**
Use a proper shell parser:
```go
import "mvdan.cc/sh/v3/syntax"
import "mvdan.cc/sh/v3/parser"

func validateShellCommand(cmd string) error {
    f, err := parser.Parse(strings.NewReader(cmd), "")
    if err != nil {
        return fmt.Errorf("parse error: %w", err)
    }
    // Walk AST, reject dangerous nodes:
    // - CommandSubst ($(...) or `...`)
    // - ArithmExpr ($((...)))
    // - ParamExp with dangerous flags
    // - Redir to /dev/tcp, /dev/udp
    return walkAST(f)
}
```

---

## Cross-Root-Cause Interactions

| Root Cause | Interacts With | Compound Effect |
|------------|----------------|-----------------|
| ROOT-001 (Git) | ROOT-002 (State) | Recovery restores phase but Git state may not match |
| ROOT-002 (State) | ROOT-005 (Recovery) | Session resume broken because state machine not synced |
| ROOT-004 (Perms) | ROOT-006 (Paths) | Headless can't validate paths before tool execution |
| ROOT-001 (Git) | ROOT-006 (Paths) | Unvalidated paths can escape via git operations |

---

## Remediation Priority Order

1. **ROOT-001** (Unscoped Git) - Data loss risk, affects 5 findings
2. **ROOT-002** (State Machine) - Session resume broken, affects 3 findings
3. **ROOT-003** (Verify Threshold) - Core correctness, standalone P0
4. **ROOT-004** (Permissions) - Headless broken, affects 1 P1 + NEW
5. **ROOT-005** (Recovery) - Session resume broken, affects 2 P1 + NEW
6. **ROOT-006** (Path Validation) - Security boundary, affects 2 P1
7. **ROOT-007** (Shell Safety) - Defense in depth, standalone P2

---

## Non-Code Remediation Needed

1. **Threat Model Update** - Document trust boundaries (agent vs user vs system)
2. **Security Audit Process** - Add path validation and Git safety to CI gates
3. **E2E Test Framework** - Build fixture-based E2E tests for Git safety, resume, headless
4. **Documentation** - Clearly document what "autonomous" means and doesn't mean
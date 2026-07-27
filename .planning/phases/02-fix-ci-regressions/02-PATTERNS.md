# Phase 2: Fix CI Test Regressions - Pattern Map

**Mapped:** 2026-07-24
**Files analyzed:** 7
**Analogs found:** 7 / 7

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `internal/core/config/merge.go` | service | transform | self (existing file) | exact |
| `internal/engine/session/manager.go` | service | file-I/O | self (existing file) | exact |
| `internal/engine/workflow/engine.go` | service | request-response | self (existing file) | exact |
| `internal/engine/workflow/execute.go` | service | request-response | self (existing file) | exact |
| `internal/testutil/ci/ci_test.go` | test | request-response | self (existing file) | exact |
| `internal/tools/exec/bash.go` | service | request-response | self (existing file) | exact |
| `internal/tools/extra_test.go` | test | request-response | `internal/tools/ai/question.go` | exact |

---

## Pattern Assignments

### `internal/core/config/merge.go` (service, transform)

**Analog:** `internal/core/config/merge.go` (lines 30-53)

**Imports pattern** (lines 1-20):
```go
package config

// mergeHelper provides type-safe merge operations that replicate the
// reflection-based merge behavior from the original mergeConfig.
type mergeHelper struct {
	defined map[string]bool
}

func newMergeHelper(defined map[string]bool) mergeHelper {
	if defined == nil {
		defined = make(map[string]bool)
	}
	return mergeHelper{defined: defined}
}

func (m mergeHelper) hasKey(key string) bool {
	return m.defined[key]
}
```

**Core pattern — boolField (template for fix)** (lines 30-37):
```go
// boolField copies overlay to base if explicitly defined or overlay is true.
// This preserves the original behavior: a bool set to false in the overlay
// only overrides the base if the key was explicitly present in the TOML.
func (m mergeHelper) boolField(base, overlay *bool, key string) {
	if m.hasKey(key) || *overlay {
		*base = *overlay
	}
}
```

**Current broken intField** (lines 39-45) — TO FIX:
```go
// intField copies overlay to base if explicitly defined in the overlay.
// This allows zero-value overrides (e.g., setting max_iterations = 0).
func (m mergeHelper) intField(base, overlay *int, key string) {
	if m.hasKey(key) {
		*base = *overlay
	}
}
```

**Current broken float64Field** (lines 47-53) — TO FIX:
```go
// float64Field copies overlay to base if explicitly defined in the overlay.
// This allows zero-value overrides.
func (m mergeHelper) float64Field(base, overlay *float64, key string) {
	if m.hasKey(key) {
		*base = *overlay
	}
}
```

**Usage pattern in mergeUIConfig** (lines 132-145):
```go
h.intField(&base.MaxIterations, &overlay.MaxIterations, prefix+".max_iterations")
h.stringField(&base.LeaderKey, &overlay.LeaderKey, prefix+".leader_key")
h.intField(&base.LeaderTimeoutMs, &overlay.LeaderTimeoutMs, prefix+".leader_timeout_ms")
// ... many more intField calls
```

**Fix pattern:** Add `|| *overlay != 0` condition to match `boolField`:
```go
func (m mergeHelper) intField(base, overlay *int, key string) {
	if m.hasKey(key) || *overlay != 0 {
		*base = *overlay
	}
}

func (m mergeHelper) float64Field(base, overlay *float64, key string) {
	if m.hasKey(key) || *overlay != 0 {
		*base = *overlay
	}
}
```

---

### `internal/engine/session/manager.go` (service, file-I/O)

**Analog:** `internal/engine/session/manager.go` (lines 334-373)

**sessionMetadata struct** (lines 336-349) — ADD Label FIELD:
```go
// sessionMetadata is a metadata-only view of Session for JSON serialization.
// It excludes Messages and Tasks to keep session.json lightweight.
type sessionMetadata struct {
	SchemaVersion    int                 `json:"schema_version"`
	ID               string              `json:"id"`
	ChildrenIDs      []string            `json:"children_ids"`
	Model            string              `json:"model"`
	Provider         string              `json:"provider"`
	StartedAt        time.Time           `json:"started_at"`
	MessageCount     int                 `json:"message_count"`
	WorkflowPhase    types.WorkflowPhase `json:"workflow_phase"`
	Project          *types.ProjectState `json:"project,omitempty"`
	ResumedAt        *time.Time          `json:"resumed_at,omitempty"`
	WorkflowGoal     string              `json:"workflow_goal,omitempty"`
	DiscussQuestions []string            `json:"discuss_questions,omitempty"`
	// ADD: Label string `json:"label,omitempty"`
}
```

**saveSessionAtomic** (lines 353-373) — ADD Label TO STRUCT LITERAL:
```go
func (m *Manager) saveSessionAtomic(session *Session) error {
	meta := sessionMetadata{
		SchemaVersion:    session.SchemaVersion,
		ID:               session.ID,
		ChildrenIDs:      session.ChildrenIDs,
		Model:            session.Model,
		Provider:         session.Provider,
		StartedAt:        session.StartedAt,
		MessageCount:     session.MessageCount,
		WorkflowPhase:    session.WorkflowPhase,
		Project:          session.Project,
		ResumedAt:        session.ResumedAt,
		WorkflowGoal:     session.WorkflowGoal,
		DiscussQuestions: session.DiscussQuestions,
		// ADD: Label: session.Label,
	}
	data, err := json.Marshal(meta)
	if err != nil {
		return fmt.Errorf("cannot marshal session: %w", err)
	}
	return m.atomicWrite(m.sessionJSONPath(), data)
}
```

**RenameSession** (lines 512-524) — Already updates session.Label, just needs persistence:
```go
func (m *Manager) RenameSession(id, label string) error {
	if err := m.lock.Lock(); err != nil {
		return fmt.Errorf("session lock: %w", err)
	}
	defer m.lock.Unlock() //nolint:errcheck

	sess, err := m.loadSessionMetadata()
	if err != nil {
		return fmt.Errorf("load session: %w", err)
	}
	sess.Label = label
	return m.saveSessionAtomic(sess)
}
```

**Pattern:** Struct tags with `omitempty` for optional fields; atomic write via temp file + rename.

---

### `internal/engine/workflow/engine.go` (service, request-response)

**Analog:** `internal/engine/workflow/engine.go` — `RunPhase` method (lines 838-914)

**RunPhase pattern** (lines 838-914):
```go
func (e *Engine) RunPhase(ctx context.Context, phase m31types.WorkflowPhase, goal string) (*PhaseResult, error) {
	e.state.currentGoal = goal
	// Budget guardrail
	if e.cfg != nil && e.cfg.Features.BudgetLimitUSD > 0 {
		cost := e.costTracker.TotalCost()
		if cost >= e.cfg.Features.BudgetLimitUSD {
			return &PhaseResult{...}, fmt.Errorf("budget limit exceeded")
		}
	}

	start := time.Now()

	// PrePhaseSetup with messagesMu lock
	e.state.messagesMu.Lock()
	var err error
	e.state.Messages, err = e.phaseCoordinator.PrePhaseSetup(...)
	e.state.messagesMu.Unlock()
	if err != nil {
		return &PhaseResult{...}, err
	}

	e.toolCallsSinceLastCompact = 0

	// TRANSITION VALIDATION — THIS IS WHAT TESTS NEED TO BYPASS
	from := e.stateMachine.CurrentPhase()
	if err := e.stateMachine.Transition(from, phase); err != nil {
		return nil, fmt.Errorf("phase transition to %s: %w", phase, err)
	}

	// Phase execution switch
	switch phase {
	case m31types.PhaseInitialize:
		result, err = e.runInitialize(ctx, goal)
	// ... other cases
	}

	if result != nil {
		result.WorkflowMode = e.WorkflowMode()
	}

	e.phaseCoordinator.PostPhaseExecution(phase, result, start)
	return result, err
}
```

**New method to add — RunPhaseDirect (test bypass)**:
```go
// RunPhaseDirect executes a single phase without state machine transition validation.
// INTENDED FOR TEST USE ONLY — does not enforce phase ordering.
// Production code MUST use RunPhase which validates transitions.
func (e *Engine) RunPhaseDirect(ctx context.Context, phase m31types.WorkflowPhase, goal string) (*PhaseResult, error) {
	e.state.currentGoal = goal

	// Budget check (same as RunPhase)
	if e.cfg != nil && e.cfg.Features.BudgetLimitUSD > 0 {
		cost := e.costTracker.TotalCost()
		if cost >= e.cfg.Features.BudgetLimitUSD {
			return &PhaseResult{Phase: phase, Success: false, Error: fmt.Sprintf("budget limit exceeded")}, fmt.Errorf("budget limit exceeded")
		}
	}

	start := time.Now()

	// PrePhaseSetup (same as RunPhase)
	e.state.messagesMu.Lock()
	var err error
	e.state.Messages, err = e.phaseCoordinator.PrePhaseSetup(ctx, phase, &budgetConfigAdapter{cfg: e.cfg}, e.state.Messages, e.proactiveCompactCheck)
	e.state.messagesMu.Unlock()
	if err != nil {
		return &PhaseResult{Phase: phase, Success: false, Error: err.Error()}, err
	}

	e.toolCallsSinceLastCompact = 0

	// SKIP: e.stateMachine.Transition(from, phase) — this is the bypass

	var result *PhaseResult
	switch phase {
	case m31types.PhaseInitialize:
		result, err = e.runInitialize(ctx, goal)
	case m31types.PhaseDiscuss:
		result, err = e.runDiscuss(ctx, goal)
	case m31types.PhasePlan:
		result, err = e.runPlan(ctx, goal)
	case m31types.PhaseExecute:
		result, err = e.runExecute(ctx, goal)
	case m31types.PhaseVerify:
		result, err = e.runVerify(ctx, goal)
	case m31types.PhaseRuntime:
		result, err = e.runRuntime(ctx, goal)
	case m31types.PhaseShip:
		result, err = e.runShip(ctx, goal)
	default:
		return nil, fmt.Errorf("%w: unknown phase %s", m31errors.ErrPhaseTransition, phase)
	}

	if result != nil {
		result.WorkflowMode = e.WorkflowMode()
	}

	e.phaseCoordinator.PostPhaseExecution(phase, result, start)
	return result, err
}
```

**Naming convention:** `RunPhaseDirect` or `RunPhaseForTest` — D-02 requires clear test-only naming.

---

### `internal/engine/workflow/execute.go` (service, request-response)

**Analog:** `internal/engine/workflow/execute.go` (lines 567-573)

**Current broken pattern — heal loop with ineffectual assignment** (around line 571):
```go
// Inside executeTaskWithTools heal loop (simplified from lines 244-710)
for task.HealsAttempted < maxHeals {
	messages := e.buildExecuteContext(ctx, *task, allTasks, goal)  // := declares NEW variable each iteration
	// ...
	messages = e.proactiveCompactCheck(messages)  // assigns to loop-local variable
	// Next iteration: messages := e.buildExecuteContext(...) redeclares, losing compacted result
}
```

**Fixed pattern** (restructure loop to use outer variable):
```go
// Declare messages outside the loop
var messages []m31types.Message

for task.HealsAttempted < maxHeals {
	messages = e.buildExecuteContext(ctx, *task, allTasks, goal)  // = assigns to outer variable
	// ...
	messages = e.proactiveCompactCheck(messages)  // result preserved for next iteration
	// ...
}
```

**Key insight:** The `:=` at line 279 (`messages := e.buildExecuteContext(...)`) creates a new variable shadowing the outer one. Change to `=` assignment to outer scope variable.

---

### `internal/testutil/ci/ci_test.go` (test, request-response)

**Analog:** `internal/testutil/ci/ci_test.go` (lines 9-77)

**Current broken pattern** (lines 68-76):
```go
for _, tc := range tests {
	t.Run(tc.name, func(t *testing.T) {
		t.Parallel()  // RACE: parallel subtests mutate global env vars
		tc.setup()
		if got := IsCI(); got != tc.wantResult {
			t.Errorf("IsCI() = %v, want %v", got, tc.wantResult)
		}
	})
}
```

**Fixed pattern — use t.Setenv()** (Go 1.17+):
```go
for _, tc := range tests {
	t.Run(tc.name, func(t *testing.T) {
		// t.Setenv() auto-restores after subtest — SAFE with t.Parallel()
		t.Setenv(tc.envVar, tc.envValue)
		if got := IsCI(); got != tc.wantResult {
			t.Errorf("IsCI() = %v, want %v", got, tc.wantResult)
		}
	})
}
```

**Alternative (if t.Setenv not preferred):** Remove `t.Parallel()` entirely — test is fast.

**Pattern:** `t.Setenv(key, value)` replaces manual `os.Setenv`/`os.Unsetenv` with deferred cleanup. Auto-restores process env after subtest completes.

---

### `internal/tools/exec/bash.go` (service, request-response)

**Analog:** `internal/tools/exec/bash.go` (lines 342-444) + git commit `f35077bd`

**Current dangerousCommandPatterns** (lines 344-369) — NEEDS EXPANSION from f35077bd:
```go
var dangerousCommandPatterns = []struct {
	pattern string
	reason  string
}{
	{"rm -rf /", "recursive root deletion"},
	{"rm -rf /*", "recursive root deletion with wildcard"},
	{"mkfs", "filesystem formatting"},
	{"dd if=", "disk imaging/overwriting"},
	{"> /dev/sd", "direct disk write"},
	{"fdisk", "disk partitioning"},
	{"parted", "disk partitioning"},
	{":(){ :|:& };:", "fork bomb"},
	{"chmod -R 777 /", "recursive permission change on root"},
	{"chown -R", "recursive ownership change"},
	{"shutdown", "system shutdown"},
	{"reboot", "system reboot"},
	{"halt", "system halt"},
	{"poweroff", "system power off"},
	{"init 0", "system halt via init"},
	{"init 6", "system reboot via init"},
	{"killall", "kill all processes by name"},
	{"pkill -9", "force kill all processes"},
	{"mv / /", "move root directory"},
	{"mv /* ", "moving from root filesystem"},
	{"cp /dev/zero /dev/sd", "zeroing disk device"},
	{"truncate -s 0 /dev/sd", "truncating disk device"},
	// MISSING from f35077bd - NEED TO ADD:
	// {"shred", "secure file deletion"},
	// {"wipefs", "filesystem signature wiping"},
	// {"nc -l", "netcat listener"},
	// {"ncat -l", "netcat listener"},
	// {"socat", "socket relay"},
	// {">/dev/tcp", "TCP redirection"},
	// {"</dev/tcp", "TCP input redirection"},
	// {"curl|sh", "curl to shell pipe"},
	// {"wget|bash", "wget to bash pipe"},
	// {"curl|bash", "curl to bash pipe"},
	// {"rm -rf *", "recursive delete all"},
	// {"rm -rf ~", "recursive delete home"},
}
```

**Current dangerousObfuscationPatterns** (lines 371-383) — NEEDS REGEX COMPILATION:
```go
var dangerousObfuscationPatterns = []struct {
	pattern string
	reason  string
}{
	{"base64 -d", "base64 decode (potential obfuscation)"},
	{"echo.*|.*sh", "piped shell execution"},  // BROKEN: regex metachars in strings.Contains
	{"curl.*|.*sh", "curl to shell pipe"},       // BROKEN
	{"wget.*|.*sh", "wget to shell pipe"},       // BROKEN
	{"eval.*", "eval command execution"},         // BROKEN
	{"exec.*", "exec replacement"},               // BROKEN
	{"source /dev/stdin", "source from stdin"},
	{". /dev/stdin", "dot source from stdin"},
}
```

**Fixed pattern — compiled regexes for obfuscation** (D-03, D-04):
```go
import "regexp"

// dangerousObfuscationRegexes — COMPILED REGEX for patterns with metacharacters
var dangerousObfuscationRegexes = []struct {
	re     *regexp.Regexp
	reason string
}{
	{regexp.MustCompile(`base64\s+-d`), "base64 decode (potential obfuscation)"},
	{regexp.MustCompile(`echo.*\|.*sh`), "piped shell execution"},
	{regexp.MustCompile(`curl.*\|.*sh`), "curl to shell pipe"},
	{regexp.MustCompile(`wget.*\|.*sh`), "wget to shell pipe"},
	{regexp.MustCompile(`eval\s+`), "eval command execution"},
	{regexp.MustCompile(`exec\s+`), "exec replacement"},
	{regexp.MustCompile(`source\s+/dev/stdin`), "source from stdin"},
	{regexp.MustCompile(`\.\s+/dev/stdin`), "dot source from stdin"},
}
```

**CheckDangerousCommand — updated to use regex** (lines 386-420):
```go
func CheckDangerousCommand(command string, additionalBlocked []string, additionalObfuscation []string) (string, bool) {
	normalized := normalizeCommand(command)

	// Variable expansion check — REGEX (D-04)
	if containsVariableExpansion(normalized) {
		return "command contains variable expansion (potential injection)", true
	}

	// Check compiled baseline (exact substring — safe patterns, no regex metachars)
	for _, dp := range dangerousCommandPatterns {
		if strings.Contains(normalized, dp.pattern) {
			return fmt.Sprintf("blocked dangerous command: %s (pattern: %q)", dp.reason, dp.pattern), true
		}
	}

	// Check COMPILED obfuscation regexes
	for _, dp := range dangerousObfuscationRegexes {
		if dp.re.MatchString(normalized) {
			return fmt.Sprintf("blocked dangerous command: %s (pattern: %q)", dp.reason, dp.re.String()), true
		}
	}

	// Custom blocklist — EXACT PREFIX MATCH (D-05)
	if reason, blocked := checkCustomBlocklist(normalized, additionalBlocked); blocked {
		return reason, true
	}

	// Custom obfuscation patterns — COMPILED REGEX
	for _, pattern := range additionalObfuscation {
		re, err := regexp.Compile(pattern)
		if err != nil {
			continue // skip invalid patterns
		}
		if re.MatchString(normalized) {
			return fmt.Sprintf("blocked by user-configured obfuscation pattern: %q", pattern), true
		}
	}

	return "", false
}
```

**containsVariableExpansion — REGEX FIX** (lines 431-444) (D-04):
```go
// BEFORE (broken - literal string match):
func containsVariableExpansion(cmd string) bool {
	patterns := []string{"$[A-Za-z_]", "${", "$(", "`"}
	for _, p := range patterns {
		if strings.Contains(cmd, p) {
			return true
		}
	}
	return false
}

// AFTER (regex match for $VAR pattern):
func containsVariableExpansion(cmd string) bool {
	varExpansionRe := regexp.MustCompile(`\$[A-Za-z_]`)
	if varExpansionRe.MatchString(cmd) {
		return true
	}
	if strings.Contains(cmd, "${") {
		return true
	}
	if strings.Contains(cmd, "$(") {
		return true
	}
	if strings.Contains(cmd, "`") {
		return true
	}
	return false
}
```

**Custom blocklist — EXACT PREFIX MATCH** (D-05):
```go
func checkCustomBlocklist(normalized string, additionalBlocked []string) (string, bool) {
	parts := strings.Fields(normalized)
	if len(parts) == 0 {
		return "", false
	}
	cmd := parts[0]
	
	for _, pattern := range additionalBlocked {
		if cmd == pattern {  // EXACT match on command name, not substring
			return fmt.Sprintf("blocked by user-configured command: %q", pattern), true
		}
	}
	return "", false
}
```

**Chaining detection — parse $(...) inner command** (D-06):
```go
func checkCommandChaining(normalized string) (string, bool) {
	separators := regexp.MustCompile(`\s*[;&|]{1,2}\s*`)
	segments := separators.Split(normalized, -1)
	
	for _, seg := range segments {
		seg = strings.TrimSpace(seg)
		if seg == "" {
			continue
		}
		
		// Check for command substitution $(...)
		if strings.HasPrefix(seg, "$(") && strings.HasSuffix(seg, ")") {
			inner := strings.TrimSpace(seg[2 : len(seg)-1])
			if reason, blocked := CheckDangerousCommand(inner, nil, nil); blocked {
				return fmt.Sprintf("blocked dangerous command substitution: %s (inner: %s)", reason, inner), true
			}
			continue
		}
		
		// Check for backtick command substitution
		if strings.HasPrefix(seg, "`") && strings.HasSuffix(seg, "`") {
			inner := strings.TrimSpace(seg[1 : len(seg)-1])
			if reason, blocked := CheckDangerousCommand(inner, nil, nil); blocked {
				return fmt.Sprintf("blocked dangerous backtick substitution: %s (inner: %s)", reason, inner), true
			}
			continue
		}
		
		// Check segment against dangerous patterns
		if reason, blocked := checkSegmentAgainstPatterns(seg); blocked {
			return fmt.Sprintf("blocked dangerous command in chain: %s (segment: %s)", reason, seg), true
		}
	}
	return "", false
}
```

---

### `internal/tools/extra_test.go` (test, request-response)

**Analog:** `internal/tools/ai/question.go` (lines 69-156) — `Execute` return pattern

**Execute return pattern** (lines 139-155):
```go
select {
case resp := <-respCh:
    elapsed := time.Since(start).Milliseconds()
    return types.ToolResult{
        Output:     resp.Answer,
        DurationMs: elapsed,
    }, nil
case <-time.After(time.Duration(timeoutSecs) * time.Second):
    t.pending.Delete(reqID)
    return types.ToolResult{
        Error:      fmt.Sprintf("question timed out after %d seconds", timeoutSecs),
        DurationMs: time.Since(start).Milliseconds(),
    }, nil  // NOTE: Go error is NIL, error message in ToolResult.Error
case <-ctx.Done():
    t.pending.Delete(reqID)
    return types.ToolResult{}, ctx.Err()
}
```

**Current broken test** (lines 3601-3618):
```go
func TestAskUserQuestion_Timeout(t *testing.T) {
	reqCh := make(chan types.QuestionRequest, 4)
	respCh := make(chan types.QuestionResponse, 4)
	var pending sync.Map
	q := NewAskUserQuestion(reqCh, respCh, &pending)

	_, err := q.Execute(context.Background(), types.ToolInput{
		Name: "AskUserQuestion",
		Params: map[string]any{
			"question": "What?",
			"timeout":  float64(1),
		},
	})
	if err == nil {  // WRONG: err is always nil on timeout!
		t.Error("expected timeout error")
	}
}
```

**Fixed test** — assert on `result.Error` not `err`:
```go
func TestAskUserQuestion_Timeout(t *testing.T) {
	reqCh := make(chan types.QuestionRequest, 4)
	respCh := make(chan types.QuestionResponse, 4)
	var pending sync.Map
	q := NewAskUserQuestion(reqCh, respCh, &pending)

	// Use very short timeout and don't respond
	result, err := q.Execute(context.Background(), types.ToolInput{
		Name: "AskUserQuestion",
		Params: map[string]any{
			"question": "What?",
			"timeout":  float64(1),
		},
	})
	
	// FIX: Check result.Error, not err
	if result.Error == "" {
		t.Error("expected timeout error in result.Error")
	}
	// err should be nil — timeout is a tool result, not execution failure
	if err != nil {
		t.Errorf("unexpected Go error: %v", err)
	}
}
```

**Pattern:** Tool errors are returned in `ToolResult.Error` field; Go error return is for execution failures (panic, context cancel).

---

## Shared Patterns

### Error Handling Pattern (all service files)
**Source:** `internal/core/errors/errors.go` (sentinel errors) + usage in all packages
```go
// Wrap with %w for error chain
return fmt.Errorf("operation failed: %w", m31errors.ErrToolExecution)

// Sentinel errors for type checks
if errors.Is(err, m31errors.ErrContextExceeded) { ... }
if errors.Is(err, m31errors.ErrPhaseTransition) { ... }
```

### Test Pattern (all test files)
**Source:** `internal/engine/workflow/engine_extra_test.go`, `internal/tools/bash_security_test.go`
```go
func TestName(t *testing.T) {
    t.Parallel()  // for independent tests
    
    // Table-driven tests
    tests := []struct {
        name     string
        input    Input
        want     Want
        wantErr  bool
    }{
        {"case 1", input1, want1, false},
        {"case 2", input2, want2, true},
    }
    
    for _, tt := range tests {
        t.Run(tt.name, func(t *testing.T) {
            t.Parallel()  // or t.Setenv() for env isolation
            got, err := FunctionUnderTest(tt.input)
            if (err != nil) != tt.wantErr {
                t.Errorf("error = %v, wantErr %v", err, tt.wantErr)
            }
            if got != tt.want {
                t.Errorf("got %v, want %v", got, tt.want)
            }
        })
    }
}
```

### Atomic File Write Pattern
**Source:** `internal/engine/session/manager.go` lines 109-111
```go
func (m *Manager) atomicWrite(path string, data []byte) error {
    return atomicWrite(path, data)
}

// Implementation in fileutil.go or similar:
// Write to temp file, then atomic rename
```

### JSON Serialization with omitempty
**Source:** `internal/engine/session/manager.go` lines 336-349
```go
type sessionMetadata struct {
    SchemaVersion    int                 `json:"schema_version"`
    Label            string              `json:"label,omitempty"`  // omitempty for optional fields
}
```

---

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| (none) | | | All 7 files have exact analogs in the codebase |

---

## Metadata

**Analog search scope:** `internal/core/config/`, `internal/engine/session/`, `internal/engine/workflow/`, `internal/testutil/ci/`, `internal/tools/exec/`, `internal/tools/ai/`, `internal/tools/`
**Files scanned:** 12 primary files + test files
**Pattern extraction date:** 2026-07-24
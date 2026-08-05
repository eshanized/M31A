# Phase 1: Reliability First - Research

**Researched:** 2026-08-05
**Domain:** Go concurrency, error handling, process management, testing
**Confidence:** HIGH

## Summary

This research analyzes the M31A codebase's current concurrency patterns, error handling, cancellation mechanisms, and test infrastructure to support a reliability-focused refactor. The engine.go file (1832 lines) contains 10+ mutex fields protecting various state groups, with a clear separation emerging between session state (WorkflowState struct) and engine orchestration. The existing codebase already follows good patterns in many areas—context propagation is widespread, process group management exists for SIGKILL, and race tests cover key state boundaries. The primary gaps are: (1) mutex organization could be more focused, (2) ~24 `defer x.Close() //nolint:errcheck` patterns need attention, (3) dispatcher edge cases and engine pause/resume lack integration tests, and (4) SSE parser edge cases need coverage.

**Primary recommendation:** Refactor mutexes into focused structs with documented lock ordering, add integration tests for dispatcher edge cases and pause/resume, and systematically address errcheck suppressions with proper error logging.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Phase transitions | Engine (workflow) | StateMachine | Engine orchestrates, StateMachine validates |
| Plan state (markdown, version) | WorkflowState | Engine | State owns the data, Engine provides accessors |
| Messages | WorkflowState | Engine | State owns the data, Engine provides accessors |
| Tool execution | Dispatcher | TaskRunner | Dispatcher handles permissions/rate limiting, TaskRunner handles parallelism |
| Process cancellation | Dispatcher (bash/devserver) | Engine (context) | Tool-level process groups, Engine-level context propagation |
| LLM streaming | Provider layer | SSEParser | Provider manages connection, Parser handles protocol |
| Subagent lifecycle | Manager | Engine | Manager orchestrates, Engine provides parent context |

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| sync.Mutex | stdlib | Protect shared state | Go's fundamental concurrency primitive |
| sync.RWMutex | stdlib | Read-heavy shared state | Allows concurrent reads |
| context.Context | stdlib | Cancellation propagation | Standard Go pattern for cancellation |
| sync.Once | stdlib | One-time initialization | Prevents TOCTOU races |
| sync/atomic | stdlib | Lock-free counters | Used for pendingPermCount |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| golang.org/x/sync/singleflight | v0.22.0 | Deduplicate concurrent requests | Already used in codebase |
| syscall | stdlib | Process group management | Unix-specific SIGTERM/SIGKILL |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Focused mutex structs | Channel-based state | Mutexes are simpler for this use case; channels better for state machines |
| sync.Map | RWMutex+map | sync.Map has higher overhead for small maps; RWMutex preferred here |

## Package Legitimacy Audit

No new external packages are being introduced in this phase. All packages are from the standard library or already in go.mod.

## Architecture Patterns

### System Architecture Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                        TUI (Bubble Tea)                     │
│  Update() ← tea.Msg ← MsgEmitter ← Engine.emit()          │
│  Query: PlanContent(), PlanVersion(), DiscussState()        │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│                     Engine (workflow)                        │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ WorkflowState                                        │   │
│  │   transitionMu ─── phase transitions                 │   │
│  │   planMu ──────── planMarkdown, planVersion          │   │
│  │   messagesMu ──── Messages                           │   │
│  │   intentResultMu ─ intentResult                      │   │
│  └─────────────────────────────────────────────────────┘   │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ Engine fields                                        │   │
│  │   modelIDMu ────── modelID, provider                 │   │
│  │   workflowModeMu ─ workflowMode                       │   │
│  │   perPhaseModelsMu ─ perPhaseModels                   │   │
│  │   codeIntelMu ──── codeIntel                         │   │
│  │   websiteTemplateMu ─ websiteTemplateDir              │   │
│  │   cacheMu ──────── cache                             │   │
│  │   discussMu ────── discussState                      │   │
│  │   pauseMu ──────── pause/resume channels             │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│                     Dispatcher                               │
│  mu ─────────────── tools, permissions, rules               │
│  batchMu ────────── batchApprovals                          │
│  rateTokens ─────── rate limiting                           │
│  concurrencySem ─── concurrent execution limit              │
└─────────────────────────────────────────────────────────────┘
```

### Recommended Project Structure
```
internal/engine/workflow/
├── engine.go              # Main orchestrator (refactor target)
├── workflow_state.go      # WorkflowState struct (extract from engine.go)
├── state_machine.go       # Phase transition validation
├── engine_concurrency.go  # Mutex definitions and helpers
└── engine_*.go            # Phase-specific logic (already split)
```

### Pattern 1: Focused Mutex Structs
**What:** Group related state and its protecting mutex into a single struct
**When to use:** When multiple fields are always accessed together under the same lock
**Example:**
```go
// Source: [ASSUMED] Based on existing WorkflowState pattern
type PlanState struct {
    mu              sync.RWMutex
    planMarkdown    string
    planVersion     int
    refineFeedback  string
    checkpointData  *CheckpointData
}

func (ps *PlanState) Content() string {
    ps.mu.RLock()
    defer ps.mu.RUnlock()
    return ps.planMarkdown
}
```

### Pattern 2: Documented Lock Ordering
**What:** Establish and document a total order for acquiring multiple locks
**When to use:** When code paths may need to acquire more than one lock
**Example:**
```go
// Lock ordering: transitionMu > planMu > messagesMu
// All lock acquisitions must follow this order to prevent deadlocks.
//
// Example (from RunPhase):
//   e.state.transitionMu.Lock()  // 1st
//   e.state.planMu.RLock()       // 2nd (if needed)
//   e.state.messagesMu.Lock()    // 3rd
```

### Pattern 3: Context-Based Cancellation
**What:** Use context.Context as the primary cancellation mechanism
**When to use:** All long-running operations, goroutines, and subprocess management
**Example:**
```go
// Source: [CITED: internal/tools/exec/bash.go]
go func() {
    select {
    case <-ctx.Done():
        if cmd.Process != nil {
            killOnce.Do(func() {
                _ = killProcessGroup(cmd.Process.Pid)
            })
        }
    case <-cmdDone:
    }
}()
```

### Pattern 4: Process Group Management
**What:** Send signals to entire process groups for reliable cancellation
**When to use:** When cancelling shell commands or child processes
**Example:**
```go
// Source: [CITED: internal/tools/exec/prockill_unix.go]
func killProcessGroup(pgid int) error {
    return syscall.Kill(-pgid, syscall.SIGTERM)
}
```

### Anti-Patterns to Avoid
- **Goroutine without context:** Always pass context to goroutines for cancellation
- **Missing defer cancel:** Always defer cancel() after context.WithCancel/WithTimeout
- **Lock ordering violation:** Never acquire locks in a different order than documented
- **Ignoring close errors:** Log at debug level, don't suppress entirely

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Process group management | Custom signal handling | syscall.Kill(-pgid, sig) | Standard Unix pattern, handles zombies |
| Rate limiting | Custom token bucket | sync/atomic + time.Ticker | Already implemented in dispatcher |
| Concurrent map access | sync.Map for small maps | RWMutex + map | Lower overhead, clearer semantics |
| One-time initialization | Manual flag checking | sync.Once | Prevents TOCTOU races |

**Key insight:** Go's standard library provides excellent concurrency primitives. The existing codebase already uses them well—the refactor is about organization, not new patterns.

## Common Pitfalls

### Pitfall 1: Mutex Ordering Deadlock
**What goes wrong:** Two goroutines acquire the same two mutexes in opposite order
**Why it happens:** No documented lock ordering, ad-hoc locking
**How to avoid:** Document lock ordering hierarchy and enforce it in code review
**Warning signs:** Intermittent test failures, goroutine dumps showing blocked mutexes

### Pitfall 2: Context Leak
**What goes wrong:** context.WithCancel created but cancel() never called
**Why it happens:** Missing defer cancel() after context creation
**How to avoid:** Always defer cancel() immediately after context creation
**Warning signs:** Growing memory usage, goroutine leaks

### Pitfall 3: Errcheck Suppression
**What goes wrong:** Close() errors silently ignored
**Why it happens:** Convenience nolint comments
**How to avoid:** Log close errors at debug level, remove nolint directives
**Warning signs:** Resource leaks, file descriptor exhaustion

### Pitfall 4: Race on Pause/Resume Channels
**What goes wrong:** Reading nil channel blocks forever
**Why it happens:** pauseCh/resumeCh accessed without pauseMu
**How to avoid:** Always access pause channels under pauseMu lock
**Warning signs:** Test hangs, goroutine leaks during pause/resume

## Code Examples

### Existing Race Tests
```go
// Source: [VERIFIED: internal/engine/workflow/engine_race_test.go]
func TestConcurrentSetModelAndProviderAndModel(t *testing.T) {
    engine, _ := setupTestEngine(t)
    var wg sync.WaitGroup
    const goroutines = 10
    const iterations = 100
    
    // Concurrent writers: SetModel
    for i := 0; i < goroutines; i++ {
        wg.Add(1)
        go func(id int) {
            defer wg.Done()
            for j := 0; j < iterations; j++ {
                p := &mockProviderWithModel{model: &types.ModelInfo{ID: "model-writer"}}
                engine.SetModel("model-writer", p)
            }
        }(i)
    }
    
    // Concurrent readers: providerAndModel
    for i := 0; i < goroutines; i++ {
        wg.Add(1)
        go func(id int) {
            defer wg.Done()
            for j := 0; j < iterations; j++ {
                _, _ = engine.providerAndModel()
            }
        }(i)
    }
    
    wg.Wait()
}
```

### Existing Process Group Cancellation
```go
// Source: [VERIFIED: internal/tools/exec/bash.go]
var killOnce sync.Once
cmdDone := make(chan struct{})
go func() {
    select {
    case <-ctx.Done():
        if cmd.Process != nil {
            killOnce.Do(func() {
                _ = killProcessGroup(cmd.Process.Pid)
            })
        }
    case <-cmdDone:
    }
}()
```

### Existing SSE Parser with Watchdog
```go
// Source: [VERIFIED: internal/integrations/provider/sse.go]
watchdog := time.AfterFunc(DefaultStreamTimeout, func() {
    _ = resp.Body.Close()
})

// Reset on each successful read
if p.watchdog != nil {
    p.watchdog.Reset(DefaultStreamTimeout)
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Single Engine struct with 10+ mutexes | WorkflowState extracted with focused mutexes | This phase | Cleaner separation of concerns |
| errcheck nolint suppressions | Debug-level error logging | This phase | Better observability |
| No pause/resume tests | Integration tests for pause/resume | This phase | Higher confidence |
| Dispatcher edge cases untested | Targeted integration tests | This phase | Fewer bugs |

**Deprecated/outdated:**
- `//nolint:errcheck` on Close() calls: Replace with explicit error logging

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Existing race tests cover main state boundaries | Test Infrastructure | May need additional tests |
| A2 | Lock ordering hierarchy (transitionMu > planMu > messagesMu) is sufficient | Architecture | May need additional locks |
| A3 | Process group management is sufficient for SIGKILL cancellation | Cancellation | May need timeout escalation |

## Open Questions

1. **Should WorkflowState be a separate file or remain in engine.go?**
   - RESOLVED: Separate file. Plan 01 extracts WorkflowState to workflow_state.go for clarity given 1800+ line engine.go.

2. **How should errcheck suppressions be addressed?**
   - RESOLVED: Log at debug level, remove nolint directives. Plan 02 Task 3 replaces all `defer x.Close() //nolint:errcheck` with debug-level slog logging.

3. **What additional tests are needed for dispatcher edge cases?**
   - RESOLVED: Plan 03 Task 2 creates dispatcher_edge_test.go covering concurrent permissions, batch approval, rate limit exhaustion, Stop during pending, context cancellation, and concurrency semaphore limits.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Go 1.25+ | Build | ✓ | 1.25.12 | — |
| make | Build | ✓ | — | — |
| golangci-lint | Lint | ✓ | — | — |

**Missing dependencies with no fallback:** None

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Go testing |
| Config file | go.mod |
| Quick run command | `make test-fast` |
| Full suite command | `make test` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| REL-01 | Mutex refactoring | unit | `go test -race ./internal/engine/workflow/...` | ✅ engine_race_test.go |
| REL-02 | Context propagation | integration | `go test -race ./internal/tools/...` | ❌ Wave 0 |
| REL-03 | Process cancellation | integration | `go test -race ./internal/tools/exec/...` | ❌ Wave 0 |
| REL-04 | Error handling | unit | `go test ./internal/...` | ❌ Wave 0 |
| REL-05 | Pause/resume | integration | `go test -race ./internal/engine/workflow/... -run Pause` | ❌ Wave 0 |
| REL-06 | Recovery | integration | `go test -race ./internal/engine/workflow/... -run TestRecovery` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `make test-fast`
- **Per wave merge:** `make test`
- **Phase gate:** Full suite green before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] `internal/engine/workflow/pause_resume_test.go` — covers REL-05 (pause/resume integration)
- [ ] `internal/tools/dispatcher_edge_test.go` — covers dispatcher edge cases
- [ ] `internal/engine/workflow/cancellation_test.go` — covers REL-02, REL-03 (context propagation)
- [ ] `internal/engine/workflow/error_handling_test.go` — covers REL-04 (error wrapping)
- [ ] `internal/engine/workflow/state_machine_test.go` — covers state machine transitions
- [ ] `internal/engine/workflow/recovery_test.go` — covers REL-06 (crash/forced exit/interrupted session recovery)

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V5 Input Validation | yes | Tool input validation in dispatcher |
| V6 Cryptography | no | Not applicable to this phase |

### Known Threat Patterns for Go

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Process injection | Tampering | Command validation in Bash tool |
| Resource exhaustion | Denial of Service | Rate limiting, concurrency semaphores |
| Goroutine leak | Denial of Service | Context propagation, proper cancellation |

## Sources

### Primary (HIGH confidence)
- [VERIFIED: internal/engine/workflow/engine.go] — Main refactor target (1832 lines, 10+ mutexes)
- [VERIFIED: internal/tools/dispatcher.go] — Tool execution dispatcher
- [VERIFIED: internal/engine/taskrunner/runner.go] — Parallel task execution
- [VERIFIED: internal/integrations/provider/sse.go] — SSE parser
- [VERIFIED: internal/tools/exec/bash.go] — Process group management

### Secondary (MEDIUM confidence)
- [CITED: Go standard library documentation] — sync.Mutex, context.Context patterns

### Tertiary (LOW confidence)
- [ASSUMED] Lock ordering hierarchy may need adjustment during implementation

## Metadata

**Confidence breakdown:**
- Standard Stack: HIGH - All packages are stdlib or already in go.mod
- Architecture: HIGH - Existing patterns are well-understood
- Pitfalls: HIGH - Common Go concurrency issues well-documented

**Research date:** 2026-08-05
**Valid until:** 2026-09-04 (30 days for stable patterns)

## RESEARCH COMPLETE

**Phase:** 1 - Reliability First
**Confidence:** HIGH

### Key Findings
- Engine has 10+ mutex fields that can be organized into focused structs
- Existing race tests cover main state boundaries (model, plan, messages, checkpoint)
- Process group management exists for SIGKILL cancellation (prockill_unix.go)
- ~24 errcheck suppressions need attention (debug-level logging recommended)
- Pause/resume channels are properly protected by pauseMu but lack integration tests

### File Created
`.planning/phases/01-reliability-first/01-RESEARCH.md`

### Confidence Assessment
| Area | Level | Reason |
|------|-------|--------|
| Standard Stack | HIGH | All packages are stdlib or already in go.mod |
| Architecture | HIGH | Existing patterns are well-understood from codebase analysis |
| Pitfalls | HIGH | Common Go concurrency issues well-documented |

### Open Questions
- Whether to split WorkflowState into separate file
- How to systematically address errcheck suppressions
- Which dispatcher edge cases need additional test coverage

### Ready for Planning
Research complete. Planner can now create PLAN.md files.

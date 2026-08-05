# Phase 1: Reliability First - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-08-05
**Phase:** 01-reliability-first
**Areas discussed:** Concurrency architecture, Cancellation propagation, Error handling standards, Testing confidence strategy

---

## Concurrency Architecture

### Restructuring Approach

| Option | Description | Selected |
|--------|-------------|----------|
| Focused structs (Recommended) | Extract state into smaller structs (e.g., WorkflowState, ModelState, PhaseState), each with its own mutex. Reduces lock scope and reasoning complexity. | ✓ |
| Channel-based state | Replace mutex-protected state with channels and goroutines (actor model). More Go-idiomatic but bigger refactor, higher risk. | |
| Minimal change | Keep current structure, just fix specific race conditions found by -race detector. Lower risk but doesn't address tech debt. | |

**User's choice:** Focused structs (Recommended)
**Notes:** None

### Concurrent Writers

| Option | Description | Selected |
|--------|-------------|----------|
| TUI + workflow only (Recommended) | Two writers: Bubble Tea Update() and workflow goroutine. Current design — mutexes just need to prevent these two from conflicting. | ✓ |
| Multi-session | Support multiple concurrent sessions in same project. Requires per-session isolation, bigger scope. | |

**User's choice:** TUI + workflow only (Recommended)
**Notes:** None

### Lock Ordering

| Option | Description | Selected |
|--------|-------------|----------|
| Yes, documented order (Recommended) | Define a lock hierarchy (e.g., transitionMu > planMu > messagesMu) and enforce it via comments/convention. Low overhead, prevents most deadlocks. | ✓ |
| Yes, runtime checks | Use runtime deadlock detection (e.g., go-deadlock). Catches violations in tests but adds overhead. | |
| No, rely on -race | Just run tests with -race flag. Catches races but not deadlocks. | |

**User's choice:** Yes, documented order (Recommended)
**Notes:** None

### State Query API

| Option | Description | Selected |
|--------|-------------|----------|
| Reads inside Update() (Recommended) | All state reads happen in Bubble Tea's Update(). No external query API needed. Simpler, aligns with Elm architecture. | ✓ |
| Thread-safe query API | Export a QueryState() method with RWMutex for safe reads from goroutines. More flexible but adds complexity. | |

**User's choice:** Reads inside Update() only, no external query API
**Notes:** None

---

## Cancellation Propagation

### Mechanism

| Option | Description | Selected |
|--------|-------------|----------|
| Context-based (Recommended) | Pass context.Context through all goroutines. Cancel parent context → all children stop. Standard Go pattern, works with select/context.Done() | ✓ |
| Signal channel | Dedicated cancel channel broadcast to all goroutines. More explicit but doesn't compose with stdlib libraries. | |

**User's choice:** Context-based (Recommended)
**Notes:** None

### LLM Streaming Cancellation

| Option | Description | Selected |
|--------|-------------|----------|
| Stop immediately (Recommended) | Cancel HTTP connection right away. Saves tokens, faster response to user intent. Partial content discarded. | ✓ |
| Finish current chunk | Let current SSE event complete, then stop. Cleaner state but wastes tokens if user already decided to stop. | |

**User's choice:** Stop immediately (Recommended)
**Notes:** None

### Tool Execution Cancellation

| Option | Description | Selected |
|--------|-------------|----------|
| Kill process tree (Recommended) | Send SIGKILL to process group. Ensures cleanup but may leave partial file writes. Most reliable for long-running commands. | ✓ |
| SIGTERM then SIGKILL | Graceful shutdown first (SIGTERM), then force kill after timeout. Better for tools that can clean up, but slower. | |
| Context only | Just cancel context, let tool finish naturally. Simplest but tools may keep running after user thinks they stopped. | |

**User's choice:** Kill process tree (Recommended)
**Notes:** None

### Subagent Cancellation

| Option | Description | Selected |
|--------|-------------|----------|
| Cancel all subagents (Recommended) | Propagate cancellation to all child subagents. They stop their work and clean up. Consistent with parent cancellation intent. | ✓ |
| Let subagents finish | Allow running subagents to complete their current task. More expensive but preserves work already done. | |

**User's choice:** Cancel all subagents when parent is cancelled
**Notes:** None

---

## Error Handling Standards

### Error Wrapping

| Option | Description | Selected |
|--------|-------------|----------|
| Wrap with context (Recommended) | Use fmt.Errorf("%w", err) with descriptive context. Preserves error chain for errors.Is/As. Current pattern in most of the codebase. | ✓ |
| Typed error wrappers | Use custom error types (ToolError, ProviderError, etc.) for every package. More structured but verbose. | |
| Minimal wrapping | Only wrap where needed for user messages. Less boilerplate but harder to trace errors. | |

**User's choice:** Wrap with context (Recommended)
**Notes:** None

### Retry Logging

| Option | Description | Selected |
|--------|-------------|----------|
| Log at debug level (Recommended) | Log retry attempts at debug level. User sees clean output; developers can debug with -v. Keeps output uncluttered. | ✓ |
| Log at warn level | Show retries as warnings. Users see when things are being retried. More transparent but noisy. | |
| No logging | Don't log retries. Cleanest output but makes debugging harder. | |

**User's choice:** Log at debug level (Recommended)
**Notes:** None

### User-Facing Error Messages

| Option | Description | Selected |
|--------|-------------|----------|
| Actionable guidance (Recommended) | Tell users what failed, why (if known), and what they can do. E.g., "Provider X unavailable — check API key or try again later." | ✓ |
| Technical details | Include error codes, stack traces, raw messages. Useful for debugging but overwhelming for most users. | |
| Minimal | Just say "Something went wrong." Least confusing but least helpful. | |

**User's choice:** Actionable guidance (Recommended)
**Notes:** None

### Close Error Handling

| Option | Description | Selected |
|--------|-------------|----------|
| Log at debug (Recommended) | Log close errors at debug level. Most close failures are benign (e.g., file already closed). Developers can investigate when needed. | ✓ |
| Check critical only | Only check close errors for database connections and file locks. Ignore file closes. Less noise but risks missing real issues. | |
| Always check | Remove all nolint directives, handle every close error. Most thorough but verbose and may surface benign errors to users. | |

**User's choice:** Log at debug level (Recommended)
**Notes:** None

---

## Testing Confidence Strategy

### Race Detection

| Option | Description | Selected |
|--------|-------------|----------|
| Race detector + stress tests (Recommended) | Run -race on all tests. Add dedicated stress tests that hammer concurrent paths with high goroutine counts. Catches real races in CI. | ✓ |
| Formal verification | Use tools like govt or model checking. More thorough but steep learning curve and slow CI. | |
| Code review only | Rely on careful code review and lock ordering docs. No automated race detection beyond -race. | |

**User's choice:** Race detector + stress tests (Recommended)
**Notes:** None

### Test Balance

| Option | Description | Selected |
|--------|-------------|----------|
| Integration-first (Recommended) | Focus on tests that exercise real component interactions (engine + dispatcher + provider). Unit tests for pure logic only. Catches more real bugs. | ✓ |
| Unit-first | Maximize unit test coverage with mocks. Faster CI but may miss integration issues. | |
| Equal balance | 50/50 split. Comprehensive but may be slow. | |

**User's choice:** Integration-first (Recommended)
**Notes:** None

### Coverage Fluff Tests

| Option | Description | Selected |
|--------|-------------|----------|
| Delete and replace (Recommended) | Remove coverage fluff. Write focused integration tests that exercise real workflows. May drop coverage % initially but improves quality. | ✓ |
| Audit and trim | Keep useful tests from the files, remove the rest. Less coverage drop but slower process. | |
| Keep as-is | Leave coverage tests, add new integration tests on top. Maintains coverage numbers but doesn't address tech debt. | |

**User's choice:** Delete and replace (Recommended)
**Notes:** None

### Gap Priority

| Option | Description | Selected |
|--------|-------------|----------|
| Fill known gaps first (Recommended) | Address dispatcher edge cases, engine pause/resume, SSE parser first. These are high-risk areas with real bug potential. | ✓ |
| New workflow tests | Write end-to-end workflow tests that exercise full pipelines. Higher-level coverage but may miss edge cases. | |
| Both in parallel | Tackle gaps and new tests simultaneously. Most comprehensive but largest scope. | |

**User's choice:** Fill known gaps first (Recommended)
**Notes:** None

---

## the agent's Discretion

- Agent may choose appropriate file boundaries when splitting engine.go
- Agent may select specific stress test patterns and goroutine counts
- Agent may decide logging format/structure for debug-level retry logs

## Deferred Ideas

None — discussion stayed within phase scope

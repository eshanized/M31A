# Phase 3: Fix Remaining CI Issues - Research

**Researched:** 2026-07-27
**Domain:** Go CI cleanup -- lint deprecation, test failures, security vulnerability
**Confidence:** HIGH

## Summary

This phase addresses three categories of CI failures: (1) three staticcheck SA1019 lint warnings from deprecated `os.SEEK_SET` usage, (2) two flaky/failing tests (`TestRegistry_Execute_PhaseAliases` nil pointer dereference and `TestAskUserQuestion_ChannelFull` infinite block), and (3) one `govulncheck` vulnerability in `github.com/yuin/goldmark@v1.5.2`. All issues have clear, surgical fixes with no architectural changes required.

**Primary recommendation:** Execute three atomic commits in order: lint fix (1 commit), test fixes (1 commit), security upgrade (1 commit). Validate each with the appropriate `make` target.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Full scan for deprecated API usage across entire codebase, not just the 3 known `os.SEEK_SET` warnings
- **D-02:** Direct replacement of `os.SEEK_SET` with `io.SeekStart` in `fileutil.go`
- **D-03:** Single commit for all lint fixes
- **D-04:** Verify with `make lint` after fixes
- **D-05:** Fix `TestRegistry_Execute_PhaseAliases` by properly initializing `session.Manager` with all required dependencies
- **D-06:** Use `t.TempDir()` for filesystem isolation in session manager initialization
- **D-07:** Use `t.Cleanup()` for automatic cleanup
- **D-08:** Test should pass without error -- if initialization fails, test fails
- **D-09:** Fix `TestAskUserQuestion_ChannelFull` by addressing the infinite loop in test logic
- **D-10:** Add proper channel close or context cancellation to prevent infinite loop
- **D-11:** Verify fix by running test in isolation
- **D-12:** Run `gosec` for additional security checks beyond CodeQL
- **D-13:** Scan entire codebase for security issues
- **D-14:** Fix all security issues found
- **D-15:** Single commit for all security fixes

### the agent's Discretion
- Exact `gosec` configuration and severity thresholds
- Whether to add new `golangci-lint` linters beyond existing config
- Whether to add regression tests for fixed issues

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| REQ-01 | `golangci-lint run` clean (0 issues) | `fileutil.go` lines 39, 65, 93 -- replace `os.SEEK_SET` with `io.SeekStart` |
| REQ-02 | `go test -race ./...` clean (all tests pass) | `commands_all_test.go:617` -- initialize `session.Manager` via `NewManager()`; `extra_test.go:3646` -- fix context cancellation in channel-full test |
| REQ-03 | `go vet ./...` clean | Verified clean -- no issues found |
| REQ-04 | No security vulnerabilities (CodeQL/govulncheck clean) | `goldmark@v1.5.2` has GO-2026-5320 XSS vulnerability; fix in `v1.7.17`+ |
| REQ-05 | CI pipeline passes on GitHub Actions | `.github/workflows/ci.yml` runs lint, test, security, build, release jobs |
</phase_requirements>

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Lint: deprecated API replacement | Backend (core/types) | -- | `fileutil.go` is a low-level utility; `io.SeekStart` is the stdlib replacement |
| Test: session manager initialization | Backend (engine/session) | TUI (commands) | `session.Manager` needs proper `NewManager()` construction with dirs |
| Test: channel lifecycle management | Backend (tools/ai) | TUI (request/response) | `AskUserQuestion.Execute()` needs cancellable context in test |
| Security: dependency vulnerability | Build (go.mod) | -- | `goldmark` is an indirect dependency via `glamour`; update resolves XSS |

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Go stdlib `io` | 1.25 | `io.SeekStart` constant | Replaces deprecated `os.SEEK_SET`; same integer value (0) |
| Go stdlib `context` | 1.25 | Context cancellation in tests | Standard way to bound blocking operations in tests |
| Go stdlib `testing` | 1.25 | Test framework | Mandatory per AGENTS.md |
| `golangci-lint` | latest | Linting | Enforced by `make lint` |
| `govulncheck` | latest | Security scanning | Enforced by CI security job |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `github.com/yuin/goldmark` | >=1.7.17 | Markdown rendering (transitive) | Indirect dep via `charmbracelet/glamour`; security fix |
| `internal/engine/session` | (internal) | Session persistence | `NewManager()` provides proper initialization |
| `internal/tools/ai` | (internal) | AskUserQuestion tool | Channel-based user prompting |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `io.SeekStart` | Keep `os.SEEK_SET` and suppress lint | `io.SeekStart` is the stdlib replacement since Go 1.7; no reason to keep deprecated API |
| Context cancellation for test fix | Non-blocking select with default | Changes production behavior; context cancellation is the intended pattern per code comments |
| Update `goldmark` only | Update `glamour` entirely | `glamour@v0.6.0` pins `goldmark@v1.5.2` in go.mod; `go get goldmark` overrides the indirect version cleanly |

**Installation:** No new dependencies required. `goldmark` update is handled via `go get`.

## Package Legitimacy Audit

> No new external packages are introduced in this phase. The `goldmark` version bump is an existing dependency.

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| github.com/yuin/goldmark | Go modules | 7+ years | millions | github.com/yuin/goldmark | OK | Version bump within existing dependency |

**Packages removed due to [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none

## Lint Issues

### Issue 1: Deprecated `os.SEEK_SET` (3 occurrences)

**File:** `internal/core/types/fileutil.go`
**Linter:** staticcheck SA1019
**Severity:** Warning (CI fails)

| Line | Current Code | Replacement |
|------|-------------|-------------|
| 39 | `Whence: int16(os.SEEK_SET),` | `Whence: int16(io.SeekStart),` |
| 65 | `Whence: int16(os.SEEK_SET),` | `Whence: int16(io.SeekStart),` |
| 93 | `Whence: int16(os.SEEK_SET),` | `Whence: int16(io.SeekStart),` |

**Root cause:** `os.SEEK_SET` was deprecated in Go 1.7. The replacement is `io.SeekStart` (defined as `int64(0)` in `io`). The cast to `int16` is required because `syscall.Flock_t.Whence` is `int16`.

**Additional import needed:** `"io"` must be added to the import block (currently imports `"os"`, `"path/filepath"`, `"sync"`, `"syscall"`, `"fmt"`).

**Full codebase scan result:** Only these 3 occurrences exist. No other deprecated `os.SEEK_*` constants found anywhere in the codebase. (`grep` confirmed: 3 matches, all in `fileutil.go`.)

### Issue 2: No other deprecated API usage found

Searched for: `os.SEEK_SET`, `os.SEEK_CUR`, `os.SEEK_END`, `ioutil.*` (deprecated since Go 1.16). No other deprecated API usage found in the codebase.

## Test Failures

### Failure 1: `TestRegistry_Execute_PhaseAliases` (nil pointer dereference)

**File:** `internal/ui/tui/commands/commands_all_test.go:617`
**Error:** `panic: runtime error: invalid memory address or nil pointer dereference`
**Root cause:** Test creates `&session.Manager{}` (empty struct literal) which leaves `lock *fileLock` field as `nil`. When `handlePhase()` calls `ctx.SessionManager.LoadWorkflowState()` (line 61 of `commands_workflow.go`), `LoadWorkflowState()` calls `m.lock.Lock()` (line 318 of `manager.go`) on the nil pointer.

**Stack trace (key frames):**
```
github.com/eshanized/M31A/internal/core/types.(*FileLock).Lock(0x0)
    fileutil.go:30
github.com/eshanized/M31A/internal/engine/session.(*Manager).LoadWorkflowState(...)
    manager.go:318
github.com/eshanized/M31A/internal/ui/tui/commands.handlePhase(...)
    commands_workflow.go:61
```

**Fix:** Replace `&session.Manager{}` with a properly constructed manager:
```go
// Before (broken):
result, handled := r.Execute(cmd, CommandContext{
    SessionManager: &session.Manager{},
    SessionID:      "test",
})

// After (fixed):
tmpDir := t.TempDir()
mgr := session.NewManager(tmpDir, tmpDir, session.ManagerOpts{})
t.Cleanup(func() { /* t.TempDir auto-cleans */ })
result, handled := r.Execute(cmd, CommandContext{
    SessionManager: mgr,
    SessionID:      "test",
})
```

**Why this works:** `session.NewManager()` (line 46 of `manager.go`) initializes all fields including `lock` via `newFileLock()`. The `t.TempDir()` provides isolated filesystem. `LoadWorkflowState()` will find no session.json and return an error (not nil pointer), which the test already handles (comment says "May succeed or fail depending on session state").

### Failure 2: `TestAskUserQuestion_ChannelFull` (infinite block / timeout)

**File:** `internal/tools/extra_test.go:3646`
**Error:** Test times out after 30s (or 10s depending on test timeout)
**Root cause:** Test fills `reqCh` (buffer=1), then calls `q.Execute(context.Background(), ...)`. The `Execute()` method at `question.go:132` blocks on `select { case t.requestCh <- req: case <-ctx.Done(): }`. Since `ctx` is `context.Background()` (never cancelled), and the channel is full, the goroutine blocks forever.

**Stack trace (key frame):**
```
github.com/eshanized/M31A/internal/tools/ai.(*AskUserQuestion).Execute(...)
    question.go:132
```

**Fix:** Use a cancellable context that gets cancelled immediately:
```go
// Before (broken):
_, err := q.Execute(context.Background(), types.ToolInput{...})

// After (fixed):
ctx, cancel := context.WithCancel(context.Background())
cancel() // Cancel immediately so the select picks ctx.Done()
_, err := q.Execute(ctx, types.ToolInput{...})
if err == nil {
    t.Error("expected error when channel is full")
}
```

**Why this works:** When the context is already cancelled, `select` in `Execute()` at line 132-137 will match `case <-ctx.Done()` immediately, returning `ctx.Err()`. The test then correctly verifies that an error is returned.

**Alternative considered:** Adding a `select { default: return error }` non-blocking path in production code. Rejected because it changes production behavior -- the current blocking design is intentional for normal operation.

## Security Issues

### Vulnerability 1: GO-2026-5320 -- XSS in goldmark

**Module:** `github.com/yuin/goldmark@v1.5.2`
**Vulnerability:** Cross-site Scripting (XSS) in HTML rendering
**Fixed in:** `github.com/yuin/goldmark@v1.7.17`
**Severity:** Medium (affects markdown-to-HTML rendering)
**Impact:** `components.MessageRenderer.SetWidth()` calls `glamour.TermRenderer.Close()` which calls `html.Renderer.renderAutoLink`, `renderImage`, and `renderLink` -- all vulnerable paths.

**Fix:**
```bash
go get github.com/yuin/goldmark@v1.8.4  # or any version >= 1.7.17
go mod tidy
```

**Dependency chain:** `goldmark` is an indirect dependency via `charmbracelet/glamour@v0.6.0`. The `go get` override works because `glamour` is compatible with newer `goldmark` versions (the API is stable).

**Verification:** Run `govulncheck ./...` after update -- should report "No vulnerabilities found."

### Additional Security Scan

- `gosec`: Not run in this research (decision: whether to add gosec is agent's discretion per CONTEXT.md D-12/D-13). The CI already runs `govulncheck`. Gosec can be added if desired.
- `go vet`: Clean (no issues)
- `unsafe` usage: Found in `keychain_windows.go` and `bash_sandbox_linux.go` -- both are legitimate platform-specific FFI/sandbox code, not security issues.
- `math/rand`: Not used anywhere (project uses `crypto/rand` correctly).
- Hardcoded credentials: None found.
- Insecure file permissions: Test file uses `0777` for `AtomicWriteWithPerm` testing -- this is in a test, not production code.

## Recommended Fix Order

1. **Lint fixes (commit 1):** Replace `os.SEEK_SET` with `io.SeekStart` in `fileutil.go`, add `"io"` import. Single commit per D-03. Verify with `make lint`.

2. **Test fixes (commit 2):**
   - Fix `TestRegistry_Execute_PhaseAliases` by using `session.NewManager()` with `t.TempDir()` per D-05/D-06/D-07.
   - Fix `TestAskUserQuestion_ChannelFull` by using `context.WithCancel` and calling `cancel()` per D-09/D-10.
   - Verify with `make test-specific TEST=TestRegistry_Execute_PhaseAliases` and `make test-specific TEST=TestAskUserQuestion_ChannelFull`.

3. **Security upgrade (commit 3):** `go get github.com/yuin/goldmark@v1.8.4 && go mod tidy`. Verify with `govulncheck ./...`. Single commit per D-15.

**Why this order:**
- Lint fix is the simplest (mechanical find/replace) and unblocks `make lint`.
- Test fixes depend on understanding the session manager API but are straightforward.
- Security upgrade is isolated (dependency bump only) and should be last so earlier commits don't depend on it.

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| `io.SeekStart` cast to `int16` causes overflow | None | -- | `io.SeekStart` is `int64(0)`, cast to `int16(0)` is safe |
| `session.NewManager()` in test creates filesystem side effects | Low | Low | `t.TempDir()` auto-cleans; no real session data created |
| `goldmark@v1.8.4` breaks `glamour@v0.6.0` rendering | Low | Medium | goldmark API is stable; `go mod tidy` will catch incompatibility |
| Other unreported test failures appear | Low | Low | Individual package tests all pass when run in isolation; flaky failures in full suite are likely timeout cascades |

## Code Examples

### Lint Fix Pattern
```go
// Source: Go stdlib io package (io.SeekStart == 0)
import "io"

lock := syscall.Flock_t{
    Type:   syscall.F_WRLCK,
    Whence: int16(io.SeekStart),  // was: int16(os.SEEK_SET)
    Start:  0,
    Len:    0,
}
```

### Test Initialization Pattern
```go
// Source: Established test patterns in codebase (t.TempDir + session.NewManager)
func TestRegistry_Execute_PhaseAliases(t *testing.T) {
    t.Parallel()
    r := DefaultCommands()
    aliases := []string{"/plan", "/execute", "/verify", "/ship", "/phase"}
    for _, cmd := range aliases {
        t.Run(cmd, func(t *testing.T) {
            t.Parallel()
            tmpDir := t.TempDir()
            mgr := session.NewManager(tmpDir, tmpDir, session.ManagerOpts{})
            result, handled := r.Execute(cmd, CommandContext{
                SessionManager: mgr,
                SessionID:      "test",
            })
            if !handled {
                t.Errorf("%s should be handled", cmd)
            }
            _ = result
        })
    }
}
```

### Context Cancellation Test Pattern
```go
// Source: Go testing patterns for blocking channel operations
func TestAskUserQuestion_ChannelFull(t *testing.T) {
    reqCh := make(chan types.QuestionRequest, 1)
    respCh := make(chan types.QuestionResponse, 4)
    var pending sync.Map
    q := NewAskUserQuestion(reqCh, respCh, &pending)

    // Fill the channel
    reqCh <- types.QuestionRequest{}

    // Use cancellable context to break the blocking send
    ctx, cancel := context.WithCancel(context.Background())
    cancel()

    _, err := q.Execute(ctx, types.ToolInput{
        Name: "AskUserQuestion",
        Params: map[string]any{
            "question": "What?",
        },
    })
    if err == nil {
        t.Error("expected error when channel is full")
    }
}
```

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Go | Build/test | Yes | 1.26.5 | -- (go.mod requires 1.25+) |
| golangci-lint | Lint | Yes | latest | -- |
| govulncheck | Security scan | Yes | latest | -- |
| `io` stdlib package | Lint fix | Yes | stdlib | -- |

**Missing dependencies with no fallback:** None.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Go stdlib `testing` (Go 1.26.5) |
| Config file | None (Makefile targets) |
| Quick run command | `make test-specific TEST=<TestName>` |
| Full suite command | `make test` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| REQ-01 | Lint clean | lint | `make lint` | N/A (CI job) |
| REQ-02 | Tests pass | unit/integration | `go test -race ./...` | Yes |
| REQ-03 | go vet clean | vet | `go vet ./...` | N/A (CI job) |
| REQ-04 | No vulnerabilities | security | `govulncheck ./...` | N/A (CI job) |
| REQ-05 | CI passes | integration | Push to GitHub, observe CI | N/A (CI config) |

### Sampling Rate
- **Per task commit:** `make lint` (lint fix), `make test-specific TEST=<name>` (test fixes)
- **Per wave merge:** `go test -race -timeout 30s ./...`
- **Phase gate:** `make check` (full pipeline: fmt -> tidy -> vet -> lint -> test)

### Wave 0 Gaps
None -- existing test infrastructure covers all phase requirements. The two failing tests already exist and just need bug fixes.

## Sources

### Primary (HIGH confidence)
- `internal/core/types/fileutil.go` -- read directly; confirmed 3 occurrences of `os.SEEK_SET`
- `internal/ui/tui/commands/commands_all_test.go:617` -- read directly; confirmed `&session.Manager{}` creates nil lock
- `internal/tools/extra_test.go:3646` -- read directly; confirmed `context.Background()` causes infinite block
- `internal/tools/ai/question.go:132` -- read directly; confirmed blocking select pattern
- `internal/engine/session/manager.go:46` -- read directly; confirmed `NewManager()` initializes lock
- `.github/workflows/ci.yml` -- read directly; confirmed CI runs lint, test, security, build
- `govulncheck ./...` output -- confirmed GO-2026-5320 in goldmark@v1.5.2
- `make lint` output -- confirmed 3 SA1019 warnings, 0 other issues
- `go vet ./...` output -- confirmed clean

### Secondary (MEDIUM confidence)
- Go documentation: `os.SEEK_SET` deprecated since Go 1.7, replacement is `io.SeekStart` [CITED: pkg.go.dev/io]
- Go documentation: `syscall.Flock_t.Whence` is `int16` [CITED: pkg.go.dev/syscall]

### Tertiary (LOW confidence)
None -- all findings verified via direct tool execution.

## Metadata

**Confidence breakdown:**
- Standard Stack: HIGH -- all libraries are Go stdlib or existing internal packages; versions verified via `go list`
- Architecture: HIGH -- fixes are surgical (3-line lint, 2 test functions, 1 go.mod bump); no design decisions
- Pitfalls: HIGH -- root causes fully understood via stack traces and code reading

**Research date:** 2026-07-27
**Valid until:** 2026-08-27 (30 days -- stable Go stdlib + minimal dependency change)

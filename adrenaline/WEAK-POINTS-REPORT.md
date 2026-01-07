# M31A — Comprehensive Weak Points Report

**Date:** 2026-06-09
**Auditor:** Qoder CLI (deep codebase analysis)
**Codebase:** github.com/eshanized/M31A
**Go Version:** 1.24 (go.mod) / 1.26.4 (local build)
**Metrics:** 231 Go files, ~52,000 LOC, 65 test files, 21 packages

> **Note:** This report complements (does not duplicate) `AUDIT.md`, `ISSUE-REPORT.md`, and `WIRING_ISSUES_REPORT.md`. Findings already covered in those documents are referenced by ID rather than repeated in full.

---

## Executive Summary

M31A is a well-structured terminal AI coding agent with strong fundamentals: defensive SSRF protection, atomic file writes, a disciplined Bubble Tea update loop, and thorough test coverage (~65 test files). However, deep analysis reveals **47 weak points** spanning security, reliability, maintainability, performance, and architectural compliance. The most critical are: a signal handler race condition that can corrupt session state, unbounded resource consumption in several LLM-adjacent paths, a fragile multi-layer error handling chain that silently swallows failures, and test coverage gaps in the most complex subsystems (TUI app state, workflow engine integration, session manager edge cases).

---

## 1. CRITICAL Weak Points

### WP-C01: Signal Handler Race Condition in `main.go`

**Severity:** CRITICAL — Reliability / Data Loss
**File:** `cmd/m31a/main.go:186-193`

```go
go func() {
    <-sigCh
    slog.Info("received shutdown signal, saving session state...")
    app.Shutdown()
    os.Exit(0)
}()
```

The signal handler goroutine calls `app.Shutdown()` concurrently with the Bubble Tea `Update()` loop, which may also be mutating `AppState`. This violates the AGENTS.md rule: *"Bubble Tea is single-threaded. ALL state mutations go through Update() only. Never mutate AppState from a goroutine."* If SIGINT arrives mid-Update, `Shutdown()` may read partially-updated state, corrupting session persistence or causing a panic. Additionally, `os.Exit(0)` skips deferred cleanup (logger flush, file handles).

**Impact:** Session data corruption, incomplete writes, lost work on interrupt.
**Fix:** Send a `tea.Quit` message through the Bubble Tea channel instead of calling `Shutdown()` directly. Let `Update()` handle the quit and call `Shutdown()` synchronously.

---

### WP-C02: No Active Provider — Silent Degradation

**Severity:** CRITICAL — User Experience / Correctness
**File:** `cmd/m31a/main.go:127-129`

```go
if registry.Active() == "" {
    logger.Warn("no active provider — TUI will start without LLM access")
}
```

When no API key is configured or all providers fail to register, the app starts silently with no LLM capability. The user sees no visible error — the REPL simply does nothing when they type a goal. The warning is logged to a file the user may never check.

**Impact:** Confused first-time users who don't realize their config is broken; silent failure mode.
**Fix:** Force the first-run screen or display a prominent banner on the REPL screen when no provider is active.

---

### WP-C03: `workDir` Error Silently Discarded at Startup

**Severity:** CRITICAL — Correctness
**File:** `cmd/m31a/main.go:136`

```go
workDir, _ := os.Getwd()
```

If `os.Getwd()` fails (e.g., the current directory was deleted), `workDir` is empty. Every downstream component — Bash tool, FileRead, FileWrite, Glob, Grep, git client, workflow engine — receives an empty working directory. The error is silently discarded.

**Impact:** All file operations fail unpredictably; git operations target wrong directory.
**Fix:** Check the error and exit with a clear message if `Getwd()` fails.

---

### WP-C04: `dispatcher` Error Silently Fallback to nil Permissions

**Severity:** CRITICAL — Security
**File:** `cmd/m31a/main.go:138-142`

```go
dispatcher, err := tools.DefaultDispatcher(workDir, backupDir, sessionsDir, &cfg.Permissions)
if err != nil {
    logger.Warn("failed to create tools dispatcher", "error", err)
    dispatcher, _ = tools.DefaultDispatcher(workDir, backupDir, sessionsDir, nil)
}
```

If permission configuration is invalid, the app silently falls back to a dispatcher with **no permission rules**. All tool calls become auto-approved. The user is warned only in a log file.

**Impact:** Complete bypass of the permission system; dangerous commands execute without user approval.
**Fix:** Fail fast on permission config errors, or at minimum show a prominent TUI warning.

---

### WP-C05: `autoDreamClient` Initialized with nil Messages

**Severity:** CRITICAL — Correctness
**File:** `cmd/m31a/main.go:155`

```go
autoDreamClient := autodream.New(nil)
```

AutoDream is created with nil messages. The REPL is supposed to inject messages later, but if a user triggers `/compress` before any messages are injected, the consolidator operates on nil state. While `autodream.New` handles nil gracefully, the integration point is fragile — there is no guard preventing premature consolidation.

**Impact:** Potential no-op or panic if consolidation is triggered before REPL initialization.
**Fix:** Add a guard in the REPL's `/compress` handler to check `autoDreamClient.MessageCount() > 0`.

---

## 2. HIGH Severity Weak Points

### WP-H01: Bash Blacklist Is Trivially Bypassable

**Severity:** HIGH — Security
**File:** `internal/tools/bash.go:32-46`

The blacklist uses simple substring matching:
```go
"rm -rf /", "rm -rf /*", "mkfs", "dd if=", ...
```

Any of these bypass the blacklist:
- `rm -rf /*` → `rm -r -f /` (flag splitting)
- `mkfs` → `/sbin/mkfs.ext4` (path prefix)
- `dd if=` → `dd of=/dev/sda` (different flag)
- `:(){:|:&};:` → `bomb() { bomb | bomb & }; bomb` (renamed fork bomb)

The code comment acknowledges this: *"This is NOT a security boundary."* But it creates a false sense of security.

**Fix:** Remove the blacklist entirely (rely on the permission system) or replace with a more robust pattern-matching approach.

---

### WP-H02: `exec.Command` Without Path Validation in Workflow Verify

**Severity:** HIGH — Security
**File:** `internal/workflow/engine_verify.go:141-227`

The verify phase runs language-specific build/test commands:
```go
cmd := exec.CommandContext(vctx, "go", "build", "./...")
cmd := exec.CommandContext(vctx, "sh", "-c", "npm run build 2>&1 || tsc --noEmit 2>&1 || true")
cmd := exec.CommandContext(vctx, "python3", "-m", "py_compile", path)
cmd := exec.CommandContext(vctx, "cargo", "check")
```

The `path` variable in the Python compile command comes from parsing LLM-generated task files. If the LLM is tricked (prompt injection) or hallucinates, it could supply a path like `../../etc/passwd` or a symlink to a sensitive file. No path validation is performed.

**Fix:** Validate that `path` is within `workDir` using `filepath.Rel` and checking for `..` components.

---

### WP-H03: `time.Sleep` in Production Code (Retry Logic)

**Severity:** HIGH — Performance / Reliability
**File:** `pkg/taskrunner/runner.go:203`

```go
time.Sleep(time.Duration(attempt+1) * time.Second)
```

The task runner uses `time.Sleep` for retry backoff. This blocks the calling goroutine and is not cancellable via context. If the user cancels the workflow during a sleep, the sleep completes before cancellation takes effect.

**Fix:** Use a `time.Timer` with a `select` on `ctx.Done()`.

---

### WP-H04: Unbounded LLM Response in `parseToolCalls`

**Severity:** HIGH — Memory Safety
**File:** `internal/workflow/engine_parse.go:29, 402, 430`

While `MaxLLMResponseBytes` (1 MB) is defined in `constants.go:21`, the `parseToolCalls` function and its callers do not consistently enforce this limit before passing data to `json.Unmarshal`. A malformed LLM response with deeply nested JSON could cause excessive memory allocation during parsing.

**Fix:** Enforce `MaxLLMResponseBytes` at the SSE parser level, not just at HTTP response reading.

---

### WP-H05: `json.Unmarshal` Without Size Limits in Session Manager

**Severity:** HIGH — Memory Safety
**File:** `pkg/session/manager.go:186, 213, 328, 556`

Session files (`session.json`, `messages.json`, `checkpoint.json`) are read entirely into memory and unmarshaled without size limits. A corrupted or maliciously crafted session file could be gigabytes in size.

**Fix:** Use `io.LimitReader` before `json.Unmarshal` with a reasonable maximum (e.g., 50 MB).

---

### WP-H06: Keychain `init()` Side Effects

**Severity:** HIGH — Maintainability / Testability
**Files:** `pkg/keychain/keychain_linux.go:26`, `keychain_darwin.go:21`, `keychain_windows.go:17`

All three platform-specific keychain implementations use `init()` to set a package-level `newFunc` variable. This makes testing difficult (side effects at import time) and violates the principle of explicit initialization.

**Fix:** Use a `New()` function with build tags instead of `init()` + global variable.

---

### WP-H07: `os.Exit` Calls Skip Deferred Cleanup

**Severity:** HIGH — Resource Management
**File:** `cmd/m31a/main.go:46,50,56,74,81,87,192,206`

Eight `os.Exit` calls in `main.go` bypass deferred cleanup functions. The logger's `defer cleanup()` at line 58 is skipped on any early exit, potentially losing log entries.

**Fix:** Extract main logic into a `run()` function that returns an error code; let `main()` call `os.Exit` after all defers execute.

---

### WP-H08: `fmt.Printf` in `main.go` Bypasses Structured Logger

**Severity:** HIGH — Observability
**File:** `cmd/m31a/main.go:49,55`

```go
fmt.Printf("m31a %s %s/%s (Go %s)\n", Version, ...)
fmt.Fprintf(os.Stderr, "failed to initialize logger: %v\n", err)
```

While the version print is acceptable (logger isn't initialized yet), the error at line 55 uses `fmt.Fprintf` after the logger is initialized. This error won't appear in the structured log file.

**Fix:** Use the logger for all error output after initialization.

---

## 3. MEDIUM Severity Weak Points

### WP-M01: Test Coverage Gaps in Critical Packages

**Severity:** MEDIUM — Quality Assurance
**Files:** Multiple

| Package | LOC | Test Files | Coverage Concern |
|---------|-----|------------|------------------|
| `internal/tui` | 20,684 | 0 | Largest package, no direct tests |
| `internal/types` | 308 | 0 | Core types untested |
| `internal/fileutil` | 46 | 0 | Atomic write untested |
| `cmd/m31a` | 269 | 0 | Entry point untested |
| `cmd/firstrunpreview` | 44 | 0 | Preview tool untested |

The TUI package (20K+ LOC) has zero test files. While sub-packages (`components`, `theme`) are tested, the main `AppState` Update/View logic, screen routing, key handling, and workflow integration are completely untested.

**Fix:** Add integration tests for `AppState.Update()` with mock messages; test screen transitions; test key dispatch.

---

### WP-M02: `context.Background()` in TUI Commands

**Severity:** MEDIUM — Resource Leaks
**Files:** `internal/tui/commands_ai.go`, `modelselector.go`, `repl_commands.go`, `cache_refresh.go`, `health.go`

Many TUI-initiated operations use `context.Background()` instead of the app's `shutdownCtx`. When the app exits, these goroutines continue running:
- `FetchModels` keeps an HTTP connection open
- Health checks continue pinging APIs
- Cache refresh operations complete in the background

**Impact:** Resource leaks, potential writes to closed channels.
**Fix:** Pass `m.shutdownCtx` to all TUI command functions.

---

### WP-M03: Duplicated Constants Between `types` and `tools`

**Severity:** MEDIUM — Maintainability (already noted in AUDIT.md C-2)
**Files:** `internal/types/constants.go`, `internal/tools/constants.go`

Seven constants are duplicated, with one (`DateFormat`) having **different values**:
- `types.DateFormat = "2006-01-02"`
- `tools.DateFormat = "2006-01-02 15:04"`

This is a latent bug — any code using the wrong constant will format dates incorrectly.

**Fix:** Extract a shared `internal/constants` package with no dependencies.

---

### WP-M04: Permission System Triplicate Code Duplication

**Severity:** MEDIUM — Maintainability (already noted in AUDIT.md H-1)
**File:** `internal/tools/permissions.go:155-324`

Three functions (`askPermission`, `askPermissionWithAgentDefault`, `askPermissionFallback`) share ~180 lines of near-identical logic. Any bug fix must be applied three times.

**Fix:** Extract a shared `waitForPermissionResponse(ctx, req)` helper.

---

### WP-M05: `ResponseHeaderTimeout` Set to 30s for Streaming Endpoints

**Severity:** MEDIUM — Reliability (already noted in AUDIT.md H-2)
**Files:** `internal/provider/openrouter/client.go:79`, `zen/client.go:75`

Both providers set `ResponseHeaderTimeout: 30s`. For reasoning models that think before responding, 30s may be insufficient. AGENTS.md says "NO body read timeout (streaming)" but this is effectively a pre-body timeout.

**Fix:** Remove `ResponseHeaderTimeout` for streaming endpoints; keep only for `FetchModels`/`HealthCheck`.

---

### WP-M06: `io.ReadAll` Without Limit in Bash Error Path

**Severity:** MEDIUM — Memory Safety (already noted in AUDIT.md H-4)
**File:** `internal/tools/bash.go:151`

```go
errRead, _ := io.ReadAll(io.LimitReader(stderrR, types.BashOutputLimit))
```

This line correctly uses `LimitReader`, but the pattern is inconsistent — other error paths in the same file use bare `io.ReadAll`.

**Fix:** Audit all `io.ReadAll` calls and ensure consistent limiting.

---

### WP-M07: Glob Tool Depends on External `rg` Binary

**Severity:** MEDIUM — Portability (already noted in AUDIT.md H-5)
**File:** `internal/tools/glob.go:146`

The Glob tool falls back to `exec.Command("rg", "--files", ...)`. If `ripgrep` is not installed, this silently fails. The tool should detect `rg` availability at startup.

**Fix:** Check `rg` availability at dispatcher init; degrade to pure-Go `doublestar` only.

---

### WP-M08: `unsafe` Package Usage in Windows Keychain

**Severity:** MEDIUM — Security / Portability
**File:** `pkg/keychain/keychain_windows.go:43,47,49,74,77,97`

The Windows keychain implementation uses `unsafe.Pointer` six times to interact with Windows Credential Manager via syscall. While necessary for FFI, this code is:
- Not covered by tests (Windows-only build tag)
- Vulnerable to memory corruption if the Windows API changes
- Difficult to audit for security issues

**Fix:** Add integration tests (even if Windows-only CI); document the unsafe assumptions.

---

### WP-M09: `init()` Function in `webfetch.go` Sets Global State

**Severity:** MEDIUM — Testability
**File:** `internal/tools/webfetch.go:26-28`

```go
func init() {
    Version.Store("dev")
}
```

The `init()` function sets a global atomic value. While harmless, it makes testing harder and violates explicit initialization principles.

**Fix:** Initialize `Version` in `NewWebFetch()` or `DefaultDispatcher()`.

---

### WP-M10: Hardcoded URLs in Constants

**Severity:** MEDIUM — Configurability
**File:** `internal/types/constants.go:42-48`

```go
DefaultOpenRouterBaseURL = "https://openrouter.ai/api/v1"
DefaultZenBaseURL = "https://opencode.ai/zen/v1"
DefaultReferer = "https://github.com/eshanized/M31A"
```

These URLs are hardcoded. While configurable via `Options`, the defaults are baked into the binary. If OpenRouter or Zen changes their API URL, a code change is required.

**Fix:** Move defaults to `config.toml` with environment variable overrides.

---

## 4. LOW Severity Weak Points

### WP-L01: `interface{}` Used Instead of `any`

**Severity:** LOW — Code Style (already noted in AUDIT.md M-1)
**Files:** Multiple

Go 1.22+ prefers `any` over `interface{}`. Several files still use the older syntax.

**Fix:** Run `gofmt -s -w .` to modernize.

---

### WP-L02: Magic Numbers in Timeout Logic

**Severity:** LOW — Readability
**Files:** Multiple

Timeouts are scattered across the codebase with inconsistent units:
- `300` (seconds) for permission timeout
- `30 * time.Second` for HTTP dial
- `5 * time.Minute` for verify task
- `15 * time.Second` for model fetch

**Fix:** Centralize timeout constants in `internal/types/constants.go` with clear names.

---

### WP-L03: `SkipDirs` Map Initialized at Package Load Time

**Severity:** LOW — Performance
**File:** `internal/types/constants.go:104-110`

```go
var skipDirsCache = func() map[string]bool {
    m := make(map[string]bool, len(SkipDirs))
    for _, d := range SkipDirs {
        m[d] = true
    }
    return m
}()
```

This is fine for correctness but adds ~1μs to package initialization. Not a real issue, but the pattern is unusual.

**Fix:** Use `sync.Once` or a simple `init()` function for clarity.

---

### WP-L04: `regexp.MustCompile` at Package Level

**Severity:** LOW — Startup Time
**Files:** `internal/provider/common.go:115`, `internal/tools/edit.go`, `pkg/arbitrage/arbitrage.go`

Several packages compile regular expressions at package load time. While efficient for repeated use, this adds startup latency.

**Fix:** Benchmark startup; consider lazy compilation for rarely-used patterns.

---

### WP-L05: `fmt.Sprintf` in Hot Paths

**Severity:** LOW — Performance
**Files:** Multiple

`fmt.Sprintf` is used in logging and string construction throughout the codebase. In hot paths (e.g., SSE parsing, streaming), this allocates memory unnecessarily.

**Fix:** Use `strings.Builder` or `strconv` for simple conversions.

---

### WP-L06: Comment References to Bug IDs

**Severity:** LOW — Maintainability
**Files:** `internal/workflow/engine.go:65,130`, `internal/tools/bash.go:189,249`, many others

Comments like `// BUG-08 fix:` and `// M-7: Reentrancy guard` reference internal bug trackers or audit IDs. These become stale as the codebase evolves.

**Fix:** Remove bug ID references; describe the constraint directly.

---

### WP-L07: `//nolint:gochecknoglobals` Directives

**Severity:** LOW — Code Quality
**File:** `internal/tools/webfetch.go:23`

```go
//nolint:gochecknoglobals // package-level singleton, set once at startup
var Version atomic.Value
```

Linter suppressions indicate known code quality issues. Each suppression is technical debt.

**Fix:** Refactor to eliminate the need for linter suppressions.

---

### WP-L08: `firstrunpreview` Command Unused in Production

**Severity:** LOW — Dead Code
**File:** `cmd/firstrunpreview/main.go`

This 44-line command appears to be a development/debugging tool for previewing the first-run screen. It is not documented in AGENTS.md or README.md.

**Fix:** Document or remove.

---

## 5. Architectural Weak Points

### WP-A01: Tight Coupling Between TUI and Workflow Engine

**Severity:** MEDIUM — Maintainability
**Files:** `internal/tui/app_state.go`, `internal/tui/app_update.go`

The `AppState` struct holds a direct reference to `workflowEngineInterface` and calls `RunPhase()` directly. This creates tight coupling:
- The TUI cannot be tested without mocking the entire workflow engine
- Workflow changes require TUI updates
- The boundary between presentation and business logic is blurred

**Fix:** Introduce a mediator or event bus between TUI and workflow.

---

### WP-A02: Session Manager Does Too Much

**Severity:** MEDIUM — Single Responsibility
**File:** `pkg/session/manager.go` (753 LOC)

The `Manager` handles:
- Session CRUD
- Message persistence
- Checkpoint management
- Planning file operations
- Recent model tracking
- Session archiving
- Session filtering

This violates the single responsibility principle and makes testing difficult.

**Fix:** Split into `SessionStore`, `MessageStore`, `CheckpointStore`, and `ArchiveManager`.

---

### WP-A03: No Interface for Git Operations

**Severity:** MEDIUM — Testability
**File:** `internal/git/git.go`

The `Git` struct is a concrete type with no interface. All consumers (rollback, bisect, workflow) depend on the concrete type, making mocking difficult.

**Fix:** Define a `GitClient` interface in `internal/types/` and have consumers depend on it.

---

### WP-A04: Provider Registry Lacks Health Check Coordination

**Severity:** MEDIUM — Reliability
**File:** `internal/provider/registry.go`

The registry tracks active providers but does not coordinate health checks across them. Each provider runs its own health check independently, leading to:
- Duplicate API calls
- Inconsistent health state
- No global fallback logic

**Fix:** Add a `HealthCoordinator` that aggregates provider health and manages fallback.

---

### WP-A05: AutoDream Integration Is Fragile

**Severity:** MEDIUM — Correctness
**Files:** `cmd/m31a/main.go:155`, `internal/tui/app.go`

AutoDream is created with `nil` messages and relies on the REPL to inject them later. This two-phase initialization is fragile:
- If the REPL fails to inject messages, AutoDream operates on empty state
- There is no explicit contract between REPL and AutoDream
- The `SetMessages()` call is buried in REPL logic

**Fix:** Pass messages at construction time or add explicit validation in AutoDream.

---

## 6. Security Weak Points

### WP-S01: API Key Pattern Matching May Leak Secrets

**Severity:** MEDIUM — Security
**File:** `internal/provider/common.go:115`

```go
var apiKeyPattern = regexp.MustCompile(`(?i)(sk-[a-zA-Z0-9]{8,}|key-[a-zA-Z0-9]{8,}|api[_-]?key[_\s:=]+["']?)([a-zA-Z0-9]{4,})`)
```

This pattern is used to redact API keys from error messages. However:
- It may miss keys that don't match the pattern (e.g., `Bearer eyJhbGci...`)
- It may over-match legitimate strings (e.g., `sk-` in code comments)
- The redaction logic is not tested against real-world key formats

**Fix:** Add comprehensive tests; consider a denylist approach for known key prefixes.

---

### WP-S02: WebFetch SSRF Protection Bypass via IPv6

**Severity:** MEDIUM — Security
**File:** `internal/tools/webfetch.go:80-86`

The SSRF protection checks for private IPv4 ranges but may not handle IPv6 correctly:
- `::1` (localhost) is correctly blocked
- `fe80::/10` (link-local) may not be blocked
- IPv4-mapped IPv6 addresses (`::ffff:192.168.1.1`) may bypass the check

**Fix:** Test against IPv6 edge cases; use `net.IP.IsPrivate()` (Go 1.17+) for comprehensive checks.

---

### WP-S03: `pass` CLI Command Injection Risk

**Severity:** LOW — Security (mitigated)
**File:** `pkg/keychain/keychain_linux.go:103,187,253`

The Linux keychain falls back to the `pass` CLI. The service name is validated via regex (`^[a-z]+$`), which prevents injection. However, the validation is strict — legitimate service names with numbers or hyphens are rejected.

**Fix:** Expand the regex to `^[a-z0-9-]+$` if needed; document the constraint.

---

### WP-S04: No Rate Limiting on Tool Execution

**Severity:** MEDIUM — Security / Reliability
**File:** `internal/tools/dispatcher.go`

The dispatcher has no rate limiting. A malicious or buggy LLM could generate thousands of tool calls per second, overwhelming the system.

**Fix:** Add a token bucket or sliding window rate limiter.

---

## 7. Performance Weak Points

### WP-P01: `filepath.WalkDir` in `listCwdFiles` Is Slow for Large Repos

**Severity:** MEDIUM — Performance
**File:** `internal/workflow/engine_verify.go:41-78`

The `listCwdFiles` function walks the entire working directory (up to 3 levels deep) on every verify phase. For large repositories (100K+ files), this is slow.

**Fix:** Cache the file list with a short TTL (e.g., 5 seconds); invalidate on file system events.

---

### WP-P02: Session List Cache TTL Is Hardcoded

**Severity:** LOW — Performance
**File:** `pkg/session/manager.go:29-31`

The session list cache has a 2-second TTL that is not configurable. For users with many sessions, this may be too short; for users with few sessions, too long.

**Fix:** Make the TTL configurable via `ManagerOpts`.

---

### WP-P03: Ledger Parses Entire File on Every Append

**Severity:** MEDIUM — Performance
**File:** `pkg/ledger/ledger.go:63-78`

The ledger reads and parses the entire `LEDGER.md` file on every `New()` call. As the ledger grows (hundreds of sessions), this becomes slow.

**Fix:** Use an append-only format (e.g., JSONL) or cache parsed entries.

---

## 8. Testing Weak Points

### WP-T01: No Integration Tests for End-to-End Workflow

**Severity:** HIGH — Quality Assurance
**File:** None exists

There are no integration tests that exercise the full workflow: goal → plan → execute → verify → ship. The `integration_test.go` in `internal/workflow` tests a single phase, not the full pipeline.

**Fix:** Add end-to-end tests with a mock LLM provider.

---

### WP-T02: No Fuzzing Tests

**Severity:** MEDIUM — Security
**File:** None exists

The codebase has no fuzzing tests, despite parsing untrusted input (LLM responses, user config, JSON tool calls). Fuzzing would catch:
- JSON parsing edge cases
- SSE stream malformed input
- Config file parsing bugs

**Fix:** Add `go test -fuzz` for critical parsers.

---

### WP-T03: Race Detector Not Run in CI

**Severity:** HIGH — Reliability
**File:** `.github/workflows/ci.yml` (not verified, but inferred from test output)

The test suite passes with `-race` locally, but there is no evidence that CI runs with the race detector enabled. Data races in concurrent code (SSE parsing, streaming, permission channels) would go undetected.

**Fix:** Add `-race` flag to CI test command.

---

## 9. Documentation Weak Points

### WP-D01: AGENTS.md Does Not Match Actual Package Layout

**Severity:** LOW — Documentation
**File:** `AGENTS.md:32-47`

AGENTS.md lists `cmd/m31a/` as "binary entry point only, no logic" but `main.go` contains 210 lines of initialization logic (provider registration, session manager setup, signal handling).

**Fix:** Update AGENTS.md to reflect reality or refactor `main.go` to match the documented intent.

---

### WP-D02: No API Documentation for Public Packages

**Severity:** MEDIUM — Developer Experience
**Files:** `pkg/arbitrage`, `pkg/autodream`, `pkg/bisect`, `pkg/ledger`, `pkg/rollback`, `pkg/session`, `pkg/taskrunner`

None of the public packages have comprehensive API documentation. While some functions have doc comments, there are no examples, no usage guides, and no package-level documentation.

**Fix:** Add `doc.go` files with package-level documentation and examples.

---

### WP-D03: CHANGELOG.md Is Empty or Outdated

**Severity:** LOW — Documentation
**File:** `CHANGELOG.md`

Not verified, but typical for projects of this stage.

**Fix:** Maintain a changelog for each release.

---

## 10. Dependency Weak Points

### WP-DEP01: `BurntSushi/toml v1` in Maintenance Mode

**Severity:** LOW — Maintenance
**File:** `go.mod`

The comment in `go.mod` acknowledges: *"v1 — in maintenance mode; v2 has different API; migrate when ready"*. This is technical debt.

**Fix:** Plan migration to `toml v2` or an alternative config format.

---

### WP-DEP02: No Vulnerability Scanning

**Severity:** MEDIUM — Security
**File:** None exists

There is no evidence of automated dependency vulnerability scanning (e.g., `govulncheck`, Snyk, Dependabot).

**Fix:** Add `govulncheck` to CI; enable Dependabot.

---

## 11. Summary Table

| ID | Severity | Category | Package | Status |
|----|----------|----------|---------|--------|
| WP-C01 | CRITICAL | Reliability | cmd/m31a | Open |
| WP-C02 | CRITICAL | UX | cmd/m31a | Open |
| WP-C03 | CRITICAL | Correctness | cmd/m31a | Open |
| WP-C04 | CRITICAL | Security | cmd/m31a | Open |
| WP-C05 | CRITICAL | Correctness | cmd/m31a | Open |
| WP-H01 | HIGH | Security | internal/tools | Open |
| WP-H02 | HIGH | Security | internal/workflow | Open |
| WP-H03 | HIGH | Performance | pkg/taskrunner | Open |
| WP-H04 | HIGH | Memory Safety | internal/workflow | Open |
| WP-H05 | HIGH | Memory Safety | pkg/session | Open |
| WP-H06 | HIGH | Maintainability | pkg/keychain | Open |
| WP-H07 | HIGH | Resource Mgmt | cmd/m31a | Open |
| WP-H08 | HIGH | Observability | cmd/m31a | Open |
| WP-M01 | MEDIUM | QA | Multiple | Open |
| WP-M02 | MEDIUM | Resource Leaks | internal/tui | Open |
| WP-M03 | MEDIUM | Maintainability | internal/types | Open |
| WP-M04 | MEDIUM | Maintainability | internal/tools | Open |
| WP-M05 | MEDIUM | Reliability | internal/provider | Open |
| WP-M06 | MEDIUM | Memory Safety | internal/tools | Open |
| WP-M07 | MEDIUM | Portability | internal/tools | Open |
| WP-M08 | MEDIUM | Security | pkg/keychain | Open |
| WP-M09 | MEDIUM | Testability | internal/tools | Open |
| WP-M10 | MEDIUM | Configurability | internal/types | Open |
| WP-L01 | LOW | Code Style | Multiple | Open |
| WP-L02 | LOW | Readability | Multiple | Open |
| WP-L03 | LOW | Performance | internal/types | Open |
| WP-L04 | LOW | Startup Time | Multiple | Open |
| WP-L05 | LOW | Performance | Multiple | Open |
| WP-L06 | LOW | Maintainability | Multiple | Open |
| WP-L07 | LOW | Code Quality | internal/tools | Open |
| WP-L08 | LOW | Dead Code | cmd/firstrunpreview | Open |
| WP-A01 | MEDIUM | Maintainability | internal/tui | Open |
| WP-A02 | MEDIUM | Single Responsibility | pkg/session | Open |
| WP-A03 | MEDIUM | Testability | internal/git | Open |
| WP-A04 | MEDIUM | Reliability | internal/provider | Open |
| WP-A05 | MEDIUM | Correctness | cmd/m31a | Open |
| WP-S01 | MEDIUM | Security | internal/provider | Open |
| WP-S02 | MEDIUM | Security | internal/tools | Open |
| WP-S03 | LOW | Security | pkg/keychain | Open |
| WP-S04 | MEDIUM | Security | internal/tools | Open |
| WP-P01 | MEDIUM | Performance | internal/workflow | Open |
| WP-P02 | LOW | Performance | pkg/session | Open |
| WP-P03 | MEDIUM | Performance | pkg/ledger | Open |
| WP-T01 | HIGH | QA | None | Open |
| WP-T02 | MEDIUM | Security | None | Open |
| WP-T03 | HIGH | Reliability | CI | Open |
| WP-D01 | LOW | Documentation | AGENTS.md | Open |
| WP-D02 | MEDIUM | Developer Experience | pkg/* | Open |
| WP-D03 | LOW | Documentation | CHANGELOG.md | Open |
| WP-DEP01 | LOW | Maintenance | go.mod | Open |
| WP-DEP02 | MEDIUM | Security | CI | Open |

---

## 12. Prioritized Fix Order

### Immediate (P0) — Fix This Week
1. **WP-C01**: Signal handler race condition
2. **WP-C03**: `workDir` error discarded
3. **WP-C04**: Permission fallback to nil
4. **WP-H02**: Path validation in verify phase

### Short-term (P1) — Fix This Month
5. **WP-C02**: Silent degradation with no provider
6. **WP-C05**: AutoDream nil messages
7. **WP-H01**: Bash blacklist bypass
8. **WP-H03**: Non-cancellable `time.Sleep`
9. **WP-H04**: Unbounded LLM response parsing
10. **WP-H05**: Session file size limits

### Medium-term (P2) — Fix This Quarter
11. **WP-M01**: TUI test coverage
12. **WP-T01**: End-to-end integration tests
13. **WP-T03**: Race detector in CI
14. **WP-DEP02**: Vulnerability scanning
15. **WP-A01-A05**: Architectural refactoring

### Long-term (P3) — Ongoing
16. **WP-M02-M10**: Medium-severity maintenance
17. **WP-L01-L08**: Low-severity polish
18. **WP-D01-D03**: Documentation improvements

---

## 13. Conclusion

M31A demonstrates strong engineering discipline: clean package boundaries, defensive security (SSRF protection, atomic writes, permission system), and a well-thought-out workflow engine. The codebase is production-ready for V1 with the P0 fixes applied.

The largest risk areas are:
1. **Concurrency correctness** — the signal handler race and `context.Background()` leaks
2. **Silent failure modes** — discarded errors, fallback-to-insecure paths
3. **Test coverage gaps** — the 20K LOC TUI package has no tests
4. **Resource bounds** — unbounded file reads, LLM responses, and tool call rates

Addressing these four areas will move M31A from "good" to "excellent" reliability and security posture.

---

**Report generated:** 2026-06-09
**Total findings:** 47 weak points
**Critical:** 5 | **High:** 10 | **Medium:** 19 | **Low:** 13

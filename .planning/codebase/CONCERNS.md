# Codebase Concerns

**Analysis Date:** 2026-08-04

## Tech Debt

**Engine Complexity (`internal/engine/workflow/engine.go`):**
- Issue: 1832-line file with 10+ mutex fields (transitionMu, planMu, messagesMu, cachedFullPromptsMu, codeIntelMu, cacheMu, pauseMu, modelIDMu, workflowModeMu, perPhaseModelsMu)
- Files: `internal/engine/workflow/engine.go`
- Impact: Hard to reason about concurrency; high risk of deadlock or race conditions
- Fix approach: Extract state management into smaller, focused structs with their own locking. Consider using channels for inter-component communication instead of mutexes.

**Large Test Files (coverage_boost_test.go patterns):**
- Issue: Multiple 1000+ line test files (`internal/tools/extra_test.go`: 4397 lines, `internal/engine/workflow/coverage_boost_test.go`: 3559 lines) appear to be auto-generated coverage boosts rather than meaningful tests
- Files: `internal/tools/extra_test.go`, `internal/engine/workflow/coverage_boost_test.go`
- Impact: Masks genuine test coverage gaps; tests may not reflect real usage patterns
- Fix approach: Audit test files for meaningful assertions; remove auto-generated fluff; add integration tests that exercise real workflows

**Deferred Error Handling (nolint:errcheck patterns):**
- Issue: ~50+ instances of `defer x.Close() //nolint:errcheck` across the codebase, particularly in session management, provider clients, and git operations
- Files: `internal/engine/session/manager.go:124`, `internal/integrations/provider/base_client.go:162`, `internal/integrations/git/git.go`
- Impact: Silent failures when closing resources; potential resource leaks under error conditions
- Fix approach: Log close errors at debug level; use explicit error checking for critical resources (database connections, file locks)

**Hardcoded Ship Preflight Test TODO:**
- Issue: `ship_preflight_test.go:20` contains `// TODO: fix this later` that was never addressed
- Files: `internal/engine/workflow/ship_preflight_test.go`
- Impact: Minor but indicates incomplete work; test may not be exercising the intended scenario
- Fix approach: Complete the test implementation or remove the TODO comment

## Known Bugs

**SSE Parser Watchdog Timer Pattern:**
- Symptoms: The SSE parser uses `resp.Body.Close()` inside a `time.AfterFunc` callback to signal timeout, which is unconventional and may cause issues if the response body is accessed after timeout
- Files: `internal/integrations/provider/sse.go:30-32`
- Trigger: Stream timeout when no data arrives within `DefaultStreamTimeout`
- Workaround: None currently; the watchdog is properly reset on each read

**Dev Server Port Race:**
- Symptoms: `waitForPort()` in `devserver.go` polls with `net.DialTimeout` but doesn't verify the connection is to the expected server
- Files: `internal/tools/exec/devserver.go:574-586`
- Trigger: Another process could claim the port before the dev server starts
- Workaround: Check server response content after port is available

## Security Considerations

**Shell Command Execution:**
- Risk: `shell_unix.go` executes commands via `sh -c <command>` which passes the entire command string to the shell, enabling shell metacharacter injection
- Files: `internal/integrations/shell/shell_unix.go:14`, `internal/tools/exec/bash_unix.go:16`
- Current mitigation: Bash tool has `CheckDangerousCommand()` blocklist and `validateCommandSyntax()` for quote balancing
- Recommendations: Consider using `exec.Command` with explicit arguments for critical operations; add input sanitization for user-provided command strings

**Environment Variable Scrubbing Completeness:**
- Risk: `ScrubEnvironment()` in `bash_sandbox_linux.go` removes known sensitive vars but may miss custom secrets (e.g., `MY_APP_SECRET`)
- Files: `internal/tools/exec/bash_sandbox_linux.go:181-256`
- Current mitigation: Prefix-based matching (`API_KEY`, `TOKEN`, `SECRET`, etc.) catches common patterns
- Recommendations: Allow users to configure additional sensitive env var patterns; consider using a whitelist approach instead of blacklist

**API Key Loading from Environment:**
- Risk: Config loader reads API keys directly from environment variables (`OPENROUTER_API_KEY`, `ZEN_API_KEY`, `NVIDIA_API_KEY`) without validation or sanitization
- Files: `internal/core/config/loader.go:568-600`
- Current mitigation: Keys are stored in OS keychain via `pkg/keychain/` when possible
- Recommendations: Validate key format before use; warn when keys are loaded from env vs keychain; consider adding key rotation support

**Dev Server Environment Passthrough:**
- Risk: `startGoServer()` and `startRustServer()` pass the full parent environment to child processes via `os.Environ()`
- Files: `internal/engine/workflow/runtime.go:226,245`
- Current mitigation: None specific to runtime phase
- Recommendations: Use the same `ScrubEnvironment()` function used by Bash tool; filter sensitive vars before spawning dev servers

## Performance Bottlenecks

**CodeIntel Build Time:**
- Problem: `getCodeIntel()` has a 30-second timeout for building the codebase indexer, which may be too long for large projects
- Files: `internal/engine/workflow/engine.go:1454-1470`
- Cause: Tree-sitter parsing of all source files on first access
- Improvement path: Make incremental; cache results across sessions; consider lazy loading per-file instead of full index build

**Rate Limiter Token Bucket:**
- Problem: Rate limiter uses a channel-based token bucket that may cause goroutine scheduling overhead under high tool concurrency
- Files: `internal/tools/dispatcher.go:95-139`
- Cause: Two separate goroutines refill token buckets at fixed intervals
- Improvement path: Consider using `golang.org/x/time/rate` for more efficient rate limiting; combine rate limiters if possible

**SSE Parser Buffer Allocation:**
- Problem: `NewSSEParserWithContext` allocates a 1MB buffer (`sseMaxLineLength`) for every SSE connection
- Files: `internal/integrations/provider/sse.go:26`
- Cause: Conservative buffer size to handle large SSE events
- Improvement path: Use pooled buffers; allocate based on typical response sizes

**Dev Server Log Buffer:**
- Problem: `ringBuffer` in devserver.go holds up to 256KB per server with no upper bound on number of servers
- Files: `internal/tools/exec/devserver.go:22-26,42-68`
- Cause: Each dev server maintains its own ring buffer
- Improvement path: Set maximum total memory for all dev server logs; implement shared buffer pool

## Fragile Areas

**Engine State Machine:**
- Files: `internal/engine/workflow/engine.go`, `internal/engine/workflow/state_machine.go`
- Why fragile: Complex state transitions with many mutex-protected fields; phase transitions serialized by `transitionMu` but other state mutations use different locks
- Safe modification: Always acquire locks in consistent order; use race detector tests; avoid holding multiple locks simultaneously
- Test coverage: `internal/engine/workflow/engine_race_test.go` covers concurrent SetModel/providerAndModel but not all mutex combinations

**Dispatcher Permission Flow:**
- Files: `internal/tools/dispatcher.go`
- Why fragile: Complex channel-based permission flow with batch approvals, timeouts, and per-request routing via `sync.Map`
- Safe modification: Test with `-race` flag; ensure `Stop()` is called before discarding dispatcher; verify channel cleanup in `Stop()`
- Test coverage: `internal/tools/dispatcher_test.go` covers basic flows but edge cases around timeout/cancel may be undertested

**Task Runner Parallel Execution:**
- Files: `internal/engine/taskrunner/runner.go`
- Why fragile: Goroutine-based parallel execution with WaitGroup and semaphore; task state mutations must be thread-safe
- Safe modification: Never modify task state from outside the runner; use the provided callbacks for lifecycle events
- Test coverage: `internal/engine/taskrunner/runner_test.go` exists but parallel execution scenarios may not be fully covered

**Keychain Abstraction:**
- Files: `internal/integrations/keychain/keychain_linux.go`, `internal/integrations/keychain/keychain_darwin.go`
- Why fragile: Platform-specific implementations with D-Bus, `pass` CLI, and macOS Keychain; fallback logic between backends
- Safe modification: Test on target platform; verify fallback behavior when primary backend unavailable
- Test coverage: `internal/integrations/keychain/extra_test.go` covers some scenarios but D-Bus integration may not be testable in CI

## Scaling Limits

**Session Manager:**
- Current capacity: Single session per project directory (`.m31a/` folder)
- Limit: Concurrent access to same project from multiple terminals may cause file lock contention
- Scaling path: Consider per-session subdirectories; implement proper file locking with retry

**Message History:**
- Current capacity: Configurable `MaxMessageHistory` (default 1000) per session
- Limit: Large conversations may exceed context window before compaction triggers
- Scaling path: Proactive compaction (already implemented); monitor `toolCallsSinceLastCompact` counter

**Tool Output Store:**
- Current capacity: `OutputStore` bounds tool output to prevent context window exhaustion
- Limit: Large file reads or command outputs may still cause memory pressure
- Scaling path: Already uses `BashOutputLimit`; consider streaming output instead of buffering

## Dependencies at Risk

**godbus/dbus/v5:**
- Risk: Linux keychain depends on D-Bus Secret Service which may not be available in all environments (containers, headless servers)
- Impact: API keys cannot be stored securely; falls back to `pass` CLI
- Migration plan: Document graceful degradation; consider `age` encryption as alternative

**charmbracelet/bubbletea:**
- Risk: TUI framework is actively developed with breaking changes between versions
- Impact: UI may break on upgrade; rendering bugs may be framework-level
- Migration plan: Pin version in go.mod; test UI changes thoroughly; monitor changelog for breaking changes

**tree-sitter (odvcencio/gotreesitter):**
- Risk: Code intelligence depends on tree-sitter bindings which require CGO for native compilation
- Impact: Build breaks if CGO unavailable; `CGO_ENABLED=0` constraint conflicts
- Migration plan: Already handled via build tags; verify `CGO_ENABLED=0` still works; consider pure-Go alternatives if performance acceptable

## Missing Critical Features

**Structured Logging:**
- Problem: Inconsistent logging across modules; some use `slog`, some use `log`, some use `fmt.Printf`
- Blocks: Observability and debugging in production; difficult to filter/aggregate logs

**Graceful Shutdown Coordination:**
- Problem: Engine has `done` channel and `cancel` func but shutdown flow across Dispatcher, DevServer, and Session Manager may not be fully coordinated
- Blocks: Clean process termination; resource cleanup on SIGINT/SIGTERM

**Configuration Hot-Reload Validation:**
- Problem: Config watcher triggers reload but doesn't validate new config before applying
- Blocks: Preventing runtime errors from invalid config changes

## Test Coverage Gaps

**Dispatcher Edge Cases:**
- What's not tested: Concurrent permission requests with batch approval; permission timeout under high load; `Stop()` called while requests pending
- Files: `internal/tools/dispatcher.go`
- Risk: Goroutine leaks or channel deadlocks in production
- Priority: High

**Engine Pause/Resume During Execute:**
- What's not tested: Pause/resume while tool calls are in-flight; skip/cancel task during parallel execution
- Files: `internal/engine/workflow/engine.go:183-297`
- Risk: State corruption or deadlock if pause/resume races with tool execution
- Priority: High

**SSE Parser Edge Cases:**
- What's not tested: Very large SSE events exceeding buffer; malformed SSE data; connection reset during stream
- Files: `internal/integrations/provider/sse.go`
- Risk: Panic or hang on malformed provider responses
- Priority: Medium

**Config File Watcher:**
- What's not tested: Rapid file changes (debounce behavior); config file deleted while watcher active; symlink to config file
- Files: `internal/core/config/loader.go:620-700`
- Risk: Panic or infinite loop on edge cases
- Priority: Medium

---

*Concerns audit: 2026-08-04*

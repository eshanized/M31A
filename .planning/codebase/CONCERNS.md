# Codebase Concerns

**Analysis Date:** 2026-07-10

## Tech Debt

**Oversized Source Files (>1000 lines):**
- Issue: Multiple source files exceed 1000 lines, indicating accumulated complexity that increases cognitive load and makes modifications riskier
- Files:
  - `internal/tui/sidebar_model.go` (1621 lines)
  - `internal/workflow/engine.go` (1494 lines)
  - `internal/tui/app_view.go` (1213 lines)
  - `internal/config/loader.go` (1154 lines)
  - `internal/tools/webfetch.go` (1147 lines)
  - `internal/tui/firstrun_view.go` (1144 lines)
  - `internal/workflow/execute.go` (1121 lines)
- Impact: Harder to navigate, review, and test individual concerns; increased merge conflict risk
- Fix approach: Extract focused sub-components; for example, split `engine.go` (1494 lines) into separate files for streaming, tool dispatch, and phase orchestration

**Hardcoded Model Capabilities Table:**
- Issue: `internal/provider/capabilities.go` contains a hardcoded fallback table of model capabilities (context windows, output tokens, tool support). New models from providers require code changes and a new release to work correctly.
- Files: `internal/provider/capabilities.go` (lines 117-170)
- Impact: New or updated models won't be recognized until the code is updated; the table will drift from reality over time
- Fix approach: The runtime detection path (API query + cache) is already implemented and should be the primary mechanism. The hardcoded table should be reduced to a minimal fallback and documented as "best-effort only." Consider auto-populating from provider API metadata when available.

**BUG-annotated Regression Tests:**
- Issue: Tests reference specific bug numbers (BUG-04, BUG-05, BUG-06, BUG-10, BUG-12, BUG-15) without linking to tracking issues or documenting the root cause. This makes it harder to understand if the underlying bugs were fully resolved.
- Files:
  - `internal/workflow/classify_test.go` (BUG-04, BUG-05, BUG-06)
  - `internal/workflow/ship.go` (BUG-10, BUG-15)
  - `internal/workflow/engine.go` (BUG-12)
- Impact: Future developers cannot determine if these are resolved fixes or active workarounds
- Fix approach: Add a comment block at the top of each test or referenced site summarizing the original bug and its resolution status, or link to the tracking issue.

**md5.Sum Used for Cache Keys:**
- Issue: `internal/workflow/execute.go:732` uses `md5.Sum([]byte(planMarkdown))` to create cache keys for parsed plans. While this is not a security concern (it's only used for in-memory caching, not cryptographic purposes), md5 is cryptographically broken and using it for any purpose sends the wrong signal.
- Files: `internal/workflow/execute.go` (line 732)
- Impact: Minor — no security risk, but could confuse security audits or code reviews
- Fix approach: Replace with `sha256.Sum256` or a faster non-cryptographic hash like `fnv` for cache keys, which is more idiomatic in Go.

## Known Bugs

**os.Setenv Race Condition in Config Loader:**
- Issue: `internal/config/loader.go:1150` calls `os.Setenv(key, value)` to load .env variables. The comment at line 1104 acknowledges this is "not goroutine-safe." The code attempts to mitigate this by calling early (before logger initialization), but the comment at `cmd/m31a/main.go:154` says "Load .env before logger to avoid goroutine race on os.Setenv." If any goroutine reads env vars after this point, there is a race.
- Files: `internal/config/loader.go` (line 1150), `cmd/m31a/main.go` (line 154)
- Trigger: Concurrent env var reads during or after .env loading
- Workaround: The current mitigation (loading early) works in practice, but this is fragile. A robust fix would pass env vars through a config struct rather than polluting the global process environment.

**Single TODO Marker in Production Code:**
- Issue: `internal/workflow/ship_preflight_test.go:20` contains `// TODO: fix this later` — the only genuine TODO comment found in the codebase. While this is in a test file, it indicates an unfinished item.
- Files: `internal/workflow/ship_preflight_test.go` (line 20)
- Impact: Low — test-only, but represents known incomplete work

## Security Considerations

**Config .env File Parsing — Variable Overwrite Guard:**
- Issue: `internal/config/loader.go:1149` checks `os.LookupEnv(key)` before setting environment variables from .env files, but if an env var is set with an empty value, the check passes and the .env value overwrites it. This is intentional per the design but could be surprising.
- Files: `internal/config/loader.go` (lines 1140-1152)
- Current mitigation: Only sets env vars that are not already present in the process environment
- Recommendations: Document this behavior explicitly in the .env.example file. Consider adding a `M31A_DOTENV_NO_OVERRIDE=true` option for strict no-overwrite mode.

**SSRF Protection in WebFetch:**
- Status: Well-implemented. The `internal/tools/webfetch.go` file implements DNS pinning, private/reserved IP blocking, redirect SSRF protection, and a post-connect paranoid re-check. This is a strong defense-in-depth pattern.
- Files: `internal/tools/webfetch.go` (lines 76-151)

**Keychain Integration for API Keys:**
- Status: Properly implemented. API keys are stored in OS keychain (`pkg/keychain/`) and never written to disk in plaintext. Platform-specific implementations exist for Linux (pass), macOS (security command), and Windows.
- Files: `pkg/keychain/keychain_linux.go`, `pkg/keychain/keychain_darwin.go`, `pkg/keychain/keychain_windows.go`

**Ship Preflight Secret Detection:**
- Status: Implemented with heuristic patterns. `internal/workflow/ship_preflight.go` detects hardcoded secrets (sk-, ghp_, glpat-, xoxb-, AKIA prefixes) and blocks the ship phase.
- Files: `internal/workflow/ship_preflight.go` (lines 66-83)
- Recommendations: The heuristic is prefix-based only. Consider adding regex patterns for base64-encoded secrets, JWT tokens, and hex-encoded keys for broader coverage.

## Performance Bottlenecks

**Polling-Based Server Readiness Checks:**
- Problem: Both `internal/workflow/runtime.go:303-326` and `internal/tools/devserver.go:576-584` use fixed 500ms sleep polling loops to check server readiness. For fast-starting servers, this adds unnecessary latency; for slow servers, the interval may be too aggressive.
- Files: `internal/workflow/runtime.go` (lines 303-326), `internal/tools/devserver.go` (lines 576-584)
- Cause: Simple polling with fixed interval; no exponential backoff or event-driven notification
- Improvement path: Implement exponential backoff starting at 100ms, or use filesystem watching for server output signals

**Gitignore Cache LRU Eviction:**
- Problem: `internal/tools/grep.go:480-533` implements a custom LRU cache with O(n) eviction due to slice-based ordering. For workspaces with many directories, the `getOrCreate` method performs a linear scan of the order slice on every cache hit.
- Files: `internal/tools/grep.go` (lines 480-533)
- Cause: The LRU order is maintained as a `[]string` with linear search for element removal
- Improvement path: Replace with a doubly-linked list + map pattern (like `container/list`) for O(1) LRU operations, or accept the current cost since 1024 max entries makes the linear scan bounded

**Codeintel Index Full Rebuild:**
- Problem: `internal/codeintel/codeintel.go:60-84` performs a full re-parse of all project files when the cache is invalidated. For large projects, this could introduce noticeable latency on first tool invocation.
- Files: `internal/codeintel/codeintel.go` (lines 60-84), `internal/codeintel/parser.go` (861 lines)
- Cause: The indexer falls back to full build when cache is missing or corrupted
- Improvement path: The incremental path (`buildIncremental`) is already implemented and used when cache exists. Consider persisting the cache across sessions (it appears to be session-scoped currently based on `workDir`)

## Fragile Areas

**TUI Emitter Channel Backpressure:**
- Files: `internal/tui/app_channel.go` (lines 62-82), `internal/tui/app.go` (lines 520-536)
- Why fragile: The `channelEmitter` drops messages after `maxRetries` attempts when the Bubble Tea channel is full. During high-load tool execution, the emitter can be saturated, causing dropped workflow messages. The `globalDropCounter` tracks this but does not trigger any recovery.
- Safe modification: Always check `DroppedMessages()` after a workflow phase completes. Consider adding a warning toast when drops exceed a threshold.
- Test coverage: `internal/tui/app_channel_test.go` covers basic send/receive patterns

**Parallel Tool Execution in Execute Phase:**
- Files: `internal/workflow/execute.go` (lines 400-510)
- Why fragile: Tools are dispatched in parallel goroutines with a semaphore, but each goroutine's panic recovery writes to `toolExecResults[idx]` without synchronization (relying on index-based access). The `recover()` handlers emit error messages to the channel, which could interleave with normal tool completion messages.
- Safe modification: Never modify the concurrency model without running `make test` (race detector enabled)
- Test coverage: `internal/tools/concurrency_test.go`, `internal/tools/dispatcher_test.go`

**Self-Heal Loop Interaction:**
- Files: `internal/workflow/execute.go` (lines 193-400)
- Why fragile: The heal loop can re-enter the LLM stream, generate new tool calls, and loop back through quality gates. The interaction between `qualityGatePending`, `healTask`, and `streamLLMWithTools` creates a complex state machine with multiple exit conditions.
- Safe modification: Never increase `MaxHealAttempts` beyond 3 without comprehensive integration testing
- Test coverage: `internal/workflow/engine_test.go`, `internal/workflow/coverage_boost_test.go`

**Runtime Server Lifecycle:**
- Files: `internal/workflow/runtime.go` (lines 180-300)
- Why fragile: Go/Rust/Python/Node server processes are started via `exec.CommandContext` with port discovery via TOCTOU-vulnerable `findFreePort`. Process cleanup relies on `killProcessGroup` with graceful/force kill escalation, but orphan processes can remain if the cleanup path is not reached (e.g., panic in the calling goroutine).
- Safe modification: Always test server start/stop cycles manually after changes; check for orphaned processes with `lsof -i :PORT`
- Test coverage: `internal/workflow/runtime.go` (integration tests), `internal/workflow/engine_test.go`

## Scaling Limits

**REPL Message History:**
- Current capacity: 500 messages (`internal/tui/constants.go:27`)
- Limit: Beyond 500 messages, older messages are pruned. This is bounded but may be insufficient for long-running sessions.
- Scaling path: The `MaxMessages` constant can be increased, but each message carries token estimates and tool call caches that consume memory. The compaction system (`pkg/compaction/`) handles proactive compression.

**Subagent Concurrency:**
- Current capacity: 3 concurrent subagents (`internal/tools/subagent/manager.go`), 50 total per session
- Limit: Hard caps prevent resource exhaustion but may be restrictive for complex multi-agent workflows
- Scaling path: Increase `MaxConcurrent` and `MaxTotalSubagents` constants if needed, but verify memory impact

**Tool Output Store:**
- Current capacity: Bounded by `internal/tools/output_store.go` with mutex-protected writes
- Limit: Tool outputs are stored in memory per session; large grep or file read outputs could consume significant memory
- Scaling path: Implement output truncation at the store level (currently done at the display layer)

## Dependencies at Risk

**go 1.25.0 in go.mod:**
- Risk: The `go.mod` specifies `go 1.25.0` which is a future Go version. This could cause issues with standard Go tooling if the actual Go version installed differs.
- Impact: Build failures or unexpected behavior with different Go versions
- Migration plan: Ensure all developers use the exact Go version specified. The Makefile should validate the Go version.

**tree-sitter Dependency:**
- Risk: `github.com/odvcencio/gotreesitter v0.20.5` is used for code intelligence parsing. Tree-sitter bindings require CGO for the underlying C library, which conflicts with the `CGO_ENABLED=0` build constraint specified in AGENTS.md.
- Files: `internal/codeintel/parser.go` (uses tree-sitter for parsing), `go.mod` (line 24)
- Impact: If tree-sitter requires CGO, the static binary build would break. This needs verification — the codebase may have a fallback path or build tag that excludes tree-sitter when CGO is disabled.
- Migration plan: Verify that the codeintel parser works without CGO. If tree-sitter requires CGO, implement a build tag to use a pure-Go fallback parser.

**tiktoken-go Dependency:**
- Risk: `github.com/pkoukk/tiktoken-go v0.1.8` is used for token estimation. This library may have its own dependency tree that could introduce CGO requirements or compatibility issues.
- Files: `internal/tokens/` (token estimation), `go.mod` (line 14)
- Impact: Token estimation is critical for context window management; if this dependency fails, the system cannot correctly estimate context usage
- Migration plan: Monitor for updates; consider maintaining a fallback token estimator based on character count heuristics

## Test Coverage Gaps

**Ship Preflight Test Coverage:**
- What's not tested: The ship preflight checks for TODO markers, debug statements, and hardcoded secrets, but only tests the Go language patterns. JavaScript/TypeScript debug patterns (console.log) are checked but the test coverage for the secret detection patterns is minimal.
- Files: `internal/workflow/ship_preflight_test.go` (117 lines)
- Risk: Secret detection heuristics could miss new secret formats or produce false positives
- Priority: Medium — the current patterns cover the most common cases

**Codeintel Parser Coverage:**
- What's not tested: The `internal/codeintel/parser.go` (861 lines) implements parsers for Go, JavaScript, TypeScript, Python, Rust, and other languages, but the test file `internal/codeintel/parser_test.go` has limited coverage of edge cases for each language.
- Files: `internal/codeintel/parser.go`, `internal/codeintel/parser_test.go`
- Risk: Parser bugs could cause incorrect import resolution or symbol indexing, affecting code intelligence quality
- Priority: Medium — parsers are used for code context, not correctness-critical operations

**TUI View Rendering:**
- What's not tested: Several TUI view files (`app_view.go`, `firstrun_view.go`, `sidebar_model.go`) contain complex rendering logic but have limited test coverage. The `coverage_boost_test.go` files suggest an effort to improve coverage.
- Files: `internal/tui/app_view.go` (1213 lines), `internal/tui/firstrun_view.go` (1144 lines)
- Risk: Visual regressions could be introduced without detection; responsive layout edge cases may not be covered
- Priority: Low — TUI rendering is primarily cosmetic, though layout bugs can affect usability

## Missing Critical Features

**Structured Error Recovery for Provider Failures:**
- Problem: When an LLM provider fails mid-stream (network error, rate limit), the system falls back to another provider, but there is no mechanism to resume the partially-completed stream from the new provider. This means the full conversation context must be re-sent.
- Blocks: Optimal cost and latency during provider failover; users may experience duplicate token charges.

**Test Coverage Reporting in CI:**
- Problem: The `Makefile` has `cover` target for local coverage reports, but there is no CI integration for tracking coverage trends over time. The 75% target (90% for critical packages) is specified in AGENTS.md but not enforced automatically.
- Blocks: Regression detection when coverage drops; no historical trend data for coverage quality.

---

*Concerns audit: 2026-07-10*

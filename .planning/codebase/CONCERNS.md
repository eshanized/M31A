# Codebase Concerns

**Analysis Date:** 2026-07-13

## Tech Debt

**Engine struct god-object:**
- Issue: `Engine` struct in `internal/workflow/engine.go:99-158` has 30+ fields including multiple mutexes (`transitionMu`, `planMu`, `messagesMu`, `modelIDMu`, `workflowModeMu`, `perPhaseModelsMu`, `codeIntelMu`), cached state, subsystem references, and config. WorkflowState (lines 43-87) is a partial extraction but the Engine still holds everything.
- Files: `internal/workflow/engine.go`
- Impact: High cognitive load when modifying workflow logic; difficult to test individual subsystems in isolation; risk of mutex ordering bugs.
- Fix approach: Continue extracting `WorkflowState` responsibilities. Consider splitting Engine into composed subsystem interfaces (e.g., `PlanManager`, `PhaseRunner`, `ContextBuilder`) and injecting them rather than accumulating fields.

**AppState struct god-object:**
- Issue: `AppState` in `internal/tui/app.go` accumulates 50+ fields across `app.go`, `app_view.go`, `app_update.go`, `app_nav.go`, `app_update_commands.go`, and `repl_state.go`. It is the sole Bubble Tea model managing all screens, workflow engine, dispatcher, session manager, sidebar, settings, and more.
- Files: `internal/tui/app.go:220-318` and related `app_*.go` files
- Impact: Every new feature adds fields to AppState. The `Update()` method across all `app_*.go` files is a monolithic message handler.
- Fix approach: Extract screen-specific state into dedicated model structs that implement `tea.Model` and delegate from AppState.

**SidebarModel excessive size:**
- Issue: `internal/tui/sidebar_model.go` is 1621 lines and the `SidebarModel` struct (lines 61-144) has 30+ fields spanning git status, TODO items, task progress, token tracking, burn rates, phase pipeline, tool call timeline, compaction events, sub-agent counts, narrative state, and file watchers.
- Files: `internal/tui/sidebar_model.go`
- Impact: Hard to understand which fields are used by which sidebar mode; rendering code mixes 5 different display modes in a single file.
- Fix approach: Extract per-mode rendering into separate functions or structs (SidebarTodoRenderer, SidebarFilesRenderer, etc.) and reduce SidebarModel fields to shared state only.

**Engine has 1551 lines across multiple files:**
- Issue: Core engine logic spans `engine.go` (1551), `engine_parse.go` (726), `engine_verify.go`, `engine_wiring_test.go`, plus `execute.go` (1121), `ship.go` (631), `runtime.go` (521), and many more.
- Files: `internal/workflow/engine.go`, `internal/workflow/engine_parse.go`, `internal/workflow/execute.go`
- Impact: The workflow package has ~10,000+ lines of non-test code. Understanding execution flow requires reading across many files.
- Fix approach: Extract `Engine` methods into logical sub-packages (e.g., `internal/workflow/execute/`, `internal/workflow/ship/`) or reduce Engine's role to orchestration only.

## Known Bugs

**Dead code / inverted condition in DevServer restartServer:**
- Symptoms: In `restartServer`, the code at lines 285-288 checks `if e, ok := d.processes[id]; !ok` (entry NOT found) and then discards `e` via `_ = e`. The comment says "entry was deleted, use stored env" but the extracted `e` is the zero value from the failed map lookup.
- Files: `internal/tools/devserver.go:285-288`
- Trigger: When `id` is not in `d.processes` (which shouldn't happen since it was just checked on line 261, but after the `stopByID` call on line 274, the entry is removed). The intent was to fall through to the loop below, but the dead `if` block is misleading.
- Workaround: The fallback loop at lines 290-294 runs regardless (it's outside the `if` block), so the feature works by accident. The dead `if` block should be removed.

**Ignored strconv.Atoi errors in diff summary:**
- Symptoms: `internal/workflow/diff_summary.go:46,49` — `strconv.Atoi` errors are discarded with `_`. If git numstat returns unexpected non-numeric values, additions/deletions silently become 0.
- Files: `internal/workflow/diff_summary.go:43-50`
- Trigger: Git diff with binary files or unusual numstat output.
- Workaround: In practice, git numstat is reliable, so this is low-impact. But the errors should be logged.

## Security Considerations

**os.Setenv goroutine-unsafe call:**
- Risk: `internal/config/loader.go:1150` calls `os.Setenv(key, value)` inside `sync.Once`. While `sync.Once` prevents duplicate .env loading, the Go docs explicitly state that `os.Setenv` is not goroutine-safe. If any goroutine reads `os.Environ()` concurrently (logger initialization, env-based config), this is a data race.
- Files: `internal/config/loader.go:1150`
- Current mitigation: `LoadDotEnv` is documented as needing to be called before goroutines start (line 1103-1104), and it's guarded by `sync.Once`. `main.go` calls it early.
- Recommendations: This is correctly sequenced in practice, but the `os.Setenv` call is inherently unsafe. Consider using a `map[string]string` for env overrides that is read explicitly rather than polluting the process environment.

**Bash tool command injection surface:**
- Risk: The Bash tool (`internal/tools/bash.go`) executes arbitrary shell commands. While it has obfuscation detection (line 85) and command syntax validation (line 93), sophisticated bypass attempts (e.g., encoded payloads via `$()` in environment variables) could potentially evade detection.
- Files: `internal/tools/bash.go:72-98`
- Current mitigation: Risk level is `RiskDangerous` requiring explicit user permission; blocklist of dangerous commands; obfuscation pattern detection.
- Recommendations: Consider a sandboxed execution environment for high-risk operations (the Linux seccomp path in `bash_sandbox_linux.go` exists but may not be active on all platforms).

**API key handling in error messages:**
- Risk: Provider error messages could leak API keys in logs or user-facing output.
- Files: `internal/provider/common.go:294-303`
- Current mitigation: `maskAPIKeys()` function redacts known patterns (sk-*, key-*, api_key) from error strings. Applied in `SanitizeProviderError()`.
- Recommendations: The regex pattern `(?i)(sk-[a-zA-Z0-9]{8,}|key-[a-zA-Z0-9]{8,}|api[_-]?key[_\s:=]+["']?)([a-zA-Z0-9]{4,})` may miss non-standard key formats. Consider adding patterns for bearer tokens and custom provider key formats.

## Performance Bottlenecks

**Excessively large test files:**
- Problem: Three test files are extremely large: `internal/tools/extra_test.go` (4441 lines), `internal/workflow/coverage_boost_test.go` (3570 lines), `internal/workflow/engine_extra_test.go` (3409 lines). These appear to be generated or bulk-added for coverage targets rather than testing meaningful scenarios.
- Files: `internal/tools/extra_test.go`, `internal/workflow/coverage_boost_test.go`, `internal/workflow/engine_extra_test.go`
- Cause: Aggressive pursuit of 75% coverage target has led to boilerplate-heavy test files.
- Improvement path: Audit these test files for redundant tests. Extract shared test helpers. Consider table-driven tests with shared fixtures to reduce line count while maintaining coverage.

**Sidebar refresh on every file change:**
- Problem: The file watcher (`internal/tui/filewatcher.go`) triggers sidebar refresh on every filesystem change. In large repositories, this can fire hundreds of times during a build, each triggering a full sidebar re-render.
- Files: `internal/tui/filewatcher.go:54-57`, `internal/tui/app.go:624-636`
- Cause: No debouncing or batching on filesystem events.
- Improvement path: Add debouncing (e.g., coalesce events within a 500ms window) before triggering sidebar refresh.

**Codeintel indexer on every execute group:**
- Problem: The codeintel index (`internal/codeintel/`) is rebuilt for each execute group, parsing all source files in the project. For large codebases, this can add significant latency.
- Files: `internal/codeintel/codeintel.go`, `internal/workflow/engine.go:132-135`
- Cause: `codeIntelBuilt` flag is reset between execute groups.
- Improvement path: Cache the index to disk (already partially implemented in `internal/codeintel/cache.go`) and invalidate based on file change timestamps rather than full rebuilds.

## Fragile Areas

**Provider model list dynamic loading:**
- Files: `internal/provider/capabilities.go:117-122`, `internal/provider/cache.go`
- Why fragile: Model lists are fetched from APIs at runtime and cached with TTL. If a provider API changes response format, returns unexpected models, or the cache becomes stale, the UI shows empty or broken model lists. The `deprecatedNvidiaModels` and `brokenOnNvidiaNIM` lists (lines 81-103) are manually maintained workarounds.
- Safe modification: Never hardcode model names. Use the pattern-matching detection in `capabilities.go` to auto-classify new models. When adding to blocklist tables, add corresponding test cases in `capabilities_test.go`.
- Test coverage: Good — `internal/provider/capabilities_test.go` has extensive tests.

**SSE streaming parser:**
- Files: `internal/provider/reasoning.go:150-180`, `internal/provider/sse.go`
- Why fragile: SSE parsing must handle multiple provider-specific quirks: NVIDIA returns errors as HTTP 200 with error objects in the body; different model families use different SSE field paths for reasoning content; the `ParseSSEChunk` function has complex nested JSON path resolution.
- Safe modification: Always test with all three providers (OpenRouter, Zen, NVIDIA). Use the `reasoningParamMap` for provider-specific field mappings rather than hardcoding paths.
- Test coverage: Moderate — `internal/provider/sse_test.go` exists but provider-specific edge cases may be underrepresented.

**Workflow state machine transitions:**
- Files: `internal/workflow/engine.go:757-768`, `internal/workflow/state_machine.go`
- Why fragile: Phase transitions are serialized via `transitionMu` and validated by `StateMachine.Transition()`. Incorrect transition sequences could deadlock or corrupt session state. The `maxDiscussPlanCycles = 3` constant (line 753) is a hardcoded guard against oscillation that may need adjustment.
- Safe modification: Always use `Engine.Transition()` to change phases. Never mutate phase state directly. Run `TestStateMachine` tests when modifying transition logic.
- Test coverage: Good — dedicated race tests exist.

**Persistent permission state:**
- Files: `internal/tools/dispatcher.go:32-38`, `internal/tools/permissions.go`
- Why fragile: Permissions are cached per project via `PersistentPermissions` and merged with config rules at startup. If the cache file becomes corrupted or contains outdated rules, tool execution behavior changes silently.
- Safe modification: Permission changes should go through the TUI permission modal, not direct file edits. The `originalRules` backup (line 33) allows reset but is session-scoped.
- Test coverage: Good — `internal/tools/permissions_test.go` covers most scenarios.

## Scaling Limits

**Tool concurrency semaphore:**
- Current capacity: `MaxConcurrentTools` limits parallel tool executions (default 4, configurable via `cfg.Tools.MaxToolConcurrency`).
- Limit: With 18 tools registered and subagents potentially spawning concurrent tool calls, the semaphore can become a bottleneck during parallel task execution in the execute phase.
- Scaling path: The semaphore is correctly implemented with `chan struct{}`. Increase `MaxConcurrentTools` if users report slow execution phases, but beware of provider rate limits.

**Session message history:**
- Current capacity: `config.UI.MaxMessages` (default 500) and `config.UI.MaxMessageHistory` (default 1000).
- Limit: Long-running sessions with many tool calls can exceed these limits, triggering compaction. The compaction logic (`internal/compaction/`) is tested but the quality of compressed messages may degrade with very long sessions.
- Scaling path: The compaction threshold is configurable. Users with long sessions should increase `MaxMessages`.

## Dependencies at Risk

**go 1.25.0:**
- Risk: `go.mod` specifies `go 1.25.0` which is a future/unreleased Go version (as of July 2026, Go 1.25 may be very recent or still in beta). Building requires this exact Go version.
- Impact: CI/CD must pin Go 1.25+. Developers with older Go versions cannot build.
- Migration plan: Monitor Go release schedule. When 1.25 is stable, ensure CI uses the latest patch release.

**charmbracelet/bubbletea v1.3.0:**
- Risk: The TUI is built entirely on Bubble Tea's Elm architecture. A breaking change in bubbletea would require rewriting all message handling across 20+ model files.
- Impact: TUI would break on upgrade.
- Migration plan: Pin to v1.3.0 and test thoroughly before upgrading. The `go.sum` has 121 lines, indicating a manageable dependency tree.

**godbus/dbus/v5:**
- Risk: Used by the Linux keychain implementation (`pkg/keychain/keychain_linux.go`) for D-Bus Secret Service access. If the D-Bus interface is unavailable (e.g., in containers), the keychain falls back to pass CLI or becomes unavailable.
- Impact: API keys cannot be stored securely on systems without D-Bus or pass.
- Migration plan: The `cachedKeychain` wrapper (`pkg/keychain/keychain.go:42-54`) gracefully handles `ErrKeychainUnavailable`. Document that users without D-Bus should set API keys via environment variables.

## Missing Critical Features

**No structured logging in TUI:**
- Problem: The TUI layer uses `slog.Warn/Info` which writes to stdout/stderr. In a terminal UI application, these messages are invisible or corrupt the display.
- Blocks: Debugging TUI issues requires reading log files rather than seeing warnings in context.

**No graceful degradation for missing providers:**
- Problem: If no API keys are configured, the application shows the first-run screen but does not clearly communicate which providers are available vs. unavailable.
- Blocks: New users may not understand why model lists are empty.

## Test Coverage Gaps

**Coverage boost tests as dominant pattern:**
- What's not tested meaningfully: `internal/tools/extra_test.go` (4441 lines), `internal/workflow/coverage_boost_test.go` (3570 lines), and `internal/workflow/engine_extra_test.go` (3409 lines) contain many tests that appear to exist solely for coverage numbers rather than testing real behavior. Many use `context.TODO()` and minimal assertions.
- Files: `internal/tools/extra_test.go`, `internal/workflow/coverage_boost_test.go`, `internal/workflow/engine_extra_test.go`
- Risk: False confidence in coverage metrics; actual edge cases may be untested while trivial paths are over-tested.
- Priority: Medium — Audit these files against actual code paths in their respective packages. Replace boilerplate tests with meaningful scenario-based tests.

**E2E tests skip real API calls by default:**
- What's not tested: `e2e_test.go` tests that hit real APIs (`TestBinary_Prompt_*RealAPI`) are skipped unless `OPENROUTER_API_KEY`, `ZEN_API_KEY`, or `NVIDIA_API_KEY` environment variables are set.
- Files: `e2e_test.go`
- Risk: Integration issues between provider clients and the actual APIs go undetected in CI.
- Priority: Low — This is intentional to avoid CI costs and flaky network-dependent tests. Consider a nightly CI job that runs real API tests.

**Platform-specific code not cross-tested:**
- What's not tested: `internal/tools/bash_sandbox_linux.go` (seccomp sandbox), `internal/tools/bash_sandbox_darwin.go`, `internal/tools/prockill_windows.go`, `internal/workflow/ship_lock_unix.go`, and `internal/workflow/runtime_proc_windows.go` use build tags and can only be tested on their target platforms.
- Files: `internal/tools/bash_sandbox_linux.go`, `internal/tools/prockill_windows.go`, `internal/workflow/runtime_proc_windows.go`
- Risk: Platform-specific bugs go undetected when developing on a single OS.
- Priority: Medium — Ensure CI runs tests on all target platforms (linux/amd64, darwin/amd64, windows/amd64 at minimum).

---

*Concerns audit: 2026-07-13*

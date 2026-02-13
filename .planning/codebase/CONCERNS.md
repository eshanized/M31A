# Codebase Concerns

**Analysis Date:** 2026-06-11

## Tech Debt

**Architecture Rule Violation — `internal/tools` imports `internal/config`:**
- Issue: The `internal/tools/` package imports `internal/config/` for `config.PermissionRule` and `config.PermissionsConfig` types. Per AGENTS.md and docs/ARCHITECTURE.md, `internal/tools/` may only import `internal/types/` and `internal/errors/`.
- Files: `internal/tools/dispatcher.go`, `internal/tools/permissions.go`, `internal/tools/defaults.go`
- Impact: Violates the declared dependency rule. Creates a circular-ish conceptual dependency where the tools layer knows about configuration layer types.
- Fix approach: Move `PermissionRule`, `PermissionsConfig`, and `PermissionsAgentConfig` from `internal/config/types.go` into `internal/types/`. Update all import paths across 6+ files. This is a Phase 12 task (D-29) per ROADMAP.md.

**TODO: `truncateToVisibleWidth` is a no-op:**
- Issue: The function at `internal/tui/app_view.go:251` returns the input string unchanged with a `TODO: use layout.TruncateToWidth when exported` comment. Styled strings wider than the terminal are rendered without truncation.
- Files: `internal/tui/app_view.go`
- Impact: Styled strings in the REPL header and message area can overflow the terminal width, causing visual glitches on narrow terminals.
- Fix approach: Export `layout.TruncateToWidth` and call it here, or implement a styled-string-aware truncation function.

**`BurntSushi/toml` v1 is in maintenance mode:**
- Issue: `go.mod` line 6 marks this as DEP-3: "in maintenance mode; v2 has different API; migrate when ready."
- Files: `go.mod`, `internal/config/loader.go`
- Impact: No new features or bug fixes from upstream. Blocking migration to tomlv2 which has a different API surface.
- Fix approach: Plan a dedicated migration phase to tomlv2 API when capacity allows.

**`doublestar` v4 pinned with known issues:**
- Issue: `go.mod` line 8 marks this as DEP-2: "pin current version; check for breaking changes before upgrading."
- Files: `internal/tools/permissions.go`, `internal/tools/grep.go`
- Impact: Glob pattern matching for permission rules and grep file filtering depends on this library's behavior. Upgrades could change matching semantics.
- Fix approach: Keep pinned. Write integration tests against doublestar's behavior before any upgrade.

**Global mutable state — `permissionRequestID` and `questionRequestIDCounter`:**
- Issue: Two package-level `atomic.Int64` counters in `internal/tools/interface.go:13` and `internal/tools/question.go:29` are never reset. Over extremely long sessions with millions of tool calls, the int64 could theoretically overflow (though practically impossible).
- Files: `internal/tools/interface.go`, `internal/tools/question.go`
- Impact: Negligible. The values are monotonically increasing and used for correlation, not indexing.
- Fix approach: No immediate fix needed. If sessions ever exceed 2^63 tool calls, use UUID-based correlation.

---

## Known Bugs

**Glob tool — rg code path fails on relative paths when CWD ≠ workDir:**
- Symptoms: When using the `rg` code path in the Glob tool, `os.Stat` fails on relative paths returned by `rg --files` when the current working directory differs from the tool's configured workDir.
- Files: `internal/tools/glob.go` (rg execution path), `internal/tools/glob_test.go:164-165, 185-186`
- Trigger: Running Glob with `rg` in `$PATH` from a different working directory than the tool's workDir.
- Workaround: Tests skip strict output validation with `t.Skipf("skipping rg-based test: %v", err)`. Production behavior degrades to returning empty results for affected patterns.

**Test failures — `TestVarSubstitution_UnsetVar`:**
- Symptoms: The test expects `${UNSET_VAR}` to be replaced with an empty string when the env var is not set, but the `substituteVars` function returns the original `${UNSET_VAR}` pattern.
- Files: `internal/config/loader_test.go:656`
- Trigger: Config values containing `${NONEXISTENT_VAR}` are not substituted.
- Workaround: None — this is a real behavior difference between test expectation and implementation.

**Test failures — `TestDispatcher_UnknownTool`:**
- Symptoms: The test expects an "unknown tool" error message with "Available tools" in the error, but gets a JSON parse error instead because the test sends empty input.
- Files: `internal/tools/dispatcher_test.go:63, 66`
- Trigger: Calling `Dispatcher.Execute` with an unknown tool name and empty input.
- Workaround: None — the test expectation doesn't match the implementation's error path (JSON parsing fails before tool lookup).

**Test failures — `TestChatCompletionStream_WithTools`:**
- Symptoms: Test expects tool definitions in the request body sent to the provider, but they are not present.
- Files: `internal/provider/zen/client_test.go:561`
- Trigger: Zen provider's `ChatCompletionStream` may not serialize tool definitions in the request body.
- Workaround: Unknown — this could be a test issue or a provider implementation gap.

---

## Security Considerations

**SSRF protection for WebFetch is well-implemented but has a subtle gap:**
- Risk: The `WebFetch` tool resolves DNS first, then checks all resolved IPs against private ranges. However, the DNS resolution and the actual HTTP connection happen in separate steps. While the DNS cache (`sync.Map` in `internal/tools/webfetch.go:51`) pins IPs for the connection, the initial resolution could be poisoned if DNS is compromised between the check and the connection.
- Files: `internal/tools/webfetch.go:61-79`
- Current mitigation: DNS cache with 5-minute TTL pins IPs for the duration of the request. All resolved IPs are checked, not just the first.
- Recommendations: Consider using a custom `net.Resolver` that enforces the IP check at the resolver level, or use a dedicated SSRF-safe HTTP client library.

**`unsafe.Pointer` usage in Windows keychain:**
- Risk: `pkg/keychain/keychain_windows.go` uses `unsafe.Pointer` extensively for Windows Credential Manager FFI calls (lines 43-88). Incorrect pointer arithmetic or lifetime management could cause memory corruption.
- Files: `pkg/keychain/keychain_windows.go`
- Current mitigation: The `unsafe` usage is confined to well-documented FFI patterns for `CredReadW`/`CredWrite`/`CredFree`. Each pointer operation has a comment explaining its purpose.
- Recommendations: Add integration tests that exercise the full read/write/delete cycle on Windows. Consider wrapping the unsafe operations in a separate file with build tags.

**Bash tool allows arbitrary command execution:**
- Risk: The Bash tool (`internal/tools/bash.go`) executes arbitrary shell commands. The permission system gates this, but if permission rules are misconfigured, dangerous commands could run without user approval.
- Files: `internal/tools/bash.go`, `internal/tools/permissions.go`
- Current mitigation: Default risk level is `RiskDangerous`, requiring permission modal approval. Rate limiting prevents resource exhaustion (WP-S04).
- Recommendations: No change needed — the permission gate is the correct defense. Ensure documentation emphasizes that `allow` rules for Bash should be used sparingly.

**API key resolution order is secure:**
- Risk: Low. API keys are resolved in order: env var → OS keychain → config file. The config file is the last resort fallback.
- Files: `internal/config/loader.go`, `pkg/keychain/`
- Current mitigation: Env vars and keychain are preferred. Config file stores plaintext only as fallback.
- Recommendations: None — the resolution order is correct per AGENTS.md requirements.

**No hardcoded secrets detected:**
- Risk: None. No API keys, passwords, or tokens found hardcoded in source files.
- Files: N/A
- Current mitigation: Secrets are resolved from env vars, keychain, or config files at runtime.
- Recommendations: Maintain this pattern. Add a CI check to prevent accidental secret commits.

---

## Performance Bottlenecks

**`AppState.Update()` is a 1703-line function with 177 `if err != nil` checks:**
- Problem: The main Bubble Tea update function in `internal/tui/app_update.go` is the single largest file at 1703 lines. Every message type is handled in one massive switch statement. This makes the function hard to navigate and increases the chance of subtle bugs when adding new message types.
- Files: `internal/tui/app_update.go`
- Cause: Bubble Tea's architecture requires a single `Update()` function, but the sheer number of message types (streaming, health, permissions, questions, workflow phases, tool results, toasts, etc.) has grown organically.
- Improvement path: Extract per-domain message handlers into separate methods (some already exist like `handleStreamMsg`, `handlePermissionResponse`). The remaining cases can be grouped into domain-specific handler methods to reduce the file size and improve readability.

**`handleWindowResize` propagates dimensions to 15+ sub-models:**
- Problem: On every terminal resize event, the function at `internal/tui/app_update.go:720-800` calls `SetDimensions` or `SetContentWidth` on 15+ sub-models sequentially. Each call is O(1) but the cumulative effect on every keystroke (which may trigger a resize in some terminal emulators) is non-trivial.
- Files: `internal/tui/app_update.go:720-800`
- Cause: No batching or debouncing of resize events. Every resize immediately propagates to all sub-models.
- Improvement path: Debounce resize events (100ms delay) to avoid redundant propagations during rapid resizing.

**Model cache refresh may block TUI on slow connections:**
- Problem: When the model cache expires (TTL = 5 minutes), the next `FetchModels` call triggers an HTTP request to the provider. If the provider is slow or unreachable, this blocks the cache refresh for up to 30 seconds (HTTP dial timeout).
- Files: `internal/provider/cache.go:46-63`, `internal/provider/openrouter/client.go`
- Cause: The `singleflight.Group` correctly deduplicates concurrent requests, but the initial fetch can still block the first caller.
- Improvement path: Cache refresh already happens in a goroutine (`internal/tui/cache_refresh.go`). The concern is the initial load on app startup — ensure it doesn't block the TUI from rendering the first frame.

**Permission modal timeout uses `time.After` in select:**
- Problem: In `internal/tools/permissions.go:233`, the permission wait uses `timeoutCtx.Done()` which is backed by `context.WithTimeout`. This is correct, but the `select` also has a `default` case at line 222-224 that immediately returns `ErrPermissionDenied` if the request channel is full, without retrying.
- Files: `internal/tools/permissions.go:221-225`
- Cause: The non-blocking send is intentional to prevent deadlock, but it means a full request channel silently denies permission rather than queuing.
- Improvement path: Consider a buffered channel with a larger capacity, or implement a retry loop with backoff for the request send.

---

## Fragile Areas

**`AppState` struct has 50+ fields — "God Object" risk:**
- Files: `internal/tui/app_state.go:49-180`
- Why fragile: The `AppState` struct holds references to 20+ sub-models, workflow state, config, provider registry, session manager, git client, and more. Adding a new feature almost always requires adding a new field here and threading it through `NewApp()`.
- Safe modification: Always add new state as a new sub-model pointer (optional, nil-checked) rather than adding fields directly to AppState. Follow the existing pattern of `if m.newModel != nil { ... }` null checks.
- Test coverage: `internal/tui/` has 0.0% test coverage (no test files exist for the main TUI package).

**`Update()` message routing has implicit ordering dependencies:**
- Files: `internal/tui/app_update.go`
- Why fragile: Some message types (like `TickMsg`) are forwarded to multiple sub-models conditionally. The order of `case` statements doesn't matter in a switch, but the conditional forwarding logic (`if m.screen == ScreenExecute && m.executeModel != nil`) creates implicit coupling between screen state and message routing.
- Safe modification: When adding new message types, follow the existing pattern of checking both screen state and nil sub-model before forwarding. Never assume a sub-model is non-nil.
- Test coverage: Zero automated tests for the TUI update loop.

**Permission system has three permission sources that can conflict:**
- Files: `internal/tools/permissions.go:65-107`
- Why fragile: Permission decisions come from three sources: explicit rules (`checkPermission`), agent defaults (`agents[activeAgent].DefaultAction`), and risk-level fallback. The precedence is: rules → agent defaults → risk-level fallback. If a rule matches with `ask`, the code falls through to the `askPermission` path, but if no rule matches and the agent default is also not set, it falls back to the risk-level check.
- Safe modification: When adding new permission sources, add them at the correct precedence level. Rules always win, agent defaults are second, risk-level is last.
- Test coverage: `internal/tools/permissions_test.go` has tests for rule matching but limited coverage for agent defaults and concurrent permission requests.

**Streaming pipeline goroutine ownership model:**
- Files: `internal/tui/streaming.go`
- Why fragile: The streaming pipeline uses a goroutine that owns a channel, sends `tea.Msg` values, and closes the channel on completion. The TUI reads from the channel in `tea.Cmd` functions. If the goroutine fails to close the channel (e.g., on panic), the TUI will hang waiting for messages.
- Safe modification: Never close the channel from the TUI side. Always rely on the goroutine's `defer close(streamCh)`. If modifying the streaming logic, ensure the goroutine always closes the channel even on panic (use `defer`).
- Test coverage: No automated tests for the streaming pipeline.

---

## Scaling Limits

**Session file size — no hard limit on `messages.json`:**
- Current capacity: The session manager reads `messages.json` with a 5MB limit (`readFileLimited` in `internal/session/manager.go:88`), but writes have no limit. A long conversation with many tool calls could produce a very large file.
- Limit: The 5MB read limit protects against OOM on load, but doesn't prevent writes from growing unbounded.
- Scaling path: Implement periodic message pruning (keep last N messages or messages within token budget). AutoDream consolidation helps but only runs when context exceeds 60%.

**Tool output cap at 50K chars (`BashOutputLimit`):**
- Current capacity: Bash tool output is capped at 50,000 characters per stream.
- Limit: Commands producing more output (e.g., `find / -name "*.go"`) will be silently truncated.
- Scaling path: The truncation is intentional and correct. The TUI renders the cap with a `[... output truncated]` indicator. No change needed.

---

## Dependencies at Risk

**`BurntSushi/toml` v1.6.0 (DEP-3):**
- Risk: In maintenance mode. No new features or bug fixes. v2 has a breaking API change.
- Impact: Config parsing in `internal/config/loader.go` depends on v1 API. Migration requires updating all `toml.Decode`/`toml.Encode` calls.
- Migration plan: Phase 12 or later. v2 migration is tracked as DEP-3 in `go.mod`.

**`tiktoken-go` v0.1.8:**
- Risk: Third-party Go port of OpenAI's tiktoken. May lag behind upstream tokenizer updates when new models are released.
- Impact: Token estimation for GPT/Claude families may be inaccurate for very new models. Fallback (`len(runes) / 4 * 1.3`) exists but is less precise.
- Migration plan: Monitor upstream releases. The char-based fallback provides reasonable accuracy for cost estimation.

**`charmbracelet/bubbletea` v1.3.0:**
- Risk: The TUI framework is actively maintained but breaking API changes between minor versions could require significant refactoring of the 1700+ line Update function.
- Impact: All TUI code depends on Bubble Tea's `tea.Model`, `tea.Cmd`, and `tea.Msg` interfaces.
- Migration plan: Pin current version. Monitor changelog for breaking changes before upgrading.

---

## Test Coverage Gaps

**`internal/tui/` — 0.0% coverage:**
- What's not tested: The entire main TUI package (`app_state.go`, `app_update.go`, `app_view.go`, `repl.go`, `repl_state.go`, `sidebar.go`, and 20+ screen models) has zero test files.
- Files: `internal/tui/*.go` (all files)
- Risk: The TUI is the primary user-facing component. Bugs in message routing, screen transitions, and state management can only be caught by manual testing.
- Priority: High — this is the largest untested surface area.

**`internal/types/` — 0.0% coverage:**
- What's not tested: Core type definitions (Message, Task, ToolCall, etc.) have no tests, though they are mostly data structs with no logic.
- Files: `internal/types/types.go`
- Risk: Low — types are simple data structures. But if any methods are added, they should be tested.
- Priority: Low.

**`internal/fileutil/` — 0.0% coverage:**
- What's not tested: The `AtomicWrite` utility function has no test coverage despite being used by session manager for critical file operations.
- Files: `internal/fileutil/`
- Risk: Medium — atomic write failures could corrupt session data.
- Priority: Medium.

**`pkg/keychain/` — 2.1% coverage:**
- What's not tested: Only the `New` constructor is tested. The `Get`, `Set`, `Delete` methods for Linux (D-Bus Secret Service / `pass` CLI), macOS (Keychain), and Windows (Credential Manager) are untested.
- Files: `pkg/keychain/keychain_linux.go`, `pkg/keychain/keychain_darwin.go`, `pkg/keychain/keychain_windows.go`
- Risk: Medium — keychain operations fail silently in CI/headless environments, making testing difficult. But production failures could prevent API key retrieval.
- Priority: Medium — add integration tests with mock D-Bus/keychain backends.

**`internal/git/` — 30.8% coverage:**
- What's not tested: Git operations (commit, diff, log, branch, bisect integration) are partially tested but many error paths and edge cases are uncovered.
- Files: `internal/git/git.go`
- Risk: Medium — git operations are critical for the workflow engine's commit and rollback features.
- Priority: Medium.

---

## Error Handling Gaps

**Dispatcher Execute returns error AND sets ToolResult.Error:**
- Issue: In `internal/tools/dispatcher.go:216-219`, when a tool returns an error, the dispatcher both sets `res.Error = err.Error()` AND returns `fmt.Errorf("tool %s: %w", call.Name, err)`. This means the caller receives both a `ToolResult` with an error string AND a Go error. Callers must handle both, which is inconsistent.
- Files: `internal/tools/dispatcher.go:216-219`
- Risk: Low — callers currently handle both paths correctly. But it's a confusing API contract.

**TUI error handling mostly logs and continues:**
- Issue: In `internal/tui/`, most error paths log a warning and continue execution. For example, `internal/tui/app_state.go:271-273` logs "failed to register provider" and shows a toast but continues to the REPL without a valid provider.
- Files: `internal/tui/app_state.go:270-273`, `internal/tui/repl_state.go:24-28`
- Risk: Low — the TUI is designed to be resilient. But silent failures could confuse users who don't notice toast notifications.

**Config validation is comprehensive but does not cover all fields:**
- Issue: `internal/config/loader.go` validates many fields but some newer config fields (e.g., `Tools.WebfetchTimeoutSecs`, `Tools.WebfetchMaxBodyBytes`) may not have validation rules.
- Files: `internal/config/loader.go:380-505`
- Risk: Low — invalid values fall back to defaults. But some fields (like negative timeouts) could cause unexpected behavior.
- Recommendations: Add validation for all numeric config fields to ensure they are non-negative and within reasonable bounds.

---

## Concurrency Issues

**Permission response dropped silently on full channel:**
- Issue: In `internal/tools/permissions.go:22-24`, if the per-request channel is full, the permission response is silently dropped with a warning log. The caller blocks forever waiting for a response that will never arrive (until timeout).
- Files: `internal/tools/permissions.go:20-34`
- Risk: Medium — if the TUI is overwhelmed and can't process permission requests fast enough, tool execution will hang until the permission timeout (default 300s).
- Improvement path: Consider increasing the channel buffer size or implementing a retry mechanism for permission response delivery.

**Rate limiter goroutine runs forever without Stop:**
- Issue: The rate limiter goroutine in `internal/tools/dispatcher.go:61-73` runs in an infinite loop until `rateDone` is closed. If `Dispatcher.Stop()` is never called (e.g., on abnormal shutdown), the goroutine leaks.
- Files: `internal/tools/dispatcher.go:61-73, 260-270`
- Risk: Low — the goroutine is lightweight and the app process will terminate it. But it's a minor resource leak on graceful shutdown paths.
- Improvement path: Ensure `Dispatcher.Stop()` is called in the app's shutdown sequence.

**`channelEmitter.Emit` can drop messages:**
- Issue: In `internal/tui/app_channel.go:21-27`, workflow messages are sent to a buffered channel with a timeout. If the channel is full, messages are dropped with a warning log.
- Files: `internal/tui/app_channel.go:21-27`
- Risk: Low — the TUI update loop drains the channel frequently. But during heavy tool execution, workflow progress messages could be lost.
- Improvement path: Increase the channel buffer size or implement a priority queue for critical messages.

---

## Missing Documentation

**No API documentation for exported types in `internal/tools/interface.go`:**
- Issue: The `PermissionRequest`, `PermissionResponse`, and `PermissionContext` types lack godoc comments explaining their fields and usage.
- Files: `internal/tools/interface.go`
- Risk: Low — these types are internal and used only by the dispatcher and TUI.
- Recommendations: Add godoc comments to exported types for maintainability.

**No documentation for the permission rule matching algorithm:**
- Issue: The `checkPermission` function in `internal/tools/permissions.go:65-107` implements a three-tier precedence model (rules → agent defaults → risk-level fallback) but this is not documented.
- Files: `internal/tools/permissions.go`
- Risk: Low — the behavior is tested. But new contributors may not understand the precedence model.
- Recommendations: Add a comment block explaining the permission decision tree.

---

*Concerns audit: 2026-06-11*

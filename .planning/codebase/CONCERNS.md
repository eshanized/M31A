# Codebase Concerns

**Analysis Date:** 2026-06-07

## Tech Debt

**Duplicate Constants Across Packages:**
- Issue: `internal/types/constants.go` (lines 5-7) intentionally duplicates constant values from `internal/tools/constants.go` to avoid import cycles. Any value change must be manually synced across both files.
- Files: `internal/types/constants.go`, `internal/tools/constants.go`
- Impact: Silent drift if one file is updated without the other; no compile-time enforcement.
- Fix approach: Extract shared constants to a leaf package (e.g., `internal/consts/`) that both can import, or use code generation.

**Architecture Violation CR-09 — `internal/tools` imports `internal/config`:**
- Issue: Per `docs/ARCHITECTURE.md`, `internal/tools/` should only import `internal/types/` and `internal/errors/`. However, three production files and three test files import `internal/config`:
  - `internal/tools/dispatcher.go` (line 13)
  - `internal/tools/permissions.go` (line 11)
  - `internal/tools/defaults.go` (line 3)
- Files: `internal/tools/dispatcher.go`, `internal/tools/permissions.go`, `internal/tools/defaults.go`
- Impact: Violates the stated dependency rules; makes it impossible to independently test or replace the tools package without pulling in config.
- Fix approach: Move `PermissionRule` type to `internal/types/types.go` and update all import paths. This is a deferred fix noted in `docs/ARCHITECTURE.md` as "Phase 26+".

**Architecture Violation W-26 — `internal/tui/components` imports `internal/tools`:**
- Issue: `internal/tui/components/permission.go` (line 9) and `internal/tui/components/question.go` (line 10) import `internal/tools` for risk level metadata. This creates a TUI→Tools dependency.
- Files: `internal/tui/components/permission.go`, `internal/tui/components/question.go`
- Impact: Low — the import is read-only (risk level lookup), not circular. But it couples TUI rendering to the tools package.
- Fix approach: Extract risk level metadata into `internal/types/` to eliminate the dependency.

**Duplicate Backup Methods:**
- Issue: `internal/tui/backup.go` contains both `backupCurrentSession()` (line 20) and `backupCurrentSessionAsync()` (line 98). The sync version was the original, then the async version was added for BUG-02 fix. Both `copyDir` calls are identical.
- Files: `internal/tui/backup.go` (lines 20-57, 95-132)
- Impact: Dead code accumulation; the original `backupCurrentSession()` is still present but only `backupCurrentSessionAsync()` is called in production (`app_update_workflow.go:269`).
- Fix approach: Remove `backupCurrentSession()` entirely since it's unused and the async version is the correct pattern.

**AppState Struct Bloat:**
- Issue: `AppState` in `internal/tui/app_state.go` (lines 55-154) has 40+ fields mixing screen state, workflow state, session state, config, UI preferences, and sidebar state. The struct is approximately 100 lines of field declarations.
- Files: `internal/tui/app_state.go`
- Impact: High cognitive load; every handler in `app_update.go`, `app_update_workflow.go`, `app_update_screen.go`, `app_update_slash.go` (2,581 lines total across these files) accesses AppState fields directly, making it hard to reason about state ownership.
- Fix approach: Group related fields into sub-structs (e.g., `WorkflowState`, `SessionState`, `UIState`) and access via composition.

**Reasoning Config Hardcoded:**
- Issue: `internal/provider/reasoning.go` (lines 19-45) defines `reasoningParamMap` as a static package-level map of model family prefixes to config. New model families require code changes.
- Files: `internal/provider/reasoning.go`
- Impact: Cannot add support for new reasoning model families without modifying code. The map is iterated on every request (sorted by key length).
- Fix approach: Make reasoning config loadable from config file or extend the model cache with per-model reasoning params from provider API.

## Known Bugs

**Uncommitted Changes Break Compilation:**
- Symptoms: `go build ./...` and `go vet ./...` fail with errors in `internal/tui/app_update_workflow.go`. The committed code (after `git stash`) compiles and all tests pass.
- Files: `internal/tui/app_update_workflow.go`, `internal/provider/registry.go`, `internal/tui/app_update_slash.go`, `internal/tui/app_view.go`, `internal/tui/cmdpalette.go`, `internal/tui/repl_view.go`, `internal/tui/repl_welcome.go`
- Trigger: 14 files have uncommitted modifications that introduce compilation errors (duplicate `handleSettingsSaved` method, references to `m.settings` field that doesn't exist, undefined `tools` import, non-existent `ListAll` method call).
- Workaround: `git stash` restores the working tree to a compilable state. All tests pass on the committed codebase.

**BUG-08 — PlanningDir Calculation:**
- Symptoms: Session-switching commands (`/fork`, `/prev`, `/next`) could miscalculate the planning directory path.
- Files: `internal/workflow/engine.go` (lines 65, 130-132, 287-288)
- Trigger: Using relative `..` navigation from planningDir to reach sessions root was fragile.
- Workaround: Fixed by storing `sessionsRoot` on Engine and deriving `planningDir` from it. Fix is in committed code.

**BUG-02 — AppState Mutation from Goroutine:**
- Symptoms: `backupCurrentSession()` mutated AppState fields from within a goroutine context, causing potential race conditions.
- Files: `internal/tui/backup.go` (line 56), `internal/tui/app_update_workflow.go` (line 266)
- Trigger: Called during Ship phase completion.
- Workaround: Fixed by adding `backupCurrentSessionAsync()` that returns the path instead of mutating state. The caller emits a `ToastMsg` via `tea.Cmd`.

## Security Considerations

**WebFetch SSRF Protection — Complex Security-Critical Code:**
- Risk: `internal/tools/webfetch.go` (635 lines) contains the entire SSRF protection system: DNS resolution with `sync.Map` cache, `isPrivateIP` checks (loopback, link-local, RFC1918, IPv6 ULA, cloud metadata), TOCTOU rebinding prevention, redirect checking, and HTML parsing. This is security-critical code concentrated in a single file.
- Files: `internal/tools/webfetch.go` (lines 46-250)
- Current mitigation: Dual DNS check (pre-connect and post-connect), 5-minute DNS cache TTL, max redirect limit (5), private IP blocking on all resolved addresses. Well-documented with inline comments.
- Recommendations: Extract `isPrivateIP` and DNS cache into a dedicated `internal/netutil/` package for independent testing. Add fuzz tests for `isPrivateIP` edge cases. Consider making `allowPrivateIPs` configurable per-user (not just per-request).

**API Key Exposure in Config File:**
- Risk: API keys can be stored in `~/.m31a/config.toml` as plaintext when keychain is unavailable.
- Files: `internal/config/loader.go`, `pkg/keychain/` (all files)
- Current mitigation: Resolution order is env var → OS keychain → config file. Config file is the last resort fallback.
- Recommendations: Log a warning when API keys are stored in the config file. Consider adding a `m31a keychain setup` prompt on first run.

**Bash Tool Command Injection:**
- Risk: The Bash tool (`internal/tools/bash.go`) executes arbitrary shell commands with no allowlist or sandboxing.
- Files: `internal/tools/bash.go` (line 84: `cmd := newShellCmd(ctx, command)`)
- Current mitigation: Permission gate (risk level: `dangerous`) requires user approval before execution. 30-minute timeout. Output capped at 50K chars.
- Recommendations: Consider adding a configurable command allowlist/denylist in config.toml for non-interactive environments.

## Performance Bottlenecks

**AppState Update() Switch Cascade:**
- Problem: `AppState.Update()` in `internal/tui/app_update.go` (518 lines) routes messages through nested `switch` statements across multiple files totaling 2,581 lines. Every keystroke, timer tick, and stream chunk traverses this entire routing tree.
- Files: `internal/tui/app_update.go`, `internal/tui/app_update_workflow.go`, `internal/tui/app_update_screen.go`, `internal/tui/app_update_slash.go`
- Cause: The Bubble Tea architecture requires a single `Update()` entry point. All message types are dispatched through the root `AppState.Update()`.
- Improvement path: The current approach is correct for Bubble Tea but should be profiled if frame rate drops below 16ms. The header cache (`headerCacheKey`/`headerCacheValue` in `app_state.go:124-126`) mitigates expensive re-renders on timer ticks.

**WebFetch HTML Parsing:**
- Problem: `htmlToMarkdown()` and `htmlToText()` in `internal/tools/webfetch.go` (lines 400-498) use recursive string replacement via `strings.Index` in loops. For large HTML responses, this is O(n²) due to repeated `strings.ToLower` and string slicing.
- Files: `internal/tools/webfetch.go` (lines 400-498)
- Cause: Manual HTML parsing instead of using a proper HTML parser library.
- Improvement path: For large responses (>100KB), consider using `golang.org/x/net/html` tokenizer. For V1, the 1MB `MaxLLMResponseBytes` cap provides some protection.

**Model Cache Refresh Under Lock:**
- Problem: `internal/provider/cache.go` uses `sync.RWMutex` for the model cache. During a refresh triggered by TTL expiry, the write lock blocks all reads until the network fetch completes (up to 15 seconds for `FetchModelsTimeout`).
- Files: `internal/provider/cache.go` (line 18)
- Cause: The refresh operation holds the write lock for the duration of the HTTP call.
- Improvement path: Use `atomic.Bool` for the `refreshing` flag (already present) and fetch outside the lock, then swap the cache under a short write lock. The existing code partially does this with the `refreshing` atomic, but the full refresh path needs verification.

## Fragile Areas

**Workflow Engine (`internal/workflow/engine.go` + `engine_parse.go`):**
- Files: `internal/workflow/engine.go` (546 lines), `internal/workflow/engine_parse.go` (591 lines)
- Why fragile: The engine orchestrates six workflow phases, each with context pruning, LLM streaming, tool execution, git operations, and file persistence. The `engine_parse.go` file handles JSON extraction from LLM responses using regex-based code block detection and bracket-depth tracking — inherently fragile against non-standard LLM output formats.
- Safe modification: Always test with at least two provider types (OpenRouter and Zen). Mock the LLM provider for unit tests. The `workflowEngineInterface` in `app_state.go:41-53` enables mock injection.
- Test coverage: 68.7% (decent but not comprehensive for the critical path).

**Streaming Pipeline (`internal/tui/streaming.go`):**
- Files: `internal/tui/streaming.go` (179 lines)
- Why fragile: Manages goroutine-to-Bubble Tea communication via channels. Historical bugs C-3 (double-close), M-21 (channel ownership), M-35-36 (race conditions) are documented inline. The goroutine ownership model is correct but easy to violate.
- Safe modification: Never close `streamCh` from outside the goroutine. Never mutate `AppState` from the streaming goroutine. Always return `tea.Msg` via the channel. The file's header comment (lines 1-23) is the authoritative guide.
- Test coverage: Part of `internal/tui` at 49.1%.

**TUI `AppState.Update()` Message Routing:**
- Files: `internal/tui/app_update.go` (518 lines), `internal/tui/app_update_workflow.go` (660 lines)
- Why fragile: The massive switch cascade handles 30+ message types. Adding a new message type requires touching the correct file and case branch. The duplicate `handleSettingsSaved` bug (introduced in uncommitted changes) demonstrates how easy it is to create conflicts when multiple files in the same package define methods on the same type.
- Safe modification: Each new message handler should be in its own file (`app_update_<feature>.go`). Never define the same method name on `AppState` in two different files.
- Test coverage: 49.1% (significant gap in Update routing logic).

## Scaling Limits

**Session Manager — Unbounded Disk Growth:**
- Current capacity: Sessions accumulate in `~/.m31a/sessions/` with no automatic cleanup by default.
- Limit: Each session contains `session.json`, `messages.json`, planning files, and backups. Heavy usage can produce hundreds of MBs.
- Scaling path: `Manager.Cleanup(maxAge)` in `pkg/session/manager.go:678` is implemented and called on startup with `SessionRetentionDays` (default 30). Backups in `~/.m31a/backups/` have `MaxBackupsPerFile` (default 10). This is adequate for V1.

**Token Estimation Accuracy:**
- Current capacity: `tiktoken-go` for GPT/Claude families, char-based fallback for others.
- Limit: The EMA correction (alpha=0.3) needs ~3 turns to converge. Early estimates may be off by 20-30%.
- Scaling path: `internal/tokens/` handles calibration with 90% test coverage. The fallback `len(runes) / 4 * 1.3` is intentionally conservative.

## Dependencies at Risk

**charmbracelet/bubbletea:**
- Risk: Bubble Tea is a single-threaded event loop framework. All state mutations must go through `Update()`. The project has 40+ message types routed through `AppState.Update()`.
- Impact: If Bubble Tea's API changes, the entire TUI layer needs updating. The framework is actively maintained but pre-1.0.
- Migration plan: No immediate need. Bubble Tea is the dominant Go TUI framework. Monitor for v1.0 breaking changes.

**charmbracelet/glamour:**
- Risk: Used for Markdown rendering in tool cards and thinking blocks. Glamour's rendering output varies between versions.
- Impact: Visual regressions in tool output display.
- Migration plan: Pin glamour version in `go.mod`. Custom dark/light stylesheets in `internal/tui/theme/` mitigate default style changes.

**tiktoken-go:**
- Risk: Third-party Go port of OpenAI's tiktoken tokenizer. May lag behind upstream tokenizer updates.
- Impact: Token count estimation could be inaccurate for new model families.
- Migration plan: The char-based fallback (`len(runes) / 4 * 1.3`) provides safety. Monitor upstream for new model tokenizer support.

## Test Coverage Gaps

**`internal/tui/` — 49.1% coverage (Critical):**
- What's not tested: Most `Update()` message handlers, screen routing logic, workflow phase transitions in the TUI, sidebar rendering, command palette interactions.
- Files: `internal/tui/app_update.go`, `internal/tui/app_update_workflow.go`, `internal/tui/app_update_screen.go`
- Risk: Regressions in TUI state management go unnoticed. The `go test -race` passes but coverage of edge cases is thin.
- Priority: **High** — the TUI is the user-facing surface; message routing bugs cause visible issues.

**`internal/tui/components/` — 39.5% coverage (Moderate):**
- What's not tested: Component rendering logic, badge formatting, sparkline calculations, message bubble layout.
- Files: `internal/tui/components/permission.go`, `internal/tui/components/question.go`, `internal/tui/components/message.go`
- Risk: Visual regressions in UI components.
- Priority: **Medium** — components are leaf nodes; failures are visible but not catastrophic.

**`pkg/keychain/` — 4.0% coverage (Low):**
- What's not tested: Linux secret-service integration, macOS Keychain, Windows Credential Manager — only the test mock is covered.
- Files: `pkg/keychain/` (all files)
- Risk: API key storage/retrieval failures on specific OS platforms.
- Priority: **Low** — keychain failures are non-fatal (fallback to config file). Cross-platform testing requires CI matrix.

**`internal/provider/` — 54.7% coverage (Moderate):**
- What's not tested: SSE parser edge cases, reasoning normalization for interleaved thinking, health check retry logic, model cache refresh race conditions.
- Files: `internal/provider/sse.go`, `internal/provider/reasoning.go`, `internal/provider/cache.go`
- Risk: Streaming failures with non-standard provider responses. The SSE parser is the most critical untested path.
- Priority: **High** — streaming is the primary LLM communication channel.

**`internal/types/` — 0% coverage:**
- What's not tested: No test file exists; the package contains only type definitions and constants.
- Files: `internal/types/types.go`, `internal/types/constants.go`
- Risk: None — types are compile-time checked. This is expected.
- Priority: **None** — no logic to test.

**`cmd/m31a/` — 0% coverage:**
- What's not tested: Binary entry point, CLI flag parsing, version output.
- Files: `cmd/m31a/main.go`
- Risk: Low — entry point is typically thin glue code.
- Priority: **Low** — manual smoke testing is sufficient.

## Fragile Code Patterns

**Error Handling by String Substring Match:**
- Issue: `internal/errors/errors.go` (lines 104-119) uses `strings.Contains(errStr, "401")`, `strings.Contains(errStr, "429")` as a fallback for unrecognized errors. This is brittle — a provider error message containing "429" in a non-HTTP-status context would incorrectly match.
- Files: `internal/errors/errors.go` (lines 104-119)
- Fix approach: Prefer typed error checking via `errors.Is()` and `errors.As()`. The string fallback should be removed or made more specific (e.g., match "HTTP 429" or "status 429" patterns).

**`context.Background()` in Production Workflow Code:**
- Issue: `internal/workflow/engine_verify.go` uses `context.Background()` for 7 separate `context.WithTimeout` calls (lines 140-226). `engine.go` line 321 also uses `context.Background()` for `healTask()`. These bypass any parent context cancellation, meaning workflow shutdown won't cancel in-flight verify/heal operations.
- Files: `internal/workflow/engine_verify.go`, `internal/workflow/engine.go`
- Fix approach: Thread the workflow context from `AppState.workflowCtx` through to `RunPhase()` and down to verify/heal calls. This requires passing the context through the `workflowEngineInterface`.

**`_ = err` Error Swallowing:**
- Issue: `internal/tui/app.go` line 309 swallows error from `sessionManager.AddRecentModel()` with `//nolint:errcheck`. This is the only `nolint` directive in the codebase (aside from the `gochecknoglobals` in `webfetch.go`).
- Files: `internal/tui/app.go` (line 309)
- Fix approach: Log the error at Debug level instead of silently swallowing it.

## Architectural Constraints at Risk

**Bubble Tea Single-Threaded Model:**
- Constraint: All state mutations MUST go through `Update()`. Goroutines may only emit `tea.Msg` via channels.
- Risk area: The streaming pipeline (`internal/tui/streaming.go`) and health check ticker (`internal/tui/health.go`) spawn goroutines. Any accidental `AppState` mutation from these goroutines would cause data races.
- Current protection: `go test -race` passes. The streaming pipeline has extensive documentation (lines 1-23 of `streaming.go`). Health check uses `atomic.Value` for cross-goroutine reads.
- Recommendation: Add a race-detection CI step that runs `go test -race ./internal/tui/...` specifically, as this package is the highest-risk for race conditions.

**No CGO Constraint:**
- Constraint: `CGO_ENABLED=0` for static binary.
- Risk area: `creack/pty` dependency for Bash tool uses OS-specific syscalls. `pkg/keychain/` uses platform-specific secret storage.
- Current protection: Build tags separate platform-specific code. Tests pass with CGO disabled.
- Recommendation: Verify `CGO_ENABLED=0` in CI build matrix (referenced in `adrenaline/ROADMAP.md` Phase 0).

---

*Concerns audit: 2026-06-07*

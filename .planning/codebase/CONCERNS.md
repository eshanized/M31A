# Codebase Concerns

**Analysis Date:** 2026-06-04

## Tech Debt

**`closeOnces` sync.Map unbounded growth:**
- Issue: The global `var closeOnces sync.Map` in `internal/tui/app.go:462` stores `*sync.Once` entries for every channel closed via `safeCloseOnce()`. Entries are never removed. Each workflow phase transition creates new channels and adds entries.
- Files: `internal/tui/app.go:462-476`
- Impact: Over a long session with many phase transitions, this map grows without bound. In practice the leak is small per-session (a few entries per phase), but it is architecturally incorrect.
- Fix approach: Either use a per-`AppState` map that is cleaned up on session end, or switch to a different close-guard pattern that does not require global bookkeeping (e.g., store the `sync.Once` inside the channel wrapper struct).

**`Stream: false` in streaming LLM calls:**
- Issue: Both `streamLLM()` (`internal/workflow/engine.go:517`) and `streamLLMStreaming()` (`internal/workflow/engine.go:546`) set `Stream: false` in the `ChatRequest`, but then call `provider.ChatCompletionStream()` which sends `stream: true` in the HTTP body. The request's `Stream` field is ignored by the provider clients (they always hardcode `"stream": true` in the JSON body), making the field misleading.
- Files: `internal/workflow/engine.go:514-519`, `internal/workflow/engine.go:543-548`
- Impact: Dead code / confusion. The `Stream` field on `ChatRequest` is effectively unused.
- Fix approach: Either use `req.Stream` in the provider clients' JSON construction, or remove the `Stream` field from `ChatRequest` since it is always overridden.

**Hardcoded model capability map:**
- Issue: `openrouterModelCapabilities` in `internal/provider/openrouter/client.go:48-60` is a hardcoded map of ~11 model IDs to capability flags. Models not in this map fall back to heuristic string-sniffing of the tokenizer/modality fields.
- Files: `internal/provider/openrouter/client.go:48-60`, `internal/provider/openrouter/client.go:186-195`
- Impact: New models are not correctly classified until the code is updated. The heuristic fallback (checking for "tools" in tokenizer string or "r1" in ID) is fragile.
- Fix approach: Prefer capabilities from the provider API response if available, or move the map to a config file that can be updated without recompilation.

**Hardcoded `normalizeToolName` only covers 5 tools:**
- Issue: `normalizeToolName()` in `internal/workflow/engine.go:975-991` maps LLM aliases to registered tool names for Bash, FileRead, FileWrite, Glob, and Grep only. The Edit and WebFetch tools have no aliases.
- Files: `internal/workflow/engine.go:974-991`
- Impact: If an LLM emits `"edit"` or `"web_fetch"` as a tool name, it will not be routed to the correct tool implementation.
- Fix approach: Add aliases for Edit (`"edit"`, `"search_replace"`) and WebFetch (`"web_fetch"`, `"fetch"`, `"http_get"`).

**Deprecated `safeClose` alias still present:**
- Issue: `safeClose()` in `internal/tui/app.go:478-481` is marked as deprecated but still called from `RunPhaseCmd()` at line 356. This creates inconsistency — some code uses `safeClose`, some uses `safeCloseOnce`.
- Files: `internal/tui/app.go:356`, `internal/tui/app.go:478-481`
- Impact: Code confusion; the deprecation is misleading since it is still actively used.
- Fix approach: Migrate all callers to `safeCloseOnce` and remove the `safeClose` alias.

**`mergeConfig` is field-by-field manual merge:**
- Issue: `mergeConfig()` in `internal/config/loader.go:118-199+` manually checks and copies every config field. Adding a new config field requires updating this function. This is error-prone and tedious.
- Files: `internal/config/loader.go:118-199+` (continues for ~150 lines)
- Impact: High maintenance cost; new config fields can silently be ignored by project-level configs.
- Fix approach: Use reflection-based merge (check non-zero values), or switch to a layered config library, or generate the merge function.

## Known Bugs

**Verify fallback uses `HEAD~50` which may not exist:**
- Symptoms: When `sessionStartHash` is empty (e.g., git operations failed during Initialize), `verify.go:63` falls back to `"HEAD~50"` as the bisect "good" commit. If the repository has fewer than 50 commits, this fails silently and bisect produces incorrect results.
- Files: `internal/workflow/verify.go:59-64`
- Trigger: Session starts in a shallow repository or a repository with fewer than 50 commits, AND verification fails after max heal attempts.
- Workaround: Ensure `sessionStartHash` is always captured in Initialize phase.

**Discuss timeout not configurable:**
- Symptoms: The discuss Q&A timeout is hardcoded to 5 minutes (`internal/tui/app_workflow.go:148-161`). This is not exposed via config.
- Files: `internal/tui/app_workflow.go:147-161`, `internal/tui/app_update.go:705-711`
- Trigger: User takes more than 5 minutes to answer a discuss question. The timeout fires and auto-skips remaining questions.
- Workaround: None — the user loses the chance to answer.

**Permission timeout default mismatch risk:**
- Symptoms: `Dispatcher` defaults `permissionTimeout` to 300 seconds (`internal/tools/dispatcher.go:45`), while the TUI uses `components.DefaultPermissionTimeout` for the modal timer. If these values diverge, the modal could auto-deny before the dispatcher's timeout fires.
- Files: `internal/tools/dispatcher.go:45`, `internal/tui/app_update.go:630-632`
- Trigger: If `config.Permissions.TimeoutSeconds` is not set and the components package default differs from 300.
- Workaround: Ensure both defaults match.

**Git operations ignore errors silently in workflow phases:**
- Symptoms: In `internal/workflow/ship.go:62` and `internal/workflow/execute.go:247`, errors from `git.HeadHash()` and `git.Log()` are silently discarded with `_ =`.
- Files: `internal/workflow/ship.go:62`, `internal/workflow/execute.go:247`, `internal/workflow/execute.go:344`
- Trigger: Git operations fail (e.g., detached HEAD, empty repository).
- Workaround: None — missing commit hashes or log entries silently degrade the Ship summary.

## Security Considerations

**SSRF protection TOCTOU gap in WebFetch:**
- Risk: `WebFetch.DialContext` checks only the first resolved IP address (`internal/tools/webfetch.go:49`) against the private IP filter, but `resolveAndCheck()` checks all resolved addresses. An attacker could use DNS rebinding: the first resolution returns a public IP (passes the DialContext check), but subsequent connections resolve to a private IP.
- Files: `internal/tools/webfetch.go:49`, `internal/tools/webfetch.go:160-165`
- Current mitigation: The DialContext does a "paranoid check" after connecting (line 64-69), but this only catches cases where the OS resolver returns a different IP than the Go resolver.
- Recommendations: Use a DNS cache that pins the resolved IP for the entire request lifecycle, or check all resolved addresses in `DialContext` before connecting.

**`os.Exit(1)` in library code:**
- Risk: `NewApp()` in `internal/tui/app.go:206` calls `os.Exit(1)` if the tool dispatcher fails to initialize. This is in library/constructor code, not main().
- Files: `internal/tui/app.go:206`
- Current mitigation: None — the process terminates without cleanup.
- Recommendations: Return an error from `NewApp()` instead. Let `cmd/m31a/main.go` handle the exit.

**API key stored in `AppState.apiKey` field:**
- Risk: The API key is held in the `AppState.apiKey` string field (`internal/tui/app.go:70`). While Go strings are immutable, this field persists in memory for the entire session lifetime and could be exposed via memory dumps or core files.
- Files: `internal/tui/app.go:70`
- Current mitigation: API keys are resolved from env var → keychain → config file, with keychain preferred.
- Recommendations: Consider zeroing the key after use (not possible with Go strings), or keeping it only in the provider clients and not propagating it to AppState.

**Backup directory accumulation:**
- Risk: `FileWrite` creates a backup on every file write (`internal/tools/filewrite.go:113-134`). There is no pruning mechanism. Over a long session with many file edits, the backup directory grows unbounded.
- Files: `internal/tools/filewrite.go:113-134`
- Current mitigation: None.
- Recommendations: Add backup pruning (e.g., keep last N backups per file, or cap total backup size).

## Performance Bottlenecks

**`listCwdFiles` uses `filepath.Walk` (O(n) on large repos):**
- Problem: `listCwdFiles()` in `internal/workflow/engine.go:1155-1190` walks the entire working directory tree. While it skips known heavy directories (`node_modules`, `vendor`, etc.) and limits depth to 3, the initial walk is still O(n) where n is the total file count.
- Files: `internal/workflow/engine.go:1155-1190`
- Cause: `filepath.Walk` traverses all directories before applying skip logic.
- Improvement path: Use `filepath.WalkDir` (cheaper per-entry) or `os.ReadDir` for shallower enumeration. For very large repos, consider caching the file list.

**HTML-to-Markdown conversion uses repeated string operations:**
- Problem: `htmlToMarkdown()` and helper functions in `internal/tools/webfetch.go:286-538` use repeated `strings.Index`, `strings.ReplaceAll`, and `strings.ToLower` operations on the full HTML body. Each pass re-lowers the entire string.
- Files: `internal/tools/webfetch.go:286-538`
- Cause: Naive regex-less HTML parsing with multiple full-body passes.
- Improvement path: Use a single-pass HTML parser (e.g., `golang.org/x/net/html`) for production use, or accept the current approach since WebFetch response bodies are capped at 5MB.

**`extractJSONObject` re-scans from each `{` position:**
- Problem: `parseToolCalls()` in `internal/workflow/engine.go:848-877` scans the content byte-by-byte looking for `{`, then calls `extractJSONObject()` which re-parses the entire remaining string. For large LLM responses with many JSON objects, this is O(n*m) where n is content length and m is the number of objects.
- Files: `internal/workflow/engine.go:848-877`
- Cause: Linear scan + nested extraction without maintaining position.
- Improvement path: The `maxJSONScanBytes` cap (64KB) limits the worst case. This is acceptable for V1.

**`closeOnces` sync.Map lookup on every channel close:**
- Problem: Every call to `safeCloseOnce()` does a `LoadOrStore` on a global `sync.Map`, which involves a hash lookup.
- Files: `internal/tui/app.go:462-476`
- Cause: Using `sync.Map` for a pattern that only needs a simple once-per-channel guarantee.
- Improvement path: Since channels are short-lived and the number is small, use a per-channel approach (store `sync.Once` in the same struct as the channel) instead of a global map.

## Fragile Areas

**Workflow engine (`internal/workflow/engine.go`):**
- Files: `internal/workflow/engine.go` (1357 lines)
- Why fragile: This single file contains the engine struct, all message types, prompt loading, task parsing, JSON extraction, tool call normalization, project detection, question parsing, verification logic, and multiple helper functions. Changes to one area risk regressions in others.
- Safe modification: Extract message types to a separate file (`messages.go`). Extract parsing functions to `parse.go`. Extract verification to a separate verifier.
- Test coverage: Has `engine_test.go`, `execute_test.go`, `plan_test.go`, `verify_test.go`, `ship_test.go`, `initialize_test.go`, `discuss_test.go`, `integration_test.go`, and many focused test files. Coverage is good for the workflow package.

**TUI app_update.go message handler:**
- Files: `internal/tui/app_update.go` (1238 lines)
- Why fragile: The `Update()` method handles 20+ message types in a single giant switch statement. Adding a new message type requires finding the right place in this switch, and changes to one case can subtly affect others through shared state.
- Safe modification: Extract message handlers into separate methods per screen (some already exist like `handleAppMsg`). Consider a handler map pattern.
- Test coverage: Has `app_test.go` and `app_update_nilsafety_test.go`. Coverage focuses on nil-safety and specific message paths.

**Streaming pipeline:**
- Files: `internal/tui/streaming.go`, `internal/tui/repl_stream.go`, `internal/tui/repl.go`
- Why fragile: The streaming pipeline spans three files with complex goroutine/channel interactions. The comments reference multiple historical fixes (C-3, M-21, M-35-36) indicating this area has been error-prone.
- Safe modification: Do not modify channel ownership or goroutine lifecycle without running `go test -race` on the full test suite.
- Test coverage: Has `streaming_test.go`, `segment_concurrency_test.go`. Coverage includes double-close, nil channel, and concurrency tests.

**Permission system:**
- Files: `internal/tools/permissions.go`, `internal/tools/dispatcher.go`
- Why fragile: The permission check flow has multiple code paths: rule-based (allow/deny/ask), agent-based defaults, risk-level fallback, and timeout handling. The interaction between these paths is complex.
- Safe modification: Add unit tests for each permission path before modifying.
- Test coverage: Has `dispatcher_test.go`, `permission_timeout_test.go`. Missing dedicated `permissions_test.go` for glob matching logic.

## Scaling Limits

**Session file accumulation:**
- Current capacity: Unbounded. Each session creates a directory under `~/.m31a/sessions/` with JSON files, planning files, checkpoints, and backups.
- Limit: Disk space. No automatic cleanup of old sessions.
- Scaling path: Add session pruning (e.g., auto-archive sessions older than 30 days).

**Ledger file growth:**
- Current capacity: `config.Ledger.MaxEntries` controls the cap (default varies). Entries are appended as markdown.
- Limit: The ledger re-parses the entire file on load (`ledger.go:74`). With thousands of entries, startup time degrades.
- Scaling path: Binary format or indexed file for large ledgers.

**Provider model cache:**
- Current capacity: ~10,000 models (typical OpenRouter catalog). Cached in memory.
- Limit: Memory usage proportional to model count. Each `ModelInfo` is ~200 bytes.
- Scaling path: Not an issue for V1. Models are evicted after TTL expiry.

## Dependencies at Risk

**`golang.org/x/sync/singleflight`:**
- Risk: Used in `internal/provider/cache.go` for deduplicating model refresh calls. This is a stable `x/` package but adds a dependency.
- Impact: Low — widely used, well-maintained.
- Migration plan: N/A — appropriate usage.

**`github.com/bmatcuk/doublestar/v4`:**
- Risk: Used in `internal/tools/permissions.go` for glob pattern matching. The v4 major version may have breaking changes.
- Impact: Low — used only for permission rule matching.
- Migration plan: Pin to current version; check for updates before major releases.

**`github.com/BurntSushi/toml`:**
- Risk: Used in `internal/config/loader.go` for TOML parsing. The `v1` package is in maintenance mode; `v2` has a different API.
- Impact: Medium — config loading is critical path.
- Migration plan: When upgrading to v2, update `toml.DecodeFile` calls and struct tags.

## Missing Critical Features

**Backup pruning:**
- Problem: FileWrite creates backups on every write with no cleanup. Over a long coding session, the backup directory can grow to hundreds of megabytes.
- Blocks: Long-running sessions and production use.

**Session auto-cleanup:**
- Problem: Old sessions are never removed from disk. Users must manually delete `~/.m31a/sessions/` contents.
- Blocks: Disk space management on long-lived installations.

**Config hot-reload:**
- Problem: Configuration is loaded once at startup. Changes to `~/.m31a/config.toml` require restarting the TUI.
- Blocks: Users who want to adjust settings without losing their session.

## Test Coverage Gaps

**`internal/tui/app_update.go`:**
- What's not tested: The `Update()` message handler (1238 lines) is tested indirectly through `app_test.go` but has no dedicated test file. Many message type handlers are untested.
- Files: `internal/tui/app_update.go`
- Risk: Regressions in message handling go undetected.
- Priority: High

**`internal/tools/permissions.go`:**
- What's not tested: `matchAnyParamValue()`, `matchToolName()`, and the full permission check flow with glob patterns have no dedicated test file.
- Files: `internal/tools/permissions.go`
- Risk: Permission rule matching regressions could allow unauthorized tool execution.
- Priority: High

**Command implementations:**
- What's not tested: `commands_ai.go`, `commands_config.go`, `commands_git.go`, `commands_session.go`, `commands_workflow.go` have no test files.
- Files: `internal/tui/commands_ai.go`, `internal/tui/commands_config.go`, `internal/tui/commands_git.go`, `internal/tui/commands_session.go`, `internal/tui/commands_workflow.go`
- Risk: Slash command regressions go undetected.
- Priority: Medium

**`internal/tui/repl_stream.go`:**
- What's not tested: Stream segment building and message assembly logic.
- Files: `internal/tui/repl_stream.go`
- Risk: Streaming display regressions.
- Priority: Medium

**`internal/tui/repl_quickactions.go`:**
- What's not tested: Quick action handling.
- Files: `internal/tui/repl_quickactions.go`
- Risk: Low — UI convenience feature.

**OS-specific keychain implementations:**
- What's not tested: `keychain_linux.go`, `keychain_darwin.go`, `keychain_windows.go` have compile-tag guards that prevent cross-platform testing. Only `keychain_test.go` (shared logic) is tested.
- Files: `pkg/keychain/keychain_linux.go`, `pkg/keychain/keychain_darwin.go`, `pkg/keychain/keychain_windows.go`
- Risk: OS-specific keychain integration may fail on certain desktop environments.
- Priority: Medium

**Error handling patterns — discarded errors in workflow:**
- What's not tested: Error paths in `ship.go` (git log failure), `execute.go` (git HeadHash failure) are silently discarded. No tests verify behavior when these operations fail.
- Files: `internal/workflow/ship.go:62`, `internal/workflow/execute.go:247,344`
- Risk: Silent data loss in commit tracking and ship summary.
- Priority: Medium

---

*Concerns audit: 2026-06-04*

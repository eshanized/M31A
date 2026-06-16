# Codebase Concerns

**Analysis Date:** 2026-06-17

## Tech Debt

**Global mutable state in codeintel (`goModulePath`):**
- Issue: `goModulePath` is a package-level `var string` with no synchronization — safe only because Go init is single-threaded, but fragile if codeintel is ever used concurrently or from multiple sessions
- Files: `internal/codeintel/graph.go:216`
- Impact: Data race if `Build()` is called from multiple goroutines or if the process runs multiple sessions
- Fix approach: Guard with `sync.Once` or pass module path as parameter to `readGoModModule()`

**Global gitignore cache has no eviction:**
- Issue: The `loadGitignore()` function in `internal/tools/grep.go` caches parsed gitignore patterns in a package-level map with no TTL or eviction
- Files: `internal/tools/grep.go:362-398`
- Impact: Memory grows monotonically across long sessions with many directories; stale patterns if `.gitignore` changes
- Fix approach: Add TTL-based invalidation or bounded LRU cache

**20 `//nolint:errcheck` suppressions in production code:**
- Issue: Numerous `defer resp.Body.Close() //nolint:errcheck` and similar suppressions indicate widespread ignore-on-close patterns
- Files: `internal/provider/openrouter/client.go:102,240`, `internal/provider/zen/client.go:88,187`, `internal/tools/fileread.go:133`, `internal/tools/grep.go:299`, `internal/tools/webfetch.go:360`, `internal/tools/websearch.go:168`, `internal/tui/commands/commands_config.go:140`, `internal/tui/filewatcher.go:49,143`, `internal/workflow/discuss.go:36`, `internal/workflow/engine.go:675,718`, `internal/fileutil/atomic.go:39`, `pkg/ledger/ledger.go:154,431`, `pkg/session/manager.go:107`
- Impact: Close errors silently swallowed; could mask disk full or I/O errors
- Fix approach: Log close errors at debug level instead of suppressing entirely

**FileDelete lacks backup pruning symmetry with FileWrite/Edit:**
- Issue: `FileDelete.pruneBackups()` uses a different pruning strategy (lexicographic sort on `*.deleted.*` prefix) vs `FileWrite.pruneBackups()` (lexicographic sort on file prefix). Both work but have different semantics
- Files: `internal/tools/filedelete.go:139-167`, `internal/tools/filewrite.go:219-249`
- Impact: Inconsistent backup behavior; backup dir can grow unbounded if delete-heavy
- Fix approach: Unify backup pruning into a shared utility function

**`os.Setenv` called after goroutines may be reading `os.Environ()`:**
- Issue: `LoadDotEnv()` in `internal/config/loader.go:1010` calls `os.Setenv`, which is not goroutine-safe. The comment at `cmd/m31a/main.go:66` acknowledges this race window
- Files: `internal/config/loader.go:963-1010`, `cmd/m31a/main.go:66`
- Impact: Potential data race if any goroutine reads `os.Environ()` concurrently with `LoadDotEnv()`; mitigated by calling `LoadDotEnv()` before logger init
- Fix approach: Read env once at startup into a map; stop calling `os.Setenv` after init phase

## Error Handling Weaknesses

### 1. Ignored Errors in Production Code

**Critical: `plan, _ = ParsePlan(planMarkdown)` — silent parse failure:**
- Issue: In `executeTaskWithTools()`, a failed plan parse is silently ignored with `plan, _ = ParsePlan(planMarkdown)`. If the plan markdown is malformed, the engine proceeds with a nil plan, losing context that could improve task execution.
- Files: `internal/workflow/execute.go:423`
- Impact: Task execution proceeds without plan context, potentially producing lower-quality implementations. No error is logged or reported.
- Fix approach: Log a warning on parse failure: `if plan, parseErr = ParsePlan(planMarkdown); parseErr != nil { e.logger.Warn(...) }`

**Critical: `plan, _ = ParsePlan(planMarkdown)` — duplicate ignored call:**
- Issue: Same pattern in `buildExecuteContext()` at line 423, where the plan parse error is discarded. The `ParsePlan` function returns errors for malformed JSON sections that could indicate corrupted session data.
- Files: `internal/workflow/execute.go:423`
- Impact: Corrupted plan data silently ignored during task execution context build.
- Fix approach: Log warning on failure, fall back to raw plan text.

**Medium: `staged, _ := e.git.DiffStaged()` in ship phase:**
- Issue: `DiffStaged()` error is silently ignored when checking for staged changes before commit. If git is in a corrupted state, this could lead to incorrect "no staged changes" determination.
- Files: `internal/workflow/ship.go:98`
- Impact: Ship phase may skip commit or commit incorrectly if git index is corrupted.
- Fix approach: On error, log warning and default to "has staged changes" to err on the side of attempting the commit.

**Medium: `dirty, _ := e.git.HasUncommittedChanges()` in ship phase:**
- Issue: Git porcelain check error is silently ignored. A corrupted `.git/` directory would cause this to return false, skipping dirty-file detection entirely.
- Files: `internal/workflow/ship.go:60`
- Impact: Ship phase may commit unrelated files if dirty detection fails silently.
- Fix approach: On error, assume dirty (safe default).

**Medium: `statusOut, _ := e.git.StatusPorcelain()` in ship phase:**
- Issue: Error ignored when checking for unrelated dirty files. Same silent-failure concern.
- Files: `internal/workflow/ship.go:61`
- Impact: Unrelated dirty files are not logged when git status fails.
- Fix approach: Log warning on error.

**Low: `_ = filepath.WalkDir(workDir, ...)` in countProjectFiles:**
- Issue: WalkDir error is silently discarded. Walk errors from permission issues or symlinks are swallowed.
- Files: `internal/workflow/classify.go:144`
- Impact: File count may be inaccurate when directories are inaccessible; project type classification may be wrong.
- Fix approach: Return 0 and log a warning, or propagate the error.

### 2. Error Wrapping Consistency

**Good: Consistent `fmt.Errorf("operation: %w", err)` pattern:**
- The codebase consistently uses Go 1.13+ error wrapping with `%w` verb throughout `internal/git/git.go` (42 wrapped errors), `pkg/session/manager.go` (55 wrapped errors), `pkg/rollback/rollback.go` (20 wrapped errors), and `pkg/bisect/bisect.go` (13 wrapped errors).
- Pattern: `fmt.Errorf("git %s: %w", ...)` — operation context + wrapped original error.
- This enables `errors.Is()` and `errors.As()` matching in `UserMessage()`.

**Weakness: Inconsistent wrapping in dispatcher:**
- Issue: `Dispatcher.Execute()` wraps tool errors with `fmt.Errorf("tool %s: %w", call.Name, err)` at line 210, but also returns `toolResultError` for permission-rule errors at line 192. The caller must distinguish these two error types.
- Files: `internal/tools/dispatcher.go:190-195,208-211`
- Impact: Callers that check `errors.Is()` on tool errors may miss permission-rule errors that are `toolResultError` type.
- Fix approach: Document the `toolResultError` vs `error` distinction clearly; consider wrapping permission errors with sentinel.

**Weakness: Some errors lack wrapping context:**
- Issue: `fmt.Errorf("unknown phase: %s", phase)` at `engine.go:292` and `fmt.Errorf("no discuss questions to answer")` at `engine.go:526` create new errors without wrapping a sentinel. These cannot be matched with `errors.Is()` in `UserMessage()`.
- Files: `internal/workflow/engine.go:292,526,529`
- Impact: `UserMessage()` falls through to generic string matching for these errors, producing less actionable messages.
- Fix approach: Wrap with `ErrPhaseTransition` or new sentinels.

### 3. Sentinel Error Completeness

**Good: `UserMessage()` covers 20+ sentinel errors with actionable messages:**
- The function at `internal/errors/errors.go:49-137` provides user-friendly text for all defined sentinel errors plus pattern-matched HTTP status codes (401, 429, 503).
- Pattern matching handles wrapped errors via `strings.Contains` for common network/HTTP failures.

**Weakness: Missing sentinels for common failure modes:**
- Issue: No sentinel for "git not initialized" errors. `fmt.Errorf("git not initialized on engine — call SetGit before runInitialize")` at `initialize.go:39` creates an ad-hoc error that gets the generic "An unexpected error occurred" message.
- Files: `internal/workflow/initialize.go:39`, `internal/workflow/verify.go:173`
- Impact: Users see "An unexpected error occurred" instead of actionable guidance when git is not initialized.
- Fix approach: Add `ErrGitNotInitialized = errors.New("git not initialized")` sentinel.

**Weakness: Missing sentinel for "file not found" / "permission denied" from tools:**
- Issue: `FileRead` returns `fmt.Errorf("%w: cannot access %s: %v", m31errors.ErrToolExecution, path, err)` at `fileread.go:131`. This wraps `ErrToolExecution` but the underlying `os.ErrNotExist` or `os.ErrPermission` is not separately matchable.
- Files: `internal/tools/fileread.go:131`
- Impact: `UserMessage()` returns "Tool execution failed — check the error details" instead of more specific guidance.
- Fix approach: Check for `os.IsNotExist(err)` or `os.IsPermission(err)` and return appropriate errors.

**Weakness: `isRetryable()` uses string matching instead of sentinels:**
- Issue: `internal/provider/openrouter/client.go:158-171` checks error strings for "500", "502", "connection reset", etc. instead of using typed errors or sentinels.
- Files: `internal/provider/openrouter/client.go:158-171`
- Impact: Fragile; could match false positives (e.g., "500" in a non-HTTP error message). Zen client likely has the same pattern.
- Fix approach: Create typed HTTP status errors from provider responses; use `errors.Is()` for retry classification.

### 4. Panic Usage

**Good: No panics in production code:**
- Only `panic()` calls are in `pkg/rollback/rollback_test.go:34,37` (test setup code).
- Production code uses `slog.Error()` + graceful degradation instead.

**Good: No `recover()` in production code:**
- The only `recover()` call is in `internal/tools/extra_test.go:1429` (test code).
- Production goroutines rely on proper error propagation rather than panic recovery.

### 5. Resource Cleanup

**Critical: Goroutines without recover() in tool execution:**
- Issue: The tool execution goroutines at `internal/workflow/execute.go:246-286` use `defer wg.Done()` but have no `recover()`. If `e.dispatcher.Execute()` panics (e.g., nil pointer from a buggy tool), the goroutine crashes and the WaitGroup counter decrements incorrectly, causing the main goroutine to deadlock on `wg.Wait()`.
- Files: `internal/workflow/execute.go:246-286`
- Impact: A panic in any tool kills the entire execute phase with no error message; other tools in the same group never execute.
- Fix approach: Add `defer func() { if r := recover(); r != nil { ... } }()` at the top of each goroutine.

**Critical: Agent loop goroutines without recover():**
- Issue: The `AgentLoop()` goroutine at `internal/tui/streaming/agent_loop.go:101-311` and `StartStreamCmd()` goroutine at `internal/tui/streaming/streaming.go:85-220` have no panic recovery. If any tool execution or JSON parsing panics, the goroutine crashes and the channel is never closed, leaving the TUI in a stuck state.
- Files: `internal/tui/streaming/agent_loop.go:101`, `internal/tui/streaming/streaming.go:85`
- Impact: Panic in streaming agent kills the goroutine; TUI hangs waiting on channel that is never closed.
- Fix approach: Add panic recovery at the top of both goroutines; send `AgentErrorMsg{Err: ...}` or `StreamErrorMsg{Err: ...}` on panic.

**Medium: Dispatcher rate-limiter goroutine has no recover():**
- Issue: The background rate-limiter goroutine at `internal/tools/dispatcher.go:67-79` has no panic recovery. If a panic occurs (unlikely but possible from channel operations), the goroutine dies silently and the rate limiter stops functioning.
- Files: `internal/tools/dispatcher.go:67-79`
- Impact: Rate limiter stops working; tool execution becomes unbounded.
- Fix approach: Add `defer func() { if r := recover(); r != nil { slog.Error("rate limiter panic", "error", r) } }()`.

**Medium: Config watcher goroutine has no recover():**
- Issue: The config watcher goroutine at `internal/tui/app.go:426-429` calls `config.WatchConfig()` which runs fsnotify event loops. A panic in the watcher kills hot-reload silently.
- Files: `internal/tui/app.go:426-429`
- Impact: Config hot-reload stops working after panic; user must restart.
- Fix approach: Add panic recovery inside the goroutine.

**Medium: Signal handler goroutine has no recover():**
- Issue: The signal handler goroutine at `cmd/m31a/main.go:270-291` sends `tea.QuitMsg{}` and has a 5-second force-exit fallback. A panic here would prevent the fallback.
- Files: `cmd/m31a/main.go:270-291`
- Impact: Signal handling stops working; Ctrl+C doesn't gracefully shut down.
- Fix approach: Add panic recovery.

**Low: Process cleanup goroutines in bash tool:**
- Issue: Four goroutines in `internal/tools/bash.go:138-203` handle signal forwarding, process wait, and stdout/stderr copying. None have panic recovery. A panic in any of them would leave the bash command hanging.
- Files: `internal/tools/bash.go:138,168,182,191`
- Impact: Bash tool hangs if a goroutine panics; the tool's context cancellation mechanism still works since the main goroutine watches `ctx.Done()`.
- Fix approach: Add panic recovery in each goroutine.

### 6. Graceful Degradation

**Good: Provider fallback with parallel health checks:**
- `FindFallbackProvider()` at `internal/provider/fallback.go:22-96` runs parallel health checks against all candidate providers and returns the first healthy one. This prevents cascading failures when a provider goes down.
- Retry logic with exponential backoff in `ChatCompletionStream()` at `internal/provider/openrouter/client.go:138-155`.

**Good: Self-heal loop with max attempts:**
- The execute phase at `internal/workflow/execute.go:146-376` retries failed tasks up to `MaxHealAttempts=2` times, feeding error context back to the LLM for correction.

**Weakness: Stream truncation returns partial content + error — caller must handle both:**
- Issue: `consumeStreamWithTools()` at `internal/workflow/engine.go:718-771` returns `(content, partialToolCalls, err)` when the stream is truncated mid-response. The caller at `executeTaskWithTools()` at line 151 receives both partial content and an error, but the partial tool calls may be incomplete (missing arguments, wrong IDs).
- Files: `internal/workflow/engine.go:729-733`, `internal/workflow/execute.go:151-152`
- Impact: Partial tool calls from a truncated stream could be dispatched, causing undefined behavior. The self-heal loop may waste attempts on malformed tool calls.
- Fix approach: Discard partial tool calls when stream error occurs; return only partial content (text so far).

**Weakness: Ship phase degrades silently when task files are empty:**
- Issue: When `taskFiles` is empty at `ship.go:88-93`, the engine falls back to `AddAll()` and commits ALL uncommitted changes including unrelated files. This is logged at Error level but the commit still proceeds.
- Files: `internal/workflow/ship.go:88-93`
- Impact: Unrelated file changes get committed to git; user may not notice until reviewing the commit.
- Fix approach: Either skip the commit (return success with warning) or require explicit user consent before AddAll().

**Weakness: Demonstration generation uses full LLM call with no size guard:**
- Issue: `generateDemonstration()` at `internal/workflow/ship.go:178` streams a full LLM response for the walkthrough document with no `MaxLLMResponseBytes` guard at the engine level (the guard is in `consumeStream` but may be hit too late for very long demonstrations).
- Files: `internal/workflow/ship.go:178`
- Impact: For sessions with many tasks, the demonstration can exceed the context window (BUG-03).
- Fix approach: Truncate task list passed to demonstration prompt to limit response size.

### 7. Error Message Quality

**Good: Tool errors include raw input for debugging:**
- `Dispatcher.Execute()` at `internal/tools/dispatcher.go:164-169` truncates raw input to 200 chars and includes it in error messages. This aids debugging malformed LLM tool calls.

**Weakness: Some error messages use `%v` instead of `%w` for wrapping:**
- Issue: `internal/tools/fileread.go:131` uses `fmt.Errorf("%w: cannot access %s: %v", m31errors.ErrToolExecution, path, err)` — the inner error uses `%v` (stringified) instead of `%w` (wrapped). This means `errors.Is(err, os.ErrNotExist)` won't match.
- Files: `internal/tools/fileread.go:131`
- Impact: Error chain is broken; callers cannot detect the underlying filesystem error type.
- Fix approach: Use `%w` for the inner error: `fmt.Errorf("%w: cannot access %s: %w", ...)`.

**Weakness: `isRetryable()` uses fragile string matching:**
- Issue: `internal/provider/openrouter/client.go:158-171` checks for "500", "502", "connection reset" etc. via `strings.Contains(msg, "500")`. This could match non-HTTP error messages containing "500" (e.g., "processed 500 bytes").
- Files: `internal/provider/openrouter/client.go:158-171`
- Impact: False positive retries on non-retryable errors.
- Fix approach: Create typed errors from HTTP status codes; match with `errors.Is()`.

## Known Bugs

**BUG-01: Git status channel ordering issue (fixed):**
- Symptoms: Race condition between `git status --porcelain` and `git diff --numstat` goroutines
- Files: `internal/git/git.go:359`
- Trigger: Concurrent git status + numstat calls
- Workaround: Fixed by using separate result variables instead of channel ordering

**BUG-08: DNS cache unbounded growth (mitigated):**
- Symptoms: `sync.Map` in `WebFetch` growing without limit when many unique hosts are fetched
- Files: `internal/tools/webfetch.go:55-58`
- Trigger: Long-running session fetching many unique URLs
- Workaround: Threshold-based eviction every 64 inserts; not a true LRU

**BUG-10: Ship commit when working tree is clean after AddAll (fixed):**
- Symptoms: Empty commit attempted when no staged changes exist after `AddAll`
- Files: `internal/workflow/ship.go:95-103`
- Trigger: Clean working tree + task-to-file mapping empty
- Workaround: `DiffStaged()` check before commit

**BUG-12: Plan↔Discuss oscillation (mitigated):**
- Symptoms: Infinite loop between Plan and Discuss phases
- Files: `internal/workflow/engine.go:100,315-318`
- Trigger: TUI state bug or automated retry loop causing repeated Plan→Discuss transitions
- Workaround: `maxDiscussPlanCycles = 3` cap with counter reset on Execute/Ship/Idle

**BUG-15: Ship diff stats numstat heuristic (fixed):**
- Symptoms: Incorrect file change statistics when comparing against wrong base ref
- Files: `internal/workflow/ship.go:278`
- Trigger: Root commit or missing session start hash
- Workaround: Porcelain-based classification instead of numstat heuristic

**BUG-17: ModelCache singleflight waiter race (fixed):**
- Symptoms: `refreshing` flag touched by non-refreshing goroutines
- Files: `internal/provider/cache.go:47`
- Trigger: Concurrent cache refresh calls
- Workaround: `refreshing` flag only set/cleared by the singleflight leader

**BUG-18: Config reload message drop (fixed):**
- Symptoms: `ConfigReloadMsg` silently dropped when TUI receiver is busy
- Files: `internal/config/loader.go:849`
- Trigger: Rapid config file changes
- Workaround: Retry with 100ms backoff before blocking

**BUG-29: Token estimation underestimation (mitigated):**
- Symptoms: Context window exceeded on tool-heavy conversations
- Files: `internal/tokens/estimator.go:171`
- Trigger: Many tool calls with large JSON inputs
- Workaround: `EstimateMessages()` now accounts for tool call input JSON and per-message overhead

**BUG-04/05/06: Project classification edge cases (test-covered):**
- Symptoms: Incorrect framework/package manager detection for multi-framework projects
- Files: `internal/workflow/classify_test.go:12,42,68`
- Trigger: Projects with multiple lock files or framework indicators
- Workaround: Test coverage ensures known edge cases are handled

## Security Considerations

**SEC-01: No ReDoS protection in pure-Go grep fallback:**
- Risk: User-supplied regex patterns compiled via `regexp.Compile()` without timeout or complexity check
- Files: `internal/tools/grep.go:257` (line `re, err := regexp.Compile(pattern)`)
- Current mitigation: Pattern length capped at 1024 chars (`MaxGrepPatternLength`); ripgrep used when available (has its own protections)
- Recommendations: Add regex complexity check (nested quantifiers, alternation depth) or use `regexp2` with timeout for pure-Go path

**SEC-02: WebSearch missing DNS cache (defense-in-depth gap):**
- Risk: WebSearch resolves DNS on every request without caching, unlike WebFetch which has a 5-minute DNS cache with IP pinning
- Files: `internal/tools/websearch.go:44-62` (custom `DialContext`)
- Current mitigation: SSRF protection via `isPrivateIP()` check on each resolution
- Recommendations: Add DNS caching similar to WebFetch's `resolveAndCache()` pattern for defense-in-depth

**SEC-03: Incomplete HTML entity decoding in WebFetch:**
- Risk: `htmlToMarkdown()` and `htmlToText()` may not decode all HTML entities, potentially leaking raw entities in output
- Files: `internal/tools/webfetch.go:384-389` (calls to `htmlToMarkdown`, `htmlToText`)
- Current mitigation: Basic entity decoding exists
- Recommendations: Audit entity coverage; use a proper HTML parser library for complete decoding

**SEC-04: FileDelete reads entire file into memory for backup:**
- Risk: Large files read entirely into memory via `os.ReadFile()` before backup
- Files: `internal/tools/filedelete.go:115`
- Current mitigation: No size check before read
- Recommendations: Add size check; skip backup for files > threshold (e.g., 50MB) or stream-copy

**SEC-05: WebFetch/WebSearch unbounded body read with LimitReader:**
- Risk: Response bodies read up to `MaxFileSize+1` bytes (5MB+1) into memory; multiple concurrent requests could exhaust memory
- Files: `internal/tools/webfetch.go:371`, `internal/tools/websearch.go:174`
- Current mitigation: `io.LimitReader` cap at 5MB
- Recommendations: Consider per-tool memory budget or concurrent request limit

## Performance Bottlenecks

**Pure-Go grep walks entire directory tree:**
- Problem: `grepPureGo()` uses `filepath.Walk` which reads every file; no parallelism, no caching
- Files: `internal/tools/grep.go:258-359`
- Cause: Sequential file scanning with per-file binary detection (read 512 bytes, scan, seek back)
- Improvement path: Use `filepath.WalkDir` (avoids `os.Stat` per entry), parallelize file scanning with worker pool, or use `fs.WalkDir` with `os.DirEntry`

**MentionCompleter reads files during Scan():**
- Problem: `Scan()` reads every file < 100KB to count lines, which is O(n) in total file size
- Files: `internal/tui/mention.go:90-93`
- Cause: Line count estimation requires reading file content
- Improvement path: Cache results with mtime-based invalidation (already has `mentionCacheTTL`); consider using `wc -l` or stat-based estimation

**CodeIntel Build reads all source files sequentially:**
- Problem: `Build()` in `internal/codeintel/graph.go` reads and parses every source file in the project
- Files: `internal/codeintel/graph.go:160-196`
- Cause: `filepath.Walk` with synchronous file reads and parse
- Improvement path: Parallel file reads with worker pool; cache index with mtime-based invalidation

**Model cache refresh blocks all Get() callers:**
- Problem: `ModelCache.Refresh()` uses `singleflight` which blocks all concurrent callers until one fetch completes
- Files: `internal/provider/cache.go:48-62`
- Cause: Singleflight design — intentional but can cause latency spikes
- Improvement path: Serve stale cache during refresh (already partially implemented via `staleTTL`)

## Fragile Areas

**Ship phase task-to-file mapping:**
- Files: `internal/workflow/ship.go:70-103`
- Why fragile: When `taskFiles` is empty, ALL uncommitted changes are committed (line 89). The guard at line 95-103 prevents empty commits but doesn't prevent committing unrelated files.
- Safe modification: Always pass task file lists; never fall back to `AddAll()` without explicit user consent
- Test coverage: `internal/workflow/engine_test.go` covers happy path; edge case of empty task files tested indirectly

**SSE parser timeout handling:**
- Files: `internal/provider/sse.go:27-45`
- Why fragile: Watchdog timer closes `resp.Body` on timeout, which can cause `io.ErrClosedPipe` on the scanner. Error handling must distinguish timeout from real errors.
- Safe modification: Always check `ctx.Err()` before returning scanner errors; use `closeOnce` to prevent double-close
- Test coverage: `internal/provider/sse_test.go` covers basic scenarios

**Permission modal channel lifecycle:**
- Files: `internal/tools/dispatcher.go:24-28`, `internal/tools/permissions.go:39-45,251-261`
- Why fragile: Per-request channels created in `sync.Map` and deleted after response; race between timeout cleanup and normal response
- Safe modification: Use `sync.Once` for channel cleanup; always delete from map in defer
- Test coverage: `internal/tools/permissions_test.go` covers basic flow

**Task runner cancellation propagation:**
- Files: `pkg/taskrunner/runner.go:220-243`
- Why fragile: Context cancellation checked before goroutine launch AND inside goroutine; backoff timer also respects cancellation. Multiple cancellation paths must be coordinated.
- Safe modification: Always check `ctx.Err()` before and after blocking operations
- Test coverage: `pkg/taskrunner/runner_test.go` covers cancellation scenarios

## Scaling Limits

**Subagent concurrency:**
- Current capacity: 8 concurrent subagents (`MaxConcurrent = 8`)
- Limit: Each subagent spawns its own provider connection, dispatcher, and worktree — memory and fd pressure at high concurrency
- Scaling path: Increase `MaxConcurrent` only after profiling; consider connection pooling across subagents

**Tool output capping:**
- Current capacity: `MaxToolOutputChars = 10,000` for tool results; `BashOutputLimit = 50,000` for bash
- Limit: Large codebases can exceed these limits, causing truncated output
- Scaling path: Adaptive capping based on remaining context window; prioritize relevant output

**Session file size:**
- Current capacity: `MaxSessionFileSize = 50MB` for session reads
- Limit: Long sessions with many messages can approach this limit
- Scaling path: Implement message archival; compress old messages

## Dependencies at Risk

**`github.com/pkoukk/tiktoken-go` v0.1.8:**
- Risk: Third-party Go port of tiktoken; may lag behind upstream Python tiktoken updates
- Impact: Token estimation could be inaccurate for new model tokenizers
- Migration plan: Monitor upstream releases; fallback to rune-count estimation already exists

**`github.com/godbus/dbus/v5` v5.2.2:**
- Risk: D-Bus integration is Linux-only; Windows keychain uses syscall, macOS uses security CLI
- Impact: Cross-platform keychain reliability varies; D-Bus daemon availability is not guaranteed
- Migration plan: Already handled with multi-backend fallback (D-Bus → pass → env var)

**`github.com/fsnotify/fsnotify` v1.1.10:**
- Risk: File system notification reliability varies across platforms; fallback polling is 5s
- Impact: Config hot-reload may be delayed on some platforms
- Migration plan: Already mitigated with 5s polling fallback + 50ms debounce

## Missing Critical Features

**`--check-config` flag:**
- Problem: Referenced in documentation and help text but not implemented in CLI parsing
- Blocks: Users cannot validate config without running the full application
- Files: `cmd/m31a/main.go` (no `--check-config` in flag parsing)

**Backup pruning in FileDelete:**
- Problem: FileDelete has backup creation but no automatic cleanup of old delete backups
- Blocks: Backup directory grows unbounded with delete-heavy workflows
- Files: `internal/tools/filedelete.go` (no periodic cleanup)

**Context window utilization metric:**
- Problem: Token estimation exists but no real-time utilization metric exposed to TUI
- Blocks: Users cannot see how close they are to context limits during long sessions
- Files: `internal/tokens/estimator.go` (estimation exists, no UI integration)

## Test Coverage Gaps

**`cmd/m31a/` (0.0% coverage):**
- What's not tested: CLI flag parsing, config path resolution, provider wiring, signal handling
- Files: `cmd/m31a/main.go`, `cmd/m31a/usage.go`
- Risk: CLI entry point regressions undetected; config resolution bugs
- Priority: Medium (integration test would cover this)

**`internal/tui/` (~38.6% coverage):**
- What's not tested: Most screen Update/View methods, message routing, keyboard handling
- Files: `internal/tui/app_update.go`, `internal/tui/app_view.go`, `internal/tui/repl_model.go`
- Risk: TUI regressions detected only via manual testing
- Priority: High (primary user interface)

**`pkg/keychain/` (~20.3% coverage):**
- What's not tested: D-Bus Secret Service integration, pass CLI fallback, Windows Credential Manager
- Files: `pkg/keychain/keychain_linux.go`, `pkg/keychain/keychain_darwin.go`, `pkg/keychain/keychain_windows.go`
- Risk: API key storage failures on specific platforms; security of credential handling
- Priority: Medium (platform-specific, hard to test in CI)

---

## Resolved Issues (2026-06-17)

### Error Handling Fixes
- **ParsePlan silent failure** (`execute.go:423`): Now logs warning on parse failure instead of discarding error
- **Ship phase git errors** (`ship.go:60-61,98`): `HasUncommittedChanges`, `StatusPorcelain`, and `DiffStaged` errors now logged with safe defaults (assume dirty on error)
- **Ship phase AddAll** (`ship.go:88-93`): No longer commits unrelated files silently; skips commit when no task-to-file mapping exists
- **FileRead error wrapping** (`fileread.go:116,131`): Changed `%v` to `%w` to preserve error chain for `errors.Is()` matching
- **Engine error wrapping** (`engine.go:292,526,529`): Wrapped with `ErrPhaseTransition` sentinel for `errors.Is()` matching
- **Missing ErrGitNotInitialized sentinel** (`initialize.go:39`, `verify.go:173`): Added sentinel error with `UserMessage()` case

### Panic Recovery
- **Tool execution goroutines** (`execute.go:246-286`): Added `recover()` that stores panic error in results and emits `ToolCompleteMsg`
- **Agent loop goroutine** (`agent_loop.go:101`): Added `recover()` that sends `AgentErrorMsg` on panic
- **Streaming goroutine** (`streaming.go:85`): Added `recover()` that sends `StreamErrorMsg` on panic
- **Dispatcher rate-limiter** (`dispatcher.go:67-79`): Added `recover()` with `slog.Error` logging
- **Config watcher** (`app.go:426-429`): Added `recover()` with `slog.Error` logging
- **Signal handler** (`main.go:270-291`): Added `recover()` with `slog.Error` logging
- **Bash tool goroutines** (`bash.go:138,168,182,191`): Added `recover()` to all 4 goroutines

### Stream Safety
- **Partial tool calls on truncation** (`engine.go:728-733`): `consumeStreamWithTools` now returns `nil` for partial tool calls on stream error, preventing dispatch of incomplete calls
- **Demonstration size guard** (`ship.go:401-459`): Task list capped to 20 tasks for demonstration generation

### Typed Error Classification
- **isRetryable string matching** (`openrouter/client.go:158-171`): Added `provider.HTTPStatusError` typed error carrying HTTP status codes; `isRetryable()` now uses `errors.As` for typed matching with string fallback for network errors

### Tech Debt Cleanup
- **Backup pruning inconsistency** (`filedelete.go`, `filewrite.go`): Extracted shared `pruneBackupsByPrefix()` utility in `internal/tools/backup.go`; both tools now delegate to it

### Already Mitigated (no changes needed)
- **Global mutable state** (`goModulePath`): Already uses `sync.Mutex`-guarded cache
- **Gitignore cache eviction**: Already uses mtime-based invalidation via `loadGitignoreCached()`
- **LoadDotEnv race**: Already guarded with `sync.Once`, called before goroutine init
- **ReDoS protection**: `checkRedos()` already detects nested and adjacent quantifiers

---

*Concerns audit: 2026-06-17*
*Resolution: 2026-06-17*

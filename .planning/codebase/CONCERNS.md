# Codebase Concerns

**Analysis Date:** 2026-06-16

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

*Concerns audit: 2026-06-16*

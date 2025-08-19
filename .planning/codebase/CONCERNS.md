# Codebase Concerns

**Analysis Date:** [YYYY-MM-DD]

## Tech Debt

**Monolithic TUI State Machine:**
- Issue: `internal/tui/app_update.go` handles dozens of message types in an 850+ line giant switch block, managing everything from repl to command palettes and fallbacks. It violates SRP.
- Files: `internal/tui/app_update.go`
- Impact: Modifying the TUI requires finding the right place in this massive switch, and changes to one case can subtly affect others through shared state.
- Fix approach: Extract message handlers into separate methods per screen or use a handler map pattern.

**Massive Session Manager:**
- Issue: The session manager handles reading, writing, rollback, auto-dream parsing, and directory operations in a single large struct.
- Files: `pkg/session/manager.go`
- Impact: High complexity, making it difficult to safely modify session lifecycle logic or add new session storage backends.
- Fix approach: Split responsibilities into discrete services (e.g., `Storage`, `Checkpointer`, `Parser`).

**`closeOnces` sync.Map unbounded growth:**
- Issue: The global `var closeOnces sync.Map` stores `*sync.Once` entries for every channel closed via `safeCloseOnce()`. Entries are never removed.
- Files: `internal/tui/app.go`
- Impact: Over a long session with many phase transitions, this map grows without bound, causing a small but architecturally incorrect memory leak.
- Fix approach: Use a per-`AppState` map that is cleaned up on session end, or store the `sync.Once` inside the channel wrapper struct.

**Hardcoded model capability map:**
- Issue: `openrouterModelCapabilities` is a hardcoded map of ~11 model IDs to capability flags. Models not in this map fall back to heuristic string-sniffing.
- Files: `internal/provider/openrouter/client.go`
- Impact: New models are not correctly classified until the code is updated. The heuristic fallback is fragile.
- Fix approach: Prefer capabilities from the provider API response if available, or move the map to a config file.

**`mergeConfig` is field-by-field manual merge:**
- Issue: `mergeConfig()` manually checks and copies every config field. Adding a new config field requires updating this function.
- Files: `internal/config/loader.go`
- Impact: High maintenance cost; new config fields can silently be ignored by project-level configs if forgotten here.
- Fix approach: Use reflection-based merge (check non-zero values), switch to a layered config library, or generate the merge function.

## Known Bugs

**Verify fallback uses `HEAD~50` which may not exist:**
- Symptoms: When `sessionStartHash` is empty, the bisect fallback uses `"HEAD~50"` as the "good" commit. If the repository has fewer than 50 commits, this fails silently.
- Files: `internal/workflow/verify.go`
- Trigger: Session starts in a shallow repository or a repository with fewer than 50 commits, AND verification fails after max heal attempts.
- Workaround: Ensure `sessionStartHash` is always captured in Initialize phase.

**Git operations ignore errors silently in workflow phases:**
- Symptoms: Errors from `git.HeadHash()` and `git.Log()` are silently discarded with `_ =`.
- Files: `internal/workflow/ship.go`, `internal/workflow/execute.go`
- Trigger: Git operations fail (e.g., detached HEAD, empty repository).
- Workaround: None — missing commit hashes or log entries silently degrade the Ship summary.

## Security Considerations

**API key stored in `AppState.apiKey` field:**
- Risk: The API key is held in the `AppState.apiKey` string field. While Go strings are immutable, this field persists in memory for the entire session lifetime and could be exposed via memory dumps.
- Files: `internal/tui/app.go`
- Current mitigation: API keys are resolved from env var → keychain → config file, with keychain preferred.
- Recommendations: Zero the key after use (using byte slices instead of strings), or keep it only in the provider clients without propagating to AppState.

**`os.Exit(1)` in library code:**
- Risk: `NewApp()` calls `os.Exit(1)` if the tool dispatcher fails to initialize. This is in library/constructor code, not main().
- Files: `internal/tui/app.go`
- Current mitigation: None — the process terminates without cleanup.
- Recommendations: Return an error from `NewApp()` instead. Let `cmd/m31a/main.go` handle the exit.

**Command execution allows arbitrary shell:**
- Risk: The Bash tool executes raw user-provided strings via `bash -c`.
- Files: `internal/tools/bash.go`
- Current mitigation: Limits output to 50KB to prevent OOM, uses context timeouts, and sets `RiskLevel: RiskDangerous`.
- Recommendations: Add further sandboxing (e.g., containerization or strict allowlists) for deployments where the AI is not fully trusted.

## Performance Bottlenecks

**`listCwdFiles` uses `filepath.Walk` (O(n) on large repos):**
- Problem: `listCwdFiles()` walks the entire working directory tree. While it skips known heavy directories, the initial walk is still O(n) where n is the total file count.
- Files: `internal/workflow/engine.go`
- Cause: `filepath.Walk` traverses all directories before applying skip logic.
- Improvement path: Use `filepath.WalkDir` (cheaper per-entry) or `os.ReadDir` for shallower enumeration. For very large repos, consider caching the file list.

**`extractJSONObject` re-scans from each `{` position:**
- Problem: `parseToolCalls()` scans the content byte-by-byte looking for `{`, then calls `extractJSONObject()` which re-parses the entire remaining string.
- Files: `internal/workflow/engine.go`
- Cause: Linear scan + nested extraction without maintaining position.
- Improvement path: Optimize the parser to maintain scan position or utilize a streaming JSON decoder.

## Fragile Areas

**TUI Testing Infrastructure:**
- Files: `internal/tui/app_test.go`, `internal/tui/commands_test.go`
- Why fragile: These files are massive (2500+ and 2200+ lines). Modifying `internal/tui` requires changing thousands of lines of fragile mock tests.
- Safe modification: Refactor testing to use smaller, isolated component tests rather than full `AppState` integration tests.
- Test coverage: Extensive, but rigid and brittle.

**Streaming pipeline:**
- Files: `internal/tui/streaming.go`, `internal/tui/repl_stream.go`
- Why fragile: The streaming pipeline spans multiple files with complex goroutine/channel interactions. Historical fixes indicate this area has been error-prone.
- Safe modification: Do not modify channel ownership or goroutine lifecycle without running `go test -race` on the full test suite.
- Test coverage: Has coverage including double-close and nil channel concurrency tests, but UI race conditions remain a risk.

## Scaling Limits

**Session file accumulation:**
- Resource/System: Disk Space.
- Current capacity: Unbounded. Each session creates a directory under `~/.m31a/sessions/` with JSON files, planning files, checkpoints, and backups.
- Limit: Disk space exhaustion over long-lived installations.
- Scaling path: Add automatic session pruning (e.g., auto-archive or delete sessions older than 30 days).

**Ledger file growth:**
- Resource/System: File I/O and Memory.
- Current capacity: `config.Ledger.MaxEntries` controls the cap. Entries are appended as markdown.
- Limit: The ledger re-parses the entire file on load. With thousands of entries, startup time degrades.
- Scaling path: Transition to a binary format or indexed file/database for large ledgers.

## Dependencies at Risk

**`github.com/bmatcuk/doublestar/v4`:**
- Package: `doublestar`
- Risk: Used for glob pattern matching. The v4 major version may have breaking changes or missing maintenance compared to newer stdlib alternatives if `io/fs` adds globbing.
- Impact: Low — used only for permission rule matching.
- Migration plan: Pin to current version; monitor for native Go `filepath` improvements.

**`github.com/BurntSushi/toml`:**
- Package: `BurntSushi/toml`
- Risk: Used for TOML parsing. The `v1` package is in maintenance mode; `v2` has a different API.
- Impact: Medium — config loading is in the critical path.
- Migration plan: Upgrade to v2 by updating `toml.DecodeFile` calls and struct tags.

## Missing Critical Features

**Backup pruning:**
- Feature gap: FileWrite creates backups on every write with no cleanup.
- Problem: Over a long coding session, the backup directory can grow to hundreds of megabytes.
- Blocks: Long-running sessions and production use on constrained file systems.

**Config hot-reload:**
- Feature gap: Configuration is loaded once at startup.
- Problem: Changes to `~/.m31a/config.toml` require restarting the TUI.
- Blocks: Users who want to adjust settings dynamically without losing their session state.

## Test Coverage Gaps

**OS-specific keychain implementations:**
- Untested area: Secure storage implementations for non-default OSes.
- What's not tested: `keychain_linux.go`, `keychain_darwin.go`, `keychain_windows.go`. Only shared logic (`keychain_test.go`) is heavily tested. (Overall package coverage: 4.0%)
- Files: `pkg/keychain/`
- Risk: OS-specific keychain integration may fail on certain desktop environments silently.
- Priority: High

**TUI Components:**
- Untested area: Granular visual components.
- What's not tested: Many sub-components lack rendering checks. (Package coverage: 33.1%)
- Files: `internal/tui/components/`
- Risk: UI rendering bugs and visual regressions.
- Priority: Medium

**Tool Implementations:**
- Untested area: Execution paths for various tools.
- What's not tested: Edge cases in file manipulation and OS interaction. (Package coverage: 49.3%)
- Files: `internal/tools/`
- Risk: Tools may misbehave on unusual file systems or fail without clear errors.
- Priority: High

---

*Concerns audit: [YYYY-MM-DD]*
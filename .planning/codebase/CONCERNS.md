# Codebase Concerns

**Analysis Date:** 2026-06-14

## Tech Debt

**BurntSushi/toml v1 dependency:**
- Issue: Using `github.com/BurntSushi/toml v1.6.0` which is in maintenance mode; v2 has a different API
- Files: `go.mod:7`, `internal/config/loader.go:16`
- Impact: No security patches or feature improvements will be backported; v2 migration required eventually
- Fix approach: Pin current version (already done via DEP-3 comment); schedule migration when v2 stabilizes

**Reflection-based config merging:**
- Issue: `mergeConfig` uses `reflect` to recursively merge TOML configs, which is fragile and hard to debug
- Files: `internal/config/loader.go:275-370`
- Impact: Silent failures when new fields are added to Config struct but not handled in merge logic; performance cost of reflection
- Fix approach: Consider code generation or manual merge for critical fields; add comprehensive test coverage for edge cases

## Known Bugs

**TaskRunner lock upgrade race condition:**
- Symptoms: Potential panic from concurrent map read/write during parallel task execution
- Files: `pkg/taskrunner/runner.go:168-193`
- Trigger: When `ExecuteGroup` pre-filters tasks with failed dependencies while worker goroutines update status concurrently
- Workaround: The pre-filter runs before parallel execution starts, so the race window is narrow in practice
- Detail: The code calls `r.mu.RUnlock()` (line 173/182) then `r.mu.Lock()` (line 174/183) to upgrade from read to write lock. Between these calls, another goroutine could modify `r.status`. Additionally, the initial RLock (line 168) is held across the dependency check loop, but the status reads (line 171) may see stale data if a worker goroutine writes between the read and the lock upgrade.

**Session ID collision retry without cleanup:**
- Symptoms: Orphaned empty session directories on disk
- Files: `pkg/session/manager.go:146-156`
- Trigger: When `generateID` produces a collision, the code retries up to 10 times but doesn't clean up partially-created directories from failed `ensureDir` calls
- Workaround: The `ensureDir` call on line 168 only creates the directory AFTER the collision check, so this is actually safe. However, if `ensureDir` succeeds but `atomicWrite` fails, the empty directory is left behind.
- Fix approach: Add cleanup in `NewSession` on error paths; or use `os.MkdirAll` + atomic write with temp file in the session directory

## Security Considerations

**API keys persisted to config file when keychain unavailable:**
- Risk: API keys written in plaintext to `~/.m31a/config.toml` when OS keychain is unavailable
- Files: `internal/config/loader.go:734-780`
- Current mitigation: File permissions are set to `0644` (world-readable); `.env` permission check warns on group/world-writable files
- Recommendations: Set config file permissions to `0600` (owner-only); add warning in TUI when keys are stored in plaintext; consider encrypted file fallback

**`.env` file permission check is incomplete:**
- Risk: `.env` files with world-readable permissions (0644 default) expose secrets to other users on the system
- Files: `internal/config/loader.go:982-985`
- Current mitigation: Only rejects group/world-writable files (0o022 check)
- Recommendations: Also warn when `.env` is world-readable (not just writable); suggest chmod 0600; consider refusing to load `.env` with permissions > 0600

**Git commit message sanitization is basic:**
- Risk: Crafted commit messages could exploit downstream tools that parse git log output
- Files: `internal/git/git.go:84-91`
- Current mitigation: Strips newlines and caps at 200 chars
- Recommendations: Also sanitize shell metacharacters; validate against known git trailer injection patterns

**Config variable substitution exposes env vars:**
- Risk: `${VAR}` patterns in config files could be used to exfiltrate environment variables if config is shared
- Files: `internal/config/loader.go:617-684`
- Current mitigation: Only resolves explicitly referenced `${VAR}` patterns; doesn't enumerate all env vars
- Recommendations: Document the security model; consider allowlist of safe variables for substitution

## Performance Bottlenecks

**Git status + numstat concurrent execution:**
- Problem: `StatusPorcelain` runs `git status --porcelain` and `git diff --numstat HEAD` concurrently but doesn't handle the case where one fails
- Files: `internal/git/git.go:351-463`
- Cause: If `git diff --numstat HEAD` fails (e.g., no commits yet), the error is silently swallowed and numstat data is empty
- Improvement path: Return partial results with a warning; or check for empty repo first

**Grep pure-Go fallback opens files twice:**
- Problem: When ripgrep is unavailable, the pure-Go grep reads each file header for binary detection, then seeks back and scans the full file
- Files: `internal/tools/grep.go:248-339`
- Cause: Binary detection reads 512 bytes, then `f.Seek(0, 0)` resets the reader
- Improvement path: Read the full file content once and check for null bytes in the initial read; or use a buffered reader that supports peeking

**Session list cache invalidation is coarse-grained:**
- Problem: Any session modification (create, delete, archive) invalidates the entire session list cache
- Files: `pkg/session/manager.go:189-193, 458-461, 488-492`
- Cause: Cache is a simple slice with TTL; no per-entry invalidation
- Improvement path: For typical usage (few sessions), this is acceptable. For power users with many sessions, consider incremental cache updates.

## Fragile Areas

**Bash tool process group kill:**
- Files: `internal/tools/bash_unix.go:17-23`, `internal/tools/bash.go:130-161`
- Why fragile: Uses negative PID (`-pid`) to kill process group, which sends SIGINT/SIGKILL to all processes in the group. If the process has spawned children that re-parented themselves, they won't be killed.
- Safe modification: The 5-second grace period before SIGKILL is a good mitigation. Don't change the kill sequence without testing with nested process trees.
- Test coverage: `internal/tools/bash_test.go`, `internal/tools/bash_kill_test.go`

**Config file watcher debounce:**
- Files: `internal/config/loader.go:876-924`
- Why fragile: Uses `time.AfterFunc` for debounce; if multiple config writes happen rapidly, only the last one triggers a reload. The 50ms debounce is aggressive and may miss rapid successive writes.
- Safe modification: Increase debounce to 100-200ms; test with editors that do atomic saves (write-to-temp + rename)
- Test coverage: `internal/config/loader_test.go`

**Session checkpoint read-modify-write:**
- Files: `pkg/session/checkpoint.go:27-51`
- Why fragile: `SaveCheckpoint` reads existing checkpoints, appends, trims, and writes back. If two concurrent saves happen, one could overwrite the other's checkpoint. The `atomicWrite` only protects the final write, not the read-modify-write cycle.
- Safe modification: Use file locking (flock) for the read-modify-write cycle, or accept that checkpoints are best-effort
- Test coverage: `pkg/session/checkpoint_test.go`

## Scaling Limits

**Subagent concurrency cap:**
- Current capacity: 8 concurrent subagents (`internal/tools/subagent/manager.go:18`)
- Limit: Each subagent spawns a goroutine, creates a dispatcher, and may create a git worktree. With 8 subagents, that's 8 goroutines + 8 dispatchers + up to 8 worktrees.
- Scaling path: Increase `MaxConcurrent` if needed; monitor memory usage per subagent; consider worktree pooling

**Session file size limit:**
- Current capacity: 50 MB (`internal/types/constants.go:27`)
- Limit: Sessions with very long conversations or many tool calls could approach this limit
- Scaling path: Already mitigated by `readFileLimited` with size check; consider streaming JSON parser for very large sessions

## Dependencies at Risk

**`github.com/pkoukk/tiktoken-go`:**
- Risk: Third-party token counting library; may lag behind OpenAI's tiktoken updates
- Impact: Token estimation could be inaccurate for newer models, leading to context window overflows
- Migration plan: Monitor for updates; consider fallback to character-based estimation

**`github.com/godbus/dbus/v5`:**
- Risk: Linux-only D-Bus dependency for keychain; may not work on all Linux desktop environments
- Impact: Keychain operations fail silently, falling back to `pass` CLI or plaintext config
- Migration plan: Already has fallback chain (D-Bus → pass → config file); test on various Linux distros

## Test Coverage Gaps

**WebFetch SSRF protection:**
- What's not tested: The DNS rebinding prevention in the dialer's `DialContext` closure is complex and hard to test without mocking the DNS resolver
- Files: `internal/tools/webfetch.go:73-114`
- Risk: SSRF bypass if the DNS cache has a race condition or the private IP check is incomplete
- Priority: Medium — the `isPrivateIP` function is well-tested, but the integration with the dialer is not

**Config hot-reload race conditions:**
- What's not tested: Concurrent config reloads from multiple fsnotify events
- Files: `internal/config/loader.go:876-924`
- Risk: Config corruption or lost updates during rapid file changes
- Priority: Low — debounce timer prevents most races; single-writer pattern in practice

**Subagent worktree cleanup on crash:**
- What's not tested: The `Sweep` function that cleans up stale worktrees on startup
- Files: `internal/tools/subagent/worktree.go:90-122`
- Risk: Stale worktrees accumulate if sweep fails or is skipped
- Priority: Low — sweep is best-effort; manual cleanup is possible

---

*Concerns audit: 2026-06-14*

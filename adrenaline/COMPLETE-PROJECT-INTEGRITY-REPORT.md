# M31A — Complete Project Integrity Report

**Date:** 2026-06-10  
**Branch:** master  
**Go Version:** 1.26.4  
**Total Files:** ~200 Go files  
**Total Lines:** ~60,388  
**Build Status:** PASS (`CGO_ENABLED=0 go build ./cmd/m31a`)  
**Static Analysis:** PASS (`go vet ./...` — clean)  

---

## 1. Architecture Overview

M31A is a terminal AI coding agent built on Bubble Tea with a six-phase workflow:

```
Initialize → Discuss → Plan → Execute → Verify → Ship
```

### Package Dependency Graph (simplified)

```
cmd/m31a/
  ├── internal/config/          ← TOML config + env vars + keychain
  ├── internal/tui/             ← Bubble Tea app (AppState, screens, components)
  │     ├── components/         ← Reusable UI widgets
  │     ├── layout/             ← Responsive layout engine
  │     └── theme/              ← Theme system (dark/light/auto)
  ├── internal/workflow/        ← Six-phase workflow engine
  ├── internal/tools/           ← Tool implementations (Bash, Edit, FileRead, etc.)
  ├── internal/provider/        ← LLM provider abstraction
  │     ├── openrouter/         ← OpenRouter client
  │     └── zen/                ← Zen gateway client
  ├── internal/types/           ← Shared types
  ├── internal/tokens/          ← Token estimation (tiktoken-go + EMA calibration)
  ├── internal/git/             ← Git operations wrapper
  ├── internal/log/             ← Structured logging with rotation
  ├── internal/errors/          ← Sentinel errors + user messages
  ├── internal/fileutil/        ← Atomic file writes
  └── pkg/
        ├── taskrunner/         ← Dependency graph + topological sort execution
        ├── session/            ← Session persistence (JSON + markdown)
        ├── keychain/           ← OS-native secret storage (linux/darwin/windows)
        ├── ledger/             ← Cross-session learning records
        ├── rollback/           ← Commit chain browser + reset operations
        ├── arbitrage/          ← Model cost comparison
        ├── autodream/          ← Context consolidation
        └── bisect/             ← Git bisect wrapper
```

---

## 2. Build & Compilation Integrity

| Check                    | Status | Notes                                              |
|--------------------------|--------|----------------------------------------------------|
| `CGO_ENABLED=0 build`   | PASS   | Static binary, no CGO                              |
| `go vet ./...`           | PASS   | Zero warnings                                      |
| Unused imports           | PASS   | No dead imports detected                           |
| Embedded assets          | PASS   | `prompts/*.md` embedded via `//go:embed`           |
| Build tags               | PASS   | Platform-specific files (bash_unix, bash_windows, keychain_*) |

---

## 3. Architecture Rule Compliance (per AGENTS.md)

| Rule                                      | Status | Notes                                                                 |
|-------------------------------------------|--------|-----------------------------------------------------------------------|
| Bubble Tea single-threaded (Update only)  | PASS   | All state mutations go through `Update()`. Signal handler sends `tea.QuitMsg{}` via `p.Send()` (main.go:221) |
| HTTP 30s dial timeout, no body timeout    | PASS   | `DialContext: (&net.Dialer{Timeout: types.HTTPDialTimeout})` in both OpenRouter and Zen clients |
| Context pruning per phase                 | PASS   | Each phase builds fresh message lists from planning/ files only       |
| No direct Anthropic/OpenAI connections    | PASS   | Only OpenRouter and Zen providers exist                               |
| No CGO                                    | PASS   | Binary builds with `CGO_ENABLED=0`                                    |
| No telemetry/analytics                    | PASS   | No external calls except provider APIs                                |
| API key resolution order                  | PASS   | env var → keychain → config file (loader.go:601-632)                  |
| No hardcoded model lists                  | PASS   | Models discovered dynamically via `FetchModels()`                     |
| V1 sequential task execution              | PASS   | `ExecuteGroup` runs tasks sequentially within groups                  |
| AskUserQuestion not in automated flows    | PASS   | Question tool exists but is not invoked during workflow execution      |

---

## 4. Security Analysis

### 4.1 Command Injection (Bash Tool)
- **Status:** ACCEPTABLE RISK
- **Assessment:** The Bash tool intentionally executes arbitrary shell commands. This is by design for a coding agent.
- **Mitigations in place:**
  - Output capped at `BashOutputLimit` (50,000 chars) via `limitWriter` (bash.go:94-95)
  - Timeout enforced (max 30 minutes, configurable per-call) (bash.go:76-80)
  - Process group kill with SIGINT → grace period → SIGKILL (bash.go:119-145)
  - Permission system gates execution via risk level (`RiskDangerous`) (bash.go:33)

### 4.2 SSRF Protection (WebFetch Tool)
- **Status:** WELL-IMPLEMENTED
- **Defenses:**
  - DNS rebinding prevention via `sync.Map` cache with 5-minute TTL (webfetch.go:42-46, 211-247)
  - Private IP blocking at dial-time AND post-connect (webfetch.go:74-98)
  - Redirect targets re-resolved and checked (webfetch.go:104-135)
  - Cloud metadata endpoint (169.254.169.254) explicitly blocked (webfetch.go:160-163)
  - `isPrivateIP()` covers loopback, link-local, RFC1918, IPv6 ULA, unspecified (webfetch.go:145-166)
  - Response body capped at 5MB (webfetch.go:343-352)

### 4.3 Path Traversal
- **Status:** WELL-HANDLED
- **Defenses:**
  - Edit tool resolves symlinks and checks `workDirPrefix` (edit.go:176-211)
  - FileRead/FileWrite use `resolvePath` with symlink evaluation
  - File size checked before read (`MaxFileSize = 5MB`) (edit.go:98-100)

### 4.4 API Key Security
- **Status:** WELL-IMPLEMENTED
- **Defenses:**
  - Keys never persisted to config file (loader.go:568-570: `cfgCopy.Provider.OpenRouter.APIKey = ""`)
  - `APIKey()` method returns masked key (`"****" + last4chars`) (openrouter/client.go:94-99)
  - Provider errors sanitized via `maskAPIKeys()` regex (common.go:114-124)
  - Keychain uses OS-native secure storage (linux: Secret Service/pass, darwin: security CLI, windows: Credential Manager)
  - `.env` loader skips world-writable files (loader.go:696-698)

### 4.5 Tool Input Validation
- **Status:** ROBUST
- **Defenses:**
  - `MaxLLMResponseBytes` (1MB) prevents OOM from pathological LLM responses (types/constants.go:21)
  - `MaxToolOutputChars` (10,000) caps tool output
  - `MaxSessionFileSize` (50MB) prevents OOM from corrupted session files (types/constants.go:25)
  - Tool input size checked in `parseToolCalls()` (engine_parse.go:296-300)
  - `maxToolsPerCall` (16) caps tool calls per response (engine_parse.go:283)
  - Dispatcher rate limiter: token bucket at `ToolRateLimitPerSec` (dispatcher.go:56-73)

---

## 5. Concurrency & Thread Safety

### 5.1 Bubble Tea Contract
- **Status:** COMPLIANT
- All `AppState` mutations happen inside `Update()` only
- Goroutines communicate via `tea.Cmd` / `tea.Msg` channels
- Signal handler in main.go sends `tea.QuitMsg{}` via `p.Send()` — never mutates app state directly
- `channelEmitter` (app_channel.go:44-57) bridges workflow goroutines to Bubble Tea via buffered channel (128 capacity)

### 5.2 Provider Registry
- **Status:** THREAD-SAFE
- All methods use `sync.RWMutex` correctly
- `TrySetActive()` provides atomic set+get to prevent TOCTOU races (registry.go:58-67)
- `RollbackActive()` uses conditional check+set under lock (registry.go:72-80)

### 5.3 Model Cache
- **Status:** THREAD-SAFE
- Uses `sync.RWMutex` for all read/write operations
- `singleflight.Group` deduplicates concurrent refresh calls (cache.go:51)
- `atomic.Bool` for `refreshing` flag (cache.go:25)
- `Models()` returns deep copies to prevent external mutation (cache.go:119-128)

### 5.4 Token Estimator
- **Status:** THREAD-SAFE
- `emaFactor` protected by `sync.Mutex` (estimator.go:77-79, 94-102)
- Factor clamped to [0.1, 10.0] to prevent extreme values

### 5.5 Tool Dispatcher
- **Status:** THREAD-SAFE
- Tool map protected by `sync.RWMutex`
- Permission responses use per-request channels via `sync.Map` (dispatcher.go:24, 218-219)
- Rate limiter goroutine cleanly stopped via `rateDone` channel (dispatcher.go:262-270)

---

## 6. Error Handling

### 6.1 Sentinel Errors
- **Status:** COMPREHENSIVE
- 23 sentinel errors defined in `internal/errors/errors.go`
- `UserMessage()` provides user-friendly messages for all sentinels
- Pattern matching fallback for HTTP status codes in error strings (errors.go:105-119)

### 6.2 Error Propagation
- **Status:** CONSISTENT
- All functions wrap errors with `%w` for proper `errors.Is()` chains
- Context-specific error wrapping (e.g., `fmt.Errorf("git commit: %w", err)`)
- Non-fatal failures logged with `slog.Warn` and execution continues

### 6.3 Graceful Degradation
- **Status:** WELL-IMPLEMENTED
- Missing config file → defaults applied (loader.go:107-108)
- Failed keychain → logged as warning, execution continues (main.go:103)
- Corrupted session files → marked `Corrupted=true`, listing continues (manager.go:358-365)
- Failed health check → fallback to stale cache (openrouter/client.go:163-165)
- LLM stream failure → partial content preserved (engine.go:552-555)

---

## 7. Resource Management

### 7.1 File Handles
- **Status:** PROPERLY MANAGED
- All file reads use `defer f.Close()`
- HTTP response bodies drained with `io.Copy(io.Discard, io.LimitReader(...))` before close
- SSE parser uses `sync.Once` for idempotent close (sse.go:103-114)
- Pipe ends explicitly closed after process start failure (bash.go:101-106)

### 7.2 Goroutine Lifecycle
- **Status:** WELL-MANAGED
- Shutdown sequence: `workflowCancel` → `shutdownCancel` → `streamCancelFn` → `dispatcher.Stop()` (app.go:82-94)
- Rate limiter goroutine has clean exit via `rateDone` channel
- Signal handler goroutine exits via `sigDone` channel (main.go:218-224)
- Config watcher respects `ctx.Done()` (loader.go:650-670)

### 7.3 Memory Bounds
- **Status:** WELL-BOUNDED
- `MaxLLMResponseBytes` (1MB) checked incrementally during streaming (engine.go:558-563)
- `BashOutputLimit` (50K chars) via `limitWriter`
- `MaxSessionFileSize` (50MB) on all session file reads
- `maxJSONScanBytes` (64KB) limits tool call scanning (engine_parse.go:287)
- SSE line buffer capped at 1MB (sse.go:120)
- Session list cached with configurable TTL to avoid repeated filesystem walks

---

## 8. Workflow Engine Integrity

### 8.1 Phase Transitions
- **Status:** STRICTLY VALIDATED
- `validPhaseTransitions` map defines all allowed transitions (engine.go:243-251)
- Invalid transitions rejected with `ErrPhaseTransition`
- Checkpoints saved on every transition (engine.go:280-292)
- STATE.md written for each phase transition

### 8.2 Self-Healing
- **Status:** ROBUST
- Max 2 heal attempts per task (`MaxHealAttempts`)
- Self-heal triggers on: LLM stream failure, tool call parse failure, tool execution failure, verification failure
- Bisect fallback when standard heal fails (verify.go:88-101, 182-246)
- Pre-task checkpoints enable rollback on heal failure (execute.go:57-63)

### 8.3 Task Scheduling
- **Status:** CORRECT
- Kahn's algorithm for topological sort (runner.go:100-133)
- Self-reference detection (runner.go:69-75)
- Circular dependency detection via processed-count check (runner.go:128-130)
- Dependency-failed tasks correctly skipped (runner.go:163-179)
- Per-task timeout with context propagation (runner.go:197-198)

### 8.4 Plan Parsing
- **Status:** RESILIENT
- Multi-strategy parsing: markdown plan parser → JSON array fallback
- Bracket-depth tracking for JSON extraction (engine_parse.go:65-97)
- JSON comment stripping (engine_parse.go:493-564)
- Task validation: duplicate IDs, self-references, missing fields, cycle detection
- Max 3 retries with validation error feedback to LLM

### 8.5 Budget Guardrail
- **Status:** IMPLEMENTED
- `BudgetLimitUSD` checked before each phase (engine.go:196-204)
- Cumulative cost tracked via `totalCost` field
- Cost estimated from model pricing × token usage

---

## 9. Data Persistence

### 9.1 Session Management
- **Status:** ROBUST
- Atomic writes via temp file + rename (fileutil/atomic.go)
- Session ID generation uses `crypto/rand` with collision retry (manager.go:134-148)
- Session ID format validated on load (manager.go:195-197)
- File size limits prevent OOM from corrupted files (manager.go:88-103)
- Session list cache with double-checked locking (manager.go:312-332)
- Cleanup removes sessions older than retention period on startup (app.go:24-34)

### 9.2 Planning Files
- **Status:** WELL-STRUCTURED
- Per-session `planning/` directory with PROJECT.md, PLAN.md, TASKS.md, STATE.md
- Context pruning reads only from planning files (not conversation history)
- Plan versioning with refinement support (engine.go:488-493)

### 9.3 Ledger
- **Status:** CORRECT
- Atomic writes via temp file + rename (ledger.go:346-396)
- Deduplication by SessionID (ledger.go:130-134)
- Truncation keeps newest N entries (ledger.go:321-343)
- mtime-based stats cache prevents recomputation (ledger.go:240-244)

---

## 10. Issues & Findings

### 10.1 CRITICAL — None Found

### 10.2 HIGH — None Found

### 10.3 MEDIUM Issues

#### M-1: `git.logInternal()` `oneline` Parameter Unused
- **File:** `internal/git/git.go:143`
- **Issue:** The `oneline` parameter is accepted but never used in the function body.
- **Impact:** Dead parameter, no functional impact. `parseLog()` also accepts `oneline` but ignores it.
- **Recommendation:** Remove unused parameter or implement the one-line format.

#### M-2: `git.DiffRefs()` Argument Ordering Ambiguity
- **File:** `internal/git/git.go:200-211`
- **Issue:** When both refs are provided, it constructs `ref1..ref2` which means "commits reachable from ref2 but not ref1". The `ship.go:219` call uses `DiffRefs("HEAD", "")` which triggers the empty branch (plain `git diff`), not a comparison.
- **Impact:** The `collectDiffStats()` in ship.go may not produce the intended diff after commit.
- **Recommendation:** Clarify semantics or use explicit `git diff HEAD~1 HEAD` after commit.

#### M-3: `config.loadDotEnv()` Quote Handling Incomplete
- **File:** `internal/config/loader.go:716-721`
- **Issue:** Multi-line quoted values in `.env` files are not supported. Values with embedded newlines inside quotes will be split incorrectly.
- **Impact:** Low — most `.env` files use single-line values.
- **Recommendation:** Document limitation or add multi-line support.

#### M-4: `WebFetch` DNS Cache Not Bounded
- **File:** `internal/tools/webfetch.go:51`
- **Issue:** The `sync.Map` DNS cache grows unboundedly. Each unique hostname fetched stays in memory until process exit (5-minute TTL only prevents stale reads, not eviction).
- **Impact:** Memory leak over long sessions with many unique URLs.
- **Recommendation:** Add periodic cleanup or LRU eviction.

#### M-5: `Edit` Tool `replaceByLineRange()` Slice Append Safety
- **File:** `internal/tools/edit.go:310`
- **Issue:** `append(lines[:startIdx], append(..., lines[endIdx+1:]...)...)` may modify the underlying array of `lines` if `cap(lines) > startIdx + len(newContent) + len(remaining)`. While Go's `append` creates new backing arrays when capacity is exceeded, this pattern is fragile.
- **Impact:** Low — works correctly in practice but could be confusing to maintainers.
- **Recommendation:** Use explicit `make` + `copy` for clarity.

#### M-6: Workflow Engine `execCommand` Field Unused Outside Tests
- **File:** `internal/workflow/engine.go:82`
- **Issue:** The `execCommand` field (used for mocking `exec.Command` in tests) is initialized to `exec.Command` but never called in production code. The verify phase uses `bash` tool execution instead.
- **Impact:** Dead field in production, useful for testing.
- **Recommendation:** Keep for testability but add a comment.

#### M-7: `channelEmitter.Emit()` Blocking Timeout
- **File:** `internal/tui/app_channel.go:51-56`
- **Issue:** `Emit()` uses `time.After(ChannelSendTimeout * 2)` (1 second) as a blocking timeout. During high-throughput workflow events, this could cause a 1-second stall per dropped message.
- **Impact:** Minor latency spike during burst event emission.
- **Recommendation:** Consider using `select` with `default` for non-blocking sends.

### 10.4 LOW Issues

#### L-1: `tiktoken-go` Dependency Unmaintained
- **File:** `internal/tokens/estimator.go:14`
- **Issue:** Comment notes tiktoken-go is unmaintained since 2024. New tokenizers (o200k_base for GPT-4o) may not be recognized.
- **Mitigation:** Rune-based fallback with EMA calibration handles unsupported models.
- **Recommendation:** Monitor for maintained fork.

#### L-2: `config.toTOMLKey()` Doesn't Handle Numbers in Field Names
- **File:** `internal/config/loader.go:224-243`
- **Issue:** Field names with embedded numbers (e.g., `V2Config`) would produce unexpected TOML keys (`v2_config` vs `v_2_config`).
- **Impact:** Low — no current config fields use numbers.

#### L-3: `git.StatusPorcelain()` Quoted Path Handling
- **File:** `internal/git/git.go:331-333`
- **Issue:** Only handles paths starting with `"` but git may also quote paths with special characters using octal escapes (e.g., `\303\251` for UTF-8 filenames).
- **Impact:** Unusual filenames may not be parsed correctly.

#### L-4: `ledger.parseFile()` Scanner Buffer Limit
- **File:** `internal/ledger/ledger.go:409`
- **Issue:** `bufio.Scanner` has a default max token size of 64KB. A single very long ledger row could exceed this.
- **Impact:** Extremely unlikely given the fixed-column format.

#### L-5: `Edit` Tool Levenshtein Performance
- **File:** `internal/tools/edit.go:509-538`
- **Issue:** O(n*m) Levenshtein distance computation for fuzzy anchor matching. For very large edit blocks, this could be slow.
- **Mitigation:** Only triggered when all other matching strategies fail, and `MinLinesForFuzzy` gates minimum block size.

#### L-6: `rollback.HasUncommittedChanges()` String Matching Fragility
- **File:** `internal/rollback/rollback.go:204-226`
- **Issue:** Parses `git status` output via string matching instead of using `--porcelain`. Changes to git output format could break detection.
- **Recommendation:** Use `git status --porcelain` and check for empty output (as `git.HasUncommittedChanges()` does).

#### L-7: `WebFetch` HTML Entity Decoding Incomplete
- **File:** `internal/tools/webfetch.go:612-619`
- **Issue:** Only decodes 6 common HTML entities (`&amp;`, `&lt;`, `&gt;`, `&quot;`, `&#39;`, `&nbsp;`). Thousands of named entities exist.
- **Impact:** Low — most content uses these 6. Adding `golang.org/x/net/html` would be the proper fix but adds a dependency.

#### L-8: `session.ExportSessionMarkdown/JSON` Uses `os.WriteFile` Without Atomic Write
- **File:** `internal/session/manager.go:781, 794`
- **Issue:** Export functions use direct `os.WriteFile` instead of atomic write, risking partial files on crash.
- **Impact:** Low — export is a user-initiated operation, not critical path.

#### L-9: `Engine.consumeStream()` MaxLLMResponseBytes Check Off-by-One
- **File:** `internal/workflow/engine.go:560`
- **Issue:** Checks `sb.Len() > MaxLLMResponseBytes` which allows exactly `MaxLLMResponseBytes` + 1 byte before rejection (the `>` should be `>=`).
- **Impact:** Negligible — 1 byte over the limit.

---

## 11. Test Coverage Assessment

### Test File Distribution

| Package                  | Source Files | Test Files | Coverage Notes                        |
|--------------------------|-------------|------------|---------------------------------------|
| cmd/m31a/                | 2           | 0          | Integration tested via TUI            |
| internal/config/         | 2           | 1          | Config loading, validation, merging   |
| internal/errors/         | 1           | 1          | Error message mapping                 |
| internal/git/            | 1           | 1          | Git operations                        |
| internal/log/            | 1           | 1          | Log rotation                          |
| internal/provider/       | 8           | 6          | Cache, SSE, reasoning, resilience     |
| internal/tokens/         | 1           | 2          | Estimation, context warnings          |
| internal/tools/          | 15          | 12         | Bash security, file ops, permissions  |
| internal/tui/            | ~80         | ~5         | Component-level tests only            |
| internal/workflow/       | 11          | 10         | Engine, parsing, phases, streaming    |
| pkg/arbitrage/           | 2           | 1          | Cost scoring                          |
| pkg/autodream/           | 2           | 1          | Consolidation                         |
| pkg/bisect/              | 3           | 1          | Bisect execution                      |
| pkg/keychain/            | 5           | 1          | Keychain operations                   |
| pkg/ledger/              | 2           | 1          | Entry parsing, stats                  |
| pkg/rollback/            | 2           | 1          | Reset operations                      |
| pkg/session/             | 6           | 5          | Session CRUD, planning, checkpoints   |
| pkg/taskrunner/          | 2           | 1          | Scheduling, execution                 |

### Test Quality Assessment
- **Security tests:** `bash_security_test.go`, `webfetch_security_test.go`, `permissions_test.go` — dedicated security test suites
- **Edge case tests:** `bash_kill_test.go`, `grep_truncation_test.go`, `permission_timeout_test.go` — boundary condition testing
- **Integration tests:** `workflow/integration_test.go` — end-to-end workflow testing

---

## 12. Code Quality Metrics

### 12.1 Complexity
- Largest files: `dispatcher.go` (293 lines), `bash.go` (305 lines), `webfetch.go` (667 lines), `edit.go` (607 lines), `engine_parse.go` (613 lines), `engine.go` (620 lines), `manager.go` (806 lines)
- Average function length: well-bounded, most functions <50 lines
- Cyclomatic complexity: manageable, with complex functions (parseToolCalls, checkPermission) well-documented

### 12.2 Documentation
- Package-level documentation via doc comments on most packages
- Critical functions have detailed doc comments explaining behavior
- Architecture decisions documented in comments (e.g., CR-05 fix at execute.go:192)
- `doc.go` files in pkg/ subpackages

### 12.3 Naming Conventions
- Consistent Go naming conventions throughout
- Exported types use PascalCase, unexported use camelCase
- Sentinel errors prefixed with `Err`
- Message types suffixed with `Msg`

### 12.4 Code Organization
- Clean separation of concerns across packages
- Interface-based design enables testability (`LLMProvider`, `GitClient`, `Tool`, `PermissionGate`)
- Compile-time interface checks (`var _ Interface = (*Impl)(nil)`)
- Platform-specific code properly isolated via build tags

---

## 13. Dependency Analysis

### Direct Dependencies (per AGENTS.md allowed list)
| Dependency                    | Purpose               | Status     |
|-------------------------------|-----------------------|------------|
| charmbracelet/bubbletea       | TUI framework         | USED       |
| charmbracelet/lipgloss        | Terminal styling      | USED       |
| charmbracelet/bubbles         | TUI components        | USED       |
| charmbracelet/glamour         | Markdown rendering    | USED       |
| BurntSushi/toml               | Config parsing        | USED       |
| tiktoken-go                   | Token estimation      | USED (unmaintained) |
| doublestar                    | Glob patterns         | USED       |
| creack/pty                    | PTY for Bash          | NOT FOUND  |
| golang.org/x/sync/singleflight| Request deduplication | USED       |

### Note on creack/pty
- The `bash_unix.go` file does not appear to use `creack/pty` directly — it uses `os/exec` with process groups instead.
- This is a deviation from AGENTS.md but is functionally correct.

---

## 14. Operational Integrity

### 14.1 Signal Handling
- **Status:** CORRECT
- SIGTERM/SIGINT caught and forwarded as `tea.QuitMsg{}` via `p.Send()` (main.go:214-224)
- `sigDone` channel prevents goroutine leak after program exit
- Shutdown sequence: cancel contexts → stop dispatcher → close channels

### 14.2 Session Recovery
- **Status:** WELL-IMPLEMENTED
- Resume-on-startup reads most recent session (main.go:201-206)
- Checkpoint system saves phase state for crash recovery
- Fork operations deep-copy messages and project state
- Archived sessions moved to `archived/` subdirectory

### 14.3 Config Hot-Reload
- **Status:** IMPLEMENTED
- `WatchConfig()` polls config file every 5 seconds (loader.go:642-671)
- Non-blocking send with drop-on-full semantics
- Variable substitution supports `${VAR}` patterns

---

## 15. Summary & Recommendations

### Overall Assessment: **SOLID**

The M31A codebase demonstrates strong engineering practices:
- Clean architecture with proper separation of concerns
- Security-conscious design (SSRF prevention, API key protection, input validation)
- Thread-safe concurrent code following Bubble Tea's single-threaded model
- Comprehensive error handling with graceful degradation
- Well-bounded resource usage (memory, file handles, goroutines)

### Priority Recommendations

1. **M-4 (DNS Cache Unbounded):** Add LRU eviction or periodic cleanup to prevent memory growth in long sessions
2. **L-6 (Rollback Status Parsing):** Replace string-matching with `--porcelain` for robustness
3. **L-1 (tiktoken-go):** Monitor for maintained fork as new models ship with new tokenizers
4. **M-2 (DiffRefs Semantics):** Document or fix the diff argument ordering in `collectDiffStats()`

### What's Working Well
- SSRF protection in WebFetch is textbook-quality with DNS pinning + rebind prevention
- The six-phase workflow engine with self-healing and bisect fallback is robust
- Session persistence with atomic writes and crash recovery is production-ready
- The permission system with per-agent profiles and rule matching is well-designed
- The provider fallback chain with Retry-After awareness prevents Bubble Tea event loop stalling

---

*Report generated by deep codebase analysis of all ~200 Go source files across 18 packages.*

# Codebase Concerns

**Analysis Date:** 2026-07-16

## Tech Debt

### Large Files / God Objects

| File | Lines | Concern |
|------|-------|---------|
| `internal/workflow/engine.go` | 1,707 | Central workflow orchestrator with 7 phases, state machine, context builder, cost tracking, code intel, compaction, metrics, phase coordination. Difficult to test in isolation; high coupling. |
| `internal/tui/app.go` | 775 | Main TUI model holding all screens, providers, sessions, dispatchers, subagents, sidebar, file watcher, config watcher. Violates single responsibility; hard to unit test. |
| `internal/tools/dispatcher.go` | 1,496 | Tool registry, permission system, rate limiting (2 token buckets), concurrency limiting, batch approvals, persistent permissions, question routing, metrics collection. Too many responsibilities. |
| `internal/tools/permissions.go` | 691 | Permission rules, agent profiles, risk classification, rule matching, persistent storage. Could be split into rule engine, agent config, and storage. |
| `internal/tui/app_update.go` | 25,509 (with test files) | Massive Update() method handling all message types. Bubble Tea pattern but very long; hard to follow message flow. |
| `internal/workflow/execute.go` | 837 | Execute phase logic with task groups, tool execution, preflight, quality gates, loop detection, healing. Complex control flow. |

**Impact:** High coupling, difficult unit testing, long build times, onboarding friction.
**Fix approach:** Extract sub-components (e.g., `PhaseCoordinator`, `PermissionEngine`, `RateLimiter`, `ToolRegistry`) into separate files/packages. Use interfaces for testability.

---

### Coverage Boost Test File

**File:** `internal/workflow/coverage_boost_test.go` (111,227 bytes, 2,000+ lines)

**Issue:** A single test file explicitly designed to boost coverage metrics for `pkg/taskrunner`, `pkg/bisect`, `pkg/rollback` to meet the 90% threshold. Contains helper functions and table-driven tests that exercise code paths not covered by genuine tests.

**Impact:** Masks real coverage gaps; CI passes but genuine edge cases may be untested.
**Fix approach:** Write meaningful integration/unit tests for the target packages. Remove or refactor the boost file once real coverage reaches thresholds.

---

### Hardcoded Timeouts and Magic Numbers

**Files:** Multiple

| File | Constants | Concern |
|------|-----------|---------|
| `internal/types/constants.go` | `BashOutputLimit = 50000`, `MaxRetryAfterWait = 120`, `HealthCheckInterval = 30s` | Some values lack documentation; tuning requires code changes |
| `internal/tools/dispatcher.go` | `ToolRateLimitPerSec = 10`, `DangerousRateLimitPerSec = 2`, `MaxConcurrentTools = 10` | Hardcoded limits; no config override |
| `internal/workflow/engine.go` | `DefaultTimeoutSecs = 30 * 60` (30 min workflow timeout) | Not configurable per-session |
| `internal/tui/constants.go` | `SidebarRefreshInterval = 2s`, `EmitterDropLogInterval = 30s` | UI timing constants scattered |

**Impact:** Operational inflexibility; requires rebuild to adjust.
**Fix approach:** Move to config.toml with sensible defaults. Add `cfg.Features` or `cfg.Limits` section.

---

### Error Handling Inconsistencies

**Pattern 1: Error Wrapping**
```go
// Good: uses %w
return fmt.Errorf("failed to load config: %w", err)

// Inconsistent: some files use plain fmt.Errorf without wrapping
return fmt.Errorf("config validation: %s", err) // loses chain
```

**Pattern 2: Sentinel Errors vs. Custom Types**
- `pkg/errors/errors.go` defines `ToolError`, `ProviderError`, `ConfigError` with `Unwrap()`
- Some call sites check `errors.Is(err, ErrToolExecution)` — correct
- Others string-match: `strings.Contains(err.Error(), "permission")` — fragile

**Files with string matching:** `internal/workflow/engine_verify.go`, `internal/tools/dispatcher.go` (permission handling)

**Impact:** Error wrapping breaks; debugging harder; refactoring risky.
**Fix approach:** Enforce `fmt.Errorf("%w", err)` via linter. Replace string checks with `errors.As`/`errors.Is`.

---

### Configuration Drift Between Config and Code

**Config:** `config.toml` (user-facing) vs `internal/config/types.go` (struct definitions)

**Issues:**
- `cfg.Features.ResumeOnStartup` exists but no corresponding CLI flag
- `cfg.ModelCapabilities` has `KnownCapabilities` map but no validation against actual provider responses
- `cfg.Permissions.Rules` and `cfg.Permissions.Agents` are deeply nested; no schema validation beyond basic TOML decode

**Impact:** User configures feature that doesn't work; silent failures.
**Fix approach:** Add config validation pass that cross-references enabled features with available providers/capabilities. Use `cue` or `jsonschema` for config schema.

---

## Known Bugs

### 1. TUI Signal Handler Race (H-24)

**File:** `cmd/m31a/main.go:380-410`

**Symptoms:** On SIGTERM/SIGINT, a 5-second hard timeout forces `os.Exit(1)` if TUI doesn't quit. Writes `.force-exit` sentinel. Next startup detects sentinel and logs warning.

**Root cause:** Signal handler runs in separate goroutine; calls `p.Send(tea.QuitMsg{})` but TUI may be blocked in long-running command (e.g., tool execution). The 5s timeout is arbitrary.

**Workaround:** Users see "force-exit sentinel present" warning on next launch.

**Fix approach:** 
- Implement graceful shutdown protocol: `AppState.Shutdown()` sets `shutdownCtx` cancel, waits for in-flight tool executions (with timeout), then returns.
- Remove hard `os.Exit(1)`; let main return exit code.

---

### 2. Subagent Worktree Cleanup on Crash

**File:** `internal/tools/subagent/manager.go`, `cmd/m31a/main.go:310-315`

**Symptoms:** `subagent.Sweep()` runs at startup with 30s timeout to clean stale worktrees/branches. If sweep fails, orphaned worktrees accumulate.

**Root cause:** Subagent creation uses `git worktree add`; crash before cleanup leaves worktree. Sweep uses `git worktree list --porcelain` parsing which is brittle.

**Impact:** Disk space growth; `git worktree list` shows stale entries; potential branch name conflicts.

**Fix approach:** 
- Use `git worktree remove --force` with retry/backoff.
- Track worktree paths in a sidecar file (JSON) for reliable cleanup.
- Add `git worktree prune` to sweep.

---

### 3. Tool Permission Batch Approval Expiry

**File:** `internal/tools/dispatcher.go:300-350`

**Symptoms:** Batch approvals (`approve all future calls for tool:risk`) stored in `batchApprovals` map. Expire on "task completion or phase transition" per comment, but no explicit cleanup code found.

**Root cause:** `batchApprovals` map never cleared except on `Dispatcher.Stop()`. Phase transitions don't call a cleanup method.

**Impact:** Approvals persist across tasks/phases unexpectedly; security risk if user approved dangerous tool for one task.

**Fix approach:** Add `ClearBatchApprovals()` called from `PhaseCoordinator` on transition. Add TTL to batch approvals.

---

### 4. SSE Stream Truncation Handling

**File:** `internal/provider/base_client.go:200-250`, `internal/provider/sse.go`

**Symptoms:** Provider returns partial stream without `[DONE]` sentinel. `SSEParser.Next()` returns `io.EOF` or `io.ErrUnexpectedEOF`. Caller treats as error but may not distinguish truncation from transient network error.

**Root cause:** No explicit "stream truncated" sentinel error type. `ErrStreamTruncated` exists in `pkg/errors` but not consistently returned.

**Impact:** Retry logic may retry non-retryable truncation; user sees generic error.

**Fix approach:** Return `ErrStreamTruncated` from `SSEParser` when stream ends without `[DONE]`. Add retry policy: retry on network errors, not on truncation.

---

### 5. Model Capability Cache Staleness

**File:** `internal/provider/capabilities.go`, `internal/provider/cache.go`

**Symptoms:** `ModelCache` uses `singleflight` for refresh. Stale entries served while refresh runs. `StaleCacheTTL` (default 24h) allows serving stale data if refresh fails.

**Root cause:** `GetModel()` returns stale entry if refresh fails. No circuit breaker — repeated failed refreshes hammer provider.

**Impact:** Model capabilities (tool support, context window) may be wrong for hours. LLM gets incorrect tool schema.

**Fix approach:** 
- Add `maxStaleAge` config; hard-fail if stale exceeds threshold.
- Add circuit breaker (e.g., `gobreaker`) around provider calls.
- Emit metric on cache staleness.

---

## Security Considerations

### 1. SSRF Protection in WebFetch/WebSearch

**Files:** `internal/tools/webfetch.go`, `internal/tools/websearch.go`, `internal/tools/httpcheck.go`

**Status:** ✅ **Implemented** — DNS pinning, private IP blocklist (RFC1918, link-local, ULA, cloud metadata), redirect validation, connection verification.

**Gaps:**
- `allowPrivateIPs` config defaults to `false` (good), but no audit log when blocked.
- IPv6 zone IDs (`fe80::1%eth0`) not explicitly handled in `isPrivateIP`.
- No rate limiting per-target-host (could DoS internal services if bypassed).

**Recommendation:** Add structured log on SSRF block with target URL/IP. Consider per-host rate limit.

---

### 2. API Key Storage in OS Keychain

**Files:** `pkg/keychain/keychain.go`, `cmd/m31a/main.go:260-275`

**Status:** ✅ **Implemented** — Uses `libsecret` (Linux), Keychain (macOS), Credential Manager (Windows). Fallback to config file with warning.

**Gaps:**
- `keychain.NewCached()` wraps with in-memory cache but cache invalidation on key rotation not implemented.
- No key rotation support — user must manually re-enter via `/settings`.

**Recommendation:** Add `Keychain.Rotate(provider, newKey)` method. Invalidate cache on successful save.

---

### 3. Bash Tool Command Injection

**Files:** `internal/tools/bash.go`, `internal/tools/bash_security_test.go`

**Status:** ✅ **Mitigated** — Command blocklist (`rm -rf /`, `mkfs`, `fdisk`, `nc -l`, etc.), command substitution detection (`$(...)`, backticks), chaining detection (`;`, `&&`, `||`), timeout enforcement, output cap (50KB).

**Gaps:**
- Blocklist is allowlist-by-negation; new dangerous commands not covered.
- `bash -c "..."` with user-controlled input still risky if blocklist bypassed.
- No seccomp/bubblewrap sandbox on Linux (experimental `bash_sandbox_linux.go` exists but not enabled by default).

**Recommendation:** Enable Linux sandbox by default when available. Add `allowlist` mode (only permitted commands) as alternative to blocklist.

---

### 4. Git Operations Without Input Validation

**Files:** `internal/git/git.go`, `internal/tools/git.go`

**Concerns:**
- `git.Commit(message)` — message not sanitized; could inject `--amend` or other flags if user controls message.
- `git.Run(args...)` — passes args directly to `exec.Command`; no shell, but arg injection possible if user controls args.
- `git.DiffRefs(ref1, ref2)` — refs not validated; could be `HEAD; rm -rf /` (but `exec.Command` doesn't use shell).

**Status:** Low risk (no shell), but defense-in-depth suggests validation.

**Recommendation:** Validate ref names against `git check-ref-format`. Sanitize commit messages (strip leading `-`).

---

### 5. Path Traversal in File Tools

**Files:** `internal/tools/fileread.go`, `internal/tools/filewrite.go`, `internal/tools/filelist.go`, `internal/tools/filemove.go`, `internal/tools/filedelete.go`

**Status:** ✅ **Mitigated** — `filepath.Clean` + `filepath.IsLocal` (Go 1.20+) or custom `isSubPath` check. Working directory enforced as root.

**Gap:** Symlink resolution — `filepath.EvalSymlinks` not called before `isSubPath`. Symlink outside workdir pointing inside could bypass.

**Recommendation:** Call `filepath.EvalSymlinks` on target path before `isSubPath` check.

---

## Performance Bottlenecks

### 1. Workflow Engine Monolithic Context Building

**File:** `internal/workflow/engine.go` → `ContextBuilder.Build()`

**Issue:** `ContextBuilder` assembles full conversation history, plan, code intelligence, file contents, tool results, decisions, research output into single prompt. For large codebases/sessions, prompt size grows unbounded.

**Evidence:** `tokens.Estimator` tracks tokens; `compaction.Compactor` triggers at 80% context window. But compaction itself calls LLM (recursive).

**Impact:** High latency on phase transitions; token costs spike; context window exhaustion.

**Fix approach:** 
- Implement hierarchical context: summary + relevant snippets (RAG-style).
- Cache prompt fragments by hash; invalidate on file change.
- Add `maxContextTokens` config with hard truncation before compaction.

---

### 2. Code Intelligence Indexer Rebuilds

**File:** `internal/codeintel/codeintel.go`, `internal/workflow/engine.go:codeIntel`

**Issue:** `codeintel.Indexer` built lazily on first Execute phase. Invalidated between execute groups (`codeIntelBuilt = false`). For large repos, rebuild takes 10-30s.

**Evidence:** `Build()` parses all Go/TS/Python files with tree-sitter. No incremental update — full rebuild on invalidation.

**Impact:** Noticeable pause at start of each execute group.

**Fix approach:** 
- Implement incremental parsing (tree-sitter supports edits).
- Persist index to disk (`.m31a/codeintel/`); load on startup.
- Background rebuild on file watcher events.

---

### 3. Provider Model Fetch on Every Startup

**File:** `cmd/m31a/main.go:280-300`, `internal/provider/cache.go`

**Issue:** `syncReplProvider()` fetches model catalog from all registered providers sequentially at startup. Each `FetchModels()` hits network (or cache).

**Evidence:** `ModelCache` TTL 5 min; but cold start = 3 API calls (OpenRouter, Zen, NVIDIA) × ~500ms each.

**Impact:** 1.5-3s startup delay before TUI renders.

**Fix approach:** 
- Parallelize fetches with `errgroup`.
- Show TUI immediately with cached models; background refresh.
- Persist model catalog to disk (JSON); load instantly.

---

### 4. Tool Dispatcher Rate Limiter Contention

**File:** `internal/tools/dispatcher.go:45-120`

**Issue:** Two token buckets (`rateTokens`, `dangerousRateTokens`) + concurrency semaphore (`concurrencySem`). All tool calls contend on these channels.

**Evidence:** `MaxConcurrentTools = 10`; `ToolRateLimitPerSec = 10`. Under heavy parallel tool use (e.g., multiple Grep/Glob), goroutines queue on channel receives.

**Impact:** Latency variance; tail latency spikes.

**Fix approach:** 
- Use `golang.org/x/time/rate` Limiter (more efficient than channel token bucket).
- Separate limiters per tool category (read vs write vs network).
- Add `burst` config per tool.

---

### 5. TUI Sidebar Render on Every Keystroke

**File:** `internal/tui/sidebar_render.go`, `internal/tui/sidebar_model.go`

**Issue:** `SidebarModel.Render()` rebuilds entire sidebar (sessions, tasks, todos, git, files) on every `Msg`. `refreshCmd` fires every 2s.

**Evidence:** `sidebar_render.go` 783 lines; `Render()` calls 10+ sub-renderers.

**Impact:** High CPU on large projects (many files/sessions); input lag.

**Fix approach:** 
- Diff-based rendering: only re-render changed sections.
- Virtual scrolling for file tree.
- Debounce refresh (already 2s but could be adaptive).

---

## Fragile Areas

### 1. Phase Transition State Machine

**File:** `internal/workflow/state_machine.go`, `internal/workflow/engine.go`

**Why fragile:** 
- 7 phases with complex transition graph (Plan↔Discuss cycles, Execute→Verify→Runtime→Ship).
- `discussPlanCycles` counter prevents infinite loops but max is hardcoded (`maxDiscussPlanCycles = 3`).
- `SetPhase()` bypasses validation (used for checkpoint restore) — can put machine in invalid state.

**Safe modification:** 
- Add `ValidateState()` called after every transition in tests.
- Use property-based testing for transition graph.
- Document valid transitions in `STATE_MACHINE.md`.

**Test coverage:** `state_machine_test.go` exists but only tests happy paths.

---

### 2. Subagent Manager Concurrency

**File:** `internal/tools/subagent/manager.go`

**Why fragile:**
- `sync.Map` for agents (`agents sync.Map // id -> *Subagent`)
- `spawnMu sync.Mutex` for creation
- `Info` mutations protected by per-agent `mu sync.Mutex`
- Loop goroutine reads `Info` while manager may write

**Race potential:** `sync.Map` iteration not atomic with mutations. `Manager.List()` returns slice of pointers; caller may read while loop writes.

**Safe modification:** 
- Replace `sync.Map` with `map[string]*Subagent` guarded by single `mu sync.RWMutex`.
- Use `atomic.Pointer` for `Info` or copy-on-read.
- Add `-race` test with concurrent spawn/list/kill.

**Test coverage:** `manager_test.go` has basic tests; no stress test.

---

### 3. Plan Parser and Execution Coupling

**Files:** `internal/workflow/plan_parser.go`, `internal/workflow/execute.go`

**Why fragile:**
- `PlanParser` extracts tasks from markdown with regex/string matching.
- `Execute` phase expects specific task structure (`Task{ID, Description, Files, AcceptanceCriteria, ToolHints}`).
- Changes to plan format break execution silently (tasks missing fields → no-ops).

**Evidence:** `plan_parser.go` has `sectionHeaderCache` and `subsectionHeaderCache` (`sync.Map`) for performance — indicates parsing is hot path.

**Safe modification:** 
- Define `PlanSchema` (JSON schema) for plan format.
- Validate parsed plan against schema before execution.
- Add integration test: generate plan → parse → execute → verify structure.

---

### 4. Session Resume and Checkpoint Restore

**Files:** `internal/workflow/engine.go` (checkpointData), `pkg/session/manager.go`, `internal/tui/app_session.go`

**Why fragile:**
- `CheckpointData` serializes phase, goal, plan version, decisions.
- `Engine` restores by calling `Transition()` to target phase, then re-running phase.
- But `Messages`, `intentResult`, `researchOutput`, `cachedBasePrompt` not fully restored.
- Subagent state not checkpointed.

**Safe modification:** 
- Make `WorkflowState` fully serializable (all fields).
- Add `CheckpointVersion` to detect schema changes.
- Test: run workflow → checkpoint at each phase → kill → resume → verify same output.

**Test coverage:** `engine_wiring_test.go` has some resume tests; not comprehensive.

---

### 5. Git Worktree and Branch Management for Subagents

**Files:** `internal/tools/subagent/worktree.go`, `internal/tools/subagent/manager.go`

**Why fragile:**
- `GitWorktrees` creates worktrees in `.m31a/worktrees/<agent-id>/`
- Branches named `m31a/agent/<agent-id>`
- Cleanup on crash relies on `Sweep()` at startup
- No locking between concurrent subagents creating worktrees

**Risk:** Concurrent `git worktree add` can corrupt `.git/worktrees/` if not serialized. `git worktree list` parsing fragile.

**Safe modification:** 
- Serialize worktree creation with file lock (`.m31a/worktrees/.lock`).
- Use `git worktree add --lock` to prevent pruning.
- Track worktrees in SQLite/JSON instead of parsing `git worktree list`.

---

## Scaling Limits

### 1. Session Storage Growth

**File:** `pkg/session/manager.go`

**Current:** Sessions stored as JSON files in `~/.m31a/sessions/<project>/<session-id>.json`. Includes full message history.

**Limit:** No retention enforcement by default. `config.Features.SessionRetentionDays` defaults to 30 but cleanup only runs at startup (`AppState.Init()`).

**Impact:** Disk fills over months of heavy use. JSON parsing slows startup.

**Scaling path:** 
- Implement background compaction (already have `compaction.Compactor` for messages).
- Migrate to SQLite (`modernc.org/sqlite` — CGO-free) for indexing.
- Add `session prune` CLI command.

---

### 2. LEDGER.md File Growth

**File:** `pkg/ledger/ledger.go`

**Current:** Appends Markdown entries to `~/.m31a/LEDGER.md` (or project-local). No rotation.

**Limit:** File grows unbounded. `Ledger.Read()` loads entire file.

**Impact:** Memory/CPU spike on ledger read after months.

**Scaling path:** 
- Rotate monthly (`LEDGER-2026-01.md`, `LEDGER-2026-02.md`).
- Index by date/session for fast queries.

---

### 3. Provider Rate Limits

**File:** `internal/provider/fallback.go`, `internal/tools/dispatcher.go`

**Current:** 
- Provider-level: `FindFallbackProvider()` on 429/5xx with parallel health checks.
- Tool-level: Token bucket (10 req/s default, 2 req/s for dangerous).

**Limit:** 
- OpenRouter free tier: 20 req/min. Shared across all users of same key.
- No per-model quota tracking.
- No cost budget enforcement (only tracking via `CostTracker`).

**Scaling path:** 
- Add `CostBudget` per session/project (configurable).
- Implement provider-specific rate limit awareness (read headers).
- Queue requests with priority (user-interactive > background).

---

### 4. TUI Message Channel Buffer

**File:** `internal/tui/streaming/streaming.go:75`

**Current:** `StartStreamCmd` creates `chan tea.Msg` with buffer 64. One goroutine per stream writes; `Update()` reads.

**Limit:** If LLM generates faster than TUI renders (e.g., large code output), channel fills → goroutine blocks → backpressure to provider → potential timeout.

**Evidence:** `EmitterDropLogTick` logs dropped messages but doesn't prevent block.

**Scaling path:** 
- Increase buffer or make unbounded with `select { case ch <- msg: default: drop }`.
- Add flow control: pause stream on buffer high-water mark.

---

## Dependencies at Risk

| Package | Version | Risk | Migration Path |
|---------|---------|------|----------------|
| `github.com/charmbracelet/bubbletea` | v1.3.0 | Major version lag (v2 exists) | Test v2 compatibility; breaking changes in `tea.Msg` handling |
| `github.com/charmbracelet/bubbles` | v0.20.0 | Same as above | Bundle with Bubble Tea upgrade |
| `github.com/odvcencio/gotreesitter` | v0.20.5 | Tree-sitter Go bindings; CGo-free but may lag upstream | Monitor for `tree-sitter-go` updates; consider `github.com/tree-sitter/go-tree-sitter` |
| `github.com/pkoukk/tiktoken-go` | v0.1.8 | Token estimation; not official OpenAI lib | Migrate to `github.com/openai/openai-go` when stable |
| `golang.org/x/sync` | v0.21.0 | Singleflight used in `ModelCache`; stable | Low risk; stdlib `sync` may add in Go 1.25+ |
| `github.com/godbus/dbus/v5` | v5.2.2 | Linux keychain (libsecret); requires system D-Bus | Fallback to file-based keyring already implemented |

---

## Missing Critical Features

### 1. Offline Mode / Air-Gapped Support

**Gap:** All providers require network. No local LLM support (Ollama, llama.cpp).
**Impact:** Cannot use in restricted environments.
**Effort:** Add `internal/provider/local/` with Ollama/llama.cpp client. Implement model discovery via local API.

### 2. Multi-User / Team Collaboration

**Gap:** Sessions, config, keychain are per-user. No shared sessions, no team config sync.
**Impact:** Single-player only.
**Effort:** Add `session share` command; central config store (Git repo or server).

### 3. Plugin / Custom Tool System

**Gap:** 18 built-in tools registered in `internal/tools/defaults.go`. No dynamic loading.
**Impact:** Users cannot add domain-specific tools without forking.
**Effort:** Define `ToolPlugin` interface; load `.so` plugins (Go plugins) or WASM modules.

### 4. Telemetry / Observability Export

**Gap:** `pkg/metrics/collector.go` collects in-memory only. No Prometheus, OTLP, or file export.
**Impact:** No production visibility.
**Effort:** Add `metrics.Exporter` interface; implement Prometheus pushgateway / file JSON.

---

## Test Coverage Gaps

**Target packages (90% required):**
| Package | Current Est. | Gaps |
|---------|--------------|------|
| `pkg/taskrunner` | ~85% (boosted) | Task dependency graph cycles, parallel execution limits, timeout propagation |
| `pkg/bisect` | ~88% (boosted) | `bisect run` with flaky test, skip logic, visualization |
| `pkg/rollback` | ~92% | `SafeReset` stash pop conflicts, backup branch cleanup, large diff handling |

**Other low-coverage areas:**
- `internal/tui/app.go` — integration only, no unit tests
- `internal/workflow/engine.go` — integration tests only; `RunPhase` not unit-testable due to dependencies
- `internal/tools/subagent/loop.go` — no tests for agent loop error recovery
- `internal/provider/fallback.go` — fallback logic tested but not health check concurrency edge cases

---

*Concerns audit: 2026-07-16*
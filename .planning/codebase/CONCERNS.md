# CONCERNS.md — Technical Debt & Risks

> Last mapped: 2026-07-10

---

## Critical Concerns

### CGO_ENABLED=0 Hard Constraint
- **Location**: `Makefile:13`, `.goreleaser.yaml:7`
- **Risk**: Any transitive dependency requiring CGO breaks the build (static binary requirement). The `github.com/godbus/dbus/v5` dependency (used for Linux keychain D-Bus Secret Service) is pure Go but could introduce CGO transitively if dependencies change.
- **Impact**: Blocks cross-compilation and releases if a dependency adds CGO.
- **Monitoring**: `make check` and CI build matrix (linux/amd64, linux/arm64, darwin/amd64, darwin/arm64, windows/amd64) catch this. Windows ARM64 explicitly excluded (`.goreleaser.yaml:26`).
- **Mitigation**: Audit new dependencies for CGO requirements before adding. Use `go mod graph` to check transitive deps.

### Provider Model Discovery is Dynamic — No Fallback Catalog
- **Location**: `internal/provider/openrouter/client.go:86-133`, `internal/provider/nvidia/client.go`, `internal/provider/zen/client.go:165`
- **Risk**: Provider APIs can change model lists, remove models, or return errors at runtime. No embedded fallback model list exists.
- **Impact**: If all providers fail to fetch models, the TUI model picker shows empty list; workflows cannot start without a model.
- **Current mitigation**: `ModelCache.StaleFallback()` (`internal/provider/cache.go:130`) returns stale cached models on fetch failure. Cache TTL: 5 min (`config/loader.go:85`), stale TTL: 24 hours.
- **Gap**: First run with no cache and no network = unusable. No hardcoded fallback models.

### API Keys Fall Back to Environment Variables in CI
- **Location**: `internal/config/loader.go:960-976`, `e2e_test.go:60, 84, 113`
- **Risk**: CI/CD uses plain env vars (`NVIDIA_API_KEY`, `ZEN_API_KEY`, `OPENROUTER_API_KEY`). Keychain (OS secure storage) is bypassed when unavailable.
- **Status**: Keychain used locally (`pkg/keychain/`); env vars used in CI and when keychain fails. Documented in `docs/CONFIG.md` and `docs/PROVIDERS.md`.
- **Impact**: Secrets in CI logs if not masked; no audit trail for CI key usage.
- **Mitigation**: GitHub Actions masks secrets; ensure `actions/upload-artifact` doesn't leak them.

---

## High Concerns

### Bubble Tea Single-Threaded Model — Goroutine Mutation Risk
- **Location**: `internal/tui/app.go`, `internal/workflow/engine.go`
- **Risk**: Bubble Tea (Elm architecture) requires all state mutations through `Update(msg tea.Msg)`. Goroutines **must** send messages via channels (`tea.Cmd`), never mutate `AppState` directly.
- **Violations to audit**:
  - `internal/tui/app.go:143-150` — `startFileWatcher()` spawns goroutine reading from channel, returns `tea.Cmd` ✅ correct
  - `internal/tui/app.go:640-660` — `startConfigWatcher()` spawns goroutine, forwards via channel ✅ correct
  - `internal/workflow/engine.go:341-372` — `RunPhaseCmd` runs engine in goroutine, returns result via `tea.Cmd` ✅ correct
  - `internal/tui/app.go:522-537` — `wireTodoWriteCallback` uses bounded retry on channel send, drops on full channel ⚠️ data loss possible under load
- **Pattern**: All goroutines use `must use `chan tea.Msg` or `tea.Cmd`. `sync.Mutex` on `AppState` fields is a red flag.

### BUG-17: Provider Model Cache Race Condition (Documented)
- **Location**: `internal/provider/cache.go:47-51`
- **Issue**: `singleflight.Group` deduplicates refresh calls. The `refreshing` atomic flag is set/cleared **only by the goroutine that executes `fetchFn`**. Waiters never touch it — correct by design but fragile if `singleflight` behavior changes.
- **Impact**: If `refreshing` flag logic diverges, cache could report stale data as fresh.
- **Status**: Documented as BUG-17; current implementation is sound but requires careful review on any `singleflight` changes.

### BUG-29: Token Estimation Underestimates Tool-Heavy Conversations
- **Location**: `internal/tokens/estimator.go:319-334`
- **Issue**: `EstimateMessages` now accounts for per-message overhead (4 tokens) and tool call input JSON, but tool call **output** tokens and system prompt tokens are not included.
- **Impact**: Preflight context checks (`internal/workflow/execute_preflight.go`) may allow requests that exceed model context window.
- **Mitigation**: EMA calibration factor (`emaFactor`) adapts over time, but cold-start estimates can be low.

### Test Coverage Gaps — Multiple Packages Below 90% Target
- **Target**: 90% for `pkg/taskrunner`, `pkg/bisect`, `pkg/rollback` (per AGENTS.md)
- **Current coverage** (`go test -cover ./...`):

| Package | Coverage | Status |
|---------|----------|--------|
| `pkg/rollback` | 72.5% | 🔴 **Below 90% target** |
| `pkg/taskrunner` | 89.0% | 🟡 Near target |
| `pkg/bisect` | 89.0% | 🟡 Near target |
| `pkg/keychain` | 17.9% | 🔴 Very low (OS-specific, hard to test) |
| `pkg/ledger` | 88.9% | 🟡 Near target |
| `pkg/narrative` | 82.5% | 🟡 Below target |
| `pkg/retry` | 85.3% | 🟡 Below target |
| `pkg/session` | 75.3% | 🟡 Below target |
| `internal/workflow` | 74.8% | 🔴 **Core engine, below target** |
| `internal/tools` | 75.4% | 🔴 **18 tools, below target** |
| `internal/tui` | 27.7% | 🔴 Very low (TUI hard to test) |
| `internal/tui/commands` | 44.8% | 🔴 |
| `internal/tui/components` | 56.9% | 🔴 |
| `internal/tui/streaming` | 33.2% | 🔴 |
| `internal/codeintel` | 58.1% | 🔴 |
| `internal/provider` | 85.0% | 🟡 |
| `internal/provider/nvidia` | 0.0% | 🔴 No tests |
| `internal/provider/openrouter` | 84.1% | 🟡 |
| `internal/config` | 87.7% | 🟡 |
| `internal/git` | 73.8% | 🔴 |

- **E2E tests**: Require real API keys (`NVIDIA_API_KEY`, `ZEN_API_KEY`, `OPENROUTER_API_KEY`) — skipped in CI (`e2e_test.go:54, 78, 107`).

### Known Bugs (Documented as BUG-XX)
| ID | Location | Description |
|----|----------|-------------|
| BUG-01 | `internal/git/git.go:451` | Channel ordering in concurrent `git status` + `git diff --numstat` — fixed by separate result vars |
| BUG-04 | `internal/workflow/classify_test.go:12` | Multiple framework indicators (e.g., Go + Node) — classifier test exists |
| BUG-05 | `internal/workflow/classify_test.go:42` | Multiple lock files (go.mod + package.json) — package manager detection |
| BUG-06 | `internal/tools/concurrency.go:48` | `sync.Map` comma-ok assertions (fixed, documented) |
| BUG-07 | `internal/tools/concurrency.go:48` | `sync.Map` comma-ok assertions (fixed, documented) |
| BUG-10 | `internal/workflow/ship.go:118` | Commit skipped when working tree clean after `AddAll` — fixed by `DiffStaged()` check |
| BUG-12 | `internal/workflow/engine.go:725` | Infinite Plan↔Discuss oscillation — capped at 3 cycles (`maxDiscussPlanCycles`) |
| BUG-15 | `internal/workflow/ship.go:347` | Diff stats heuristic — fixed by `git status --porcelain` on range |
| BUG-17 | `internal/provider/cache.go:47` | `refreshing` flag only set by fetch executor (see High concern above) |
| BUG-18 | `internal/config/loader.go:989` | Config reload message drop — fixed by blocking send with retry |
| BUG-19 | `internal/tools/concurrency.go:48` | `sync.Map` comma-ok assertions (fixed, documented) |
| BUG-29 | `internal/tokens/estimator.go:320` | Token underestimation for tool-heavy conversations |

---

## Medium Concerns

### Secret Handling — Keychain Unavailable on Headless Linux
- **Location**: `pkg/keychain/keychain_linux.go:41-56`, `pkg/keychain/keychain.go:56-65`
- **Issue**: D-Bus Secret Service requires running `dbus-daemon` and a Secret Service provider (gnome-keyring, kwallet). On headless servers/CI, `NewCached()` wraps keychain and marks unavailable after first failure (`cachedKeychain.unavailable.Store(true)`).
- **Fallback**: `pass` CLI (GPG-based) if installed and initialized.
- **Gap**: No fallback if both D-Bus and `pass` unavailable — keychain returns `ErrKeychainUnavailable`, config loader falls back to env vars silently (`internal/config/loader.go:960-976`).
- **Impact**: API keys stored in plaintext env vars on headless systems.

### Windows Keychain Uses Unsafe Pointer Arithmetic
- **Location**: `pkg/keychain/keychain_windows.go:55-71`, `pkg/keychain/keychain_windows.go:101-107`
- **Risk**: `unsafe.Pointer` conversions for Win32 `CredReadW`/`CredWriteW`. Memory layout must exactly match Win32 `CREDENTIAL` struct. Any Go version change affecting struct layout could corrupt memory.
- **Mitigation**: Compile-time checks limited; runtime testing on Windows required.

### Error Handling — Sentinel Errors vs Wrapped Errors
- **Location**: `internal/errors/errors.go`, throughout codebase
- **Pattern**: Sentinel errors (`ErrProviderUnreachable`, `ErrInvalidKey`, etc.) wrapped with `fmt.Errorf("%w", err)`. `UserMessage()` uses `errors.Is`/`errors.As` for user-friendly messages.
- **Gap**: Not all call sites use `errors.Is`/`As` — some compare error strings (brittle). Example: `internal/tools/webfetch.go` checks `strings.Contains(err.Error(), "404")`.
- **Recommendation**: Enforce `errors.Is`/`As` via linter (`errorlint` not in golangci-lint config).

### Provider Fallback Chain — No Health-Based Reordering
- **Location**: `internal/provider/registry.go`, `internal/config/loader.go:29-32`
- **Config**: `ProviderConfig.FallbackPriority: ["nvidia", "zen", "openrouter"]`
- **Issue**: Fallback order is static. If primary provider is unhealthy (rate limited, no credits), the app tries next in list but doesn't reorder based on health.
- **Impact**: Repeated failures on unhealthy primary before falling back.
- **Mitigation**: Health check runs on startup (`internal/tui/app.go:86`), but not continuously during workflow.

### Workflow Engine — Phase Transition Mutex Contention
- **Location**: `internal/workflow/engine.go:44-45`, `internal/workflow/engine.go:733`
- **Issue**: `WorkflowState.transitionMu` serializes all phase transitions. Long-running phases (Execute) block other transitions.
- **Impact**: If `Execute` phase takes 10 minutes, `Ship` cannot start even if ready. No timeout on transition lock.

### Subagent Manager — Spawn Rate Limiting but No Global Cap
- **Location**: `internal/tools/subagent/manager.go`, `internal/tools/subagent/loop.go`
- **Config**: `Features.MaxParallelTasks` (default 8, `config/loader.go:128`)
- **Gap**: Subagent spawn rate limited (`spawnMu` + token bucket), but no hard cap on total concurrent subagents across session. Could exhaust file descriptors or memory on large tasks.

---

## Low Concerns

### Go Version Pinning — 1.25.0 Required
- **Location**: `go.mod:3`, `.github/workflows/ci.yml:22, 50, 87`, `Makefile` (implied)
- **Risk**: Go 1.25.0 is very new (released ~Aug 2025). If CI runner lacks it, builds fail. `go.mod` enforces minimum but not maximum.
- **Mitigation**: CI uses `actions/setup-go@v5` with `go-version: "1.25"`.

### Dependency Freshness — Some Indirect Deps Old
- **Location**: `go.mod:28-56`
- **Examples**: `github.com/erikgeiser/coninput v0.0.0-20211004...` (4+ years), `github.com/muesli/termenv v0.16.0` (2023).
- **Risk**: Unmaintained transitive deps may have vulnerabilities or incompatibilities.
- **Mitigation**: `govulncheck` runs in CI (`.github/workflows/ci.yml:68-70`).

### Bubble Tea v1 — Breaking Changes from v0
- **Location**: `go.mod:9` (`github.com/charmbracelet/bubbletea v1.3.0`)
- **Risk**: Major version upgrade; TUI code may use deprecated APIs. Codebase appears updated but full audit needed.

### TUI Layout Package — Complex, Low Coverage
- **Location**: `internal/tui/layout/` (coverage 91.3% but `layout_test.go` only)
- **Risk**: Custom constraint solver (`solver.go`) with responsive breakpoints. Edge cases on terminal resize not fully tested.

### Configuration Watcher — fsnotify Fallback to Polling
- **Location**: `internal/config/loader.go:1013-1061`
- **Issue**: If `fsnotify` fails (e.g., no inotify on container), falls back to 500ms polling (`ConfigWatchInterval`). Polling wakes CPU unnecessarily.
- **Impact**: Minor battery drain on laptops; negligible on servers.

### Magic Numbers / Hardcoded Timeouts
- **Location**: Scattered
- **Examples**:
  - `internal/tui/app.go:65` — `HealthCheckInterval = 30 * time.Second`
  - `internal/tui/app.go:66` — `SidebarRefreshInterval = 5 * time.Second`
  - `internal/workflow/engine.go:108` — `startTime` for cost tracking
  - `internal/provider/cache.go:15` — `DefaultCacheRefreshInterval = 5 * time.Minute`
- **Risk**: Tuning requires code change; not configurable via `config.toml`.

### Internal/Pkg Boundary — Enforced by Module System
- **Location**: `go.mod`, directory structure
- **Rule**: `internal/` packages cannot be imported outside module. `pkg/` packages are public API.
- **Current**: `pkg/keychain`, `pkg/taskrunner`, `pkg/bisect`, `pkg/rollback` are public but have low test coverage.
- **Risk**: External consumers (if any) depend on unstable internals.

---

## Informational

### Architectural Debt
| Area | Concern | Location |
|------|---------|----------|
| Provider Interface | `LLMProvider` interface in `internal/provider/interface.go` is large (10+ methods). Could split into `ModelFetcher`, `ChatCompleter`, `HealthChecker`. | `internal/provider/interface.go` |
| Tool Permissions | Permission rules in `internal/tools/permissions.go` are string-based (glob patterns). No structured policy engine. | `internal/tools/permissions.go` |
| Workflow State | `WorkflowState` struct (`internal/workflow/engine.go:43-79`) has 30+ fields — consider splitting into phase-specific state objects. | `internal/workflow/engine.go` |
| Cost Tracking | `CostTracker` (`internal/workflow/cost_tracker.go`) uses `float64` for USD — floating point precision issues over long sessions. | `internal/workflow/cost_tracker.go` |

### Cross-Cutting Concerns Not Fully Addressed
- **Structured logging**: Uses `slog` with `slog.Attr` but no correlation IDs across workflow phases.
- **Metrics**: `pkg/metrics/collector.go` collects but no Prometheus/OpenTelemetry exporter.
- **Distributed tracing**: None — workflow phases, tool calls, LLM requests not traced.

### File References
- `Makefile` (lines 13, 48, 162-182, 239)
- `.goreleaser.yaml` (lines 7, 26)
- `go.mod` (lines 3, 9, 12, 15-16, 28-56)
- `internal/config/loader.go` (lines 29-32, 85, 960-976, 989-1007)
- `internal/provider/registry.go` (lines 11-116)
- `internal/provider/cache.go` (lines 13-139)
- `internal/provider/openrouter/client.go` (lines 86-133)
- `internal/provider/nvidia/client.go`
- `internal/provider/zen/client.go` (line 165)
- `internal/tokens/estimator.go` (lines 319-334)
- `internal/errors/errors.go` (lines 9-274)
- `internal/workflow/engine.go` (lines 44-45, 341-372, 725-728)
- `internal/workflow/ship.go` (lines 118, 347)
- `internal/git/git.go` (line 451)
- `internal/tui/app.go` (lines 65-66, 143-150, 522-537, 640-660)
- `internal/tools/subagent/manager.go`
- `internal/tools/subagent/loop.go`
- `internal/tools/concurrency.go` (lines 28-81)
- `internal/tools/permissions.go`
- `internal/tools/dispatcher.go` (lines 522-537)
- `pkg/keychain/keychain.go` (lines 38-87)
- `pkg/keychain/keychain_linux.go` (lines 41-56, 120-136)
- `pkg/keychain/keychain_windows.go` (lines 55-71, 101-107)
- `pkg/keychain/keychain_darwin.go` (lines 36-55)
- `e2e_test.go` (lines 53-126)
- `.github/workflows/ci.yml` (lines 22, 50, 68-70, 87)

---

## Severity Summary

| Severity | Count | Items |
|----------|-------|-------|
| CRITICAL | 3 | CGO constraint, dynamic provider models, CI secret fallback |
| HIGH | 4 | Bubble Tea threading, BUG-17 cache race, BUG-29 token estimation, test coverage gaps |
| MEDIUM | 7 | Keychain headless Linux, Windows unsafe pointers, error handling patterns, static fallback chain, phase transition mutex, subagent cap, config watcher polling |
| LOW | 6 | Go version pin, old transitive deps, Bubble Tea v1, TUI layout complexity, magic numbers, internal/pkg boundary |
| INFO | 5 | Architectural debt (provider interface, tool permissions, workflow state, cost tracking, observability) |

---

*Concerns audit: 2026-07-10*
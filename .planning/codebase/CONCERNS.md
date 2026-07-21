# Technical Concerns & Debt

**Analysis Date:** 2026-07-21

---

## Known Issues & TODOs (BUG-XX Tracked)

The codebase maintains a numbered bug tracking system referenced in code comments. Key documented issues:

| Bug ID | Location | Description | Status |
|--------|----------|-------------|--------|
| **BUG-01** | `internal/git/git.go:451` | Channel ordering issue in git command execution — mutex used to prevent race between concurrent git operations | Mitigated with mutex |
| **BUG-04** | `internal/workflow/classify_test.go:12` | Multiple framework indicators in a project (e.g., both `go.mod` and `package.json`) cause ambiguous package manager detection | Test documents expected behavior |
| **BUG-05** | `internal/workflow/classify_test.go:42` | Multiple lock files exist — returned package manager must be deterministic | Test documents expected behavior |
| **BUG-06** | `internal/provider/capabilities.go:119`<br>`internal/tools/concurrency.go:37` | Data races from value-type map replacement in sync.Map usage — fixed via typed mutex-guarded map | Fixed with mutex-guarded map |
| **BUG-07** | Same as BUG-06 | Same root cause as BUG-06 — sync.Map comma-ok assertions required | Fixed |
| **BUG-10** | `internal/workflow/ship.go:118` | Git commit skipped when working tree clean after `AddAll` — fixed by using `DiffStaged` to check index vs HEAD | Fixed in `runShipPhase` |
| **BUG-12** | `internal/workflow/engine.go:894` | Infinite oscillation between Plan↔Discuss phases — capped at 3 cycles (`maxDiscussPlanCycles = 3`) | Mitigated with cycle limit |
| **BUG-15** | `internal/workflow/ship.go:347` | Numstat-based heuristic misclassified add-only modifications — fixed by using `git status --porcelain` and `--diff-filter` | Fixed in `collectDiffStats` |
| **BUG-17** | `internal/provider/cache.go:47` | Cache stampede prevention — waiters must not touch stale entry while fetchFn runs | Documented pattern |
| **BUG-18** | `internal/config/loader.go:620` | Config reload message could be dropped silently if channel full — now blocks with timeout fallback | Fixed in `sendReload` |
| **BUG-19** | `internal/tools/concurrency.go:50` | Related to BUG-06/07 — sync.Map comma-ok type assertions required | Fixed |
| **BUG-29** | `internal/tokens/estimator.go:400` | Token estimation may underestimate tool-heavy conversations, allowing requests exceeding model context window | Documented in `EstimateMessages` comment |

**TODO/FIXME Markers** (grep for `TODO\|FIXME\|HACK\|XXX` in `*.go` excluding tests):
- `internal/tools/subagent/loop.go:50` — `// SECURITY: Add prompt injection defenses`
- `internal/workflow/ship_preflight.go:43-49` — Detects TODO/FIXME/HACK/XXX/BROKEN markers in changed files as warnings
- `internal/workflow/engine_verify.go:276` — Checks for placeholder signals (TODO, FIXME, XXX, PLACEHOLDER, lorem ipsum) during verification

---

## Security Concerns

### Secrets Management

**API Key Storage:**
- Keys stored in OS keychain via `internal/keychain/` — never written to disk in plaintext
- Platform implementations:
  - **macOS** (`keychain_darwin.go`): `/usr/bin/security` CLI with `-w -` (stdin) to avoid command-line exposure
  - **Linux** (`keychain_linux.go`): D-Bus Secret Service (primary) → `pass` CLI (fallback) with service name validation (`^[a-z0-9][a-z0-9-]*$`) to prevent command injection
  - **Windows** (`keychain_windows.go`): Windows Credential Manager via `advapi32.dll` (CredReadW/CredWriteW/CredDeleteW) with UTF-16 string handling
- Key format: `m31a/{provider}` (e.g., `m31a/openrouter`, `m31a/zen`, `m31a/nvidia`)
- Keychain interface in `internal/keychain/keychain.go:13-28` with `Get`, `Set`, `Delete` methods
- Cached availability tracking in `cachedKeychain` (`keychain.go:42-87`) to suppress repeated D-Bus/pass connection attempts

**Environment Files:**
- `.env` files gitignored (only `.env.example` committed)
- `.env.test` exists for test configuration (gitignored)
- API keys never logged — `internal/logging/audit.go` provides `RedactSecrets` and `SanitizeLogValue` to mask `sk-*`, bearer tokens, and 32+ char hex strings

**Secret Scanning (Pre-commit / Ship Phase):**
- `internal/workflow/ship_preflight.go:66-83` scans changed files for hardcoded secret patterns:
  - `sk-` (OpenAI/Stripe)
  - `ghp_` (GitHub PAT)
  - `glpat-` (GitLab PAT)
  - `xoxb-` (Slack bot token)
  - `AKIA` (AWS access key)
- Fails ship if secrets detected (`result.Passed = false`)

**Audit Logging:**
- `internal/logging/audit.go:AuditLogForSecrets` — static analysis tool scanning Go source for log statements that might leak secrets (variables named `key`, `secret`, `token`, `password`, `credential`, `auth` without redaction)
- `SanitizeLogValue` redacts values for sensitive keys at log time

### Prompt Injection Risk
- **Location:** `internal/tools/subagent/loop.go:50` — explicit TODO: `// SECURITY: Add prompt injection defenses`
- Subagent runs in isolated goroutine with system prompt built in `buildSystemPrompt()` (not yet reviewed for injection hardening)
- User prompt passed directly as first user message without sanitization

### Command Injection Prevention
- Linux keychain: `validServiceName` regex (`^[a-z0-9][a-z0-9-]*$`) validated before passing to `pass` CLI (`keychain_linux.go:22, 34-39, 122`)
- macOS keychain: `validServiceName` regex (`^[a-z]+$`) before `security` CLI (`keychain_darwin.go:17, 29-34`)
- Windows keychain: Same validation before Win32 API calls (`keychain_windows.go:16, 18-23`)
- Tool dispatcher: Uses `exec.Command` with separate args (not shell) — `internal/tools/dispatcher.go` and fileops use direct argv

---

## Technical Debt

### CGO Constraint (Hard Requirement)
- **File:** `go.mod:3` — `go 1.25.0`
- **Build:** `Makefile:13` — `CGO_ENABLED=0` mandatory (static binary)
- **Risk:** Any dependency introducing CGO breaks the build. Currently all deps are pure Go.
- **Impact:** Cannot use libraries requiring C bindings (e.g., certain SQLite drivers, some crypto libs)

### Bubble Tea Single-Threaded Architecture
- **Location:** `cmd/m31a/main.go`, `internal/tui/`
- **Constraint:** TUI follows Elm architecture — all state mutations through `Update()` only
- **Rule:** Never mutate `AppState` from goroutines. Use `tea.Cmd` / `tea.Msg` for cross-goroutine communication
- **Evidence:** `internal/tui/app_update.go`, `internal/tui/app_update_commands.go` — all async operations return `tea.Cmd`
- **Risk:** Accidental shared mutable state from goroutines causes data races (race detector catches this)

### Dynamic Provider Model Lists
- **Location:** `internal/provider/{openrouter,zen,nvidia}/client.go`
- **Rule:** Never hardcode model names — discovered dynamically from provider `/models` endpoints
- **Cache:** `internal/provider/cache.go` with stampede protection (BUG-17)
- **Capability Inference:** `internal/provider/capabilities.go` heuristics from model ID (tools, vision, reasoning, JSON mode)

### Token Estimation Accuracy (BUG-29)
- **File:** `internal/tokens/estimator.go:393-414`
- **Issue:** `EstimateMessages` accounts for per-message overhead (~4 tokens) and tool call input JSON, but provider-specific tokenization of tool schemas/definitions not modeled
- **Impact:** Preflight context checks may underestimate, allowing requests that exceed context window
- **Mitigation:** EMA calibration (`Calibrate` method) adjusts factor from actual vs estimated usage

---

## Fragile Areas

### Cross-Compilation Targets
- **File:** `Makefile:24-25`, `.goreleaser.yaml`
- **Targets:**
  - `linux/amd64`, `linux/arm64`
  - `darwin/amd64`, `darwin/arm64`
  - `windows/amd64` (✅)
  - `windows/arm64` (❌ EXCLUDED — goreleaser config skips)
- **Build:** `make cross` or `goreleaser release --snapshot --clean`
- **Risk:** Platform-specific code in `internal/keychain/` (build tags) and `internal/fileutil/lock_{unix,windows}.go` must compile on all targets

### Test Coverage Gaps
- **Overall Target:** 75% (per `AGENTS.md`)
- **Critical Packages (90% target):**
  - `pkg/taskrunner` — `internal/taskrunner/`
  - `pkg/bisect` — `internal/bisect/`
  - `pkg/rollback` — `internal/rollback/`
- **Current Large Test Files (coverage boost):**
  - `internal/tools/extra_test.go` — 4,391 lines
  - `internal/workflow/coverage_boost_test.go` — 3,582 lines
  - `internal/workflow/engine_extra_test.go` — 3,410 lines
  - `internal/tui/components/extra_test.go` — 1,703 lines
  - `internal/rollback/rollback_test.go` — 1,198 lines
- **Race Detector Tests:** Dedicated race test files:
  - `internal/workflow/engine_race_test.go`
  - `internal/workflow/messages_race_test.go`
  - `internal/workflow/plan_race_test.go`
  - `internal/git/git_extra_test.go` (race-related)

### E2E Tests — External Dependencies
- **File:** `e2e_test.go`
- **Real API Tests** (skipped without env vars):
  - `TestBinary_Prompt_NvidiaRealAPI` — needs `NVIDIA_API_KEY`
  - `TestBinary_Prompt_ZenRealAPI` — needs `ZEN_API_KEY`
  - `TestBinary_Prompt_OpenRouterRealAPI` — needs `OPENROUTER_API_KEY`
- **Timeout Test:** `TestBinary_Prompt_Timeout` uses dummy key, expects auth error or timeout
- **Risk:** CI runs without API keys → real API paths untested in automation

### Configuration Loading
- **Files:** `internal/config/loader.go` (785 lines), `internal/config/merge.go`, `internal/config/validate.go`
- **Sources (priority):** defaults → `~/.config/m31a/config.toml` → `.env` → env vars → CLI flags
- **Hot Reload:** `WatchConfig` uses `fsnotify` with 50ms debounce, falls back to polling
- **BUG-18 Fix:** Config reload message never dropped silently — blocks until delivered or context cancelled (`loader.go:621-638`)

### Git Operations
- **File:** `internal/git/git.go` (827 lines)
- **BUG-01:** Channel ordering issue mitigated with mutex around `git.Run` calls
- **Operations:** `Run`, `DiffStaged`, `LogSince`, `Add`, `AddAll`, `CommitStaged`, `HeadHash`, `HasUncommittedChanges`, `RevParse`
- **Risk:** Shell command injection — uses `exec.Command` with separate args (safe), but git args constructed from user input in some paths

---

## Performance Concerns

### Static Binary Constraint
- `CGO_ENABLED=0` prevents use of:
  - `libgit2` (would be faster than shelling out to `git` CLI)
  - Optimized crypto (Go stdlib pure-Go is slower than assembly)
  - Some compression libraries
- **Mitigation:** Go 1.25+ has improved pure-Go performance

### Provider API Rate Limiting
- **File:** `internal/tools/dispatcher.go:48-55, 115-137`
- **Token Bucket:** General tools: `ToolRateLimitPerSec` (default from config), burst `ToolRateLimitBurst`
- **Dangerous Tools:** Separate bucket `DangerousRateLimitPerSec`, burst `DangerousRateLimitBurst` (M6 feature)
- **Concurrency:** Semaphore `MaxConcurrentTools` (M5 feature, default from config)

### TUI Rendering Performance
- **Framework:** Bubble Tea (immediate mode) + Lipgloss
- **Large Files:** `internal/tui/screens/sidebar/sidebar_view.go` (607 lines), `internal/tui/sidebar_render.go` (607 lines)
- **Virtualization:** `MaxTodoItems` constant (`internal/tui/constants.go:29`, `internal/tui/tuitypes/tuitypes.go:232`) limits sidebar TODO rendering

### Token Estimation Overhead
- **File:** `internal/tokens/estimator.go`
- **tiktoken-go** loaded only for OpenAI models (line 123-128)
- Other providers use heuristic ratios (chars/token) with code vs prose detection (`isCodeHeavy`)
- **EMA Calibration:** Lock-free atomic CAS loop (`Calibrate` method, lines 315-337)

### Compaction Trigger
- **File:** `internal/compaction/compaction.go:79-106`
- **Threshold:** 80% of context window (configurable via `ContextTruncationThreshold`)
- **Auto-compaction** uses LLM to summarize — adds latency but prevents context overflow
- **Fallback:** Crude truncation if compaction fails or insufficient

---

## Configuration Concerns

### Key Files
| File | Purpose |
|------|---------|
| `~/.config/m31a/config.toml` | Primary user config (TOML) |
| `.env` | Local API keys (gitignored) |
| `.env.example` | Template for .env |
| `internal/config/types.go` | Config struct definitions (426+ lines) |

### Validation
- **File:** `internal/config/validate.go`
- **Checks:** Negative values, `healthcheck_slow_ms >= healthcheck_live_ms`, valid log levels, valid workflow modes
- **Missing:** No schema validation for provider-specific configs

### API Keys
- **Priority:** Keychain > `.env` > env vars > config file
- **Keychain:** OS-native (see Security section)
- **Never written to disk** in plaintext

---

## Key Files Requiring Review

### High-Risk / Complex Logic
| File | Lines | Concern |
|------|-------|---------|
| `internal/workflow/engine.go` | 1,707 | Core workflow orchestration, 7 phases, phase transitions, checkpoints, compaction |
| `internal/workflow/execute.go` | 837 | Task execution, self-heal loops, pause/resume, TODO sync |
| `internal/tools/dispatcher.go` | 523 | Permission system, rate limiting, concurrency, batch approvals |
| `internal/config/loader.go` | 785 | Config merge, hot reload, keychain integration, env var loading |
| `internal/git/git.go` | 827 | All git operations, BUG-01 mutex, command construction |
| `internal/tokens/estimator.go` | 415 | Token estimation heuristics, EMA calibration, BUG-29 |
| `internal/provider/capabilities.go` | 391 | Model capability inference, hardcoded model caps, thread-safe cache |
| `internal/keychain/keychain_linux.go` | 367 | D-Bus + pass fallback, command injection prevention |
| `internal/session/manager.go` | ~700 | Session persistence, checkpoints, task state |

### Security-Sensitive
| File | Concern |
|------|---------|
| `internal/keychain/keychain_{linux,darwin,windows}.go` | Platform secret storage, command injection prevention |
| `internal/logging/audit.go` | Secret redaction in logs, static analysis for leaks |
| `internal/workflow/ship_preflight.go` | Pre-commit secret scanning |
| `internal/tools/subagent/loop.go:50` | Prompt injection TODO |

### Test Coverage Critical (90% target)
| Package | Test Files |
|---------|------------|
| `internal/taskrunner/` | `runner_test.go` (560 lines), `doc_test.go` |
| `internal/bisect/` | `bisect_test.go`, `extra_test.go`, `doc_test.go` |
| `internal/rollback/` | `rollback_test.go` (1,198 lines), `doc_test.go` |

---

## Scaling Limits

| Resource | Current Limit | Bottleneck |
|----------|---------------|------------|
| Concurrent tools | `MaxConcurrentTools` (config) | Semaphore in dispatcher |
| Tool call rate | `ToolRateLimitPerSec` (config) | Token bucket |
| Dangerous tool rate | `DangerousRateLimitPerSec` (config) | Separate token bucket |
| Context window | Model-dependent (32K–200K+) | Token estimation accuracy (BUG-29) |
| Subagent turns | 25 (`maxTurns` in `loop.go:43`) | Hardcoded constant |
| Heal attempts | 2 (`MaxHealAttempts` in `constants.go:13`) | Configurable via `max_heal_attempts` |
| Plan↔Discuss cycles | 3 (`maxDiscussPlanCycles` in `engine.go:897`) | Hardcoded constant |

---

## Dependencies at Risk

| Package | Version | Risk |
|---------|---------|------|
| `github.com/charmbracelet/bubbletea` | v1.3.0 | Core TUI framework — major version changes break API |
| `github.com/charmbracelet/bubbles` | v0.20.0 | TUI components — coupled to Bubble Tea |
| `github.com/pkoukk/tiktoken-go` | v0.1.8 | Token estimation — only for OpenAI models |
| `github.com/godbus/dbus/v5` | v5.2.2 | Linux keychain — Linux-only, CGO-free but D-Bus dependent |
| `golang.org/x/sync` | v0.21.0 | `singleflight` for cache stampede (BUG-17) — stable x/ package |

---

## Missing Critical Features / Test Gaps

1. **Prompt Injection Defense** — `internal/tools/subagent/loop.go:50` TODO
2. **E2E Real API Tests** — Require API keys in CI (currently skipped)
3. **Windows Keychain Tests** — Build-tag limited, hard to test on Linux CI
4. **Linux Keychain D-Bus Tests** — Requires Secret Service daemon (may not exist in CI)
5. **Provider Health Check** — `internal/provider/capabilities.go:369` returns `nil` (always healthy) — stub
6. **Config Schema Validation** — No JSON Schema / TOML schema for config validation
7. **Audit Log Secret Scanning** — `AuditLogForSecrets` exists but not integrated into CI

---

*Concerns audit: 2026-07-21*
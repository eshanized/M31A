# Codebase Concerns

**Analysis Date:** 2026-07-11

## Tech Debt

### Hardcoded Model Capabilities and Metadata

**Files:** `internal/provider/capabilities.go:127-191`, `internal/provider/model_metadata.go:149-191`

**Issue:** Large hardcoded fallback tables for model capabilities and metadata (context windows, pricing, tool support). These become stale as providers release new models.

**Impact:** Incorrect capability detection leads to wrong tool availability, context window miscalculations, and failed API calls. New models won't work until hardcoded tables are updated.

**Fix Approach:**
- Keep hardcoded tables as fallbacks only
- Prioritize runtime detection from provider APIs (OpenRouter `/models`, NVIDIA `/models`)
- Add automated test to detect staleness vs. live API data
- Consider external config file for overrides instead of recompilation

---

### BUG Comments Indicating Known Issues

**Files:** Multiple (see below)

**Known Bugs Tracked in Code:**

| Bug ID | File | Description |
|--------|------|-------------|
| BUG-01 | `internal/git/git.go:451` | Channel ordering issue in git operations |
| BUG-06, BUG-07, BUG-19 | `internal/provider/capabilities.go:119`, `internal/tools/concurrency.go:37,50,71` | Data races from sync.Map value-type replacement; fixed by typed mutex-guarded maps |
| BUG-10 | `internal/workflow/ship.go:118` | Ship phase issue when working tree clean after AddAll |
| BUG-12 | `internal/workflow/engine.go:725` | Infinite oscillation between Plan/Discuss phases; capped at 3 cycles |
| BUG-15 | `internal/workflow/ship.go:347` | Numstat-based heuristic for change classification |
| BUG-17 | `internal/provider/cache.go:47` | Cache stampede potential; waiters never touch the fetching goroutine |
| BUG-18 | `internal/config/loader.go:989` | Message dropped silently in config watcher |
| BUG-29 | `internal/tokens/estimator.go:400` | Token estimation may allow requests exceeding model context window |

**Impact:** These are documented known issues that may cause incorrect behavior in edge cases.

**Fix Approach:** Address each BUG comment with a fix and remove the comment. Add regression tests.

---

### Large Files Exceeding Maintainability Thresholds

**Files:**
- `internal/workflow/engine.go` — 1,520 lines (workflow engine with 7 phases)
- `internal/tools/extra_test.go` — 4,441 lines (test coverage boost)
- `internal/workflow/coverage_boost_test.go` — 3,570 lines
- `internal/workflow/engine_extra_test.go` — 3,409 lines
- `internal/tui/sidebar_model.go` — 1,621 lines
- `internal/tools/dispatcher.go` — 981 lines
- `internal/config/loader.go` — 1,154 lines

**Impact:** Large files are harder to review, test, and modify. `engine.go` combines all 7 workflow phases; `sidebar_model.go` handles too many UI concerns.

**Fix Approach:**
- Split `engine.go` into phase-specific files (`engine_initialize.go`, `engine_discuss.go`, etc.)
- Move test coverage boost files to separate `_test.go` per package
- Extract sidebar components (TODO, sessions, git status) into separate models

---

### sync.Map Usage with Type Assertion Risks

**Files:**
- `internal/tools/subagent/manager.go:66` — `agents sync.Map`
- `internal/tools/dispatcher.go:26,31` — `pendingResponses`, `pendingQuestions sync.Map`
- `internal/tools/dns_cache.go:25` — `cache sync.Map`
- `internal/tools/question.go:41` — `pending *sync.Map`
- `internal/workflow/plan_parser.go:26-27` — `sectionHeaderCache`, `subsectionHeaderCache sync.Map`

**Issue:** `sync.Map` requires type assertions on Load(). Incorrect type assertions panic at runtime. The codebase documents this risk in `internal/tools/concurrency.go` and has migrated package-level caches to typed mutex-guarded maps, but these instances remain.

**Impact:** Runtime panics if type assertion fails. No compile-time safety.

**Fix Approach:** Replace remaining `sync.Map` with typed `map[T]V` protected by `sync.RWMutex` where feasible, or add explicit type assertions with error handling.

---

### Empty Test Coverage Output

**File:** `coverage.out` (1 line: `mode: atomic`)

**Issue:** Coverage file exists but contains no package data. The `make test` target generates coverage but it's not being aggregated properly.

**Impact:** No visibility into actual test coverage. Cannot enforce 75% overall / 90% critical package targets.

**Fix Approach:**
- Run `make test` and verify coverage.out has data
- Check `go test -coverprofile=coverage.out ./...` works
- Add CI step to fail if coverage drops below thresholds

---

## Known Bugs

### Config Watcher Drops Messages Silently (BUG-18)

**File:** `internal/config/loader.go:989`

**Description:** Comment states "the message silently (BUG-18)" in the fsnotify event handler. Config file changes may not be picked up.

**Trigger:** Rapid config file modifications or fsnotify event loss.

**Workaround:** Restart application to reload config.

---

### Ship Phase Clean Tree Handling (BUG-10)

**File:** `internal/workflow/ship.go:118`

**Description:** "when the working tree is clean after AddAll (BUG-10)." Ship phase may behave incorrectly when no changes to commit.

---

### Token Estimation May Exceed Context Window (BUG-29)

**File:** `internal/tokens/estimator.go:400`

**Description:** "conversations and may allow requests that exceed the model's window (BUG-29)." Token counting may underestimate, causing API errors.

---

### Cache Stampede in Provider Capabilities (BUG-17)

**File:** `internal/provider/cache.go:47`

**Description:** "runs fetchFn; waiters never touch it (BUG-17)." Multiple concurrent requests for same model capabilities may all trigger fetches.

---

## Security Considerations

### API Keys in Environment and Keychain

**Files:** `cmd/m31a/main.go:155,206-217`, `pkg/keychain/keychain.go`, `internal/config/loader.go`

**Current State:**
- `.env` files auto-loaded via `config.LoadDotEnv()` at startup (line 155)
- `.gitignore` excludes `.env` and `.env.*` except `.env.example` (lines 64-66)
- `.env.test` is COMMITTED (present in repo) — contains test credentials
- OS keychain used via `pkg/keychain/` (D-Bus Secret Service on Linux, `security` on macOS, Credential Manager on Windows)
- Keychain availability cached; falls back to env vars if unavailable
- Bash tool scrubs sensitive env vars from subprocess environment (`internal/tools/bash_sandbox_linux.go:21`)

**Risks:**
1. `.env.test` committed — if it contains real keys, they're in git history
2. `.env` auto-load from cwd — if user runs from untrusted directory, malicious `.env` could inject keys
3. Keychain unavailable warning logged but execution continues with env vars
4. No validation that `.env` file permissions are restrictive (world-readable check mentioned in wiki but not enforced in code)

**Mitigations in Place:**
- Environment scrubbing in bash sandbox (all platforms)
- Landlock/sandbox-exec filesystem restrictions
- Keychain preferred over env vars

---

### Landlock Sandbox Graceful Degradation

**File:** `internal/tools/bash_sandbox_linux.go:24-27`

**Issue:** On kernels < 5.13 or without Landlock support, only environment scrubbing applies. Warning logged at debug level only.

**Impact:** Users on older kernels have no filesystem isolation for bash commands. No prominent warning to user.

**Fix Approach:** Emit WARNING log (not debug) when Landlock unavailable. Document minimum kernel requirement prominently.

---

### SSH Key and Git Credential Exposure

**Files:** `internal/git/git.go`, `internal/workflow/ship.go`

**Issue:** Git operations use system git with user's SSH keys and credential helpers. No isolation of git credentials during automated operations.

**Risk:** Malicious prompt could cause git push to attacker-controlled remote with user's credentials.

**Mitigation:** Permission gating on Bash tool (ask/allow/deny), but git operations may bypass if invoked indirectly.

---

### SSRF Protection in WebFetch

**File:** `internal/tools/webfetch.go` (referenced in SECURITY.md:44)

**Status:** SECURITY.md claims WebFetch blocks private/loopback/link-local IPs. Need to verify implementation enforces this.

---

## Performance Bottlenecks

### Provider Model Fetch on Every Startup

**File:** `cmd/m31a/main.go:72-76`

**Issue:** Headless mode fetches all models from provider on every invocation to pick first model. No caching.

**Impact:** Adds 1-3 seconds latency to `--prompt` commands.

**Fix Approach:** Cache model list with TTL (config has `ModelCacheTTLMinutes: 5` but not used in headless path).

---

### Regex Recompilation in Plan Parser

**File:** `internal/workflow/plan_parser.go:26-28`

**Issue:** `sync.Map` caches compiled regexes per section name, but cache never evicted. Unbounded growth with many unique section names.

**Impact:** Memory leak over long-running sessions with many plans.

**Fix Approach:** Add LRU eviction or size limit to regex caches.

---

### Large Test Files Slow Down Test Runs

**Files:** `internal/tools/extra_test.go` (4,441 lines), `internal/workflow/coverage_boost_test.go` (3,570 lines)

**Impact:** `go test ./...` spends significant time compiling/running these mega-test files.

**Fix Approach:** Split into per-package test files; use `go test -run` to target specific tests.

---

## Fragile Areas

### Bubble Tea TUI Single-Threaded Constraint

**Files:** `internal/tui/app_state.go`, `internal/tui/app_update.go`, `internal/tui/app_update_commands.go`

**Constraint:** All state mutations MUST go through `Update()` method. Never mutate `AppState` from goroutines.

**Current Pattern:** Uses `tea.Cmd` / `tea.Msg` for async work. Workflow engine emits messages via channel (`emitterCh`).

**Risk:** Any direct goroutine write to `AppState` fields causes data races and UI corruption.

**Evidence of Correctness:** `app_state.go:68-69` documents the constraint. Channel emitter pattern used.

**Safe Modification:** Only send messages via `emitterCh` from goroutines. Never access `AppState` directly.

---

### Workflow Engine Phase Oscillation (BUG-12)

**File:** `internal/workflow/engine.go:724-728`

**Issue:** Hardcoded `maxDiscussPlanCycles = 3` prevents infinite Plan↔Discuss loops.

**Root Cause:** TUI state bug or automated retry loop causes phase bouncing.

**Impact:** Workflow stalls after 3 cycles; user sees no progress.

**Fix Approach:** Root-cause the oscillation instead of capping. Add observability (metrics/logging) on phase transitions.

---

### Subagent Spawn Depth Limit

**File:** `internal/tools/subagent/manager.go`, SECURITY.md:47

**Constraint:** Max nesting depth of 2 to prevent runaway agent spawning.

**Implementation:** Checked in subagent spawn logic. Hard limit.

**Risk:** Legitimate complex tasks requiring >2 levels of delegation fail silently.

---

### Dependency Rule: pkg/ Must Not Import internal/

**Enforcement:** Go module system (internal/ not exported). No lint rule.

**Risk:** Accidental import of `internal/` from `pkg/` breaks build but only at compile time.

**Fix Approach:** Add `go vet` check or custom linter rule to enforce boundary.

---

## Scaling Limits

### Session History Memory Growth

**Files:** `internal/tui/chathistory_model.go`, `pkg/history/history.go`

**Issue:** `MaxMessages: 500` (config default) but no automatic truncation of old sessions. `SessionRetentionDays: 30` but cleanup not implemented.

**Impact:** Unbounded disk/memory growth with heavy usage.

---

### Provider Model List Discovery

**Files:** `internal/provider/openrouter/client.go`, `internal/provider/nvidia/client.go`, `internal/provider/zen/client.go`

**Issue:** Each provider implements own `FetchModels()` with different response parsing. No unified caching layer at registry level.

**Impact:** Repeated fetches, inconsistent model metadata across providers.

---

### Cross-Compilation Targets

**File:** `Makefile:24-25, 160-182`

**Targets:** linux/amd64, linux/arm64, darwin/amd64, darwin/arm64, windows/amd64

**Excluded:** windows/arm64

**Issue:** Windows ARM64 excluded without documented reason. Goreleaser config may differ.

---

## Dependencies at Risk

### godbus/dbus v5 (Linux Keychain)

**File:** `go.mod:12`, `pkg/keychain/keychain_linux.go`

**Risk:** Requires CGO (uses `github.com/godbus/dbus/v5` which links to libdbus). Build constraint `CGO_ENABLED=0` in Makefile means this **will not compile on Linux** if keychain is used.

**Current Status:** Build succeeds because keychain is optional (graceful degradation). But Linux users lose secure key storage.

**Fix Approach:** 
- Find pure-Go D-Bus implementation (e.g., `github.com/godbus/dbus/v5` has CGO)
- Or accept CGO for Linux keychain only (breaks static binary guarantee)
- Or use `pass` CLI / `secret-tool` as pure-Go fallback (already has pass CLI fallback)

---

### golang.org/x/sys v0.46.0

**File:** `go.mod:24`

**Risk:** `x/sys` is frequently updated. May contain CGO-dependent code for some platforms. Current version works but needs monitoring.

---

### tiktoken-go v0.1.8

**File:** `go.mod:14`, `internal/tokens/estimator.go`

**Risk:** Token estimation depends on this. If model encodings change, estimation breaks. No automated validation against provider APIs.

---

## Missing Critical Features

### No Automated Dependency Security Scanning

**Issue:** No `govulncheck` or `nancy` in CI pipeline. Dependencies not scanned for CVEs.

---

### No Config Validation for Security Settings

**Issue:** Config allows disabling permission prompts (`permission_mode: allow`), disabling sandbox, etc. No audit trail or warning when insecure configs loaded.

---

### No Structured Audit Logging

**File:** `internal/logging/audit.go` exists but minimal.

**Issue:** Security-relevant events (permission grants, tool executions, keychain access) not logged in queryable format.

---

### Headless Workflow Mode Not Implemented

**File:** `cmd/m31a/main.go:57-60`

**Status:** `runHeadlessWorkflow` returns "not yet fully implemented".

**Impact:** Cannot run full workflows in CI/automation without TUI.

---

## Test Coverage Gaps

### Critical Packages Below 90% Target

**Target:** 90% for `pkg/taskrunner`, `pkg/bisect`, `pkg/rollback`

**Status:** Unknown — coverage.out empty. Need to run `make cover` and verify.

---

### E2E Tests Require Manual API Keys

**File:** `e2e_test.go:52-56, 76-80`

**Issue:** `TestBinary_Prompt_NvidiaRealAPI`, `TestBinary_Prompt_ZenRealAPI`, `TestBinary_Prompt_OpenRouterRealAPI` skip if env vars unset. No CI integration.

**Impact:** No automated verification of real provider integration.

---

### Test File Exclusions from Linters

**Config:** `.golangci.yml` (not found in repo) or AGENTS.md:58-59

**Status:** "Test files are excluded from `errcheck` and `unused`."

**Risk:** Bugs in test code (dead code, unchecked errors) not caught. Test quality degrades silently.

---

## Architectural Constraints Violated (Potential)

### CGO_ENABLED=0 Hard Constraint

**File:** `Makefile:13` (`GOFLAGS := CGO_ENABLED=0`)

**Verification:** No automated check that all dependencies satisfy this. `godbus/dbus` may violate.

**Check Needed:** `go build -ldflags="-linkmode=external"` or `CGO_ENABLED=0 go build` in CI for all targets.

---

### Go Version Lock

**File:** `go.mod:3` (`go 1.25.0`)

**Verification:** No CI check that build environment uses Go 1.25.x. Developer machines may have different version.

**Check Needed:** `go version` check in CI / Makefile `check` target.

---

### .env.example Maintenance

**File:** `.env.example`

**Issue:** Must stay in sync with actual config structure. No automated validation.

---

## Anti-Patterns

### Package-Level Mutable State

**Files:** 
- `internal/provider/capabilities.go:13-16, 120-123` — `capabilityConfig`, `modelCapabilitiesCache`
- `internal/provider/model_metadata.go` — fallback tables
- `internal/tools/concurrency.go` — review function with hardcoded patterns

**Issue:** Global mutable state makes testing harder and creates hidden coupling.

**Mitigation:** Most state is protected by mutexes and initialized at startup. `SetCapabilityConfig` called once from `main.go`.

---

### Magic Numbers in Config Defaults

**File:** `internal/config/loader.go:27-100`

**Issue:** Many hardcoded defaults (timeouts, sizes, thresholds) with no central documentation.

**Fix Approach:** Move to constants in `internal/types/constants.go` with doc comments.

---

### Emoji in Documentation (Not Code)

**Files:** `SECURITY.md`, `AGENTS.md`

**Issue:** Project rule: "No emojis in code or docs" (AGENTS.md:48). But `SECURITY.md` uses `:white_check_mark:` and `:x:` emojis.

**Impact:** Inconsistent with project style.

---

## Recommendations Priority

| Priority | Concern | Effort |
|----------|---------|--------|
| **P0** | Verify CGO_ENABLED=0 builds on all targets (especially Linux with godbus/dbus) | Low |
| **P0** | Fix empty coverage.out — enable coverage tracking in CI | Low |
| **P0** | Remove committed `.env.test` or ensure it contains no secrets | Low |
| **P1** | Address BUG-18 (config watcher silent drop) | Medium |
| **P1** | Add warning log when Landlock unavailable on Linux | Low |
| **P1** | Split engine.go into phase-specific files | High |
| **P2** | Replace remaining sync.Map with typed maps | Medium |
| **P2** | Implement headless workflow mode | High |
| **P2** | Add govulncheck to CI | Low |
| **P3** | Automate model capability freshness checks | Medium |
| **P3** | Add config validation for security settings | Medium |
| **P3** | Document windows/arm64 exclusion reason | Low |

---

*Concerns audit: 2026-07-11*
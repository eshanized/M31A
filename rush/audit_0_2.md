# Audit Report: Phase 0–2 Implementation vs Spec

**Auditor**: CTO / Principal Architect  
**Date**: 2026-05-27  
**Scope**: M31A V1 implementation (Phase 0: Foundation, Phase 1: Provider Abstraction, Phase 2: TUI Foundation)  
**Reference**: `adrenaline/idea.md`, `adrenaline/REFERENCE.md`, `adrenaline/ROADMAP.md`, `walkthrough_0.md`, `walkthrough_1.md`, `walkthrough_2.md`  
**Verification**: Read-only audit of all `.go` source files, configs, docs, and CI artifacts.

---

## Executive Summary

The Phase 0–2 implementation is **substantially complete** and functionally solid. All code compiles to a static binary (CGO_ENABLED=0), passes `go vet` with zero warnings, and all 152 tests pass with race detection enabled. The architecture cleanly separates concerns across 7 internal packages and 8 pkg stubs. The Bubble Tea TUI is well-structured with proper screen routing, health tick lifecycle, and a 6-state first-run wizard.

However, **5 spec deviations** and **3 inaccurate walkthrough claims** were identified that should be addressed before Phase 3 begins. One deviation (Go version) could cause CI failures. The provider badge format, cost estimation, and cache behavior differ from the spec in ways that downstream consumers (arbitrage, model selector, settings screen) may depend on.

---

## Scorecard

| Area | Status | Notes |
|------|--------|-------|
| Project Structure & `go.mod` | ⚠️ PASS | Go 1.24.2 instead of 1.22 — CI uses 1.22 |
| Core Types & Interfaces | ✅ PASS | All types defined; ProviderRegistry removed per plan |
| Provider Layer | ⚠️ PASS | 38/40 tests pass; 2 missing tests |
| TUI Foundation | ✅ PASS | 114 tests; all compile and pass |
| CI/CD Pipeline | ⚠️ PASS | Go 1.22 in CI vs 1.24.2 local — drift risk |
| Documentation | ✅ PASS | ARCHITECTURE.md, INTERFACES.md, TYPES.md correct |
| Walkthrough Accuracy | ⚠️ 3 claims inaccurate | See Deviation Register §5 |
| `pkg/` Stubs (Phase 3+) | ✅ PASS | All 7 present with `.gitkeep`; correct |

---

## 1. Critical Deviations

### CRIT-01: Go Version Mismatch — `go.mod` says 1.24.2, spec requires 1.22

| Field | Spec | Actual |
|-------|------|--------|
| Go version | `go 1.22` | `go 1.24.2` |
| CI go-version | `"1.22"` | `"1.22"` |

**Impact**: CI builds and tests with Go 1.22, but local development uses 1.24.2. If any Go 1.23+ features are used (range-over-func, iterator funcs, new stdlib additions), CI will fail. Currently no such features are detected, but the drift is risky. `go.mod` should be pinned to `1.22` and `go mod tidy` re-run.

**Recommendation**: Run `go mod tidy -go=1.22` to downgrade `go.mod` to `1.22` and ensure compatibility.

---

## 2. Medium Deviations

### MED-01: Provider Badge Format — `[OP]`/`[ZE]` instead of `[OR]`/`[ZEN]`

**File**: `internal/tui/header.go:23`  
**Code**: `prefix := strings.ToUpper(provider[:2])`  
**Spec**: `adrenaline/idea.md` says badges should be `[OR]` for OpenRouter and `[ZEN]` for Zen.  
**Actual**: Produces `[OP]` for "openrouter" and `[ZE]` for "zen".  
**Impact**: Cosmetic, but the spec's expected format is embedded in acceptance criteria. Phase 3's model selector and settings screen may render these badges differently.

### MED-02: `EstimateCost` Always Returns 0 — No Model ID in Interface

**File**: `internal/provider/interface.go:13`  
**Signature**: `EstimateCost(usage types.Usage) float64`  
**Spec**: `adrenaline/idea.md` §ProviderInterface → `EstimateCost(usage, modelID)`.  
**Actual**: No `modelID` parameter in the interface. Both clients return `0`.  
**Impact**: Phase 3's `pkg/arbitrage` depends on cost comparison between models. Without model-aware costing, arbitrage cannot function. **This is a Phase 3 blocker** unless the interface is revised now.

### MED-03: `ModelCache.Get()` No Stale Read — Only Fresh Data Served

**File**: `internal/provider/cache.go:26-35`  
**Behavior**: `Get()` returns `nil, false` if the cache TTL has expired, even if stale data is available. The `IsStale()` method and `staleTTL` (24h) exist but are only used by `staleFallback()` in the clients (on API errors), not during normal cache lookups.  
**Spec**: `adrenaline/idea.md` says "cache-first: return cached if not expired, fall back to API, use stale on network error."  
**Impact**: Every `GetModel()` call fails when TTL expires until `FetchModels` is called again. If Phase 3's model selector reads models via `GetModel()` after TTL expiry, it may show empty state even with usable stale data.

### MED-04: `FetchModels` Always Hits API First — Cache Used Only for Stale Fallback

**File**: `internal/provider/openrouter/client.go:72-116` (same pattern in `zen/client.go:64-118`)  
**Behavior**: Both clients unconditionally make an HTTP request. The cache is updated after a successful fetch and used only on network errors via `staleFallback()`.  
**Spec**: `adrenaline/idea.md` says "check cache first, return cached if not expired."  
**Impact**: Unnecessary API calls on every model list request. Minor perf issue but spec violation. Walkthrough acknowledges this deviation.

### MED-05: `calculateNextInterval` Has No Jitter/Backoff — Walkthrough Claim Is Wrong

**File**: `internal/tui/app.go:233-243`  
**Code**: Returns `120s` for offline/rate-limited, `60s` otherwise.  
**Walkthrough Claim**: "exponential backoff with jitter (60s base, 5 min max, uniform)."  
**Actual**: Simple two-state interval with no randomness or exponential component.  
**Impact**: Multiple clients health-checking simultaneously could thundering-herd. Walkthrough overpromises.

### MED-06: Screen Enum Mismatch — Walkthrough Claims `ScreenHelp`, `ScreenExit`

**File**: `internal/tui/types.go:7-14`  
**Walkthrough Claim**: `ScreenREPL, ScreenFirstRun, ScreenHelp, ScreenExit`  
**Actual**: `ScreenFirstRun`, `ScreenREPL`, `ScreenModelSelector`, `ScreenSettings`, `ScreenResume`, `ScreenPermission`

### MED-07: Terminal Guard Threshold — Walkthrough Says 80×24, Code Uses 40×10

**File**: `internal/tui/app.go:184`  
**Code**: `if m.width < 40 || m.height < 10`  
**Walkthrough**: "terminal-too-small guard (80×24 threshold)"  
**Impact**: Walkthrough is wrong; actual guard is more permissive.

---

## 3. Minor / Informational Deviations

### MIN-01: No `ctrl+w` Skip in Welcome Screen

**File**: `internal/tui/firstrun.go:92-101`  
**Walkthrough**: "pressing `ctrl+w` at Welcome jumps to Complete → REPL"  
**Actual**: No `ctrl+w` handler. Only `enter`/`space` and global `ctrl+c` are handled.

### MIN-02: First-Run Validation Failure Returns to KeyInput (Does NOT Call `tea.Quit`)

**File**: `internal/tui/firstrun.go:195-208`  
**Behavior**: On validation failure, `updateValidating` returns `nil, nil` and state goes back to `FirstRunKeyInput`.  
**Note**: This is actually **correct** behavior that matches the spec (user re-enters key). No action needed.

### MIN-03: `HealthCheckTicker` Ignores `ctx` for Cancellation

**File**: `internal/tui/health.go:13-27`  
**Code**: Accepts `ctx` but only checks `nil`. Never selects on `ctx.Done()`.  
**Impact**: `tea.Tick` is the actual lifecycle mechanism so this is not a bug, but the unused `ctx` parameter is misleading.

### MIN-04: SSE Field Format Uses Dots-Only (Not Brackets)

**File**: `internal/provider/reasoning.go:79-104`  
**Path**: `choices.0.delta.*` instead of `choices[0].delta.*`  
**Impact**: If a JSON key contains a `.`, the split fails. Low risk for known providers.

### MIN-05: Anthropic SSE Handling May Be Incorrect

**File**: `internal/provider/reasoning.go:143-150`  
**Code**: Checks `deltaMap["type"] == "thinking"` — but Anthropic's streaming API uses `content_block` events with `delta: { type: "thinking", ... }`. The SSEField is set to `choices.0.delta.content` for anthropic, which likely never matches actual Anthropic response structure.

### MIN-06: Provider Test Count Mismatch

**Walkthrough claim**: 40 tests  
**Actual**: 38 tests (13 openrouter + 7 zen + 5 sse + 8 reasoning + 5 registry)  
**Missing**: 2 tests — possibly the `HealthCheck` tests for zen (3 states claimed but only 1 health test function found).

### MIN-07: `go.sum` Now Exists

Walkthrough 0 documented its absence as a deviation. `go.sum` is now present (5.2KB) with all charmbracelet dependencies. Resolved.

---

## 4. Acceptance Criteria Status

Criteria from `adrenaline/ROADMAP.md`:

| ID | Criterion | Status | Notes |
|----|-----------|--------|-------|
| AC-0.1 | Go module with `go 1.22` | ❌ FAIL | Module is `go 1.24.2` |
| AC-0.2 | Static binary (CGO_ENABLED=0) | ✅ PASS | Verified: ELF statically linked |
| AC-0.3 | CI/CD with lint, test, build, release | ✅ PASS | All 4 jobs configured |
| AC-0.4 | 15 sentinel errors | ✅ PASS | Exactly 15, verified |
| AC-1.1 | OpenRouter provider | ✅ PASS | Full implementation |
| AC-1.2 | Zen provider | ✅ PASS | Full implementation |
| AC-1.3 | Thread-safe model cache with stale fallback | ⚠️ PARTIAL | Cache exists but `Get()` has no stale read |
| AC-1.4 | SSE streaming | ✅ PASS | With multi-line data, event types, `[DONE]` |
| AC-1.5 | Auto-fallback between providers | ✅ PASS | `ShouldFallback` / `FindFallbackProvider` |
| AC-1.6 | 40 provider tests | ❌ FAIL | 38 tests (2 missing) |
| AC-2.1 | Theme system (dark/light/auto) | ✅ PASS | 12 colors + 18 styles + Manager |
| AC-2.2 | Header: brand, badge, model, context, health | ✅ PASS | With truncation logic |
| AC-2.3 | REPL screen (viewport + textarea + spinner) | ✅ PASS | 4-region layout |
| AC-2.4 | First-run wizard (6 states) | ✅ PASS | Welcome→Select→Key→Validate→Keychain→Complete |
| AC-2.5 | Health tick (60s, adaptive) | ⚠️ PARTIAL | 60s tick works but "adaptive" is simple 60s/120s |
| AC-2.6 | 110+ TUI tests | ✅ PASS | 114 tests |
| AC-2.7 | No CGO, static binary | ✅ PASS | Verified |

**Summary**: 14 PASS, 2 PARTIAL, 3 FAIL

---

## 5. Walkthrough Inaccuracy Register

| Walkthrough | Claim | Actual | Severity |
|-------------|-------|--------|----------|
| `walkthrough_2.md` Line 31 | `calculateNextInterval` has "exponential backoff with jitter (60s base, 5 min max, uniform)" | Two-state: 60s/120s, no jitter, no max | HIGH |
| `walkthrough_2.md` Line 38 | Terminal guard "80×24 threshold" | Code: `40×10` | MEDIUM |
| `walkthrough_2.md` Line 6 | Screen enum includes `ScreenHelp`, `ScreenExit` | Actual: `ScreenModelSelector`, `ScreenSettings`, `ScreenResume`, `ScreenPermission` | MEDIUM |
| `walkthrough_1.md` Line 56 | "40 tests" in provider | Actual: 38 tests | LOW |
| `walkthrough_2.md` Line 27 | "pressing `ctrl+w` at Welcome jumps to Complete" | No `ctrl+w` handler exists | LOW |

---

## 6. Phase 3 Readiness Assessment

### Blockers (must fix before Phase 3)

1. **CRIT-01: Go version pinning** — Must restore `go 1.22` in `go.mod` to match CI and spec.
2. **MED-02: `EstimateCost` interface** — Must add `modelID string` parameter before `pkg/arbitrage` depends on it. Changing the interface after Phase 3 touches arbitrage will be costly.

### Recommended Fixes (before Phase 3)

3. **MED-01: Provider badge format** — Change `provider[:2]` logic to use explicit mapping: `"openrouter" → "OR"`, `"zen" → "ZEN"`.
4. **MED-03: `ModelCache.Get()` stale fallback** — Add stale read path: if expired but not stale-stale, serve cached data.
5. **MED-04: `FetchModels` cache-first** — Check cache before API call.
6. **MED-06/MED-07: Walkthrough accuracy** — Update walkthroughs with actual values, or more importantly, align code with spec if spec intent differs.

### Clean Open Items (no Phase 3 impact)

7. **MIN-01: `ctrl+w` skip** — Either add the handler or correct the walkthrough.
8. **MIN-04: SSE dots-only** — Low risk; acceptable.
9. **MIN-05: Anthropic SSE** — Needs investigation if Anthropic support is planned.
10. **MIN-06: Missing 2 tests** — Add 2 test cases to reach 40.

---

## 7. Spec Drift Commentary

The implementation team made **pragmatic tradeoffs** during execution, most of which were properly documented in walkthrough deviations sections. The badge prefix (`[OP]` vs `[OR]`) is the most visible cosmetic drift and should be fixed for consistency. The `EstimateCost` interface gap is the only **semantic drift** that will cause real problems in Phase 3.

The walkthroughs claim capabilities (exponential jitter backoff, 80×24 threshold, ScreenHelp/ScreenExit) that do not exist in code. This is concerning — walkthroughs are supposed to document what was **actually built**, not what was planned. Recommend walkthroughs be audited against code before each Phase X handoff.

---

*End of audit report. 7 deviations found: 1 critical, 5 medium, 1 minor. 3 walkthrough inaccuracies.*

# CONCERNS.md

**Last Updated:** 2026-07-23
**Project:** M31A

---

## Overview

This document catalogs technical debt, known issues, security concerns, performance bottlenecks, and fragile areas in the M31A codebase as of 2026-07-23.

---

## Technical Debt

### BUG-01: Git Commit Channel Ordering
**Location:** `internal/integrations/git/git.go:451`
**Severity:** Medium
**Description:** Commit operations may have channel ordering issues during concurrent git operations. The comment warns "to avoid channel ordering issues (BUG-01)."
**Impact:** Potential race conditions in git commit workflows under concurrent load.

### BUG-06, BUG-07, BUG-19: Sync.Map Replacement Race Conditions
**Locations:** 
- `internal/tools/exec/concurrency.go:37-71`
- `internal/integrations/provider/capabilities.go:119`
- `internal/integrations/provider/cache.go:47`
- `internal/integrations/provider/capabilities_test.go:419`

**Severity:** High
**Description:** Multiple locations reference replacement of sync.Map usage to prevent data races from value-type replacement (BUG-06, BUG-07, BUG-19). The codebase has been migrating to typed mutex-guarded maps but some instances may remain.
**Impact:** Potential data races under concurrent access to capability caches and concurrency control structures.

### BUG-17: Provider Cache Waiter Race
**Location:** `internal/integrations/provider/cache.go:47`
**Severity:** Medium
**Description:** Comment indicates "runs fetchFn; waiters never touch it (BUG-17)" - potential issue with cache waiter pattern.
**Impact:** Cache stampede or stale data under concurrent cache misses.

### BUG-29: Token Estimator Window Overflow
**Location:** `internal/engine/tokens/estimator.go:400`
**Severity:** Medium
**Description:** Token estimation for conversations "may allow requests that exceed the model's window (BUG-29)."
**Impact:** Requests exceeding context window limits, causing API errors.

### BUG-18: Silent Message Drop
**Location:** `internal/core/config/loader.go:620`
**Severity:** Low
**Description:** Comment indicates "the message silently (BUG-18)" - a message may be dropped without error.
**Impact:** Silent failures in configuration loading.

---

## Known Issues (TODOs/FIXMEs)

### Glob Tool Bug
**Location:** `internal/tools/glob_test.go:164, 185`
**Severity:** Medium
**Description:** "BUG(glob): rg code path has a known issue where os.Stat fails on relative paths" - ripgrep-based glob has path resolution issues.
**Impact:** File globbing may fail on certain relative path patterns.

### Test-Only Panics
**Locations:**
- `cmd/m31a/main.go:577` - `os.Exit(1)` on TUI initialization failure
- `internal/engine/rollback/rollback_test.go:34, 37` - `panic()` in tests
- `internal/engine/workflow/execute_test.go:382` - `os.Exit(1)` in test
- `internal/engine/workflow/engine_verify.go:285` - `panic("not implemented")` stub

**Severity:** Low (test code only)
**Description:** Panic and os.Exit usage in test files and one production error path.

---

## Security Concerns

### Unsafe Pointer Usage (Windows Keychain)
**Location:** `internal/integrations/keychain/keychain_windows.go:53-130`
**Severity:** Medium
**Description:** Extensive use of `unsafe.Pointer` for Windows Credential Manager API (CredReadW, CredWriteW, CredDeleteW). 18+ unsafe pointer conversions.
**Risk:** Memory safety issues if pointer arithmetic is incorrect; potential for crashes or memory corruption on Windows.
**Mitigation:** Well-commented with safety rationale; CGO_ENABLED=0 means this code only compiles on Windows.

### Linux Sandbox (seccomp/bpf)
**Location:** `internal/tools/exec/bash_sandbox_linux.go:58-84`
**Severity:** Low
**Description:** Uses `unsafe.Pointer` for seccomp BPF attribute configuration.
**Risk:** Incorrect BPF filter setup could allow sandbox escape.
**Mitigation:** Only used when sandbox enabled; minimal surface area.

### Prompt Injection Defenses (Subagent)
**Location:** `internal/tools/subagent/loop.go:397-398`
**Severity:** Medium
**Description:** Comment "SECURITY: Add prompt injection defenses" - indicates known gap in subagent prompt sanitization.
**Risk:** Subagent prompts could be manipulated via user input.
**Status:** TODO item, not yet implemented.

---

## Performance Concerns

### Token Estimation Accuracy
**Location:** `internal/engine/tokens/estimator.go:400`
**Severity:** Medium
**Description:** Token estimator may underestimate context usage (BUG-29), leading to API errors for long conversations.

### Capability Cache Stampede
**Location:** `internal/integrations/provider/cache.go:47`
**Severity:** Low
**Description:** Cache waiter pattern may allow thundering herd on cache miss (BUG-17).

### Large File Handling in Grep/Glob Tools
**Location:** `internal/tools/grep_skip_comments_test.go:200-217`
**Severity:** Low
**Description:** Tests show grep handles large files; need to verify streaming behavior for very large files.

---

## Fragile Areas

### Bubble Tea TUI Single-Threaded Constraint
**Location:** `cmd/m31a/main.go`, `internal/ui/tui/`
**Severity:** High (architectural constraint)
**Description:** Bubble Tea (Elm architecture) is strictly single-threaded. All state mutations must go through `Update()` via `tea.Cmd`/`tea.Msg`. 
**Risk:** Goroutines mutating `AppState` directly cause data races and UI corruption.
**Pattern:** Use channels for cross-goroutine communication; never share mutable state.

### CGO_ENABLED=0 Hard Constraint
**Location:** `go.mod`, `Makefile`, `AGENTS.md`
**Severity:** High (build constraint)
**Description:** Binary must be fully static (CGO_ENABLED=0). Any dependency requiring CGO breaks the build.
**Impact:** Windows keychain uses CGO (but is build-tagged); Linux sandbox uses syscalls directly.

### Provider Model Discovery (Dynamic)
**Location:** `internal/integrations/provider/`
**Severity:** Medium
**Description:** Model lists are discovered dynamically from provider APIs (OpenRouter, Zen, Nvidia). No hardcoded model names allowed.
**Risk:** API changes break model selection; requires network at startup.

### OS Keychain for API Keys
**Location:** `pkg/keychain/`, `internal/integrations/keychain/`
**Severity:** Medium
**Description:** API keys stored in OS keychain (macOS Keychain, Windows Credential Manager, Linux libsecret). Never written to disk in plaintext.
**Risk:** Keychain unavailable → auth failures; cross-platform compatibility issues.

### Workflow Engine Seven-Phase Pipeline
**Location:** `internal/workflow/engine.go`
**Severity:** Medium
**Description:** Fixed seven-phase pipeline (Initialize → Discuss → Plan → Execute → Verify → Runtime → Ship). Phase transitions are rigid.
**Risk:** Adding/removing phases requires engine changes; phase skipping not supported.

---

## Test Coverage Gaps

### Coverage Targets (from AGENTS.md)
- **75%** overall
- **90%** for `pkg/taskrunner`, `pkg/bisect`, `pkg/rollback`

### E2E Tests Require API Keys
**Location:** `e2e_test.go`
**Severity:** Low
**Description:** Tests `TestBinary_Prompt_*RealAPI` skip when `OPENROUTER_API_KEY`, `ZEN_API_KEY`, `NVIDIA_API_KEY` not set.
**Impact:** CI may not run full integration tests without secrets.

### Test Files Excluded from Linting
**Location:** `.golangci.yml`
**Severity:** Low
**Description:** Test files excluded from `errcheck` and `unused` linters.
**Impact:** Dead code or unchecked errors in tests not caught by CI.

---

## Dependency Risks

### Go Version Pinning
**Location:** `go.mod:3` (`go 1.25.0`)
**Severity:** Medium
**Description:** Requires Go 1.25+. Must verify local Go version matches.

### Bubble Tea Version Lock
**Location:** `go.mod`
**Severity:** Low
**Description:** TUI framework upgrades may break Elm architecture patterns.

### Provider SDKs
**Location:** `internal/integrations/provider/`
**Severity:** Low
**Description:** Direct API calls to OpenRouter, Zen, Nvidia (no official SDKs). API changes require code updates.

---

## Configuration Fragility

### .env Files Gitignored
**Location:** `.gitignore`, `.env.example`
**Severity:** Low
**Description:** `.env` files gitignored except `.env.example`. Local config not tracked.

### Keychain Fallback
**Location:** `pkg/keychain/`
**Severity:** Medium
**Description:** If OS keychain unavailable, API key storage falls back (behavior unclear from code).

---

## Summary

| Category | Count | High Severity |
|----------|-------|---------------|
| Technical Debt (BUG-*) | 5 | 1 |
| Known Issues | 2 | 0 |
| Security | 3 | 0 |
| Performance | 2 | 0 |
| Fragile Areas | 6 | 2 |
| Test Coverage | 2 | 0 |
| Dependencies | 3 | 0 |
| Configuration | 2 | 0 |
| **Total** | **25** | **3** |

---

## Recommendations

1. **High Priority:** Address BUG-06/07/19 sync.Map races; verify all replaced with mutex-guarded maps
2. **High Priority:** Document and enforce Bubble Tea single-threaded pattern in contributor docs
3. **Medium Priority:** Implement prompt injection defenses in subagent loop
4. **Medium Priority:** Fix token estimator (BUG-29) to prevent context window overflow
5. **Medium Priority:** Add CI check for CGO_ENABLED=0 compliance
6. **Low Priority:** Clean up test-only panics/os.Exit; improve glob tool path handling

---

*Generated by gsd-codebase-mapper on 2026-07-23*
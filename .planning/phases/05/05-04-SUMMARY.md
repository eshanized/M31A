---
phase: 05-session-state-configuration
plan: 04
subsystem: config
tags: ["toml", "tiktoken", "config-loader", "token-estimation", "env-override", "keychain-resolution"]
requires:
  - phase: 05-01-keychain
    provides: Keychain interface (Get/Set/Delete) for API key resolution
provides:
  - Config loader with TOML parsing via BurntSushi/toml
  - Env var override support (M31A_CONFIG, M31A_THEME, M31A_DEFAULT_MODEL)
  - API key resolution with env var → keychain → config file priority
  - Atomic config save (temp file + rename) with automatic parent dir creation
  - Token estimator using tiktoken-go with rune-based fallback
  - EMA calibration correction (alpha=0.3) for token estimates
  - Context usage formatting and warning banner
affects: [06-workflow-engine, 05-settings-screen, phase-03-rendering]

tech-stack:
  added:
    - github.com/BurntSushi/toml v1.6.0 — TOML config parsing/serialization
    - github.com/pkoukk/tiktoken-go v0.1.8 — Token estimation for GPT model families
  patterns:
    - Config resolution order: env var → keychain → config file field
    - Atomic write pattern (crypto/rand temp name, same-directory rename)
    - EMA calibration for streaming token estimation

key-files:
  created:
    - internal/config/loader.go — Load, Save, DefaultConfig, ResolveAPIKeys
    - internal/config/loader_test.go — 13 tests for config loader
    - internal/tokens/estimator.go — Estimator struct with Estimate/Calibrate/FormatUsage/ContextWarningBanner
    - internal/tokens/estimator_test.go — 21 tests for token estimator
  modified:
    - go.mod — Added BurntSushi/toml and tiktoken-go dependencies
    - go.sum — Updated checksums

key-decisions:
  - "ResolveAPIKeys never returns error for missing keys — first-run flow or settings screen handles key input"
  - "EMA factor clamped to [0.1, 10.0] to prevent extreme values from corrupting estimates"
  - "M31A_CONFIG env var overrides file path argument in both Load and Save"
  - "Missing config file returns DefaultConfig (zero-valued) with nil error — first-run flow creates it"
  - "rune count for fallback (not byte count) ensures correct CJK/multibyte character handling"

patterns-established:
  - "Config resolution: env var > keychain. Get() > config file field for each provider independently"
  - "Token estimation: tiktoken-go for GPT models, len([]rune)/4*1.3 for others, EMA factor applied"
  - "Atomic writes: same-directory temp file with crypto/rand name, Sync(), then os.Rename()"
  - "Threshold-based warnings: ContextWarningBanner returns empty string below threshold (default 80%)"

requirements-completed: [P5.1, P5.7]

duration: 3 min
completed: 2026-05-27
---

# Phase 5: Session State & Configuration — Plan 4 Summary

**Config loader (TOML + env var override + keychain resolution) and token estimator (tiktoken-go + rune fallback + EMA calibration) for context usage tracking and configuration management**

## Performance

- **Duration:** 3 min
- **Started:** 2026-05-27T19:46:40Z
- **Completed:** 2026-05-27T19:49:57Z
- **Tasks:** 3
- **Files modified:** 4 (created) + 2 (deps)

## Accomplishments

- Config loader with `Load()` (TOML decode, `M31A_CONFIG`/`M31A_THEME`/`M31A_DEFAULT_MODEL` overrides, missing file returns defaults), `Save()` (atomic temp+rename with `MkdirAll` parent dir), `DefaultConfig()` (zero-valued), and `ResolveAPIKeys()` (env var → keychain → config file resolution for OpenRouter and Zen)
- Token estimator with `Estimator` struct: `NewEstimator` uses `tiktoken.EncodingForModel` for supported models, `Estimate()` applies tiktoken or rune fallback with EMA factor, `Calibrate()` updates running factor (alpha=0.3) clamped to [0.1, 10.0], `FormatUsage()` renders `"used / total (XX%)"`, `ContextWarningBanner()` returns lipgloss-styled warning at 80%+ threshold
- 34 tests total (13 config loader + 21 token estimator), all passing with `go vet` clean
- Dependencies: `BurntSushi/toml` v1.6.0 and `pkoukk/tiktoken-go` v0.1.8 added

## Task Commits

Each task was committed atomically:

1. **Task 1: Config loader — Load, DefaultConfig, Save, ResolveAPIKeys** - `fdcbdde` (feat)
2. **Task 2: Token estimator — Estimate, Calibrate, FormatUsage, ContextWarningBanner** - `7e76fe5` (feat)
3. **Task 3: Tests for config loader (13) and token estimator (21)** - `fc0214a` (test)

## Files Created/Modified

- `internal/config/loader.go` — Config loader with Load, Save, DefaultConfig, ResolveAPIKeys
- `internal/config/loader_test.go` — 13 tests covering TOML parse, missing file, env overrides, save, key resolution
- `internal/tokens/estimator.go` — Token estimator struct with Estimate/Calibrate/FormatUsage/ContextWarningBanner
- `internal/tokens/estimator_test.go` — 21 tests covering known/unknown models, fallback, EMA, clamp, formatting, warning
- `go.mod` — Added BurntSushi/toml v1.6.0, tiktoken-go v0.1.8
- `go.sum` — Updated dependency checksums

## Decisions Made

- **ResolveAPIKeys never returns error for missing keys** — Keys are optional; first-run flow or settings screen handles input. Missing keys result in empty strings, not errors.
- **EMA factor clamped to [0.1, 10.0]** — Prevents extreme calibration values from corrupting estimates after pathological single-request ratios. With alpha=0.3, even a 10x error converges to useful range in ~5 iterations.
- **M31A_CONFIG env var overrides file path** — Consistent behavior: both Load() and Save() check the env var after receiving the path argument. This allows users to redirect config location without modifying application code.
- **Missing config file returns defaults** — The file does not autocreate on Load(). First-run flow creates it via Save() after user completes setup. This avoids writing empty config files on every startup.
- **Rune-based fallback for multibyte safety** — Using `len([]rune(text))` instead of `len(text)` ensures CJK characters are counted correctly (Chinese/Japanese/Korean each count as 1 token-equivalent unit, not 2-4 bytes).

## Deviations from Plan

None — plan executed exactly as written.

### Test Count Verification

| Package | Required | Actual | Status |
|---------|----------|--------|--------|
| internal/config/loader_test.go | 12+ | 13 | ✅ |
| internal/tokens/estimator_test.go | 14+ | 21 | ✅ |

## Issues Encountered

None — all tasks completed without issues.

## Next Phase Readiness

- Config loader and token estimator ready for integration
- Settings screen (Plan 05-05) can use `config.Load()` / `config.Save()` for config editing
- Header component can use `tokens.Estimator` for context usage bar and warning banner
- Ready for Plans 05-05 and 05-06

---

*Phase: 05-session-state-configuration*
*Completed: 2026-05-27*

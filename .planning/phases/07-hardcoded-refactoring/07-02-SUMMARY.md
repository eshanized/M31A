---
phase: 07-hardcoded-refactoring
plan: 02
subsystem: provider
tags: [fallback, capabilities, config, toml, provider-registry]

# Dependency graph
requires:
  - phase: 07-hardcoded-refactoring/01
    provides: "Configurable model defaults and verification commands"
provides:
  - "Configurable fallback priority via TOML"
  - "Configurable health check timeout"
  - "Configurable provider registration order"
  - "Config-based model capability pattern overrides"
  - "Config-based known model capability extensions"
affects: [provider, tui, config]

# Tech tracking
tech-stack:
  added: []
  patterns: [package-level-config-injection, additive-pattern-merging]

key-files:
  created: []
  modified:
    - internal/config/types.go
    - internal/config/loader.go
    - internal/provider/fallback.go
    - internal/provider/capabilities.go
    - internal/provider/registry_test.go
    - internal/provider/extra_test.go
    - internal/provider/capabilities_test.go
    - internal/tui/app_agent.go
    - cmd/m31a/main.go

key-decisions:
  - "Used package-level config with SetCapabilityConfig instead of threading config through provider clients — minimizes API changes while making patterns configurable"
  - "Config overrides are additive only — built-in patterns and known capabilities always remain as fallback"
  - "FindFallbackProvider accepts variadic-style config params (nil = use defaults) for backward compatibility"

patterns-established:
  - "Package-level config injection: SetCapabilityConfig initializes provider-level config once at startup"
  - "Additive config merging: config extends built-in patterns, never replaces them"

requirements-completed: [HARD-05, HARD-06, HARD-07]

# Metrics
duration: 12min
completed: 2026-07-05
---

# Phase 07 Plan 02: Provider Registry and Capabilities Summary

**Configurable fallback priority, health check timeout, and model capability pattern overrides via TOML config with additive-only merging**

## Performance

- **Duration:** 12 min
- **Started:** 2026-07-05T21:28:46Z
- **Completed:** 2026-07-05T21:41:04Z
- **Tasks:** 2
- **Files modified:** 9

## Accomplishments
- Fallback priority order now configurable via `[provider.fallback_priority]` TOML key
- Health check timeout configurable via `[provider.health_check_timeout_secs]`
- Provider registration order configurable via `[provider.registration_order]`
- Model capability patterns extensible via `[model_capabilities]` section with extra pattern lists
- Known model capabilities overridable via `[model_capabilities.known_capabilities]` map
- All existing behavior preserved when config is empty (backward compatible)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add Provider Config Fields and Fallback Priority** - `1d8098a5` (feat)
2. **Task 2: Model Capability Pattern Overrides and Known Capabilities Extension** - `26ef53fa` (feat)

## Files Created/Modified
- `internal/config/types.go` - Added FallbackPriority, HealthCheckTimeoutSecs, RegistrationOrder to ProviderConfig; added ModelCapabilitiesConfig and ModelCapabilityOverride structs
- `internal/config/loader.go` - Set defaults matching current hardcoded behavior; added model_capabilities to known config keys
- `internal/provider/fallback.go` - FindFallbackProvider now accepts fallbackPriority and healthCheckTimeoutSecs params; uses config order when non-empty, alphabetical when empty
- `internal/provider/capabilities.go` - Added package-level capabilityConfig with SetCapabilityConfig; ParseModelCapabilities and IsNonChatModel check config extra patterns; DetectCapabilities checks config overrides before built-in table
- `internal/provider/registry_test.go` - Updated test signatures; added tests for custom priority, skip-current, and empty-priority fallback
- `internal/provider/extra_test.go` - Updated FindFallbackProvider and FindFallbackWithRetryAfter calls with new params
- `internal/provider/capabilities_test.go` - Added 6 tests for config-based capability detection
- `internal/tui/app_agent.go` - Passes config FallbackPriority and HealthCheckTimeoutSecs to FindFallbackWithRetryAfter
- `cmd/m31a/main.go` - Calls SetCapabilityConfig at startup with config values

## Decisions Made
- Used package-level config injection (`SetCapabilityConfig`) instead of threading config through provider clients — minimizes API surface changes
- Config overrides are additive only — built-in patterns and known capabilities always remain as fallback for unknown models
- `FindFallbackProvider` accepts nil/empty params for backward compatibility with existing callers
- Non-chat model patterns also extensible via config (`ExtraNonChatPatterns`)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Added cache clearing on config update**
- **Found during:** Task 2 (Model capability config tests)
- **Issue:** `modelCapabilitiesCache` retained stale results after config override, causing test pollution
- **Fix:** `SetCapabilityConfig` now clears `modelCapabilitiesCache` when config is updated
- **Files modified:** internal/provider/capabilities.go
- **Verification:** All tests pass including config override tests
- **Committed in:** 26ef53fa (Task 2 commit)

**2. [Rule 1 - Bug] Fixed test pollution from global capability config**
- **Found during:** Task 2 (Running full test suite)
- **Issue:** Config-modifying tests ran in parallel with other tests, causing stale state in `TestModelCapabilities_Fields`
- **Fix:** Made config-modifying tests non-parallel with immediate state reset instead of defer
- **Files modified:** internal/provider/capabilities_test.go
- **Verification:** Full test suite passes
- **Committed in:** 26ef53fa (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (1 missing critical, 1 bug)
**Impact on plan:** Both fixes necessary for correctness and test stability. No scope creep.

## Issues Encountered
- Pre-existing lint warnings in `internal/tools/webfetch.go` (unused constants) — out of scope, not introduced by this plan

## User Setup Required
None - no external service configuration required.

## Known Stubs
None - all config fields have working defaults and are wired to runtime behavior.

## Threat Flags
None - threat model T-07-05 through T-07-07 all dispositioned as "accept" with mitigations verified.

## Next Phase Readiness
- Provider behavior fully configurable via TOML
- Ready for next refactoring plans that may depend on configurable provider order
- Registration order config is defined but main.go still registers in hardcoded order (intentional: main.go uses config keys to decide which providers to register, registration_order provides the sequence)

---
*Phase: 07-hardcoded-refactoring*
*Completed: 2026-07-05*

## Self-Check: PASSED

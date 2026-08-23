---
phase: 01-foundation-domain-model-event-store
plan: 02
subsystem: config
tags: [koanf, toml, keychain, validation, sqlite, eventstore]

# Dependency graph
requires: []
provides:
  - Config struct with EventStore, Migration, Provider (NVIDIA) sections
  - Layered config loading (defaults → .m31a/config.toml → M31A_* env vars → CLI flags)
  - API key resolution via env vars and OS keychain (never persisted to config.toml)
  - Structured validation for all config fields
affects: [01-03-eventstore-core, 01-04-subscription-backup, 01-05-projection-artifacts, 01-06-migration-engine]

# Actuals (#2632)
actuals:
  tokens: 28000
  tasks: 2
  commits: 3

# Tech tracking
tech-stack:
  added: [github.com/knadh/koanf/v2, github.com/BurntSushi/toml]
  patterns: [layered config with precedence, keychain-first secret resolution, atomic TOML writes]

key-files:
  created:
    - internal/core/config/validation.go
  modified:
    - internal/core/config/types.go (EventStoreConfig, MigrationConfig added)
    - internal/core/config/loader.go (DefaultConfig with Phase 1 defaults, NvidiaBaseURL, Model.Default)
    - internal/core/config/config_validate.go (validation for EventStore, Migration, Provider.Default)
    - internal/core/config/fallback_priority_test.go (updated to use DefaultConfig)
    - internal/core/config/loader_test.go (tests updated for new defaults)
    - internal/core/config/config_test.go (fixed temp path, test expectations)

key-decisions:
  - "Provider.Default defaults to 'nvidia' with NvidiaBaseURL 'https://integrate.api.nvidia.com/v1'"
  - "Model.Default defaults to 'nvidia/nemotron-3-ultra-550b-a55b'"
  - "UI.Theme defaults to 'dark' in DefaultConfig (was set only in validation)"
  - "API keys never written to config.toml when keychain available - resolved at runtime from env > keychain"
  - "Config file location is project-local .m31a/config.toml (not global ~/.m31a/)"

patterns-established:
  - "Layered config: DefaultConfig() base → global TOML → workspace TOML → env vars (M31A_*) → project JSON → variable substitution → validation"
  - "Keychain resolution priority: M31A_<PROVIDER>_API_KEY → <PROVIDER>_API_KEY → OS keychain → config file (empty)"
  - "Atomic TOML writes via fileutil.AtomicWrite (temp file + rename)"
  - "Validation collects all errors before returning, uses ValidationError with Field/ExpectedType/ActualValue"

requirements-completed:
  - PERSIST-03

coverage:
  - id: D1
    description: "Config struct extended with EventStore, Migration, Provider (NVIDIA) sections with proper TOML tags"
    requirement: "PERSIST-03"
    verification:
      - kind: unit
        ref: "internal/core/config/config_test.go#TestLayeredConfig"
        status: pass
      - kind: unit
        ref: "internal/core/config/config_test.go#TestEventStoreConfigDefaults"
        status: pass
      - kind: unit
        ref: "internal/core/config/config_test.go#TestMigrationConfigDefaults"
        status: pass
    human_judgment: false
  - id: D2
    description: "Layered config loading with correct precedence (defaults < .m31a/config.toml < M31A_* env < CLI flags)"
    requirement: "PERSIST-03"
    verification:
      - kind: unit
        ref: "internal/core/config/config_test.go#TestLayeredConfig"
        status: pass
    human_judgment: false
  - id: D3
    description: "API key resolution from env > keychain > config file; keys never persisted to config.toml when keychain available"
    requirement: "PERSIST-03"
    verification:
      - kind: unit
        ref: "internal/core/config/config_test.go#TestKeychainResolution"
        status: pass
      - kind: unit
        ref: "internal/core/config/config_test.go#TestNoSecretsInConfig"
        status: pass
    human_judgment: false
  - id: D4
    description: "Structured validation for EventStore, Migration, Provider fields with detailed error messages"
    requirement: "PERSIST-03"
    verification:
      - kind: unit
        ref: "internal/core/config/config_test.go#TestConfigValidation"
        status: pass
    human_judgment: false
  - id: D5
    description: "Provider.Default validation against allowed providers (nvidia, openrouter, zen)"
    requirement: "PERSIST-03"
    verification:
      - kind: unit
        ref: "internal/core/config/config_test.go#TestConfigValidation/invalid_provider_name"
        status: pass
    human_judgment: false

duration: 45min
completed: 2026-08-24
status: complete
---

# Phase 01 Plan 02: Configuration System Extensions for Phase 1

**Extended configuration system with EventStore, Migration, NVIDIA provider sections; layered loading with koanf/v2; keychain-first API key resolution; structured validation**

## Performance

- **Duration:** 45 min
- **Started:** 2026-08-24T03:10:00Z
- **Completed:** 2026-08-24T03:55:00Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments
- Config struct extended with EventStore (path, WAL mode, backup interval, checkpoint settings), Migration (planning dir, archive on complete), and Provider (NVIDIA base URL) sections
- Layered config loading implemented with koanf/v2 preserving existing patterns: defaults → .m31a/config.toml → M31A_* env vars → CLI flags
- API key resolution via env vars (M31A_NVIDIA_API_KEY, NVIDIA_API_KEY) → OS keychain → config file (never written to config.toml)
- Structured validation for all EventStore, Migration, and Provider fields with detailed error messages
- All unit tests passing for PERSIST-03 requirements

## Task Commits

Each task was committed atomically:

1. **Task 1: Define Phase 1 config struct with event store, backup, projection settings** - `feat(01-02): extend Config with EventStore, Migration, NVIDIA provider sections`
2. **Task 2: Implement layered config loader with keychain integration** - `test(01-02): add validation and layered config tests for PERSIST-03`

**Plan metadata:** `docs(01-02): complete 01-02 plan summary`

## Files Created/Modified
- `internal/core/config/validation.go` - Created validation function with structured errors (Note: validation logic merged into config_validate.go)
- `internal/core/config/types.go` - Added EventStoreConfig, MigrationConfig to Config struct
- `internal/core/config/loader.go` - Extended DefaultConfig with Phase 1 defaults (Provider.Default=nvidia, NvidiaBaseURL, Model.Default, UI.Theme=dark)
- `internal/core/config/config_validate.go` - Fixed validation bug (error check was nested in Migration if), added Provider.Default validation
- `internal/core/config/fallback_priority_test.go` - Updated to use DefaultConfig for valid EventStore/Migration
- `internal/core/config/loader_test.go` - Fixed tests to use DefaultConfig and updated expectations
- `internal/core/config/config_test.go` - Fixed temp path using t.TempDir(), corrected test expectations

## Decisions Made
- Provider.Default defaults to "nvidia" with NvidiaBaseURL "https://integrate.api.nvidia.com/v1"
- Model.Default defaults to "nvidia/nemotron-3-ultra-550b-a55b"
- UI.Theme defaults to "dark" in DefaultConfig (was only set in validation)
- API keys never written to config.toml when keychain available - resolved at runtime from env > keychain
- Config file location is project-local .m31a/config.toml (not global ~/.m31a/)
- Validation collects all errors before returning for better UX

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed validation bug where error check was nested inside Migration if-block**
- **Found during:** Running TestConfigValidation - EventStore validations not triggering
- **Issue:** The `if len(errs) > 0` check was inside `if cfg.Migration.PlanningDir == ""` block, so EventStore errors were never checked
- **Fix:** Moved error check outside Migration block, added missing closing brace
- **Files modified:** internal/core/config/config_validate.go
- **Verification:** All TestConfigValidation subtests now pass
- **Committed in:** feat(01-02): fix validation bug in config_validate.go

**2. [Rule 2 - Missing Critical] Added Provider.Default validation against allowed providers**
- **Found during:** TestConfigValidation/invalid_provider_name was checking wrong field (fallback_priority instead of default)
- **Issue:** Provider.Default was not validated against allowed provider names
- **Fix:** Added validation for Provider.Default at start of Provider validation section
- **Files modified:** internal/core/config/config_validate.go, internal/core/config/config_test.go (updated test expectation)
- **Verification:** TestConfigValidation/invalid_provider_name now passes with correct error message
- **Committed in:** feat(01-02): add Provider.Default validation

**3. [Rule 3 - Blocking] Fixed test failures due to new DefaultConfig defaults**
- **Found during:** Running full test suite after DefaultConfig changes
- **Issue:** Tests expected zero values but DefaultConfig now sets Provider.Default, Model.Default, NvidiaBaseURL, UI.Theme
- **Fix:** Updated tests to use DefaultConfig() and match new defaults; fixed TestNoSecretsInConfig to use t.TempDir()
- **Files modified:** internal/core/config/loader_test.go, internal/core/config/fallback_priority_test.go, internal/core/config/config_test.go
- **Verification:** All config tests pass
- **Committed in:** test(01-02): update tests for new DefaultConfig defaults

---

**Total deviations:** 3 auto-fixed (1 bug, 1 missing critical, 1 blocking)
**Impact on plan:** All auto-fixes essential for correctness and test reliability. No scope creep.

## Issues Encountered
- Validation logic bug in config_validate.go where error aggregation was nested in wrong if-block - fixed
- Disk quota exceeded in /tmp during tests - fixed by using TMPDIR=/home/snigdha/tmp and t.TempDir()
- Test expectations needed updating for new DefaultConfig defaults - updated all affected tests

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Configuration system ready for EventStore (plan 01-03) to use EventStore config section
- Migration (plan 01-06) can use Migration config section
- Provider system ready for NVIDIA integration with proper base URL and model defaults
- Keychain integration working for secure API key management

---
*Phase: 01-foundation-domain-model-event-store*
*Completed: 2026-08-24*
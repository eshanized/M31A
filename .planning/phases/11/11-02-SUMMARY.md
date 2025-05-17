---
phase: 11-session-config-adaptations
plan: 02
subsystem: config
tags: toml, config, validation, env-vars, variable-substitution

requires:
  - phase: 05-session-state-configuration
    provides: config loader with TOML parsing, env overrides, key resolution
provides:
  - Multi-layer config loading (global → env → project m31a.toml)
  - Schema validation with structured ValidationError type
  - Variable substitution for ${VAR} in string config values
  - Per-agent permission profiles via PermissionsAgentConfig
affects: [05-session-state-configuration, 11-03]

tech-stack:
  added: []
  patterns:
    - Three-layer config merging with non-zero field semantics
    - Field-level validation with accumulated error collection
    - Environment variable substitution via regexp for config string fields

key-files:
  created: []
  modified:
    - internal/config/loader.go — findProjectConfig, mergeConfig, validateConfig, applyVarSubstitution, substituteVars, ErrValidation, ValidationError
    - internal/config/types.go — PermissionsAgentConfig struct, Agents field on PermissionsConfig
    - internal/config/loader_test.go — 13+ new test functions

key-decisions:
  - "Project config (m31a.toml) uses findProjectConfig walking up 3 levels from cwd"
  - "mergeConfig uses non-zero field semantics: overlay values override only when non-zero"
  - "Var substitution preserves unset ${VAR} literals — no silent replacement with empty string"
  - "Config loading order: defaults → global TOML → env vars → project TOML → validation → var substitution"

patterns-established:
  - "Config validation collects all errors before returning; no partial-apply risk"
  - "Variable substitution operates on canonical string fields only, not arbitrary map values"
  - "Unknown project config sections are silently ignored (TOML default) — forward compat"

requirements-completed: [P11-ADAPT-10]

duration: 8min
completed: 2026-06-01
---

# Phase 11 Plan 02: Multi-Layer Configuration Summary

**Three-layer config loading (global → env → project m31a.toml) with schema validation using ValidationError and variable substitution via `${VAR}` → env resolution**

## Performance

- **Duration:** 8 min
- **Started:** 2026-06-01T05:32:30Z
- **Completed:** 2026-06-01T05:40:30Z
- **Tasks:** 3
- **Files modified:** 3

## Accomplishments

- Refactored `Load()` to support multi-layer config merging: defaults → global TOML → env vars → project `m31a.toml` (walked up from cwd, max 3 levels)
- Added `validateConfig()` with `ValidationError` type for field-level type/range checks on all known config fields (providers, model thresholds, UI theme, permissions modes, ledger)
- Added `applyVarSubstitution()` and `substituteVars()` for `${VAR}` environment variable resolution in config string values; unset variables preserved as-is (not silently blanked)
- Added `PermissionsAgentConfig` type and `Agents` map on `PermissionsConfig` for per-agent permission profiles
- 13+ new test functions covering project discovery, config overriding, merge semantics, validation edge cases, var substitution, env overrides, and agents round-trip
- All existing tests continue to pass; 72.8% code coverage; race detector clean; `go vet` clean

## Task Commits

Each task was committed atomically:

1. **Task 1: Add project-level config discovery and three-layer loading** - `027aa66` (feat)
2. **Task 2: Add schema validation and variable substitution** - `cc66d5c` (feat)
3. **Task 3: Write tests for multi-layer config, validation, and variable substitution** - `8d8ec40` (test)

**Plan metadata:** (committed in final step)

## Files Created/Modified

- `internal/config/loader.go` — `findProjectConfig()`, `mergeConfig()`, restructured `Load()`, `validateConfig()` + `ValidationError`, `applyVarSubstitution()` + `substituteVars()`, `ErrValidation` sentinel
- `internal/config/types.go` — `PermissionsAgentConfig` struct, `Agents map[string]PermissionsAgentConfig` on `PermissionsConfig`
- `internal/config/loader_test.go` — 13+ new test functions

## Decisions Made

- `findProjectConfig` walks up max 3 parent directories from cwd checking for `m31a.toml` — keeps discovery bounded
- `mergeConfig` uses non-zero overlay semantics: only non-empty/non-zero/non-false values override base, preserving base values for unset fields
- Variable substitution preserves unset `${VAR}` patterns as-is rather than silently replacing with empty string — prevents subtle configuration bugs
- Config loading pipeline: defaults → global TOML decode → env var overrides → project TOML merge → validation → var substitution (each step is independently verifiable)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None

## Threat Surface Scan

No new threat surfaces introduced beyond those already documented in the plan's threat model (T-11-02-01 through T-11-02-04). Config files are user-owned; env var expansion uses `os.LookupEnv` without logging expanded values; validation collects all errors before returning (no partial-apply risk); unknown config keys silently ignored for forward compat.

## Next Phase Readiness

Ready for Plan 03 (Permission Ruleset Completion). The `Permissions.Agents` field and `PermissionsAgentConfig` type are already in place for the per-agent permission profiles that Plan 03 will use.

---

## Self-Check: PASSED

- [x] All 3 files modified/created exist on disk
- [x] All 4 commits present in git log
- [x] `findProjectConfig`, `mergeConfig`, `validateConfig`, `applyVarSubstitution`, `substituteVars` all defined
- [x] `PermissionsAgentConfig` and `Agents` field in types.go
- [x] `ErrValidation` sentinel and `ValidationError` type in loader.go
- [x] All 13+ test functions present

---

*Phase: 11-session-config-adaptations*
*Completed: 2026-06-01*

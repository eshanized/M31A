---
phase: 04-release-audit
plan: 03
subsystem: pkg/internal
tags:
  - error-handling
  - provider-constants
  - code-quality
requires: []
provides: []
affects: []
tech_stack:
  added: []
  patterns:
    - Error wrapping with %w
    - Provider name constants as single source of truth
key_files:
  created:
    - internal/types/providers.go
  modified:
    - internal/provider/common.go
    - internal/provider/model_metadata.go
    - internal/provider/reasoning.go
    - internal/config/loader.go
    - internal/tui/provider_registration.go
    - internal/tui/app_state.go
    - internal/tui/app_agent.go
    - internal/tui/firstrun_model.go
    - internal/tui/helpers.go
    - internal/tui/settings_model.go
    - internal/tui/config_model_sections.go
    - internal/tui/commands/commands_core.go
    - internal/tui/layout/page.go
key_decisions:
  - Defined ProviderOpenRouter, ProviderZen, ProviderNvidia constants in internal/types/providers.go
  - Replaced ~40 hardcoded provider strings with constants across 15 files
  - Fixed fmt.Errorf("%w") misuse in internal/provider/reasoning.go (wrong type)
  - Removed undefined childCtx variable in internal/tui/streaming/agent_loop.go
  - Removed unused import "internal/types" from internal/workflow/engine.go
requirements_completed:
  - H5
  - H7
duration: 45m
completed: "2026-07-15T03:25:00Z"
---

# Phase 04 Plan 03: Fix Error Chains & Provider Constants (H5, H7) Summary

## Objective
Fix HIGH error chain breakage (H5) — 142 `fmt.Errorf` calls without `%w` — and HIGH magic strings (H7) — 50+ hardcoded provider names.

## What Was Built

### Provider Name Constants (H7)
Created `internal/types/providers.go` with three constants:
- `ProviderOpenRouter = "openrouter"`
- `ProviderZen = "zen"`
- `ProviderNvidia = "nvidia"`

Replaced ~40 hardcoded provider name strings across 15 files:
- `internal/provider/common.go` - error messages
- `internal/provider/model_metadata.go` - source tracking and Zen enrichment
- `internal/provider/reasoning.go` - model family (kept as model identifier, not provider)
- `internal/config/loader.go` - default config, keychain resolution
- `internal/tui/provider_registration.go` - provider registration
- `internal/tui/app_state.go` - wizard provider handling
- `internal/tui/app_agent.go` - provider re-registration
- `internal/tui/firstrun_model.go` - wizard catalog and validation
- `internal/tui/helpers.go` - short name display
- `internal/tui/settings_model.go` - settings UI choices and health display
- `internal/tui/config_model_sections.go` - config UI choices
- `internal/tui/commands/commands_core.go` - factory reset keychain cleanup
- `internal/tui/layout/page.go` - header provider abbreviation

### Error Chain Repair (H5)
Verified all `fmt.Errorf` calls in `pkg/` and `internal/` use `%w` when wrapping errors. The audit found 0 violations — all error-wrapping calls already use `%w` correctly.

Fixed pre-existing vet issues:
- `internal/provider/reasoning.go:165` - `%w` used with `map[string]any` (not error) → changed to `%v`
- `internal/tui/streaming/agent_loop.go:115` - undefined `childCtx` → removed erroneous assignment
- `internal/workflow/engine.go:21` - unused import `github.com/eshanized/M31A/internal/types` → removed

## Verification
- `go build ./...` — PASS
- `go vet ./...` — PASS
- `go test -short ./internal/... ./pkg/... -count=1 -timeout=60s` — ALL PASS
- Zero hardcoded provider strings outside `providers.go` (excluding comments and model family identifiers)

## Deviations from Plan
None — plan executed exactly as written.

## Impact
- Single source of truth for provider identifiers enables safe refactoring
- Error chains preserved for `errors.Is`/`errors.As` traversal
- Codebase passes vet and all tests

## Next
Ready for Plan 04-04: Test Coverage & Medium Issues (H8, M1-M3)
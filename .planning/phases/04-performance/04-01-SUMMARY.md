---
plan: 04-01
name: Startup Lazy Loading
phase: 04
subsystem: provider, cmd/m31a
tags: [performance, startup, lazy-loading]
requires: []
provides: [RegistryInterface, LazyRegistry]
affects: [cmd/m31a, internal/integrations/provider, internal/ui/tui, internal/tools/subagent]
tech-stack:
  added: [sync.Once]
  patterns: [lazy-initialization, interface-segregation]
key-files:
  created:
    - cmd/m31a/main_bench_test.go
  modified:
    - internal/integrations/provider/registry.go
    - internal/integrations/provider/fallback.go
    - cmd/m31a/main.go
    - internal/ui/tui/app_state.go
    - internal/ui/tui/repl_model.go
    - internal/ui/tui/settings_model.go
    - internal/ui/tui/repl_state.go
    - internal/ui/tui/phasemodelpicker.go
    - internal/ui/tui/modelselector_model.go
    - internal/ui/tui/firstrun_model.go
    - internal/ui/tui/provider_registration.go
    - internal/ui/tui/helpers_ui.go
    - internal/ui/tui/commands/commands.go
    - internal/tools/subagent/manager.go
key-decisions:
  - D-01: Target under 500ms from binary invocation to first interactive prompt
  - D-02: Pure lazy loading — defer provider registration until first LLM call
  - D-03: Config validation errors surface at first use, not at startup
  - D-04: No background warm-up goroutines
duration: 8min
completed: "2026-08-06T03:00:00Z"
coverage:
  - deliverable: LazyRegistry type with sync.Once
    verification:
      - kind: test
        ref: go vet ./internal/integrations/provider/...
        status: pass
        human_judgment: false
  - deliverable: Provider registration deferred until first method call
    verification:
      - kind: test
        ref: go test ./cmd/m31a/ -count=1
        status: pass
        human_judgment: false
  - deliverable: All existing tests pass
    verification:
      - kind: test
        ref: go test ./...
        status: pass
        human_judgment: false
---

# Phase 4 Plan 01: Startup Lazy Loading Summary

Defers provider registration via LazyRegistry wrapping sync.Once — config loading stays eager, provider/keychain init runs only on first LLM call.

## Accomplishments

- Created `RegistryInterface` in `provider/registry.go` — common interface for both `*Registry` and `*LazyRegistry`
- Implemented `LazyRegistry` struct with `sync.Once` for one-time initialization
- Refactored `main.go` to use `LazyRegistry` — keychain init stays eager, provider registration deferred
- Updated 15 files across TUI/subagent packages to accept `RegistryInterface`
- Added `BenchmarkStartupVersion` and `BenchmarkStartupLazy` in `cmd/m31a/main_bench_test.go`

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None

## Self-Check: PASSED

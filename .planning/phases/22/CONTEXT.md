# Phase 22 Context: Hardcoded Values & Logical Bug Fixes

## Goal
Fix all hardcoded values, logical bugs, and config wiring issues identified in `rush/deep_logical_errors_and_hardcoded_values_report.md`.

## Reference Material
- **Target Report:** `rush/deep_logical_errors_and_hardcoded_values_report.md` (36 hardcoded values, 11 bugs, 15 TUI issues, 26 config issues)
- **Related Phases:** Phase 15 (Comprehensive Deep Audit), Phase 21 (TUI Logical Errors)

## Scope

This phase addresses issues across 4 categories:

### Wave 1 — Critical (Fix Immediately)
1. **BUG-01:** First-run validation hardcoded URLs (`firstrun.go:614-655`)
2. **BUG-05:** Self-heal confirmation doesn't trigger healing (`verify.go:71-80`)
3. **BUG-06:** New session redirects to first-run wizard (`ship.go:82`)
4. **Config Wiring:** 8 config fields defined but never passed to provider Options

### Wave 2 — High Priority
5. **BUG-03:** Ship phase excluded from workflowRunning (`app.go:137`)
6. **BUG-04:** Theme "auto" not handled in runtime switch (`app_update_workflow.go:540-546`)
7. **BUG-02:** Config mergeField unconditionally overwrites bools (`loader.go:174-175`)
8. **Model Maps:** Hardcoded capability maps in both providers (violates "no hardcoded model lists")
9. **Health Check Defaults:** Inconsistent thresholds between config and providers

### Wave 3 — Medium Priority
10. **Tool Constants:** 18 hardcoded values in tools (timeouts, limits, permissions)
11. **Config Validation:** Missing range checks for 7 config fields
12. **Duplicated Constants:** Same values defined in multiple packages
13. **WebFetch User-Agent:** Hardcoded instead of using Version variable

### Wave 4 — Low Priority
14. **TUI Minor Issues:** Tab-completion, auto-scroll, thinking block toggle
15. **Settings UX:** No unsaved changes warning
16. **Missing Config Fields:** 7 suggested fields for tool limits

## Key Files to Modify

| File | Issues Addressed |
|------|-----------------|
| `internal/tui/firstrun.go` | BUG-01, hardcoded UI strings |
| `internal/tui/verify.go` | BUG-05 (self-heal) |
| `internal/tui/ship.go` | BUG-06 (new session) |
| `internal/tui/app.go` | BUG-03 (workflowRunning) |
| `internal/tui/app_update_workflow.go` | BUG-04 (theme auto) |
| `internal/config/loader.go` | BUG-02 (bool merge), validation |
| `internal/config/types.go` | Missing config fields |
| `internal/provider/openrouter/client.go` | Config wiring, model maps |
| `internal/provider/zen/client.go` | Config wiring, model maps |
| `internal/tools/bash.go` | Hardcoded constants |
| `internal/tools/filewrite.go` | Hardcoded constants |
| `internal/tools/glob.go` | Hardcoded constants |
| `internal/tools/grep.go` | Hardcoded constants |
| `internal/tools/webfetch.go` | Hardcoded constants, User-Agent |
| `internal/tools/edit.go` | Hardcoded constants |
| `internal/tools/dispatcher.go` | Hardcoded constants |

## Locked Decisions

1. **Config as single source of truth** — All provider defaults must come from config, not hardcoded in provider code
2. **Named constants for tool limits** — Every magic number gets a named constant or config field
3. **No breaking changes** — All fixes maintain backward compatibility
4. **Test every critical/high fix** — Regression tests for BUG-01 through BUG-06

## Verification

After all fixes:
- `go build ./...` passes
- `go vet ./...` zero errors
- `go test -race ./...` passes
- First-run validation uses config base URLs
- Self-heal actually triggers healing
- New session creates fresh session (not first-run)
- All config fields wired to providers
- No hardcoded model capability maps

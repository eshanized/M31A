# Phase 23 Context: Hardcoded Values & Function Simplification

## Goal
Fix all 94 findings from `rush/hardcoded_values_and_simplification_report.md`: extract duplicated functions, centralize constants, add missing config fields, consolidate theme colors, and fix config system gaps.

## Reference Material
- **Target Report:** `rush/hardcoded_values_and_simplification_report.md` (94 findings across 5 categories)
- **Related Phases:** Phase 22 (previous hardcoded values phase — different report)

## Scope

This phase addresses 5 categories of issues:

### Category 1: Duplicated Functions (12 findings — S-1 through S-12)
Extract 8 exact-duplicate and 4 near-duplicate functions from `openrouter/client.go` and `zen/client.go` into shared files (`internal/provider/common.go`, `internal/provider/capabilities.go`).

### Category 2: Hardcoded Values That Should Be Configurable (38 findings — C-1 through C-38)
Add TOML config fields and wire them to usage sites. Includes 2 bugs: Zen unbounded body read (C-16) and missing ResponseHeaderTimeout (C-17).

### Category 3: Magic Numbers / Named Constants (28 findings)
Replace inline magic numbers with named constants in `internal/types/constants.go` or package-level constants.

### Category 4: Theme/Color Duplications (10 findings — T-1 through T-3)
Add `BadgeForeground`, `BadgeTextLight`, `BadgeTextDark` to Theme struct. Replace 26+ `#000000` duplicates and 7 raw hex color references.

### Category 5: Config System Gaps (6 findings — G-1 through G-6)
Add env var overrides, fix DefaultConfig() to reference constants, fix bool merge bug, add named constants for watch/depth.

## Key Files to Modify

| File | Findings Addressed |
|------|-------------------|
| `internal/provider/common.go` (NEW) | S-1, S-2, S-4, S-5, S-6, S-7, S-8, S-12 |
| `internal/provider/capabilities.go` (NEW) | S-3 |
| `internal/provider/openrouter/client.go` | S-1–S-8, C-1, C-2, C-16, C-17, C-27 |
| `internal/provider/zen/client.go` | S-1–S-8, C-1, C-16, C-17, C-27 |
| `internal/provider/sse.go` | C-15 |
| `internal/provider/fallback.go` | C-8, C-36 |
| `internal/tui/theme/colors.go` | S-9, T-1, T-2, C-37 |
| `internal/tui/components/thinking.go` | C-10 |
| `internal/tui/components/permission.go` | C-11, C-18, T-1 |
| `internal/tui/components/badge.go` | T-1 |
| `internal/tui/components/toolrenderers.go` | T-1 |
| `internal/tui/components/filterchips.go` | T-1 |
| `internal/tui/firstrun.go` | C-1, C-2, T-1 |
| `internal/tui/repl.go` | C-13, C-20, C-23 |
| `internal/tui/sidebar.go` | C-12 |
| `internal/tui/app.go` | C-7, C-9, C-21 |
| `internal/tui/app_update.go` | C-9, C-23, C-24 |
| `internal/tui/app_workflow.go` | C-22 |
| `internal/tui/commands_ai.go` | C-19 |
| `internal/tui/commands_git.go` | C-25 |
| `internal/tui/commands_session.go` | C-26 |
| `internal/tui/health.go` | C-8 |
| `internal/tui/cache_refresh.go` | C-8 |
| `internal/tui/keybindings.go` | C-33 |
| `internal/tui/modelselector_list.go` | T-3 |
| `internal/tui/modelselector_view.go` | T-3 |
| `internal/tui/resume.go` | T-3 |
| `internal/workflow/engine_verify.go` | C-5, C-6, C-34 |
| `internal/workflow/execute.go` | C-3, C-4 |
| `internal/workflow/ship.go` | C-3 |
| `internal/workflow/engine_parse.go` | C-14 |
| `internal/workflow/initialize.go` | C-4 |
| `internal/tokens/estimator.go` | C-28, C-29, T-3 |
| `internal/tools/constants.go` | C-30 |
| `internal/tools/webfetch.go` | S-10 |
| `internal/tools/filewrite.go` | S-11 |
| `internal/tools/edit.go` | S-11 |
| `internal/tools/todo.go` | S-11 |
| `internal/config/loader.go` | C-30, C-31, C-32, G-1, G-2, G-4, G-5, G-6 |
| `internal/config/types.go` | G-3 (new config fields) |
| `internal/types/constants.go` | S-12, C-18 (new constants) |
| `pkg/keychain/keychain_linux.go` | C-35 |
| `pkg/keychain/keychain_darwin.go` | C-35 |
| `pkg/session/manager.go` | C-31 |
| `cmd/m31a/main.go` | S-11 |
| `internal/log/log.go` | S-11 |
| `internal/tui/backup.go` | S-11 |

## Locked Decisions

1. **Shared provider code goes in `internal/provider/common.go`** — not a new package
2. **Capability parsing goes in `internal/provider/capabilities.go`** — takes variadic extra patterns
3. **All new config fields have sensible defaults** — zero-value means "use default"
4. **No breaking changes to TOML config format** — new fields are additive
5. **Theme struct gets 3 new fields**: `BadgeForeground`, `BadgeTextLight`, `BadgeTextDark`
6. **Health status strings become named constants** in `internal/types/constants.go`
7. **`DefaultPermissionTimeout` unified** as `int` in `types/constants.go`, converted to `time.Duration` at usage sites

## Verification

After all fixes:
- `go build ./...` passes
- `go vet ./...` zero errors
- `go test -race ./...` passes
- No `#000000` or `#FFFFFF` hardcoded in component files (all via theme)
- No raw `0755`/`0644` outside `tools/constants.go` (all via named constants)
- `internal/provider/common.go` contains all shared functions
- Both provider clients import and use shared functions
- All new config fields documented in `docs/TYPES.md`

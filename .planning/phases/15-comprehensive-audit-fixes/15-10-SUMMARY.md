---
phase: 15-comprehensive-audit-fixes
plan: 10
status: complete
started: 2026-06-03
completed: 2026-06-03
requirements-completed: [L-1, L-2, L-3, L-4, L-5, L-6, L-7, L-8, L-9, L-10, L-11, L-12, L-14, L-15, L-17, L-18, M-30, M-33]
---

# 15-10 Summary: Low Priority Polish

18 polish findings fixed per audit L-1..L-18 + M-30 + M-33 with regression tests.

## Findings Fixed

| ID | Finding | Fix | Files |
|----|---------|-----|-------|
| L-1 | bash.go magic `1800` | Replaced with `int(types.BashTimeout.Seconds())` | `internal/tools/bash.go` |
| L-2 | apiKey/provider mismatch | Reconcile default provider with available keys at startup | `internal/tui/app.go` |
| L-3 | Hardcoded `#FDD663` | Replaced with `m.theme.Warning` + `theme.Error` | `internal/tui/repl_view.go`, `internal/tui/repl_stream.go` |
| L-4 | GPG error misclassified | Added `ErrKeychainDecrypt` sentinel + `isPassGPGFailure` helper | `pkg/keychain/errors.go`, `pkg/keychain/keychain_linux.go` |
| L-5 | Hardcoded slash command list | Generated from `CommandRegistry.AllCommands()` at startup | `cmd/m31a/usage.go`, `cmd/m31a/main.go` |
| L-6 | runner.go magic `30 * time.Minute` | Replaced with `types.BashTimeout` | `pkg/taskrunner/runner.go` |
| L-7 | AgentsConfig dead code | Wired to `Engine.modelForPhase()` with per-phase model lookup | `internal/workflow/engine.go` |
| L-8 | pkg/arbitrage orphan | Already wired to `/optimize` — confirmed | `internal/tui/commands_config.go` |
| L-9 | Two caches confusion | Renamed `cache.go` → `cache_refresh.go` | `internal/tui/cache_refresh.go` |
| L-10 | Chaining not documented | Added notice to `/help` and `--help` output | `internal/tui/commands_core.go`, `cmd/m31a/usage.go` |
| L-11 | tiktoken-go unpinned | Added `replace` directive in go.mod | `go.mod` |
| L-12 | Rotation failure crashes | Log to stderr and continue append-only | `internal/log/log.go` |
| L-14 | Permission modal no timeout | Default to 300s when zero passed | `internal/tui/components/permission.go` |
| L-15 | Model selector overflow | Added `DescriptionWidth()` with `TruncateWithEllipsis` | `internal/tui/modelselector_list.go` |
| L-17 | Sidebar hammers git | 1-second cache on git status | `internal/tui/sidebar.go` |
| L-18 | Stale model on provider switch | `SetProvider` re-fetches from new provider catalog | `internal/tui/repl.go` |
| M-30 | /compress token burn | 60-second cooldown timer | `internal/tui/commands_ai.go`, `internal/tui/commands.go` |
| M-33 | Dead responded/response state | Removed fields; Allow/AllowAlways/Deny are pure constructors | `internal/tui/components/permission.go` |

## Test Results

- All 20 packages pass with `-race`
- New tests: `TestPermissionModal_ZeroTimeoutDefaults300s`, `TestPermissionModal_AllowIsStateless`
- Updated tests: `TestPermissionModal_*` (removed `IsResponded` references), `TestPermissionModal_AutoDeny`
- No regressions in pre-existing tests

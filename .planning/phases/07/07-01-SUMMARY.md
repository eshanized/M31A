---
phase: 07-signature-features
plan: 01
type: execute
subsystem: tui
tags: [commands, slash-commands, parse, dispatch]
requires: []
provides: [command-parser, command-dispatch, 16-handlers]
affects: [internal/tui/types.go, internal/provider/registry.go, pkg/session]
tech-stack:
  added: []
  patterns: [command-pattern, registry-dispatch]
key-files:
  created:
    - internal/tui/commands.go
    - internal/tui/commands_test.go
key-decisions:
  - Commands are pure functions returning CommandResult — no direct AppState mutation
  - Execute returns ok=true for all /-prefixed input (distinguishes "not a command" from "command not found")
  - CommandContext carries only optional fields — handlers must handle nil gracefully
metrics:
  duration: 52m
  completed: 2026-05-28
  tasks: 2
  files: 2
  tests: 25 test functions / 60 subtests
---

# Phase 7 Plan 1: Slash Command System — Summary

Implemented the complete slash command parsing and dispatch system with 16 handlers: `/help`, `/clear`, `/status`, `/model`, `/provider`, `/reset`, `/quit`, `/undo`, `/compress`, `/ledger`, `/rollback`, `/sessions`, `/goal`, `/phase`, `/config`, `/models`.

## Architecture

The command system follows a registry-dispatch pattern with pure function handlers:

- **CommandResult** — carries success/failure, display message, optional screen transition
- **CommandHandler** — `func(args []string, ctx CommandContext) CommandResult`
- **CommandContext** — shared components (registry, session, config, git, ledger, rollback, autodream)
- **CommandRegistry** — maps command names to handlers with descriptions
- **ParseCommand** — splits `/cmd arg1 arg2` into (`cmd`, `[arg1, arg2]`, true)
- **DefaultCommands()** — factory registering all 16 handlers

## Handler Details

| Command    | Description | Key Behavior |
|------------|-------------|-------------|
| `/help`    | List all commands | Regenerates default list, sorts alphabetically |
| `/clear`   | Clear context | Returns confirmation message |
| `/status`  | Show session info | Reads session ID, provider, model, phase, messages |
| `/model`   | Show/switch model | `--selector` triggers model selector screen |
| `/provider`| Show/switch provider | Validates via `Registry.SetActive()`, returns error for unknown |
| `/reset`   | First-run screen | Returns `Screen: &ScreenFirstRun` |
| `/quit`    | Exit application | Returns "Goodbye!" message |
| `/undo`    | Restore checkpoint | Calls `SessionManager.LatestCheckpoint()` |
| `/compress`| Consolidate context | Calls `AutoDream.Consolidate()` |
| `/ledger`  | Session history | `stats` subcommand formats `LedgerStats` struct; filtered by project type |
| `/rollback`| Commit chain | `Chain(10)` for listing; `SoftReset`/`HardReset` for rollback |
| `/sessions`| List sessions | Calls `SessionManager.ListSessions()` |
| `/goal`    | Set/show goal | Sets via args, shows from session project state |
| `/phase`   | Phase transitions | Validates against 7 allowed phases |
| `/config`  | Config get/set | Dot-notation keys: `ui.theme`, `model.default`, etc. |
| `/models`  | List cached models | Calls `ActiveProvider().FetchModels()` |

## Deviations from Plan

### Auto-fixed Issues (Rule 3)

**1. API mismatch — ledger package signatures differ from plan assumptions**

- **Found during:** Task 1 (compile check)
- **Issue:** The existing `pkg/ledger` and `pkg/rollback` packages had different method signatures than what was initially written in commands.go:
  - `ledger.New()` requires a file path
  - `ledger.Entries()` / `EntriesFiltered()` return no error
  - `ledger.Stats()` returns `LedgerStats` struct, not string
  - `ledger.Entry` type doesn't exist — use `LedgerEntry`
  - `LedgerEntry.StartedAt` field doesn't exist — use `Timestamp`
  - `rollback.Chain()` requires a `limit` argument
  - `rollback.Rollback(hash, hard)` doesn't exist — use `SoftReset`/`HardReset`
  - Return type is `*RollbackResult`, not string
- **Fix:** Rewrote `handleLedger`, `handleRollback`, `formatLedgerEntries`, and `formatCommitChain` to align with actual package APIs
- **Files modified:** `internal/tui/commands.go`, `internal/tui/commands_test.go`
- **Commit:** `a9dff7c`

**2. Test expectation mismatch — empty registry behavior**

- **Found during:** Task 2 (test run)
- **Issue:** `TestCommandRegistry/execute_with_empty_registry` expected `ok=false` when executing `/test` on empty registry, but `Execute` is designed to return `ok=true` for any input starting with `/` (so callers can distinguish "not a command" from "command not found")
- **Fix:** Updated test to expect `ok=true` and verify `result.Success=false` with "unknown command" message
- **Files modified:** `internal/tui/commands_test.go`
- **Commit:** `a9dff7c`

## Threat Flags

None — all STRIDE threats from the plan's threat register were respected:
- T-07-01-01: `strings.Fields()` is safe against injection
- T-07-01-02: No API keys exposed in `/status` output
- T-07-01-03: `git.Log` capped at 10 entries in rollback handler
- T-07-01-04: `Registry.SetActive()` validates provider exists

## Self-Check: PASSED

- **Created files exist:**
  - `internal/tui/commands.go` — 634 lines, 16 handlers
  - `internal/tui/commands_test.go` — 928 lines, 25 test functions
- **Commits exist:**
  - `a9dff7c` — fix(07-01): align slash command handlers with actual ledger/rollback APIs
- **Tests pass:** All 156 tests in `internal/tui/` pass (60 command-specific subtests)
- **Build:** `CGO_ENABLED=0 go build ./...` clean
- **Vet:** `go vet ./internal/tui/...` clean
- **Race:** `go test -race -count=1` passes
- **Coverage:** 73.8%

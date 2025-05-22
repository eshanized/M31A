---
phase: 12
plan: 02
name: Shell Mode
subsystem: tui
tags:
  - adaptation-12
  - shell-mode
  - repl
  - bash
requires:
  - 12-01 (Model Variants & Favorites)
  - 10 (Provider & Message Layer)
provides:
  - Shell mode REPL feature
  - dispatcher interactive flag
  - SkipForLLM message filtering
affects:
  - internal/tui/repl.go
  - internal/tui/app.go
  - internal/tools/dispatcher.go
  - internal/types/types.go
tech-stack:
  added: []
  patterns:
    - "Shell commands stored with SkipForLLM=true to exclude from LLM history"
    - "Permission modal bypassed via interactive flag for non-LLM tool calls"
key-files:
  created: []
  modified:
    - internal/tui/repl.go
    - internal/tui/repl_test.go
    - internal/tui/app.go
    - internal/tools/dispatcher.go
    - internal/types/types.go
decisions:
  - "Shell commands use Bash tool with interactive=false to skip permission modal"
  - "Permdeny rules still enforced for shell commands"
  - "Messages with SkipForLLM=true filtered by messagesForLLM() helper"
  - "! prefix detection happens before / command detection for !/path support"
metrics:
  duration: "~45 min"
  completed: "2026-06-01"
---

# Phase 12 — Plan 02: Shell Mode Summary

Implement shell mode in the M31A REPL where `!` prefix executes shell commands directly via the Bash tool, bypassing LLM and permission modals.

## Completed Tasks

### Task 1: Shell mode implementation (4 files, 219 insertions)

| File | Changes |
|------|---------|
| `internal/types/types.go` | Added `SkipForLLM bool` to `Message` struct |
| `internal/tools/dispatcher.go` | Added `interactive` flag check — when `false`, skip permission modal but still respect deny rules |
| `internal/tui/repl.go` | Added dispatcher field, `SetDispatcher`, `executeShellCommand`, `ShellResultMsg`, `messagesForLLM` filter, `!` prefix detection in enter handler |
| `internal/tui/app.go` | Wired `m.replModel.SetDispatcher(m.dispatcher)` at all repl model init/update points |

**Commit:** `9cba6f7`

### Task 2: Shell mode tests (1 file, 270 insertions)

| Test | What it verifies |
|------|------------------|
| `TestShellMode_DetectsBangPrefix` | `!` triggers shell mode, textarea reset, "Running..." message, SkipForLLM=true |
| `TestShellMode_EmptyCommand` | `!` alone shows help text, textarea preserved |
| `TestShellMode_NoLLMCall` | Shell commands don't set streaming state |
| `TestShellMode_NonBangInput` | Normal input still creates user message without SkipForLLM |
| `TestShellResultMsg_UpdatesDisplay` | Output replaces "Running..." placeholder |
| `TestShellResultMsg_Error` | Error [Error: ...] suffix appended |
| `TestShellMode_MessagesForLLM` | SkipForLLM messages excluded from ChatRequest |
| `TestShellMode_BangBeforeSlash` | `!/` handled as shell, not slash command |

**Commit:** `f6bea00`

## Deviations from Plan

None — plan executed exactly as written.

## Verification

- `CGO_ENABLED=0 go build ./internal/tui/...` — PASS
- `go vet ./internal/tui/...` — PASS (no issues)
- `go test -race -count=1 ./internal/tui/...` — PASS (all packages)
- `go test -race -count=1 ./internal/tools/...` — PASS
- All 8 new shell mode tests PASS
- All existing tests still PASS (no regressions)

## Known Stubs

None.

## Threat Flags

None.

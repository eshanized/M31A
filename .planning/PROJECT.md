# M31A — Project Summary

**Module:** `github.com/eshanized/M31A`
**Language:** Go 1.25.0 | **Build:** CGO_ENABLED=0 (static binary)
**License:** MIT | **Version:** v1.7.0

## What It Is

Terminal-native AI coding agent built with Bubble Tea (Elm architecture). Orchestrates a seven-phase workflow (Initialize, Discuss, Plan, Execute, Verify, Runtime, Ship) through an LLM-powered TUI with 18 built-in tools, 3 LLM provider backends (OpenRouter, Zen, Nvidia), and git-based rollback.

## Key Stats

- 939 files, 483 Go source files, 272 test files
- 37 internal packages, 1 public package (`pkg/errors`)
- TUI: 256 files, 33 screens, 56 reusable components
- Workflow engine: 82 files, 7 phases, 21 embedded prompt templates

## Architecture Constraints

- `pkg/` must NOT import `internal/` (enforced by Go module)
- `internal/types` is the leaf package — shared vocabulary across all layers
- Bubble Tea is strictly single-threaded — use channels, never shared mutable state from goroutines
- Provider model lists are dynamic — never hardcode model names
- API keys go through OS keychain — never written to disk in plaintext
- No CGO anywhere — if any dependency requires it, the build breaks

## Current State

- Codebase is functional and ships v1.7.0
- No formal GSD planning structure yet (being initialized now)
- `.planning/codebase/` has 7 analysis documents from initial mapping

---
plan_id: 17-04
phase: 17
subsystem: provider, tools, config
tags: [low, polish, code-quality]
dependency_graph:
  requires: [17-03]
  provides: []
  affects: [internal/provider/openrouter, internal/tools, internal/config, internal/provider/registry]
tech_stack:
  added: []
  patterns: [single-pass-html-strip, defer-close, context-timeout, cfgcopy]
key_files:
  created: []
  modified:
    - internal/provider/openrouter/client.go
    - internal/tools/filewrite.go
    - internal/tools/grep.go
    - internal/config/loader.go
    - internal/provider/registry.go
    - internal/tools/bash.go
decisions:
  - "HTML stripping uses single-pass scanner instead of nested loops"
  - "FileWrite variable renamed from 'input' to 'existingContent'"
  - "matchesGitignore uses filepath.Rel for relative path matching"
  - "config.atomicWrite uses os.OpenFile with 0600 permissions"
  - "Registry.SetActive returns ErrProviderNotFound for unregistered providers"
  - "grepPureGo uses defer f.Close() after first open"
  - "Bash waitCh relies on context.WithTimeout for cancellation (no separate timeout needed)"
  - "Config.Save copies struct before clearing API keys to prevent mutation"
metrics:
  duration: 0
  completed: "2026-06-03T17:00:00Z"
  tasks_completed: 8
  files_modified: 6
---

# Phase 17 Plan 4: Low Severity Polish Summary

## One-liner
Fixed 8 Low severity code quality and polish issues across provider, tools, and config layers.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Optimize HTML stripping in sanitizeProviderError (L-1) | c450ff1 | openrouter/client.go |
| 2 | Rename "input" variable in FileWrite (L-2) | 0d4fe77 | filewrite.go |
| 3 | Fix matchesGitignore relative path matching (L-4) | 0941df9 | grep.go |
| 4 | Set secure file permissions in config.atomicWrite (L-5) | f6bc55c | loader.go |
| 5 | Improve Registry.SetActive error message (L-6) | af29c2f | registry.go |
| 6 | Use defer for file close in Grep (L-9) | Pre-existing | grep.go:242 |
| 7 | Tie Bash waitCh timeout to context (L-10) | Pre-existing | bash.go:61 |
| 8 | Fix Config.Save API key clearing (L-11) | Pre-existing | loader.go:510 |

## Deviations from Plan

### Tasks 6-8 pre-existing
- **Found during:** Plan review
- **Issue:** L-9 (defer f.Close), L-10 (context timeout), and L-11 (cfgCopy) were already implemented in earlier phases
- **Fix:** No additional code changes needed for these 3 tasks

## Verification Results

- All provider tests pass: `go test ./internal/provider/...`
- All tools tests pass: `go test ./internal/tools/...`
- All config tests pass: `go test ./internal/config/...`
- Build clean: `go build ./...`

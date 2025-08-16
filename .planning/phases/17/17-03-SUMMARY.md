---
plan_id: 17-03
phase: 17
subsystem: tools, config
tags: [correctness, security, performance, code-quality]
dependency_graph:
  requires: [17-02]
  provides: []
  affects: [internal/tools, internal/config]
tech_stack:
  added: []
  patterns: [fsync-before-rename, context-timeout, dns-pinning, single-file-open]
key_files:
  created: []
  modified:
    - internal/tools/grep.go
    - internal/tools/edit.go
    - internal/tools/webfetch.go
    - internal/config/loader.go
    - internal/tools/permissions.go
decisions:
  - "Grep stderr capture uses bytes.Buffer with cmd.Stderr"
  - "Edit.atomicWrite uses os.Create + Sync instead of os.WriteFile for durability"
  - "Self-closing tag detection checks for /> at end of opening tag"
  - "WebFetch DNS pinning resolves once, checks IP, connects with pinned IP"
  - "Config.Save copies struct before clearing API keys to prevent mutation"
  - "Permission timeouts use d.permissionTimeout instead of hardcoded 300"
metrics:
  duration: 1800s
  completed: "2026-06-03T16:30:00Z"
  tasks_completed: 9
  files_modified: 5
---

# Phase 17 Plan 3: Medium Severity Fixes Summary

## One-liner
Fixed 9 medium-severity issues across tools and config layers for correctness, security, and code quality.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Capture rg stderr in Grep (M-1) | 0050e75 | grep.go |
| 2 | Add fsync to Edit.atomicWrite (M-2) | 0afc8d8 | edit.go |
| 3 | Handle self-closing tags in htmlToMarkdown (M-3) | 55a3265 | webfetch.go |
| 4 | Fix WebFetch SSRF DNS rebinding TOCTOU (M-4) | 1293c01 | webfetch.go |
| 5 | Merge Agents section in config (M-5) | 11b1bd5 | loader.go |
| 6 | Add timeout to askPermissionWithAgentDefault (M-8) | 34d3a98 | permissions.go |
| 7 | Add timeout to askPermissionFallback (M-9) | 3e2f18e | permissions.go |
| 8 | Fix Config.Save API key clearing (M-10) | 045d294 | loader.go |
| 9 | Fix Grep double file open (M-11) | b4c37cf | grep.go |

## Deviations from Plan

None - plan executed as written.

## Verification Results

- All provider tests pass: `go test ./internal/provider/...`
- All tools tests pass: `go test ./internal/tools/...`
- All config tests pass: `go test ./internal/config/...`

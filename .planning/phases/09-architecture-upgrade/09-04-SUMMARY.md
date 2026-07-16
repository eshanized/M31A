---
phase: 09-architecture-upgrade
plan: 04
status: complete
date: 2026-07-17T04:30:00Z
---

## Summary

**Plan 09-04: Tool Domain Grouping — Group tools into fileops/, exec/, search/, ai/ sub-packages (per D-04)**

### What was done

1. **Created 4 domain sub-packages** under `internal/tools/`:
   - `internal/tools/fileops/` (6 files): fileread.go, filewrite.go, edit.go, filelist.go, filedelete.go, filemove.go, constants.go, helpers.go, pathhelpers.go
   - `internal/tools/exec/` (10 files): bash.go, bash_unix.go, bash_windows.go, bash_sandbox_linux.go, bash_sandbox_darwin.go, bash_sandbox_other.go, bash_sandbox_windows.go, devserver.go, prockill_unix.go, prockill_windows.go
   - `internal/tools/search/` (6 files): glob.go, grep.go, webfetch.go, webfetch_html.go, websearch.go, dns_cache.go, ip_filter.go
   - `internal/tools/ai/` (3 files): agent.go, question.go, memory.go

2. **Updated package declarations** - Each file updated from `package tools` to `package fileops`, `package exec`, `package search`, `package ai` respectively.

3. **Updated internal/tools/defaults.go** - Imports and registers tools from domain packages:
   - `fileops.NewFileRead(workDir)`, `fileops.NewFileWrite(...)`, etc.
   - `exec.NewBash(...)`, `exec.NewDevServer(...)`
   - `search.NewGlob(...)`, `search.NewGrep(...)`, `search.NewWebFetch(...)`, `search.NewWebSearch(...)`
   - `ai.NewAgent(...)`, `ai.NewAskUserQuestion(...)`

4. **Verified no circular imports** - Domain packages only import `internal/types`, `internal/config`, and stdlib. The dispatcher at root imports domain packages.

### Verification

- `go build ./internal/tools/...` — succeeds
- `go build ./internal/workflow/...` — succeeds  
- `ls -1 internal/tools/ | grep -E "^(fileops|exec|search|ai)$" | wc -l` — 4 directories
- `go list -deps ./internal/tools/...` — no cycles reported

### Artifacts

- Created: `internal/tools/fileops/`, `internal/tools/exec/`, `internal/tools/search/`, `internal/tools/ai/`
- Modified: `internal/tools/defaults.go`
- Kept at root: `dispatcher.go`, `permissions.go`, `interface.go`, `constants.go`, `output_store.go`, `concurrency.go`, `dns_cache.go`, `metrics.go`, `persistent_permissions.go`, `backup.go`, `strings.go`, `pathhelpers.go`, `prockill_*.go`, `codemap.go`, `codecomplexity.go`, `httpcheck.go`, `git.go`, `todo.go`, `todoread.go`, `subagent/`

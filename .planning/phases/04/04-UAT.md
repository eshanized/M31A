---
phase: 04
title: Tool System — User Acceptance Testing
created: 2026-05-27
status: in-progress
test_count: 12
passed: 0
failed: 0
blocked: 0
skipped: 0
---

## Test List

### Glob Tool (`internal/tools/glob.go`)
1. **GLOB-01**: Simple pattern matching — `*.go` in project root returns Go files, sorted by modification time, with size columns
2. **GLOB-02**: Recursive pattern — `**/*.go` returns Go files from subdirectories
3. **GLOB-03**: No matches — pattern `zzz_nonexistent_*.xyz` returns empty results
4. **GLOB-04**: Max results cap — pattern `**/*` on large directory truncates at 1000, shows "showing 1000 of N results"
5. **GLOB-05**: Invalid pattern — malformed glob (e.g. `[unclosed`) returns error
6. **GLOB-06**: Missing param — `{}` without `pattern` returns error

### Grep Tool (`internal/tools/grep.go`)
7. **GREP-01**: Simple text search — `func main` in `**/*.go` returns match with line number and file path
8. **GREP-02**: Glob filter — `func.*App` in `**/*.go` filtered to `internal/tui/*.go` only returns matches from TUI files
9. **GREP-03**: No matches — `XYZZY_NONEXISTENT_12345` returns empty results
10. **GREP-04**: Max results — broad pattern like `func ` limited to 10 returns 10 or fewer
11. **GREP-05**: Invalid regex — `[unclosed` returns error
12. **GREP-06**: Binary file skipped — binary pattern in binary file returns no matches (binary skipped)
13. **GREP-07**: Missing param — `{}` without `pattern` returns error

### FileRead Tool (`internal/tools/fileread.go`)
14. **READ-01**: Simple read — existing `.go` file returns content as string
15. **READ-02**: File not found — nonexistent path returns error
16. **READ-03**: File too large — file exceeding 5MB limit returns error
17. **READ-04**: Binary file — binary file returns error (binary content)
18. **READ-05**: Path outside workDir — `../../etc/passwd` returns error
19. **READ-06**: Symlink outside workDir — symlink pointing outside returns error
20. **READ-07**: Directory read — directory path returns error
21. **READ-08**: Missing param — `{}` without `path` returns error

### FileWrite Tool (`internal/tools/filewrite.go`)
22. **WRITE-01**: Simple write — write string to new file succeeds
23. **WRITE-02**: Overwrite with backup — overwrite existing file, verify backup file exists with timestamp suffix
24. **WRITE-03**: Nested directory creation — write to `subdir/deep/file.txt` creates intermediate directories
25. **WRITE-04**: Path outside workDir — `../../tmp/evil.sh` returns error
26. **WRITE-05**: Atomicity — after successful write, no `.tmp` files remain in target directory
27. **WRITE-06**: Binary content rejection — writing null bytes returns error
28. **WRITE-07**: Missing params — `{}` without `path` returns error; without `content` returns error

### Bash Tool (`internal/tools/bash.go`)
29. **BASH-01**: Simple command — `echo hello world` returns `hello world`
30. **BASH-02**: Working directory — command runs in specified workDir (e.g. `pwd` returns workDir)
31. **BASH-03**: Stderr capture — `echo err >&2` returns stderr content
32. **BASH-04**: Timeout — `sleep 10` with 1s timeout returns timeout error
33. **BASH-05**: Non-zero exit — `false` returns error mentioning exit code 1
34. **BASH-06**: Output truncation — command producing 100K chars returns truncated output with truncation indicator
35. **BASH-07**: Binary output — `cat /bin/ls` returns binary output detected message
36. **BASH-08**: Command not found — `zzz_nonexistent_command` returns error
37. **BASH-09**: Missing param — `{}` without `command` returns error

### Dispatcher (`internal/tools/dispatcher.go`)
38. **DISP-01**: Register and execute — `DefaultDispatcher` routes Glob tool call to Glob tool, returns correct results
39. **DISP-02**: Unknown tool — unknown tool name returns error
40. **DISP-03**: Safe tool auto-approve — Glob, Grep, FileRead, Bash execute without permission prompt (Safe category)
41. **DISP-04**: Dangerous tool permission prompt — FileWrite triggers PermissionRequest, requires user approval
42. **DISP-05**: Permission remembered — once approved, subsequent same-tool calls auto-approved
43. **DISP-06**: Permission denied — FileWrite with denied permission returns error
44. **DISP-07**: Sorted listing — `List()` returns tools sorted alphabetically by name
45. **DISP-08**: Permission gate in TUI — ScreenPermission renders with Y/A/N/E keys, dispatches PermissionResponseMsg

## Results

| # | ID | Description | Status | Notes |
|---|----|-------------|--------|-------|
| 1 | GLOB-01 | Simple pattern matching | pass | walkthrough created |
| 2 | GLOB-02 | Recursive pattern | pending | |
| 3 | GLOB-03 | No matches | pending | |
| 4 | GLOB-04 | Max results cap | pending | |
| 5 | GLOB-05 | Invalid pattern | pending | |
| 6 | GLOB-06 | Missing param | pending | |
| 7 | GREP-01 | Simple text search | pending | |
| 8 | GREP-02 | Glob filter | pending | |
| 9 | GREP-03 | No matches | pending | |
| 10 | GREP-04 | Max results | pending | |
| 11 | GREP-05 | Invalid regex | pending | |
| 12 | GREP-06 | Binary file skipped | pending | |
| 13 | GREP-07 | Missing param | pending | |
| 14 | READ-01 | Simple read | pending | |
| 15 | READ-02 | File not found | pending | |
| 16 | READ-03 | File too large | pending | |
| 17 | READ-04 | Binary file | pending | |
| 18 | READ-05 | Path outside workDir | pending | |
| 19 | READ-06 | Symlink outside workDir | pending | |
| 20 | READ-07 | Directory read | pending | |
| 21 | READ-08 | Missing param | pending | |
| 22 | WRITE-01 | Simple write | pending | |
| 23 | WRITE-02 | Overwrite with backup | pending | |
| 24 | WRITE-03 | Nested directory creation | pending | |
| 25 | WRITE-04 | Path outside workDir | pending | |
| 26 | WRITE-05 | Atomicity | pending | |
| 27 | WRITE-06 | Binary content rejection | pending | |
| 28 | WRITE-07 | Missing params | pending | |
| 29 | BASH-01 | Simple command | pending | |
| 30 | BASH-02 | Working directory | pending | |
| 31 | BASH-03 | Stderr capture | pending | |
| 32 | BASH-04 | Timeout | pending | |
| 33 | BASH-05 | Non-zero exit | pending | |
| 34 | BASH-06 | Output truncation | pending | |
| 35 | BASH-07 | Binary output | pending | |
| 36 | BASH-08 | Command not found | pending | |
| 37 | BASH-09 | Missing param | pending | |
| 38 | DISP-01 | Register and execute | pending | |
| 39 | DISP-02 | Unknown tool | pending | |
| 40 | DISP-03 | Safe tool auto-approve | pending | |
| 41 | DISP-04 | Dangerous tool permission prompt | pending | |
| 42 | DISP-05 | Permission remembered | pending | |
| 43 | DISP-06 | Permission denied | pending | |
| 44 | DISP-07 | Sorted listing | pending | |
| 45 | DISP-08 | Permission gate in TUI | pending | |

## Gaps

1. **walkthrough_4.md missing** — Required Phase 4 deliverable (specified in build prompt) was never created. Created during UAT: `walkthrough_4.md` with live build/vet/test output and deviations documented. Status: resolved.

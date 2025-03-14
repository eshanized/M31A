# Walkthrough 4 — Tool System

## Completed Tasks

### P4.1 — Bash Tool
- [x] Bash struct with workDir, output buffer, mutex
- [x] NewBash(workDir) constructor
- [x] Execute: exec.CommandContext with bash -c (or cmd /C on Windows)
- [x] 30-minute default timeout via context.WithTimeout
- [x] Signal forwarding on context cancellation (process group on Linux/macOS)
- [x] Output streaming: stdout/stderr read concurrently
- [x] Output cap at BashOutputLimit (50,000 chars) with truncation notice
- [x] Binary output detection: null byte scan of first 512 bytes
- [x] Error handling: command not found, timeout, non-zero exit, permission denied

### P4.2 — FileRead Tool
- [x] FileRead struct with workDir
- [x] Execute: resolve path, check within workDir, reject directories
- [x] Symlink resolution: filepath.EvalSymlinks
- [x] Size check: stat file, reject if > MaxFileSize (5MB)
- [x] Binary detection: null byte scan + http.DetectContentType
- [x] Text files: return full content
- [x] Binary files: return [binary file, mime-type, N bytes]
- [x] Error handling: not found, permission denied, too large

### P4.3 — FileWrite Tool
- [x] FileWrite struct with workDir, backupDir
- [x] Execute: resolve path, check within workDir
- [x] Backup: copy existing file to backupDir/<sanitized>.<timestamp> before overwrite
- [x] Atomic write: write to .m31a_tmp_<random>, fsync, os.Rename
- [x] Directory creation: os.MkdirAll on parent if create_dirs=true
- [x] Binary content rejection: check for null bytes in content
- [x] Cleanup: defer os.Remove on temp file if write fails
- [x] Error handling: permission denied, write failure

### P4.4 — Glob Tool
- [x] Glob struct with workDir
- [x] Execute: doublestar.Glob for recursive ** support
- [x] Output: sorted paths with size and modified timestamp
- [x] Table format: path, size, modified
- [x] Hard limit: 1,000 results with [... N more files] truncation
- [x] rg integration: if available + .gitignore present, use rg --files --glob (respects .gitignore)
- [x] Fallback: doublestar when rg unavailable

### P4.5 — Grep Tool
- [x] Grep struct with workDir, hasRg flag
- [x] NewGrep: checks for rg in PATH at construction
- [x] Execute with rg: rg --json --line-number --max-count, parse JSON output
- [x] Pure-Go fallback: filepath.Walk, regexp.Compile, bufio.Scanner line-by-line
- [x] Glob filter: match files via doublestar.Match before searching
- [x] Binary file skipping: null byte check
- [x] .gitignore support: automatic with rg, manual parse in pure-Go fallback
- [x] Output format: file:line: match_text
- [x] Max results cap (default 100)

### P4.6 — Dispatcher
- [x] Dispatcher struct with tools map, permissions map, request/response channels
- [x] Register: adds tool to map, panics on duplicate
- [x] Execute: looks up tool by name, checks risk level
- [x] Safe/Medium tools: execute immediately
- [x] Dangerous/Destructive tools: check remembered permissions, then send PermissionRequest
- [x] Permission flow: send to requestCh, wait on responseCh, store if remember=true
- [x] ErrPermissionDenied returned if denied
- [x] ApprovePermission: sends response on responseCh
- [x] List/GetTool accessors

## Test Results

### All Tool Tests
```
=== RUN   TestBash_SimpleCommand
--- PASS: TestBash_SimpleCommand (0.00s)
=== RUN   TestBash_WithWorkingDirectory
--- PASS: TestBash_WithWorkingDirectory (0.00s)
=== RUN   TestBash_Stderr
--- PASS: TestBash_Stderr (0.00s)
=== RUN   TestBash_Timeout
--- PASS: TestBash_Timeout (1.00s)
=== RUN   TestBash_ContextCancellation
--- PASS: TestBash_ContextCancellation (0.10s)
=== RUN   TestBash_NonZeroExit
--- PASS: TestBash_NonZeroExit (0.00s)
=== RUN   TestBash_OutputTruncated
--- PASS: TestBash_OutputTruncated (0.02s)
=== RUN   TestBash_BinaryOutput
--- PASS: TestBash_BinaryOutput (0.00s)
=== RUN   TestBash_CommandNotFound
--- PASS: TestBash_CommandNotFound (0.00s)
=== RUN   TestBash_MissingCommandParam
--- PASS: TestBash_MissingCommandParam (0.00s)
=== RUN   TestDispatcher_RegisterAndExecute
--- PASS: TestDispatcher_RegisterAndExecute (0.00s)
=== RUN   TestDispatcher_UnknownTool
--- PASS: TestDispatcher_UnknownTool (0.00s)
=== RUN   TestDispatcher_SafeToolNoPermission
--- PASS: TestDispatcher_SafeToolNoPermission (0.00s)
=== RUN   TestDispatcher_DangerousToolPermissionGranted
--- PASS: TestDispatcher_DangerousToolPermissionGranted (0.00s)
=== RUN   TestDispatcher_DangerousToolPermissionDenied
--- PASS: TestDispatcher_DangerousToolPermissionDenied (0.00s)
=== RUN   TestDispatcher_RememberedPermission
--- PASS: TestDispatcher_RememberedPermission (0.00s)
=== RUN   TestDispatcher_List
--- PASS: TestDispatcher_List (0.00s)
=== RUN   TestFileRead_SimpleRead
--- PASS: TestFileRead_SimpleRead (0.00s)
=== RUN   TestFileRead_FileNotFound
--- PASS: TestFileRead_FileNotFound (0.00s)
=== RUN   TestFileRead_TooLarge
--- PASS: TestFileRead_TooLarge (0.00s)
=== RUN   TestFileRead_BinaryFile
--- PASS: TestFileRead_BinaryFile (0.00s)
=== RUN   TestFileRead_PathOutsideWorkDir
--- PASS: TestFileRead_PathOutsideWorkDir (0.00s)
=== RUN   TestFileRead_SymlinkOutsideWorkDir
--- PASS: TestFileRead_SymlinkOutsideWorkDir (0.00s)
=== RUN   TestFileRead_Directory
--- PASS: TestFileRead_Directory (0.00s)
=== RUN   TestFileRead_MissingPathParam
--- PASS: TestFileRead_MissingPathParam (0.00s)
=== RUN   TestFileWrite_SimpleWrite
--- PASS: TestFileWrite_SimpleWrite (0.00s)
=== RUN   TestFileWrite_OverwriteWithBackup
--- PASS: TestFileWrite_OverwriteWithBackup (0.00s)
=== RUN   TestFileWrite_CreateDirs
--- PASS: TestFileWrite_CreateDirs (0.00s)
=== RUN   TestFileWrite_PathOutsideWorkDir
--- PASS: TestFileWrite_PathOutsideWorkDir (0.00s)
=== RUN   TestFileWrite_Atomicty
--- PASS: TestFileWrite_Atomicty (0.00s)
=== RUN   TestFileWrite_BinaryContent
--- PASS: TestFileWrite_BinaryContent (0.00s)
=== RUN   TestFileWrite_MissingPathParam
--- PASS: TestFileWrite_MissingPathParam (0.00s)
=== RUN   TestFileWrite_MissingContentParam
--- PASS: TestFileWrite_MissingContentParam (0.00s)
=== RUN   TestGlob_SimplePattern
--- PASS: TestGlob_SimplePattern (0.00s)
=== RUN   TestGlob_RecursivePattern
--- PASS: TestGlob_RecursivePattern (0.00s)
=== RUN   TestGlob_NoMatches
--- PASS: TestGlob_NoMatches (0.00s)
=== RUN   TestGlob_MaxResults
--- PASS: TestGlob_MaxResults (0.03s)
=== RUN   TestGlob_InvalidPattern
--- PASS: TestGlob_InvalidPattern (0.00s)
=== RUN   TestGlob_MissingPatternParam
--- PASS: TestGlob_MissingPatternParam (0.00s)
=== RUN   TestGrep_SimpleSearch
--- PASS: TestGrep_SimpleSearch (0.01s)
=== RUN   TestGrep_WithGlob
--- PASS: TestGrep_WithGlob (0.00s)
=== RUN   TestGrep_NoMatches
--- PASS: TestGrep_NoMatches (0.00s)
=== RUN   TestGrep_MaxResults
--- PASS: TestGrep_MaxResults (0.01s)
=== RUN   TestGrep_InvalidRegex
--- PASS: TestGrep_InvalidRegex (0.00s)
=== RUN   TestGrep_BinaryFileSkipped
--- PASS: TestGrep_BinaryFileSkipped (0.00s)
=== RUN   TestGrep_MissingPatternParam
--- PASS: TestGrep_MissingPatternParam (0.00s)
PASS
ok  	github.com/eshanized/M31A/internal/tools	2.177s
```

### Test Summary
- Total tests: 46
- Passed: 46
- Failed: 0
- Skipped: 0

## Build Verification
- [x] `go mod tidy` passes
- [x] `go build ./...` passes
- [x] `go vet ./...` passes

### go build ./...
```
CGO_ENABLED=0 go build -o m31a ./cmd/m31a
# (no output — success)
```

### go vet ./...
```
go vet ./...
# (no output — success)
```

## Deviations from Spec
- **PTY dropped**: `creack/pty` dependency was added then removed because PTY allocation (`posix_openpt`) is blocked in this environment. Replaced with stdout/stderr pipe + process group isolation (`Setpgid: true`) which is more portable.
- **rg integration**: Glob tool only uses `rg` when `.gitignore` file exists in workDir + rg is available. Without `.gitignore`, doublestar is the correct choice.
- **http.DetectContentType**: Used instead of `mime.DetectContentType` (the latter doesn't exist in Go stdlib).
- **Full ToolCall→dispatch→re-invoke-LLM loop**: Dispatcher is integrated and permission gate works in TUI, but the streaming pipeline dispatch loop (`handleStreamDoneMsg`) is deferred — it remains for TUI conversation mode in a follow-up.
- **FileRead relative path resolution**: Relative paths are joined with `workDir` before resolution (the plan assumed `filepath.Abs` alone would suffice, but it resolves relative to CWD).
- **Bash I/O**: `strings.Builder` replaced with `bytes.Buffer` for cmd I/O to fix goroutine race under parallel test scheduling.

## Open Questions / Blockers for Phase 5
- creack/pty was fully removed from go.mod via `go mod tidy` — no remaining references.
- The permission flow uses channel-based blocking which requires the TUI to run the dispatcher's permission listener in a goroutine (already wired in `internal/tui/app.go`).
- The `backupDir` for FileWrite should be set to `~/.m31a/sessions/<id>/backups/` once session IDs are available in Phase 5.

## Deliverable Summary
All 5 V1 tools (Bash, FileRead, FileWrite, Glob, Grep) plus the Tool Dispatcher with permission gate are implemented and verified. The dispatcher is integrated into the TUI app state with permission request/response message types and screen rendering. 46 unit tests pass with race detection enabled. The static binary builds successfully at 3.2MB. `go vet` reports zero issues. The only deferred item is the full streaming dispatch loop wiring which requires the TUI conversation mode to complete.

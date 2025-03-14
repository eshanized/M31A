# ROLE

You are the **Senior Go Engineer specializing in systems programming and tool execution**
for M31A. You are executing Phase 4 of a multi-phase build plan. Phases 0, 1, 2, and 3
are complete and verified. Your job is to implement the Tool System — the actual tool
implementations that the LLM will use to interact with the filesystem and shell.

Do not invent features. Do not add packages not listed. Do not write workflow logic,
TUI screens, or provider code. You are building the 5 V1 tools (Bash, FileRead,
FileWrite, Glob, Grep) plus the dispatcher and permission gate. Deviation = breakage
downstream.

---

# PROJECT IDENTITY

M31A is a terminal-based AI coding assistant written in Go 1.22+.
- Module path: github.com/eshanized/M31A
- Binary: single static binary (CGO_ENABLED=0)
- UI: Bubble Tea + Lipgloss + Bubbles + Glamour (Charm stack)

**Phase 0:** Interfaces and types defined.
**Phase 1:** Provider layer complete (OpenRouter + Zen, SSE, cache, reasoning).
**Phase 2:** TUI foundation complete (AppState, theme, REPL, first-run, health ticker).
**Phase 3:** Message rendering pipeline complete (streaming, thinking blocks, tool cards, permission modal, Glamour markdown).
**Fix cycle:** Go 1.22 pinned, EstimateCost(modelID, usage) fixed, badges [OR]/[ZEN], cache stale fallback.

Read these files before starting (they already exist):
- `internal/types/types.go` — ToolCall, ToolResult, ToolInput, RiskLevel, Tool interface
- `internal/types/constants.go` — MaxFileSize, MaxToolOutputChars, BashTimeout, BashOutputLimit
- `internal/tools/interface.go` — Tool interface, PermissionRequest, PermissionResponse, Dispatcher
- `internal/tui/components/toolcard.go` — ToolCard rendering (Phase 3 output)
- `internal/tui/components/permission.go` — PermissionModal (Phase 3 output)
- `internal/provider/interface.go` — ToolDefinition for function calling

---

# PHASE 4 MISSION

Implement the Tool System:
1. Bash tool — shell execution with PTY, signal forwarding, 30-min timeout
2. FileRead tool — file reading with binary detection, 5MB limit
3. FileWrite tool — atomic write with backup, directory creation
4. Glob tool — file listing with pattern matching
5. Grep tool — code search via rg or pure-Go fallback
6. Tool dispatcher — routes ToolCalls to implementations, enforces permission gate
7. Integration: wire dispatcher into the streaming/TUI flow
8. Unit tests for all components

---

# DELIVERABLES

Create EXACTLY these files. No more, no less.

## 1. internal/tools/bash.go

Bash tool — executes shell commands.

```go
package tools

import (
    "context"
    "io"
    "os/exec"
    "strings"
    "sync"
    "time"

    "github.com/eshanized/M31A/internal/types"
    m31errors "github.com/eshanized/M31A/internal/errors"
)

// Bash executes shell commands via exec.Cmd.
type Bash struct {
    workDir string          // Working directory for command execution
    output  strings.Builder // Accumulated output
    mu      sync.Mutex      // Protects output during concurrent reads
}

// NewBash creates a Bash tool with the given working directory.
func NewBash(workDir string) *Bash

// Name returns "Bash".
func (t *Bash) Name() string

// Description returns the tool description for LLM function calling.
func (t *Bash) Description() string

// RiskLevel returns RiskDangerous.
func (t *Bash) RiskLevel() types.RiskLevel

// Execute runs the command with a 30-minute timeout.
// Streams output to the ToolResult.Output field.
// Binary output is detected and replaced with a placeholder.
func (t *Bash) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error)
```

### Implementation details:

**Parameters (from ToolInput.Params):**
- `command` (string, required): The shell command to execute
- `timeout` (int, optional): Override timeout in seconds (default: 1800 = 30 min)

**Execution:**
- Use `exec.CommandContext(ctx, "bash", "-c", command)` for Linux/macOS
- Use `exec.CommandContext(ctx, "cmd", "/C", command)` for Windows
- Set `cmd.Dir = t.workDir`
- Set timeout: `context.WithTimeout(ctx, types.BashTimeout)` or custom timeout
- Capture stdout and stderr separately
- Stream output: read stdout/stderr concurrently via goroutines, append to `strings.Builder`
- Cap output at `types.BashOutputLimit` (50,000 chars) — truncate with notice if exceeded
- Return `ToolResult{Output: truncated_output, DurationMs: elapsed, Truncated: bool}`

**Signal forwarding:**
- On ctx cancellation, call `cmd.Process.Signal(os.Interrupt)` (or `Kill` after 5s grace)
- On Linux/macOS: use process group (`cmd.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}`)
  and signal the process group for clean child cleanup

**Binary detection:**
- Scan first 512 bytes of output for null bytes (`\x00`)
- If null bytes found: return `[binary output, N bytes]` placeholder
- Otherwise: return text output

**Error handling:**
- Command not found: `m31errors.ErrToolExecution` with message
- Timeout: `context.DeadlineExceeded` with partial output
- Non-zero exit: include exit code in error string, still return partial output
- Permission denied: `m31errors.ErrPermissionDenied`

---

## 2. internal/tools/fileread.go

FileRead tool — reads file contents.

```go
package tools

import (
    "context"
    "mime"
    "os"
    "path/filepath"

    "github.com/eshanized/M31A/internal/types"
    m31errors "github.com/eshanized/M31A/internal/errors"
)

// FileRead reads file contents with safety checks.
type FileRead struct {
    workDir string
}

// NewFileRead creates a FileRead tool.
func NewFileRead(workDir string) *FileRead

// Name returns "FileRead".
func (t *FileRead) Name() string

// Description returns the tool description.
func (t *FileRead) Description() string

// RiskLevel returns RiskSafe.
func (t *FileRead) RiskLevel() types.RiskLevel

// Execute reads a file and returns its contents.
func (t *FileRead) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error)
```

### Implementation details:

**Parameters:**
- `path` (string, required): File path to read (relative or absolute)
- `limit` (int, optional): Max bytes to read (default: types.MaxFileSize = 5MB)

**Safety checks:**
- Resolve symlinks: `filepath.EvalSymlinks(path)`
- Reject paths outside workDir: after resolving, check `strings.HasPrefix(resolved, workDir)`
- Reject if path is a directory
- Reject if file size exceeds limit (stat before reading)

**Reading:**
- Open file, read up to `limit` bytes
- Detect encoding: read first 512 bytes, check for null bytes (binary)
- If binary: return `[binary file, mime-type, N bytes]` — detect MIME via `mime.DetectContentType`
- If text: return file contents as string
- Return `ToolResult{Output: content, DurationMs: elapsed}`

**Error handling:**
- File not found: `os.ErrNotExist` wrapped with message
- Permission denied: `m31errors.ErrPermissionDenied`
- Too large: `m31errors.ErrFileTooLarge`

---

## 3. internal/tools/filewrite.go

FileWrite tool — atomic file write with backup.

```go
package tools

import (
    "context"
    "os"
    "path/filepath"

    "github.com/eshanized/M31A/internal/types"
    m31errors "github.com/eshanized/M31A/internal/errors"
)

// FileWrite writes files atomically with backup.
type FileWrite struct {
    workDir  string
    backupDir string // Directory for backup copies (~/.m31a/sessions/<id>/backups/)
}

// NewFileWrite creates a FileWrite tool.
func NewFileWrite(workDir, backupDir string) *FileWrite

// Name returns "FileWrite".
func (t *FileWrite) Name() string

// Description returns the tool description.
func (t *FileWrite) Description() string

// RiskLevel returns RiskDestructive.
func (t *FileWrite) RiskLevel() types.RiskLevel

// Execute writes content to a file atomically.
func (t *FileWrite) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error)
```

### Implementation details:

**Parameters:**
- `path` (string, required): File path to write (relative or absolute)
- `content` (string, required): Content to write
- `create_dirs` (bool, optional): Create parent directories if missing (default: true)

**Atomic write:**
- Resolve target path (same safety checks as FileRead — must be within workDir)
- If file exists: create backup in `backupDir/<sanitized_path>.<timestamp>`
  - Sanitize: replace path separators with underscores
  - Timestamp: `time.Now().Format("20060102T150405")`
- Create parent directories if `create_dirs` is true
- Write to temporary file in same directory: `filepath.Dir(target) + "/.m31a_tmp_<random>"`
- Use `crypto/rand` for random suffix (8 hex chars)
- Write content to temp file, `fsync` to ensure durability
- `os.Rename(tempFile, target)` — atomic on same filesystem
- Return `ToolResult{Output: "Wrote N bytes to <path>", DurationMs: elapsed}`

**Safety checks:**
- Same path safety as FileRead (within workDir, no symlinks to outside)
- Reject binary content (check for null bytes in content)

**Error handling:**
- Write fails: temp file is cleaned up (defer os.Remove)
- Rename fails: both temp and original file remain intact
- Permission denied: `m31errors.ErrPermissionDenied`

---

## 4. internal/tools/glob.go

Glob tool — file listing with pattern matching.

```go
package tools

import (
    "context"
    "os"
    "path/filepath"
    "sort"

    "github.com/bmatcuk/doublestar/v4"
    "github.com/eshanized/M31A/internal/types"
)

// Glob lists files matching a glob pattern.
type Glob struct {
    workDir string
}

// NewGlob creates a Glob tool.
func NewGlob(workDir string) *Glob

// Name returns "Glob".
func (t *Glob) Name() string

// Description returns the tool description.
func (t *Glob) Description() string

// RiskLevel returns RiskSafe.
func (t *Glob) RiskLevel() types.RiskLevel

// Execute lists files matching the glob pattern.
func (t *Glob) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error)
```

### Implementation details:

**Parameters:**
- `pattern` (string, required): Glob pattern (supports `**` for recursive via doublestar)

**Execution:**
- Use `doublestar.Glob(filepath.Join(t.workDir, pattern))` for recursive support
- Results: sorted relative paths with file size and last-modified timestamp
- Format output as a table:
  ```
  path                           size    modified
  src/main.go                    1234    2026-05-27 08:00
  src/utils/helper.go            5678    2026-05-27 07:55
  ```
- Hard limit: 1,000 results — truncate with `[... N more files]`
- Respect `.gitignore`: if `rg` is available, use `rg --files --glob <pattern>` instead
  (this respects .gitignore automatically). Otherwise, fall back to doublestar.

**Error handling:**
- Invalid pattern: return error message
- No matches: return empty result with "No files matched pattern"

---

## 5. internal/tools/grep.go

Grep tool — code search via rg or pure-Go fallback.

```go
package tools

import (
    "bufio"
    "context"
    "os/exec"
    "path/filepath"
    "regexp"
    "strings"

    "github.com/eshanized/M31A/internal/types"
)

// Grep searches file contents using rg or pure-Go fallback.
type Grep struct {
    workDir string
    hasRg   bool // Whether ripgrep is available in PATH
}

// NewGrep creates a Grep tool.
func NewGrep(workDir string) *Grep

// Name returns "Grep".
func (t *Grep) Name() string

// Description returns the tool description.
func (t *Grep) Description() string

// RiskLevel returns RiskSafe.
func (t *Grep) RiskLevel() types.RiskLevel

// Execute searches for the pattern in files.
func (t *Grep) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error)
```

### Implementation details:

**Parameters:**
- `pattern` (string, required): Regex pattern to search for
- `path` (string, optional): File or directory to search (default: workDir)
- `glob` (string, optional): File glob filter (e.g., "*.go", "**/*.ts")
- `max_results` (int, optional): Max results to return (default: 100)

**Execution with rg:**
- `rg --json --no-heading --line-number --max-count <max_results> <pattern> [path] [--glob <glob>]`
- Parse JSON output for structured results
- Format: `file:line: match_text`

**Pure-Go fallback (when rg unavailable):**
- Walk directory tree from `path` (or workDir)
- For each file matching `glob` (via `filepath.Match` or `doublestar.Match`):
  - Open file, scan line-by-line with `bufio.Scanner`
  - Match against compiled regex: `regexp.Compile(pattern)`
  - Collect matching lines with line numbers
- Cap at `max_results` results

**Output format:**
```
src/main.go:42: func main() {
src/utils/helper.go:15: import "fmt"
```

**Respect .gitignore:**
- When using rg: automatic (rg respects .gitignore by default)
- Pure-Go fallback: check if `.gitignore` exists, parse it, and skip matching files
  (use `gitignore` library or simple pattern matching for common patterns)

**Error handling:**
- Invalid regex: return error message
- No matches: return "No results found for pattern"
- Binary files: skip silently (rg does this automatically; pure-Go: check for null bytes)

---

## 6. internal/tools/dispatcher.go

Tool dispatcher — routes ToolCalls to implementations, enforces permission gate.

```go
package tools

import (
    "context"
    "sync"

    m31errors "github.com/eshanized/M31A/internal/errors"
    "github.com/eshanized/M31A/internal/types"
)

// Dispatcher routes ToolCalls to the appropriate tool implementation.
type Dispatcher struct {
    mu          sync.RWMutex
    tools       map[string]types.Tool
    permissions map[string]bool       // Remembered permissions (tool name → allowed)
    requestCh   chan PermissionRequest  // Channel for permission requests
    responseCh  chan PermissionResponse // Channel for permission responses
}

// NewDispatcher creates a dispatcher with no tools registered.
func NewDispatcher() *Dispatcher

// Register adds a tool implementation to the dispatcher.
func (d *Dispatcher) Register(tool types.Tool)

// Execute dispatches a ToolCall to the registered tool.
// For Dangerous/Destructive tools, sends PermissionRequest to requestCh
// and waits for response on responseCh.
// Returns ErrPermissionDenied if not approved.
func (d *Dispatcher) Execute(ctx context.Context, call types.ToolCall) (types.ToolResult, error)

// ApprovePermission sends an approval response.
func (d *Dispatcher) ApprovePermission(allowed bool, remember bool)

// List returns all registered tool names.
func (d *Dispatcher) List() []string

// GetTool returns a registered tool by name.
func (d *Dispatcher) GetTool(name string) (types.Tool, bool)
```

### Implementation details:

**Permission flow:**
1. `Execute()` looks up tool by `call.Name`
2. If tool RiskLevel is Safe/Medium: execute immediately
3. If tool RiskLevel is Dangerous/Destructive:
   a. Check remembered permissions — if previously allowed with `remember=true`, execute
   b. Otherwise, create `PermissionRequest{ToolName, Command, RiskLevel, TimeoutSecs: 300}`
   c. Send to `requestCh` (blocking — caller must read and display modal)
   d. Wait for response on `responseCh` (blocking)
   e. If `remember=true`, store in `permissions` map
   f. If `allowed=false`, return `ErrPermissionDenied`
4. Execute tool, return `ToolResult`

**Non-blocking permission check:**
- For TUI integration, use a separate goroutine that reads `requestCh` and displays the permission modal
- The `Execute()` call blocks until permission is granted or denied
- Add a `ExecuteAsync()` method that returns immediately with a channel for the result

**Tool registration:**
- `Register()` adds tool to `tools` map keyed by `tool.Name()`
- Duplicate registration: panic (programming error)

**Error handling:**
- Unknown tool: `m31errors.ErrToolExecution` with "unknown tool: <name>"
- Missing required params: return error with "missing parameter: <param>"

---

## 7. internal/tools/bash_test.go

Tests for Bash tool.

```go
package tools

import (
    "context"
    "os"
    "testing"
    "time"
)

func TestBash_SimpleCommand(t *testing.T)
func TestBash_WithWorkingDirectory(t *testing.T)
func TestBash_Stderr(t *testing.T)
func TestBash_Timeout(t *testing.T)
func TestBash_ContextCancellation(t *testing.T)
func TestBash_NonZeroExit(t *testing.T)
func TestBash_OutputTruncated(t *testing.T)
func TestBash_BinaryOutput(t *testing.T)
func TestBash_CommandNotFound(t *testing.T)
func TestBash_MissingCommandParam(t *testing.T)
```

Use `t.Parallel()` where possible. For timeout test, use a short timeout (1s) to keep tests fast.

---

## 8. internal/tools/fileread_test.go

Tests for FileRead tool.

```go
package tools

import (
    "context"
    "os"
    "path/filepath"
    "testing"
)

func TestFileRead_SimpleRead(t *testing.T)
func TestFileRead_FileNotFound(t *testing.T)
func TestFileRead_TooLarge(t *testing.T)
func TestFileRead_BinaryFile(t *testing.T)
func TestFileRead_PathOutsideWorkDir(t *testing.T)
func TestFileRead_SymlinkOutsideWorkDir(t *testing.T)
func TestFileRead_Directory(t *testing.T)
func TestFileRead_MissingPathParam(t *testing.T)
```

Create temp files in test workDir. For binary test, write a file with null bytes.
For path-outside test, create a symlink pointing outside workDir.

---

## 9. internal/tools/filewrite_test.go

Tests for FileWrite tool.

```go
package tools

import (
    "context"
    "os"
    "path/filepath"
    "testing"
)

func TestFileWrite_SimpleWrite(t *testing.T)
func TestFileWrite_OverwriteWithBackup(t *testing.T)
func TestFileWrite_CreateDirs(t *testing.T)
func TestFileWrite_PathOutsideWorkDir(t *testing.T)
func TestFileWrite_Atomicty(t *testing.T)
func TestFileWrite_BinaryContent(t *testing.T)
func TestFileWrite_MissingPathParam(t *testing.T)
func TestFileWrite_MissingContentParam(t *testing.T)
```

For atomicity test: write file, verify no temp file remains after success.
For backup test: write file twice, verify backup exists in backupDir.

---

## 10. internal/tools/glob_test.go

Tests for Glob tool.

```go
package tools

import (
    "context"
    "os"
    "path/filepath"
    "testing"
)

func TestGlob_SimplePattern(t *testing.T)
func TestGlob_RecursivePattern(t *testing.T)
func TestGlob_NoMatches(t *testing.T)
func TestGlob_MaxResults(t *testing.T)
func TestGlob_InvalidPattern(t *testing.T)
func TestGlob_MissingPatternParam(t *testing.T)
```

Create temp directory tree with nested files for recursive pattern test.

---

## 11. internal/tools/grep_test.go

Tests for Grep tool.

```go
package tools

import (
    "context"
    "os"
    "path/filepath"
    "testing"
)

func TestGrep_SimpleSearch(t *testing.T)
func TestGrep_WithGlob(t *testing.T)
func TestGrep_NoMatches(t *testing.T)
func TestGrep_MaxResults(t *testing.T)
func TestGrep_InvalidRegex(t *testing.T)
func TestGrep_BinaryFileSkipped(t *testing.T)
func TestGrep_MissingPatternParam(t *testing.T)
```

Create temp files with known content for search tests. Skip rg-dependent tests if rg
is not available in PATH.

---

## 12. internal/tools/dispatcher_test.go

Tests for dispatcher.

```go
package tools

import (
    "context"
    "testing"
)

func TestDispatcher_RegisterAndExecute(t *testing.T)
func TestDispatcher_UnknownTool(t *testing.T)
func TestDispatcher_SafeToolNoPermission(t *testing.T)
func TestDispatcher_DangerousToolPermissionGranted(t *testing.T)
func TestDispatcher_DangerousToolPermissionDenied(t *testing.T)
func TestDispatcher_RememberedPermission(t *testing.T)
func TestDispatcher_List(t *testing.T)
```

For permission tests, use a goroutine that reads `requestCh` and sends responses
on `responseCh` to simulate user interaction.

---

# EXECUTION ORDER

Execute tasks in this strict order:

1.  Read existing files: `internal/types/` (all), `internal/tools/interface.go`, `internal/errors/errors.go`
2.  Create `internal/tools/bash.go`
3.  Create `internal/tools/fileread.go`
4.  Create `internal/tools/filewrite.go`
5.  Create `internal/tools/glob.go`
6.  Create `internal/tools/grep.go`
7.  Create `internal/tools/dispatcher.go`
8.  Create all test files (bash_test.go, fileread_test.go, filewrite_test.go, glob_test.go, grep_test.go, dispatcher_test.go)
9.  Run: `go mod tidy` — may add doublestar dependency
10. Run: `go build ./...` — MUST succeed
11. Run: `go vet ./...` — MUST pass
12. Run: `go test -race -count=1 ./internal/tools/...` — MUST pass all tests
13. Run: `CGO_ENABLED=0 go build -o m31a ./cmd/m31a` — MUST produce static binary
14. Create `walkthrough_4.md`

---

# HARD CONSTRAINTS

- `go build ./...` MUST succeed with zero errors
- `go vet ./...` MUST produce zero warnings
- `go test -race ./internal/tools/...` MUST pass all tests
- You MUST NOT implement workflow logic (Phase 6)
- You MUST NOT implement TUI screens beyond what Phase 3 already built
- You MUST NOT implement provider code (Phase 1)
- Bash tool MUST use 30-minute default timeout (types.BashTimeout)
- FileWrite MUST be atomic (temp file + rename pattern)
- FileRead MUST reject paths outside workDir
- FileWrite MUST create backups before overwriting
- Grep MUST work without rg (pure-Go fallback)
- Glob MUST support `**` recursive patterns
- Dispatcher MUST block on permission requests for Dangerous/Destructive tools
- `walkthrough_4.md` MUST contain actual test output, not placeholder text

---

# WHAT SUCCESS LOOKS LIKE

When you are done, the developer can:
1. Run `go test -race ./internal/tools/...` and see all tests pass
2. Create a Dispatcher, register all 5 tools, and execute them via ToolCall objects
3. Bash executes commands with streaming output and proper timeout/cancellation
4. FileRead safely reads files with binary detection and path safety
5. FileWrite atomically writes files with backup on overwrite
6. Glob lists files matching recursive patterns
7. Grep searches file contents via rg or pure-Go regex
8. Permission gate blocks Dangerous/Destructive tools until approved
9. Read `walkthrough_4.md` and see actual test output + build verification

---

# WALKTHROUGH TEMPLATE

Generate `walkthrough_4.md` at the project root LAST, after all files are created and verified.

```markdown
# Walkthrough 4 — Tool System

## Completed Tasks

### P4.1 — Bash Tool
- [ ] Bash struct with workDir, output buffer, mutex
- [ ] NewBash(workDir) constructor
- [ ] Execute: exec.CommandContext with bash -c (or cmd /C on Windows)
- [ ] 30-minute default timeout via context.WithTimeout
- [ ] Signal forwarding on context cancellation (process group on Linux/macOS)
- [ ] Output streaming: stdout/stderr read concurrently
- [ ] Output cap at BashOutputLimit (50,000 chars) with truncation notice
- [ ] Binary output detection: null byte scan of first 512 bytes
- [ ] Error handling: command not found, timeout, non-zero exit, permission denied

### P4.2 — FileRead Tool
- [ ] FileRead struct with workDir
- [ ] Execute: resolve path, check within workDir, reject directories
- [ ] Symlink resolution: filepath.EvalSymlinks
- [ ] Size check: stat file, reject if > MaxFileSize (5MB)
- [ ] Binary detection: null byte scan + mime.DetectContentType
- [ ] Text files: return full content
- [ ] Binary files: return [binary file, mime-type, N bytes]
- [ ] Error handling: not found, permission denied, too large

### P4.3 — FileWrite Tool
- [ ] FileWrite struct with workDir, backupDir
- [ ] Execute: resolve path, check within workDir
- [ ] Backup: copy existing file to backupDir/<sanitized>.<timestamp> before overwrite
- [ ] Atomic write: write to .m31a_tmp_<random>, fsync, os.Rename
- [ ] Directory creation: os.MkdirAll on parent if create_dirs=true
- [ ] Binary content rejection: check for null bytes in content
- [ ] Cleanup: defer os.Remove on temp file if write fails
- [ ] Error handling: permission denied, write failure

### P4.4 — Glob Tool
- [ ] Glob struct with workDir
- [ ] Execute: doublestar.Glob for recursive ** support
- [ ] Output: sorted paths with size and modified timestamp
- [ ] Table format: path, size, modified
- [ ] Hard limit: 1,000 results with [... N more files] truncation
- [ ] rg integration: if available, use rg --files --glob (respects .gitignore)
- [ ] Fallback: doublestar when rg unavailable

### P4.5 — Grep Tool
- [ ] Grep struct with workDir, hasRg flag
- [ ] NewGrep: checks for rg in PATH at construction
- [ ] Execute with rg: rg --json --line-number --max-count, parse JSON output
- [ ] Pure-Go fallback: filepath.Walk, regexp.Compile, bufio.Scanner line-by-line
- [ ] Glob filter: match files via doublestar.Match before searching
- [ ] Binary file skipping: null byte check
- [ .gitignore support: automatic with rg, manual parse in pure-Go fallback
- [ ] Output format: file:line: match_text
- [ ] Max results cap (default 100)

### P4.6 — Dispatcher
- [ ] Dispatcher struct with tools map, permissions map, request/response channels
- [ ] Register: adds tool to map, panics on duplicate
- [ ] Execute: looks up tool by name, checks risk level
- [ ] Safe/Medium tools: execute immediately
- [ ] Dangerous/Destructive tools: check remembered permissions, then send PermissionRequest
- [ ] Permission flow: send to requestCh, wait on responseCh, store if remember=true
- [ ] ErrPermissionDenied returned if denied
- [ ] ApprovePermission: sends response on responseCh
- [ ] List/GetTool accessors

## Test Results

### All Tool Tests
```
[paste actual output of: go test -race -v ./internal/tools/...]
```

### Test Summary
- Total tests: [count]
- Passed: [count]
- Failed: [count]
- Skipped: [count]

## Build Verification
- [ ] `go mod tidy` passes
- [ ] `go build ./...` passes (output below)
- [ ] `go vet ./...` passes (output below)

### go build ./...
```
[paste actual output]
```

### go vet ./...
```
[paste actual output]
```

## Deviations from Spec
[List any deviation from ROADMAP.md Phase 4 tasks, or write "None"]

## Open Questions / Blockers for Phase 5
[List anything Phase 5 (Session State & Configuration) needs to know, or write "None"]

## Deliverable Summary
[3-5 sentences summarizing what was implemented and confirming all Phase 4 deliverables are met.]
```

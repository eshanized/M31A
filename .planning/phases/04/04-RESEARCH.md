# Phase 4: Tool System — Research

**Researched:** 2026-05-27
**Domain:** Go tool system architecture, process execution, file I/O, globbing, code search, permission gating
**Confidence:** HIGH

## Summary

Phase 4 implements the V1 Tool System for M31A — five tools (Bash, FileRead, FileWrite, Glob, Grep) plus a dispatcher and permission gate. The tools implement the `types.Tool` interface already defined in `internal/types/types.go`, and the dispatcher skeleton exists as function-field struct in `internal/tools/interface.go`. The TUI layer already has the `ToolCard`, `PermissionModal`, and `ThinkingBlock` components built in Phase 2/3 — this phase connects them.

The project already has `ripgrep` available on the system (`/usr/bin/rg`), Go 1.26.3, and all the charm dependencies. Two new external dependencies are needed: `github.com/bmatcuk/doublestar/v4` (glob `**` support) and `github.com/creack/pty` (PTY for Linux/macOS). Both are already declared in AGENTS.md as approved dependencies.

**Primary recommendation:** Implement each tool as a separate file in `internal/tools/` implementing the `types.Tool` interface. Convert the `Dispatcher` from a function-field struct to a concrete struct with a `map[string]types.Tool` registry. The permission gate bridges the dispatcher and the TUI's existing `PermissionModal` via channel/bubbletea message patterns.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- All workflow state stored as human-readable Markdown + JSON in ~/.m31a/sessions/<id>/planning/. Never binary formats. Always resumable.
- Two-phase token estimation: client-side tiktoken-go during streaming, server calibration from final SSE chunk usage field. EMA alpha=0.3 correction factor per model family.
- Each workflow phase discards prior conversation. Reads only structured state files (PROJECT.md, TASKS.md, STATE.md) plus system prompt. No conversation history carried between phases.

### the agent's Discretion
- N/A for Phase 4 — all tool implementations are specified in ROADMAP.md

### Deferred Ideas (OUT OF SCOPE)
- N/A for Phase 4
</user_constraints>

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| P4.1 | Bash tool with 30-min timeout, signal forwarding, PTY, binary detection | `exec.Cmd` + `context.WithTimeout` + `creack/pty` for PTY; channel-based output streaming; `pty.Start()` for Linux/macOS, `exec.Cmd` pipes for Windows |
| P4.2 | FileRead with path safety, binary detection, 5MB limit | `os.Open` + `filepath.Clean` + symlink resolution via `os.Readlink`/`filepath.EvalSymlinks`; mime sniff via `net/http` `DetectContentType`; size check before read |
| P4.3 | FileWrite with atomic write, backup, directory creation | Write to `.m31a_tmp_<random>` then `os.Rename`; backup to `~/.m31a/sessions/<id>/backups/`; `os.MkdirAll` on parent |
| P4.4 | Glob with `doublestar` `**` support, sorted relative paths, 1000 result limit | `doublestar.Glob()` returns matches; sort by path; limit to 1000 with truncation message |
| P4.5 | Grep with rg detection and pure-Go fallback, .gitignore support | `exec.LookPath("rg")` then shell out with `--json`; fallback: `bufio.Scanner` line-by-line regex match |
| P4.6 | Tool dispatcher + permission gate + tests | Convert `Dispatcher` to concrete struct; `map[string]types.Tool` registry; `PermissionRequest` → PermissionModal → `PermissionResponse` flow |

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Tool execution | internal/tools | — | Each tool is a self-contained implementation of `types.Tool.Execute()` |
| Permission gating | internal/tools | internal/tui (PermissionModal) | Dispatcher checks risk level, emits PermissionRequest; TUI displays modal and returns PermissionResponse |
| Tool routing (LLM → implementation) | internal/tools (Dispatcher) | — | Dispatcher.Parse() routes ToolCall by Name to registered Tool |
| Tool definition generation | internal/provider | — | ToolDefinition struct in provider/interface.go; generated from registered tools for LLM function calling |
| Output streaming (Bash) | internal/tools | — | Bash tool uses goroutine-safe channel for real-time output; caller reads from channel |

## Standard Stack

### Core
| Library | Version | Import Path | Purpose | Why Standard |
|---------|---------|-------------|---------|--------------|
| Go stdlib `os/exec` | 1.22+ | `os/exec` | Command execution | Standard; no external dep needed for basic execution |
| Go stdlib `context` | 1.22+ | `context` | Timeout and cancellation | Standard pattern; `context.WithTimeout` for bash 30-min limit |
| doublestar | v4.6.1+ | `github.com/bmatcuk/doublestar/v4` | Glob `**` matching | Listed in AGENTS.md; only mature Go doublestar library |
| creack/pty | v1.1.24 | `github.com/creack/pty` | PTY on Linux/macOS | Listed in AGENTS.md; standard Go PTY library, 2k stars |

### Supporting
| Library | Version | Import Path | Purpose | When to Use |
|---------|---------|-------------|---------|-------------|
| Go stdlib `bufio` | 1.22+ | `bufio` | Line-by-line reading | Grep fallback scanner, bash output reader |
| Go stdlib `mime/multipart` `net/http` | 1.22+ | `net/http` | Binary content detection | FileRead: `http.DetectContentType()` on first 512 bytes |
| Go stdlib `path/filepath` | 1.22+ | `path/filepath` | Path manipulation | Glob path cleaning, FileRead path safety |
| Go stdlib `crypto/rand` | 1.22+ | `crypto/rand` | Temp file name generation | FileWrite atomic temp file naming |
| Go stdlib `regexp` | 1.22+ | `regexp` | Regex matching | Grep fallback line matching |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `creack/pty` | Raw `os.StartProcess` with PTY via `syscall.ForkExec` | Building PTY from scratch is error-prone; `creack/pty` handles platform-specific PTY setup (linux, darwin, bsd, solaris) |
| `doublestar` | Manual recursive filepath.Walk | `doublestar` is simpler, handles edge cases, and already approved in AGENTS.md |
| `exec.LookPath("rg")` | Pure-Go grep always | rg is 10-100x faster for large codebases, available on system, and reduces binary size |

**Installation:**
```bash
go get github.com/bmatcuk/doublestar/v4@latest
go get github.com/creack/pty@latest
```

**Version verification:**
```bash
go list -m github.com/bmatcuk/doublestar/v4
go list -m github.com/creack/pty
```

## Package Legitimacy Audit

| Package | Registry | Age | Downloads | Source Repo | slopcheck | Disposition |
|---------|----------|-----|-----------|-------------|-----------|-------------|
| `github.com/bmatcuk/doublestar/v4` | Go (pkg.go.dev) | ~5 yrs | 488+ importers | github.com/bmatcuk/doublestar | Canonical — already listed in AGENTS.md | Approved |
| `github.com/creack/pty` | Go (pkg.go.dev) | ~10 yrs | 1,263+ importers, 2k stars | github.com/creack/pty | Canonical — already listed in AGENTS.md | Approved |

**Packages removed due to slopcheck [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none — both packages are already approved in AGENTS.md as project dependencies

## Architecture Patterns

### System Architecture Diagram

```
                    LLM Response (SSE stream)
                            │
                            ▼
                    Streaming pipeline (streaming.go)
                            │
                            ▼
                    Parse ToolCalls from message
                            │
                            ▼
                    ┌──────────────────────────────┐
                    │       Dispatcher              │
                    │  internal/tools/dispatcher.go │
                    │                              │
                    │   map[string]types.Tool       │
                    │   (Bash, FileRead, FileWrite, │
                    │    Glob, Grep)                │
                    │                              │
                    │   Execute(ToolCall)           │
                    │        │                      │
                    │   Check RiskLevel             │
                    │   if Dangerous/Destructive:   │
                    │     ─► PermissionRequest     │
                    │     ◄─ PermissionResponse    │
                    └──────────┬───────────────────┘
                               │
                    ┌──────────▼───────────┐
                    │  Tool.Execute(ctx,   │
                    │   ToolInput)          │
                    │                      │
                    │  ┌──────┐ ┌──────┐  │
                    │  │ Bash │ │Read  │  │
                    │  ├──────┤ ├──────┤  │
                    │  │Write │ │Glob  │  │
                    │  ├──────┤ ├──────┤  │
                    │  │ Grep │ │ ...  │  │
                    │  └──────┘ └──────┘  │
                    └──────────┬──────────┘
                               │
                    ┌──────────▼───────────┐
                    │   ToolResult          │
                    │   (Output, Error,     │
                    │    DurationMs,        │
                    │    Truncated)          │
                    │                      │
                    ▼                      ▼
            ToolCard (TUI)       Provider layer
          collapsible render     feeds back to LLM
```

### Recommended Project Structure

```
internal/tools/
├── interface.go          # Existing: PermissionRequest, PermissionResponse, Dispatcher type
├── dispatcher.go         # Concrete Dispatcher: Register, Execute, List methods
├── dispatcher_test.go    # Dispatcher tests
├── bash.go               # Bash tool — exec.Cmd, PTY, streaming, signal forwarding
├── bash_test.go          # Bash tests
├── fileread.go           # FileRead tool — safe file reading, binary detection
├── fileread_test.go      # FileRead tests
├── filewrite.go          # FileWrite tool — atomic write, backup, dir creation
├── filewrite_test.go     # FileWrite tests
├── glob.go               # Glob tool — doublestar wrapper, sorted results, 1000 limit
├── glob_test.go          # Glob tests
├── grep.go               # Grep tool — rg detection + pure-Go fallback
├── grep_test.go          # Grep tests
├── pathutil.go           # Shared path utilities (symlink resolution, cwd validation)
└── pathutil_test.go      # Path utility tests
```

### Pattern 1: Tool Interface Implementation

**What:** Each tool is a struct implementing `types.Tool` interface (Name, Description, RiskLevel, Execute). Tools are stateless — all state is in the input parameters.

**Source:** [VERIFIED: internal/types/types.go lines 109-114]

```go
// bash.go
type BashTool struct{}

func (BashTool) Name() string            { return "Bash" }
func (BashTool) Description() string     { return "Execute shell commands" }
func (BashTool) RiskLevel() RiskLevel    { return types.RiskDangerous }

func (t BashTool) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
    command := input.Params["command"].(string)
    // ... execution logic
}
```

### Pattern 2: Concrete Dispatcher (replaces function-field struct)

**What:** Convert the existing `Dispatcher` type (which uses function fields) to a concrete struct with a map of registered tools. This avoids the "implement interface via function fields" anti-pattern.

**Source:** [VERIFIED: internal/tools/interface.go lines 21-25]

```go
// dispatcher.go
type Dispatcher struct {
    tools      map[string]types.Tool
    permission chan PermissionRequest // requests to TUI
    response   chan PermissionResponse // responses from TUI
}

func NewDispatcher() *Dispatcher {
    return &Dispatcher{
        tools:      make(map[string]types.Tool),
        permission: make(chan PermissionRequest),
        response:   make(chan PermissionResponse),
    }
}

func (d *Dispatcher) Register(name string, tool types.Tool) {
    d.tools[name] = tool
}

func (d *Dispatcher) Execute(ctx context.Context, call types.ToolCall) (types.ToolResult, error) {
    tool, ok := d.tools[call.Name]
    if !ok {
        return types.ToolResult{}, fmt.Errorf("unknown tool: %s", call.Name)
    }
    
    // Check permission for dangerous/destructive tools
    if tool.RiskLevel() >= types.RiskDangerous {
        allowed, err := d.requestPermission(ctx, call.Name, tool.RiskLevel())
        if err != nil || !allowed {
            return types.ToolResult{}, m31errors.ErrPermissionDenied
        }
    }
    
    input := parseInput(call)
    result, err := tool.Execute(ctx, input)
    // ... wrap result
}
```

**Key decision:** The existing `Dispatcher` type in `interface.go` uses function fields (`Register func(...)`, `Execute func(...)`, `List func() []string`). This pattern was chosen in Phase 0 as a placeholder. Phase 4 should **replace** this with a concrete struct implementation. The function-field struct in interface.go should be updated to match the concrete struct or maintained as backward-compatible. **Recommendation**: Update `interface.go` to use a concrete `Dispatcher` struct type directly.

### Pattern 3: Permission Gate Flow

**What:** Tools with RiskDangerous or RiskDestructive levels require user approval before execution. The dispatcher emits a PermissionRequest to the TUI, which displays the PermissionModal and returns a PermissionResponse.

**Source:** [VERIFIED: internal/tui/components/permission.go lines 13-19]

```go
// dispatcher.go — permission check
func (d *Dispatcher) requestPermission(ctx context.Context, toolName string, risk types.RiskLevel) (bool, error) {
    req := PermissionRequest{
        ToolName:    toolName,
        RiskLevel:   risk,
        TimeoutSecs: 300, // 5 min default
    }
    
    select {
    case d.permission <- req:
        // Sent to TUI — wait for response
    case <-ctx.Done():
        return false, ctx.Err()
    }
    
    select {
    case resp := <-d.response:
        return resp.Allowed, nil
    case <-ctx.Done():
        return false, ctx.Err()
    }
}
```

**Critical:** The TUI integration (`AppState.Update()`) must listen on the dispatcher's permission channel and return `PermissionRequestMsg` as a `tea.Cmd`. When the modal responds (Y/N/A/E keys), it sends `PermissionResponseMsg` back, which the dispatcher must capture. This requires integration wiring between `internal/tui/` and `internal/tools/` (see Integration Points section).

### Pattern 4: Bash Tool — PTY with Fallback

**What:** On Linux/macOS, use `creack/pty` to allocate a PTY for the command (enables psuedo-terminal features like color output, SIGINT propagation). On Windows, fall back to plain pipes. Use Go build tags for platform selection.

**Source:** [CITED: creack/pty README](https://github.com/creack/pty) — `pty.Start()` and `pty.Open()` usage

```go
// bash_pty.go — build tag: !windows
//go:build !windows

package tools

import (
    "os/exec"
    "github.com/creack/pty"
)

func startWithPTY(cmd *exec.Cmd) (*os.File, error) {
    return pty.Start(cmd)
}
```

```go
// bash_windows.go — build tag: windows
//go:build windows

package tools

import (
    "io"
    "os/exec"
)

func startWithPTY(cmd *exec.Cmd) (*os.File, error) {
    // Windows: use pipes instead of PTY
    stdout, err := cmd.StdoutPipe()
    if err != nil {
        return nil, err
    }
    // ... wire up pipes
    return nil, nil // Return nil; caller handles pipe mode differently
}
```

**Key design choice:** PTY is allocated only when the command needs a terminal (e.g., interactive programs). For non-interactive commands, plain pipes are sufficient and more predictable. The tool should detect this or provide a `pty` input parameter (default: `auto`).

**Signal forwarding:** When the Bash tool is cancelled (ctx.Done), send SIGINT to the child process group:
```go
if cmd.Process != nil {
    syscall.Kill(-cmd.Process.Pid, syscall.SIGINT) // negative PID = process group
}
```

**Output streaming:** Read PTY output in a goroutine, send lines to a `chan string`. The `Execute` method collects output up to `BashOutputLimit` characters, then truncates.

### Pattern 5: FileRead — Path Safety and Binary Detection

**What:** Safe file reading with multiple protection layers: workspace path restriction, symlink resolution, binary detection, size limit.

```go
func (FileReadTool) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
    path := input.Params["path"].(string)
    
    // 1. Resolve to absolute path
    absPath, err := filepath.Abs(path)
    if err != nil { return errorResult(err) }
    
    // 2. Resolve symlinks
    realPath, err := filepath.EvalSymlinks(absPath)
    if err != nil { return errorResult(err) }
    
    // 3. Verify within allowed root (user's cwd)
    if !isWithinCwd(realPath) {
        return errorResult(ErrPathOutsideAllowed)
    }
    
    // 4. Check file size before reading
    fi, err := os.Stat(realPath)
    if err != nil { return errorResult(err) }
    if fi.Size() > MaxFileSize {
        return types.ToolResult{Error: m31errors.ErrFileTooLarge.Error()}
    }
    
    // 5. Read first 512 bytes for binary detection
    f, _ := os.Open(realPath); defer f.Close()
    header := make([]byte, 512)
    f.Read(header); f.Seek(0, 0)
    
    mimeType := http.DetectContentType(header)
    if isBinaryMIME(mimeType) {
        return types.ToolResult{
            Output: fmt.Sprintf("[binary file, %s, %d bytes]", mimeType, fi.Size()),
        }
    }
    
    // 6. Read everything
    data, _ := io.ReadAll(f)
    return types.ToolResult{Output: string(data)}
}
```

**Binary MIME types:** `application/octet-stream`, `application/x-executable*`, `application/x-sharedlib*`, `application/pdf`, `image/*`, `audio/*`, `video/*`, `application/zip`, `application/gzip`, etc.

### Pattern 6: FileWrite — Atomic Write with Backup

**What:** File write that never corrupts existing content. Write to a temp file, then atomically rename. Copy existing file to session backup directory before overwriting.

```go
func (FileWriteTool) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
    path := input.Params["path"].(string)
    content := input.Params["content"].(string)
    
    // 1. Path safety (same as FileRead)
    safePath, err := resolveSafePath(path)
    if err != nil { return errorResult(err) }
    
    // 2. MkdirAll on parent directory
    os.MkdirAll(filepath.Dir(safePath), 0755)
    
    // 3. Backup existing file (if any)
    if _, err := os.Stat(safePath); err == nil {
        backupDir := filepath.Join(sessionBackupDir(), fmt.Sprintf("%s.%d",
            filepath.Base(safePath), time.Now().UnixNano()))
        copyFile(safePath, backupDir)
    }
    
    // 4. Atomic write: temp file → rename
    tmpPath := filepath.Join(filepath.Dir(safePath),
        fmt.Sprintf(".m31a_tmp_%x", randomBytes(8)))
    
    // Clean up temp file if rename fails
    defer os.Remove(tmpPath)
    
    if err := os.WriteFile(tmpPath, []byte(content), 0644); err != nil {
        return errorResult(err)
    }
    
    if err := os.Rename(tmpPath, safePath); err != nil {
        return errorResult(err)
    }
    
    return types.ToolResult{Output: fmt.Sprintf("Written %d bytes to %s", len(content), path)}
}
```

**Atomic rename guarantee:** `os.Rename` is atomic on the same filesystem (POSIX). The temp file must be on the same filesystem as the target — which it is because we create it in the same directory.

### Pattern 7: Glob Tool — doublestar Pattern Matching

**What:** Use `doublestar.Glob` for recursive pattern matching with `**` support. Sort results by path. Limit to 1000 results.

```go
import "github.com/bmatcuk/doublestar/v4"

func (GlobTool) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
    pattern := input.Params["pattern"].(string)
    
    matches, err := doublestar.Glob(os.DirFS("/"), pattern)
    if err != nil {
        return errorResult(err)
    }
    
    // Sort by path
    sort.Strings(matches)
    
    // Limit to 1000
    truncated := false
    if len(matches) > 1000 {
        matches = matches[:1000]
        truncated = true
    }
    
    // Format: relative path, size, last modified
    var b strings.Builder
    for _, m := range matches {
        fi, err := os.Stat(m)
        if err == nil {
            fmt.Fprintf(&b, "%s\t%d\t%s\n", m, fi.Size(), fi.ModTime().Format(time.RFC3339))
        }
    }
    
    if truncated {
        b.WriteString("[... more files not shown]\n")
    }
    
    return types.ToolResult{Output: b.String()}
}
```

### Pattern 8: Grep Tool — rg with Pure-Go Fallback

**What:** Detect ripgrep via `exec.LookPath`. If available, run `rg --json` for structured results. Fallback: pure-Go `bufio.Scanner` line-by-line regex search.

```go
func (GrepTool) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
    pattern := input.Params["pattern"].(string)
    path := input.Params["path"].(string)
    
    if rgPath, err := exec.LookPath("rg"); err == nil {
        return grepWithRG(ctx, rgPath, pattern, path)
    }
    return grepWithPureGo(ctx, pattern, path)
}

func grepWithRG(ctx context.Context, rgPath, pattern, path string) (types.ToolResult, error) {
    cmd := exec.CommandContext(ctx, rgPath, "--json", pattern, path)
    output, err := cmd.Output()
    // Parse NDJSON lines: each line is {"type":"match","data":{"path":{"text":"..."},"lines":{"text":"..."}}}
    // Extract file path, line number, line content
    // Format as table
}

func grepWithPureGo(ctx context.Context, pattern, path string) (types.ToolResult, error) {
    re, err := regexp.Compile(pattern)
    if err != nil { return errorResult(err) }
    
    file, err := os.Open(path)
    if err != nil { return errorResult(err) }
    defer file.Close()
    
    scanner := bufio.NewScanner(file)
    lineNum := 0
    var b strings.Builder
    for scanner.Scan() {
        lineNum++
        line := scanner.Text()
        if re.MatchString(line) {
            fmt.Fprintf(&b, "%s:%d: %s\n", path, lineNum, line)
        }
    }
    return types.ToolResult{Output: b.String()}, scanner.Err()
}
```

### Anti-Patterns to Avoid

- **Mutating AppState from goroutine:** Goroutines in Bash tool (PTY reader) must send data via channels, never directly update state. Use `tea.Cmd`/`tea.Msg` pattern.
- **Blocking in Execute:** Tool execution blocks the calling goroutine. The caller (provider streaming loop) must run `Execute` in a separate goroutine and receive results via channel.
- **Custom path resolution instead of `filepath.EvalSymlinks`:** Manual symlink walking is error-prone. Use stdlib.
- **Appending to ToolResult after truncation:** Once `BashOutputLimit` is reached, stop collecting. Don't append metadata after truncation flag.
- **Sharing os.File between goroutines without sync:** PTY file descriptors from `creack/pty` require careful goroutine coordination for read/write.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Glob `**` matching | Manual `filepath.Walk` recursion | `doublestar/v4` | Handles edge cases (symlinks, permissions, pattern validation, cross-platform paths) |
| PTY allocation | Raw `syscall.ForkExec` | `creack/pty` | Cross-platform PTY (linux/darwin/bsd), correct termios setup, window size management |
| Binary/MIME detection | Custom byte sniffing | `net/http.DetectContentType()` | Follows MIME sniffing spec, handles all common binary formats |
| Regex grep from scratch | Custom line-by-line grep | `regexp.Regexp.MatchString` | stdlib, tested, fast for moderate-sized files |
| Atomic file write | Direct `os.WriteFile` | Temp file + `os.Rename` | Prevents partial writes from crashes; Rename is atomic on same filesystem |

**Key insight:** The tool system is where M31A interacts with the user's system. Every hand-rolled file I/O or process execution pattern is a potential security hole or platform portability bug. Use stdlib and proven libraries.

## Common Pitfalls

### Pitfall 1: PTY Not Available / CGO Dependency
**What goes wrong:** `creack/pty` may introduce implicit CGO dependencies or fail on platforms without PTY support.
**Why it happens:** `creack/pty` uses syscall-level PTY allocation which is platform-specific. Some build configurations link against C libraries.
**How to avoid:** Use build tags (`!windows` for PTY, `windows` for pipes). Ensure `CGO_ENABLED=0` builds still work by testing. The `creack/pty` library is pure Go for Linux/macOS — no CGO needed. Use `pty.ErrUnsupported` for platforms without PTY support and fall back to pipes.
**Warning signs:** Build failure on `CGO_ENABLED=0`, `unsupported platform` error at runtime.

### Pitfall 2: Symlink Path Traversal
**What goes wrong:** A symlink inside the allowed directory points to a file outside it (e.g., `/home/user/project/link -> /etc/passwd`).
**Why it happens:** Resolving only the directory of the symlink, not the symlink's target.
**How to avoid:** ALWAYS call `filepath.EvalSymlinks()` on the full path and verify the resolved path is within the allowed root. Never trust user-provided paths.
**Warning signs:** FileRead returns content of files outside the project directory.

### Pitfall 3: Bash Context Cancellation Not Killing Child Process
**What goes wrong:** When the 30-minute timeout fires or user cancels, the Bash child process continues running as an orphan.
**Why it happens:** `exec.Cmd` cancellation via `context.WithTimeout` kills the process but not its children. Shell commands often spawn child processes.
**How to avoid:** Start the command in its own process group (`cmd.SysProcAttr.Setpgid = true` on Linux). On cancel, send SIGINT/SIGKILL to the negative PID (whole process group). On PTY, closing the PTY sends SIGHUP to the child.
**Warning signs:** Orphan processes accumulating on the system.

### Pitfall 4: FileWrite Concurrent Access
**What goes wrong:** Two tool executions try to write the same file simultaneously; one overwrites the other's temp file before rename.
**Why it happens:** `crypto/rand` temp file names reduce collision probability but don't eliminate it.
**How to avoid:** `crypto/rand` gives 2^64 namespace per temp file — collision probability is negligible. Use `os.O_EXCL` flag when creating the temp file to fail if it already exists.
**Warning signs:** "file exists" errors during temp file creation (extremely rare).

### Pitfall 5: Bash Output Exhausting Memory
**What goes wrong:** A command produces gigabytes of output, exhausting memory.
**Why it happens:** The tool reads all output before returning; no streaming to disk.
**How to avoid:** Cap collection at `BashOutputLimit` (50,000 chars). Set `cmd.Stdout` to a limited pipe reader. Use `io.LimitReader` on the PTY output. Once limit is hit, signal truncation in `ToolResult.Truncated`.
**Warning signs:** Memory grows unbounded during long-running bash commands.

### Pitfall 6: rg in Directory with No .gitignore
**What goes wrong:** The pure-Go grep fallback needs to manually parse `.gitignore` files, which is complex (nested .gitignore, negation patterns, `**` patterns).
**Why it happens:** rg handles `.gitignore` natively; the fallback doesn't.
**How to avoid:** The pure-Go fallback is a best-effort simplification. It can either: (a) skip `.gitignore` entirely (document as limitation), (b) do a basic `.gitignore` pattern parse, or (c) require `rg` for full `.gitignore` support. Recommendation: skip `.gitignore` in fallback and document that rg provides full support.
**Warning signs:** Grep matches files that should be gitignored.

## Code Examples

### Example 1: Bash Tool Complete Structure

```go
// internal/tools/bash.go
package tools

import (
	"bufio"
	"context"
	"fmt"
	"io"
	"os/exec"
	"strings"
	"time"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/types"
)

type BashTool struct{}

func (BashTool) Name() string           { return "Bash" }
func (BashTool) Description() string    { return "Execute shell commands in a subprocess" }
func (BashTool) RiskLevel() types.RiskLevel { return types.RiskDangerous }

type streamWriter struct {
	lines  chan string
	done   chan struct{}
	closed bool
}

func (sw *streamWriter) Write(p []byte) (int, error) {
	if sw.closed {
		return len(p), nil
	}
	// Send each line to the stream channel
	scanner := bufio.NewScanner(strings.NewReader(string(p)))
	for scanner.Scan() {
		select {
		case sw.lines <- scanner.Text():
		case <-sw.done:
			return len(p), nil
		}
	}
	return len(p), nil
}

func (t BashTool) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
	command, ok := input.Params["command"].(string)
	if !ok {
		return types.ToolResult{}, fmt.Errorf("missing 'command' parameter")
	}

	start := time.Now()

	cmd := exec.CommandContext(ctx, "bash", "-c", command)
	
	// Create process group for signal forwarding
	cmd.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}

	// Set up output streaming
	sw := &streamWriter{
		lines: make(chan string, 100),
		done:  make(chan struct{}),
	}
	cmd.Stdout = sw
	cmd.Stderr = sw

	// Try PTY on supported platforms
	usePTY := false
	if ptyFile, err := startWithPTY(cmd); err == nil {
		usePTY = true
		defer ptyFile.Close()
	} else {
		// Fall back to pipes
		if err := cmd.Start(); err != nil {
			return errorResult(err, start), nil
		}
	}

	// Collect output (capped at BashOutputLimit)
	var output strings.Builder
	outputCount := 0
	truncated := false

outputLoop:
	for {
		select {
		case line, ok := <-sw.lines:
			if !ok {
				break outputLoop
			}
			if outputCount >= types.BashOutputLimit {
				truncated = true
				continue
			}
			lineLen := len(line) + 1 // +1 for newline
			if outputCount+lineLen > types.BashOutputLimit {
				output.WriteString(line[:types.BashOutputLimit-outputCount])
				output.WriteString("\n[... output truncated]\n")
				truncated = true
				outputCount = types.BashOutputLimit
			} else {
				output.WriteString(line)
				output.WriteString("\n")
				outputCount += lineLen
			}
		case <-ctx.Done():
			// Kill process group
			if cmd.Process != nil {
				syscall.Kill(-cmd.Process.Pid, syscall.SIGINT)
			}
			return types.ToolResult{
				Output:     output.String(),
				Error:      ctx.Err().Error(),
				DurationMs: time.Since(start).Milliseconds(),
				Truncated:  truncated,
			}, nil
		}
	}

	err := cmd.Wait()
	if err != nil {
		return types.ToolResult{
			Output:     output.String(),
			Error:      fmt.Sprintf("command failed: %v", err),
			DurationMs: time.Since(start).Milliseconds(),
			Truncated:  truncated,
		}, nil
	}

	// Check for binary content
	if isBinaryOutput(output.String()) {
		return types.ToolResult{
			Output:     fmt.Sprintf("[binary output, %d bytes]", output.Len()),
			DurationMs: time.Since(start).Milliseconds(),
			Truncated:  truncated,
		}, nil
	}

	return types.ToolResult{
		Output:     output.String(),
		DurationMs: time.Since(start).Milliseconds(),
		Truncated:  truncated,
	}, nil
}

func isBinaryOutput(s string) bool {
	// Check first 1K bytes for null chars
	checkLen := len(s)
	if checkLen > 1024 {
		checkLen = 1024
	}
	for i := 0; i < checkLen; i++ {
		if s[i] == 0 {
			return true
		}
	}
	return false
}

func errorResult(err error, start time.Time) types.ToolResult {
	return types.ToolResult{
		Error:      err.Error(),
		DurationMs: time.Since(start).Milliseconds(),
	}
}
```

### Example 2: Dispatcher — Permission Gate

```go
// internal/tools/dispatcher.go
package tools

import (
	"context"
	"fmt"
	"time"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/types"
)

// PermissionGate is the interface the TUI implements to handle permission dialogs.
type PermissionGate interface {
	RequestPermission(ctx context.Context, req PermissionRequest) (PermissionResponse, error)
}

type Dispatcher struct {
	tools   map[string]types.Tool
	gate    PermissionGate
}

func NewDispatcher(gate PermissionGate) *Dispatcher {
	return &Dispatcher{
		tools: make(map[string]types.Tool),
		gate:  gate,
	}
}

func (d *Dispatcher) Register(name string, tool types.Tool) {
	d.tools[name] = tool
}

func (d *Dispatcher) Execute(ctx context.Context, call types.ToolCall) (types.ToolResult, error) {
	tool, ok := d.tools[call.Name]
	if !ok {
		return types.ToolResult{}, fmt.Errorf("%w: unknown tool %q", m31errors.ErrToolExecution, call.Name)
	}

	// Permission gate for non-safe tools
	if tool.RiskLevel() >= types.RiskDangerous && d.gate != nil {
		input, _ := call.Input.MarshalJSON()
		req := PermissionRequest{
			ToolName:    call.Name,
			Command:     string(input),
			RiskLevel:   tool.RiskLevel(),
			TimeoutSecs: 300,
		}
		resp, err := d.gate.RequestPermission(ctx, req)
		if err != nil {
			return types.ToolResult{}, err
		}
		if !resp.Allowed {
			return types.ToolResult{}, m31errors.ErrPermissionDenied
		}
	}

	// Parse ToolInput from ToolCall
	input := types.ToolInput{
		Name: call.Name,
	}
	if err := call.Input.UnmarshalJSON(&input.Params); err != nil {
		// Try direct parsing for simple string inputs
		var strVal string
		if err2 := call.Input.UnmarshalJSON(&strVal); err2 == nil {
			input.Params = map[string]any{"path": strVal}
		} else {
			input.Params = make(map[string]any)
		}
	}

	return tool.Execute(ctx, input)
}

func (d *Dispatcher) List() []string {
	names := make([]string, 0, len(d.tools))
	for name := range d.tools {
		names = append(names, name)
	}
	sort.Strings(names)
	return names
}
```

### Example 3: Testing a Tool

```go
// internal/tools/fileread_test.go
package tools

import (
	"context"
	"os"
	"path/filepath"
	"testing"

	"github.com/eshanized/M31A/internal/types"
)

func TestFileRead_Success(t *testing.T) {
	// Create temp file
	dir := t.TempDir()
	content := "hello world"
	path := filepath.Join(dir, "test.txt")
	if err := os.WriteFile(path, []byte(content), 0644); err != nil {
		t.Fatal(err)
	}

	tool := FileReadTool{}
	input := types.ToolInput{
		Name: "FileRead",
		Params: map[string]any{
			"path": path,
		},
	}

	result, err := tool.Execute(context.Background(), input)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Output != content {
		t.Errorf("expected %q, got %q", content, result.Output)
	}
}

func TestFileRead_BinaryDetection(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "binary.bin")
	if err := os.WriteFile(path, []byte{0x00, 0x01, 0x02, 0x03}, 0644); err != nil {
		t.Fatal(err)
	}

	tool := FileReadTool{}
	input := types.ToolInput{Name: "FileRead", Params: map[string]any{"path": path}}

	result, err := tool.Execute(context.Background(), input)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Output == "hello world" {
		t.Error("expected binary content message, got raw content")
	}
}

func TestFileRead_TooLarge(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "large.txt")
	data := make([]byte, types.MaxFileSize+1)
	if err := os.WriteFile(path, data, 0644); err != nil {
		t.Fatal(err)
	}

	tool := FileReadTool{}
	input := types.ToolInput{Name: "FileRead", Params: map[string]any{"path": path}}

	_, err := tool.Execute(context.Background(), input)
	if err == nil || err.Error() != m31errors.ErrFileTooLarge.Error() {
		t.Errorf("expected ErrFileTooLarge, got %v", err)
	}
}

func TestFileRead_PathTraversal(t *testing.T) {
	tool := FileReadTool{}
	input := types.ToolInput{
		Name: "FileRead",
		Params: map[string]any{
			"path": "/etc/passwd", // outside cwd
		},
	}

	_, err := tool.Execute(context.Background(), input)
	if err == nil {
		t.Error("expected error for path outside allowed directory")
	}
}

func TestFileRead_SymlinkInsideDir(t *testing.T) {
	dir := t.TempDir()
	realContent := "sensitive data"
	realFile := filepath.Join(dir, "real.txt")
	os.WriteFile(realFile, []byte(realContent), 0644)

	// Create symlink pointing outside dir
	linkPath := filepath.Join(dir, "link.txt")
	os.Symlink("/etc/passwd", linkPath) // or temp dir outside

	// ... test that symlink is either followed but validated, or rejected
}
```

### Example 4: Permission Gate Integration with TUI

```go
// internal/tui/app.go — excerpt showing integration
// This shows how the TUI AppState bridges the permission gate.

// The AppState creates an implementation of tools.PermissionGate that
// communicates via Bubble Tea messages.

type permissionGate struct {
	app *AppState
}

func (g *permissionGate) RequestPermission(ctx context.Context, req tools.PermissionRequest) (tools.PermissionResponse, error) {
	// Send permission request to TUI
	result := make(chan tools.PermissionResponse, 1)
	
	g.app.sendPermissionRequest(req, result)
	
	select {
	case resp := <-result:
		return resp, nil
	case <-ctx.Done():
		return tools.PermissionResponse{Allowed: false}, ctx.Err()
	}
}

// In AppState.Update():
case permissionResponseMsg:
	m.permissionResult <- msg.Response
	return m, nil
```

## Integration Points

### Provider Layer (internal/provider/)

The provider layer's `ChatRequest` struct already has a `Tools []ToolDefinition` field. The dispatcher must provide ToolDefinitions from registered tools. When `ChatCompletionStream` returns a message with `ToolCalls`, the streaming pipeline (streaming.go) must detect tool calls and invoke the dispatcher.

**Flow:**
1. Provider sends ChatRequest with ToolDefinitions
2. LLM returns message with ToolCalls → streaming.go emits StreamDoneMsg
3. StreamDoneMsg handler detects ToolCalls → calls dispatcher.Execute()
4. ToolResult is returned → appended to conversation as ToolResult role
5. Re-sends to LLM with tool results for continued conversation

### TUI Layer (internal/tui/)

The TUI already has:
- `components.ToolCard` — renders tool results as collapsible cards
- `components.PermissionModal` — centered permission dialog with timeout
- `components.ThinkingBlock` — collapsible thinking blocks

The dispatcher's permission gate connects to the TUI's ScreenPermission screen. When a permission request arrives:
1. AppState switches to ScreenPermission
2. PermissionModal renders centered overlay
3. User presses Y/A/N → PermissionResponse sent back
4. AppState switches back to ScreenREPL

### Error Constants

Already defined in `internal/errors/errors.go`:
- `ErrPermissionDenied` — tool execution blocked
- `ErrToolExecution` — tool implementation error
- `ErrFileTooLarge` — FileRead exceeds 5MB
- `ErrNoBinaryContent` — binary content detected

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Go standard testing (`testing.T`) |
| Config file | none — standard `_test.go` files |
| Quick run command | `go test ./internal/tools/... -short` |
| Full suite command | `go test -race -cover ./internal/tools/...` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| P4.1 | Bash executes command, returns output | unit | `go test ./internal/tools/... -run TestBash_Execute` | ❌ Wave 0 |
| P4.1 | Bash respects context timeout | unit | `go test ./internal/tools/... -run TestBash_Timeout` | ❌ Wave 0 |
| P4.1 | Bash detects binary output | unit | `go test ./internal/tools/... -run TestBash_Binary` | ❌ Wave 0 |
| P4.1 | Bash truncates at BashOutputLimit | unit | `go test ./internal/tools/... -run TestBash_Truncate` | ❌ Wave 0 |
| P4.2 | FileRead reads text files | unit | `go test ./internal/tools/... -run TestFileRead_Success` | ❌ Wave 0 |
| P4.2 | FileRead detects binary content | unit | `go test ./internal/tools/... -run TestFileRead_Binary` | ❌ Wave 0 |
| P4.2 | FileRead rejects large files | unit | `go test ./internal/tools/... -run TestFileRead_TooLarge` | ❌ Wave 0 |
| P4.2 | FileRead prevents path traversal | unit | `go test ./internal/tools/... -run TestFileRead_PathTraversal` | ❌ Wave 0 |
| P4.2 | FileRead resolves symlinks safely | unit | `go test ./internal/tools/... -run TestFileRead_Symlink` | ❌ Wave 0 |
| P4.3 | FileWrite writes files atomically | unit | `go test ./internal/tools/... -run TestFileWrite_Atomic` | ❌ Wave 0 |
| P4.3 | FileWrite creates parent directories | unit | `go test ./internal/tools/... -run TestFileWrite_Mkdir` | ❌ Wave 0 |
| P4.3 | FileWrite creates backups | unit | `go test ./internal/tools/... -run TestFileWrite_Backup` | ❌ Wave 0 |
| P4.4 | Glob matches `**` patterns | unit | `go test ./internal/tools/... -run TestGlob_Doublestar` | ❌ Wave 0 |
| P4.4 | Glob limits to 1000 results | unit | `go test ./internal/tools/... -run TestGlob_Limit` | ❌ Wave 0 |
| P4.5 | Grep uses rg when available | unit | `go test ./internal/tools/... -run TestGrep_RG` | ❌ Wave 0 |
| P4.5 | Grep fallback works without rg | unit | `go test ./internal/tools/... -run TestGrep_Fallback` | ❌ Wave 0 |
| P4.6 | Dispatcher routes ToolCall to correct tool | unit | `go test ./internal/tools/... -run TestDispatcher_Route` | ❌ Wave 0 |
| P4.6 | Dispatcher rejects unknown tool | unit | `go test ./internal/tools/... -run TestDispatcher_Unknown` | ❌ Wave 0 |
| P4.6 | Permission gate blocks dangerous tools | unit | `go test ./internal/tools/... -run TestDispatcher_Permission` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `go test ./internal/tools/... -short`
- **Per wave merge:** `go test -race -cover ./internal/tools/...`
- **Phase gate:** Full suite green before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] `internal/tools/bash_test.go` — covers execution, timeout, signal, truncation
- [ ] `internal/tools/fileread_test.go` — covers success, binary, size limit, path traversal
- [ ] `internal/tools/filewrite_test.go` — covers atomic write, backup, mkdir, path safety
- [ ] `internal/tools/glob_test.go` — covers doublestar matching, sorting, limit
- [ ] `internal/tools/grep_test.go` — covers rg mode, pure-Go fallback
- [ ] `internal/tools/dispatcher_test.go` — covers routing, unknown tool, permission gating
- [ ] `internal/tools/pathutil_test.go` — covers symlink resolution, cwd validation

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V5 Input Validation | Yes | Path traversal prevention via `filepath.EvalSymlinks` + cwd restriction |
| V6 Cryptography | No | No crypto operations in tool system |
| V8 Data Protection | Yes | 5MB file size limit prevents memory exhaustion; binary detection prevents terminal escape sequences |
| V12 Files and Resources | Yes | Atomic writes prevent partial file corruption; backup before overwrite |
| V15 File Execution | Yes | Bash tool runs commands in subprocess; 30-minute timeout; process group isolation |

### Known Threat Patterns for Tools

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Path traversal symlink attack | Tampering | `filepath.EvalSymlinks()` on entire path; verify resolved path is within allowed root |
| Command injection via file paths | Tampering | File paths used as arguments only; never shell-interpolated; `exec.Command` (no shell) |
| Bash child process escape | Elevation of Privilege | Process group isolation via `Setpgid: true`; kill process group on timeout |
| Binary content in terminal | Information Disclosure | MIME sniff on first 512 bytes; null-byte detection; return `[binary file, ...]` instead |
| File overwrite race condition | Tampering | Atomic write: temp file with `O_EXCL` + `os.Rename` |
| File descriptor exhaustion | Denial of Service | All tools close files promptly via `defer`; Bash has absolute 30-min timeout |
| RIP grep with crafted patterns | Denial of Service | Pure-Go fallback uses `regexp.Compile` which can panic on ReDoS; set match timeout or use `Regexp.MatchString` with `MatchTimeout` context |

## Sources

### Primary (HIGH confidence)
- [VERIFIED: internal/types/types.go] — Tool interface, ToolCall, ToolResult, ToolInput, RiskLevel
- [VERIFIED: internal/types/constants.go] — BashTimeout, BashOutputLimit, MaxFileSize, MaxToolOutputChars
- [VERIFIED: internal/tools/interface.go] — PermissionRequest, PermissionResponse, Dispatcher struct
- [VERIFIED: internal/errors/errors.go] — ErrPermissionDenied, ErrToolExecution, ErrFileTooLarge, ErrNoBinaryContent
- [VERIFIED: internal/tui/components/permission.go] — PermissionModal, NewPermissionModal, Render
- [VERIFIED: internal/tui/components/toolcard.go] — ToolCard, ToolState, NewToolCard
- [VERIFIED: internal/provider/interface.go] — ToolDefinition struct, ChatRequest.Tools field
- [VERIFIED: AGENTS.md] — Approved deps (doublestar, creack/pty), architecture rules (no CGO, V1 only)
- [VERIFIED: docs/ARCHITECTURE.md] — Threading model (single-threaded Bubble Tea), package dependency rules
- [CITED: github.com/bmatcuk/doublestar/v4 README] — doublestar Glob, Match, PathMatch API
- [CITED: github.com/creack/pty README] — pty.Start, pty.Open for PTY allocation

### Secondary (MEDIUM confidence)
- [CITED: man7.org/pty(7)] — PTY architecture (master/slave), process group signaling
- [CITED: pkg.go.dev/github.com/creack/pty] — API documentation, build constraints, ErrUnsupported

### Tertiary (LOW confidence)
- None — all findings verified against project codebase or official library documentation

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `creack/pty` works with CGO_ENABLED=0 on Linux | Standard Stack | Low — the library is pure Go; syscall-level only |
| A2 | `rg --json` output format is stable | Grep Tool | Low — ripgrep 15.1.0 is installed; format is well-documented and stable |
| A3 | The existing `Dispatcher` function-field struct should be replaced | Architecture Patterns | Low — the function-field pattern was a Phase 0 placeholder; concrete struct is more maintainable |

## Open Questions

1. **How does the Dispatcher get the session backup directory?**
   - What we know: Backups go to `~/.m31a/sessions/<id>/backups/`
   - What's unclear: Does the dispatcher receive the session ID from the caller, or discover it from context?
   - Recommendation: Pass session ID as part of tool context or as a field in a tool execution environment struct that wraps the context.

2. **How does the streaming pipeline invoke tools?**
   - What we know: `streaming.go` `StartStreamCmd` processes SSE chunks and emits `StreamDoneMsg`
   - What's unclear: Whether tool execution blocks the stream or runs asynchronously
   - Recommendation: V1 should execute tools synchronously (blocking the LLM return). When `StreamDoneMsg` has `ToolCalls`, the handler dispatches each tool, collects results, and re-invokes the LLM. This is sequential (per V1 constraints).

3. **How should the permission gate be wired into the TUI?**
   - What we know: PermissionModal exists; AppState has ScreenPermission
   - What's unclear: Whether the dispatcher polls a channel or the TUI pushes responses
   - Recommendation: Use a callback-based `PermissionGate` interface. The TUI implements it by sending a `tea.Cmd` that blocks a goroutine until the user responds. This bridges the synchronous `requestPermission` call with the async Bubble Tea event loop.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Go compiler | All | ✓ | 1.26.3 | — |
| `doublestar/v4` | Glob tool | Will install | latest | Pure-Go `filepath.Walk` |
| `creack/pty` | Bash PTY | Will install | v1.1.24 | Plain pipes (always available) |
| `ripgrep` (`rg`) | Grep tool | ✓ | 15.1.0 | Pure-Go regex fallback (built-in) |
| `bash` | Bash tool | ✓ | system default | `sh` fallback |

**Missing dependencies with no fallback:** none
**Missing dependencies with fallback:** none

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — All deps already listed in AGENTS.md; stdlib for everything else
- Architecture: HIGH — Patterns (Dispatcher, Permission Gate, Tool interface) are straightforward Go
- Pitfalls: HIGH — All are well-known Go tool execution issues with documented mitigations

**Research date:** 2026-05-27
**Valid until:** 2026-07-01 (stdlib patterns are stable; doublestar and creack/pty are mature)

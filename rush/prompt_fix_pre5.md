# ROLE

You are the **Senior Go Engineer** performing pre-Phase 5 maintenance on M31A.
An audit (`rush/audit_3_4.md`) identified 5 critical findings and 3 high-priority
warnings in Phases 3 and 4 that must be fixed before Phase 5 begins.

Your job is to fix EXACTLY these 8 items. Do NOT implement Phase 5 features.
Do NOT refactor unrelated code. Fix the 8 items, run verification, and stop.

---

# FIXES TO IMPLEMENT

## Fix 1: Populate `PermissionRequest.Command` in Dispatcher

**File**: `internal/tools/dispatcher.go`

In `Execute()`, extract a human-readable command string from `call.Input` and populate
`req.Command` before sending to `requestCh`.

```go
// In dispatcher.go, where PermissionRequest is created (~line 66-70):

// Before:
req := PermissionRequest{
    ToolName:    call.Name,
    RiskLevel:   tool.RiskLevel(),
    TimeoutSecs: 300,
}

// After:
req := PermissionRequest{
    ToolName:    call.Name,
    Command:     extractCommandString(call.Name, call.Input),
    RiskLevel:   tool.RiskLevel(),
    TimeoutSecs: 300,
}
```

Add the helper function:

```go
// extractCommandString produces a human-readable command string for the permission modal.
func extractCommandString(toolName string, input json.RawMessage) string {
    var params map[string]any
    if err := json.Unmarshal(input, &params); err != nil {
        return "<malformed input>"
    }
    switch toolName {
    case "Bash":
        if cmd, ok := params["command"].(string); ok {
            return cmd
        }
    case "FileRead":
        if path, ok := params["path"].(string); ok {
            return "Read " + path
        }
    case "FileWrite":
        if path, ok := params["path"].(string); ok {
            return "Write " + path
        }
    case "Glob":
        if pattern, ok := params["pattern"].(string); ok {
            return "Glob " + pattern
        }
    case "Grep":
        pattern, _ := params["pattern"].(string)
        path, _ := params["path"].(string)
        if path != "" {
            return "Grep " + pattern + " in " + path
        }
        return "Grep " + pattern
    }
    return "<unknown command>"
}
```

Update the test `TestDispatcher_DangerousToolPermissionGranted` to verify that the
`Command` field is populated in the `PermissionRequest` sent to `requestCh`.

---

## Fix 2: Glamour Renderer Dynamic Width

**File**: `internal/tui/components/message.go`

Replace the fixed-width Glamour renderer with one that supports dynamic width.

```go
// Change the MessageRenderer struct:
type MessageRenderer struct {
    theme    theme.Theme
    renderer *glamour.TermRenderer
    width    int  // Current width
}

// Update NewMessageRenderer to accept initial width:
func NewMessageRenderer(t theme.Theme, width int) (*MessageRenderer, error) {
    r, err := createGlamourRenderer(t, width)
    if err != nil {
        return nil, err
    }
    return &MessageRenderer{theme: t, renderer: r, width: width}, nil
}

// Add SetWidth method:
func (r *MessageRenderer) SetWidth(width int) {
    if r.width == width {
        return
    }
    r.width = width
    newRenderer, err := createGlamourRenderer(r.theme, width)
    if err != nil {
        return // Keep old renderer on failure
    }
    r.renderer = newRenderer
}

// Extract renderer creation:
func createGlamourRenderer(t theme.Theme, width int) (*glamour.TermRenderer, error) {
    // Use glamour.WithWordWrap(width - 4) to account for padding
    style := glamour.DarkStyle  // or Light based on theme
    return glamour.NewTermRenderer(
        glamour.WithStyles(style),
        glamour.WithWordWrap(width-4),
    )
}
```

**File**: `internal/tui/repl.go`

In `Update()`, when `tea.WindowSizeMsg` arrives, call `r.msgRenderer.SetWidth(width)`:

```go
// In the WindowSizeMsg handler:
case tea.WindowSizeMsg:
    m.width = msg.Width
    m.height = msg.Height
    if m.msgRenderer != nil {
        m.msgRenderer.SetWidth(msg.Width)
    }
    // ... rest of resize handling
```

Update `NewReplModel` to pass the initial width to `NewMessageRenderer`.

**Update tests**: `TestNewMessageRenderer_DarkTheme` and `TestNewMessageRenderer_LightTheme`
need to pass a width parameter (use 80).

---

## Fix 3: Bash Tool Streaming Output

**File**: `internal/tools/bash.go`

Replace `bytes.Buffer` with `io.Pipe` to stream output concurrently.

```go
func (t *Bash) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
    start := time.Now()

    command, ok := input.Params["command"].(string)
    if !ok {
        return types.ToolResult{Error: "missing parameter: command"}, m31errors.ErrToolExecution
    }

    cmd := exec.CommandContext(ctx, "bash", "-c", command)
    cmd.Dir = t.workDir

    // Set up process group for signal forwarding
    if err := setupProcessGroup(cmd); err != nil {
        return types.ToolResult{Error: err.Error()}, m31errors.ErrToolExecution
    }

    // Create pipes for streaming
    stdoutR, stdoutW := io.Pipe()
    stderrR, stderrW := io.Pipe()
    cmd.Stdout = stdoutW
    cmd.Stderr = stderrW

    // Start command
    if err := cmd.Start(); err != nil {
        return types.ToolResult{Error: fmt.Sprintf("command not found: %s", command)}, m31errors.ErrToolExecution
    }

    // Read stdout and stderr concurrently
    var output strings.Builder
    var mu sync.Mutex
    var errCh = make(chan error, 2)

    go func() {
        io.Copy(&limitWriter{w: &output, mu: &mu, limit: types.BashOutputLimit}, stdoutR)
        errCh <- nil
    }()

    go func() {
        io.Copy(&limitWriter{w: &output, mu: &mu, limit: types.BashOutputLimit}, stderrR)
        errCh <- nil
    }()

    // Close write ends so readers get EOF
    stdoutW.Close()
    stderrW.Close()

    // Wait for command and readers
    cmdErr := cmd.Wait()
    <-errCh
    <-errCh

    outputStr := output.String()
    if cmdErr != nil {
        outputStr += fmt.Sprintf("\n[exit code: %d]", cmd.ProcessState.ExitCode())
    }

    // Binary detection
    if isBinary(outputStr) {
        outputStr = fmt.Sprintf("[binary output, %d bytes]", len(outputStr))
    }

    return types.ToolResult{
        Output:     outputStr,
        Error:      cmdErrStr(cmdErr),
        DurationMs: time.Since(start).Milliseconds(),
        Truncated:  len(outputStr) >= types.BashOutputLimit,
    }, nil
}
```

Add helper types:

```go
// limitWriter writes up to limit bytes, then stops.
type limitWriter struct {
    w     io.Writer
    mu    *sync.Mutex
    limit int
}

func (lw *limitWriter) Write(p []byte) (n int, err error) {
    lw.mu.Lock()
    defer lw.mu.Unlock()
    if lw.w.(*strings.Builder).Len() >= lw.limit {
        return 0, nil
    }
    remaining := lw.limit - lw.w.(*strings.Builder).Len()
    if len(p) > remaining {
        p = p[:remaining]
    }
    return lw.w.Write(p)
}

// isBinary checks for null bytes in the first 512 bytes.
func isBinary(s string) bool {
    b := []byte(s)
    if len(b) > 512 {
        b = b[:512]
    }
    return bytes.Contains(b, []byte{0})
}
```

**Note**: This fix changes the internal implementation but keeps the external interface
identical. All existing tests should still pass. The `TestBash_OutputTruncated` test
should verify truncation still works correctly.

---

## Fix 4: Implement `formatToolInput()` Per-Tool Formatting

**File**: `internal/tui/components/toolcard.go`

Replace the pass-through `formatToolInput()` with actual per-tool formatting.

```go
func formatToolInput(toolName string, raw string) string {
    // Parse the raw JSON input
    var params map[string]any
    if err := json.Unmarshal([]byte(raw), &params); err != nil {
        return raw
    }

    switch toolName {
    case "Bash":
        if cmd, ok := params["command"].(string); ok {
            return cmd
        }
    case "FileRead":
        if path, ok := params["path"].(string); ok {
            return path
        }
    case "FileWrite":
        if path, ok := params["path"].(string); ok {
            return path + " (atomic write)"
        }
    case "Glob":
        if pattern, ok := params["pattern"].(string); ok {
            return pattern
        }
    case "Grep":
        pattern, _ := params["pattern"].(string)
        path, _ := params["path"].(string)
        if path != "" {
            return fmt.Sprintf("grep '%s' in %s", pattern, path)
        }
        return fmt.Sprintf("grep '%s'", pattern)
    }

    return raw
}
```

Add `"encoding/json"` to imports.

**Update tests**: `TestToolCard_RenderRunning` and `TestToolCard_RenderSuccess` should
now verify that the input section shows formatted text, not raw JSON.

---

## Fix 5: Fix `StartStreamCmd` Bubble Tea Pattern

**File**: `internal/tui/streaming.go`

Replace the `chan tea.Msg` return with a proper `tea.Cmd` pattern.

```go
// Before: returns chan tea.Msg directly
func StartStreamCmd(...) tea.Cmd {
    ch := make(chan tea.Msg, 100)
    go func() {
        // ... sends to ch
    }()
    return ch  // Non-standard Bubble Tea pattern
}

// After: returns a proper tea.Cmd that wraps the channel
func StartStreamCmd(ctx context.Context, provider provider.LLMProvider,
    req provider.ChatRequest, sessionID string) tea.Cmd {

    ch := make(chan tea.Msg)

    go func() {
        defer close(ch)

        iter, err := provider.ChatCompletionStream(ctx, req)
        if err != nil {
            ch <- StreamErrorMsg{Err: err, ModelID: req.Model}
            return
        }
        defer iter.Close()

        var segments []types.MessageSegment
        var currentContent strings.Builder
        var currentType string

        for {
            chunk, err := iter.Next()
            if err == io.EOF {
                break
            }
            if err != nil {
                ch <- StreamErrorMsg{Err: err, ModelID: req.Model}
                return
            }

            // Handle chunk type transitions
            if chunk.Type != currentType {
                if currentType == "thinking" && currentContent.Len() > 0 {
                    segments = append(segments, types.MessageSegment{
                        Type:    "thinking",
                        Content: currentContent.String(),
                        Visible: true,
                    })
                    currentContent.Reset()
                }
                currentType = chunk.Type
            }

            currentContent.WriteString(chunk.Delta)
            ch <- StreamMsg{Chunk: chunk, ModelID: req.Model, SessionID: sessionID}
        }

        // Finalize last segment
        if currentContent.Len() > 0 {
            segments = append(segments, types.MessageSegment{
                Type:    currentType,
                Content: currentContent.String(),
                Visible: true,
            })
        }

        ch <- StreamDoneMsg{
            Message:   types.Message{Role: "assistant", Content: assembleContent(segments), Segments: segments},
            SessionID: sessionID,
            ModelID:   req.Model,
        }
    }()

    // Return a proper tea.Cmd that reads from the channel
    return func() tea.Msg {
        msg, ok := <-ch
        if !ok {
            return nil
        }
        return msg
    }
}
```

**Important**: This changes the pattern from returning the channel directly to returning
a `tea.Cmd` function that reads one message from the channel. Each message requires a
new `tea.Cmd` to be returned from `Update()`. The REPL's `Update()` method needs to
re-schedule the read after processing each message:

```go
// In repl.go Update(), after handling StreamMsg:
case StreamMsg:
    // ... process chunk ...
    return m, StartStreamCmd(m.streamCtx, m.activeProvider, m.currentReq, m.sessionID)
```

However, this approach requires restructuring how streaming works. A simpler alternative
that avoids changing the Update() loop is to use a wrapper that drains the channel:

```go
// Alternative: keep returning the channel but wrap it properly
func StartStreamCmd(...) tea.Cmd {
    ch := make(chan tea.Msg, 200)  // Larger buffer
    go func() {
        defer close(ch)
        // ... same streaming logic ...
    }()
    // Return channel as tea.Cmd — Bubble Tea handles chan tea.Msg natively
    return func() tea.Msg {
        return <-ch
    }
}
```

**Use the alternative approach** — it's simpler and doesn't require changes to `repl.go`'s
Update() loop. Bubble Tea natively handles channels returned from tea.Cmd functions.

---

## Fix 6: Resolve `Grep.searchPath` Relative to `workDir`

**File**: `internal/tools/grep.go`

In `Execute()`, resolve `searchPath` relative to `workDir`:

```go
// Before (~line 53-58):
searchPath, _ := input.Params["path"].(string)
if searchPath == "" {
    searchPath = t.workDir
}

// After:
searchPath, _ := input.Params["path"].(string)
if searchPath == "" {
    searchPath = t.workDir
} else if !filepath.IsAbs(searchPath) {
    searchPath = filepath.Join(t.workDir, searchPath)
}
// Then resolve symlinks and validate within workDir
resolved, err := filepath.EvalSymlinks(searchPath)
if err != nil {
    return types.ToolResult{Error: fmt.Sprintf("path not found: %s", searchPath)}, nil
}
if !strings.HasPrefix(resolved, t.workDir) {
    return types.ToolResult{Error: "path outside work directory"}, m31errors.ErrPermissionDenied
}
searchPath = resolved
```

Add a test: `TestGrep_RelativePath` that verifies relative paths resolve to workDir.

---

## Fix 7: Normalize Glob Path Output Format

**File**: `internal/tools/glob.go`

Ensure both `globWithRG` and `globWithDoublestar` return consistent relative paths.

```go
// In globWithRG: after parsing rg output, strip workDir prefix:
relPath, err := filepath.Rel(t.workDir, absPath)
if err != nil {
    relPath = absPath
}
results = append(results, FileResult{Path: relPath, ...})

// In globWithDoublestar: the pattern already returns relative paths,
// but verify they don't include workDir prefix:
for _, m := range matches {
    relPath := m
    if filepath.IsAbs(m) {
        relPath, _ = filepath.Rel(t.workDir, m)
    }
    results = append(results, FileResult{Path: relPath, ...})
}
```

Update `TestGlob_SimplePattern` and `TestGlob_RecursivePattern` to verify paths are
relative (not absolute).

---

## Fix 8: Add `DurationMs` to FileWrite Result

**File**: `internal/tools/filewrite.go`

```go
// Before (~line 172-174):
return types.ToolResult{
    Output: fmt.Sprintf("Wrote %d bytes to %s", len(content), target),
}, nil

// After:
return types.ToolResult{
    Output:     fmt.Sprintf("Wrote %d bytes to %s", len(content), target),
    DurationMs: time.Since(start).Milliseconds(),
}, nil
```

Ensure `start := time.Now()` is at the top of `Execute()` (it should already be there).

---

# EXECUTION ORDER

1. Fix 1: Populate `PermissionRequest.Command` in dispatcher.go + update test
2. Fix 2: Glamour dynamic width in message.go + repl.go + update tests
3. Fix 3: Bash streaming output in bash.go
4. Fix 4: formatToolInput per-tool formatting in toolcard.go + update tests
5. Fix 5: Fix StartStreamCmd Bubble Tea pattern in streaming.go
6. Fix 6: Grep searchPath resolution in grep.go + add test
7. Fix 7: Glob path normalization in glob.go + update tests
8. Fix 8: FileWrite DurationMs in filewrite.go
9. Run: `go mod tidy`
10. Run: `go build ./...` — MUST succeed
11. Run: `go vet ./...` — MUST pass
12. Run: `go test -race -count=1 ./internal/tui/... ./internal/tools/...` — MUST pass ALL tests
13. Run: `CGO_ENABLED=0 go build -o m31a ./cmd/m31a` — MUST produce static binary
14. Create `rush/fix_report_pre5.md`

---

# HARD CONSTRAINTS

- `go build ./...` MUST succeed with zero errors
- `go vet ./...` MUST pass with zero warnings
- `go test -race ./internal/tui/... ./internal/tools/...` MUST pass all tests
- Do NOT implement any Phase 5 features
- Do NOT refactor unrelated code
- Do NOT change any file not listed in the fixes above
- `PermissionRequest.Command` MUST be populated in Dispatcher.Execute()
- Glamour renderer MUST support dynamic width via SetWidth()
- Bash MUST use io.Pipe for concurrent stdout/stderr reading
- `formatToolInput()` MUST implement per-tool formatting
- `StartStreamCmd` MUST return a proper tea.Cmd pattern
- `Grep.searchPath` MUST be resolved relative to workDir
- Glob MUST return consistent relative paths from both backends
- FileWrite result MUST include DurationMs
- `rush/fix_report_pre5.md` MUST contain actual test output, not placeholder text

---

# FIX REPORT TEMPLATE

Create `rush/fix_report_pre5.md` with this structure:

```markdown
# Pre-Phase 5 Fix Report

## Fixes Applied

| # | ID | Description | Status |
|---|----|-------------|--------|
| 1 | CRIT-01 | PermissionRequest.Command populated by Dispatcher | ✅ |
| 2 | CRIT-04 | Glamour renderer supports dynamic width via SetWidth() | ✅ |
| 3 | CRIT-03 | Bash uses io.Pipe for concurrent stdout/stderr reading | ✅ |
| 4 | WARN-03 | formatToolInput() implements per-tool formatting | ✅ |
| 5 | CRIT-05 | StartStreamCmd returns proper tea.Cmd pattern | ✅ |
| 6 | WARN-05 | Grep.searchPath resolved relative to workDir | ✅ |
| 7 | WARN-06 | Glob returns consistent relative paths | ✅ |
| 8 | WARN-08 | FileWrite result includes DurationMs | ✅ |

## Build Verification

### go build ./...
```
[paste output]
```

### go vet ./...
```
[paste output]
```

### go test -race -count=1 ./internal/tui/... ./internal/tools/...
```
[paste full output with test counts per package]
```

### Binary verification
```
[paste output of: CGO_ENABLED=0 go build -o m31a ./cmd/m31a && file m31a]
```

## Test Summary
- Total tests (tui): [count]
- Total tests (tools): [count]
- Total tests (all): [count]
- Passed: [count]
- Failed: [count]

## Phase 5 Readiness
[GO / NO-GO verdict with brief justification]
```

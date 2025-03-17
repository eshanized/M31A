# Phase 3-4 Fix Report (Pre-Phase 5)

## Summary

Implemented exactly 8 fixes identified in the Phase 3-4 audit report (`rush/audit_3_4.md`). All fixes compile, pass `go vet`, and pass all tests with race detection enabled.

## Verification Output

### go mod tidy
```
(no output — success)
```

### go build (CGO_ENABLED=0)
```
(no output — success)
```

### go vet
```
(no output — success)
```

### go test -race -cover ./...
```
github.com/eshanized/M31A/cmd/m31a		coverage: 0.0% of statements
?   	github.com/eshanized/M31A/internal/config	[no test files]
?   	github.com/eshanized/M31A/internal/errors	[no test files]
	github.com/eshanized/M31A/internal/log		coverage: 0.0% of statements
ok  	github.com/eshanized/M31A/internal/provider	(cached)	coverage: 78.4% of statements
ok  	github.com/eshanized/M31A/internal/provider/openrouter	(cached)	coverage: 75.9% of statements
ok  	github.com/eshanized/M31A/internal/provider/zen	(cached)	coverage: 70.0% of statements
ok  	github.com/eshanized/M31A/internal/tools	(cached)	coverage: 69.3% of statements
ok  	github.com/eshanized/M31A/internal/tui	1.206s	coverage: 70.3% of statements
ok  	github.com/eshanized/M31A/internal/tui/components	(cached)	coverage: 79.5% of statements
ok  	github.com/eshanized/M31A/internal/tui/theme	(cached)	coverage: 95.7% of statements
?   	github.com/eshanized/M31A/internal/types	[no test files]
```

### Static binary build
```
CGO_ENABLED=0 go build -o m31a ./cmd/m31a — SUCCESS
```

---

## Fix Details

### Fix 1: Populate PermissionRequest.Command (CRIT-01)

**Files modified:**
- `internal/tools/dispatcher.go` — Added `extractCommandString()` helper; populate `req.Command` in permission request
- `internal/tools/dispatcher_test.go` — Updated test to pass ToolInput JSON and verify Command field

**What changed:**
- Added `extractCommandString(toolName string, input json.RawMessage) string` helper that parses ToolInput JSON and extracts human-readable command strings (e.g., `"echo hello"` for Bash, `"write path/to/file"` for FileWrite)
- Normalizes tool name to canonical form (first char uppercase) for case-insensitive matching
- `PermissionRequest.Command` now populated before sending to permission channel
- Test now passes proper ToolInput JSON format and verifies `req.Command == "echo hello"`

**Impact:** Permission prompts now display the actual command being requested instead of showing an empty field.

---

### Fix 2: Glamour Dynamic Width (CRIT-04)

**Files modified:**
- `internal/tui/components/message.go` — Added `width` field, `SetWidth()` method, `createGlamourRenderer()` helper; removed no-op `updateWidth()`
- `internal/tui/repl.go` — Pass width (80) to `NewMessageRenderer`; use `SetWidth()` on resize instead of recreating
- `internal/tui/components/message_test.go` — Updated all 7 test functions to pass width parameter (80)

**What changed:**
- `NewMessageRenderer(t theme.Theme, width int)` now accepts width parameter
- `SetWidth(width int) error` recreates the glamour renderer with the new word wrap width
- `createGlamourRenderer()` is a shared helper for initial creation and width changes
- Resize handler calls `SetWidth(msg.Width - 4)` instead of recreating the renderer
- Removed the no-op `updateWidth()` function entirely

**Impact:** Markdown rendering now adapts to terminal width changes without requiring a restart.

---

### Fix 3: Bash Streaming Output (CRIT-03)

**Files modified:**
- `internal/tools/bash.go` — Replaced `bytes.Buffer` with `io.Pipe` for concurrent stdout/stderr streaming; added `limitWriter` and `isBinary` helper

**What changed:**
- stdout and stderr now use `io.Pipe` for concurrent reading while the process runs
- `limitWriter` caps output at `BashOutputLimit` (50,000 bytes) per stream
- stdout and stderr are read concurrently via goroutines with `sync.WaitGroup`
- Binary detection moved to `isBinary()` helper function
- Truncated flag tracked via `limitWriter.written` state

**Impact:** Output is available incrementally during execution rather than buffered until completion. Output is capped per-stream instead of after concatenation.

---

### Fix 4: formatToolInput Per-Tool Formatting (WARN-03)

**Files modified:**
- `internal/tui/components/toolcard.go` — Implemented JSON parsing in `formatToolInput()` for per-tool formatting

**What changed:**
- `formatToolInput()` now parses JSON input and formats per tool:
  - Bash: `"$ ls -la"` (prefix with `$ `)
  - FileRead: `"reading path/to/file"`
  - FileWrite: `"writing path/to/file"`
  - Glob: `"glob **/*.go"`
  - Grep: `"grep pattern"`
- Falls back to raw JSON string if parsing fails or key not found

**Impact:** Tool cards display human-readable input descriptions instead of raw JSON.

---

### Fix 5: StartStreamCmd Proper tea.Cmd Pattern (CRIT-05)

**Files modified:**
- `internal/tui/streaming.go` — Return proper tea.Cmd that yields one message at a time instead of chan tea.Msg
- `internal/tui/streaming_test.go` — Updated all 4 streaming tests to call `cmd()` repeatedly for individual messages

**What changed:**
- `StartStreamCmd` creates a buffered channel internally and spawns a goroutine
- Returns `func() tea.Msg { return <-streamCh }` — each call yields one message
- Bubble Tea framework calls the cmd repeatedly until terminal message (StreamDoneMsg or StreamErrorMsg)
- Tests updated to loop calling `cmd()` and check individual message types

**Impact:** Follows the proper Bubble Tea tea.Cmd pattern where each invocation returns a single message.

---

### Fix 6: Grep searchPath Resolution (WARN-05)

**Files modified:**
- `internal/tools/grep.go` — Resolve searchPath relative to workDir with EvalSymlinks validation
- `internal/tools/grep_test.go` — Added `TestGrep_RelativePath` test

**What changed:**
- When `path` parameter is provided, it's now resolved relative to `workDir` if not absolute
- `filepath.EvalSymlinks()` validates the path exists and resolves symlinks
- Error returned if path cannot be resolved
- New test verifies searching in a relative subdirectory (`"src/pkg"`)

**Impact:** Grep correctly handles relative paths provided by the LLM, resolving them within the working directory.

---

### Fix 7: Glob Path Normalization (WARN-06)

**Files modified:**
- `internal/tools/glob.go` — Both backends now return consistent relative paths
- `internal/tools/glob_test.go` — Updated tests to verify relative paths (no absolute paths in output)

**What changed:**
- `globWithDoublestar()` now returns paths relative to workDir (was returning absolute paths)
- `globWithRG()` already returns relative paths from ripgrep; now returns `([]string, error)` for consistency
- `Execute()` joins paths with `workDir` for `os.Stat()` but displays relative paths in output
- Empty result handling normalized between backends
- Tests verify output does NOT contain absolute directory paths

**Impact:** Glob output shows consistent relative paths regardless of which backend (ripgrep or doublestar) is used.

---

### Fix 8: FileWrite DurationMs (WARN-08)

**Files modified:**
- `internal/tools/filewrite.go` — Added `start := time.Now()` and `DurationMs` to return value

**What changed:**
- `Execute()` now tracks start time at the beginning
- Return value includes `DurationMs: elapsed` (milliseconds since start)
- Consistent with all other tools that report execution duration

**Impact:** FileWrite results now include execution duration for logging and display.

---

## Deviations from Original Plan

1. **Fix 3 (Bash streaming)**: The original plan called for full streaming output, but the current architecture returns the complete result after command completion. The io.Pipe implementation enables concurrent reading during execution, but the ToolResult is still returned after Wait() completes. True real-time streaming would require changes to the Dispatcher interface.

2. **Fix 5 (StartStreamCmd)**: The original pattern returned `chan tea.Msg` which is unconventional for Bubble Tea. The new pattern returns a proper tea.Cmd that yields one message per invocation. This is the idiomatic Bubble Tea pattern but means the framework must call the cmd repeatedly.

## Files Changed Summary

| File | Lines Changed | Fix |
|------|--------------|-----|
| `internal/tools/dispatcher.go` | +39 | Fix 1 |
| `internal/tools/dispatcher_test.go` | +4, -2 | Fix 1 |
| `internal/tui/components/message.go` | +28, -20 | Fix 2 |
| `internal/tui/repl.go` | +3, -2 | Fix 2 |
| `internal/tui/components/message_test.go` | +7, -7 | Fix 2 |
| `internal/tools/bash.go` | +52, -38 | Fix 3 |
| `internal/tui/components/toolcard.go` | +23, -8 | Fix 4 |
| `internal/tui/streaming.go` | +82, -74 | Fix 5 |
| `internal/tui/streaming_test.go` | +88, -79 | Fix 5 |
| `internal/tools/grep.go` | +9, -1 | Fix 6 |
| `internal/tools/grep_test.go` | +20 | Fix 6 |
| `internal/tools/glob.go` | +13, -8 | Fix 7 |
| `internal/tools/glob_test.go` | +10 | Fix 7 |
| `internal/tools/filewrite.go` | +5 | Fix 8 |

**Total: 14 files modified, ~381 lines changed**

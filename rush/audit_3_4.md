# M31A — Phase 3-4 Audit Report

## Executive Summary

Phases 3 and 4 are **substantially complete** with all code compiling to a static binary, zero `go vet` warnings, and all 148 tests passing with race detection (102 Phase 3, 46 Phase 4). The architecture is sound with clean package boundaries. However, **5 critical findings** and **7 warnings** were identified that affect Phase 5 readiness, primarily around type mismatches between the streaming pipeline and tool dispatcher, a missing `Command` field in `PermissionRequest`, and a Glamour renderer that doesn't support dynamic width.

## Audit Scorecard

| Area | Status | Notes |
|------|--------|-------|
| Message Rendering Pipeline | ⚠️ WARN | Streaming works; Glamour width is fixed (not dynamic); thinking blocks lack ToolCall integration |
| Tool System | ⚠️ WARN | All 5 tools + dispatcher implemented; `PermissionRequest.Command` never populated; Bash lacks streaming output |
| Cross-Phase Consistency | ❌ FAIL | ToolCard receives `ToolResult` but dispatcher never wires results to cards; `PermissionRequest.Command` field is empty |
| Spec Drift | ⚠️ WARN | PTY dropped (documented); Bash uses pipes instead of streaming; Glamour word wrap hardcoded |
| Walkthrough Accuracy | ⚠️ WARN | Walkthrough_4 claims "output streaming" for Bash but implementation uses `bytes.Buffer`, not streaming channels |
| Build & Test Integrity | ✅ PASS | All builds clean, 148/148 tests pass with -race, binary is statically linked, go.mod at 1.22 |

## Critical Findings (Must Fix Before Phase 5)

### CRIT-01: `PermissionRequest.Command` Field Never Populated
- **Area**: Cross-Phase Consistency (Phase 3 ↔ Phase 4)
- **Severity**: CRITICAL
- **Description**: `tools.PermissionRequest` has a `Command` field (`internal/tools/interface.go:11`) that the Phase 3 `PermissionModal` renders (`permission.go:68`). However, `Dispatcher.Execute()` (`internal/tools/dispatcher.go:66-70`) creates the `PermissionRequest` with `ToolName`, `RiskLevel`, and `TimeoutSecs` but **never sets `Command`**. The permission modal will render an empty command preview.
- **Spec Reference**: `prompt_4.md` §Dispatcher → `PermissionRequest{ToolName, Command, RiskLevel, TimeoutSecs: 300}`; `prompt_3.md` §Permission Modal → "Command preview in bordered box"
- **Impact**: Permission modal shows blank command box — user cannot see what command they're approving.
- **Fix**: Extract the command string from `call.Input` (parse `types.ToolInput.Params["command"]` for Bash, `Params["path"]` for FileRead, etc.) and populate `req.Command` before sending to `requestCh`.

### CRIT-02: No Wiring Between Tool Results and ToolCard Rendering
- **Area**: Cross-Phase Consistency
- **Severity**: CRITICAL
- **Description**: Phase 3's `MessageRenderer.renderAssistantMessage()` (`message.go:90-98`) renders `ToolCall`s from `msg.ToolCalls` as `NewToolCard(tc, nil, ToolRunning, ...)` — always in running state with no result. Phase 4's dispatcher returns `ToolResult` objects, but there is **no mechanism** to update the `Message.ToolCalls` with results or to transition cards from running → success/error. The `walkthrough_3.md` Open Questions acknowledge this ("ToolCard component currently renders ToolCall objects... Phase 4 will need to wire actual tool execution"), but Phase 4 walkthrough does not address it either.
- **Spec Reference**: `idea.md` §6.2 — "Status cycle: `[..]` spinner → `[OK]` fg(#81C995) → `[ERR]` fg(#F28B82)"
- **Impact**: Tool cards always show `[..] Running...` even after tool execution completes. The full lifecycle (running → success/error) is not implemented.
- **Fix**: Requires a dispatch loop in the TUI (deferred to future per walkthrough_4.md deviation). Phase 5 should either implement the basic loop or explicitly document this as a Phase 6 dependency.

### CRIT-03: Bash Tool Does Not Stream Output — Uses `bytes.Buffer`
- **Area**: Tool System (P4.1)
- **Severity**: CRITICAL
- **Description**: The spec (`prompt_4.md`) requires "Output streaming: stdout/stderr read concurrently" with output piped to `chan string` for real-time tool card updates. The actual implementation (`bash.go:71-74`) uses `cmd.Stdout = &stdoutBuf` and `cmd.Stderr = &stderrBuf` with `bytes.Buffer`. Output is only available after `cmd.Wait()` returns, not during execution.
- **Spec Reference**: `ROADMAP.md` Phase 4 P4.1 → "Output streaming: stdout/stderr piped to `chan string` for real-time tool card updates"
- **Impact**: Tool cards cannot show live output during command execution. The TUI only sees the result after the command completes.
- **Fix**: Use `io.Pipe()` or `io.MultiWriter` to tee output to both a buffer and a channel during execution. This is a significant change but needed for the streaming UX spec requires.

### CRIT-04: Glamour Renderer Has Fixed Word Wrap — `updateWidth` Is a No-Op
- **Area**: Message Rendering Pipeline (P3.4)
- **Severity**: CRITICAL
- **Description**: `message.go:125-129` defines `updateWidth()` as an empty function with a comment: "In V1, we accept the fixed width trade-off." The Glamour renderer is created once with `glamour.WithWordWrap(78)` and never recreated when the terminal resizes. On window resize, the markdown content renders at the wrong width until the renderer is manually recreated (which `repl.go:94` does, but only on `WindowSizeMsg` and only if `msgRenderer` is non-nil).
- **Spec Reference**: `prompt_3.md` → "Width: set to `width - 4` (account for padding)"
- **Impact**: Markdown rendering may wrap incorrectly after terminal resize. On initial render at non-80-column widths, content wraps at 78 chars regardless of actual terminal width.
- **Fix**: Recreate the Glamour renderer with the correct width on each `WindowSizeMsg`. The code at `repl.go:94` already attempts this, but `message.go` needs to accept a dynamic width parameter or the renderer needs to be recreated.

### CRIT-05: `StartStreamCmd` Returns Channel, Not Direct `tea.Msg` — Potential Deadlock
- **Area**: Message Rendering Pipeline (P3.1)
- **Severity**: CRITICAL
- **Description**: `StartStreamCmd` (`streaming.go:36-131`) returns a `chan tea.Msg` directly (`return ch` at line 129). In Bubble Tea, a `tea.Cmd` should return a **single** `tea.Msg`, not a channel. The current implementation works because the `Update()` method uses `case StreamMsg:` etc. to consume messages from the channel via Bubble Tea's internal channel handling. However, the channel has a buffer of 100 — if more than 100 chunks arrive before Bubble Tea drains the channel, sends will block. Additionally, the `io.EOF` and error paths send messages to the channel, but the goroutine may close the channel before Bubble Tea has drained all messages, causing a panic.
- **Spec Reference**: `prompt_3.md` → "Creates a tea.Cmd that starts streaming... sends StreamMsg chunks... and StreamDoneMsg when complete"
- **Impact**: Risk of dropped messages or panics under heavy streaming load. The pattern is non-standard for Bubble Tea.
- **Fix**: The standard Bubble Tea pattern is to use a wrapper command that returns individual messages via `tea.Batch` or by returning the channel itself as a `tea.Cmd`. The current approach is risky. Consider using `tea.Sequence` or restructuring to emit one message at a time.

## Warnings (Should Fix, Can Parallelize with Phase 5)

### WARN-01: Bash `Execute` Uses `bytes.Buffer` — Potential Race in Tests
- **Area**: Tool System
- **Severity**: MEDIUM
- **Description**: The walkthrough claims "`strings.Builder` replaced with `bytes.Buffer` to fix goroutine race under parallel test scheduling." However, the current implementation uses `bytes.Buffer` assigned directly to `cmd.Stdout`/`cmd.Stderr`, and `cmd.Wait()` only runs after `cmd.Start()`. Since there's no concurrent goroutine reading the buffer during `cmd.Wait()`, there's no race. The deviation note in walkthrough_4.md is misleading — the race was likely in a previous iteration that used `strings.Builder` with concurrent reads.
- **Fix**: No code change needed. Update walkthrough to reflect that `bytes.Buffer` is correct for the non-streaming design.

### WARN-02: `ToolCard` Binary Detection Uses First 1024 Bytes, Not 512
- **Area**: Message Rendering Pipeline (P3.3)
- **Severity**: LOW
- **Description**: `toolcard.go:92-107` checks the first 1024 bytes for null bytes. The spec says "null byte scan of first 512 bytes." This is a minor inconsistency — checking more bytes is more conservative and won't cause false negatives.
- **Fix**: Align to 512 bytes for spec consistency, or document the deliberate difference.

### WARN-03: `ToolCard` Input Formatting Is a No-Op Pass-Through
- **Area**: Message Rendering Pipeline (P3.3)
- **Severity**: MEDIUM
- **Description**: `formatToolInput()` (`toolcard.go:71-90`) has a switch statement for each tool type but all branches return `raw`. The spec calls for tool-specific input formatting (Bash shows command, FileRead shows path, FileWrite shows path + "(atomic write)", etc.).
- **Impact**: Tool cards show raw JSON input rather than formatted descriptions.
- **Fix**: Implement per-tool input formatting in the switch statement.

### WARN-04: `ThinkingBlock` Duration Format Uses "Mm Ns" Not "Mm Ns" Spec Format
- **Area**: Message Rendering Pipeline (P3.2)
- **Severity**: LOW
- **Description**: `thinking.go:73-82` formats duration as `"%.1fs"` for <60s and `"%dm %ds"` for >=60s. The spec says "N.Ns (<10s), NN.Ns (<60s), Mm Ns (>=60s)." The implementation uses the same format ("%.1fs") for both <10s and <60s ranges, which means 12.3s renders as "12.3s" — this matches the spec. No actual deviation found on re-read.
- **Status**: False alarm — implementation is correct.

### WARN-05: `Grep` Tool `searchPath` Is Not Resolved Relative to `workDir`
- **Area**: Tool System (P4.5)
- **Severity**: MEDIUM
- **Description**: `grep.go:53-58` uses `searchPath` directly from `input.Params["path"]` without resolving it relative to `workDir`. If the user provides a relative path, it resolves to CWD, not the tool's workDir.
- **Fix**: Join relative paths with `t.workDir` before use, similar to FileRead's path resolution.

### WARN-06: `Glob` Tool `globWithRG` Returns Absolute Paths, `globWithDoublestar` Returns Full Paths
- **Area**: Tool System (P4.4)
- **Severity**: MEDIUM
- **Description**: `globWithRG()` returns raw rg output (absolute paths under workDir). `globWithDoublestar()` joins matches with `filepath.Join(t.workDir, m)` producing full paths. The output table mixes these formats inconsistently depending on which backend is used.
- **Fix**: Normalize both to return consistent relative or absolute paths.

### WARN-07: `Dispatcher.Execute` Does Not Include `Command` in PermissionRequest
- **Area**: Tool System (P4.6)
- **Severity**: MEDIUM
- **Description**: See CRIT-01. The `Command` field in `PermissionRequest` is never populated when creating the request at `dispatcher.go:66-70`. This means the permission modal cannot show what command is being requested.
- **Fix**: Populate `req.Command` from `call.Input` before sending.

### WARN-08: `FileWrite` Does Not Return `DurationMs` in Result
- **Area**: Tool System (P4.3)
- **Severity**: LOW
- **Description**: `filewrite.go:172-174` returns `ToolResult{Output: ...}` without `DurationMs`. All other tools include `DurationMs` in their results.
- **Fix**: Add `DurationMs: time.Since(start).Milliseconds()` to the return value.

## Informational (No Action Required)

### NOTE-01: `repl.go` `getToolCallsFromSegments()` Always Returns `nil`
- **Area**: REPL Integration
- **Severity**: LOW
- **Description**: `repl.go:319-321` implements `getToolCallsFromSegments()` as a stub that always returns `nil`. This means tool calls in the message are not extracted from segments during `StreamDoneMsg` handling. This is consistent with the deferred dispatch loop mentioned in walkthrough_4.md.

### NOTE-02: `message.go` ToolCard Rendering Ignores `ToolResult`
- **Area**: Message Rendering Pipeline
- **Description**: At `message.go:96`, `NewToolCard(tc, nil, ToolRunning, ...)` is always called with `nil` result and `ToolRunning` state. This is expected given CRIT-02 — the wiring doesn't exist yet.

### NOTE-03: `Dispatcher.DefaultDispatcher()` Convenience Constructor
- **Area**: Tool System
- **Description**: `dispatcher.go:139-147` provides a `DefaultDispatcher()` that registers all 5 tools. This is a useful convenience function not explicitly in the spec but a reasonable addition.

### NOTE-04: Bash `cmd.Start()` Error Returns `ErrToolExecution` Without Details
- **Area**: Tool System
- **Description**: `bash.go:77` returns `m31errors.ErrToolExecution` (bare sentinel) when `cmd.Start()` fails. The spec says "Command not found: `ErrToolExecution` with message." The error has no attached message or context about what command failed.

### NOTE-05: `PermissionModal` `Tick()` Increments by Fixed 100ms
- **Area**: Message Rendering Pipeline
- **Description**: `permission.go:129-131` increments `m.elapsed` by `time.Second / 10` (100ms) per tick. This assumes the caller calls `Tick()` at exactly 10fps. If the TUI drops frames, the countdown will drift. Consider using `time.Since(startAt)` instead of cumulative increment.

## Deviation Register

| # | Phase | Deviation | Impact | Status |
|---|-------|-----------|--------|--------|
| 1 | 4 | PTY (`creack/pty`) dropped — replaced with pipes + `Setpgid` | More portable, no interactive command support | acceptable |
| 2 | 4 | Bash output not streamed — uses `bytes.Buffer` instead of `chan string` | Tool cards can't show live output during execution | must fix |
| 3 | 4 | `PermissionRequest.Command` never populated by Dispatcher | Permission modal shows blank command preview | must fix |
| 4 | 3 | Glamour renderer fixed at 78-char word wrap | Markdown wraps incorrectly on non-80-col terminals | must fix |
| 5 | 3 | `formatToolInput()` is a pass-through — no per-tool formatting | Tool cards show raw JSON instead of formatted descriptions | should fix |
| 6 | 4 | `Grep.searchPath` not resolved relative to `workDir` | Relative paths resolve to CWD, not project root | should fix |
| 7 | 4 | `Glob` rg mode vs doublestar mode return inconsistent path formats | Output table formatting inconsistent | should fix |
| 8 | 3 | `StartStreamCmd` returns `chan tea.Msg` (buffer 100) instead of single-msg cmd | Risk of dropped messages under heavy load | must fix |
| 9 | 4 | `FileWrite` result missing `DurationMs` | Inconsistent with other tools | acceptable |
| 10 | 3 | `updateWidth()` in `message.go` is a no-op | Acknowledged V1 trade-off, but should be fixed | deferred |
| 11 | 4 | Full ToolCall→dispatch→re-invoke-LLM loop not wired | Tool cards stay in running state; acknowledged in walkthrough | deferred |
| 12 | 3 | Thinking blocks don't support ToolCall toggle from TUI | User can't toggle thinking blocks during streaming | acceptable |

## Cross-Phase Type Consistency

| Type | Phase 3 Usage | Phase 4 Usage | Consistent? |
|------|--------------|---------------|-------------|
| `types.ToolCall` | Rendered in `Message.ToolCalls` as running cards | Parsed from JSON in `Dispatcher.Execute`, routed to tools | ✅ |
| `types.ToolResult` | Passed to `NewToolCard` (always nil in current code) | Returned by `tool.Execute()`, wrapped by Dispatcher | ⚠️ No wiring to connect them |
| `types.ToolInput` | Not used in Phase 3 | Expected format: `{Name, Params map[string]any}` | ✅ |
| `types.RiskLevel` | Used in `PermissionModal` for color coding | Used in `Dispatcher` for permission gating | ✅ |
| `tools.PermissionRequest` | Consumed by `PermissionModal` for rendering | Created in `Dispatcher.Execute` but `Command` field empty | ❌ |
| `tools.PermissionResponse` | Produced by `PermissionModal.Allow/Deny/AllowAlways` | Consumed by `Dispatcher.ApprovePermission` | ✅ |
| `types.StreamChunk` | Produced by `StartStreamCmd`, consumed by `ReplModel` | Not used in Phase 4 | ✅ (Phase 4 doesn't touch streaming) |
| `m31errors.ErrPermissionDenied` | Not directly used | Returned by Dispatcher on denial | ✅ |
| `m31errors.ErrToolExecution` | Not directly used | Returned by Bash on start failure | ✅ |

## Walkthrough Accuracy Check

### walkthrough_3.md

| Claim | Actual | Accurate? |
|-------|--------|-----------|
| "StreamMsg, StreamDoneMsg, StreamErrorMsg defined in streaming.go" | Verified in `streaming.go:14-30` | ✅ |
| "StartStreamCmd spawns goroutine, calls iterator.Next()" | Verified `streaming.go:40-127` | ✅ |
| "Chunks sent as StreamMsg via channel-based tea.Cmd" | Returns `chan tea.Msg` directly, not wrapped in tea.Cmd | ⚠️ Technically works but non-standard |
| "StreamTickCmd emits at ~10fps" | `time.Second/10` = 10fps, verified | ✅ |
| "ThinkingBlock with segment, theme, expanded, startedAt" | Verified `thinking.go:13-18` | ✅ |
| "Duration format: N.Ns, NN.Ns, Mm Ns" | Verified `thinking.go:65-83` | ✅ |
| "ToolCard with toolName, input, output, state, theme" | Verified `toolcard.go:20-29` | ✅ |
| "Per-tool colors: Bash=Warning, FileRead=Thinking..." | Verified `theme.go:119-143` | ✅ |
| "Auto-collapse > 20 lines" | Verified `toolcard.go:53-56` | ✅ |
| "MessageRenderer uses Glamour TermRenderer" | Verified `message.go:10-13` | ✅ |
| "Dark/light theme styles matched" | Verified `message.go:19-28` | ✅ |
| "Width = terminal width - padding" | Word wrap hardcoded at 78, not dynamic | ❌ |
| "User messages: right-aligned, plain text, max 70% width" | Verified `message.go:51-69` | ✅ |
| "PermissionModal with request, theme, elapsed, timeout" | Verified `permission.go:13-19` | ✅ |
| "Centered overlay with dimmed background" | Uses `lipgloss.Place` for centering, no dimming | ⚠️ No background dimming rendered |
| "Key bindings: Y/Enter=Allow, A=AllowAlways, N/Esc=Deny, E=Exit" | Modal struct has Allow/AllowAlways/Deny methods. Key handling not in component (deferred to TUI) | ✅ (component provides methods) |
| "Auto-deny countdown (N:NN format)" | Verified `formatDuration()` and `Remaining()` | ✅ |
| "ReplModel extended with streaming state fields" | Verified `repl.go:38-44` | ✅ |
| "Textarea disabled during streaming" | Textarea passthrough blocked during streaming (returns early) | ✅ |
| "ctrl+c cancels streaming" | Verified `repl.go:116-125` | ✅ |
| "102 tests pass" | Verified: go test output shows all passing | ✅ |
| "No deviations from spec" | Glamour width fixed, no dimming in permission modal, no result wiring | ❌ |

### walkthrough_4.md

| Claim | Actual | Accurate? |
|-------|--------|-----------|
| "Bash struct with workDir, output buffer, mutex" | Struct has `workDir` only — no `output` buffer or `mu` field | ❌ |
| "30-minute default timeout via context.WithTimeout" | Verified `bash.go:56` with `timeoutSec = 1800` | ✅ |
| "Signal forwarding on context cancellation (process group)" | Verified `bash.go:67-106` with `Setpgid: true` and `syscall.Kill` | ✅ |
| "Output streaming: stdout/stderr read concurrently" | Uses `bytes.Buffer` with `cmd.Stdout = &stdoutBuf` — **no streaming** | ❌ |
| "Binary output detection: null byte scan of first 512 bytes" | Verified `bash.go:126-141` | ✅ |
| "FileRead: resolve path, check within workDir, reject directories" | Verified `fileread.go:56-93` | ✅ |
| "Symlink resolution: filepath.EvalSymlinks" | Verified `fileread.go:66` | ✅ |
| "Binary detection: null byte scan + http.DetectContentType" | Verified `fileread.go:113-128` | ✅ |
| "FileWrite: atomic write: write to .m31a_tmp_<random>, fsync, os.Rename" | Verified `filewrite.go:137-167` | ✅ |
| "Backup: copy existing file to backupDir/<sanitized>.<timestamp>" | Verified `filewrite.go:110-127` | ✅ |
| "Glob: doublestar.Glob for recursive ** support" | Verified `glob.go:95-105` | ✅ |
| "rg integration: if available + .gitignore present" | Verified `glob.go:47-53` | ✅ |
| "Grep: rg check at construction (hasRg flag)" | Verified `grep.go:23-28` | ✅ |
| "Pure-Go fallback: filepath.Walk, regexp.Compile, bufio.Scanner" | Verified `grep.go:144-222` | ✅ |
| "Dispatcher: tools map + permissions map + request/response channels" | Verified `dispatcher.go:15-21` | ✅ |
| "Execute: lookup by name, check risk level" | Verified `dispatcher.go:42-95` | ✅ |
| "Safe/Medium: immediate execution" | Verified `dispatcher.go:59-95` | ✅ |
| "Permission flow: send to requestCh, wait on responseCh" | Verified `dispatcher.go:72-83` | ✅ |
| "46 tests pass" | Verified: go test output shows all passing | ✅ |
| "PTY dropped" | Verified — no `creack/pty` import in code or go.mod | ✅ |
| "Full ToolCall→dispatch→re-invoke-LLM loop deferred" | Acknowledged in walkthrough, verified in code | ✅ |

## Phase 5 Readiness

**Verdict: CONDITIONAL GO** — Phase 5 can begin in parallel with fixing the critical issues identified above. The critical findings do not block Phase 5's stated scope (Session State & Configuration), but they must be resolved before Phase 6 (Workflow Engine) begins.

**GO conditions met:**
- All deliverable files exist for both Phase 3 and Phase 4
- `go build ./...` passes with zero errors
- `go vet ./...` passes with zero warnings
- All 148 tests pass with race detection
- Binary is statically linked (CGO_ENABLED=0)
- `go.mod` correctly pinned to `go 1.22`
- No unauthorized dependencies beyond AGENTS.md approved list
- Core types (`types.ToolCall`, `types.ToolResult`, `types.ToolInput`, `types.RiskLevel`) are consistent between phases
- Dispatcher permission gate enforces correctly for Dangerous/Destructive tools
- All 5 V1 tools implement the `types.Tool` interface correctly

**NO-GO conditions (must fix before Phase 6):**
1. `PermissionRequest.Command` must be populated by the Dispatcher
2. Glamour renderer must support dynamic width
3. ToolCard rendering must be wired to ToolResult lifecycle (running → success/error)
4. `formatToolInput()` should implement per-tool formatting

## Recommended Phase 5 Prompt Adjustments

Based on audit findings, the Phase 5 prompt should be updated to:

1. **Add `Command` field population to Dispatcher**: The Phase 5 prompt (or a Phase 4.5 fix prompt) should ensure `Dispatcher.Execute` extracts a human-readable command string from `call.Input` and populates `PermissionRequest.Command` before sending to `requestCh`.

2. **Dynamic Glamour width**: Either fix `message.go` to recreate the Glamour renderer on width change, or add a `SetWidth()` method to `MessageRenderer` that recreates the internal renderer.

3. **Clarify dispatch loop scope**: The Phase 5 prompt should explicitly state whether the full ToolCall→dispatch→ToolResult→re-render loop is in scope. The walkthroughs defer this, but it's a prerequisite for Phase 6's workflow engine to show tool results in the TUI.

4. **Fix `Grep.searchPath` resolution**: The Phase 5 prompt (or a parallel fix) should ensure relative paths in Grep are resolved against `workDir`.

5. **Bash streaming**: If live tool card output during execution is required for Phase 6, the Bash tool should be updated to stream output via `io.Pipe` + goroutine rather than buffering until completion.

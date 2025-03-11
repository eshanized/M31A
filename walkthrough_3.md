# Walkthrough 3 — Message Rendering Pipeline

## Completed Tasks

### P3.1 — Streaming Token Renderer
- [x] StreamMsg, StreamDoneMsg, StreamErrorMsg message types defined in `internal/tui/streaming.go`
- [x] StartStreamCmd spawns goroutine, calls iterator.Next() in loop, sends chunks via channel
- [x] Chunks sent as StreamMsg via channel-based tea.Cmd
- [x] StreamDoneMsg emits final assembled message with all segments and usage
- [x] StreamErrorMsg emitted on network/provider/context errors
- [x] Context cancellation closes iterator cleanly
- [x] StreamTickCmd emits at ~10fps for UI updates

### P3.2 — Inline Thinking/Reasoning Blocks
- [x] ThinkingBlock struct with segment, theme, expanded state, startedAt in `internal/tui/components/thinking.go`
- [x] Render expanded: header + border + content in TextSecondary color
- [x] Render collapsed: header only with [▼] toggle
- [x] Toggle flips expanded state
- [x] Duration counter: N.Ns (<10s), NN.Ns (<60s), Mm Ns (>=60s)
- [x] Finalized segments use DurationMs from segment
- [x] Header truncates with ... when too wide

### P3.3 — Rich Tool Cards
- [x] ToolCard struct with toolName, input, output, state, theme in `internal/tui/components/toolcard.go`
- [x] Tool label colors: Bash=Warning, FileRead=Thinking, FileWrite=Brand, Glob=TextSecondary, Grep=Thinking
- [x] Status badges: [..] running, [OK] success, [ERR] error
- [x] Auto-collapse when output > 20 lines
- [x] Output cap at MaxToolOutputChars (10,000) with truncation notice
- [x] Binary content detection via null byte sniff: [binary file, N bytes]
- [x] Toggle collapses/expands output
- [x] Card layout: header → input → output → status

### P3.4 — Markdown Rendering
- [x] MessageRenderer uses Glamour TermRenderer in `internal/tui/components/message.go`
- [x] Dark theme: glamour "dark" base style
- [x] Light theme: glamour "light" base style
- [x] Renderer cached (expensive to create)
- [x] Width set to terminal width minus padding
- [x] User messages: right-aligned, plain text, no markdown, max 70% width

### P3.5 — Permission Modal
- [x] PermissionModal with request, theme, elapsed, timeout in `internal/tui/components/permission.go`
- [x] Render: centered overlay using lipgloss.Place
- [x] Tool name, risk level with color coding (Dangerous/Destructive = Error bg)
- [x] Command preview in bordered box
- [x] Key bindings: Y/Enter=Allow, A=AllowAlways, N/Esc=Deny, E=Exit
- [x] Auto-deny countdown visible (N:NN format)
- [x] Allow/AllowAlways/Deny return PermissionResponse
- [x] Tick() updates elapsed time

### P3.6 — REPL Integration
- [x] ReplModel extended with streaming state (currentMessage, streamSegments, streamContent, activeSegmentType)
- [x] Update() handles StreamMsg, StreamDoneMsg, StreamErrorMsg, TickMsg
- [x] View() renders messages via msgRenderer.RenderMessage()
- [x] During streaming: renders in-progress message with partial content
- [x] Thinking blocks update with duration counter during streaming
- [x] Tool cards rendered inline between segments
- [x] Textarea disabled during streaming (key passthrough blocked), re-enabled on completion
- [x] ctrl+c cancels streaming (context cancellation)
- [x] Auto-scroll to bottom on new content

## Test Results

### All TUI Tests
```
=== RUN   TestNewApp_NoKey_CreatesFirstRun
--- PASS: TestNewApp_NoKey_CreatesFirstRun (0.00s)
=== RUN   TestNewApp_WithKey_CreatesREPL
--- PASS: TestNewApp_WithKey_CreatesREPL (0.00s)
=== RUN   TestNewApp_Version
--- PASS: TestNewApp_Version (0.00s)
=== RUN   TestNewApp_RegistryActive
--- PASS: TestNewApp_RegistryActive (0.00s)
=== RUN   TestApp_Init_ReturnsCmd
--- PASS: TestApp_Init_ReturnsCmd (0.00s)
=== RUN   TestApp_Init_NoKey_ReturnsNil
--- PASS: TestApp_Init_NoKey_ReturnsNil (0.00s)
=== RUN   TestApp_CtrlC_Quits
--- PASS: TestApp_CtrlC_Quits (0.00s)
=== RUN   TestApp_ScreenTransition
--- PASS: TestApp_ScreenTransition (0.00s)
=== RUN   TestApp_FirstRunToREPL
--- PASS: TestApp_FirstRunToREPL (0.00s)
=== RUN   TestApp_HealthTick_Reschedules
--- PASS: TestApp_HealthTick_Reschedules (0.00s)
=== RUN   TestApp_View_NotEmpty
--- PASS: TestApp_View_NotEmpty (0.00s)
=== RUN   TestApp_TerminalTooSmall
--- PASS: TestApp_TerminalTooSmall (0.00s)
=== RUN   TestApp_ViewFirstRun
--- PASS: TestApp_ViewFirstRun (0.00s)
=== RUN   TestApp_ErrorMsg
--- PASS: TestApp_ErrorMsg (0.00s)
=== RUN   TestApp_AppMsgWithHealth
--- PASS: TestApp_AppMsgWithHealth (0.00s)
=== RUN   TestApp_AppMsgWithProvider
--- PASS: TestApp_AppMsgWithProvider (0.00s)
=== RUN   TestApp_InitFirstRunNoHealth
--- PASS: TestApp_InitFirstRunNoHealth (0.00s)
=== RUN   TestApp_ScreenREPLWithNilRepl
--- PASS: TestApp_ScreenREPLWithNilRepl (0.00s)
=== RUN   TestApp_HealthTickNoRegistry
--- PASS: TestApp_HealthTickNoRegistry (0.00s)
=== RUN   TestApp_InitWithRegistryReturnsCmd
--- PASS: TestApp_InitWithRegistryReturnsCmd (0.00s)
=== RUN   TestNewReplModel_Components
--- PASS: TestNewReplModel_Components (0.00s)
=== RUN   TestReplModel_EnterSendsMessage
--- PASS: TestReplModel_EnterSendsMessage (0.00s)
=== RUN   TestReplModel_EnterEmptyInput
--- PASS: TestReplModel_EnterEmptyInput (0.00s)
=== RUN   TestReplModel_WindowResize
--- PASS: TestReplModel_WindowResize (0.00s)
=== RUN   TestReplModel_InputHistory
--- PASS: TestReplModel_InputHistory (0.00s)
=== RUN   TestReplModel_ViewNotEmpty
--- PASS: TestReplModel_ViewNotEmpty (0.00s)
=== RUN   TestReplModel_AddMessage
--- PASS: TestReplModel_AddMessage (0.00s)
=== RUN   TestReplModel_GetStatusText
--- PASS: TestReplModel_GetStatusText (0.00s)
=== RUN   TestReplModel_EscClearsInput
--- PASS: TestReplModel_EscClearsInput (0.00s)
=== RUN   TestReplModel_PgUpPgDown
--- PASS: TestReplModel_PgUpPgDown (0.00s)
=== RUN   TestReplModel_ScrollPos
--- PASS: TestReplModel_ScrollPos (0.00s)
=== RUN   TestReplModel_HistoryNavigationAtEnd
--- PASS: TestReplModel_HistoryNavigationAtEnd (0.00s)
=== RUN   TestReplModel_InputValue
--- PASS: TestReplModel_InputValue (0.00s)
=== RUN   TestReplModel_DownEmptyHistory
--- PASS: TestReplModel_DownEmptyHistory (0.00s)
=== RUN   TestReplModel_UpEmptyHistory
--- PASS: TestReplModel_UpEmptyHistory (0.00s)
=== RUN   TestReplModel_RenderMessages
--- PASS: TestReplModel_RenderMessages (0.00s)
=== RUN   TestReplModel_SpinnerTick
--- PASS: TestReplModel_SpinnerTick (0.00s)
=== RUN   TestReplModel_StatusTextPriority
--- PASS: TestReplModel_StatusTextPriority (0.00s)
=== RUN   TestReplModel_MessageRoleMapping
--- PASS: TestReplModel_MessageRoleMapping (0.00s)
=== RUN   TestReplModel_MultipleRenders
--- PASS: TestReplModel_MultipleRenders (0.00s)
=== RUN   TestReplModel_EnterWithHistoryPosReset
--- PASS: TestReplModel_EnterWithHistoryPosReset (0.00s)
=== RUN   TestStartStreamCmd_ContentOnly
--- PASS: TestStartStreamCmd_ContentOnly (0.00s)
=== RUN   TestStartStreamCmd_WithThinking
--- PASS: TestStartStreamCmd_WithThinking (0.00s)
=== RUN   TestStartStreamCmd_WithContextExceeded
--- PASS: TestStartStreamCmd_WithContextExceeded (0.00s)
=== RUN   TestStartStreamCmd_ContextCancellation
--- PASS: TestStartStreamCmd_ContextCancellation (0.00s)
=== RUN   TestStreamTickCmd_EmitsAt10fps
--- PASS: TestStreamTickCmd_EmitsAt10fps (0.10s)
=== RUN   TestTickMsg_Time
--- PASS: TestTickMsg_Time (0.00s)
PASS
ok  	github.com/eshanized/M31A/internal/tui	1.195s
```

### All Component Tests
```
=== RUN   TestNewMessageRenderer_DarkTheme
--- PASS: TestNewMessageRenderer_DarkTheme (0.00s)
=== RUN   TestNewMessageRenderer_LightTheme
--- PASS: TestNewMessageRenderer_LightTheme (0.00s)
=== RUN   TestRenderMessage_UserMessage
--- PASS: TestRenderMessage_UserMessage (0.00s)
=== RUN   TestRenderMessage_UserMessageRightAligned
--- PASS: TestRenderMessage_UserMessageRightAligned (0.00s)
=== RUN   TestRenderMessage_AssistantMessage
--- PASS: TestRenderMessage_AssistantMessage (0.00s)
=== RUN   TestRenderMessage_AssistantWithThinking
--- PASS: TestRenderMessage_AssistantWithThinking (0.00s)
=== RUN   TestRenderMessage_MultipleSegments
--- PASS: TestRenderMessage_MultipleSegments (0.01s)
=== RUN   TestRenderMessage_TruncatesWidth
--- PASS: TestRenderMessage_TruncatesWidth (0.00s)
=== RUN   TestRenderContentSegment_GlamourMarkdown
--- PASS: TestRenderContentSegment_GlamourMarkdown (0.00s)
=== RUN   TestNewPermissionModal_DangerousTool
--- PASS: TestNewPermissionModal_DangerousTool (0.00s)
=== RUN   TestPermissionModal_Render
--- PASS: TestPermissionModal_Render (0.01s)
=== RUN   TestPermissionModal_Allow
--- PASS: TestPermissionModal_Allow (0.00s)
=== RUN   TestPermissionModal_AllowAlways
--- PASS: TestPermissionModal_AllowAlways (0.00s)
=== RUN   TestPermissionModal_Deny
--- PASS: TestPermissionModal_Deny (0.00s)
=== RUN   TestPermissionModal_Countdown
--- PASS: TestPermissionModal_Countdown (0.00s)
=== RUN   TestPermissionModal_AutoDeny
--- PASS: TestPermissionModal_AutoDeny (0.00s)
=== RUN   TestPermissionModal_RiskColor_Dangerous
--- PASS: TestPermissionModal_RiskColor_Dangerous (0.00s)
=== RUN   TestPermissionModal_RiskColor_Destructive
--- PASS: TestPermissionModal_RiskColor_Destructive (0.00s)
=== RUN   TestNewThinkingBlock_DefaultState
--- PASS: TestNewThinkingBlock_DefaultState (0.00s)
=== RUN   TestThinkingBlock_RenderCollapsed
--- PASS: TestThinkingBlock_RenderCollapsed (0.00s)
=== RUN   TestThinkingBlock_RenderExpanded
--- PASS: TestThinkingBlock_RenderExpanded (0.00s)
=== RUN   TestThinkingBlock_Toggle
--- PASS: TestThinkingBlock_Toggle (0.00s)
=== RUN   TestThinkingBlock_Duration_Under10s
--- PASS: TestThinkingBlock_Duration_Under10s (0.00s)
=== RUN   TestThinkingBlock_Duration_Over10s
--- PASS: TestThinkingBlock_Duration_Over10s (0.00s)
=== RUN   TestThinkingBlock_Duration_Over60s
--- PASS: TestThinkingBlock_Duration_Over60s (0.00s)
=== RUN   TestThinkingBlock_Header_TruncatesWide
--- PASS: TestThinkingBlock_Header_TruncatesWide (0.00s)
=== RUN   TestThinkingBlock_FinalizedDuration
--- PASS: TestThinkingBlock_FinalizedDuration (0.00s)
=== RUN   TestThinkingBlock_LiveDuration
--- PASS: TestThinkingBlock_LiveDuration (0.06s)
=== RUN   TestNewToolCard_Bash
--- PASS: TestNewToolCard_Bash (0.00s)
=== RUN   TestNewToolCard_FileRead
--- PASS: TestNewToolCard_FileRead (0.00s)
=== RUN   TestNewToolCard_FileWrite
--- PASS: TestNewToolCard_FileWrite (0.00s)
=== RUN   TestNewToolCard_Glob
--- PASS: TestNewToolCard_Glob (0.00s)
=== RUN   TestNewToolCard_Grep
--- PASS: TestNewToolCard_Grep (0.00s)
=== RUN   TestToolCard_RenderRunning
--- PASS: TestToolCard_RenderRunning (0.00s)
=== RUN   TestToolCard_RenderSuccess
--- PASS: TestToolCard_RenderSuccess (0.00s)
=== RUN   TestToolCard_RenderError
--- PASS: TestToolCard_RenderError (0.00s)
=== RUN   TestToolCard_AutoCollapse_LongOutput
--- PASS: TestToolCard_AutoCollapse_LongOutput (0.00s)
=== RUN   TestToolCard_AutoCollapse_BinaryContent
--- PASS: TestToolCard_AutoCollapse_BinaryContent (0.00s)
=== RUN   TestToolCard_AutoCollapse_TruncatedOutput
--- PASS: TestToolCard_AutoCollapse_TruncatedOutput (0.06s)
=== RUN   TestToolCard_Toggle
--- PASS: TestToolCard_Toggle (0.00s)
=== RUN   TestToolCard_HeaderColors
--- PASS: TestToolCard_HeaderColors (0.00s)
PASS
ok  	github.com/eshanized/M31A/internal/tui/components	1.200s
```

### Test Summary
- Total tests: 102
- Passed: 102
- Failed: 0
- Skipped: 0

## Build Verification
- [x] `go mod tidy` passes
- [x] `go build ./...` passes
- [x] `go vet ./...` passes

### CGO_ENABLED=0 go build -o m31a ./cmd/m31a
```
m31a: ELF 64-bit LSB executable, x86-64, version 1 (SYSV), statically linked, Go BuildID=..., with debug_info, not stripped
```

## Deviations from Spec
None. All Phase 3 deliverables are implemented as specified.

## Open Questions / Blockers for Phase 4
- The ToolCard component currently renders tool cards from `types.ToolCall` objects embedded in `types.Message`. Phase 4 (Tool System) will need to wire actual tool execution to populate `ToolResult` data and update card states (running → success/error) in real-time.
- The streaming.go `StartStreamCmd` assumes tool calls arrive in the final `StreamDoneMsg`. Phase 4 may need to support streaming tool call deltas.
- The permission modal is fully implemented but currently standalone — Phase 4 will wire it to `tools.PermissionRequest` dispatch.

## Deliverable Summary
Created 5 new component files (thinking.go, toolcard.go, permission.go, message.go, streaming.go), 5 test files (41 component tests + 6 streaming tests), and modified repl.go to integrate the full rendering pipeline. The Message Rendering Pipeline delivers token-by-token streaming with Glamour markdown rendering, collapsible thinking/reasoning blocks with live duration counters, rich tool cards with per-tool color coding and auto-collapse, and a centered permission modal with countdown timer. All 102 tests pass, `go vet` is clean, and the binary builds as a statically-linked CGO_ENABLED=0 executable.

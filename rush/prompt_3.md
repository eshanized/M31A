# ROLE

You are the **Senior Go Engineer specializing in Terminal UI rendering and streaming**
for M31A. You are executing Phase 3 of a multi-phase build plan. Phases 0, 1, and 2
are complete and verified. The fix cycle (prompt_fix_pre3.md) has resolved all audit
blockers. Your job is to implement the Message Rendering Pipeline.

Do not invent features. Do not add packages not listed. Do not write workflow screens,
tool implementations, or settings UI. You are building the visual rendering layer that
makes M31A look premium — streaming tokens, thinking blocks, tool cards, and markdown.
Deviation = breakage downstream.

---

# PROJECT IDENTITY

M31A is a terminal-based AI coding assistant written in Go 1.22+.
- Module path: github.com/eshanized/M31A
- Binary: single static binary (CGO_ENABLED=0)
- UI: Bubble Tea + Lipgloss + Bubbles + Glamour (Charm stack)

**Phase 0 output:** All interfaces and types defined.
**Phase 1 output:** Provider layer complete (OpenRouter + Zen, SSE parser, cache, reasoning, 40+ tests).
**Phase 2 output:** TUI foundation complete (AppState, theme system, REPL screen, first-run wizard, health ticker, 114+ tests).
**Fix cycle:** Go 1.22 pinned, EstimateCost(modelID, usage) fixed, badges [OR]/[ZEN], cache stale fallback, cache-first FetchModels.

Read these files before starting (they already exist):
- `internal/types/types.go` — Message, MessageSegment, ToolCall, ToolResult, Usage, etc.
- `internal/types/constants.go` — MaxToolOutputChars, MaxFileSize, etc.
- `internal/provider/interface.go` — LLMProvider, ChatRequest, StreamIterator, StreamChunk
- `internal/provider/sse.go` — SSEParser for streaming responses
- `internal/provider/reasoning.go` — ParseSSEChunk for thinking/content detection
- `internal/provider/openrouter/client.go` — ChatCompletionStream implementation
- `internal/tui/app.go` — AppState, screen routing
- `internal/tui/repl.go` — ReplModel with viewport + textarea
- `internal/tui/theme/theme.go` — Theme with ThinkingBlock, ToolCard, ToolLabel styles
- `internal/tui/types.go` — Screen enum, AppMsg, HealthUpdateMsg, etc.

---

# PHASE 3 MISSION

Implement the Message Rendering Pipeline:
1. Streaming message renderer — token-by-token rendering without flicker
2. Inline thinking/reasoning blocks — collapsible thinking segments
3. Rich tool cards — status badges, collapsible output, per-tool color coding
4. Markdown rendering via Glamour for assistant responses
5. Permission modal — centered overlay for dangerous tool approval
6. Integration: wire streaming from provider layer into REPL screen
7. Unit tests for all components

---

# DELIVERABLES

Create EXACTLY these files. No more, no less.

## 1. internal/tui/components/message.go

Message bubble renderer — handles user and assistant messages with proper alignment and styling.

```go
package components

import (
    "github.com/charmbracelet/lipgloss"
    "github.com/charmbracelet/glamour"
    "github.com/eshanized/M31A/internal/types"
    "github.com/eshanized/M31A/internal/tui/theme"
)

// MessageRenderer renders a single message bubble.
type MessageRenderer struct {
    theme    theme.Theme
    renderer *glamour.TermRenderer  // Glamour markdown renderer
}

// NewMessageRenderer creates a renderer with the given theme.
func NewMessageRenderer(t theme.Theme) (*MessageRenderer, error)

// RenderMessage renders a complete message (user or assistant).
// User messages are right-aligned with subtle background tint.
// Assistant messages are left-aligned with full width.
func (r *MessageRenderer) RenderMessage(msg types.Message, width int) string

// renderUserMessage renders a user message bubble (right-aligned).
func (r *MessageRenderer) renderUserMessage(content string, width int) string

// renderAssistantMessage renders an assistant message with thinking blocks and tool cards.
func (r *MessageRenderer) renderAssistantMessage(msg types.Message, width int) string

// renderContentSegment renders a content segment with Glamour markdown.
func (r *MessageRenderer) renderContentSegment(content string, width int) string
```

### Implementation details:

**Glamour integration:**
- Create `glamour.TermRenderer` with a custom style matching the theme
- Dark mode: use glamour's "dark" base style with custom colors
- Light mode: use glamour's "light" base style
- Cache the renderer — it's expensive to create
- Width: set to `width - 4` (account for padding)

**User message:**
- Right-aligned, Surface background tint, 1px padding, 2px border-radius
- Plain text rendering (no markdown — user messages are raw text)
- Max width: 70% of terminal width

**Assistant message:**
- Left-aligned, full terminal width minus padding
- Iterate `msg.Segments` in order:
  - `type == "content"`: render with Glamour markdown
  - `type == "thinking"`: render as thinking block (delegated to ThinkingBlock component)
- Generous padding between segments (1 empty line)

---

## 2. internal/tui/components/thinking.go

Thinking/reasoning block renderer — collapsible blocks within assistant messages.

```go
package components

import (
    "fmt"
    "strings"
    "time"

    "github.com/charmbracelet/lipgloss"
    "github.com/eshanized/M31A/internal/types"
    "github.com/eshanized/M31A/internal/tui/theme"
)

// ThinkingBlock renders a collapsible thinking segment.
type ThinkingBlock struct {
    segment  types.MessageSegment
    theme    theme.Theme
    expanded bool
    startedAt time.Time  // When thinking started (for duration counter)
}

// NewThinkingBlock creates a thinking block from a message segment.
func NewThinkingBlock(segment types.MessageSegment, theme theme.Theme, expanded bool) *ThinkingBlock

// Render renders the thinking block.
// If expanded: shows header + content with left border accent.
// If collapsed: shows only header with "[▼]" toggle indicator.
func (b *ThinkingBlock) Render(width int) string

// Toggle flips the expanded state.
func (b *ThinkingBlock) Toggle()

// IsExpanded returns the current expanded state.
func (b *ThinkingBlock) IsExpanded() bool

// Duration returns the elapsed thinking duration formatted as "N.Ns".
func (b *ThinkingBlock) Duration() string

// Header renders the collapsible header line.
func (b *ThinkingBlock) Header(width int) string
```

### Implementation details:

**Expanded state:**
```
┌──────────────────────────────────────────────────────┐
│ [▲] Thinking (1.2s)                                  │  ← [▲] fg(Thinking)
│ ─────────────────────────────────────────────────────│  ← Border line (Border color)
│ The user wants a Next.js restaurant site. I should   │  ← TextSecondary color
│ first check if there's an existing project...        │
└──────────────────────────────────────────────────────┘
```

**Collapsed state:**
```
┌──────────────────────────────────────────────────────┐
│ [▼] Thinking (1.2s)                                  │  ← [▼] fg(Thinking)
└──────────────────────────────────────────────────────┘
```

**Styling:**
- Background: SurfaceElevated
- Left border: 3px, Thinking color
- Header: `Thinking color` for toggle indicator, `TextSecondary` for duration
- Content: `TextSecondary` color (dimmer than normal text)
- Padding: 1px horizontal, 0px vertical

**Duration counter:**
- Calculate from `startedAt` to `time.Now()`
- Format: "1.2s" for < 10s, "12.3s" for < 60s, "1m 23s" for >= 60s
- For finalized segments (streaming complete), use `segment.DurationMs` instead

**Header format:**
- `[▼]` or `[▲]` toggle indicator
- "Thinking" label
- Duration in parentheses
- Truncate with `...` if content is too wide

---

## 3. internal/tui/components/toolcard.go

Tool execution card — renders tool calls with status badges and collapsible output.

```go
package components

import (
    "fmt"

    "github.com/charmbracelet/lipgloss"
    "github.com/eshanized/M31A/internal/types"
    "github.com/eshanized/M31A/internal/tui/theme"
)

// ToolState represents the execution state of a tool call.
type ToolState int

const (
    ToolRunning ToolState = iota
    ToolSuccess
    ToolError
)

// ToolCard renders a single tool execution card.
type ToolCard struct {
    toolName    string
    input       string          // Command or input text
    output      string          // Tool output
    state       ToolState
    durationMs  int64
    truncated   bool
    theme       theme.Theme
    collapsed   bool            // Auto-collapse if output > 20 lines
}

// NewToolCard creates a tool card from a ToolCall and optional ToolResult.
func NewToolCard(call types.ToolCall, result *types.ToolResult, state ToolState, theme theme.Theme) *ToolCard

// Render renders the tool card.
func (c *ToolCard) Render(width int) string

// Toggle collapses or expands the output.
func (c *ToolCard) Toggle()

// IsCollapsed returns the current collapsed state.
func (c *ToolCard) IsCollapsed() bool

// renderHeader renders the tool label badge + status badge.
func (c *ToolCard) renderHeader(width int) string

// renderInput renders the command/input section.
func (c *ToolCard) renderInput(input string, width int) string

// renderOutput renders the tool output (collapsible).
func (c *ToolCard) renderOutput(output string, width int) string
```

### Implementation details:

**Card layout:**
```
┌─────────────────────────────────────────────────────┐
│  [BASH]                                             │  ← Lipgloss: fg(Warning) bold
├─────────────────────────────────────────────────────┤
│  ls -la                                             │
├─────────────────────────────────────────────────────┤
│  total 128                                          │
│  drwxr-xr-x  12 user user  4096 May 26 08:00 .      │
│  drwxr-xr-x   3 user user  4096 May 26 07:50 ..     │
│  -rw-r--r--   1 user user  2200 May 26 08:00 README │
├─────────────────────────────────────────────────────┤
│  [OK] Completed in 0.12s                            │  ← Lipgloss: fg(Success)
└─────────────────────────────────────────────────────┘
```

**Tool label colors (from spec §6.2):**
| Tool | Color |
|------|-------|
| Bash | Warning (`#FDD663` dark / `#F9AB00` light) |
| FileRead | Thinking (`#8AB4F8` dark / `#1967D2` light) |
| FileWrite | Brand (`#D77757` dark / `#C45C3A` light) |
| Glob | TextSecondary (`#9AA0A6` dark / `#5F6368` light) |
| Grep | Thinking (`#8AB4F8` dark / `#1967D2` light) |

**Status badges:**
- Running: `[..]` with Spinner (fg TextSecondary)
- Success: `[OK]` fg(Success) + "Completed in N.NNs"
- Error: `[ERR]` fg(Error) + error message

**Auto-collapse:**
- If output > 20 lines, collapse by default
- Show `[+N lines]` toggle hint
- Hard cap: truncate output at `MaxToolOutputChars` (10,000) characters
- Truncated output shows `[... output truncated, full output in session log]`
- Binary content: detect via mime sniff or null bytes, show `[binary file, N bytes]`

**Input rendering:**
- Plain text for Bash commands
- For FileRead: show file path
- For FileWrite: show file path + "(atomic write)"
- For Glob: show pattern
- For Grep: show pattern + " in <glob>"

---

## 4. internal/tui/components/permission.go

Permission modal — centered overlay for dangerous tool approval.

```go
package components

import (
    "fmt"
    "time"

    "github.com/charmbracelet/lipgloss"
    "github.com/eshanized/M31A/internal/tools"
    "github.com/eshanized/M31A/internal/tui/theme"
)

// PermissionModal renders the permission request overlay.
type PermissionModal struct {
    request    tools.PermissionRequest
    theme      theme.Theme
    elapsed    time.Duration
    timeout    time.Duration
    responded  bool
    response   tools.PermissionResponse
}

// NewPermissionModal creates a permission modal from a request.
func NewPermissionModal(request tools.PermissionRequest, theme theme.Theme, timeout time.Duration) *PermissionModal

// Render renders the centered modal overlay.
// The background is dimmed (rendered as full-screen with dimmed style).
func (m *PermissionModal) Render(width, height int) string

// Allow responds with "allow once".
func (m *PermissionModal) Allow() tools.PermissionResponse

// AllowAlways responds with "allow and remember".
func (m *PermissionModal) AllowAlways() tools.PermissionResponse

// Deny responds with "deny".
func (m *PermissionModal) Deny() tools.PermissionResponse

// IsResponded returns true if the user has responded.
func (m *PermissionModal) IsResponded() bool

// Tick updates the elapsed time for the countdown display.
func (m *PermissionModal) Tick()

// Remaining returns the time remaining before auto-deny.
func (m *PermissionModal) Remaining() time.Duration
```

### Implementation details:

**Layout:**
```
┌─────────────────────────────────────────────────────────────┐
│                                                             │
│     ┌─────────────────────────────────────────────────┐     │
│     │  [LOCK] Permission Required                     │     │  ← fg(Warning) bold
│     ├─────────────────────────────────────────────────┤     │
│     │                                                 │     │
│     │  Tool:     Bash                                 │     │
│     │  Risk:     [DANGER]                             │     │  ← fg(Error) bold
│     │                                                 │     │
│     │  Command:                                       │     │
│     │  ┌─────────────────────────────────────────┐    │     │
│     │  │ rm -rf node_modules/.cache              │    │     │
│     │  └─────────────────────────────────────────┘    │     │
│     │                                                 │     │
│     │  [Y] Allow Once    [A] Always Allow             │     │
│     │  [N] Deny          [E] Exit M31A                │     │
│     │                                                 │     │
│     │  Auto-deny in 4:59...                           │     │
│     └─────────────────────────────────────────────────┘     │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

**Background dimming:**
- Render the full screen with a darkened background (multiply opacity or use darker color)
- Terminal can't do true frosted glass, so use Surface color with darker border
- Center the modal both horizontally and vertically

**Risk color coding:**
- Safe: TextSecondary (gray) — no modal needed, but label shows
- Medium: Warning (amber)
- Dangerous: Error (red) — requires approval
- Destructive: Error (red) bold — requires approval with extra warning

**Key bindings (when modal is active):**
- `Y` / `Enter`: Allow Once
- `A`: Always Allow (remember choice)
- `N` / `Esc`: Deny
- `E`: Exit M31A (quit immediately)

**Auto-deny countdown:**
- Visible countdown: "4:59..." → "0:01..." → "Denied"
- Default timeout: 300s (5 minutes) from `types` constants or config
- Pause countdown on user activity (key press) — but don't reset

---

## 5. internal/tui/streaming.go

Streaming integration — connects provider SSE stream to REPL message updates.

```go
package tui

import (
    "context"
    "time"

    tea "github.com/charmbracelet/bubbletea"
    "github.com/eshanized/M31A/internal/provider"
    "github.com/eshanized/M31A/internal/types"
)

// StreamMsg is emitted when a streaming chunk arrives.
type StreamMsg struct {
    Chunk      *types.StreamChunk
    ModelID    string
    SessionID  string
}

// StreamDoneMsg is emitted when streaming completes.
type StreamDoneMsg struct {
    Message    types.Message    // Final assembled message with all segments
    Usage      *types.Usage     // Token usage from final chunk
    ModelID    string
    SessionID  string
}

// StreamErrorMsg is emitted when a streaming error occurs.
type StreamErrorMsg struct {
    Err     error
    ModelID string
}

// StartStreamCmd creates a tea.Cmd that starts streaming from the provider.
// It sends StreamMsg chunks during streaming and StreamDoneMsg when complete.
func StartStreamCmd(ctx context.Context, provider provider.LLMProvider,
    req provider.ChatRequest, sessionID string) tea.Cmd

// TickMsg is emitted periodically during streaming for UI updates
// (thinking duration counter, spinner animation).
type TickMsg struct{ Time time.Time }

// StreamTickCmd creates a tea.Cmd that emits TickMsg at ~10fps.
func StreamTickCmd() tea.Cmd
```

### Implementation details:

**StartStreamCmd:**
- Calls `provider.ChatCompletionStream(ctx, req)` to get `*types.StreamIterator`
- Spawns a goroutine that calls `iterator.Next()` in a loop
- Each chunk is sent as `StreamMsg{Chunk: chunk, ModelID: req.Model}`
- When `Next()` returns `io.EOF`, assemble the final message from all segments
  and emit `StreamDoneMsg{Message: assembled, Usage: usage}`
- On error, emit `StreamErrorMsg{Err: err}`
- The goroutine must call `iterator.Close()` on completion or error

**Chunk assembly:**
- Maintain a `[]types.MessageSegment` slice
- When `chunk.Type == "content"`, append to the active content segment
- When `chunk.Type == "thinking"`, create a new thinking segment
- When `chunk.Type == "done"`, finalize the current segment
- The assembled message is built from all segments + flattened content

**StreamTickCmd:**
- Use `tea.Tick(time.Second/10, ...)` for 10fps updates
- Thinking duration counters update on each tick
- Spinner animation advances on each tick

**Error handling:**
- Context cancellation: close iterator, emit `StreamErrorMsg{Err: context.Canceled}`
- Network errors: emit `StreamErrorMsg{Err: err}`
- Provider errors (429, 401, 503): emit `StreamErrorMsg` with appropriate sentinel error

---

## 6. internal/tui/repl.go (MODIFIED)

Modify the existing REPL model to integrate streaming, thinking blocks, and tool cards.

**Changes to `ReplModel`:**

Add fields:
```go
// Streaming state
streaming       bool              // Is the LLM currently streaming?
currentMessage  *types.Message    // Message being streamed (incomplete)
streamSegments  []types.MessageSegment // Segments accumulated during streaming
streamContent   strings.Builder   // Current content segment being built
thinkingStartAt time.Time         // When current thinking segment started
activeSegmentType string          // "content" or "thinking" — current streaming segment

// Thinking blocks
thinkingBlocks  map[int]*components.ThinkingBlock // index → block, for toggle

// Tool cards
toolCards       map[int]*components.ToolCard // tool call index → card

// Message renderer
msgRenderer     *components.MessageRenderer
```

**Modify `Update()` to handle:**
- `StreamMsg`: Append chunk to current streaming state, create/update segments
  - If `chunk.Type == "thinking"`, open new thinking segment, set `thinkingStartAt`
  - If `chunk.Type == "content"` after thinking, close thinking segment, start content segment
  - Update `streamContent` with `chunk.Delta`
  - Return model + no-op cmd (view will re-render)
- `StreamDoneMsg`: Finalize the message, append to `messages`, clear streaming state,
  re-enable textarea
- `StreamErrorMsg`: Clear streaming state, append error message to history,
  re-enable textarea
- `TickMsg`: If streaming, update thinking duration counters
- Tool call detection: When a tool call appears in the stream, render a ToolCard in running state

**Modify `View()` to:**
- Render messages using `msgRenderer.RenderMessage()`
- During streaming, render the in-progress message (segments + current partial content)
- Render thinking blocks with current duration counter
- Render tool cards inline between message segments
- Auto-scroll to bottom when new content arrives
- Disable textarea during streaming, enable on StreamDoneMsg/StreamErrorMsg

**Key bindings during streaming:**
- `ctrl+c`: Cancel streaming (cancel context, close iterator)
- Other keys: normal REPL behavior

---

## 7. internal/tui/components/message_test.go

Tests for message rendering.

```go
package components

import (
    "testing"
)

func TestNewMessageRenderer_DarkTheme(t *testing.T)
func TestNewMessageRenderer_LightTheme(t *testing.T)
func TestRenderMessage_UserMessage(t *testing.T)
func TestRenderMessage_UserMessageRightAligned(t *testing.T)
func TestRenderMessage_AssistantMessage(t *testing.T)
func TestRenderMessage_AssistantWithThinking(t *testing.T)
func TestRenderMessage_MultipleSegments(t *testing.T)
func TestRenderMessage_TruncatesWidth(t *testing.T)
func TestRenderContentSegment_GlamourMarkdown(t *testing.T)
```

---

## 8. internal/tui/components/thinking_test.go

Tests for thinking blocks.

```go
package components

import (
    "testing"
)

func TestNewThinkingBlock_DefaultState(t *testing.T)
func TestThinkingBlock_RenderCollapsed(t *testing.T)
func TestThinkingBlock_RenderExpanded(t *testing.T)
func TestThinkingBlock_Toggle(t *testing.T)
func TestThinkingBlock_Duration_Under10s(t *testing.T)
func TestThinkingBlock_Duration_Over10s(t *testing.T)
func TestThinkingBlock_Duration_Over60s(t *testing.T)
func TestThinkingBlock_Header_TruncatesWide(t *testing.T)
func TestThinkingBlock_FinalizedDuration(t *testing.T)
```

---

## 9. internal/tui/components/toolcard_test.go

Tests for tool cards.

```go
package components

import (
    "testing"
)

func TestNewToolCard_Bash(t *testing.T)
func TestNewToolCard_FileRead(t *testing.T)
func TestNewToolCard_FileWrite(t *testing.T)
func TestNewToolCard_Glob(t *testing.T)
func TestNewToolCard_Grep(t *testing.T)
func TestToolCard_RenderRunning(t *testing.T)
func TestToolCard_RenderSuccess(t *testing.T)
func TestToolCard_RenderError(t *testing.T)
func TestToolCard_AutoCollapse_LongOutput(t *testing.T)
func TestToolCard_AutoCollapse_BinaryContent(t *testing.T)
func TestToolCard_AutoCollapse_TruncatedOutput(t *testing.T)
func TestToolCard_Toggle(t *testing.T)
func TestToolCard_HeaderColors(t *testing.T)
```

---

## 10. internal/tui/components/permission_test.go

Tests for permission modal.

```go
package components

import (
    "testing"
    "time"

    "github.com/eshanized/M31A/internal/tools"
    "github.com/eshanized/M31A/internal/types"
)

func TestNewPermissionModal_DangerousTool(t *testing.T)
func TestPermissionModal_Render(t *testing.T)
func TestPermissionModal_Allow(t *testing.T)
func TestPermissionModal_AllowAlways(t *testing.T)
func TestPermissionModal_Deny(t *testing.T)
func TestPermissionModal_Countdown(t *testing.T)
func TestPermissionModal_AutoDeny(t *testing.T)
func TestPermissionModal_RiskColor_Dangerous(t *testing.T)
func TestPermissionModal_RiskColor_Destructive(t *testing.T)
```

---

## 11. internal/tui/streaming_test.go

Tests for streaming integration.

```go
package tui

import (
    "context"
    "testing"
)

func TestStartStreamCmd_ContentOnly(t *testing.T)
func TestStartStreamCmd_WithThinking(t *testing.T)
func TestStartStreamCmd_WithContextExceeded(t *testing.T)
func TestStartStreamCmd_ContextCancellation(t *testing.T)
func TestStreamTickCmd_EmitsAt10fps(t *testing.T)
```

Use a mock provider that implements `LLMProvider` with a test `StreamIterator`.

---

# EXECUTION ORDER

Execute tasks in this strict order:

1.  Read existing files: all `internal/types/`, `internal/provider/`, `internal/tui/` files
2.  Create `internal/tui/components/thinking.go`
3.  Create `internal/tui/components/toolcard.go`
4.  Create `internal/tui/components/permission.go`
5.  Create `internal/tui/streaming.go`
6.  Create `internal/tui/components/message.go` (depends on thinking.go, toolcard.go)
7.  Modify `internal/tui/repl.go` to integrate streaming, message renderer, thinking blocks, tool cards
8.  Create all test files (message_test.go, thinking_test.go, toolcard_test.go, permission_test.go, streaming_test.go)
9.  Run: `go mod tidy` — may add glamour dependency
10. Run: `go build ./...` — MUST succeed
11. Run: `go vet ./...` — MUST pass
12. Run: `go test -race -count=1 ./internal/tui/...` — MUST pass all tests
13. Run: `CGO_ENABLED=0 go build -o m31a ./cmd/m31a` — MUST produce static binary
14. Create `walkthrough_3.md`

---

# HARD CONSTRAINTS

- `go build ./...` MUST succeed with zero errors
- `go vet ./...` MUST produce zero warnings
- `go test -race ./internal/tui/...` MUST pass all tests
- You MUST NOT implement workflow screens (Phase 6)
- You MUST NOT implement tool execution (Phase 4) — only rendering of tool cards
- You MUST NOT implement model selector UI (Phase 7)
- You MUST NOT implement settings screen (Phase 7)
- Streaming must render token-by-token without flicker (no full re-render on each tick)
- Thinking blocks MUST be collapsible with independent toggle state per segment
- Tool cards MUST use per-tool color coding as specified
- Permission modal MUST center on screen with countdown timer
- Glamour markdown rendering MUST use theme-matched stylesheet
- All Bubble Tea models must properly implement the tea.Model interface
- `walkthrough_3.md` MUST contain actual test output, not placeholder text

---

# WHAT SUCCESS LOOKS LIKE

When you are done, the developer can:
1. Run `go test -race ./internal/tui/...` and see all tests pass
2. Import `internal/tui/components` and render messages, thinking blocks, tool cards, permission modals
3. Stream tokens from a provider — REPL updates token-by-token with no flicker
4. Thinking segments render as collapsible blocks with live duration counters
5. Tool calls render as rich cards with status badges and collapsible output
6. Permission modal appears centered with countdown for dangerous tools
7. Read `walkthrough_3.md` and see actual test output + build verification

---

# WALKTHROUGH TEMPLATE

Generate `walkthrough_3.md` at the project root LAST, after all files are created and verified.

```markdown
# Walkthrough 3 — Message Rendering Pipeline

## Completed Tasks

### P3.1 — Streaming Token Renderer
- [ ] StreamMsg, StreamDoneMsg, StreamErrorMsg message types defined
- [ ] StartStreamCmd spawns goroutine, calls iterator.Next() in loop
- [ ] Chunks sent as StreamMsg via tea.Cmd channel
- [ ] StreamDoneMsg emits final assembled message with all segments
- [ ] StreamErrorMsg emitted on network/provider/context errors
- [ ] Context cancellation closes iterator cleanly
- [ ] StreamTickCmd emits at ~10fps for UI updates

### P3.2 — Inline Thinking/Reasoning Blocks
- [ ] ThinkingBlock struct with segment, theme, expanded state, startedAt
- [ ] Render expanded: header + border + content in TextSecondary color
- [ ] Render collapsed: header only with [▼] toggle
- [ ] Toggle flips expanded state
- [ ] Duration counter: N.Ns (<10s), NN.Ns (<60s), Mm Ns (>=60s)
- [ ] Finalized segments use DurationMs from segment
- [ ] Header truncates with ... when too wide

### P3.3 — Rich Tool Cards
- [ ] ToolCard struct with toolName, input, output, state, theme
- [ ] Tool label colors: Bash=Warning, FileRead=Thinking, FileWrite=Brand, Glob=TextSecondary, Grep=Thinking
- [ ] Status badges: [..] running, [OK] success, [ERR] error
- [ ] Auto-collapse when output > 20 lines
- [ ] Output cap at MaxToolOutputChars (10,000) with truncation notice
- [ ] Binary content detection: [binary file, N bytes]
- [ ] Toggle collapses/expands output
- [ ] Card layout: header → input → output → status

### P3.4 — Markdown Rendering
- [ ] MessageRenderer uses Glamour TermRenderer
- [ ] Dark theme: glamour "dark" base style with custom colors
- [ ] Light theme: glamour "light" base style
- [ ] Renderer cached (expensive to create)
- [ ] Width set to terminal width minus padding
- [ ] User messages: right-aligned, plain text, no markdown

### P3.5 — Permission Modal
- [ ] PermissionModal with request, theme, elapsed, timeout
- [ ] Render: centered overlay with dimmed background
- [ ] Tool name, risk level with color coding (Dangerous/Destructive = red)
- [ ] Command preview in bordered box
- [ ] Key bindings: Y/Enter=Allow, A=AllowAlways, N/Esc=Deny, E=Exit
- [ ] Auto-deny countdown visible (4:59 → 0:01 → Denied)
- [ ] Allow/AllowAlways/Deny return PermissionResponse

### P3.6 — REPL Integration
- [ ] ReplModel extended with streaming state (currentMessage, streamSegments, streamContent)
- [ ] Update() handles StreamMsg, StreamDoneMsg, StreamErrorMsg, TickMsg
- [ ] View() renders messages via msgRenderer.RenderMessage()
- [ ] During streaming: renders in-progress message with partial content
- [ ] Thinking blocks update with live duration counter during streaming
- [ ] Tool cards rendered inline between segments
- [ ] Textarea disabled during streaming, enabled on completion
- [ ] ctrl+c cancels streaming (context cancellation)
- [ ] Auto-scroll to bottom on new content

## Test Results

### All TUI Tests
```
[paste actual output of: go test -race -v ./internal/tui/...]
```

### All Component Tests
```
[paste actual output of: go test -race -v ./internal/tui/components/...]
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
[List any deviation from ROADMAP.md Phase 3 tasks, or write "None"]

## Open Questions / Blockers for Phase 4
[List anything Phase 4 (Tool System) needs to know, or write "None"]

## Deliverable Summary
[3-5 sentences summarizing what was implemented and confirming all Phase 3 deliverables are met.]
```

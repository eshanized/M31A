---
phase: 03
phase_name: Message Rendering Pipeline
type: RESEARCH
created_at: 2026-06-08
---

## RESEARCH COMPLETE

### Phase 3 Implementation Status

Phase 3 (Message Rendering Pipeline) is **partially implemented**. The core streaming architecture exists but has these gaps:

#### ✅ What Works
- **Streaming goroutine** (`streaming.go`): `StartStreamCmd` spawns goroutine, reads SSE chunks, sends typed `tea.Msg` via channel
- **Segment handling** (`repl_stream.go`): `handleStreamMsg` routes chunks by type (content/thinking/done), `closeActiveSegment` finalizes segments, `handleStreamDoneMsg` builds message with segments/thinking blocks/tool cards
- **Thinking blocks** (`components/thinking.go`): `ThinkingBlock` renders collapsible panel with duration, scroll support
- **Tool cards** (`components/toolcard.go`): `ToolCard` renders inline or block style with status/timing/truncation
- **Message rendering** (`components/message.go`): `MessageRenderer` handles user/assistant messages with gutter, Glamour markdown, segment rendering
- **Header** (`header.go`): `RenderHeader` shows brand, phase badge, context meter, model/provider badge
- **Status bar** (`repl_view.go` via `RenderStatusBar`): StatusBarInfo struct with streaming/thinking/spinner/cwd/git branch/tokens/cost

#### 🔴 Unused Functions to Address (53 staticcheck U1000)
- `repl_thinking.go:13`: `renderThinkingToggleHint` — replaced by `components/thinking.go` `ThinkingBlock.Header()`
- `repl_stream.go:215`: `streamTickCmds` — tick logic moved to `StreamTickCmd()` in `streaming.go`
- `helpers.go`: `ensureSidebarModel`, `propagateSessionID`, `applySessionRestored`, `renderSectionHeader`
- `app_view.go`: `renderHeader`, `errorf`, `simpleError`
- `repl.go`: `renderQuickActions`
- `repl_quickactions.go`: entire file (`renderQuickActionsPanel`)
- `repl_welcome.go`: `renderBottomBar`
- `execute_view.go`: `renderProgressBar`, `animatedProgressBarWidth`, `renderTaskSpinner`
- `plan_view.go`: `renderPlanHeader`
- `ship_view.go`: `renderShipStatsGrid`
- `settings_view.go`: `renderSettingCard`, `maskedKey`
- `sidebar.go`: `refreshCmd`, `fileStatusIcon`
- `header.go`: `formatDurationMs` (duplicated by `components/thinking.go:189` `Duration()`)
- `components/truncate.go`: `truncateMiddle`
- `transition.go`: `transitionOverlayWidth` (unused constant)
- `app_channel.go`: entire file (abandoned channel implementation)
- `backup.go`: entire file (async backup implementation never called)
- `workflow/engine_verify.go`: `verifyTaskContext`

#### 🔧 Key Observations
1. `renderThinkingToggleHint` is dead code — hint rendering is built into `ThinkingBlock.Header()`
2. `streamTickCmds` is dead — tick logic is directly in `StreamTickCmd()` 
3. `app_channel.go` and `backup.go` are entire files that can be deleted
4. Many view helpers (`renderQuickActions`, `renderBottomBar`, etc.) were replaced by newer component-based rendering
5. `formatDurationMs` duplicates `ThinkingBlock.Duration()` — consolidate into shared location

### Provider Streaming Normalization

Two patterns must be handled:
1. **Pre-content reasoning** (DeepSeek R1, OpenAI o-series): All thinking tokens arrive before content — current code handles this
2. **Interleaved reasoning** (Claude extended thinking): Alternating thinking/content segments — `handleStreamMsg` handles this via segment type switching

The `StreamIterator.Next()` in `internal/provider/sse.go` detects segment boundaries.

### Architecture Constraints
- Bubble Tea single-threaded — all state in `Update()`
- Theme via `theme.Manager` — no raw hex strings in rendering code
- No CGO — static binary only
- Context pruning per phase

### Files to Modify
| File | Action |
|------|--------|
| `internal/tui/repl_thinking.go` | Remove `renderThinkingToggleHint` (function + callers) |
| `internal/tui/repl_stream.go` | Remove `streamTickCmds` |
| `internal/tui/helpers.go` | Remove `ensureSidebarModel`, `propagateSessionID`, `applySessionRestored`, `renderSectionHeader` |
| `internal/tui/app_view.go` | Remove `renderHeader`, `errorf`, `simpleError` |
| `internal/tui/repl.go` | Remove `renderQuickActions` |
| `internal/tui/repl_quickactions.go` | Delete file |
| `internal/tui/repl_welcome.go` | Remove `renderBottomBar` |
| `internal/tui/execute_view.go` | Remove `renderProgressBar`, `animatedProgressBarWidth`, `renderTaskSpinner` |
| `internal/tui/plan_view.go` | Remove `renderPlanHeader` |
| `internal/tui/ship_view.go` | Remove `renderShipStatsGrid` |
| `internal/tui/settings_view.go` | Remove `renderSettingCard`, `maskedKey` |
| `internal/tui/sidebar.go` | Remove `refreshCmd`, `fileStatusIcon` |
| `internal/tui/header.go` | Remove `formatDurationMs` |
| `internal/tui/components/truncate.go` | Remove `truncateMiddle` |
| `internal/tui/transition.go` | Remove `transitionOverlayWidth` |
| `internal/tui/app_channel.go` | Delete file |
| `internal/tui/backup.go` | Delete file |
| `internal/tui/backup_test.go` | Delete file |
| `internal/workflow/engine_verify.go` | Remove `verifyTaskContext` |

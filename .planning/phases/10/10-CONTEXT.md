# Phase 10: Provider & Message Layer Adaptations — Context

**Gathered:** 2026-06-01
**Status:** Ready for planning
**Source:** `rush/opencode_adaptation_report.md` (adaptation items 1, 3, 4)

<domain>
## Phase Boundary

Phase 10 implements three OpenCode adaptations in M31A's core provider and message handling layers. These are foundational reliability improvements that downstream phases depend on.

**Depends on:** Phase 7 (Signature Features) — all existing packages and interfaces

**Duration:** 2 weeks | **Complexity:** 7/10 | **Milestone:** Structured tool calls from provider responses; session undo restores conversation state; auto-compaction triggers at threshold

### Adaptations

1. **Structured Tool Call Handling** (adoption #4) — Replace fragile regex-based JSON extraction (`parseToolCalls()` in `engine.go`) with native provider API tool call handling. Currently uses `` ```json `` block extraction from streaming text, which is model-dependent and fragile. Must update SSE parser to extract `tool_use` blocks and `function_call` events from provider responses, and carry them as typed `StreamChunk` events.

2. **Session Undo/Revert** (adoption #3) — Complete the partial `/undo` implementation. Currently `/undo` only displays checkpoint info but does NOT restore conversation state. Need `reverted_to` field on messages, actual revert logic, and `/redo` for revert undo.

3. **Context Compaction Auto-Trigger** (adoption #1) — `pkg/autodream/` already has full `Consolidator` with `Consolidate()`, `Pause()`, `Resume()`. Missing: auto-trigger in `internal/workflow/engine.go` when context exceeds 60% threshold. Also add `/compact` alias.
</domain>

<decisions>
## Implementation Decisions

### Structured Tool Call Handling

- Add `ToolCalls []ToolCall` field to `StreamChunk` in `internal/types/types.go`
- Update `ParseSSEChunk()` in `internal/provider/reasoning.go` to extract `tool_use` blocks (Anthropic format) and `function_call` (OpenAI format) from SSE delta events
- Both OpenRouter and Zen clients use OpenAI-compatible streaming format — `delta.tool_calls` array in SSE chunks
- After structured extraction, remove `parseToolCalls()`, `extractJSONObject()`, `stripCodeBlocks()` from `internal/workflow/engine.go`
- Keep `normalizeToolName()` as it handles provider-specific naming differences
- Engine's `processStream()` must be updated to pipe `StreamChunk.ToolCalls` directly to the dispatcher instead of going through text parsing

### Session Undo/Revert

- Add `RevertedTo *int` field to `Message` in `internal/types/types.go` — points to message index where undo was applied
- `Session` struct gets a `reverted_at` timestamp in `session.json`
- `/undo` implementation: parses `RevertedTo` from checkpoint, truncates `messages.json` to that point, sets `RevertedTo` marker, writes updated state
- `/redo` implementation: reads `reverted_at` from session, restores truncated messages from backup (keep a `messages.json.bak` before truncation)
- Checkpoint system already exists (`pkg/session/checkpoint.go`) — leverage `LatestCheckpoint()` for current state
- Keep last 2 checkpoints; after undo, create new checkpoint of the reverted state

### Context Compaction Auto-Trigger

- Workflow engine's `processStream()` checks `Consolidator.CanConsolidate()` after each completed message pair
- Trigger when context > 60% threshold (existing `AutoDreamThreshold = 0.60`)
- Auto-consolidation runs as a `tea.Cmd` to stay in Bubble Tea's single-threaded model
- Never auto-compact during tool execution — `Consolidator.IsPaused()` already handles this
- Add `/compact` as alias for `/compress` in command registry
</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Files to Modify

- `internal/types/types.go` — Message, StreamChunk structs
- `internal/provider/reasoning.go` — ParseSSEChunk function
- `internal/provider/interface.go` — StreamChunk type
- `internal/provider/openrouter/client.go` — streaming iterator
- `internal/provider/zen/client.go` — streaming iterator
- `internal/workflow/engine.go` — parseToolCalls, processStream, auto-compact
- `internal/tui/commands.go` — /undo, /redo, /compact
- `pkg/session/session.go` — Session struct fields
- `pkg/session/manager.go` — RevertSession, RedoSession
- `pkg/autodream/autodream.go` — already exists, minor tweaks if needed

### Existing Code to Study

- `internal/workflow/engine.go` lines 590-700 — parseToolCalls, extractJSONObject, stripCodeBlocks
- `internal/provider/reasoning.go` lines 100-180 — ParseSSEChunk
- `pkg/session/checkpoint.go` — Checkpoint struct, LatestCheckpoint()
- `internal/tui/commands.go` lines 340-370 — handleUndo, handleCompress
- `pkg/autodream/autodream.go` — Consolidate(), CanConsolidate(), Pause(), Resume()
- `internal/types/types.go` — StreamChunk, Message, ToolCall structs
- `docs/INTERFACES.md` — All interface definitions
</canonical_refs>

<specifics>
## Wave Structure

**Wave 1** (no dependencies, fully parallel):
- Plan 01: Structured Tool Call Handling
- Plan 02: Session Undo/Revert Completion

**Wave 2** (depends on Wave 1 patterns):
- Plan 03: Context Compaction Auto-Trigger

### Key Patterns to Follow

- StreamChunk events drive progressive rendering — do not break existing streaming flow
- All state mutations through Update() only — never from goroutines
- Atomic file writes for all session state changes (temp file + rename)
- Existing test patterns from Phase 7 for commands, session, and provider tests
</specifics>

<deferred>
## Deferred Ideas

- MCP Integration (adoption #5) — explicitly excluded from this wave
- Plugin/Extensibility System (adoption #6) — explicitly excluded
- Full reverted_to history browsing — just basic undo/redo for now
</deferred>

---

*Phase: 10-provider-message-layer-adaptations*
*Context gathered: 2026-06-01 via OpenCode Adaptation Report*

# Phase 13: Infrastructure & Sharing Adaptations — Context

**Gathered:** 2026-06-01
**Status:** Ready for planning
**Source:** `rush/opencode_adaptation_report.md` (adaptation items 15, 16)

<domain>
## Phase Boundary

Phase 13 implements two OpenCode adaptations focused on infrastructure improvements — session sharing/export and an internal event bus for decoupled communication.

**Depends on:** Phase 12 (UX & Editor Experience Adaptations)

**Duration:** 1.5 weeks | **Complexity:** 5/10 | **Milestone:** Session export produces shareable markdown; pub/sub decouples internal events

### Adaptations

1. **Session Sharing/Export** (adoption #15) — Export session history to markdown or HTML for sharing. Lower priority for standalone CLI but enables collaboration workflows.

2. **Bus/PubSub Event System** (adoption #16) — Lightweight typed event bus to decouple M31A's internal communication. Currently uses direct `tea.Cmd`/`tea.Msg` channel patterns which creates tight coupling between components.
</domain>

<decisions>
## Implementation Decisions

### Session Sharing/Export

- `pkg/session/export.go` with exported `FormatSession()` function:
  - `FormatSessionAsMarkdown(session, messages) (string, error)` — produces markdown with session metadata + message history
  - `FormatSessionAsHTML(session, messages) (string, error)` — produces basic HTML with embedded styling
- `/export` command: accepts optional `--format markdown|html` (default markdown) and `--output <file>` (default stdout)
- Markdown format: H1 title (session goal), metadata table (model, provider, duration, token count), then each message as quote block with role header
- HTML format: basic page with `<pre>` blocks for messages, inline CSS
- Session directory path: `~/.m31a/sessions/<id>/export/` — exports cached there
- File size cap: 5MB (same as MaxFileSize)

### Bus/PubSub Event System

- `internal/bus/bus.go` — lightweight, no external dependencies:
  - `EventType` string type for typed event routing
  - `Event` struct: `Type EventType`, `Data interface{}`, `Timestamp time.Time`
  - `Bus` struct with:
    - `Subscribe(eventType EventType, ch chan<- Event)` — subscribe typed channel
    - `Publish(event Event)` — non-blocking publish to all matching subscribers
    - `Unsubscribe(eventType EventType, ch chan<- Event)` — remove subscription
    - `Close()` — close all channels
  - Wildcard subscription: `Subscribe("*", ch)` receives all events
- Initial event types: `SessionChanged`, `ToolExecuted`, `PhaseTransitioned`, `ModelSwitched`, `ProviderSwitched`, `MessageAdded`, `ContextConsolidated`
- Integration: phases opt-in to publish/receive bus events
- NOT replacing tea.Cmd/Msg pattern entirely — bus supplements it for cross-component communication where direct channel references are currently used
- Channel buffer size: 100 events (non-blocking for fast publishers)
- Thread-safe: RWMutex for subscriber map

### Bus Integration Points (Phase 13 scope)

- Phase transitions publish `PhaseTransitioned` events
- Model/provider switches publish `ModelSwitched`/`ProviderSwitched` events  
- Session saves publish `SessionChanged` events
- Tool executions publish `ToolExecuted` events
- These events enable loose coupling between the TUI components and backend subsystems
</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Files to Create/Modify

- `pkg/session/export.go` — session export (create)
- `internal/bus/bus.go` — event bus (create)
- `internal/tui/commands.go` — /export command
- `internal/workflow/engine.go` — publish phase transition events
- `internal/tui/app.go` — subscribe to bus events (optional initial integration)
- `internal/provider/registry.go` — publish model/provider switch events

### Existing Code to Study

- `internal/tui/app.go` — AppState, existing channel patterns (dispatcher.RequestCh, msgChan)
- `internal/workflow/engine.go` — Transition(), phase lifecycle
- `internal/provider/registry.go` — SetActive(), ActiveProvider()
- `internal/tools/dispatcher.go` — Execute(), tool call lifecycle
- `pkg/session/manager.go` — SaveSession(), session lifecycle
- `internal/tui/commands.go` — command pattern for /export
</canonical_refs>

<specifics>
## Wave Structure

**Wave 1** (no dependencies, fully parallel):
- Plan 01: Session Sharing/Export
- Plan 02: Bus/PubSub Event System

### Key Patterns to Follow

- No external dependencies for bus — pure Go channels and sync primitives
- Non-blocking publish with buffered channels
- Existing command pattern for /export
- Atomic file writes for export files
- Thread safety with RWMutex
</specifics>

<deferred>
## Deferred Ideas

- URL-based session sharing (would require server component)
- Secure share links with TTL
- Bus event persistence and replay
- Remote bus subscribers (cross-process events)
- Full migration from tea.Cmd pattern to bus (Phase 13 only establishes the bus, doesn't migrate all existing patterns)
- Plugin hook system on bus events

---

*Phase: 13-infrastructure-sharing-adaptations*
*Context gathered: 2026-06-01 via OpenCode Adaptation Report*

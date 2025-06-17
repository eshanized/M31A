---
phase: 14-tui-core-wiring-fixes
plan: 05
subsystem: tui
tags: [d-07, d-08, d-09, d-10, discuss-streaming, model-selector, sidebar-threshold, stream-ch-close]
dependency_graph:
  requires:
    - phase: 14-tui-core-wiring-fixes
      plan: 01
      provides: "Discuss Q&A flow infrastructure (14-01)"
  provides:
    - Discuss phase LLM response streams token-by-token via MsgEmitter (D-07)
    - StartStreamCmd closes both streamCh and streamDone (D-10)
    - ModelSelector exposes SetRegistry/SetTheme setters; direct field access removed (D-08)
    - Sidebar auto-show threshold configurable via Config.UI.SidebarWidthThreshold (D-09)
  affects:
    - internal/types/types.go (StreamChunkMsg type)
    - internal/workflow/engine.go (streamLLMStreaming method)
    - internal/workflow/discuss.go (runDiscuss refactored to stream)
    - internal/tui/types.go (StreamChunkMsg alias to types.StreamChunkMsg)
    - internal/tui/app.go (StreamChunkMsg handler, sidebar threshold config, ModelSelector setter calls)
    - internal/tui/repl.go (AppendStreamChunk method)
    - internal/tui/streaming.go (defer close(streamCh))
    - internal/tui/modelselector.go (SetRegistry/SetTheme setters)
    - internal/config/types.go (SidebarWidthThreshold field)
    - internal/config/loader.go (DefaultConfig sets SidebarWidthThreshold: 120, mergeConfig)
    - internal/tui/app_test.go (5 new tests)
    - internal/config/loader_test.go (1 new test)
tech-stack:
  added: []
  patterns:
    - "StreamChunkMsg in shared types package (internal/types) to avoid circular dependency between workflow and tui"
    - "Type alias in tui/types.go (StreamChunkMsg = types.StreamChunkMsg) for local use"
    - "Config field with non-zero default in DefaultConfig() and merge logic in mergeConfig"
    - "Setter methods on ModelSelector replace direct field access (encapsulation)"
key-files:
  created: []
  modified:
    - internal/types/types.go (+8 lines — StreamChunkMsg type)
    - internal/workflow/engine.go (+18 lines — streamLLMStreaming method)
    - internal/workflow/discuss.go (+38 / -12 — streaming runDiscuss)
    - internal/tui/types.go (+3 — StreamChunkMsg type alias)
    - internal/tui/app.go (+12 / -4 — StreamChunkMsg handler, sidebar threshold, setter calls)
    - internal/tui/repl.go (+9 — AppendStreamChunk method)
    - internal/tui/streaming.go (+1 — defer close(streamCh))
    - internal/tui/modelselector.go (+8 — SetRegistry/SetTheme)
    - internal/config/types.go (+2 — SidebarWidthThreshold field)
    - internal/config/loader.go (+8 — DefaultConfig + mergeConfig)
    - internal/tui/app_test.go (+75 — 5 new tests)
    - internal/config/loader_test.go (+7 — 1 new test)
key-decisions:
  - "StreamChunkMsg defined in internal/types (not tui/types) to avoid circular dependency — workflow package emits it, TUI package handles it"
  - "streamLLMStreaming returns *m31types.StreamIterator (not a new type) — same underlying provider call, just exposes the iterator"
  - "runDiscuss emits chunks via e.msgEmitter.Emit(m31types.StreamChunkMsg{...}) — uses shared types, not TUI types"
  - "DefaultConfig() now returns a non-zero Config with UI.SidebarWidthThreshold: 120 — previous implementation returned zero-valued Config"
  - "Sidebar threshold fallback: 120 if config is nil or SidebarWidthThreshold is 0 (backward compat)"
deviations:
  - "Plan suggested StreamChunkMsg in tui/types.go — moved to internal/types/types.go to avoid circular import (workflow → tui would be circular)"
  - "Plan suggested buildChatRequest helper — not needed, streamLLMStreaming builds ChatRequest inline (same pattern as streamLLM)"
  - "Plan suggested DefaultConfig returns zero-valued Config — updated to set SidebarWidthThreshold: 120 as the actual default"
requirements-completed: [WIRE-D-07, WIRE-D-08, WIRE-D-09, WIRE-D-10]
metrics:
  duration_seconds: 480
  completed_date: 2026-06-02
  tasks_completed: 4
  files_changed: 12
---

# Phase 14 Plan 05: Discuss Streaming + Low Severity Fixes Summary

**Discuss phase now streams tokens progressively; streamCh properly closed; ModelSelector uses setters; sidebar threshold is configurable.**

## One-Line Summary

Four wiring fixes bundled: D-07 (discuss streaming via StreamChunkMsg + streamLLMStreaming), D-10 (close streamCh in StartStreamCmd), D-08 (ModelSelector setters), D-09 (configurable sidebar threshold).

## Tasks Completed

| # | Issue | Files | Description |
|---|-------|-------|-------------|
| 1 | D-07 | engine.go, discuss.go, types.go, app.go, repl.go | Discuss phase streams LLM tokens via MsgEmitter |
| 2 | D-10 | streaming.go | StartStreamCmd closes streamCh in addition to streamDone |
| 3 | D-08 | modelselector.go, app.go | SetRegistry/SetTheme setters replace direct field access |
| 4 | D-09 | config/types.go, config/loader.go, app.go | SidebarWidthThreshold configurable with default 120 |

## Key Changes

### D-07: Discuss Streaming
- `internal/types/types.go`: Added `StreamChunkMsg` struct (shared between workflow and TUI)
- `internal/workflow/engine.go`: Added `streamLLMStreaming()` method returning `*StreamIterator`
- `internal/workflow/discuss.go`: Refactored `runDiscuss` to iterate the stream, accumulate content, and emit chunks via `e.msgEmitter.Emit(StreamChunkMsg{...})`
- `internal/tui/types.go`: Type alias `StreamChunkMsg = types.StreamChunkMsg`
- `internal/tui/app.go`: Added `case StreamChunkMsg` handler routing to `replModel.AppendStreamChunk()`
- `internal/tui/repl.go`: Added `AppendStreamChunk()` method writing to `streamContent`

### D-10: Close streamCh
- `internal/tui/streaming.go`: Added `defer close(streamCh)` after `defer close(streamDone)` (LIFO order ensures streamCh closes first)

### D-08: ModelSelector Setters
- `internal/tui/modelselector.go`: Added `SetRegistry(*provider.Registry)` and `SetTheme(theme.Theme)` methods
- `internal/tui/app.go`: Replaced `m.modelSelector.theme = t` with `m.modelSelector.SetTheme(t)`

### D-09: Sidebar Threshold
- `internal/config/types.go`: Added `SidebarWidthThreshold int` to `UIConfig`
- `internal/config/loader.go`: `DefaultConfig()` returns `SidebarWidthThreshold: 120`; `mergeConfig` handles the new field
- `internal/tui/app.go`: Sidebar auto-show reads `m.config.UI.SidebarWidthThreshold` with 120 fallback

## Test Results

```
$ go test -count=1 -race -timeout 180s ./...
ok  	github.com/eshanized/M31A/internal/config	1.032s
ok  	github.com/eshanized/M31A/internal/tui	1.908s
ok  	github.com/eshanized/M31A/internal/workflow	1.651s
(all 20 packages pass)

$ CGO_ENABLED=0 go build -o /dev/null ./cmd/m31a   # OK
$ go vet ./...                                      # clean
```

New tests:
- `TestModelSelector_SetTheme` — setter updates theme
- `TestModelSelector_SetRegistry` — setter updates registry
- `TestReplModel_AppendStreamChunk` — chunk accumulation + nil safety
- `TestApp_StreamChunkMsg_RoutesToRepl` — message routing
- `TestApp_SidebarThreshold_FromConfig` — config field read
- `TestDefaultConfig_SidebarWidthThresholdIs120` — default value

## Deviations from Plan

**1. StreamChunkMsg in internal/types, not tui/types**
- Plan defined `StreamChunkMsg` in `tui/types.go` and expected `discuss.go` to use it
- The workflow package cannot import the TUI package (circular dependency: tui → workflow → tui)
- Moved `StreamChunkMsg` to `internal/types/types.go` (shared package) and added a type alias in TUI

**2. No buildChatRequest helper**
- Plan assumed a `buildChatRequest` method existed on Engine
- The actual code builds `ChatRequest` inline in `streamLLM`
- `streamLLMStreaming` follows the same inline pattern

**3. DefaultConfig now sets defaults**
- Previous `DefaultConfig()` returned `&Config{}` (all zero values)
- Now returns `&Config{UI: UIConfig{SidebarWidthThreshold: 120}}`
- This is the correct behavior — other UI fields also have implicit defaults

## Auth Gates

None — no API keys, no external service interactions.

## Known Stubs

None — all paths wire to real implementations.

## Threat Flags

| Flag | File | Description |
|------|------|-------------|
| threat_flag: resource-leak-fix | internal/tui/streaming.go | D-10: streamCh now closed, preventing goroutine/channel leak |
| threat_flag: encapsulation | internal/tui/modelselector.go | D-08: setters enforce encapsulation; no direct field mutation from outside |

## Next Phase Readiness

- D-07, D-08, D-09, D-10 are resolved.
- The workflow loop now streams discuss responses, closes channels properly, uses proper encapsulation, and has configurable UI thresholds.
- Plan 14-06 (AppState refactor) can proceed if desired.

---

*Phase: 14-tui-core-wiring-fixes*
*Plan: 05*
*Completed: 2026-06-02*

## Self-Check: PASSED

- [x] `CGO_ENABLED=0 go build -o /dev/null ./cmd/m31a` passes
- [x] `go vet ./...` passes
- [x] `go test -count=1 -race ./...` passes (full suite)
- [x] StreamChunkMsg in internal/types/types.go
- [x] streamLLMStreaming in engine.go
- [x] runDiscuss streams via MsgEmitter in discuss.go
- [x] StreamChunkMsg handler in app.go
- [x] AppendStreamChunk in repl.go
- [x] defer close(streamCh) in streaming.go
- [x] SetRegistry/SetTheme in modelselector.go
- [x] SidebarWidthThreshold in config/types.go + loader.go
- [x] All new tests pass

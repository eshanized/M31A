# Internal Wiring and Logic Audit Report

**Date:** 2026-06-04
**Target:** `internal/` and `pkg/` directories of M31A codebase
**Objective:** Deep study to identify internal wiring issues, interface mismatches, dependency injection flaws, state management errors, and logical bugs.

## Executive Summary
The M31A codebase exhibits several critical wiring and logical flaws that impact its stability, resource tracking, and functionality. The primary issues stem from significant synchronization gaps between the TUI (Bubble Tea) state and the backend Workflow Engine. These disconnections affect session management, per-phase configuration overrides, and proper token usage reporting. Fixing these requires implementing strict message-based state updates, centralizing session management, and ensuring the Workflow Engine is reactively synchronized with TUI state changes.

---

## Detailed Findings

### 1. Token Usage Tracking Reset (Critical Bug)
- **File:** `internal/tui/streaming.go`
- **Symbols:** `StartStreamCmd`
- **Issue:** The streaming pipeline contains a critical bug where token usage is ignored or reset during stream processing. The `Usage` field is not correctly aggregated or passed back to the `AppState`, leading to 0-usage reporting for all streamed LLM responses. This invalidates token counting and context estimation.

### 2. TUI State Concurrency Violation
- **File:** `internal/tui/sidebar.go`
- **Symbols:** `refreshCmd`
- **Issue:** Implements a dangerous race condition by modifying Bubble Tea model fields directly from a `tea.Cmd` goroutine instead of returning a `tea.Msg` to be handled by the single-threaded `Update()` loop. This violates Bubble Tea's architectural rules and can cause erratic UI behavior and panic under load.

### 3. Ignored Per-Phase Model Configuration (Wiring Defect)
- **File:** `internal/workflow/engine.go`
- **Symbols:** `Engine.streamLLM`, `Engine.streamLLMStreaming`, `Engine.modelForPhase`
- **Issue:** While a `modelForPhase` method exists to honor `config.toml` overrides for different workflow phases, the engine actively ignores it. The methods `streamLLM` and `streamLLMStreaming` hardcode `req.Model = e.modelID` (the default), causing the AI agent phase configuration to be entirely bypassed during runtime.

### 4. TUI / Engine Synchronization Failure
- **File:** `internal/tui/app_update.go`
- **Symbols:** `Update` (`ScreenResume` and `handleAppMsg` cases)
- **Issue:** When a user selects a different model or resumes a session from the UI, the changes only update the local `AppState` in the TUI but fail to synchronize back to the workflow engine and tool dispatcher. Consequently, the TUI displays the new model or session state, but backend operations proceed using stale dependencies.

### 5. Resumable Workflow Deadlock
- **File:** `internal/tui/app.go`
- **Symbols:** `NewApp`
- **Issue:** The TUI initializes a fresh default session *before* it checks for resumable workflows. Because the active session state is already locked to this fresh instance, cross-restart workflow resumption becomes practically impossible, breaking the functionality of the "resumable" toast mechanism.

### 6. Redundant Phase Transition Logic
- **File:** `internal/workflow/initialize.go`
- **Symbols:** `runInitialize`
- **Issue:** There is redundant phase transition logic embedded within the workflow engine that overlaps significantly with TUI-level state management. This creates fragile pathing where the UI and the backend can get out of sync regarding the active operational phase.

---

## Actionable Recommendations

1. **Strict Message Passing:** Refactor `internal/tui/sidebar.go` to emit `tea.Msg` events rather than directly mutating model state in goroutines.
2. **Synchronize Backend Engines:** In `internal/tui/app_update.go`, implement explicit sync functions (or engine re-instantiation) whenever `ScreenResume` or `ModelSelected` occurs, ensuring the Workflow engine gets the new SessionID and ModelID.
3. **Apply Phase Overrides:** Modify `Engine.streamLLM` and `Engine.streamLLMStreaming` in `internal/workflow/engine.go` to use `e.modelForPhase(e.currentPhase)` instead of `e.modelID`.
4. **Fix Streaming Analytics:** Overhaul `StartStreamCmd` in `internal/tui/streaming.go` to capture the final chunk containing usage statistics and emit an aggregated usage message to the TUI's `Update()` loop.
5. **Defer Session Initialization:** Modify `NewApp` in `internal/tui/app.go` to delay session creation until after the resumption check, or provide a clean method to swap the initial session out without leaving dangling state.
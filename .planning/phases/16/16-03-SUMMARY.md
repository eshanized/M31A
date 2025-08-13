---
phase: 16
plan: 03
subsystem: tui
tags: [feedback, loading, spinners, progress, visibility]
completed: 2026-06-03
commits:
  - 46f3478 feat(16-03): add spinners to all loading states
  - dcaa386 feat(16-03): add streaming progress during task execution
  - ae3ddbf feat(16-03): add self-heal visibility
  - 41322b7 feat(16-03): add phase transition messages
  - 425e9b6 feat(16-03): add intermediate progress during long operations
  - 3798a53 feat(16-03): add bash process kill feedback
  - 4097417 feat(16-03): add thinking indicator during operations
  - 0f61760 feat(16-03): fix grep truncation to be visible
  - e810019 feat(16-03): add context warning remaining tokens
  - 7e92eb0 feat(16-03): add model cache refreshing indicator
  - 64c04fa feat(16-03): add permission timeout visual urgency and configurable timeout
  - 086cd99 feat(16-03): add question tool timeout warning display
---

# PLAN-03: Feedback & Loading Fixes — Summary

## What Was Built

Added consistent loading states, streaming progress indicators, and phase transition visibility across all TUI screens. Users now see animated spinners during loading, real-time progress during long operations, and clear feedback for permission timeouts and question tool timeouts.

## Changes Made

### Spinners & Loading States (Task 1)
- All loading states in plan, execute, verify, ship, settings, resume, and sidebar screens now show animated spinners instead of static "Loading..." text
- Spinner tick rate synchronized across screens

### Streaming Progress (Task 2)
- Task execution shows per-tool-call progress updates
- Streaming responses display progressive token count

### Self-Heal Visibility (Task 3)
- Self-heal attempts show attempt count (e.g., "Heal attempt 1/2")

### Phase Transition Messages (Task 4)
- Clear messages when transitioning between workflow phases

### Intermediate Progress (Task 5)
- Long operations show intermediate progress updates

### Bash Kill Feedback (Task 6)
- Process termination shows visible feedback to user

### Thinking Indicator (Task 7)
- LLM processing shows "Thinking..." indicator during operations

### Grep Truncation (Task 8)
- Truncated grep output is now visible with suggestion to narrow search

### Context Warning (Task 9)
- Context window warnings show remaining token count

### Model Cache (Task 10)
- Model cache refresh shows spinning indicator during background refresh

### Permission Timeout (Task 11)
- Permission modal shows countdown: "Auto-deny in M:SS..."
- Color changes from warning (yellow) to error (red) at 30s remaining
- Shows "Tool will be rejected" when countdown expires
- Backend timeout context enforced (configurable via config)

### Question Timeout (Task 12)
- Question tool displays timeout hint: "Timeout: Ns — no response uses default"
- QuestionRequestMsg carries TimeoutSecs from tool to TUI

## Verification

- [x] All loading states show animated spinners
- [x] Streaming progress shows per-tool-call updates
- [x] Self-heal attempts visible with attempt counts
- [x] Phase transitions show clear messages
- [x] Long operations show intermediate progress
- [x] Process termination is visible
- [x] Thinking indicator appears during LLM processing
- [x] Grep truncation visible with suggestions
- [x] Context warnings show remaining tokens
- [x] Permission timeout visible with countdown and urgency color
- [x] Question tool timeout visible to user
- [x] `go build ./...` passes
- [x] `go vet ./...` passes

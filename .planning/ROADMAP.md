# Roadmap: M31A UI Refactor

## Overview

M31A has a solid Elm architecture foundation with Bubble Tea, but two critical wiring issues prevent users from interacting with the product: a blank REPL screen (no messages, no keyboard input) and hardcoded model names throughout the codebase. This refactor is surgical — both fixes are bounded to 4-6 files each, with core infrastructure already in place. No new dependencies needed. Phase 1 fixes the REPL rendering pipeline so users can interact with the AI assistant. Phase 2 cleans up hardcoded model references and wires dynamic model selection to the working REPL.

## Phases

- [ ] **Phase 1: Fix REPL Screen** - Viewport displays messages, textarea accepts input, streaming works
- [ ] **Phase 2: Dynamic Model Selection** - Models fetched from providers, selection persists, providers wire correctly

## Phase Details

### Phase 1: Fix REPL Screen
**Goal**: Users can interact with the AI assistant through a working REPL
**Depends on**: Nothing (first phase)
**Requirements**: REPL-01, REPL-02, REPL-03, REPL-04, REPL-05, REPL-06
**Success Criteria** (what must be TRUE):
  1. User can see messages appear in the REPL viewport when they arrive
  2. User can type in the textarea and send messages
  3. User can see streaming responses display in real-time
  4. User can see error messages when they occur
  5. User sees a welcome screen when no messages exist
  6. User can see current model and provider in the status bar
**Plans**: TBD
**UI hint**: yes

### Phase 2: Dynamic Model Selection
**Goal**: Model selection is dynamic and configurable instead of hardcoded
**Depends on**: Phase 1
**Requirements**: MODEL-01, MODEL-02, MODEL-03, MODEL-04, PROV-01, PROV-02, PROV-03
**Success Criteria** (what must be TRUE):
  1. Model selector displays real models fetched from provider APIs
  2. Selecting a model changes the active model used for LLM calls
  3. Selected model persists when the application restarts
  4. Provider registration works correctly for all 3 providers
  5. Model fetching completes before the REPL renders
**Plans**: TBD
**UI hint**: yes

## Progress

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Fix REPL Screen | 0/TBD | Not started | - |
| 2. Dynamic Model Selection | 0/TBD | Not started | - |

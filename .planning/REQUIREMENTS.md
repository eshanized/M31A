# Requirements: M31A UI Refactor

**Defined:** 2025-07-31
**Core Value:** Fix the REPL screen so users can interact with the AI assistant, and make model selection dynamic and configurable.

## v1 Requirements

Requirements for initial release. Each maps to roadmap phases.

### REPL Display

- [ ] **REPL-01**: Viewport displays messages when they arrive
- [ ] **REPL-02**: Textarea accepts keyboard input and sends messages
- [ ] **REPL-03**: Streaming responses display in viewport in real-time
- [ ] **REPL-04**: Error messages are visible in REPL
- [ ] **REPL-05**: Welcome screen displays when no messages exist
- [ ] **REPL-06**: Status bar shows current model and provider

### Model Selection

- [ ] **MODEL-01**: Models are dynamically fetched from providers
- [ ] **MODEL-02**: Model selection updates the active model used for LLM calls
- [ ] **MODEL-03**: Model selector UI displays available models from all providers
- [ ] **MODEL-04**: Selected model persists across sessions

### Provider Integration

- [ ] **PROV-01**: Provider registration works correctly for all 3 providers
- [ ] **PROV-02**: Model fetching completes before REPL renders
- [ ] **PROV-03**: Provider health checks work correctly

## v2 Requirements

Deferred to future release. Tracked but not in current roadmap.

### Polish

- **POL-01**: Model pricing display in status bar
- **POL-02**: Context meter showing token usage
- **POL-03**: Auto-arbitrage for cost optimization
- **POL-04**: Model capability detection (reasoning, tools, completion)

### Testing

- **TEST-01**: Replace hardcoded model names in test files
- **TEST-02**: Add integration tests for dynamic model fetching
- **TEST-03**: Add E2E tests for REPL interaction

## Out of Scope

| Feature | Reason |
|---------|--------|
| Full UI redesign | Keep existing patterns, focus on fixing bugs |
| New commands | Fix existing functionality first |
| Plugin system | Not core to current requirements |
| Multi-window | Out of scope for this refactor |
| Bubble Tea v2 | Not stable, existing v1.3.0 is correct |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| REPL-01 | Phase 1 | Pending |
| REPL-02 | Phase 1 | Pending |
| REPL-03 | Phase 1 | Pending |
| REPL-04 | Phase 1 | Pending |
| REPL-05 | Phase 1 | Pending |
| REPL-06 | Phase 1 | Pending |
| MODEL-01 | Phase 2 | Pending |
| MODEL-02 | Phase 2 | Pending |
| MODEL-03 | Phase 2 | Pending |
| MODEL-04 | Phase 2 | Pending |
| PROV-01 | Phase 2 | Pending |
| PROV-02 | Phase 2 | Pending |
| PROV-03 | Phase 2 | Pending |

**Coverage:**
- v1 requirements: 13 total
- Mapped to phases: 13
- Unmapped: 0 ✓

---
*Requirements defined: 2025-07-31*
*Last updated: 2025-07-31 after initial definition*
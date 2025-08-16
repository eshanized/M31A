# UX Polish Phase — Master Plan

**Phase:** UX Polish  
**Objective:** Fix all 203 UX issues identified in the UX Improvement Report  
**Source:** `rush/ux_improvement_report.md`  
**Estimated Duration:** 6-8 weeks (parallel execution possible)  

---

## Overview

This phase addresses **203 distinct UX improvement opportunities** across ~170 Go source files. The issues are organized into 8 executable plans, grouped by impact area and dependency.

### Issue Distribution

| Tier | Count | Description |
|------|-------|-------------|
| **P0 — Broken/Lying** | 12 | Commands that don't do what they claim |
| **P1 — High Impact** | 38 | Missing feedback, confusing errors |
| **P2 — Medium Impact** | 65 | Inconsistent styling, missing states |
| **P3 — Polish** | 88 | Minor visual issues, edge cases |
| **Total** | **203** | |

### Top 5 Most Impactful (by user-perceived quality)

1. **Broken commands** — `/clear`, `/undo`, `/pause`, `/resume-task` are no-ops or lies
2. **Error messages** — raw Go errors dumped to users, no actionable guidance
3. **Missing feedback** — no loading spinners, no streaming progress, no phase transition visibility
4. **Theme inconsistency** — command palette, execute/ship screens hardcode colors
5. **Permission UX** — `RiskDangerous` and `RiskDestructive` look identical; `E` key exits entire app

---

## Plan Structure

### Wave 1: Critical Trust & Safety (Week 1-2)

| Plan | Objective | Issues | Files |
|------|-----------|--------|-------|
| **PLAN-01** | Trust & Safety | 12 P0 | 8 files |
| **PLAN-02** | Error UX | 15 P0-P1 | 12 files |

**Rationale:** These plans fix the most critical issues — broken commands and confusing errors that erode user trust. Must be completed first.

### Wave 2: Feedback & Visual (Week 2-3)

| Plan | Objective | Issues | Files |
|------|-----------|--------|-------|
| **PLAN-03** | Feedback & Loading | 12 P1 | 15 files |
| **PLAN-04** | Theme & Visual | 10 P0-P1 | 10 files |

**Rationale:** These plans add loading states and fix theme consistency. Can run in parallel with Wave 1.

### Wave 3: Navigation & Screens (Week 3-5)

| Plan | Objective | Issues | Files |
|------|-----------|--------|-------|
| **PLAN-05** | Navigation & Help | 10 P1 | 8 files |
| **PLAN-06** | Screen-specific | 40+ P2-P3 | 20+ files |

**Rationale:** These plans fix navigation issues and screen-specific problems. Depends on Wave 1-2 for theme and error infrastructure.

### Wave 4: Tools & Config (Week 4-6)

| Plan | Objective | Issues | Files |
|------|-----------|--------|-------|
| **PLAN-07** | Tool & Provider | 30+ P1-P3 | 15+ files |
| **PLAN-08** | Config & Polish | 25+ P2-P3 | 12+ files |

**Rationale:** These plans fix tool rendering, provider issues, and config problems. Can run in parallel with Wave 3.

---

## Dependencies

```
PLAN-01 (Trust) ──────────┐
                          ├──► PLAN-05 (Navigation) ──► PLAN-06 (Screens)
PLAN-02 (Errors) ─────────┘         │
                                    ▼
PLAN-03 (Feedback) ────────► PLAN-07 (Tools)
                                    │
PLAN-04 (Theme) ───────────► PLAN-08 (Config)
```

### Critical Path

1. **PLAN-01** → Fix broken commands (blocks user trust)
2. **PLAN-02** → Fix error messages (blocks user understanding)
3. **PLAN-03** → Add feedback (blocks perceived speed)
4. **PLAN-04** → Fix themes (blocks light mode users)
5. **PLAN-05** → Fix navigation (blocks discoverability)
6. **PLAN-06** → Fix screens (blocks usability)
7. **PLAN-07** → Fix tools (blocks tool reliability)
8. **PLAN-08** → Fix config (blocks polish)

---

## Execution Strategy

### Parallel Execution

- **Wave 1:** PLAN-01 and PLAN-02 can run in parallel (different files)
- **Wave 2:** PLAN-03 and PLAN-04 can run in parallel (different files)
- **Wave 3:** PLAN-05 and PLAN-06 can run in parallel (different files)
- **Wave 4:** PLAN-07 and PLAN-08 can run in parallel (different files)

### Sequential Dependencies

- PLAN-05 depends on PLAN-01 (needs fixed commands for help system)
- PLAN-06 depends on PLAN-02 (needs error infrastructure)
- PLAN-07 depends on PLAN-03 (needs feedback infrastructure)
- PLAN-08 depends on PLAN-04 (needs theme infrastructure)

### Risk Mitigation

- Each plan has independent test coverage
- No cross-plan dependencies within waves
- Rolling back a plan doesn't affect other plans
- Each plan can be verified independently

---

## File Impact Summary

### High-Impact Files (modified by multiple plans)

| File | Plans | Issues |
|------|-------|--------|
| `internal/tui/commands_core.go` | PLAN-01, PLAN-05 | 6 |
| `internal/tui/commands_workflow.go` | PLAN-01, PLAN-05 | 4 |
| `internal/tui/components/permission.go` | PLAN-01, PLAN-04 | 5 |
| `internal/workflow/engine.go` | PLAN-03, PLAN-07 | 8 |
| `internal/tools/dispatcher.go` | PLAN-02, PLAN-07 | 4 |
| `internal/errors/errors.go` | PLAN-02 | 19 |
| `internal/tui/theme/theme.go` | PLAN-04 | 10+ |

### File Change Distribution

| Package | Files Changed | Issues Fixed |
|---------|---------------|--------------|
| `internal/tui/` | 25+ | 100+ |
| `internal/tui/components/` | 10+ | 30+ |
| `internal/workflow/` | 6 | 20+ |
| `internal/tools/` | 8 | 15+ |
| `internal/provider/` | 5 | 10+ |
| `internal/errors/` | 1 | 19 |
| `internal/config/` | 2 | 5 |
| `pkg/` | 8 | 15+ |

---

## Testing Strategy

### Per-Plan Testing

Each plan includes:
- Unit tests for modified functions
- Integration tests for new behaviors
- Regression tests for existing functionality

### Cross-Plan Testing

After each wave:
- `go test ./... -count=1` — full test suite
- `go vet ./...` — static analysis
- `golangci-lint run ./...` — linting

### Final Verification

After all waves:
- `go test -race ./...` — race condition detection
- Manual testing of all fixed commands
- Theme switching verification (dark/light)
- Cross-platform testing (Linux/macOS/Windows)

---

## Acceptance Criteria

### Phase-Level Criteria

- [ ] All 203 issues are addressed
- [ ] All P0 issues are fixed (12/12)
- [ ] All P1 issues are fixed (38/38)
- [ ] All P2 issues are fixed (65/65)
- [ ] All P3 issues are fixed (88/88)
- [ ] No regressions in existing functionality
- [ ] All tests pass: `go test -race ./...`
- [ ] All linting passes: `golangci-lint run ./...`

### Quality Criteria

- [ ] No hardcoded hex colors remain in TUI
- [ ] All loading states show animated spinners
- [ ] All error messages are user-friendly
- [ ] All destructive actions require confirmation
- [ ] All screens have consistent Esc handling
- [ ] All commands have truthful descriptions
- [ ] All empty states show helpful messages
- [ ] All truncation is indicated

---

## Risk Register

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Breaking existing commands | Medium | High | Comprehensive test coverage per plan |
| Theme regressions | Medium | Medium | Manual testing with both themes |
| Performance regression | Low | Medium | Benchmark critical paths |
| Cross-platform issues | Medium | Medium | CI matrix testing |
| Scope creep | High | Medium | Strict plan adherence |

---

## Rollback Strategy

Each plan is independent:
- Revert individual plan commits if issues arise
- No cross-plan dependencies within waves
- Each plan can be verified independently
- Git bisect can identify problematic plans

---

## Monitoring

### During Execution

- Monitor test coverage per plan
- Track issue completion rate
- Watch for regressions
- Verify theme consistency

### Post-Execution

- Full test suite pass
- Manual verification of all fixed commands
- User acceptance testing
- Performance benchmarking

---

## Plan Files

| Plan | File | Issues |
|------|------|--------|
| PLAN-01 | `PLAN-01-trust-safety.md` | 12 P0 |
| PLAN-02 | `PLAN-02-error-ux.md` | 15 P0-P1 |
| PLAN-03 | `PLAN-03-feedback-loading.md` | 12 P1 |
| PLAN-04 | `PLAN-04-theme-visual.md` | 10 P0-P1 |
| PLAN-05 | `PLAN-05-navigation-help.md` | 10 P1 |
| PLAN-06 | `PLAN-06-screen-specific.md` | 40+ P2-P3 |
| PLAN-07 | `PLAN-07-tool-provider.md` | 30+ P1-P3 |
| PLAN-08 | `PLAN-08-config-polish.md` | 25+ P2-P3 |

---

## Next Steps

1. Review all plan files for completeness
2. Prioritize Wave 1 execution (PLAN-01, PLAN-02)
3. Set up parallel execution infrastructure
4. Begin with PLAN-01 (Trust & Safety fixes)

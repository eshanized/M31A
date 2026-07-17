## ISSUES FOUND

**Phase:** 10-fix-test-errors-from-architecture-upgrade
**Plans checked:** 3
**Issues:** 2 blocker(s), 2 warning(s)

### Blockers (must fix)

**1. [Dimension 8e] VALIDATION.md not found for phase 10**
- Re-run `/gsd-plan-phase 10 --research` to regenerate.
- Without VALIDATION.md, Nyquist validation checks (8a-8d) cannot proceed.
- This is a gate requirement: all validation architecture must be documented before execution.

**2. [Dimension 11] RESEARCH.md has unresolved open questions**
- File: `.planning/phases/10-fix-test-errors-from-architecture-upgrade/10-RESEARCH.md`
- Section: `## Open Questions` (no `(RESOLVED)` suffix)
- Unresolved questions:
  1. Should pkg/types/ be created as an alias to internal/types/?
  2. How to handle package-specific test helpers that need internal access?
  3. What about the e2e_test.go location?
- Fix: Resolve each question and mark section as `## Open Questions (RESOLVED)`.

### Warnings (should fix)

**1. [Dimension 5] Plan 10-02 scope exceeds file threshold**
- Plan: `10-02`
- Metrics: 16 files modified (target: 5-8, warning: 10, blocker: 15+)
- 16 files is above the warning threshold; execution quality may degrade.
- Fix_hint: Consider splitting Plan 10-02 into two plans: one for mock centralization, one for consumer migration.

**2. [Dimension 7b] D-01 scope reduction risk**
- Plan: `10-02`
- Decision: D-01: "Move ALL test infrastructure to internal/testutil/ — shared mocks, setup helpers, test data builders, fixtures"
- Current plan: Centralizes 8 mock types (mockProvider, mockKeychain, mockTool, mockDispatcher) out of 22 identified in research.
- Risk: Plans may not fully deliver D-01 if additional shared mocks exist.
- Fix_hint: Verify that all shared mocks (used by 2+ packages) are centralized, or document why some remain colocated.

### Structured Issues

```yaml
issues:
  - plan: null
    dimension: "validation_architecture"
    severity: "blocker"
    description: "VALIDATION.md missing for phase 10"
    fix_hint: "Re-run /gsd-plan-phase 10 --research to regenerate VALIDATION.md"

  - plan: null
    dimension: "research_resolution"
    severity: "blocker"
    description: "RESEARCH.md Open Questions section not marked RESOLVED"
    file: "10-RESEARCH.md"
    unresolved_questions:
      - "Should pkg/types/ be created as an alias to internal/types/?"
      - "How to handle package-specific test helpers that need internal access?"
      - "What about the e2e_test.go location?"
    fix_hint: "Resolve questions and rename section to '## Open Questions (RESOLVED)'"

  - plan: "10-02"
    dimension: "scope_sanity"
    severity: "warning"
    description: "Plan 10-02 modifies 16 files, exceeding warning threshold"
    metrics:
      tasks: 3
      files: 16
    fix_hint: "Consider splitting into two plans for better context management"

  - plan: "10-02"
    dimension: "scope_reduction"
    severity: "warning"
    description: "D-01 may not be fully delivered — only 8 of 22 mock types centralized"
    decision: "D-01: Move ALL test infrastructure to internal/testutil/"
    fix_hint: "Verify all shared mocks are centralized, or document rationale for keeping some colocated"
```

### Recommendations

1. **Fix blockers first**: VALIDATION.md and resolved open questions are required before execution.
2. **Consider splitting Plan 10-02**: 16 files is a lot for one execution context; splitting would improve quality.
3. **Verify D-01 coverage**: Ensure all shared mocks (used by multiple packages) are moved to testutil/mocks/.

### What Passed

- **Requirement Coverage**: NFR-4 is covered by all three plans.
- **Task Completeness**: All tasks have Files, Action, Verify (automated), Acceptance Criteria, Done.
- **Dependency Graph**: Linear chain (10-01 → 10-02 → 10-03) with no cycles.
- **Key Links**: must_haves.artifacts are specific and traceable to phase goal.
- **must_haves Derivation**: Truths are user-observable (build succeeds, tests pass).
- **Context Compliance**: Decisions D-02 through D-13 are addressed in plan tasks.
- **AGENTS.md Compliance**: Plans use `goimports`, `make check`, `go vet`, and follow project conventions.

---

**Status:** ISSUES FOUND — 2 blocker(s) require revision before execution.

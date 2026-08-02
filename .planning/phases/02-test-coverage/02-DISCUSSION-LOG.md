# Phase 02: Test Coverage Remediation - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-08-02
**Phase:** 02-Test Coverage Remediation
**Areas discussed:** Priority Order, Testing Strategy, Coverage Targets, Test File Organization

---

## Priority Order

| Option | Description | Selected |
|--------|-------------|----------|
| Zero-first | Start with 0% packages, then gaps | ✓ |
| Gap-first | Start with biggest gaps first | |
| Critical-first | Start with critical path packages first | |

**User's choice:** Zero-first — start with zero-coverage packages (exec, fileops, network, ai, nvidia), then critical gaps, then moderate gaps
**Notes:** This approach builds foundation first, then extends to existing partial coverage

---

## Testing Strategy

| Option | Description | Selected |
|--------|-------------|----------|
| Table-driven | Table-driven tests as primary pattern | ✓ |
| Behavior-driven | BDD-style with ginkgo/gomega | |
| Mixed approach | Table-driven + some BDD for complex cases | |

**User's choice:** Table-driven tests — consistent with existing codebase patterns
**Notes:** Go standard library style, well-supported, easy to maintain

---

## Coverage Targets

| Option | Description | Selected |
|--------|-------------|----------|
| 75% all | 75% minimum for all packages | ✓ |
| 75%/90% split | 75% general, 90% critical path | ✓ |
| 80% all | 80% minimum for all packages | |

**User's choice:** 75% all packages, 90% for critical path (rollback, workflow, compaction)
**Notes:** Critical path needs higher coverage due to complexity and impact

---

## Test File Organization

| Option | Description | Selected |
|--------|-------------|----------|
| Co-located | *_test.go alongside source files | ✓ |
| Separate dir | tests/ directory structure | |
| Both | Co-located + integration tests/ dir | |

**User's choice:** Co-located test files — standard Go convention
**Notes:** Easy to find, maintain, and follow Go project layout

---

## the agent's Discretion

- Exact test case selection within each package
- Mock implementations for external dependencies
- Test helper function design
- Coverage verification approach

## Deferred Ideas

- Integration test suite (could be separate phase)
- Performance/load testing for tools
- Property-based testing patterns

---

*Discussion completed: 2026-08-02*

# Phase 3: Engineering Excellence - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-08-05
**Phase:** 03-engineering-excellence
**Areas discussed:** Refactoring Priority, Documentation Strategy, Module Boundary Rules, Developer Experience Scope

---

## Refactoring Priority

| Option | Description | Selected |
|--------|-------------|----------|
| Engine First | Start with engine.go split (1832 lines → focused structs). Higher risk, higher payoff. | ✓ |
| Docs/DX First | Document architecture + add DX tools first. Lower risk, enables later work. | |
| Interleaved | Interleave: document as you refactor. Balanced approach. | |

**User's choice:** Engine First (Recommended)
**Notes:** User agreed that splitting engine.go first unlocks safer concurrent changes later.

---

## Documentation Strategy

| Option | Description | Selected |
|--------|-------------|----------|
| Planning Directory | Put docs in .planning/codebase/. Keeps code clean, docs versioned with plans. | ✓ |
| In-Code Comments | Doc comments in Go files + godoc. Keeps docs next to code, auto-generated. | |
| Hybrid Approach | Both: brief doc comments in code, detailed docs in .planning/. | |

**User's choice:** Planning Directory (Recommended)
**Notes:** User preferred keeping code clean and docs versioned with plans.

---

## Module Boundary Rules

| Option | Description | Selected |
|--------|-------------|----------|
| Convention + Interface | Go module system for hard boundaries, interfaces for soft boundaries. Pragmatic, low overhead. | ✓ |
| Compile-Time Enforcement | Add compile-time checks (go vet rules, package dependency graphs). | |
| Runtime DI Container | Runtime dependency injection with container. Maximum flexibility, but adds complexity. | |

**User's choice:** Convention + Interface (Recommended)
**Notes:** User preferred low overhead approach without runtime DI or compile-time tooling.

---

## Developer Experience Scope

| Option | Description | Selected |
|--------|-------------|----------|
| All Three | Debug logging, profiling setup, and release automation. Comprehensive but takes longer. | ✓ |
| Debug + Profile Only | Focus on debug logging and profiling. Release automation can wait. | |
| Debug Logging Only | Focus on debug logging. Most immediately useful for contributor productivity. | |

**User's choice:** All Three (Recommended)
**Notes:** User wanted comprehensive DX improvements including release automation.

---

## the agent's Discretion

- Agent may choose specific file boundaries when splitting engine.go
- Agent may select documentation format within .planning/codebase/
- Agent may design debug logging format and profiling integration
- Agent may choose release automation tooling (goreleaser, Makefile targets, etc.)

## Deferred Ideas

None — discussion stayed within phase scope

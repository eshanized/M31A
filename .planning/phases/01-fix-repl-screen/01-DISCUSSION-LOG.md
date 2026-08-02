# Phase 01: Fix REPL Screen - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-08-02
**Phase:** 01-fix-repl-screen
**Areas discussed:** Test Coverage Scope, Idempotency Reporting, app_nav.go Workaround

---

## Test Coverage Scope

| Option | Description | Selected |
|--------|-------------|----------|
| Core bug only (Recommended) | One test: SwitchTo(id) for unregistered screen, then Register(id), then assert View() returns mock content. Minimal, surgical. | ✓ |
| Core + nil active | Core test plus: test View() returns empty string when no screen registered. | |
| Core + ordering | Core test plus: test Register then SwitchTo works (happy path). | |
| Full suite | All edge cases: nil active, happy path, SwitchTo with id=0, multiple SwitchTo before Register, etc. | |

**User's choice:** Core bug only (Recommended)
**Notes:** Surgical approach — one regression test that reproduces the exact bug.

---

## Idempotency Reporting

| Option | Description | Selected |
|--------|-------------|----------|
| Summary table only (Recommended) | List affected screen names in a table. No code snippets. | |
| Full analysis | List each screen with the specific lines calling Register(), whether model is nil-guarded, and impact assessment. | ✓ |
| Skip reporting | Don't mention idempotency in CONTEXT.md at all. | |

**User's choice:** Full analysis
**Notes:** Want complete picture of which screens are affected for future reference.

---

## app_nav.go Workaround

| Option | Description | Selected |
|--------|-------------|----------|
| Document it (Recommended) | Note in CONTEXT.md that routeToScreen() re-invokes Register + SwitchTo as a workaround, and the router fix should make it unnecessary — but leave it in place for now. | ✓ |
| Don't mention it | Keep CONTEXT.md focused on the router fix only. | |
| Also fix it | Remove the redundant re-invocation logic in routeToScreen() as part of this change. | |

**User's choice:** Document it (Recommended)
**Notes:** Router fix should make the workaround unnecessary, but confirm via test rather than assume.

---

## agent's Discretion

- Comment explaining unconditional activeID set in SwitchTo() — agent may choose
- Mock model implementation for test — agent may choose approach

## Deferred Ideas

None — discussion stayed within phase scope.

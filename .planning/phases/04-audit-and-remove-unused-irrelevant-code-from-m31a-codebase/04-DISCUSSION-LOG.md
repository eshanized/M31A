# Phase 4: Audit and remove unused/irrelevant code from M31A codebase - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-27
**Phase:** 04-audit-and-remove-unused-irrelevant-code-from-m31a-codebase
**Areas discussed:** Scope of 'irrelevant' definition, Cleanup aggressiveness, Test file handling

---

## Scope of 'irrelevant' definition

| Option | Description | Selected |
|--------|-------------|----------|
| Outside core purpose | Code that doesn't serve M31A's core purpose: AI-powered CLI agent with TUI, workflow engine, and multi-provider LLM support | |
| Experimental/abandoned | Features that were started but never completed, or experiments that didn't work out | |
| Provider-specific bloat | Code specific to one provider (OpenRouter/Zen/Nvidia) that could be simplified or removed if not essential | |
| All of the above | Remove anything that isn't clearly part of M31A's active, working feature set | ✓ |

**User's choice:** All of the above
**Notes:** Comprehensive cleanup covering all categories of irrelevant code

---

## Cleanup aggressiveness

| Option | Description | Selected |
|--------|-------------|----------|
| Conservative | Only remove code that is clearly dead (no references, no tests, no imports). Keep anything that might be used. | ✓ |
| Moderate | Remove dead code AND code that's only referenced by other dead code. Keep anything with active callers. | |
| Aggressive | Remove anything not actively exercised by tests or main code paths. If it's not tested or called from main workflow, remove it. | |

**User's choice:** Conservative
**Notes:** Safety first — only remove clearly dead code

---

## Test file handling

| Option | Description | Selected |
|--------|-------------|----------|
| Remove tests too | If the code is dead, its tests are also dead. Remove both together. | ✓ |
| Keep tests as docs | Keep tests even if code is removed, as documentation of expected behavior patterns. | |

**User's choice:** Remove tests too
**Notes:** Clean removal — dead code and its tests go together

---

## Agent's Discretion

- Methodology for identifying dead code (grep for references, go vet, static analysis)
- Whether to run `go mod tidy` to clean unused dependencies
- Order of removal (packages first, then functions, then files)

## Deferred Ideas

None — discussion stayed within phase scope.

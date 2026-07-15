# Phase 5: Codebase Maintainability — Split Large Files - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-15
**Phase:** 5-codebase-maintainability-split-large-files
**Areas discussed:** File size threshold, Splitting strategy, Testing approach, Priority order, Naming conventions, Commit strategy, Import management

---

## File Size Threshold

| Option | Description | Selected |
|--------|-------------|----------|
| 200 lines (Recommended) | Moderate — catches files that are getting unwieldy but not yet critical. Good for proactive maintenance. | ✓ |
| 300 lines | Only split files that are genuinely large. Fewer changes, less risk of breaking things. | |
| 500 lines | Aggressive — splits even moderately sized files. More work but maximum maintainability. | |

**User's choice:** 200 lines (Recommended)
**Notes:** None

---

## Splitting Strategy

| Option | Description | Selected |
|--------|-------------|----------|
| By functionality (Recommended) | Separate files by what they DO (e.g., 'permissions.go', 'rate_limit.go' from dispatcher.go). Most natural for Go packages. | ✓ |
| By type | Separate interfaces from implementations (e.g., 'interface.go' + 'impl.go'). Good for large provider or tool files. | |
| You decide | Agent decides the best approach per file based on code structure. Maximum flexibility but less consistent. | |

**User's choice:** By functionality (Recommended)
**Notes:** None

---

## Testing Approach

| Option | Description | Selected |
|--------|-------------|----------|
| Full verification (Recommended) | Run 'make check' (fmt → tidy → vet → lint → test with race detector). Full verification that nothing is broken. | ✓ |
| Quick test only | Run 'make test-fast' (tests without race detector). Faster but less thorough. | |
| Architectural verification | Run tests + verify zero pkg/ to internal/ imports + check go.mod tidy. Focus on architectural constraints. | |

**User's choice:** Full verification (Recommended)
**Notes:** None

---

## Priority Order

| Option | Description | Selected |
|--------|-------------|----------|
| Largest files first (Recommended) | Start with the biggest files first. Highest impact on maintainability per change. | |
| Critical path first | Start with files in the critical path (engine, dispatcher, main). Riskiest but most impactful. | |
| You decide | Agent decides the optimal order based on dependencies and risk. Most flexible. | ✓ |

**User's choice:** You decide
**Notes:** Agent has flexibility to prioritize based on dependencies and risk

---

## Naming Conventions

| Option | Description | Selected |
|--------|-------------|----------|
| snake_case.go (Recommended) | Follow existing convention: snake_case.go (e.g., 'permissions.go', 'rate_limit.go'). Consistent with codebase. | |
| Descriptive names | Use descriptive names based on content (e.g., 'tool_permission_handler.go'). More explicit but longer. | ✓ |
| You decide | Agent decides the best name per file. Maximum flexibility. | |

**User's choice:** Descriptive names
**Notes:** More explicit naming helps developers understand file purpose at a glance

---

## Commit Strategy

| Option | Description | Selected |
|--------|-------------|----------|
| One commit per split (Recommended) | Each file split gets its own commit. Easy to review, easy to revert if something breaks. | ✓ |
| Batch by package | Batch related splits (e.g., all splits in internal/tui/) into one commit. Fewer commits but harder to isolate issues. | |
| You decide | Agent decides commit granularity based on risk and relationship. Maximum flexibility. | |

**User's choice:** One commit per split (Recommended)
**Notes:** None

---

## Import Management

| Option | Description | Selected |
|--------|-------------|----------|
| goimports automatically (Recommended) | Run goimports -w automatically after each split. Standard Go tool, fixes imports and formatting. | ✓ |
| Manual review | Review imports manually after each split. More control but slower. | |
| You decide | Agent decides the best approach per file. Maximum flexibility. | |

**User's choice:** goimports automatically (Recommended)
**Notes:** None

---

## the agent's Discretion

- Priority order: Agent decides optimal order based on dependencies and risk
- Splitting approach: Agent can determine the best approach per file based on code structure
- Commit granularity: Agent decides when batching related splits

## Deferred Ideas

None — discussion stayed within phase scope

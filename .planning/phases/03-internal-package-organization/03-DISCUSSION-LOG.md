# Phase 3: Internal Package Organization - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-23
**Phase:** 3-internal-package-organization
**Areas discussed:** Tools root file grouping, TUI root file grouping, Import path disruption, Test file placement

---

## Tools root file grouping

| Option | Description | Selected |
|--------|-------------|----------|
| Group by category | Create new subdirs: git/, network/, codeanalysis/, todo/. Keep core at root. | |
| Keep flat, minimal moves | Only move files that clearly belong together. Keep most at root. | |
| Move all tools to subdirs | Every tool implementation gets its own subdir. Root only has core files. | ✓ |

**User's choice:** Move all tools to subdirs
**Notes:** User wants clean separation — root only for interface.go, dispatcher.go, defaults.go, constants.go, toolcall.go, tooldefs.go, tools_reexport.go

---

## Subdir granularity (follow-up)

| Option | Description | Selected |
|--------|-------------|----------|
| Group related tools | git/ has git.go. todo/ has todo.go+todoread.go. codeanalysis/ has codecomplexity.go+codemap.go. | ✓ |
| One subdir per tool | Each tool gets its own directory regardless of file count. | |
| Agent decides | Let the planner decide groupings based on dependencies. | |

**User's choice:** Group related tools
**Notes:** Grouping by logical relationship, not file count

---

## TUI root file grouping

| Option | Description | Selected |
|--------|-------------|----------|
| Move screens to screens/ | Move *_model.go and *_view.go to screens/ subdirs. Keep app_*, handler_*, repl_* at root. | |
| Group by responsibility | Create subdirs: core/, handlers/, screens/, repl/, config/. | ✓ |
| Minimal moves | Only move files that clearly belong in existing subdirs. | |

**User's choice:** Group by responsibility
**Notes:** Clear separation of concerns — core logic, handlers, screens, repl, config

---

## Core AppState split (follow-up)

| Option | Description | Selected |
|--------|-------------|----------|
| Keep app_* together in core/ | All app_*.go files move to tui/core/. Simple, keeps AppState cohesive. | |
| Split by function | app_handlers*.go → handlers/, app_input*.go → input/, etc. | ✓ |
| Agent decides | Let planner decide based on dependency analysis. | |

**User's choice:** Split by function
**Notes:** More granular organization — handlers/, input/, update/, routing/ subdirs

---

## Import path disruption

| Option | Description | Selected |
|--------|-------------|----------|
| Minimize changes | Move only files that clearly benefit. Keep commonly-imported files at root. | |
| Ideal structure | Move everything to proper home regardless of import churn. Use sed/goimports. | ✓ |
| Phased approach | Phase 3a: tools (55 imports). Phase 3b: TUI (223 imports). | |

**User's choice:** Ideal structure
**Notes:** 55 files import internal/tools, 223 import internal/ui/tui — all will be updated in one pass

---

## Test file placement

| Option | Description | Selected |
|--------|-------------|----------|
| Keep tests with source | Standard Go convention. Test stays next to source file. | |
| Consolidate tests | Move all *_test.go to tests/ directory. Cleaner source tree. | ✓ |
| Agent decides per file | Unit tests stay with source, integration tests move. | |

**User's choice:** Consolidate tests
**Notes:** Breaks Go convention but user wants cleaner source tree. Planner will decide which tests need unexported access.

---

## the agent's Discretion

- Exact tool subgroupings based on dependency analysis
- Which tests need unexported access vs can be consolidated
- Migration strategy and risk assessment

## Deferred Ideas

None — discussion stayed within phase scope.

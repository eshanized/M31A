# Phase 3: Internal Package Organization - Research

**Researched:** 2026-07-23
**Domain:** Go project organization, package structure, import path management
**Confidence:** HIGH

## Summary

This phase reorganizes `internal/tools/` (71 root files + 5 subdirs) and `internal/ui/tui/` (182 root files + 8 subdirs) into a professional directory structure. The current flat organization creates cognitive overhead and makes it difficult to understand package boundaries. The reorganization follows standard Go project conventions while maintaining all existing functionality.

**Primary recommendation:** Move all tool implementations to logical subdirectories while keeping core infrastructure at root. For TUI, group files by responsibility (core, handlers, screens, routing) rather than by file prefix.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Move ALL tool implementations to subdirectories — root only keeps core infrastructure (interface.go, dispatcher.go, defaults.go, constants.go, toolcall.go, tooldefs.go, tools_reexport.go)
- **D-02:** Group related tools into shared subdirectories: git/ (git.go), todo/ (todo.go + todoread.go), codeanalysis/ (codecomplexity.go + codemap.go), network/ (dns_cache.go + httpcheck.go)
- **D-04:** Group by responsibility, not by file prefix — create subdirs: core/, handlers/, screens/, repl/, config/
- **D-05:** Split app_*.go files by function into: core/ (app.go, app_state.go, app_view.go), handlers/ (app_handlers*.go), input/ (app_input*.go), update/ (app_update*.go), routing/ (app_routing.go, app_nav.go, app_screens.go)
- **D-08:** Ideal structure — move everything to its proper home regardless of import churn
- **D-11:** Consolidate test files into a separate tests/ directory structure (breaks standard Go _test.go convention)
- **D-12:** Mirror the source directory structure in tests/ (tests/tools/git/, tests/tui/core/, etc.)
- **D-13:** Unit tests that need unexported access stay with source; integration/e2e tests move to tests/

### the agent's Discretion
- Agent has flexibility in determining exact subgroupings based on dependency analysis
- Agent can decide which tests need unexported access vs can be consolidated
- Agent can adjust timeout values and migration strategy based on risk assessment

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| TOOLS-01 | Move tool implementations to subdirectories | Dependency analysis shows clean separation between tool types |
| TOOLS-02 | Maintain core infrastructure at root | interface.go, dispatcher.go, defaults.go have cross-cutting dependencies |
| TUI-01 | Group TUI files by responsibility | File analysis shows clear separation: state, handlers, input, update, routing |
| TUI-02 | Split app_*.go files by function | Dependency analysis shows minimal cross-dependencies between handler types |
| TEST-01 | Consolidate test files | Test analysis shows which tests need unexported access |
| IMPORT-01 | Update all import paths | Import analysis shows 55 files import tools, 223 import tui |
</phase_requirements>

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Tool registration & dispatch | internal/tools/ | internal/types/ | Core infrastructure manages tool lifecycle |
| File operations | internal/tools/fileops/ | internal/tools/ | Specialized subpackage for file I/O |
| Shell execution | internal/tools/exec/ | internal/tools/ | Platform-specific process management |
| Search operations | internal/tools/search/ | internal/tools/ | Web and filesystem search |
| AI interaction | internal/tools/ai/ | internal/tools/subagent/ | Question handling and subagent coordination |
| TUI state management | internal/ui/tui/core/ | internal/ui/tui/ | Central AppState and lifecycle |
| TUI message handling | internal/ui/tui/handlers/ | internal/ui/tui/core/ | Message processing and dispatch |
| TUI screen management | internal/ui/tui/screens/ | internal/ui/tui/ | Individual screen implementations |
| TUI input processing | internal/ui/tui/input/ | internal/ui/tui/handlers/ | Keyboard and mouse handling |
| TUI routing | internal/ui/tui/routing/ | internal/ui/tui/core/ | Screen navigation and state transitions |

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Go | 1.25+ | Language runtime | Project requirement (CGO_ENABLED=0) |
| bubbletea | latest | TUI framework | Elm architecture for complex UIs |
| lipgloss | latest | TUI styling | Declarative styling for terminal |
| bubbles | latest | TUI components | Reusable UI primitives |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| goimports | latest | Import organization | After any import path changes |
| golangci-lint | latest | Code quality | Pre-commit verification |
| go test | built-in | Testing | All verification |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Manual file moves | gofmt + goimports | Automated formatting after moves |
| sed for imports | goimports | More reliable import path updates |
| Manual testing | make check | Comprehensive verification |

**Installation:**
```bash
go install golang.org/x/tools/cmd/goimports@latest
make lint  # Uses golangci-lint
```

**Version verification:** Go 1.25+ required (per go.mod), CGO_ENABLED=0 hard constraint.

## Package Legitimacy Audit

> **Required** whenever this phase installs external packages. Run the Package Legitimacy Gate protocol before completing this section.

No external packages are being installed in this phase. This is purely a code reorganization.

**Packages removed due to [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none

*Packages discovered via WebSearch or training data that have not been verified against an authoritative source are tagged `[ASSUMED]` and the planner must gate each install behind a `checkpoint:human-verify` task.*

## Architecture Patterns

### System Architecture Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                      cmd/m31a/main.go                      │
│  (Entry point: config, providers, TUI construction)        │
└─────────────┬───────────────────────────────┬───────────────┘
              │                               │
              ▼                               ▼
┌─────────────────────────┐     ┌─────────────────────────────┐
│    internal/tools/      │     │      internal/ui/tui/       │
│  ┌─────────────────┐    │     │  ┌───────────────────────┐  │
│  │ Core (root)     │    │     │  │ Core (core/)          │  │
│  │ - interface.go  │    │     │  │ - app.go              │  │
│  │ - dispatcher.go │    │     │  │ - app_state.go        │  │
│  │ - defaults.go   │    │     │  │ - app_view.go         │  │
│  └────────┬────────┘    │     │  └───────────┬───────────┘  │
│           │             │     │              │              │
│  ┌────────▼────────┐    │     │  ┌───────────▼───────────┐  │
│  │ Subdirectories  │    │     │  │ Responsibility Groups │  │
│  │ - ai/           │    │     │  │ - handlers/           │  │
│  │ - exec/         │    │     │  │ - input/              │  │
│  │ - fileops/      │    │     │  │ - update/             │  │
│  │ - search/       │    │     │  │ - routing/            │  │
│  │ - subagent/     │    │     │  │ - screens/            │  │
│  │ - git/ (new)    │    │     │  │ - repl/               │  │
│  │ - todo/ (new)   │    │     │  │ - config/             │  │
│  │ - codeanalysis/ │    │     │  └───────────────────────┘  │
│  │ - network/ (new)│    │     │                             │
│  └─────────────────┘    │     └─────────────────────────────┘
└─────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────────────────────────┐
│                    internal/types/                          │
│  (Shared vocabulary: no internal imports)                   │
└─────────────────────────────────────────────────────────────┘
```

### Recommended Project Structure
```
internal/
├── tools/
│   ├── interface.go          # Tool interface definition
│   ├── dispatcher.go         # Central executor
│   ├── defaults.go           # Tool registration
│   ├── constants.go          # Tool constants
│   ├── toolcall.go           # Tool call types
│   ├── tooldefs.go           # Tool definitions
│   ├── tools_reexport.go     # Re-exports for backward compatibility
│   ├── ai/                   # AI tools (AskUserQuestion)
│   ├── exec/                 # Execution tools (Bash, DevServer)
│   ├── fileops/              # File operation tools
│   ├── search/               # Search tools (Glob, Grep, WebSearch)
│   ├── subagent/             # Subagent management
│   ├── git/                  # Git tool (NEW)
│   ├── todo/                 # Todo tools (NEW)
│   ├── codeanalysis/         # Code analysis tools (NEW)
│   └── network/              # Network tools (NEW)
├── ui/tui/
│   ├── core/                 # Core TUI infrastructure (NEW)
│   │   ├── app.go
│   │   ├── app_state.go
│   │   └── app_view.go
│   ├── handlers/             # Message handlers (NEW)
│   │   ├── app_handlers*.go
│   │   └── handler_*.go
│   ├── input/                # Input processing (NEW)
│   │   └── app_input*.go
│   ├── update/               # Update logic (NEW)
│   │   └── app_update*.go
│   ├── routing/              # Screen routing (NEW)
│   │   ├── app_routing.go
│   │   ├── app_nav.go
│   │   └── app_screens.go
│   ├── screens/              # Screen implementations (EXISTING)
│   ├── components/           # Reusable components (EXISTING)
│   ├── commands/             # Command registry (EXISTING)
│   ├── layout/               # Layout calculations (EXISTING)
│   ├── streaming/            # SSE streaming (EXISTING)
│   ├── theme/                # Lipgloss theme (EXISTING)
│   ├── tuitypes/             # TUI message types (EXISTING)
│   └── a11y/                 # Accessibility (EXISTING)
└── tests/                    # Consolidated tests (NEW)
    ├── tools/
    │   ├── git/
    │   ├── todo/
    │   ├── codeanalysis/
    │   └── network/
    └── tui/
        ├── core/
        ├── handlers/
        ├── input/
        ├── update/
        └── routing/
```

## Import Dependency Map

### Tools Package Dependencies

```
internal/tools/ (root)
├── interface.go → internal/core/types
├── dispatcher.go → internal/core/config, internal/core/errors, internal/core/types, internal/tools/fileops, internal/tools/subagent
├── defaults.go → internal/tools/ai, internal/tools/exec, internal/tools/fileops, internal/tools/search
├── constants.go → (no internal imports)
├── toolcall.go → internal/core/types
├── tooldefs.go → internal/core/types
└── tools_reexport.go → internal/tools/ai, internal/tools/exec, internal/tools/fileops, internal/tools/search, internal/tools/subagent

internal/tools/ai/
├── agent.go → internal/tools/subagent
├── memory.go → internal/core/types
└── question.go → internal/core/types

internal/tools/exec/
├── bash.go → internal/core/errors, internal/core/types
├── devserver.go → internal/core/errors, internal/core/types
└── prockill_*.go → (no internal imports)

internal/tools/fileops/
├── edit.go → internal/core/errors, internal/core/types
├── file*.go → internal/core/errors, internal/core/types
└── helpers.go → internal/core/types

internal/tools/search/
├── glob.go → internal/core/errors, internal/core/types
├── grep.go → internal/core/errors, internal/core/types
├── webfetch.go → internal/core/errors, internal/core/types, internal/tools/search (dns_cache)
└── websearch.go → internal/core/errors, internal/core/types

internal/tools/subagent/
├── manager.go → internal/core/types, internal/tools (dispatcher interface)
├── events.go → internal/core/types
└── worktree.go → internal/core/types
```

### TUI Package Dependencies

```
internal/ui/tui/ (root)
├── app.go → internal/core/config, internal/core/errors, internal/core/types, internal/engine/tokens, internal/engine/workflow, internal/integrations/arbitrage, internal/integrations/metrics, internal/tools, internal/tools/exec
├── app_state.go → internal/core/config, internal/core/errors, internal/core/types, internal/engine/decision, internal/engine/narrative, internal/engine/rollback, internal/engine/session, internal/engine/workflow, internal/integrations/arbitrage, internal/integrations/autodream, internal/integrations/git, internal/integrations/history, internal/integrations/keychain, internal/integrations/ledger, internal/integrations/metrics, internal/integrations/provider, internal/tools, internal/tools/subagent, internal/ui/tui/components, internal/ui/tui/theme
├── app_view.go → internal/ui/tui/components, internal/ui/tui/theme
├── app_handlers*.go → internal/core/errors, internal/core/types
├── app_input*.go → internal/core/types
├── app_update*.go → internal/core/types, internal/engine/workflow
├── app_routing.go → internal/core/types, internal/engine/workflow
├── app_nav.go → internal/core/types
└── app_screens.go → internal/core/types

internal/ui/tui/components/
└── *.go → internal/ui/tui/theme, internal/ui/tui/tuitypes

internal/ui/tui/commands/
└── *.go → internal/core/types, internal/ui/tui

internal/ui/tui/screens/
└── */*.go → internal/core/types, internal/ui/tui, internal/ui/tui/components, internal/ui/tui/theme

internal/ui/tui/streaming/
└── *.go → internal/core/types, internal/ui/tui

internal/ui/tui/theme/
└── *.go → (no internal imports)

internal/ui/tui/tuitypes/
└── *.go → (no internal imports)

internal/ui/tui/layout/
└── *.go → (no internal imports)

internal/ui/tui/a11y/
└── *.go → (no internal imports)
```

### Critical Import Chains

1. **Tools → Types:** All tools import `internal/core/types` (leaf package)
2. **TUI → Tools:** TUI imports `internal/tools` and `internal/tools/subagent`
3. **Workflow → Tools:** Workflow engine imports `internal/tools`
4. **Components → Theme:** All TUI components import `internal/ui/tui/theme`

### Import Path Change Summary

| Original Path | New Path | Files Affected |
|---------------|----------|----------------|
| `internal/tools` | `internal/tools/git` | 55 files |
| `internal/tools` | `internal/tools/todo` | 55 files |
| `internal/tools` | `internal/tools/codeanalysis` | 55 files |
| `internal/tools` | `internal/tools/network` | 55 files |
| `internal/ui/tui` | `internal/ui/tui/core` | 223 files |
| `internal/ui/tui` | `internal/ui/tui/handlers` | 223 files |
| `internal/ui/tui` | `internal/ui/tui/input` | 223 files |
| `internal/ui/tui` | `internal/ui/tui/update` | 223 files |
| `internal/ui/tui` | `internal/ui/tui/routing` | 223 files |

## Move Operations List

### Tools Directory Moves

| Source File | Destination | Package Change | Import Path Change |
|-------------|-------------|----------------|-------------------|
| `internal/tools/git.go` | `internal/tools/git/git.go` | `tools` → `git` | `github.com/eshanized/M31A/internal/tools` → `github.com/eshanized/M31A/internal/tools/git` |
| `internal/tools/todo.go` | `internal/tools/todo/todo.go` | `tools` → `todo` | `github.com/eshanized/M31A/internal/tools` → `github.com/eshanized/M31A/internal/tools/todo` |
| `internal/tools/todoread.go` | `internal/tools/todo/todoread.go` | `tools` → `todo` | `github.com/eshanized/M31A/internal/tools` → `github.com/eshanized/M31A/internal/tools/todo` |
| `internal/tools/codecomplexity.go` | `internal/tools/codeanalysis/codecomplexity.go` | `tools` → `codeanalysis` | `github.com/eshanized/M31A/internal/tools` → `github.com/eshanized/M31A/internal/tools/codeanalysis` |
| `internal/tools/codemap.go` | `internal/tools/codeanalysis/codemap.go` | `tools` → `codeanalysis` | `github.com/eshanized/M31A/internal/tools` → `github.com/eshanized/M31A/internal/tools/codeanalysis` |
| `internal/tools/dns_cache.go` | `internal/tools/network/dns_cache.go` | `tools` → `network` | `github.com/eshanized/M31A/internal/tools` → `github.com/eshanized/M31A/internal/tools/network` |
| `internal/tools/httpcheck.go` | `internal/tools/network/httpcheck.go` | `tools` → `network` | `github.com/eshanized/M31A/internal/tools` → `github.com/eshanized/M31A/internal/tools/network` |
| `internal/tools/backup.go` | `internal/tools/fileops/backup.go` | `tools` → `fileops` | `github.com/eshanized/M31A/internal/tools` → `github.com/eshanized/M31A/internal/tools/fileops` |
| `internal/tools/concurrency.go` | `internal/tools/exec/concurrency.go` | `tools` → `exec` | `github.com/eshanized/M31A/internal/tools` → `github.com/eshanized/M31A/internal/tools/exec` |
| `internal/tools/metrics.go` | `internal/tools/search/metrics.go` | `tools` → `search` | `github.com/eshanized/M31A/internal/tools` → `github.com/eshanized/M31A/internal/tools/search` |
| `internal/tools/performance.go` | `internal/tools/search/performance.go` | `tools` → `search` | `github.com/eshanized/M31A/internal/tools` → `github.com/eshanized/M31A/internal/tools/search` |
| `internal/tools/pathhelpers.go` | `internal/tools/fileops/pathhelpers.go` | `tools` → `fileops` | `github.com/eshanized/M31A/internal/tools` → `github.com/eshanized/M31A/internal/tools/fileops` |
| `internal/tools/strings.go` | `internal/tools/search/strings.go` | `tools` → `search` | `github.com/eshanized/M31A/internal/tools` → `github.com/eshanized/M31A/internal/tools/search` |
| `internal/tools/output_store.go` | `internal/tools/exec/output_store.go` | `tools` → `exec` | `github.com/eshanized/M31A/internal/tools` → `github.com/eshanized/M31A/internal/tools/exec` |
| `internal/tools/prockill_unix.go` | `internal/tools/exec/prockill_unix.go` | `tools` → `exec` | `github.com/eshanized/M31A/internal/tools` → `github.com/eshanized/M31A/internal/tools/exec` |
| `internal/tools/prockill_windows.go` | `internal/tools/exec/prockill_windows.go` | `tools` → `exec` | `github.com/eshanized/M31A/internal/tools` → `github.com/eshanized/M31A/internal/tools/exec` |
| `internal/tools/permissions.go` | `internal/tools/permissions.go` | `tools` → `tools` | None (stays at root) |
| `internal/tools/persistent_permissions.go` | `internal/tools/persistent_permissions.go` | `tools` → `tools` | None (stays at root) |

### TUI Directory Moves

| Source File | Destination | Package Change | Import Path Change |
|-------------|-------------|----------------|-------------------|
| `internal/ui/tui/app.go` | `internal/ui/tui/core/app.go` | `tui` → `core` | `github.com/eshanized/M31A/internal/ui/tui` → `github.com/eshanized/M31A/internal/ui/tui/core` |
| `internal/ui/tui/app_state.go` | `internal/ui/tui/core/app_state.go` | `tui` → `core` | `github.com/eshanized/M31A/internal/ui/tui` → `github.com/eshanized/M31A/internal/ui/tui/core` |
| `internal/ui/tui/app_view.go` | `internal/ui/tui/core/app_view.go` | `tui` → `core` | `github.com/eshanized/M31A/internal/ui/tui` → `github.com/eshanized/M31A/internal/ui/tui/core` |
| `internal/ui/tui/app_handlers.go` | `internal/ui/tui/handlers/app_handlers.go` | `tui` → `handlers` | `github.com/eshanized/M31A/internal/ui/tui` → `github.com/eshanized/M31A/internal/ui/tui/handlers` |
| `internal/ui/tui/app_handlers_config.go` | `internal/ui/tui/handlers/app_handlers_config.go` | `tui` → `handlers` | `github.com/eshanized/M31A/internal/ui/tui` → `github.com/eshanized/M31A/internal/ui/tui/handlers` |
| `internal/ui/tui/app_handlers_misc.go` | `internal/ui/tui/handlers/app_handlers_misc.go` | `tui` → `handlers` | `github.com/eshanized/M31A/internal/ui/tui` → `github.com/eshanized/M31A/internal/ui/tui/handlers` |
| `internal/ui/tui/app_handlers_provider.go` | `internal/ui/tui/handlers/app_handlers_provider.go` | `tui` → `handlers` | `github.com/eshanized/M31A/internal/ui/tui` → `github.com/eshanized/M31A/internal/ui/tui/handlers` |
| `internal/ui/tui/app_handlers_tick.go` | `internal/ui/tui/handlers/app_handlers_tick.go` | `tui` → `handlers` | `github.com/eshanized/M31A/internal/ui/tui` → `github.com/eshanized/M31A/internal/ui/tui/handlers` |
| `internal/ui/tui/app_handlers_workflow.go` | `internal/ui/tui/handlers/app_handlers_workflow.go` | `tui` → `handlers` | `github.com/eshanized/M31A/internal/ui/tui` → `github.com/eshanized/M31A/internal/ui/tui/handlers` |
| `internal/ui/tui/app_input_action.go` | `internal/ui/tui/input/app_input_action.go` | `tui` → `input` | `github.com/eshanized/M31A/internal/ui/tui` → `github.com/eshanized/M31A/internal/ui/tui/input` |
| `internal/ui/tui/app_input_resize.go` | `internal/ui/tui/input/app_input_resize.go` | `tui` → `input` | `github.com/eshanized/M31A/internal/ui/tui` → `github.com/eshanized/M31A/internal/ui/tui/input` |
| `internal/ui/tui/app_input_route.go` | `internal/ui/tui/input/app_input_route.go` | `tui` → `input` | `github.com/eshanized/M31A/internal/ui/tui` → `github.com/eshanized/M31A/internal/ui/tui/input` |
| `internal/ui/tui/app_input_theme.go` | `internal/ui/tui/input/app_input_theme.go` | `tui` → `input` | `github.com/eshanized/M31A/internal/ui/tui` → `github.com/eshanized/M31A/internal/ui/tui/input` |
| `internal/ui/tui/app_update.go` | `internal/ui/tui/update/app_update.go` | `tui` → `update` | `github.com/eshanized/M31A/internal/ui/tui` → `github.com/eshanized/M31A/internal/ui/tui/update` |
| `internal/ui/tui/app_update_commands.go` | `internal/ui/tui/update/app_update_commands.go` | `tui` → `update` | `github.com/eshanized/M31A/internal/ui/tui` → `github.com/eshanized/M31A/internal/ui/tui/update` |
| `internal/ui/tui/app_update_phase.go` | `internal/ui/tui/update/app_update_phase.go` | `tui` → `update` | `github.com/eshanized/M31A/internal/ui/tui` → `github.com/eshanized/M31A/internal/ui/tui/update` |
| `internal/ui/tui/app_routing.go` | `internal/ui/tui/routing/app_routing.go` | `tui` → `routing` | `github.com/eshanized/M31A/internal/ui/tui` → `github.com/eshanized/M31A/internal/ui/tui/routing` |
| `internal/ui/tui/app_nav.go` | `internal/ui/tui/routing/app_nav.go` | `tui` → `routing` | `github.com/eshanized/M31A/internal/ui/tui` → `github.com/eshanized/M31A/internal/ui/tui/routing` |
| `internal/ui/tui/app_screens.go` | `internal/ui/tui/routing/app_screens.go` | `tui` → `routing` | `github.com/eshanized/M31A/internal/ui/tui` → `github.com/eshanized/M31A/internal/ui/tui/routing` |
| `internal/ui/tui/app_agent.go` | `internal/ui/tui/core/app_agent.go` | `tui` → `core` | `github.com/eshanized/M31A/internal/ui/tui` → `github.com/eshanized/M31A/internal/ui/tui/core` |
| `internal/ui/tui/app_channel.go` | `internal/ui/tui/core/app_channel.go` | `tui` → `core` | `github.com/eshanized/M31A/internal/ui/tui` → `github.com/eshanized/M31A/internal/ui/tui/core` |
| `internal/ui/tui/app_helpers.go` | `internal/ui/tui/core/app_helpers.go` | `tui` → `core` | `github.com/eshanized/M31A/internal/ui/tui` → `github.com/eshanized/M31A/internal/ui/tui/core` |
| `internal/ui/tui/app_session.go` | `internal/ui/tui/core/app_session.go` | `tui` → `core` | `github.com/eshanized/M31A/internal/ui/tui` → `github.com/eshanized/M31A/internal/ui/tui/core` |
| `internal/ui/tui/handler_config.go` | `internal/ui/tui/handlers/handler_config.go` | `tui` → `handlers` | `github.com/eshanized/M31A/internal/ui/tui` → `github.com/eshanized/M31A/internal/ui/tui/handlers` |
| `internal/ui/tui/handler_modal.go` | `internal/ui/tui/handlers/handler_modal.go` | `tui` → `handlers` | `github.com/eshanized/M31A/internal/ui/tui` → `github.com/eshanized/M31A/internal/ui/tui/handlers` |
| `internal/ui/tui/handler_navigation.go` | `internal/ui/tui/handlers/handler_navigation.go` | `tui` → `handlers` | `github.com/eshanized/M31A/internal/ui/tui` → `github.com/eshanized/M31A/internal/ui/tui/handlers` |
| `internal/ui/tui/handler_runtime.go` | `internal/ui/tui/handlers/handler_runtime.go` | `tui` → `handlers` | `github.com/eshanized/M31A/internal/ui/tui` → `github.com/eshanized/M31A/internal/ui/tui/handlers` |
| `internal/ui/tui/handler_sidebar.go` | `internal/ui/tui/handlers/handler_sidebar.go` | `tui` → `handlers` | `github.com/eshanized/M31A/internal/ui/tui` → `github.com/eshanized/M31A/internal/ui/tui/handlers` |
| `internal/ui/tui/handler_stream.go` | `internal/ui/tui/handlers/handler_stream.go` | `tui` → `handlers` | `github.com/eshanized/M31A/internal/ui/tui` → `github.com/eshanized/M31A/internal/ui/tui/handlers` |
| `internal/ui/tui/handler_tool.go` | `internal/ui/tui/handlers/handler_tool.go` | `tui` → `handlers` | `github.com/eshanized/M31A/internal/ui/tui` → `github.com/eshanized/M31A/internal/ui/tui/handlers` |
| `internal/ui/tui/handler_workflow.go` | `internal/ui/tui/handlers/handler_workflow.go` | `tui` → `handlers` | `github.com/eshanized/M31A/internal/ui/tui` → `github.com/eshanized/M31A/internal/ui/tui/handlers` |

### Test File Moves (Consolidated to tests/)

| Source File | Destination | Notes |
|-------------|-------------|-------|
| `internal/tools/*_test.go` | `internal/tests/tools/*_test.go` | Mirror source structure |
| `internal/ui/tui/*_test.go` | `internal/tests/tui/*_test.go` | Mirror source structure |
| `internal/ui/tui/screens/*_test.go` | `internal/tests/tui/screens/*_test.go` | Keep with screen implementations |

### Import Path Updates Required

**Files importing `internal/tools` (55 files):**
- `cmd/m31a/main.go` → Update to import subpackages
- `internal/engine/workflow/engine.go` → Update to import subpackages
- All tool test files → Update import paths

**Files importing `internal/ui/tui` (223 files):**
- `cmd/m31a/main.go` → Update to import core package
- `cmd/m31a/usage.go` → Update to import core package
- All TUI test files → Update import paths
- All component files → Update import paths

## Risk Assessment

### Circular Import Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Tools subdirectory → root tools | Medium | High | Keep core infrastructure at root, use interfaces |
| TUI core → handlers → core | Low | High | Use interfaces for cross-package communication |
| Test files → source packages | Low | Medium | Keep unit tests with source, move only integration tests |

### Interface Contract Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Tool implementations break interface | Low | High | Compile-time interface checks (`var _ types.Tool = (*Tool)(nil)`) |
| TUI models break tea.Model interface | Low | High | Compile-time interface checks |
| Dispatcher loses tool registration | Low | High | Update defaults.go imports after moves |

### Import Path Update Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Missed import updates | Medium | High | Use `goimports` after all moves |
| Inconsistent import paths | Medium | Medium | Run `make check` after updates |
| External consumer breakage | Low | Medium | Use re-export pattern for backward compatibility |

### Test Consolidation Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Tests lose unexported access | High | Medium | Keep unit tests with source files |
| Test isolation breaks | Medium | Medium | Mirror source structure in tests/ |
| Test failures after move | Medium | High | Run `make test` after each wave |

### Migration Order Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| TUI moves break before tools | Low | Medium | Start with tools (fewer dependencies) |
| Parallel moves cause conflicts | Medium | Medium | Sequential migration, one directory at a time |
| Build breaks during migration | High | Medium | Commit after each successful move batch |

## Verification Strategy

### Pre-Migration Verification

1. **Baseline Tests:** Run `make check` to ensure all tests pass before migration
2. **Import Analysis:** Document all import paths that will change
3. **Interface Verification:** Verify all compile-time interface checks exist

### During Migration Verification

1. **Batch Moves:** Move files in logical groups (git/, todo/, codeanalysis/, network/)
2. **Compile Check:** Run `go build ./...` after each batch
3. **Import Update:** Run `goimports -w .` after import path changes
4. **Test Verification:** Run `make test-fast` after each batch

### Post-Migration Verification

1. **Full Build:** `make build` (optimized binary)
2. **Full Test Suite:** `make test` (race-enabled with coverage)
3. **Lint Check:** `make lint` (golangci-lint)
4. **Format Check:** `make fmt` (gofmt + goimports)
5. **Cross-Platform:** `make cross` (verify all targets compile)

### Wave-Based Verification

**Wave 1: Tools Reorganization**
- Move git/, todo/, codeanalysis/, network/ to subdirectories
- Update imports in defaults.go and dispatcher.go
- Verify: `go build ./internal/tools/...`

**Wave 2: TUI Reorganization**
- Create core/, handlers/, input/, update/, routing/ directories
- Move app_*.go files to appropriate directories
- Verify: `go build ./internal/ui/tui/...`

**Wave 3: Import Path Updates**
- Update all import paths across codebase
- Run `goimports -w .`
- Verify: `make check`

**Wave 4: Test Consolidation**
- Create tests/ directory structure
- Move test files (keeping unit tests with source)
- Verify: `make test`

### Rollback Strategy

1. **Git Backup:** Create branch before migration
2. **Checkpoint Commits:** Commit after each successful wave
3. **Quick Revert:** `git revert` if critical issues found
4. **Selective Rollback:** Revert individual waves if needed

### Pattern 1: Tool Implementation Pattern
**What:** Each tool type gets its own subdirectory with clean interface boundaries
**When to use:** When grouping related tools that share dependencies or domain logic
**Example:**
```go
// Source: internal/tools/git/git.go
package git

import (
    "github.com/eshanized/M31A/internal/core/types"
)

// Git provides structured git operations as a tool
type Git struct {
    workDir string
}

// Compile-time interface check
var _ types.Tool = (*Git)(nil)

func (g *Git) Name() string               { return "Git" }
func (g *Git) RiskLevel() types.RiskLevel { return types.RiskDangerous }
func (g *Git) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
    // Implementation
}
```

### Pattern 2: TUI Responsibility Separation
**What:** Group TUI files by functional responsibility rather than file prefix
**When to use:** When multiple file types serve the same architectural purpose
**Example:**
```go
// Source: internal/ui/tui/core/app.go
package core

import (
    tea "github.com/charmbracelet/bubbletea"
    "github.com/eshanized/M31A/internal/core/types"
)

// AppState is the top-level Bubble Tea model
type AppState struct {
    // State fields
}

// Init implements tea.Model
func (m *AppState) Init() tea.Cmd {
    // Initialization
}

// Update implements tea.Model
func (m *AppState) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
    // Message handling
}

// View implements tea.Model
func (m *AppState) View() string {
    // Rendering
}
```

### Anti-Patterns to Avoid
- **Circular imports:** Never create import cycles between subdirectories
- **God packages:** Avoid putting all tools in one package; use subdirectories
- **Mixed responsibilities:** Don't mix input handling with message processing
- **Breaking interface contracts:** Maintain all existing interfaces during reorganization

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Import path updates | Manual sed replacements | goimports | Handles edge cases and formatting |
| Package organization | Custom scripts | Standard Go conventions | Community expectations |
| Test consolidation | Manual file moves | Go test conventions | Maintains test isolation |
| Interface maintenance | Custom validation | Compile-time checks | Runtime safety |

**Key insight:** Go's package system enforces clean boundaries. Use the compiler to verify correctness after reorganization.

## Runtime State Inventory

> Include this section for rename/refactor/migration phases only. Omit entirely for greenfield phases.

This is a code reorganization phase, not a runtime migration. No runtime state changes are required.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | None — code organization only | None |
| Live service config | None — no external services | None |
| OS-registered state | None — no OS registrations | None |
| Secrets/env vars | None — no secret changes | None |
| Build artifacts | None — will rebuild after moves | `make clean && make build` |

**Nothing found in category:** State explicitly ("None — verified by grep analysis").

## Common Pitfalls

### Pitfall 1: Import Cycle Creation
**What goes wrong:** Creating circular dependencies between subdirectories
**Why it happens:** Moving files without analyzing import chains
**How to avoid:** Map all imports before moving; use `go build ./...` to verify
**Warning signs:** Compiler errors about import cycles

### Pitfall 2: Broken Interface Contracts
**What goes wrong:** Tools no longer implement required interfaces
**Why it happens:** Changing package structure without updating type assertions
**How to avoid:** Keep compile-time interface checks; run `go vet` after moves
**Warning signs:** Runtime panics or test failures

### Pitfall 3: Test Isolation Breakage
**What goes wrong:** Tests can't access unexported functions after moves
**Why it happens:** Moving tests to separate directories without considering access
**How to avoid:** Keep unit tests with source; only move integration tests
**Warning signs:** Test compilation failures

### Pitfall 4: Import Path Scattering
**What goes wrong:** Import paths become inconsistent across the codebase
**Why it happens:** Manual import updates miss some files
**How to avoid:** Use `goimports` after all moves; run `make check`
**Warning signs:** Build failures or inconsistent formatting

## Code Examples

Verified patterns from official sources:

### Tool Registration Pattern
```go
// Source: internal/tools/defaults.go
func DefaultDispatcher(workDir, backupDir, sessionsDir string, cfg *config.PermissionsConfig, toolsCfg *config.ToolsConfig) (*Dispatcher, error) {
    d := newDispatcher(cfg)
    
    // Register fileops tools
    if err := d.Register(fileops.NewFileRead(workDir)); err != nil {
        return nil, err
    }
    
    // Register exec tools
    if err := d.Register(exec.NewBash(workDir, bashMaxTimeoutSecs, additionalBlockedCommands, additionalObfuscationPatterns)); err != nil {
        return nil, err
    }
    
    // Register search tools
    if err := d.Register(search.NewWebFetch(sessionsDir, webfetchMaxRetries, dnsCache)); err != nil {
        return nil, err
    }
    
    // Register AI tools
    if err := d.Register(ai.NewAskUserQuestion(d.questionReqCh, d.questionRespCh, &d.pendingQuestions)); err != nil {
        return nil, err
    }
    
    // Register remaining root tools
    if err := d.Register(NewGit(workDir)); err != nil {
        return nil, err
    }
    
    return d, nil
}
```

### TUI Screen Registration Pattern
```go
// Source: internal/ui/tui/app_routing.go
func (m *AppState) initScreenUpdaters() {
    m.screenUpdaters = make(map[Screen]screenUpdateFunc, 32)
    
    // Register tea.Model-returning screens
    registerTeaModel := func(screen Screen, getPtr func() **any, setFn func(any), updateFn func(tea.Msg) (tea.Model, tea.Cmd)) {
        m.screenUpdaters[screen] = teaUpdate(getPtr, setFn, updateFn)
    }
    
    // Register screens
    registerTeaModel(ScreenHome, func() **any { return &m.homeModel }, func(v any) { m.homeModel = v.(*home.Model) }, m.homeModel.Update)
    registerTeaModel(ScreenRepl, func() **any { return &m.replModel }, func(v any) { m.replModel = v.(*repl.Model) }, m.replModel.Update)
    // ... more screens
}
```

### Import Path Update Pattern
```bash
# After reorganizing tools
find . -name "*.go" -type f -exec sed -i 's|github.com/eshanized/M31A/internal/tools|github.com/eshanized/M31A/internal/tools/git|g' {} \;

# Fix imports with goimports
goimports -w .

# Verify build
go build ./...
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Flat file structure | Subdirectory organization | Phase 3 | Better maintainability |
| File prefix grouping | Responsibility grouping | Phase 3 | Clearer architecture |
| Scattered tests | Consolidated test directory | Phase 3 | Better test organization |
| Manual import updates | Automated goimports | Phase 3 | Reliable path updates |

**Deprecated/outdated:**
- Flat tool directory: All tools in root package → Move to subdirectories
- File prefix organization: app_*.go files → Group by responsibility
- Test files in source directories → Move to tests/ directory

## Assumptions Log

> List all claims tagged `[ASSUMED]` in this research. The planner and discuss-phase use this
> section to identify decisions that need user confirmation before execution.

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | All tool implementations can be moved without breaking interfaces | Standard Stack | Interface contracts may break |
| A2 | Test consolidation won't break existing test patterns | Common Pitfalls | Tests may fail to compile |
| A3 | Import path updates can be automated reliably | Code Examples | Manual updates may miss files |
| A4 | No circular imports will be created | Common Pitfalls | Build will fail |

**If this table is empty:** All claims in this research were verified or cited — no user confirmation needed.

## Open Questions

1. **Which tests need unexported access?**
   - What we know: Some tests access unexported functions
   - What's unclear: Exact list of tests requiring source location
   - Recommendation: Analyze test files before moving to tests/ directory

2. **How to handle backward compatibility?**
   - What we know: Import paths will change
   - What's unclear: Whether to maintain re-exports for external consumers
   - Recommendation: Use tools_reexport.go pattern for temporary compatibility

3. **What's the migration order?**
   - What we know: Multiple directories need reorganization
   - What's unclear: Optimal sequence to minimize breakage
   - Recommendation: Start with tools/ (fewer dependencies), then TUI

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Go | Build system | ✓ | 1.25+ | — |
| make | Build automation | ✓ | — | — |
| goimports | Import organization | ✓ | latest | Manual sed |
| golangci-lint | Code quality | ✓ | latest | go vet |

**Missing dependencies with no fallback:** None

**Missing dependencies with fallback:** None

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Go testing (built-in) |
| Config file | none — uses make targets |
| Quick run command | `make test-fast` |
| Full suite command | `make test` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| TOOLS-01 | Tool subdirectory organization | unit | `go build ./internal/tools/...` | ✅ Wave 0 |
| TOOLS-02 | Core infrastructure at root | unit | `go vet ./internal/tools/` | ✅ Wave 0 |
| TUI-01 | TUI responsibility grouping | unit | `go build ./internal/ui/tui/...` | ✅ Wave 0 |
| TUI-02 | App file splitting | unit | `go vet ./internal/ui/tui/` | ✅ Wave 0 |
| TEST-01 | Test consolidation | unit | `go test ./...` | ✅ Wave 0 |
| IMPORT-01 | Import path updates | integration | `make check` | ✅ Wave 0 |

### Sampling Rate
- **Per task commit:** `make test-fast`
- **Per wave merge:** `make check`
- **Phase gate:** Full suite green before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] Verify all existing tests pass before reorganization
- [ ] Create test directory structure for consolidated tests
- [ ] Update import paths in all affected files

## Security Domain

> Required when `security_enforcement` is enabled (absent = enabled). Omit only if explicitly `false` in config.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | — |
| V3 Session Management | no | — |
| V4 Access Control | no | — |
| V5 Input Validation | yes | Tool input validation via dispatcher |
| V6 Cryptography | no | — |

### Known Threat Patterns for {stack}

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Import path manipulation | Tampering | Compile-time verification |
| Interface contract breaking | Elevation of Privilege | Interface checks |
| Test isolation bypass | Information Disclosure | Proper test organization |

## Sources

### Primary (HIGH confidence)
- Go documentation: Package organization conventions
- Existing codebase analysis: STRUCTURE.md, CONVENTIONS.md
- Tool implementation patterns: internal/tools/defaults.go

### Secondary (MEDIUM confidence)
- Go best practices: Standard project layout
- TUI framework patterns: bubbletea examples

### Tertiary (LOW confidence)
- None — all findings verified against codebase

## Metadata

**Confidence breakdown:**
- Standard Stack: HIGH - Verified against go.mod and existing code
- Architecture: HIGH - Based on existing codebase patterns
- Pitfalls: HIGH - Common Go reorganization issues documented

**Research date:** 2026-07-23
**Valid until:** 2026-08-22 (30 days for stable Go patterns)

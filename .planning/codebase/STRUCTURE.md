---
title: STRUCTURE.md
project: M31A
last_mapped: 2026-07-11
---

# Codebase Structure

## Top-Level Layout

```
M31A/
├── cmd/m31a/              # Binary entry point (main.go, usage.go)
├── internal/              # Private application packages
│   ├── codeintel/         # Code intelligence / indexer
│   ├── config/            # Config loading, merging, types
│   ├── context/           # Dynamic context registry
│   ├── decision/          # Decision logging subsystem
│   ├── errors/            # Typed error vocabulary
│   ├── fileutil/          # File utility helpers
│   ├── git/               # Git wrapper
│   ├── log/               # Logging bootstrapper
│   ├── logging/           # Structured logging helpers
│   ├── provider/          # LLM provider layer (OpenRouter, Zen, Nvidia)
│   ├── shell/             # Shell helpers
│   ├── testutil/          # Internal test utilities
│   ├── tokens/            # Token estimation
│   ├── tools/             # 18 built-in agent tools + dispatcher
│   ├── tui/               # Bubble Tea UI (app, screens, models, views)
│   ├── types/             # Shared type vocabulary
│   ├── wiring/            # Integration/wiring-level tests
│   └── workflow/          # 7-phase workflow engine
├── pkg/                   # Reusable packages (no internal/ imports)
│   ├── arbitrage/         # Model arbitrage / cost scoring
│   ├── autodream/         # Autonomous planning consolidator
│   ├── bisect/            # Binary bisect subsystem
│   ├── compaction/        # Session compaction
│   ├── coordinator/       # Phase coordination helpers
│   ├── history/           # Frecent history tracking
│   ├── keychain/          # OS keychain (Darwin/Linux/Windows)
│   ├── ledger/            # Session record persistence (LEDGER.md)
│   ├── metrics/           # Session observability metrics
│   ├── narrative/         # Narrative engine / story bridge
│   ├── retry/             # Retry policy
│   ├── rollback/          # Git-based rollback
│   ├── session/           # Session manager
│   ├── skills/            # Skills subsystem
│   └── taskrunner/        # Task runner (coverage-critical)
├── docs/                  # 13 developer/user documentation files
├── scripts/               # Shell scripts: validate-release.sh, verify_v1.sh
├── M31A.wiki/             # GitHub wiki mirror
├── .m31a/                 # Runtime config/state directory (gitignored)
├── .planning/             # GSD planning artifacts
├── .github/               # CI/CD workflows
├── Makefile               # Primary build system
├── go.mod / go.sum        # Go module manifest
├── .goreleaser.yaml       # Multi-platform release config
├── .golangci.yml          # Linter config
├── e2e_test.go            # Binary-level E2E test (root package)
├── AGENTS.md              # AI coding agent rules
├── CHANGELOG.md           # Version history
└── layout.test            # Large test layout fixture (~40 MB)
```

## Entry Point

**`cmd/m31a/main.go`** (14.7 KB) — Flag parsing, config loading, provider registration, TUI construction.
**`cmd/m31a/usage.go`** (2.5 KB) — CLI usage/help text.

## Key Internal Packages

### `internal/workflow/` (89 files, ~300 KB)

The heart of the agent. Implements the 7-phase engine.

| File | Role |
|------|------|
| `engine.go` | `Engine` struct, phase dispatch, model-per-phase routing (~1521 lines) |
| `execute.go` | Execute phase: tool orchestration (~36 KB) |
| `plan.go` | Plan phase: generation and refinement (~18 KB) |
| `ship.go` | Ship phase: git commit/PR logic (~20 KB) |
| `discuss.go` | Discuss phase: goal clarification |
| `verify.go` | Verify phase: coverage gates |
| `runtime.go` | Runtime phase: dev server orchestration |
| `initialize.go` | Initialize phase: context setup |
| `state_machine.go` | Phase transition FSM |
| `engine_parse.go` | Tool call parsing from LLM stream (~19 KB) |
| `context_builder.go` | Builds per-phase LLM context |
| `workflow_cache.go` | Caches base prompts and full system prompts |
| `prompts/` | Embedded `.md` prompt templates |
| `templates/` | Embedded website templates (Next.js etc.) |

### `internal/tui/` (152 files, ~500 KB)

Pure Bubble Tea (Elm-style) UI. Every mutation goes through `Update()`.

| File | Role |
|------|------|
| `app.go` | Root model factory, wiring (~23 KB) |
| `app_state.go` | `AppState` struct — single mutable model (~13 KB) |
| `app_update.go` | Master `Update()` dispatch (~25 KB) |
| `app_view.go` | Master `View()` render (~43 KB) |
| `app_update_commands.go` | Command handler routing (~15 KB) |
| `sidebar_model.go` | Sidebar state and rendering (~47 KB) |
| `repl.go` | REPL input loop (~15 KB) |
| `settings_model.go` | Settings screen (~24 KB) |
| `firstrun_model.go` | First-run wizard (~22 KB) |
| `firstrun_view.go` | First-run wizard view (~38 KB) |
| `cmdpalette.go` | Command palette popup (~14 KB) |
| `theme/` | Theme/color manager |
| `components/` | Shared UI components |
| `layout/` | Layout primitives |
| `streaming/` | Streaming text renderer |
| `tuitypes/` | TUI-internal type aliases |

### `internal/tools/` (83 files + `subagent/`)

18 built-in tools registered via `defaults.go`:

| Tool File | Tool Name |
|-----------|-----------|
| `bash.go` | Bash execution (with sandbox) |
| `fileread.go` | FileRead |
| `filewrite.go` | FileWrite |
| `edit.go` | Edit (patch-style) |
| `todo.go` / `todoread.go` | TodoWrite / TodoRead |
| `webfetch.go` | WebFetch |
| `websearch.go` | WebSearch |
| `question.go` | AskUserQuestion |
| `glob.go` | Glob |
| `grep.go` | Grep |
| `filelist.go` | FileList |
| `filedelete.go` | FileDelete |
| `filemove.go` | FileMove |
| `codemap.go` | CodeMap |
| `codecomplexity.go` | CodeComplexity |
| `devserver.go` | DevServer |
| `httpcheck.go` | HTTPCheck |
| `memory.go` | Memory |

`dispatcher.go` (14.5 KB) — Routes calls, enforces permissions, handles rate-limiting and concurrency.
`permissions.go` (14.8 KB) — Permission model with risk levels: safe / medium / dangerous / destructive.
`bash_sandbox_linux.go` (7.2 KB) — Linux seccomp/namespace sandbox for bash.

### `internal/provider/` (26 files)

Three provider implementations in sub-packages:

- `openrouter/` — OpenRouter REST client
- `zen/` — Zen API client
- `nvidia/` — Nvidia API client

Shared:
- `registry.go` — Provider registry
- `capabilities.go` — Model capability detection
- `model_metadata.go` — Dynamic model metadata
- `fallback.go` — Fallback routing on error
- `reasoning.go` — Extended thinking support
- `sse.go` — Server-sent events streaming

### `internal/types/` (11 files)

Shared type vocabulary. `pkg/` must NOT import `internal/`.

Key types in `types.go`:
- `WorkflowPhase` — string enum: idle / initialize / discuss / plan / execute / verify / runtime / ship
- `WorkflowMode` — auto / full / fast / direct
- `RiskLevel` — safe / medium / dangerous / destructive
- `IntentType` — feature / bugfix / refactor / question / explanation / exploration / chore
- `IntentResult` — LLM classifier output struct
- `ComplexityLevel` — trivial / simple / moderate / complex

### `internal/config/` (12 files)

- `types.go` (24 KB) — Top-level `Config` struct with 23 sub-configs (Provider, Model, UI, Permissions, Features, Ledger, Tools, Agents, Git, Verify, Compaction, Instructions, Skills, ModelCapabilities, Prompts, Narrative, Templates)
- `loader.go` (39 KB) — Multi-source config loading, merging, validation
- `merge.go` (19 KB) — Deep config merge logic

## `pkg/` Reusable Packages

| Package | Role |
|---------|------|
| `taskrunner/` | Task scheduling with coverage target ≥90% |
| `bisect/` | Git-based bisect for regression isolation; coverage ≥90% |
| `rollback/` | Git commit rollback; coverage ≥90% |
| `session/` | Session lifecycle management |
| `keychain/` | Platform keychain: Darwin (Keychain), Linux (D-Bus/Secret Service), Windows (DPAPI) |
| `ledger/` | Append-only ledger file (LEDGER.md) |
| `metrics/` | Tool call, LLM usage, phase duration, heal event collection |
| `compaction/` | Proactive + reactive session compaction |
| `arbitrage/` | Model selection cost/quality scoring |
| `autodream/` | Autonomous planning dream/consolidation loop |
| `narrative/` | Narrative engine for session storytelling |
| `history/` | Frecent (frequent+recent) history for prompt recall |
| `coordinator/` | Phase pre/post coordination (side effects, metrics) |
| `retry/` | Configurable retry policy |
| `skills/` | Skills loading and dispatch |

## Naming Conventions

### Files

- **`*_model.go`** — Bubble Tea model struct for a screen or sub-component
- **`*_view.go`** — View rendering for a model
- **`*_handler*.go`** — Message handlers split from main `Update()`
- **`*_test.go`** — Unit tests (co-located with source)
- **`*_extra_test.go`** — Additional/supplemental test cases
- **`coverage_boost_test.go`** — Coverage gap-fill tests (in `internal/tools/`, `internal/workflow/`)
- **`*_benchmark_test.go`** — Go benchmark tests
- **`*_integration_test.go`** — Integration-level tests
- **`*_unix.go` / `*_windows.go`** — OS-specific build tag files
- **`*_darwin.go` / `*_linux.go`** — Platform-specific implementations
- **`engine_*.go`** — Engine sub-files (parse, verify, messages, wiring)
- **`app_*.go`** — AppState sub-files (handlers, nav, routing, session, input, update)

### Packages

- `internal/` — Application-private; never imported by `pkg/`
- `pkg/` — Reusable, dependency-free from `internal/`; can be extracted
- All packages use lowercase snake_case module path segments

### Go Identifiers

- Exported types: `PascalCase` with doc comments required
- Unexported fields: `camelCase`
- Constants: `PascalCase` for exported, `camelCase` for unexported
- Error wrapping: `fmt.Errorf("context: %w", err)` pattern throughout
- No `panic()` in production code (only in test helpers)

## Special Files / Directories

| Path | Purpose |
|------|---------|
| `.m31a/` | Runtime: sessions, tool output, API keys (gitignored) |
| `.planning/` | GSD planning artifacts |
| `layout.test` | Large binary fixture for layout tests (~40 MB) |
| `e2e_test.go` | Root-package binary integration test; skips without real API keys |
| `internal/workflow/templates/` | Embedded Next.js website template (go:embed) |
| `internal/workflow/prompts/` | Embedded LLM prompt templates (go:embed) |
| `scripts/validate-release.sh` | Pre-release validation script |
| `scripts/verify_v1.sh` | V1 feature verification script |
| `.goreleaser.yaml` | Multi-platform release: linux/{amd64,arm64}, darwin/{amd64,arm64}, windows/amd64 |

## Dependency Rule

```
cmd/          → internal/ → pkg/
                           ↑
                    (pkg/ cannot import internal/)
```

Enforced by Go module system. Violation causes compile failure.

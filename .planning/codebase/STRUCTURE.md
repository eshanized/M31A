# Directory Structure — M31A

> Mapped: 2026-07-09

## Top-Level Layout

```
M31A/
  cmd/m31a/           Entry point (2 files)
  docs/               Documentation (13 files)
  internal/           Private packages (18 sub-packages)
  pkg/                Public packages (15 sub-packages)
  scripts/            Shell scripts (2 files)
  .github/            CI/CD, issue templates, funding
  .planning/          GSD workflow artifacts
```

## Entry Point

### `cmd/m31a/`

- `main.go` (~300 lines) — Entry point: flag parsing, config load, provider init, TUI launch
- `usage.go` (~100 lines) — CLI flag definitions and help text

## Core Internal Packages (`internal/`)

### `internal/workflow/` (91 files)

Seven-phase engine. Key files:
- `engine.go` — Core workflow engine
- `phase_coordinator.go` — Phase lifecycle management
- `state_machine.go` — Phase state transitions
- `workflow_cache.go` — Workflow data caching
- `context_builder.go` — Dynamic context assembly
- `cost_tracker.go` — Per-phase cost tracking
- `prompt_builder.go` — Prompt template assembly
- `intent.go` — Intent classification

Per-phase files:
- Phase 1: `initialize.go`, `init_deep.go`
- Phase 2: `discuss.go`, `discuss_check.go`
- Phase 3: `plan.go`, `plan_chunk.go`, `plan_check.go`, `plan_parser.go`
- Phase 4: `execute.go`, `execute_preflight.go`, `execute_quality.go`
- Phase 5: `verify.go`, `verify_report.go`
- Phase 6: `runtime.go`
- Phase 7: `ship.go`, `ship_preflight.go`

Supporting: `agent_switch.go`, `classify.go`, `diff_summary.go`, `retry.go`, `coverage_gates.go`, `thinking_indicator.go` + templates in `prompts/` and `templates/`

### `internal/tui/` (158 files)

Bubble Tea TUI application. Key structure:
- `app.go`, `app_state.go` — Root application
- `app_routing.go` — Screen navigation
- `app_update.go` — Central update handler
- `app_view.go` — Central view handler
- `repl.go`, `repl_view.go`, `repl_model.go` — Main REPL interface
- `sidebar_model.go` — File/session sidebar
- `streaming.go`, `streaming/` — LLM response streaming
- `layout/` (14 files) — Responsive layout engine
- `commands/` (16 files) — Slash command implementations
- `components/` — Reusable UI components
- `theme/` — M31A dark theme
- `a11y/` — Accessibility (reduced motion)
- `tuitypes/` — TUI-specific type definitions

Screen models: `home_model.go`, `dashboard_model.go`, `diff_model.go`, `plan_model.go`, `execute_model.go`, `verify_model.go`, `ship_model.go`, `config_model.go`, `settings_model.go`, `chathistory_model.go`, `ledger_model.go`, `rollback_model.go`, `modelselector_model.go`, `fileexplorer_model.go`, `discuss_model.go`, `runtime_model.go`, `bisect_model.go`, `subagents_model.go`, `ghostpicker_model.go`

Event handlers: `handler_config.go`, `handler_modal.go`, `handler_navigation.go`, `handler_runtime.go`, `handler_sidebar.go`, `handler_stream.go`, `handler_tool.go`, `handler_workflow.go`

### `internal/tools/` (79 files)

18 tools + infrastructure:
- `dispatcher.go` — Tool dispatch with permissions
- `permissions.go` — Permission system (modal, persistent, timeout)
- `concurrency.go` — Rate limiting, semaphore
- `defaults.go` — Tool registration
- `bash.go` (6 files, unix + windows variants)
- `edit.go` (4 files) — 7-strategy cascade edit
- `fileread.go`, `filewrite.go`, `filedelete.go`, `filemove.go`, `filelist.go`
- `glob.go`, `grep.go` (with skip-comments and truncation variants)
- `webfetch.go` (with SSRF protection), `websearch.go`, `httpcheck.go`
- `codemap.go`, `codecomplexity.go`
- `question.go`, `todo.go`, `todoread.go`
- `devserver.go`, `agent.go`, `memory.go`

Subagent infrastructure: `subagent/manager.go`, `worktree.go`, `profile.go`, `loop.go`, `events.go`

### `internal/provider/` (26 files)

- `interface.go` — Provider interface
- `base_client.go` — Shared client infrastructure
- `cache.go` — Model metadata cache
- `fallback.go` — Auto-fallback
- `capabilities.go` — Model capability registry
- `registry.go` — Provider registration
- `sse.go` — SSE streaming
- `reasoning.go` — Extended thinking
- `openrouter/` — OpenRouter provider
- `zen/` — OpenCode Zen provider
- `nvidia/` — Nvidia NIM provider

### `internal/codeintel/` (13 files)

- `codeintel.go` — Main code intelligence
- `parser.go` — Multi-language parser
- `graph.go` — Import dependency graph
- `index.go` — Symbol indexing
- `relevance.go` — File relevance scoring
- `trie.go` — Prefix matching trie
- `cache.go` — Parse result caching

### Other internal packages

| Package | Files | Purpose |
|---------|-------|---------|
| `config/` | 12 | TOML loading, merging, project context |
| `context/` | ~5 | Dynamic context registry, diff notifications |
| `decision/` | ~5 | Decision receipts, ring buffer |
| `errors/` | ~3 | Sentinel errors |
| `fileutil/` | ~3 | Atomic file operations |
| `git/` | ~5 | Git operations |
| `log/` | ~3 | Structured logging |
| `logging/` | ~3 | Audit logging, secret redaction |
| `shell/` | ~3 | Platform shell execution |
| `tokens/` | ~3 | Token estimation, EMA calibration |
| `types/` | 11 | Shared types, constants, plan types |
| `wiring/` | ~3 | Integration/regression tests |
| `testutil/` | ~3 | Test helpers |

## Public Packages (`pkg/`)

| Package | Purpose |
|---------|---------|
| `arbitrage/` | Model-cost optimizer |
| `autodream/` | Context consolidation, reentrancy guard |
| `bisect/` | Git-bisect wrapper |
| `compaction/` | Session compaction, LLM summarization |
| `coordinator/` | Drain session management, coalescing |
| `history/` | Frecent prompt history, scoring |
| `keychain/` (7 files) | OS keychain (Linux/macOS/Windows) |
| `ledger/` | Cross-session learning store |
| `metrics/` | Session metrics, JSON persistence |
| `narrative/` | Event to progress description transformer |
| `retry/` | Exponential backoff, error classification |
| `rollback/` | Commit-chain manager |
| `session/` (15 files) | Session lifecycle, persistence |
| `skills/` | Skill management |
| `taskrunner/` | Kahn's algorithm, bounded parallelism |

## Documentation (`docs/`)

`ARCHITECTURE.md`, `CONFIG.md`, `INTERFACES.md`, `KEYBINDINGS.md`, `ONBOARDING.md`, `PROVIDERS.md`, `QUICKSTART.md`, `SCREENS.md`, `SLASH_COMMANDS.md`, `TOOLS.md`, `TROUBLESHOOTING.md`, `TYPES.md`, `WORKFLOW.md` — 13 files total.

## Infrastructure Files

- `go.mod` / `go.sum` — Go module definition (go 1.25.0)
- `Makefile` — Build orchestration (321 lines)
- `.golangci.yml` — Linter config (5 linters enabled)
- `.goreleaser.yaml` — Release pipeline config
- `.gitignore` — Ignore rules
- `.env.example` — API key template
- `m31a.json` — Scoop manifest
- `e2e_test.go` — End-to-end binary tests
- `install.sh` — One-liner installer
- `CHANGELOG.md` — Release history
- `AGENTS.md` — M31A-specific agent guidance

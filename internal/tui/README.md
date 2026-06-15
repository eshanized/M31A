# internal/tui — M31 Autonomous Terminal UI

This directory implements the Bubble Tea-based terminal user interface for M31 Autonomous.

## Package Organization

```
internal/tui/
├── app.go, app_state.go, app_update*.go, app_view*.go, app_channel.go
│   Core Bubble Tea model (Init/Update/View loop), screen routing, state management
│
├── types.go              # Re-exports from tuitypes (backward compatibility)
├── constants.go          # Layout width thresholds, refresh intervals
├── helpers.go            # Message constructors, session helpers, layout utils
├── truncate.go           # ANSI-aware string truncation
├── transition.go         # Screen transition dim-and-reveal effects
├── health.go             # Health check ticker + sidebar refresh
├── cache_refresh.go      # Model cache refresh ticker
│
├── tuitypes/             # Shared types package
│   └── tuitypes.go       # Screen enum, all message types, WorkflowEngine interface
│
├── streaming/            # LLM streaming pipeline
│   ├── streaming.go      # StartStreamCmd, StreamTickCmd, stream message types
│   └── agent_loop.go     # AgentLoop (autonomous tool-use), BuildAgentMessages
│
├── commands/             # Slash command system
│   ├── commands.go       # CommandInfo, CommandResult, CommandContext, CommandRegistry
│   ├── core.go           # /help, /clear, /status, /reset, /quit, /undo, etc.
│   ├── session.go        # /sessions, /export, /fork, /goal, /resume, /ledger
│   ├── git.go            # /diff, /rollback, /bisect
│   ├── ai.go             # /model, /models, /fallback, /provider, /compress, /memory
│   ├── config.go         # /settings, /config, /theme, /cost, /log, /key, /tokens
│   ├── agent.go          # /agent, /agent-cancel
│   └── workflow.go       # /new, /workflow, /phase, /plan, /execute, /verify, /ship
│
├── components/           # Reusable UI components (41 files)
│   badge, bash_renderer, breadcrumb, card, codeblock, confirm, datatable,
│   divider, dropdown, file_renderers, filetree, filterchips, logo, message,
│   metriccard, notification_list, permission, progress, question, search,
│   sparkline, special_renderers, spinner, splitpane, starfield, statrow,
│   tabbar, taskgraph, thinking, timeline, toolcard, toolrenderers, truncate,
│   workflow_phasebar
│
├── layout/               # Responsive layout system
│   ├── page.go           # PageLayout, PageChrome
│   ├── minscreen.go      # Minimum screen fallback
│   └── responsive.go     # Breakpoint detection, ShowSidebar()
│
├── theme/                # Theme system
│   ├── theme.go          # Theme struct definition
│   ├── colors.go         # Color constants and palettes
│   ├── registry.go       # Theme registry, ByID(), ListThemes()
│   ├── borders.go        # Border style helpers
│   ├── shadow.go         # Shadow effects
│   ├── tabs.go           # Tab styles
│   └── unicode.go        # Box-drawing characters
│
└── [Screen models]       # Full-screen UI models (flat in tui/ for dependency reasons)
    repl_model.go, repl.go, repl_state.go, repl_view.go, repl_welcome.go,
    repl_stream.go, repl_thinking.go, repl_commands.go, repl_clipboard.go,
    repl_quickactions.go, repl_keys.go,
    plan_model.go, plan_view.go, plan_refine.go,
    execute_model.go, execute_view.go,
    verify.go,
    ship_model.go, ship_view.go,
    discuss.go,
    firstrun_model.go, firstrun_view.go,
    resume_model.go, resume_view.go, sessiondetail_model.go,
    settings_model.go, settings_view.go, settings_tabs.go, settings_edit.go,
    config_model.go,
    modelselector.go, modelselector_view.go, modelselector_list.go,
    diff_model.go, diff_view.go,
    bisect_model.go,
    rollback.go,
    ledger.go,
    dashboard_model.go,
    metrics.go,
    goalinput.go,
    phasemodelpicker.go, phasemodelpicker_view.go,
    fileexplorer_model.go,
    tooldetail_model.go,
    themepicker_model.go,
    help.go
```

## Key Design Decisions

### Why are screen models flat (not in sub-packages)?

Go doesn't allow circular imports. Screen models depend on message types (`AppMsg`, `PopScreenMsg`, etc.) defined in `tuitypes`. If screen models were in sub-packages like `screens/repl/`, they'd need to import `tuitypes` — but the core `tui` package also imports `tuitypes` for routing. This would work, but the real blocker is that screen models also depend on shared utilities (`helpers.go`, `constants.go`, `types.go`) that live in the core `tui` package. Moving screens to sub-packages would require those utilities to also move, creating a cascade of changes with diminishing returns.

The flat structure with file naming conventions (`repl_*.go`, `plan_*.go`) provides clear organization while avoiding circular dependency issues.

### Sub-package extraction strategy

Three sub-packages were extracted because they have clean dependency boundaries:

1. **`tuitypes`** — Screen enum + message types + WorkflowEngine interface. No TUI dependencies; only imports `git`, `tools`, `types`, `workflow`, `arbitrage`, `session`.

2. **`streaming`** — LLM streaming pipeline + agent loop. Zero TUI dependencies; fully self-contained with `provider`, `types`, `tools`, `tokens`, `config`.

3. **`commands`** — Slash command handlers. Depends on `tuitypes` for Screen/messages and `config`, `git`, `provider`, `tools`, `session`, `ledger`, `rollback`, `autodream`.

### Backward compatibility

The core `tui` package re-exports all types from `tuitypes`, `streaming`, and `commands` via type aliases and variable aliases. This means existing code within `internal/tui/` continues to work without import changes. New code should import the sub-packages directly.

## File Naming Conventions

| Pattern | Meaning |
|---------|---------|
| `*_model.go` | Model struct definition |
| `*_view.go` | View() rendering |
| `*_extra_test.go` | Additional test coverage |
| `app_*.go` | Core app loop files |
| `repl_*.go` | REPL screen files |
| `commands_*.go` | Command handler files |
| `*_test.go` | Unit tests |

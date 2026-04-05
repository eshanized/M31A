# STRUCTURE.md — Directory Layout & Organization

**Last updated:** 2026-06-13
**Project:** M31A — Terminal AI Coding Agent

## Top-Level Layout

```
M31A/
├── cmd/                    # Entry points
│   ├── m31a/               # Main application binary
│   │   ├── main.go         # CLI entry, dependency wiring, TUI launch
│   │   └── usage.go        # Usage/help formatting
│   └── firstrunpreview/    # First-run preview tool
├── internal/               # Private application packages
│   ├── config/             # TOML config loading, validation, hot-reload
│   ├── errors/             # Sentinel errors, user-friendly messages
│   ├── fileutil/           # Atomic file operations
│   ├── git/                # Git operations wrapper
│   ├── log/                # Structured logging (slog)
│   ├── provider/           # LLM provider interface & implementations
│   │   ├── openrouter/     # OpenRouter API client
│   │   └── zen/            # Zen API client
│   ├── tokens/             # Token estimation engine
│   ├── tools/              # Tool system (dispatcher, tools, perms)
│   │   └── subagent/       # Parallel subagent manager
│   ├── tui/                # Terminal UI (Bubble Tea)
│   │   ├── components/     # Reusable TUI components (30+)
│   │   ├── layout/         # Responsive layout system
│   │   └── theme/          # Theme engine (colors, borders, shadows)
│   ├── types/              # Shared types and constants
│   └── workflow/           # GSD workflow engine (6-phase)
│       └── prompts/        # Embedded prompt templates (*.md)
├── pkg/                    # Public/potentially reusable packages
│   ├── arbitrage/          # Provider cost arbitrage
│   ├── autodream/          # Autonomous task chaining
│   ├── bisect/             # Automated git bisect
│   ├── keychain/           # OS keychain integration
│   ├── ledger/             # Decision tracking ledger
│   ├── rollback/           # Git-based rollback
│   ├── session/            # Session persistence & management
│   └── taskrunner/         # Parallel task execution
├── adrenaline/             # Audit & analysis reports (28 docs)
├── docs/                   # Documentation (usage, commands, troubleshooting)
├── images/                 # Screenshots
├── scripts/                # Build/helper scripts
├── .github/                # GitHub Actions CI/CD
├── go.mod / go.sum         # Go module definition
├── Makefile                # Build/test/lint/release targets
├── .golangci.yml           # Linter configuration
├── .goreleaser.yaml        # Release automation
├── .gitignore              # Git ignore rules
└── install.sh              # Installation script
```

## Key Source File Locations

### Entry Points
| File | Purpose |
|---|---|
| `cmd/m31a/main.go` | Application entry, DI wiring, TUI launch |
| `cmd/m31a/usage.go` | CLI usage text |
| `cmd/firstrunpreview/main.go` | First-run preview |

### Core Config & Types
| File | Purpose |
|---|---|
| `internal/config/types.go` | Config struct definitions |
| `internal/config/loader.go` | TOML loading, validation, hot-reload (1012 lines) |
| `internal/config/project_context.go` | Project config auto-discovery |
| `internal/types/types.go` | Message, ModelInfo, ToolCall, Usage types |
| `internal/types/constants.go` | All shared constants |

### Provider / AI Layer
| File | Purpose |
|---|---|
| `internal/provider/interface.go` | LLMProvider interface definition |
| `internal/provider/registry.go` | Thread-safe provider registry |
| `internal/provider/base_client.go` | HTTP client base, caching, SSE parsing |
| `internal/provider/cache.go` | Model cache layer |
| `internal/provider/capabilities.go` | Model capabilities detection |
| `internal/provider/common.go` | Shared provider utilities |
| `internal/provider/fallback.go` | Auto-fallback between providers |
| `internal/provider/reasoning.go` | Reasoning/thinking support |
| `internal/provider/sse.go` | SSE stream parsing |
| `internal/provider/openrouter/client.go` | OpenRouter API client |
| `internal/provider/zen/client.go` | Zen API client |

### Tool System
| File | Purpose |
|---|---|
| `internal/tools/interface.go` | Permission interfaces |
| `internal/tools/dispatcher.go` | Central tool dispatcher + registry |
| `internal/tools/constants.go` | Tool constants |
| `internal/tools/defaults.go` | Default tool configurations |
| `internal/tools/bash.go` | Bash tool (Unix + Windows impls) |
| `internal/tools/edit.go` | Edit tool |
| `internal/tools/fileread.go` | Read tool |
| `internal/tools/filewrite.go` | Write tool |
| `internal/tools/filedelete.go` | FileDelete tool |
| `internal/tools/filelist.go` | FileList tool |
| `internal/tools/filemove.go` | FileMove tool |
| `internal/tools/glob.go` | Glob tool |
| `internal/tools/grep.go` | Grep tool |
| `internal/tools/webfetch.go` | WebFetch tool |
| `internal/tools/question.go` | Question tool |
| `internal/tools/todo.go` | TodoWrite tool |
| `internal/tools/agent.go` | Agent (subagent) tool |
| `internal/tools/permissions.go` | Permissions manager tool |
| `internal/tools/tooldefs.go` | Tool definition metadata |
| `internal/tools/subagent/manager.go` | Subagent lifecycle management |
| `internal/tools/subagent/loop.go` | Subagent interaction loop |
| `internal/tools/subagent/loop_parse.go` | Subagent tool call parsing |
| `internal/tools/subagent/worktree.go` | Git worktree management |
| `internal/tools/subagent/events.go` | Subagent event aggregation |

### TUI Layer
| File | Purpose |
|---|---|
| `internal/tui/app.go` | Top-level Bubble Tea app model |
| `internal/tui/app_state.go` | Application state management |
| `internal/tui/app_update.go` | Main update handler |
| `internal/tui/app_update_commands.go` | App command generation |
| `internal/tui/app_update_phase.go` | Phase-specific updates |
| `internal/tui/app_view.go` | Main view routing |
| `internal/tui/app_channel.go` | Channel-based message dispatch |
| `internal/tui/repl_model.go` | Main REPL/chat model |
| `internal/tui/repl_view.go` | REPL view rendering |
| `internal/tui/repl_state.go` | REPL state management |
| `internal/tui/repl_stream.go` | Streaming state |
| `internal/tui/repl_commands.go` | REPL command handling |
| `internal/tui/repl_keys.go` | REPL keybindings |
| `internal/tui/repl_thinking.go` | Thinking block rendering |
| `internal/tui/repl_clipboard.go` | Clipboard operations |
| `internal/tui/repl_quickactions.go` | Quick actions panel |
| `internal/tui/repl_welcome.go` | Welcome screen |
| `internal/tui/commands.go` | Command registry |
| `internal/tui/commands_core.go` | Core slash commands |
| `internal/tui/commands_agent.go` | Agent-related commands |
| `internal/tui/commands_ai.go` | AI-related commands |
| `internal/tui/commands_config.go` | Config commands |
| `internal/tui/commands_git.go` | Git commands |
| `internal/tui/commands_session.go` | Session commands |
| `internal/tui/commands_workflow.go` | Workflow commands |
| `internal/tui/agent_loop.go` | LLM agent interaction loop |
| `internal/tui/header.go` | App header |
| `internal/tui/statusbar.go` | Status bar |
| `internal/tui/sidebar.go` | Sidebar component |
| `internal/tui/helpers.go` | TUI helper utilities |
| `internal/tui/constants.go` | TUI-specific constants |
| `internal/tui/types.go` | TUI-type Message types |

### TUI Components
| File | Purpose |
|---|---|
| `internal/tui/components/message.go` | Message renderer |
| `internal/tui/components/file_renderers.go` | File content renderers |
| `internal/tui/components/special_renderers.go` | Special content rendering |
| `internal/tui/components/toolrenderers.go` | Tool call renderers |
| `internal/tui/components/bash_renderer.go` | Bash output renderer |
| `internal/tui/components/codeblock.go` | Code block rendering |
| `internal/tui/components/thinking.go` | Thinking block component |
| `internal/tui/components/toolcard.go` | Tool card component |
| `internal/tui/components/spinner.go` | Loading spinner |
| `internal/tui/components/progress.go` | Progress bar |
| `internal/tui/components/badge.go` | Badge component |
| `internal/tui/components/logo.go` | Logo animation |
| `internal/tui/components/starfield.go` | Starfield animation |
| `internal/tui/components/timeline.go` | Timeline view |
| `internal/tui/components/taskgraph.go` | Task dependency graph |
| `internal/tui/components/datatable.go` | Data table component |
| `internal/tui/components/dropdown.go` | Dropdown selector |
| `internal/tui/components/filterchips.go` | Filter chips |
| `internal/tui/components/card.go` | Card component |
| `internal/tui/components/divider.go` | Divider component |
| `internal/tui/components/breadcrumb.go` | Breadcrumb nav |
| `internal/tui/components/tabbar.go` | Tab bar |
| `internal/tui/components/statrow.go` | Statistics row |
| `internal/tui/components/metriccard.go` | Metric card |
| `internal/tui/components/truncate.go` | Text truncation |
| `internal/tui/components/search.go` | Search input |
| `internal/tui/components/confirm.go` | Confirmation dialog |
| `internal/tui/components/permission.go` | Permission modal |
| `internal/tui/components/question.go` | Question panel |
| `internal/tui/components/notification_list.go` | Notifications list |
| `internal/tui/components/filetree.go` | File tree component |
| `internal/tui/components/sparkline.go` | Sparkline chart |
| `internal/tui/components/workflow_phasebar.go` | Workflow phase indicator |
| `internal/tui/components/splitpane.go` | Split pane layout |

### TUI Theme & Layout
| File | Purpose |
|---|---|
| `internal/tui/theme/theme.go` | Theme definition |
| `internal/tui/theme/colors.go` | Color definitions |
| `internal/tui/theme/registry.go` | Theme registry |
| `internal/tui/theme/borders.go` | Border styles |
| `internal/tui/theme/shadow.go` | Shadow effects |
| `internal/tui/theme/tabs.go` | Tab styles |
| `internal/tui/theme/unicode.go` | Unicode characters |
| `internal/tui/layout/responsive.go` | Responsive layout |
| `internal/tui/layout/page.go` | Page management |
| `internal/tui/layout/minscreen.go` | Minimum screen size |

### Workflow Engine
| File | Purpose |
|---|---|
| `internal/workflow/engine.go` | Core workflow engine (828 lines) |
| `internal/workflow/engine_messages.go` | Workflow message types |
| `internal/workflow/engine_parse.go` | Tool call parsing in workflow |
| `internal/workflow/engine_verify.go` | Verification logic |
| `internal/workflow/discuss.go` | Discuss phase implementation |
| `internal/workflow/plan.go` | Plan phase implementation |
| `internal/workflow/plan_parser.go` | Plan parsing from LLM output |
| `internal/workflow/execute.go` | Execute phase implementation |
| `internal/workflow/verify.go` | Verify phase implementation |
| `internal/workflow/ship.go` | Ship phase implementation |
| `internal/workflow/initialize.go` | Initialization phase |

### Domain Packages
| File | Purpose |
|---|---|
| `pkg/arbitrage/arbitrage.go` | Cost-based model selection |
| `pkg/autodream/autodream.go` | Autonomous task chaining |
| `pkg/bisect/bisect.go` | Automated git bisect |
| `pkg/bisect/exec.go` | Bisect command execution |
| `pkg/keychain/keychain.go` | Keychain interface |
| `pkg/keychain/keychain_darwin.go` | macOS implementation |
| `pkg/keychain/keychain_linux.go` | Linux (Secret Service) impl |
| `pkg/keychain/keychain_windows.go` | Windows implementation |
| `pkg/ledger/ledger.go` | Decision ledger |
| `pkg/rollback/rollback.go` | Git rollback |
| `pkg/session/session.go` | Session data model |
| `pkg/session/manager.go` | Session lifecycle manager |
| `pkg/session/session_info.go` | Session metadata |
| `pkg/session/checkpoint.go` | Checkpoint management |
| `pkg/session/planning.go` | Planning session data |
| `pkg/taskrunner/runner.go` | Concurrent task runner |

## Naming Conventions

- **Packages:** lowercase, single word where possible (e.g., `config`, `tui`, `workflow`)
- **Files:** `snake_case.go` matching package purpose
- **Types:** PascalCase exported, camelCase unexported
- **Test files:** `*_test.go` alongside source; `extra_test.go` for integration tests
- **Platform-specific files:** `*_unix.go`, `*_windows.go`, `*_darwin.go`, `*_linux.go` suffixes

## File Size Distribution

- **Large files (>500 lines):** `internal/config/loader.go` (1012), `internal/workflow/engine.go` (828), `internal/tui/repl_stream.go`, `internal/tui/app_update.go`, `internal/tui/repl_view.go`
- **Medium files (100-500 lines):** Most tool implementations, TUI models, workflow phases
- **Small files (<100 lines):** Interface definitions, doc files, helper utilities

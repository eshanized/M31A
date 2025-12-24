# M31A — Agent Instructions

## Project Overview

M31A is a Go 1.24+ terminal AI coding agent.

- Module: github.com/eshanized/M31A
- Binary: CGO_ENABLED=0 static binary
- UI: charmbracelet/bubbletea + charmbracelet/lipgloss + charmbracelet/bubbles + charmbracelet/glamour

## Build & Test Commands

```
Build: CGO_ENABLED=0 go build -o m31a ./cmd/m31a
Test:  go test -race -cover ./...
Lint:  golangci-lint run ./...
Vet:   go vet ./...
Clean: rm -f m31a
```

## Architecture Rules (CRITICAL — agent must follow these)

- Bubble Tea is single-threaded. ALL state mutations go through Update() only.
  Never mutate AppState from a goroutine. Use tea.Cmd and tea.Msg.
- HTTP client: 30s dial timeout via DialContext. NO body read timeout (streaming).
- Context pruning: each workflow phase discards prior conversation; reads state from
  planning/ files only.
- No direct Anthropic/OpenAI connections. Only OpenRouter and Zen gateways.
- No CGO. Binary must be static (CGO_ENABLED=0).
- No telemetry. No analytics. No external calls except to OpenRouter/Zen APIs.
- API keys: resolved in order: env var -> OS keychain -> config file. Never plaintext.
- V1 tools (core): Bash, FileRead, FileWrite, Glob, Grep, Edit, TodoWrite, WebFetch, AskUserQuestion.
  Additional tools: FileList, FileDelete, FileMove.
  AskUserQuestion is available for interactive sessions but MUST NOT be
  used in automated task execution flows.
- V1 task execution is SEQUENTIAL. The dispatcher handles permission
  prompts synchronously. Do not implement concurrency beyond this.
- No hardcoded model lists. Models discovered dynamically from provider APIs.

## Package Layout

```
cmd/m31a/          -- binary entry point only, no logic
internal/config/   -- config parsing, env vars, keychain resolution
internal/provider/ -- LLMProvider interface + OpenRouter + Zen clients
internal/tui/      -- Bubble Tea app, all screens
internal/workflow/ -- six workflow phases
internal/tools/    -- Bash, FileRead, FileWrite, Glob, Grep + dispatcher
internal/log/      -- structured logger (slog) with rotation
internal/git/      -- git operations wrapper
internal/types/    -- shared core types (Message, Task, ToolCall, etc.)
internal/tokens/   -- token estimation for LLM cost tracking
pkg/taskrunner/    -- dependency graph, topological sort, task lifecycle
pkg/arbitrage/     -- complexity scoring, model cost comparison
pkg/bisect/        -- git bisect wrapper
pkg/ledger/        -- cross-session learning ledger
pkg/rollback/      -- commit chain browser
pkg/autodream/     -- context consolidation
pkg/session/       -- session lifecycle, file persistence
pkg/keychain/      -- OS-specific keychain (linux/darwin/windows)
```

## Absolute Prohibitions

- DO NOT add direct Anthropic or OpenAI provider support
- DO NOT use CSS-style animations (Bubble Tea uses Unicode spinners + frame redraws)
- DO NOT implement V1.1 features (ghost mode, PiP, subagents, deferred tools)
- DO NOT store API keys in plaintext
- DO NOT add telemetry or analytics
- DO NOT hardcode model lists
- DO NOT use AskUserQuestion in automated task execution flows
- DO NOT add new tools beyond the current set without explicit request

## File Paths That Always Matter

- Config: ~/.m31a/config.toml
- Sessions: ~/.m31a/sessions/<id>/
- Ledger: ~/.m31a/LEDGER.md
- Global AGENTS: ~/.config/opencode/AGENTS.md

## Key Go Dependencies (use only these)

```
charmbracelet/bubbletea    -- TUI framework
charmbracelet/lipgloss     -- terminal styling
charmbracelet/bubbles      -- TUI components (textarea, viewport, spinner, list)
charmbracelet/glamour      -- Markdown rendering
BurntSushi/toml            -- config parsing
tiktoken-go                -- token estimation for GPT/Claude families
doublestar                 -- glob pattern matching with ** support
creack/pty                 -- PTY for Bash tool on Linux/macOS
```

# REQUIREMENTS.md — M31A

## Project Scope

M31A is a terminal-based AI coding agent that guides development tasks through a structured seven-phase workflow. The system combines a Bubble Tea TUI for interactive use with headless CLI modes for automation.

## Functional Requirements

### FR-1: TUI Interface
- **FR-1.1**: First-run wizard guides API key setup for 3 providers (OpenRouter, Zen, Nvidia)
- **FR-1.2**: Home screen with logo, prompt input, suggestions, tips
- **FR-1.3**: REPL chat interface with streaming responses, slash commands, history
- **FR-1.4**: Sidebar with git status, file tree, todos, metrics (toggleable)
- **FR-1.5**: Screen routing: Home, REPL, Plan, Execute, Verify, Runtime, Ship, Settings, ModelSelector, Resume, Dashboard, etc.
- **FR-1.6**: Visual phase transitions with slide/fade animations
- **FR-1.7**: Permission modals for tool execution (bash, write, etc.)
- **FR-1.8**: Theme system (dark only, no light/auto)

### FR-2: Seven-Phase Workflow Engine
- **FR-2.1**: Initialize — gather project context, environment info
- **FR-2.2**: Discuss — clarify requirements via Q&A with user
- **FR-2.3**: Plan — generate structured task plan with dependencies
- **FR-2.4**: Execute — implement tasks using tools with self-healing
- **FR-2.5**: Verify — run tests, validate correctness
- **FR-2.6**: Runtime — dev server lifecycle + smoke tests
- **FR-2.7**: Ship — commit changes, finalize session
- **FR-2.8**: Phase transitions with user confirmation
- **FR-2.9**: Checkpoint/resume at phase boundaries
- **FR-2.10**: Dual-model support (planning vs coding models)

### FR-3: LLM Provider Layer
- **FR-3.1**: OpenRouter, Zen, Nvidia providers with dynamic model discovery
- **FR-3.2**: Streaming chat completions with token usage tracking
- **FR-3.3**: Model capability detection (tools, streaming, context window)
- **FR-3.4**: Automatic fallback on provider errors
- **FR-3.5**: API keys stored in OS keychain, never plaintext

### FR-4: Tools & Permissions
- **FR-4.1**: 18 built-in tools (bash, read, write, edit, glob, grep, task, etc.)
- **FR-4.2**: Permission system with allow/deny/always modes
- **FR-4.3**: Rate limiting and concurrency control
- **FR-4.4**: Subagent support — parallel child agents with isolated worktrees

### FR-5: Session Management
- **FR-5.1**: Project-local sessions in `.m31a/sessions/`
- **FR-5.2**: Message history, workflow state, checkpoints persisted
- **FR-5.3**: Resume from any phase with full context restoration
- **FR-5.4**: Session browser with search, export, rename

### FR-6: Headless Modes
- **FR-6.1**: `--prompt` — single LLM query, print response
- **FR-6.2**: `--goal` — full workflow without TUI
- **FR-6.3**: `--model` — override model for headless modes

## Non-Functional Requirements

### NFR-1: Performance
- Cold start < 2s on modern hardware
- TUI frame render < 16ms (60fps)
- Memory < 200MB baseline

### NFR-2: Reliability
- Zero data loss on crash (session autosave)
- Graceful degradation when providers unavailable
- Static binary — no runtime dependencies

### NFR-3: Security
- API keys only in OS keychain
- No telemetry without explicit opt-in
- Tool permissions default to ask

### NFR-4: Maintainability
- 75% test coverage overall, 90% for critical packages
- All code gofmt-clean, passes golangci-lint
- Conventional commits

## Acceptance Criteria

### AC-1: First Run Experience
```bash
$ m31a
# Shows FirstRun wizard → select provider → enter API key → pick model → done
# Then shows Home screen ready for input
```

### AC-2: Interactive Workflow
```bash
$ m31a
# Type "Fix the login bug" → Enter
# Goes through Discuss → Plan → Execute → Verify → Runtime → Ship
# Each phase shows progress, allows user input at decision points
```

### AC-3: Headless Workflow
```bash
$ m31a --goal "Add user authentication" --model "anthropic/claude-3.5-sonnet"
# Runs all 7 phases without TUI, prints phase summaries
# Exits 0 on success, 1 on failure
```

### AC-4: Session Resume
```bash
$ m31a
# If previous session exists and ResumeOnStartup=true
# Shows Resume screen → pick session → restores exact state
```

### AC-5: Blank Screen Fix
```bash
$ m31a
# No blank screens at any point
# All screens render content within 100ms of navigation
# Dimensions handled correctly on resize
```

## Out of Scope (v1)
- Light/auto theme support
- Windows ARM64
- Plugin system / custom tools
- Multi-repo workspaces
- Web UI / remote access
- Team collaboration features
- Built-in code review automation
EOF
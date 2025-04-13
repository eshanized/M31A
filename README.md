# M31A — Terminal AI Coding Agent

M31A is a terminal-based AI coding assistant written in Go. It uses a six-phase workflow to understand, plan, execute, verify, and ship code changes — all from your terminal.

## Features

- **Six-phase workflow**: Initialize → Discuss → Plan → Execute → Verify → Ship
- **Dual provider support**: OpenRouter and OpenCode Zen gateways with auto-fallback on failure
- **10-screen Bubble Tea TUI** with dark/light themes and keyboard navigation
- **5 core tools**: Bash, FileRead, FileWrite, Glob, Grep
- **Model selector** with fuzzy search and cost comparison
- **Cross-session learning ledger** for pattern tracking across sessions
- **Commit rollback chain** with git bisect integration
- **AutoDream** context consolidation for long conversations
- **Model arbitrage** for cost optimization
- **Session persistence** and resume
- **OS keychain integration** (Linux/macOS/Windows)
- **16 slash commands** for session, provider, and workflow control

## Quick Start

### Build from source

```bash
git clone https://github.com/eshanized/M31A.git
cd M31A
CGO_ENABLED=0 go build -o m31a ./cmd/m31a
./m31a
```

### Install via Homebrew (macOS)

```bash
brew install eshanized/tap/m31a
m31a
```

### Install via script (Linux/macOS)

```bash
curl -fsSL https://raw.githubusercontent.com/eshanized/M31A/main/install.sh | bash
```

### First Run

On first launch, M31A prompts for your API key (OpenRouter or Zen). Keys are stored in your OS keychain — never in plaintext. Set `M31A_CONFIG` to use a custom config path.

## Six-Phase Workflow

| Phase | Description |
|-------|-------------|
| **Initialize** | Detects project type, reads file tree, initializes git, creates session |
| **Discuss** | Asks clarifying questions about requirements and constraints |
| **Plan** | Generates a task graph with dependencies, files, and acceptance criteria |
| **Execute** | Runs tasks in dependency order, dispatching tools and committing changes |
| **Verify** | Validates outputs, triggers self-heal or bisect on failure |
| **Ship** | Final commit, ledger entry, session archive |

Start a workflow with `/workflow <your goal>` from the REPL.

## Slash Commands

| Command | Description |
|---------|-------------|
| `/help` | List all available commands |
| `/clear` | Clear current conversation |
| `/status` | Show current session info |
| `/model <name>` | View model info or open selector with `--selector` |
| `/provider [name]` | Show or switch provider |
| `/reset` | Return to first-run screen |
| `/quit` | Exit the application |
| `/undo` | Show latest checkpoint |
| `/compress` | Trigger AutoDream context consolidation |
| `/ledger [stats\|<type>]` | Show session history or statistics |
| `/rollback [<hash>]` | Show commit chain or reset to commit (`--hard`) |
| `/sessions` | List recent sessions |
| `/goal <text>` | Set or show session goal |
| `/phase [<name>]` | Show or transition workflow phase |
| `/config [key value]` | Show or set configuration |
| `/models` | List all cached models |
| `/fallback [<name>]` | Show or switch provider fallback |

Full reference: [docs/SLASH_COMMANDS.md](docs/SLASH_COMMANDS.md)

## Key Bindings

| Key | Screen | Action |
|-----|--------|--------|
| `Ctrl+C` | All | Cancel stream (if streaming), then exit on second press |
| `Enter` | REPL | Send message |
| `Esc` | Model selector | Exit selector |
| `y/a/n/e` | Permission modal | Allow / Allow always / Deny / Exit |
| `x` | Fallback banner | Dismiss notification |

## Configuration

Config file: `~/.m31a/config.toml`

```toml
[provider]
default = "openrouter"
auto_fallback = true

[provider.openrouter]
api_key = ""

[provider.zen]
api_key = ""

[model]
default = ""
context_warning_threshold = 0.8
show_thinking_by_default = false
auto_collapse_tools = true
auto_arbitrage = false
arbitrage_threshold = 0.1

[ui]
theme = "dark"
compact_mode = false
show_token_usage = true
show_cost_estimate = true
max_iterations = 100

[permissions]
default_mode = "ask"
timeout_seconds = 300

[features]
autodream_enabled = true
subagent_enabled = false
auto_backup = true
resume_on_startup = false

[ledger]
enabled = true
max_entries = 100

[ghost]
enabled = false
```

Full reference: [docs/CONFIG.md](docs/CONFIG.md)

## Architecture

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for package dependency graph, data flows, threading model, and error handling strategy.

## Project Status

**v1.0.0** — Core feature complete. V1.1 features (ghost mode, PiP, subagents, deferred tools) are planned for future releases.

## License

MIT

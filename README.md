# M31A

<p align="center">
  <img src="https://img.shields.io/badge/M31A-Autonomous_Agentic_Coding-blue?style=for-the-badge&logo=rust" alt="M31A Logo" />
</p>

<p align="center">
  <a href="https://github.com/eshanized/M31A/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/eshanized/M31A/ci.yml?branch=master&label=Rust%20CI&style=flat-square" alt="Rust CI" /></a>
  <a href="https://github.com/eshanized/M31A/actions/workflows/bazel.yml"><img src="https://img.shields.io/github/actions/workflow/status/eshanized/M31A/bazel.yml?branch=master&label=Bazel%20CI&style=flat-square" alt="Bazel CI" /></a>
  <a href="https://opensource.org/licenses/MIT"><img src="https://img.shields.io/badge/License-MIT-green.svg?style=flat-square" alt="License: MIT" /></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-2024%20Edition-orange.svg?style=flat-square&logo=rust" alt="Rust 2024" /></a>
  <a href="https://build.nvidia.com/"><img src="https://img.shields.io/badge/Powered%20By-NVIDIA%20NIM-76B900.svg?style=flat-square&logo=nvidia" alt="NVIDIA NIM" /></a>
  <a href="https://m31a.tonmoyinfrastructure.org/"><img src="https://img.shields.io/badge/Docs-m31a.tonmoyinfrastructure.org-blue.svg?style=flat-square" alt="Documentation" /></a>
</p>

```
                 __  __ _____  _    _        
                |  \/  |___ / / |  / \       
                | |\/| | |_ \ | | / _ \      
                | |  | |___) || |/ ___ \     
                |_|  |_|____/ |_/_/   \_\    
```

**M31A** is a high-performance, autonomous agentic coding assistant inspired by OpenAI Codex CLI, rebuilt from first principles as a modular 39-crate Rust workspace and powered by [NVIDIA NIM](https://build.nvidia.com/) frontier intelligence.

- **Author**: Eshan Roy
- **License**: MIT
- **Documentation**: [https://m31a.tonmoyinfrastructure.org/](https://m31a.tonmoyinfrastructure.org/)
- **Repository**: [https://github.com/eshanized/M31A](https://github.com/eshanized/M31A)

---

## Table of Contents

- [Core Capabilities & Codex Parity](#core-capabilities--codex-parity)
- [Quickstart Guide](#quickstart-guide)
  - [1. Prerequisites](#1-prerequisites)
  - [2. Build & Install](#2-build--install)
  - [3. Configure NVIDIA NIM Credentials](#3-configure-nvidia-nim-credentials)
  - [4. Verify System Health](#4-verify-system-health)
  - [5. Interactive TUI Mode](#5-interactive-tui-mode)
- [TUI Keyboard Shortcuts & Slash Commands](#tui-keyboard-shortcuts--slash-commands)
- [CLI Command Reference](#cli-command-reference)
- [SDK Ecosystem (TypeScript & Python)](#sdk-ecosystem-typescript--python)
- [Security & Sandboxing Architecture](#security--sandboxing-architecture)
- [Workspace Monorepo Layout (39 Crates)](#workspace-monorepo-layout-39-crates)
- [Testing & Quality Verification](#testing--quality-verification)
- [Documentation Index](#documentation-index)
- [Contributing](#contributing)
- [Security Policy](#security-policy)
- [License](#license)

---

## Core Capabilities & Codex Parity

- **Interactive Ratatui/Crossterm TUI (`m31a-tui`)**: Full terminal user interface featuring a live streaming transcript view, collapsible DeepSeek R1 reasoning paths, rich syntax highlighting for file diffs and tool cards, multiline prompt composer with history navigation, live slash commands autocompletion, model switcher modal (`Ctrl+M`), fuzzy palette (`Ctrl+P`), approval dialogs, and SQLite session resume picker (`Ctrl+R`).
- **NVIDIA NIM Frontier Intelligence (`m31a-provider`)**: Dynamic model discovery querying `/v1/models` and integration with models on [build.nvidia.com](https://build.nvidia.com/):
  - `nvidia/nemotron-3.5-lightning-30b-a3b` (Default frontier workhorse with reasoning support)
  - `nvidia/nemotron-3.5-bolt-e4b` (Ultra-low latency efficient instruction following)
  - `deepseek-ai/deepseek-v4.1-flash` (Advanced frontier reasoning with thinking stream support)
  - `mistralai/codestral-22b-instruct-v0.1` (Code specialization)
  - `nvidia/llama-3.1-nemotron-70b-instruct` (High tool-calling precision)
  - Support for any custom or remote NIM model slug without restriction.
- **Autonomous Flow-State & Vibe Coding Engine (`m31a-agent-engine::vibe_loop` & `m31a vibe`)**:
  - Continuous non-intrusive workspace filesystem watcher (`m31a-watcher`) with configurable debouncing.
  - Zero-babysitter auto-healing loop: continuously verifies builds in the background, detects compiler/type errors, synthesizes targeted corrective multi-file Codex patches via low-latency NVIDIA NIM models, and automatically repairs workspace state without breaking user flow.
- **Speculative Parallel Reality Arena (`m31a-agent-engine::speculative` & `m31a speculate`)**:
  - Concurrently evaluates competing solution hypotheses in isolated temporary sandbox workspaces.
  - Generates composite multi-criteria scoring matrix across compiler checks, full test suites, execution speed, and blast-radius safety.
  - Automatically selects and promotes the winning candidate branch directly into the working tree.
- **Autonomous Visual TDD Engine (`m31a-verify::visual_tdd` & `m31a-browser`)**:
  - Headless Chromium CDP automation validating live web frontends: element presence, visibility, exact text match, computed CSS styles, and attribute states.
  - Automatic inspection for zero console errors, zero HTTP 4xx/5xx network failures, and visual screenshot artifact capture.
  - Formulates actionable self-healing guidance for UI regressions.
- **Codebase Safety & Semantic Blast-Radius Gate (`m31a-verify::safety` & `m31a-ast`)**:
  - Full AST symbol cross-referencing and transitive call-graph blast radius score (0-100).
  - Intra-procedural data-flow taint tracking detecting unconsumed mutations and untrusted inputs flowing into sensitive sinks.
- **Codex Multi-File Patch Engine (`m31a-patch` & `m31a-tools`)**:
  - Full support for the OpenAI Codex patch specification (`*** Begin Patch`, `*** Add File:`, `*** Delete File:`, `*** Update File:`, `*** Move to:`, `@@ context`, `*** End Patch`).
  - 4-tier fuzzy match engine (`seek_sequence`): exact match, right-strip whitespace, full trim, and unicode typographic normalization (hyphens, typographic quotes, non-breaking spaces).
- **18-Tool Orchestration Suite (`m31a-tools`)**:
  - Execution: `exec_command`, `write_stdin`
  - Filesystem: `read_file`, `write_file`, `apply_patch`, `list_dir`, `file_search`, `view_image`
  - Interactive & Extensibility: `request_user_input`, `tool_search`
  - Multi-Agent Orchestration: `spawn_agent`, `send_input`, `wait_agent`, `close_agent`, `list_agents`
  - Mission Goal Tracking: `create_goal`, `update_goal`, `get_goal`
- **Code Mode & Tool Orchestration (`m31a-code-mode`)**:
  - `exec` and `wait` tool runtime.
  - Script pragmas (`// @exec: {"yield_time_ms": 10000, "max_output_tokens": 2000}`).
  - Global helpers: `text(val)`, `store(key, val)`, `load(key)`, `yield_control()`.
  - Nested tool invocation via `await tools.<name>(args)`.
- **Bubblewrap Sandbox (`m31a-sandbox`)**:
  - Linux namespace isolation (PID, IPC, UTS, Mount) with read-only root system mounts and writable workspace binds.
  - Read-only protection masks over sensitive subpaths (`.git`, `.m31a`, `.codex`, `.ssh`, `.env`, `*.key`, `*.pem`).
  - Network namespace isolation (`--unshare-net`) and WSL interop protection.
- **Starlark/Prefix ExecPolicy (`m31a-execpolicy`)**:
  - Prefix pattern rules with alternative tokens (`PatternToken::Alts`).
  - Network destination filtering rules (host, protocol, port).
  - Decision classification into `allow`, `prompt`, or `deny` with detailed justifications.
- **Guardian Safety Reviewer (`m31a-guardian`)**:
  - Autonomous supervisor evaluating tool operations, destructive shell commands, and patch mutations.
  - Sliding-window circuit breaker (`AUTO_REVIEW_DENIAL_WINDOW_SIZE = 5`, `MAX_DENIALS_BEFORE_TRIP = 3`).
- **Skills Subsystem (`m31a-core::skills`)**:
  - Discovers skills from `~/.m31a/skills`, `.m31a/skills`, `~/.codex/skills`, and `.codex/skills`.
  - Parses YAML frontmatter (`name`, `description`) and renders instruction blocks into agent context.
- **JSON-RPC 2.0 App Server Daemon (`m31a-app-server`)**:
  - Implements complete JSON-RPC 2.0 protocol over `stdio`, Unix Domain Socket, and HTTP/SSE for IDE extensions (VS Code, Cursor) and automation.
  - Supports 40+ methods: `thread/*`, `turn/*`, `command/*`, `model/*`, `skills/*`, `config/*`, `server/diagnostics`.
- **Durable Storage & Agent Message Board (`m31a-storage`)**:
  - Persistent SQLite database (`threads`, `thread_turns`, `thread_items`, `thread_spawn_edges`, `thread_goals`, `channels`, `posts`, `subscriptions`).
  - Supports branching/forking sessions, thread archival, token accounting, and multi-agent pub-sub communication.

---

## Quickstart Guide

### 1. Prerequisites

- **Rust**: Version 1.85+ (Rust 2024 edition).
- **Bubblewrap** (Linux only): `sudo apt install bubblewrap` (or `dnf`, `pacman`).
- **NVIDIA NIM API Key**: Free API key from [build.nvidia.com](https://build.nvidia.com/).

### 2. Build & Install

```bash
# Clone the repository
git clone https://github.com/eshanized/M31A.git
cd M31A

# Build and install m31a binary
cargo install --path crates/m31a-cli

# Or build release binary directly
cargo build --release -p m31a-cli
cp target/release/m31a ~/.local/bin/
```

*Alternatively, build with Bazel 7*:
```bash
bazel build //bazel:m31a
```

### 3. Configure NVIDIA NIM Credentials

Export the key in your shell:
```bash
export NVIDIA_API_KEY="nvapi-..."
```

Or run the interactive onboarding wizard:
```bash
m31a setup
```

### 4. Verify System Health

Run the diagnostic healthcheck:
```bash
m31a doctor
```

### 5. Interactive TUI Mode

Launch the full-screen terminal user interface:
```bash
# Launch interactive TUI directly
m31a

# Launch TUI pre-populated with a task
m31a "Inspect the codebase, run tests, and fix any failed assertions"
```

---

## TUI Keyboard Shortcuts & Slash Commands

### Keyboard Shortcuts
| Shortcut | Action |
| :--- | :--- |
| `Ctrl + P` | Open Universal Fuzzy Command Palette (quick tool & command launcher) |
| `Tab` / `Shift+Tab` | Cycle Agent Mode (`[⚡ BUILD]` autonomous vs `[📋 PLAN]` advisory) |
| `!` (prefix) | Instant local shell execution (e.g. `!cargo check`) with output injection |
| `Enter` | Submit prompt / complete command |
| `Shift+Enter` | Insert newline in composer |
| `Ctrl + M` | Open NVIDIA NIM Model Selector modal |
| `Ctrl + D` | Open interactive Git diff viewer |
| `Ctrl + R` | Open Session Picker (resume past conversations) |
| `Ctrl + Y` | Extract and copy latest assistant code block to clipboard |
| `Ctrl + T` | Fold / unfold tool call outputs and DeepSeek R1 reasoning |
| `Ctrl + A` | Open session token & cost analytics dashboard |
| `Ctrl + L` | Clear transcript screen |
| `Ctrl + C` | Cancel active generation / double-press to exit |
| `Ctrl + Q` / `Esc` | Close popup dialog / Exit application |
| `?` (empty prompt) | Open keyboard shortcuts and help overlay |
| `PgUp` / `PgDown` | Scroll conversation transcript history |

### Slash Commands
| Command | Action |
| :--- | :--- |
| `/help` | View help card with all keybindings and commands |
| `/settings` | Open interactive Settings Manager (General, Security, TUI, Features) |
| `/palette` | Open universal fuzzy command palette |
| `/build` | Switch to autonomous Build mode (full file edit & execution) |
| `/plan` | Toggle Plan mode (advisory architecture exploration) |
| `/vibe` | Engage continuous background auto-healing vibe loop |
| `/visual [url]` | Run autonomous Visual TDD check on web frontend |
| `/safety` | Audit codebase blast-radius and data-flow invariants |
| `/model [slug]` | Open model switcher modal or switch directly |
| `/diff` | View colorized uncommitted Git diff in working directory |
| `/undo` | Revert session and workspace mutations to previous checkpoint |
| `/verify` | Run autonomous compiler and linter checks on workspace |
| `/test` | Run workspace test suite with diagnostic reporting |
| `/review` | Conduct automated code review of git changes |
| `/analytics` | Show session token accounting and pricing metrics |
| `/apply` | Inspect and verify working tree patch state |
| `/fork` | Fork active conversation session into a new branch |
| `/clear` | Clear the active transcript view |
| `/compact` | Compact conversation history to reduce token context |
| `/resume` | Browse and resume previous SQLite sessions |
| `/checkpoints` | Browse session checkpoints and revert DAG |
| `/procs` | Manage background processes and servers |
| `/locks` | Inspect active path-based concurrency locks |
| `/sandbox` | Cycle sandbox policy (`ReadOnly` → `WorkspaceWrite` → `DangerFullAccess`) |
| `/approval` | Cycle approval policy (`OnRequest` → `Never` → `ApproveForMe`) |
| `/doctor` | Run runtime diagnostic healthchecks |
| `/board` | Display recent multi-agent message board messages |
| `/exit` | Exit M31A |

---

## CLI Command Reference

The `m31a` CLI exposes 54 subcommands for scripting, CI pipelines, and advanced automation:

```bash
# Continuous Vibe Coding Loop with background auto-healing & visual check
m31a vibe
m31a vibe "Build dark-mode pricing cards with smooth CSS transitions" --visual-url http://localhost:3000

# Speculative Parallel Reality Arena (competes multiple hypotheses concurrently)
m31a speculate "Optimize database query serialization" \
  "Approach A: Use simd-json with zerocopy parsing" \
  "Approach B: Use buffer reuse with rkyv" \
  --promote

# Non-interactive task execution
m31a exec "Write tests for m31a-tools"

# Multi-agent swarm orchestration
m31a swarm "Refactor the IPC transport layer"

# Automated code review on git diff
m31a review --base main

# Apply patch to repository
m31a apply [patch_file]

# Resume interactive session
m31a resume --last
m31a resume <thread_id>

# Manage Model Context Protocol (MCP) servers
m31a mcp list
m31a mcp add filesystem npx -y @modelcontextprotocol/server-filesystem /path

# Inspect and toggle feature flags
m31a features list
m31a features enable code_mode

# Run command inside sandbox directly
m31a sandbox ls -la

# Run JSON-RPC 2.0 daemon for IDE extensions
m31a app-server --stdio
```

For the complete reference of all 54 subcommands and options, see [`docs/CLI_REFERENCE.md`](docs/CLI_REFERENCE.md).

---

## SDK Ecosystem (TypeScript & Python)

M31A includes official, production-ready client libraries for building IDE extensions, agent swarms, and automation pipelines:

| Language | Package Directory | Package Name | Transports | Key Highlights |
| :--- | :--- | :--- | :--- | :--- |
| **TypeScript / Node.js** | [`sdk/typescript`](sdk/typescript) | `@m31a/sdk` | UDS, Stdio, HTTP/SSE, Mock | Async iterable streaming, DeepSeek R1 reasoning paths, approvals, sections |
| **Python** | [`sdk/python`](sdk/python) | `m31a` | UDS, Stdio, HTTP/SSE, Mock | `asyncio` & synchronous client (`SyncM31AClient`), zero external dependencies |

Consult [`sdk/README.md`](sdk/README.md) for quickstarts and API references.

---

## Security & Sandboxing Architecture

M31A operates on a defense-in-depth model:
1. **Linux Bubblewrap Sandbox (`m31a-sandbox`)**: Isolates execution using Linux namespaces (PID, IPC, UTS, Mount). Root filesystem is mounted strictly read-only (`ro-bind`). Protected subpaths (`.git`, `.m31a`, `.ssh`, `.env`) are masked with read-only protections.
2. **AST & Prefix ExecPolicy (`m31a-execpolicy`)**: Evaluates command tokens against pattern rules and network destination filters before execution.
3. **Guardian Autonomous Supervisor (`m31a-guardian`)**: Reviews mutating operations with an out-of-band supervisor and trips a sliding-window circuit breaker upon repeated policy denials.

For full architectural details, see [`docs/SANDBOX_SECURITY.md`](docs/SANDBOX_SECURITY.md).

---

## Workspace Monorepo Layout (39 Crates)

The M31A repository is organized into 39 decoupled domain crates across clear architectural boundaries:

```
crates/
├── m31a-cli/                     # Unified 'm31a' CLI binary (54 subcommands)
├── m31a-tui/                     # Ratatui/Crossterm terminal UI application
├── m31a-composer/                # Multiline prompt composer & syntax highlighter
├── m31a-term/                    # Terminal raw mode, ANSI escape sequences & colors
├── m31a-fuzzy/                   # High-performance fuzzy matching algorithms
├── m31a-diff/                    # Syntactic diff computation & colored formatting
├── m31a-analytics/               # Session token accounting & real-time cost metrics
├── m31a-agent-engine/            # Multi-turn turn loop, vibe loop, speculative arena & swarm
├── m31a-provider/                # NVIDIA NIM SSE streaming, models catalog & reasoning
├── m31a-elicitation/             # Clarifying question prompt generator
├── m31a-review/                  # Automated Git diff code reviewer
├── m31a-verify/                  # Autonomous compiler checks, tests & visual TDD
├── m31a-browser/                 # Headless Chromium CDP controller & screenshot engine
├── m31a-ast/                     # Symbol call-graph analysis & blast-radius scoring
├── m31a-sandbox/                 # Linux Bubblewrap isolation & path protection masks
├── m31a-execpolicy/              # Prefix command rules & network destination filters
├── m31a-guardian/                # Review supervisor & sliding-window circuit breaker
├── m31a-trust/                   # Workspace trust boundary & git hook armor
├── m31a-tools/                   # 18-tool orchestration suite
├── m31a-patch/                   # Codex multi-file patch parser & 4-tier fuzzy seek
├── m31a-code-mode/               # Script execution runtime with @exec pragmas & yields
├── m31a-shell/                   # Subprocess execution & stream multiplexer
├── m31a-proc/                    # Background process supervisor & dev server manager
├── m31a-core/                    # Configuration, session data structures & skills loader
├── m31a-storage/                 # SQLite threads, checkpoints & pub-sub board
├── m31a-features/                # Runtime feature flag management
├── m31a-compaction/              # Conversation history pruning & context summarization
├── m31a-telemetry/               # System & session telemetry metrics
├── m31a-watcher/                 # Debounced filesystem change watcher for vibe coding
├── m31a-app-server/              # JSON-RPC 2.0 daemon (stdio, UDS, HTTP/SSE)
├── m31a-extension-api/           # Standardized plugin interfaces & manifest schemas
├── m31a-hooks/                   # Deterministic lifecycle hooks
├── m31a-mcp/                     # Model Context Protocol (MCP) client
├── m31a-collaboration-templates/ # Pre-configured multi-agent collaboration setups
├── m31a-git-attribution/         # AI co-authorship commit trailer generator
├── m31a-image-generation/        # Multimodal image synthesis integration
├── m31a-web-search/              # Real-time web search retrieval client
├── m31a-prompts/                 # System prompt builders & model preamble templates
└── m31a-cloud-tasks/             # Remote cloud task dispatch client
```

For detailed crate specifications, consult [`crates/README.md`](crates/README.md).

---

## Testing & Quality Verification

M31A strictly enforces a **Zero-Mock Policy** for workspace testing. Tests run real SQLite databases, real Bubblewrap sandboxes, real git repositories, and real process streams.

```bash
# Execute full workspace test suite
cargo test --workspace

# Run formatting check
cargo fmt --all -- --check

# Run Clippy with strict error escalation
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Run Python SDK tests
python3 -m unittest discover -s sdk/python/tests

# Run TypeScript SDK tests
npm test --prefix sdk/typescript
```

## Documentation Index

- [Documentation Hub](docs/README.md)
- [Getting Started Walkthrough](docs/GETTING_STARTED.md)
- [Terminal User Interface (TUI) User Guide](docs/TUI_GUIDE.md)
- [System Architecture](docs/ARCHITECTURE.md)
- [CLI Command Reference](docs/CLI_REFERENCE.md)
- [Configuration Guide](docs/CONFIGURATION.md)
- [App Server & JSON-RPC 2.0 Protocol](docs/APP_SERVER_RPC.md)
- [Sandbox & Security Architecture](docs/SANDBOX_SECURITY.md)
- [Extensions, Skills & MCP Integration](docs/EXTENSIONS_AND_SKILLS.md)
- [Multi-Agent Swarm & Distributed Mesh](docs/SWARM_AND_MULTI_AGENT.md)
- [TUI Architecture](docs/tui/TUI-ARCHITECTURE.md)
- [Deep Implementation Gap & Production Remediation Report](docs/reports/DEEP_IMPLEMENTATION_GAP_REMEDIATION_REPORT.md)
- [Logical Bug & Security Remediation Report](docs/reports/LOGICAL_BUG_REMEDIATION_REPORT.md)
- [Roadmap](ROADMAP.md)
- [Changelog](CHANGELOG.md)

---

## Contributing

We welcome contributions from developers, researchers, and systems engineers! Please review our [Contributing Guidelines](CONTRIBUTING.md) and [Code of Conduct](CODE_OF_CONDUCT.md) before submitting pull requests.

---

## Security Policy

Please review our [Security Policy](SECURITY.md) to report vulnerabilities privately. Do not file public GitHub issues for security reports.

---

## License

This project is licensed under the [MIT License](LICENSE).

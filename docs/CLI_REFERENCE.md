# M31A CLI Command Reference

This document provides a comprehensive reference for the `m31a` command-line interface, including all global options, subcommands, arguments, and environment variables.

---

## Global Usage Syntax

```bash
m31a [OPTIONS] [PROMPT] [COMMAND]
```

When invoked with `[PROMPT]` and no subcommand, `m31a` launches the interactive Terminal User Interface (TUI) pre-populated with your prompt. When invoked with neither, it launches the interactive TUI in an empty session.

---

## Global Options

| Option | Shorthand | Description | Default / Values |
| :--- | :--- | :--- | :--- |
| `--model <MODEL>` | `-m` | Model slug to use on NVIDIA NIM. | `nvidia/nemotron-3.5-lightning-30b-a3b` |
| `--sandbox <SANDBOX>` | `-s` | Sandbox filesystem isolation policy. | `workspace-write` (`read-only`, `workspace-write`, `danger-full-access`) |
| `--ask-for-approval <APPROVAL>` | `-a` | Policy for prompting human approvals for mutating tool calls. | `on-request` (`on-request`, `never`, `approve-for-me`) |
| `--cd <PATH>` | `-C` | Set working directory before executing commands. | Current working directory |
| `--config <KEY=VALUE>` | `-c` | Override a configuration value dynamically (repeatable). | - |
| `--worktree` | - | Run session in a newly managed Git worktree for isolation. | `false` |
| `--search` | - | Enable live web search tool during this session. | `false` |
| `--image <PATH>` | `-i` | Attach one or more local image files to the prompt (repeatable). | - |
| `--enable <FEATURE>` | - | Enable a feature flag by name (repeatable). | - |
| `--disable <FEATURE>` | - | Disable a feature flag by name (repeatable). | - |
| `--approve-for-me` | - | Route approval requests through Guardian auto-review supervisor. | `false` |
| `--dangerously-bypass-approvals-and-sandbox` | - | Skip approvals and execute outside sandbox (**danger**). | `false` |
| `--preset <PRESET>` | - | Apply security preset. | `standard` (`read-only`, `standard`, `permissive`, `airgapped`, `danger-full-access`) |
| `--profile <PROFILE>` | `-p` | Layer a profile configuration on top of base user config. | - |
| `--strict-config` | - | Reject unknown config fields (escalates warnings to errors). | `false` |
| `--help` | `-h` | Print help information. | - |
| `--version` | `-V` | Print version information. | - |

---

## Subcommands Overview

### 1. Environment & Setup

#### `m31a setup`
Interactive setup wizard configuring NVIDIA NIM API key, default model, and security policies.
```bash
m31a setup
```

#### `m31a doctor`
Diagnose local environment, NVIDIA API key, `bwrap` binary, and database health.
```bash
m31a doctor
```

#### `m31a login`
Store credentials for NVIDIA NIM or custom endpoint securely.
```bash
m31a login
```

#### `m31a logout`
Purge cached authentication credentials and session tokens.
```bash
m31a logout
```

#### `m31a trust`
Inspect workspace trust boundary and verify Git hook safety armor.
```bash
m31a trust
m31a trust allow
```

#### `m31a feedback`
Collect an anonymized diagnostic feedback bundle (ZIP) for error reporting and bug triage.
```bash
m31a feedback
```

#### `m31a config-schema`
Generate JSON Schema for M31A configuration files.
```bash
m31a config-schema > ~/.m31a/schema.json
```

#### `m31a completion`
Generate shell completions for `bash`, `zsh`, or `fish`.
```bash
m31a completion bash > ~/.bash_completion.d/m31a
m31a completion zsh > /usr/local/share/zsh/site-functions/_m31a
```

---

### 2. Autonomous Execution Engines

#### `m31a exec`
Run M31A non-interactively for a single autonomous task.
```bash
m31a exec "Write tests for crates/m31a-patch"
m31a exec --model deepseek-ai/deepseek-v4.1-flash "Analyze memory safety in m31a-sandbox"
```

#### `m31a vibe`
Continuous vibe coding loop: watches filesystem for changes, verifies builds, and auto-repairs errors.
```bash
m31a vibe
m31a vibe "Build dark mode CSS components" --visual-url http://localhost:3000
```

#### `m31a speculate`
Speculative multi-branch parallel reality arena: competes multiple implementation hypotheses concurrently in temporary sandboxes and merges the winning branch.
```bash
m31a speculate "Optimize database query serialization" \
  "Approach A: SIMD-JSON zerocopy" \
  "Approach B: FlatBuffers zero-allocation" \
  --promote
```

#### `m31a swarm`
Execute an autonomous multi-agent swarm with specialized roles (`Architect`, `Coder`, `Tester`, `Guardian`).
```bash
m31a swarm "Refactor the IPC transport layer to use epoll and shared memory rings"
```

#### `m31a gsd`
Execute spec-driven autonomous phase workflows (plan checking, verification, review).
```bash
m31a gsd specs/feature.md
```

---

### 3. Verification, Diff & Code Quality

#### `m31a verify`
Autonomously verify project with compiler checks, linters, or test suites.
```bash
m31a verify
m31a verify --test
```

#### `m31a review`
Run automated code review on uncommitted working tree diffs or across git branches.
```bash
m31a review
m31a review --base main
```

#### `m31a apply`
Apply the latest diff or patch produced by an agent to the local working tree.
```bash
m31a apply
```

#### `m31a apply-patch`
Apply a structured multi-file code patch directly to current workspace.
```bash
m31a apply-patch patch.txt
```

---

### 4. Interactive Sessions & History Management

#### `m31a resume`
Resume a past interactive session.
```bash
m31a resume --last
m31a resume <thread_id>
```

#### `m31a fork`
Fork a previous session into a new branch.
```bash
m31a fork --last
m31a fork <thread_id> --title "Experimental Refactor"
```

#### `m31a undo`
Undo workspace changes to the previous turn checkpoint.
```bash
m31a undo
```

#### `m31a revert`
Revert workspace and conversation back to a specific checkpoint or turn ID.
```bash
m31a revert <checkpoint_id>
```

#### `m31a archive` / `unarchive` / `delete`
Manage session lifecycles in SQLite storage.
```bash
m31a archive <thread_id>
m31a unarchive <thread_id>
m31a delete <thread_id>
```

#### `m31a history`
Display or inspect global prompt and turn history.
```bash
m31a history --limit 20
```

#### `m31a export`
Export session transcript as Markdown, JSONL, or HTML documents.
```bash
m31a export <thread_id> --format markdown -o session.md
```

#### `m31a sections`
Organize conversation threads and turns into named semantic sections.
```bash
m31a sections list
m31a sections create "Phase 1: Architecture Exploration"
```

---

### 5. Daemons, Servers & Transports

#### `m31a app-server`
Run the local JSON-RPC 2.0 app server daemon.
```bash
# Stdio transport (default for IDE extensions)
m31a app-server --stdio

# Unix Domain Socket transport
m31a app-server --uds ~/.m31a/app-server-control/app-server-control.sock
```

#### `m31a daemon`
Manage background M31A daemon process.
```bash
m31a daemon start
m31a daemon status
m31a daemon stop
```

#### `m31a serve`
Run HTTP/SSE daemon server for remote clients and web frontends.
```bash
m31a serve --host 127.0.0.1 --port 8080
```

#### `m31a responses-proxy`
Hardened local reverse proxy caching and locking credentials in memory.
```bash
m31a responses-proxy --port 8081
```

---

### 6. Process Supervisor

#### `m31a ps`
List managed background processes, dev servers, and active background tasks.
```bash
m31a ps
```

#### `m31a logs`
Inspect or tail background process output logs.
```bash
m31a logs <proc_id> --follow
```

#### `m31a kill`
Terminate a managed background process.
```bash
m31a kill <proc_id>
```

---

### 7. Extensibility, Tools & Swarm

#### `m31a mcp`
Manage external Model Context Protocol (MCP) servers.
```bash
m31a mcp list
m31a mcp add filesystem npx -y @modelcontextprotocol/server-filesystem /path
m31a mcp remove filesystem
```

#### `m31a features`
Inspect and toggle experimental runtime feature flags.
```bash
m31a features list
m31a features enable code_mode
m31a features disable guardian_approval
```

#### `m31a board`
Browse or post to the multi-agent pub-sub message board.
```bash
m31a board list
m31a board post general "Refactoring completed in crates/m31a-patch"
```

#### `m31a sandbox`
Run an arbitrary shell command directly inside the Bubblewrap sandbox.
```bash
m31a sandbox ls -la
m31a sandbox --policy read-only make test
```

---

## Environment Variables

| Variable | Description |
| :--- | :--- |
| `NVIDIA_API_KEY` | NVIDIA NIM authentication key (`nvapi-...`). |
| `NVIDIA_API_BASE` | Base URL for NIM endpoint (default: `https://integrate.api.nvidia.com/v1`). |
| `M31A_CONFIG` | Path to custom configuration TOML file. |
| `M31A_SANDBOX` | Default sandbox policy (`read-only`, `workspace-write`, `danger-full-access`). |
| `M31A_APPROVAL` | Default approval policy (`on-request`, `never`, `approve-for-me`). |
| `M31A_MODEL` | Default model slug to invoke on NVIDIA NIM. |
| `RUST_LOG` | Tracing filter directive (e.g. `m31a=debug,info`). |
| `RUST_BACKTRACE` | Set to `1` or `full` for detailed crash backtraces. |

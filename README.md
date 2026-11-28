<p align="center">
  <img src="./M31A.svg" alt="M31A Logo" width="128" />
</p>

# M31A

> **M31A** is an MIT-licensed agentic coding terminal maintained by **Eshan
> Roy** ([github.com/eshanized/M31A](https://github.com/eshanized/M31A)),
> originally forked from **gemini-cli** (Apache-2.0, © Google LLC), with Google
> sign-in, telemetry, and Gemini-only backends removed in favor of **NVIDIA
> NIM** via an OpenAI-compatible runtime. It is **not affiliated with, endorsed
> by, or a product of Google or NVIDIA**. Upstream Google copyright and
> Apache-2.0 notices are retained; see [`LICENSE`](LICENSE),
> [`LICENSE-APACHE-2.0`](LICENSE-APACHE-2.0), [`NOTICE`](NOTICE), and
> [`MODIFICATIONS.md`](MODIFICATIONS.md).

[![Version](https://img.shields.io/npm/v/@m31a/cli)](https://www.npmjs.com/package/@m31a/cli)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/eshanized/M31A/blob/main/LICENSE)

M31A (`m31a`) is an open-source terminal coding agent powered by NVIDIA NIM
models, with local file/shell tools, workspace trust policies, sandboxing, MCP
server support, skills, and interactive TUI workflows.

## Installation

### Run without installing

```bash
npx @m31a/cli
```

### Install globally

```bash
npm install -g @m31a/cli
```

## Authentication (NVIDIA NIM)

Set your NVIDIA NIM API key (or configure it interactively via `/auth`):

```bash
export NVIDIA_API_KEY="nvapi-..."
m31a
```

## Usage

```bash
# Launch interactive TUI in the current workspace
m31a

# Select a specific model
m31a -m nvidia/nemotron-3-ultra-550b-a55b

# Include extra workspace directories
m31a --include-directories ../lib,../shared

# Run non-interactively (headless)
m31a -p "Explain the architecture of this codebase"

# Structured JSON / streaming JSON output
m31a -p "Explain the architecture of this codebase" --output-format json
m31a -p "Run tests and summarize failures" --output-format stream-json
```

## Key Features

- **Codebase Exploration & Editing**: Read, search (`grep_search` / ripgrep /
  `glob`), edit (`replace` / `apply_patch`), write files, and execute shell
  commands with policy confirmation controls.
- **Session & Context Management**: Local SQLite-backed session history,
  checkpointing (`/chat` / `/resume` / `/rewind`), context compression, and
  workspace `M31A.md` instructions.
- **Extensibility**: Model Context Protocol (MCP) servers (`/mcp`), Agent Skills
  (`/skills`), custom hooks, and ACP mode (`--acp`).
- **Sandboxing & Security**: Folder trust enforcement, configurable policy
  engine, macOS Seatbelt profiles, and Docker/Podman/LXC container sandboxing.

## Development

```bash
npm install
npm run build
npm run test
npm run lint
npm run typecheck
```

## Links

- **[GitHub Repository](https://github.com/eshanized/M31A)**
- **[Issues](https://github.com/eshanized/M31A/issues)**
- **[Roadmap](./ROADMAP.md)**
- **[Contributing](./CONTRIBUTING.md)**
- **[Security Policy](./SECURITY.md)**

## Legal

- **Author**: Eshan Roy
- **License**: [MIT License](LICENSE) (upstream Apache-2.0 notices in
  [`LICENSE-APACHE-2.0`](LICENSE-APACHE-2.0), [`NOTICE`](NOTICE), and
  [`MODIFICATIONS.md`](MODIFICATIONS.md))

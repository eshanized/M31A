# M31 Autonomous (M31A)

[![CI](https://github.com/eshanized/M31A/actions/workflows/release-gates.yml/badge.svg)](https://github.com/eshanized/M31A/actions)
[![Release](https://img.shields.io/github/v/release/eshanized/M31A?include_prereleases&color=blue)](https://github.com/eshanized/M31A/releases/latest)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.80%2B-orange.svg)](rust-toolchain.toml)
[![Website](https://img.shields.io/badge/website-m31a.tonmoyinfrastructure.org-blue.svg)](https://m31a.tonmoyinfrastructure.org/)

> **"The model proposes. The runtime decides."**

**Website:** [https://m31a.tonmoyinfrastructure.org/](https://m31a.tonmoyinfrastructure.org/) &bull; **Repository:** [https://github.com/eshanized/M31A/](https://github.com/eshanized/M31A/)

M31A (M31 Autonomous) is a single-crate, high-assurance, Rust-native autonomous software engineering runtime. It provides deterministic lifecycle control, strict multi-layer security policies, resource-bounded execution, continuous verification, crash-resilient checkpoints, and local observability for autonomous coding agents.

Unlike ad-hoc agent scripts or loose orchestration frameworks that delegate execution authority to non-deterministic large language models, M31A treats the LLM as an untrusted reasoning component. The runtime strictly owns state, scheduling, file access, command execution, policies, verification, and completion criteria.

---

## Architectural Principles

1. **Single Trusted Kernel**: Implemented as a single, clean Rust crate without foreign runtime dependencies (no Node.js, Python, or GPU required for core runtime execution).
2. **Deterministic Governance**: Every side effect (file writes, process spawning, Git operations, network requests) passes through an 11-stage policy evaluation gate.
3. **Evidence-Based Completion**: No mission or task is marked complete without deterministic, multi-tier verification evidence.
4. **Resilient Recovery**: Two-phase atomic checkpoints, startup crash scanners, and differential DAG replanners preserve completed work across crashes.
5. **Bounded Confinement**: Multi-dimensional budget models (10 hard resource dimensions) and platform-aware confinement (cgroups v2, POSIX rlimits, Windows Job Objects) prevent runaway execution.
6. **Local Observability**: SQLite compact event indexes, append-only NDJSON execution streams, and zero-leak secret redaction before durable storage.

---

## Installation

### 1. One-Liner Install Script

On **Linux** and **macOS**:
```bash
curl -fsSL https://raw.githubusercontent.com/eshanized/M31A/master/scripts/install.sh | bash
```

On **Windows** (PowerShell):
```powershell
irm https://raw.githubusercontent.com/eshanized/M31A/master/scripts/install.ps1 | iex
```

### 2. Pre-Built Standalone Binaries

Download native standalone release archives directly from [GitHub Releases](https://github.com/eshanized/M31A/releases/latest):

| OS / Architecture | Target | Archive |
|:---|:---|:---|
| **Linux (x86_64)** | `x86_64-unknown-linux-gnu` | [`m31a-linux-x64.tar.gz`](https://github.com/eshanized/M31A/releases/latest) |
| **Linux (ARM64)** | `aarch64-unknown-linux-gnu` | [`m31a-linux-arm64.tar.gz`](https://github.com/eshanized/M31A/releases/latest) |
| **macOS (Apple Silicon)** | `aarch64-apple-darwin` | [`m31a-darwin-arm64.tar.gz`](https://github.com/eshanized/M31A/releases/latest) |
| **macOS (Intel)** | `x86_64-apple-darwin` | [`m31a-darwin-x64.tar.gz`](https://github.com/eshanized/M31A/releases/latest) |
| **Windows (x86_64)** | `x86_64-pc-windows-msvc` | [`m31a-windows-x64.zip`](https://github.com/eshanized/M31A/releases/latest) |
| **Windows (ARM64)** | `aarch64-pc-windows-msvc` | [`m31a-windows-arm64.zip`](https://github.com/eshanized/M31A/releases/latest) |

### 3. Build from Source

```bash
git clone https://github.com/eshanized/M31A.git
cd M31A
cargo build --release
cargo install --path .
```

---

## Quickstart

### 1. Configure Provider Credentials

Set your preferred provider API key (or configure via `m31a config set-credential <provider> <key>`):

```bash
export ANTHROPIC_API_KEY="sk-ant-..."
# or
export OPENAI_API_KEY="sk-..."
```

### 2. Run Your First Mission

Launch an autonomous mission in your workspace:

```bash
# Execute a guided coding mission
m31a mission run "Refactor database queries to use parameterized statements" --profile coding

# Launch the interactive terminal cockpit
m31a tui

# Inspect telemetry and execution metrics
m31a telemetry inspect <mission-id> --summary --spans --metrics

# Run an unattended evaluation benchmark suite
m31a eval run --all
```

---

## Architecture Overview (L0–L9)

The M31A runtime enforces a downward-dependency layered hierarchy:

```
L9  CLI / TUI Cockpit / External Integrations
L8  Autonomy Controller / Mission Orchestration
L7  Verification Engine / Recovery / Checkpoints
L6  Execution Engine / Jobs / Artifact Store
L5  Planning Engine / Candidate DAG / Reconciler
L4  Agent Coordination / Role State Machines
L3  Intelligence / Prompt Envelopes / Memory
L2  Capability Providers / Tool Catalog / Confinement
L1  Security / Multi-Layer Policy Gate / Sandbox
L0  Kernel / Foundation IDs / Shared Error Model
```

Lower layers never import or depend on higher layers. The TUI and CLI are pure projections of authoritative SQLite state.

---

## Documentation & Subsystem Manuals

The full documentation suite is available in the [`docs/`](docs/README.md) directory:

| Subsystem | Document | Scope & Guarantees |
|:---|:---|:---|
| **Runtime Architecture** | [`docs/architecture/ARCHITECTURE.md`](docs/architecture/ARCHITECTURE.md) | Layered architecture, 12-stage controller loop, 8 agent roles, hybrid storage. |
| **Autonomy Engine** | [`docs/subsystems/AUTONOMY.md`](docs/subsystems/AUTONOMY.md) | 5 autonomy modes, sliding-window loop detector, 10 budget dimensions. |
| **CLI & Telemetry** | [`docs/subsystems/CLI.md`](docs/subsystems/CLI.md) | Complete CLI subcommand suite, arguments, exit codes, telemetry tracing. |
| **Configuration** | [`docs/subsystems/CONFIGURATION.md`](docs/subsystems/CONFIGURATION.md) | 7-tier precedence hierarchy, schema validation, 7 canonical profiles. |
| **Tool Catalog** | [`docs/subsystems/TOOLS.md`](docs/subsystems/TOOLS.md) | Catalog of all 28 core tools, parameter schemas, execution risks, error categories. |
| **Policy Gate** | [`docs/subsystems/POLICY.md`](docs/subsystems/POLICY.md) | 11-stage policy gate, decision matrix (`ALLOW`/`DENY`/`ASK`/`ESCALATE`), 9-layer precedence. |
| **Security & Sandbox** | [`docs/subsystems/SECURITY.md`](docs/subsystems/SECURITY.md) | ASVS L1 compliance, 11-threat hardening matrix, secret redactor, trust envelopes. |
| **Process Confinement** | [`docs/subsystems/PLUGIN.md`](docs/subsystems/PLUGIN.md) | In-process extension architecture, tool registration, lifecycle hooks, sandbox isolation. |
| **Recovery System** | [`docs/subsystems/RECOVERY.md`](docs/subsystems/RECOVERY.md) | Two-phase checkpoints, 15 failure classifications, differential replanning. |
| **Testing & Verification** | [`docs/subsystems/TESTING.md`](docs/subsystems/TESTING.md) | Multi-tier test strategy, automated contract tests, security hardening suite. |
| **Platform Support** | [`docs/PLATFORM-SUPPORT.md`](docs/PLATFORM-SUPPORT.md) | Authoritative cross-platform matrix (Linux, macOS, Windows). |
| **Release Engineering** | [`docs/RELEASE.md`](docs/RELEASE.md) | Release validation gates, packaging, reproducibility, and deployment flow. |

---

## Community & Contributing

- **[Contributing Guide](CONTRIBUTING.md)**: Architecture rules, local development setup, verification pipeline, and PR workflow.
- **[Security Policy](SECURITY.md)**: Vulnerability disclosure guidelines and response SLA.
- **[Code of Conduct](CODE_OF_CONDUCT.md)**: Contributor Covenant community standards.

---

## License

Licensed under either of:
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE) or http://opensource.org/licenses/MIT)

at your option.

# M31 Autonomous (M31A)

> **"The model proposes. The runtime decides."**

M31A (M31 Autonomous) is a single-crate, high-assurance, Rust-native autonomous software engineering runtime. It provides deterministic lifecycle control, strict multi-layer security policies, resource-bounded execution, continuous verification, crash-resilient checkpoints, and local observability for autonomous coding agents.

Unlike ad-hoc agent scripts or loose orchestration frameworks that delegate execution authority to non-deterministic large language models, M31A treats the LLM as an untrusted reasoning component. The runtime strictly owns state, scheduling, file access, command execution, policies, verification, and completion criteria.

---

## Architectural Principles

1. **Single Trusted Kernel**: Implemented as a single, clean Rust crate without foreign runtime dependencies (no Node.js, Python, or GPU required for core runtime execution).
2. **Deterministic Governance**: Every side effect (file writes, process spawning, Git operations, network requests) passes through an 11-stage policy evaluation gate.
3. **Evidence-Based Completion**: No mission or task is marked complete without deterministic, multi-tier verification evidence.
4. **Resilient Recovery**: Two-phase atomic checkpoints, startup crash scanners, and differential DAG replanners preserve completed work across crashes.
5. **Bounded Confinement**: Multi-dimensional budget models (10 hard resource dimensions) and platform-aware confinement (cgroups v2, POSIX rlimits) prevent runaway execution.
6. **Local Observability**: SQLite compact event indexes, append-only NDJSON execution streams, and zero-leak secret redaction before durable storage.

---

## Quickstart

### Prerequisites

- **Rust toolchain**: 1.80+ (stable)
- **Git**: 2.30+
- **Platform**: Linux x86_64 is the release-qualified target. macOS and Windows builds exist but remain conditionally supported / compile-only — see `docs/PLATFORM-SUPPORT.md` for the authoritative per-target matrix.

### Installation

Clone the repository and build using Cargo:

```bash
git clone https://github.com/TIVerse/M31A.git
cd M31A
cargo build --release
```

Install the binary to your local path:

```bash
cargo install --path .
```

### Running Your First Mission

Initialize an autonomous task in your project workspace:

```bash
# Run a guided bugfix with the coding profile
m31a mission run "Fix failing assertion in calculator test" --profile coding

# Run an unattended evaluation benchmark across acceptance scenarios
m31a eval run --all

# Inspect real-time execution telemetry and metrics
m31a telemetry inspect <mission-id> --summary --spans --metrics

# Launch the interactive terminal cockpit
m31a tui
```

---

## Architecture Overview (L0–L9)

The M31A system is structured into a downward-dependency layered hierarchy:

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

Lower layers never depend on higher layers. The TUI and CLI are pure projections of authoritative SQLite state.

---

## Subsystems & Architecture

The complete M31A specification and operational design:

| Subsystem | Scope & Guarantees |
|:---|:---|
| Runtime Architecture | Runtime kernel, 12-stage controller loop, 8 agent roles, hybrid storage, and 15 capabilities. |
| Security & Sandbox | ASVS L1 compliance, threat model covering all 11 vectors, secret redactor, process confinement, and trust envelopes. |
| Configuration | 7-tier hierarchical configuration precedence, schema validation, 7 canonical profiles, and monotonic inheritance. |
| CLI & Telemetry | Complete CLI subcommand suite, arguments, exit codes, telemetry tracing, and machine-readable JSON formats. |
| Extension Plugins | In-process extension architecture, tool registration, lifecycle hooks, and policy subordination. |
| Tool Catalog | Comprehensive catalog of all 28 core tools, parameter schemas, execution risks, and error categories. |
| Policy Gate | 11-stage policy gate, decision matrix (`ALLOW`/`DENY`/`ASK`/`ESCALATE`), 9-layer precedence, and defaults. |
| Autonomy Engine | 5 autonomy modes, sliding-window loop detector, two-phase budget reservation, and 10-dimensional limits. |
| Recovery System | Two-phase checkpoints, 15 failure classifications, differential replanning, and crash recovery scanner. |
| Testing & Verification | Multi-tier test strategy, automated contract tests, security hardening suite, and evaluation scenarios. |

---

## License

Apache-2.0 or MIT at your option.

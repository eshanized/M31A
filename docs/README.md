# M31A Documentation Index

Welcome to the comprehensive documentation for **M31 Autonomous (M31A)** — the Rust-native autonomous software-engineering runtime.

> **"The model proposes. The runtime decides."**

- **Official Website**: [https://m31a.tonmoyinfrastructure.org/](https://m31a.tonmoyinfrastructure.org/)
- **Source Repository**: [https://github.com/eshanized/M31A/](https://github.com/eshanized/M31A/)

---

## Getting Started

- **[Installation Guide](getting-started/installation.md)**: Install via pre-built binary, cargo, or one-liner install script.
- **[Quickstart Tutorial](getting-started/quickstart.md)**: Initialize your first autonomous coding mission in minutes.
- **[Platform Support Matrix](PLATFORM-SUPPORT.md)**: Hardware, OS (Linux, macOS, Windows), and container compatibility.

---

## Core Architecture

- **[Runtime Architecture](architecture/ARCHITECTURE.md)**: Layered hierarchy (L0–L9), 12-stage autonomy controller loop, 8 agent roles, and downward-dependency invariants.
- **[Release Engineering](RELEASE.md)**: Release gates, matrix builds, reproducible artifacts, and deployment flow.

---

## Subsystem Manuals

Detailed technical references for every operational subsystem:

| Subsystem | Document | Description |
|:---|:---|:---|
| **Autonomy Engine** | [AUTONOMY.md](subsystems/AUTONOMY.md) | 12-stage loop, loop detection, two-phase budget reservation, 10 budget dimensions. |
| **CLI & Telemetry** | [CLI.md](subsystems/CLI.md) | Complete reference for all CLI subcommands, flags, exit codes, and JSON outputs. |
| **Configuration** | [CONFIGURATION.md](subsystems/CONFIGURATION.md) | 7-tier precedence hierarchy, `.m31a/config.toml`, and 7 canonical execution profiles. |
| **Tool Catalog** | [TOOLS.md](subsystems/TOOLS.md) | Catalog of all 28 core tools, schemas, side-effect classifications, and error kinds. |
| **Policy Engine** | [POLICY.md](subsystems/POLICY.md) | 11-stage policy gate, decision hierarchy (`ALLOW`, `DENY`, `ASK`, `ESCALATE`), and audit logs. |
| **Security & Hardening** | [SECURITY.md](subsystems/SECURITY.md) | 11-threat hardening matrix, ASVS L1 compliance, secret redaction, and trust envelopes. |
| **Process Confinement** | [PLUGIN.md](subsystems/PLUGIN.md) | In-process extension model, external tool registration, lifecycle hooks, and sandbox isolation. |
| **Crash Recovery** | [RECOVERY.md](subsystems/RECOVERY.md) | Two-phase atomic checkpoints, startup crash scanner, and differential DAG replanning. |
| **Verification & Testing** | [TESTING.md](subsystems/TESTING.md) | Multi-tier test strategy, automated contracts, property tests, and evaluation harness. |

---

## Governance & Community

- **[Contributing Guide](../CONTRIBUTING.md)**: How to set up a development environment, run tests, and submit PRs.
- **[Security Policy](../SECURITY.md)**: Vulnerability disclosure guidelines and security contacts.
- **[Code of Conduct](../CODE_OF_CONDUCT.md)**: Community standards and expectations.

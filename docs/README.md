# Documentation Index

Welcome to the documentation for **M31 Autonomous (M31A)**, a Rust-native autonomous software-engineering runtime.

> **The model proposes. The runtime decides.**

M31A is under active development. For security-sensitive, platform-specific, and release-critical behavior, prefer executable source, tests, CI evidence, and the current release metadata over informal descriptions.

## Choose your path

### I want to use M31A
- [Installation](getting-started/installation.md) — obtain a binary or build from source.
- [Quickstart](getting-started/quickstart.md) — initialize a workspace, configure a model, and run an execution.
- [CLI reference](subsystems/CLI.md) — command hierarchy, options, output formats, and exit behavior.
- [Configuration](subsystems/CONFIGURATION.md) — config locations, precedence, profiles, model selection, and settings authority.
- [Platform support](PLATFORM-SUPPORT.md) — distinguish supported platforms from integrated or compile-checked targets.
- [Deployment and channels](DEPLOYMENT.md) — production/development separation, installation, update, and rollback behavior.
- [Troubleshooting](operations/troubleshooting.md) — common setup, model, runtime, TUI, persistence, and update failures.

### I want to understand how it works
- [Runtime architecture](architecture/ARCHITECTURE.md) — layers, roles, capability families, and execution stages.
- [Runtime authority model](architecture/RUNTIME-AUTHORITY.md) — which components own state and non-delegable decisions.
- [Prompt authority](architecture/PROMPT-AUTHORITY.md) — trust boundaries and untrusted model/repository input.
- [Autonomy subsystem](subsystems/AUTONOMY.md) — execution-loop and completion controls.
- [Policy subsystem](subsystems/POLICY.md) — authorization decisions and approvals.
- [Security subsystem](subsystems/SECURITY.md) — security boundaries, threat controls, and limitations.
- [Tools subsystem](subsystems/TOOLS.md) — available tools and their behavior.
- [Recovery subsystem](subsystems/RECOVERY.md) — checkpoints, interruption, recovery, and replanning.
- [Testing subsystem](subsystems/TESTING.md) — tests and acceptance evaluation.
- [Deployment](DEPLOYMENT.md) and [release engineering](RELEASE.md) — artifact lifecycle and release gates.

### I want to contribute
- [Contributing guide](../CONTRIBUTING.md) — contribution workflow and project conventions.
- [Agent instructions](../AGENTS.md) — architectural invariants and implementation guardrails for humans and coding agents.
- [Documentation roadmap](DOCUMENTATION-ROADMAP.md) — planned documentation improvements and acceptance criteria.
- [Wiring remediation audit](audits/M31A-WIRING-REMEDIATION.md) — a historical audit; verify its findings against current source before treating them as current state.

## Existing subsystem references

| Area | Guide |
|---|---|
| CLI, commands, telemetry | [CLI.md](subsystems/CLI.md) |
| Configuration authority and profiles | [CONFIGURATION.md](subsystems/CONFIGURATION.md) |
| Tools and capabilities | [TOOLS.md](subsystems/TOOLS.md) |
| Policy decisions and approvals | [POLICY.md](subsystems/POLICY.md) |
| Security and hardening | [SECURITY.md](subsystems/SECURITY.md) |
| Autonomous execution | [AUTONOMY.md](subsystems/AUTONOMY.md) |
| Recovery and checkpoints | [RECOVERY.md](subsystems/RECOVERY.md) |
| Test strategy and evaluations | [TESTING.md](subsystems/TESTING.md) |
| Plugin and process confinement model | [PLUGIN.md](subsystems/PLUGIN.md) |
| User-defined slash commands | [USER-DEFINED-SLASH-COMMANDS.md](subsystems/USER-DEFINED-SLASH-COMMANDS.md) |

## Reference and operational policies

- [Platform support](PLATFORM-SUPPORT.md)
- [Deployment and update channels](DEPLOYMENT.md)
- [Release procedure](RELEASE.md)
- [Security reporting](../SECURITY.md)
- [Contributing](../CONTRIBUTING.md)
- [Code of Conduct](../CODE_OF_CONDUCT.md)

## Documentation quality rules

Documentation must describe current implemented behavior and state its evidence level. Distinguish **implemented**, **tested**, **CI-verified**, **release-qualified**, and **planned**. A published binary does not automatically mean that its target platform is fully supported. A defined live integration test does not prove that the integration is currently passing.

See the [Documentation Roadmap](DOCUMENTATION-ROADMAP.md) for the proposed target structure, priorities, and definition of done.

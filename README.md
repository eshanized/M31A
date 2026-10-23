<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/M31A.svg">
    <img src="assets/M31A.svg" alt="M31 Autonomous Logo" width="128" height="128">
  </picture>
</p>

# M31 Autonomous (M31A)

**M31A is a Rust-native autonomous software-engineering runtime.** It takes a user-defined engineering objective, plans and executes bounded work, applies runtime-owned policy to side effects, verifies changes using empirical checks, and records execution state for inspection and recovery.

> **The model proposes. The runtime decides.**

M31A is actively developed. Treat platform qualification, current release status, and provider-backed verification as evidence-based claims: use the current release and CI results rather than assuming every documented or compiled target is production-qualified.

## Start here

- **Install:** [Installation guide](docs/getting-started/installation.md)
- **Run your first workflow:** [Quickstart](docs/getting-started/quickstart.md)
- **Understand the commands:** [CLI guide](docs/subsystems/CLI.md)
- **Configure M31A:** [Configuration authority](docs/subsystems/CONFIGURATION.md)
- **Check host and target support:** [Platform support matrix](docs/PLATFORM-SUPPORT.md)
- **Read the manuals:** [Documentation index](docs/README.md)
- **Help improve the docs:** [Documentation roadmap](docs/DOCUMENTATION-ROADMAP.md)

## What M31A is designed to provide

- **Structured execution:** convert engineering objectives into a task graph with explicit dependencies and bounded scheduling.
- **Runtime-owned authorization:** model output requests actions; runtime policy controls whether side effects are allowed, denied, or require approval.
- **Evidence-based completion:** use compiler, test, and verification results instead of relying solely on a model's claim that work is finished.
- **Inspectable state:** expose execution, task, telemetry, and artifact information through CLI/TUI workflows.
- **Recovery mechanisms:** persist execution state and checkpoints to support recovery from interruption or failure.
- **Constrained execution:** use filesystem containment, process controls, and available host sandbox mechanisms according to platform capabilities.

These are design goals and implemented subsystem areas, not a promise that every host provides identical isolation or that every workload will complete successfully. Review [Security](docs/subsystems/SECURITY.md), [Recovery](docs/subsystems/RECOVERY.md), and [Platform support](docs/PLATFORM-SUPPORT.md) before relying on M31A for sensitive or unattended workloads.

## Quickstart

### 1. Install prerequisites

For a release binary or source-build requirements, follow the [Installation Guide](docs/getting-started/installation.md). Source builds require the Rust toolchain version declared in `Cargo.toml`.

### 2. Initialize M31A

Run first-run onboarding:

```bash
m31a init
```

The onboarding flow manages provider credentials and model discovery. Follow its prompts and avoid placing secrets in command-line arguments, configuration examples, issue reports, or logs.

### 3. Validate setup

```bash
m31a doctor
m31a --help
m31a config validate
```

Run the config validation command after you create or modify configuration. A present but invalid configuration may cause startup to fail rather than silently replacing it with defaults.

### 4. Start an execution

From the target project directory, begin with a small, reviewable objective:

```bash
m31a run "Explain the project structure and identify one small, testable improvement"
```

Inspect the available command options with `m31a run --help` before adding flags. Use the interactive cockpit when appropriate:

```bash
m31a tui
```

### 5. Inspect outcomes

Use the CLI help for the commands supported by the installed version:

```bash
m31a execution --help
m31a task --help
m31a artifact --help
m31a telemetry --help
```

Review proposed changes and verification evidence before integrating changes into an important branch or deploying them.

## Core execution model

At a high level, M31A coordinates the following stages:

1. **Intake:** validate the objective, workspace, and effective configuration.
2. **Context and planning:** gather relevant project context and prepare a structured plan.
3. **Scheduling and execution:** run eligible tasks under runtime-controlled capabilities and policy.
4. **Verification:** execute applicable checks and retain their outcomes.
5. **Checkpoint and report:** persist state and produce inspectable results.

The exact path depends on the command, configuration, policy decisions, available host capabilities, and verification adapters. The detailed explanation lives in the [Runtime Architecture](docs/architecture/ARCHITECTURE.md) and [Autonomy subsystem](docs/subsystems/AUTONOMY.md) guides.

## Safety and limitations

- Model and repository content must be treated as untrusted input.
- A policy decision may deny an operation or require approval; unattended execution should fail closed rather than bypass approval.
- Security isolation depends on the host platform and available native mechanisms. Check the [Platform Support Matrix](docs/PLATFORM-SUPPORT.md).
- Do not infer that a provider integration works merely because its code compiles or its test exists; inspect current verification evidence.
- Avoid running autonomous changes against production systems, valuable data, or sensitive repositories without suitable backups, access controls, and human review.

Read the [Security Policy](SECURITY.md) for vulnerability disclosure instructions.

## Documentation

The [documentation index](docs/README.md) is the navigation hub for installation, quickstart, CLI, architecture, configuration, policy, security, tools, recovery, testing, platform support, deployment, and releases.

The [Documentation Roadmap](docs/DOCUMENTATION-ROADMAP.md) tracks planned improvements and defines the standard for evidence-backed documentation.

## Development

M31A is a Rust project.

```bash
git clone https://github.com/eshanized/M31A.git
cd M31A
cargo fmt --check
cargo check
cargo clippy --all-targets -- -D warnings
cargo test
```

Use the exact commands and feature/channel constraints documented in `AGENTS.md`, `CONTRIBUTING.md`, and the relevant CI workflow. In particular, do not enable mutually exclusive deployment-channel features together.

- [Contributing guide](CONTRIBUTING.md)
- [Agent implementation guardrails](AGENTS.md)
- [Release engineering](docs/RELEASE.md)

## License

M31A is licensed under the [MIT License](LICENSE).

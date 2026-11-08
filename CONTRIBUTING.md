# Contributing to M31A

Thank you for your interest in contributing to **M31A**! We welcome contributions from developers, researchers, and systems engineers worldwide.

This document outlines the guidelines, development setup, and workflow for contributing to the M31A monorepo.

---

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Monorepo Architecture](#monorepo-architecture)
- [Prerequisites & Development Setup](#prerequisites--development-setup)
- [Building the Project](#building-the-project)
- [Running Tests](#running-tests)
- [Code Style & Quality Standards](#code-style--quality-standards)
- [Commit Message Conventions](#commit-message-conventions)
- [Submitting a Pull Request](#submitting-a-pull-request)
- [Reporting Bugs & Suggesting Features](#reporting-bugs--suggesting-features)

---

## Code of Conduct

All contributors and maintainers are expected to abide by our [Code of Conduct](CODE_OF_CONDUCT.md). Please report unacceptable behavior to `security@tonmoyinfrastructure.org`.

---

## Monorepo Architecture

M31A is organized as a Cargo workspace with Just orchestration and cargo-nextest test execution, containing 39 Rust crates, official SDKs, and examples:

```
M31A/
├── Cargo.toml                  # Cargo workspace definition (Rust 2024 edition)
├── justfile                    # Unified developer command runner
├── crates/                     # 39 modular domain crates
│   ├── m31a-cli/               # Unified 'm31a' binary entrypoint (54 subcommands)
│   ├── m31a-core/              # Core types, configuration, session models, skills
│   ├── m31a-provider/          # NVIDIA NIM SSE streaming, models catalog, reasoning
│   ├── m31a-agent-engine/      # Autonomous vibe coding, spec arena, turn execution
│   ├── m31a-tui/               # Ratatui/Crossterm terminal UI
│   ├── m31a-tools/             # 18-tool suite & Codex multi-file patch engine
│   ├── m31a-sandbox/           # Linux Bubblewrap isolation & protection masks
│   ├── m31a-execpolicy/        # Starlark/prefix execution policy engine
│   ├── m31a-guardian/          # Supervisor review engine & circuit breaker
│   ├── m31a-storage/           # SQLite threads, checkpoints, pub-sub board
│   ├── m31a-app-server/        # JSON-RPC 2.0 daemon (stdio / UDS / HTTP)
│   └── ...                     # Additional specialized subsystem crates
├── sdk/
│   ├── python/                 # Official Python SDK (async & sync client)
│   └── typescript/             # Official TypeScript SDK (@m31a/sdk)
├── examples/                   # Reference implementations & architectural examples
│   └── distributed_task_mesh/  # Distributed multi-agent task orchestration demo
└── docs/                       # Technical specs, architecture docs, and reports
```

For a comprehensive explanation of each crate's responsibilities, consult [`crates/README.md`](crates/README.md) and [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

---

## Prerequisites & Development Setup

### System Requirements

- **Linux** (x86_64 or aarch64) or **macOS** (Apple Silicon or Intel).
  - *Linux note*: `bubblewrap` (`bwrap`) is required for sandbox execution:
    ```bash
    sudo apt install bubblewrap   # Debian/Ubuntu
    sudo dnf install bubblewrap   # Fedora/RHEL
    sudo pacman -S bubblewrap     # Arch Linux
    ```
- **Rust Toolchain**: Rust stable (version 1.85+ / 2024 edition recommended)
  ```bash
  rustup default stable
  rustup component add clippy rustfmt
  ```
- **Just Command Runner**: [just](https://github.com/casey/just) command orchestrator.
- **cargo-nextest**: [cargo-nextest](https://nexte.st/) fast test runner.
- **cargo-dist**: [cargo-dist](https://opensource.axo.dev/cargo-dist/) for release packaging.
- **Python (Optional for Python SDK & Benchmarks)**: Python >= 3.10
- **Node.js (Optional for TypeScript SDK)**: Node.js >= 18 and npm

---

## Building the Project

### Using Just (Recommended)

```bash
# Build workspace in development mode
just build

# Build production release binary
just release
```

### Using Cargo Directly

To compile the entire workspace in debug mode:

```bash
cargo build --workspace
```

To compile the release binary:

```bash
cargo build --release -p m31a-cli
# Resulting binary: target/release/m31a
```

To run the binary directly:

```bash
cargo run -p m31a-cli -- doctor
```

---

## Running Tests

### Workspace Rust Tests

We maintain an uncompromising testing standard. Run tests using Just or Cargo:

```bash
# Run unit & integration tests fast with cargo-nextest
just test-fast

# Run the complete test suite including documentation tests
just test

# Or run directly with Cargo
cargo test --workspace
cargo test --workspace --doc
```

Run crate-specific test suites:

```bash
cargo nextest run -p m31a-tools
cargo nextest run -p m31a-tui
cargo nextest run -p m31a-execpolicy
cargo nextest run -p m31a-sandbox
cargo nextest run -p m31a-agent-engine
```

### Live NVIDIA NIM Tests (Optional)

If you have a valid NVIDIA NIM API key and wish to run live inference validation:

```bash
export NVIDIA_API_KEY="nvapi-..."
export M31A_RUN_LIVE_NVIDIA_TESTS="1"
cargo test -p m31a-provider --features live-nvidia-tests --test live_provider_test -- --ignored --test-threads=1
cargo test -p m31a-agent-engine --features live-nvidia-tests --test live_nvidia_e2e -- --ignored --test-threads=1
```

### Python SDK Tests

```bash
cd sdk/python
python3 -m unittest discover -s tests
```

### TypeScript SDK Tests

```bash
cd sdk/typescript
npm install
npm test
```

---

## Code Style & Quality Standards

All pull requests must pass automated formatting and linting checks in CI.

### Rust Formatting & Linting

Run `rustfmt` across all crates:
```bash
cargo fmt --all -- --check
```
Auto-format your changes:
```bash
cargo fmt --all
```

Run `clippy` with strict warnings:
```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

### Core Architectural Principles

- **No Mocks Policy**: Unit and integration tests should exercise real components, real SQLite in-memory databases, and real filesystem sandboxes wherever feasible. Mocks are restricted solely to offline simulated network endpoints.
- **Fail-Safe Defaults**: Security checks in `m31a-sandbox`, `m31a-execpolicy`, and `m31a-guardian` default to denying or prompting when ambiguous.
- **Error Propagation**: Use `thiserror` for library crate error enumerations and `anyhow` for top-level application orchestration. Preserve full causal chains with `.context(...)`.

---

## Commit Message Conventions

We enforce [Conventional Commits](https://www.conventionalcommits.org/) format:

```
<type>(<scope>): <subject>

[optional body]

[optional footer(s)]
```

### Allowed Types

- `feat`: A new feature or capability.
- `fix`: A bug fix.
- `docs`: Documentation improvements or updates.
- `refactor`: Code change that neither fixes a bug nor adds a feature.
- `perf`: A code change that improves performance.
- `test`: Adding missing tests or correcting existing tests.
- `chore`: Changes to build process, dependency upgrades, or tooling.

### Examples

```
feat(tui): add collapsible DeepSeek R1 reasoning view
fix(patch): handle unicode non-breaking spaces in fuzzy seek
docs(cli): document all 54 CLI subcommands and options
refactor(storage): streamline thread turn pagination queries
```

---

## Submitting a Pull Request

1. **Fork the repository** on GitHub and clone your fork locally.
2. **Create a topic branch** from `master` or `main`:
   ```bash
   git checkout -b feat/my-new-feature
   ```
3. **Make your changes**, keeping them focused and atomic.
4. **Run formatting, lints, and tests**:
   ```bash
   cargo fmt --all
   cargo clippy --workspace --all-targets --all-features -- -D warnings
   cargo test --workspace
   ```
5. **Commit your changes** with descriptive commit messages following Conventional Commits.
6. **Push to your fork** and submit a Pull Request to the `main` branch.
7. Fill out the provided [Pull Request Template](.github/PULL_REQUEST_TEMPLATE.md).

Maintainers will review your PR, provide constructive feedback, and guide it to merge.

---

## Reporting Bugs & Suggesting Features

- **Bugs**: Use the [Bug Report template](.github/ISSUE_TEMPLATE/bug_report.yml). Provide clear reproduction steps and include output from `m31a doctor`.
- **Feature Requests**: Use the [Feature Request template](.github/ISSUE_TEMPLATE/feature_request.yml) to describe the problem and proposed solution.
- **Security Vulnerabilities**: Follow our [Security Policy](SECURITY.md) and disclose privately.

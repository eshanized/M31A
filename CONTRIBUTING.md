# Contributing to M31 Autonomous (M31A)

Thank you for your interest in contributing to M31A! We welcome bug reports, documentation improvements, architectural proposals, and pull requests.

M31A is an autonomous software-engineering runtime built on high-assurance design principles:

> **"The model proposes. The runtime decides."**

To maintain system reliability, security boundaries, and deterministic behavior, all contributions must adhere to the engineering guidelines detailed below.

---

## Non-Negotiable Architectural Invariants

Before writing or refactoring code, please review these fundamental rules:

1. **Single Crate Architecture**: Do NOT split M31A into a Cargo workspace. Module boundaries (`pub(crate)`) must enforce domain encapsulation.
2. **Pure Rust Core**: No Python, Node.js, or GPU runtime dependencies are permitted for the core runtime or tests.
3. **Policy Supremacy**: Every side effect (filesystem, process, git, network) MUST pass through the Layer 1 Policy Gate. No "internal" or "debug" bypasses are allowed.
4. **TUI is a Pure Projection**: The Ratatui terminal cockpit is strictly a read-only projection of authoritative SQLite state; it must never maintain authoritative execution state.
5. **No Fake Success or Placeholders**: Do not introduce mock returns, TODO-only implementations, or silent fallbacks. Unimplemented or blocked paths must return explicit typed errors.
6. **Completion Requires Evidence**: No mission, task, or verification target may be marked complete without deterministic execution evidence.
7. **Bounded Execution**: Every loop, retry counter, process spawn, and memory allocation must be bounded. Never introduce unbounded background operations.
8. **Real Cancellation**: All asynchronous tasks and supervised processes must support instant, clean cancellation and signal escalation.
9. **External Data is Untrusted**: Prompt inputs, repository contents, model outputs, and plugin payloads are untrusted and must pass through trust envelopes and input sanitization.
10. **Layer Dependency Discipline**: Architecture layers (L0 through L9) depend downward only. A lower layer (e.g. L1 Security) may never import or depend on a higher layer (e.g. L8 Autonomy or L9 CLI).

---

## Development Setup

### Prerequisites

- **Rust**: 1.80+ (stable toolchain)
- **Git**: 2.30+
- **Platform**: Linux (x86_64, ARM64), macOS (Apple Silicon, Intel), or Windows (x86_64, ARM64)

### Clone & Build

```bash
git clone https://github.com/eshanized/M31A.git
cd M31A

# Check compilation across all targets (binaries, tests, examples)
cargo check --all-targets

# Build debug binary
cargo build
```

---

## Verification Pipeline

Before submitting a pull request, run the canonical 4-gate verification suite locally in order:

```bash
# 1. Format check
cargo fmt --check

# 2. Type check
cargo check --all-targets

# 3. Linter gate (zero warnings allowed)
cargo clippy --all-targets --all-features -- -D warnings

# 4. Deterministic test suite
cargo test
```

### Running Targeted Test Suites

```bash
# Documentation contract tests
cargo test --test docs_contract

# Core state machines & foundation IDs
cargo test --test phase_01_events_ids_kernel
cargo test --test phase_01_state_machines

# Security hardening & protected path boundary tests
cargo test --test phase_12_security_hardening
cargo test --test security_p0_protected_paths

# Process supervision & background recovery
cargo test --test phase_09_process_jobs_recovery

# Interactive PTY & cockpit tests (Unix only)
cargo test --test golden_tui_pty_scenario
```

---

## Pull Request Guidelines

1. **Focused Commits**: Keep changes atomic and well-documented with clear conventional commit messages (`feat:`, `fix:`, `docs:`, `chore:`, `refactor:`).
2. **Add Tests Alongside Stateful Changes**: Any modification to policies, sandboxes, process management, or task transitions must include accompanying automated tests.
3. **Preserve Documentation Parity**: If you add or modify a CLI subcommand, tool, agent role, profile, or budget dimension, ensure corresponding subsystem docs in `docs/` are updated to keep `docs_contract` passing.
4. **All CI Checks Must Pass Green**: Every PR must pass all matrix checks (Linux x86_64/ARM64, macOS, Windows, CodeQL, deterministic gates).

---

## Reporting Issues

- For bug reports or feature suggestions, please use the structured issue templates in [GitHub Issues](https://github.com/eshanized/M31A/issues).
- For security vulnerabilities, please refer to [SECURITY.md](SECURITY.md) and do NOT file a public issue.

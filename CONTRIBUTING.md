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

- **Rust**: 1.85+ (stable toolchain, per Cargo.toml `rust-version`)
- **Git**: 2.30+
- **Platform**: Linux (x86_64, ARM64), macOS (Apple Silicon, Intel), or Windows (x86_64, ARM64)

### Clone & Build

```bash
git clone https://github.com/eshanized/M31A.git
cd M31A

# Validate environment and toolchain
make bootstrap

# Check compilation across all targets (both channels)
make check

# Build debug binary (production channel by default)
make build

# Or build development channel binary
make build-dev

# Install locally side-by-side (installs m31a-dev into PATH)
make install-dev
```

Channel notes (see `docs/DEPLOYMENT.md`): build profiles (`dev`/`release`)
are compiler settings, not the deployment channel. The channel is the
compile-time `development` cargo feature (production = default). A
development build is never a policy/sandbox bypass — both channels enforce
identical security controls.

---

## Verification Pipeline

Before submitting a pull request, run the canonical verification suite:

```bash
make verify
```

`make verify` executes all 4 canonical quality gates in order:
1. Format check (`make fmt-check`)
2. Configuration authority and secret guards (`make config-guard`, `make secret-scan`)
3. Type checks across both channels (`make check`)
4. Channel mutual exclusion check (`make check-channels`)
5. Linter checks across both channels (`make clippy`)
6. Full deterministic test suite (`make test`)

### Targeted Verification Commands

```bash
# Run unit tests only (fast, bounded resources)
make test-unit

# Run all TUI & onboarding regression suites
make test-tui

# Run real POSIX pseudo-terminal (PTY) lifecycle tests
make test-pty

# Run critical smoke verification suite (unit, contracts, PTY)
make smoke

# Run individual integration test targets
cargo test --test docs_contract
cargo test --test phase_01_events_ids_kernel
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

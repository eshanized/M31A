<div align="center">

# M31A

### The autonomous coding agent that lives in your terminal.

> **The model proposes. The runtime decides.**

[![CI](https://github.com/eshanized/M31A/actions/workflows/release-gates.yml/badge.svg)](https://github.com/eshanized/M31A/actions/workflows/release-gates.yml)
[![Release](https://img.shields.io/github/v/release/eshanized/M31A?color=coral)](https://github.com/eshanized/M31A/releases/latest)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](rust-toolchain.toml)
[![Website](https://img.shields.io/badge/website-m31a.tonmoyinfrastructure.org-coral.svg)](https://m31a.tonmoyinfrastructure.org/)

[Website](https://m31a.tonmoyinfrastructure.org/) &bull; [Documentation](docs/README.md) &bull; [Quickstart](#quickstart) &bull; [Architecture](docs/architecture/ARCHITECTURE.md) &bull; [Releases](https://github.com/eshanized/M31A/releases)

</div>

---

## What is M31A?

**M31A** (M31 Autonomous) is an open-source, Rust-native autonomous coding agent designed from first principles to live directly in your terminal. It takes high-level software engineering objectives, inspects your codebase, synthesizes dependency-aware execution plans, writes code, runs builds and test suites, diagnoses compiler failures, and iteratively fixes regressions until your acceptance criteria are met.

### Who is it for?
Software engineers who want genuine autonomous capability for non-trivial engineering tasks—refactoring architectures, resolving regressions, updating dependencies, porting APIs, or writing regression test suites—without surrendering shell access or trusting a black-box model to run wild in their repository.

### What makes it different?
Conventional coding agents hand an LLM direct shell execution and ambient credentials, hoping the model does not delete files, leak tokens, or fall into hallucinated infinite loops.

M31A enforces a fundamental architectural boundary:

> **Intelligence is not authority.**

The large language model is treated as an untrusted reasoning engine that proposes structured actions. The **Rust runtime strictly owns all state, policy evaluation, process sandboxing, atomic checkpoints, and verification gates**. A task is never admitted as complete simply because the model outputs "I finished"—it is marked complete only when empirical evidence (compiler checks, test passes, lint cleanups) proves that the code functions correctly.

```
┌─────────────────────────────────────────────────────────────┐
│                    THE MODEL PROPOSES                       │
│    (Untrusted Reasoning: Planning, Code Edits, Analysis)    │
└──────────────────────────────┬──────────────────────────────┘
                               │ Structured Actions
                               ▼
┌─────────────────────────────────────────────────────────────┐
│                    THE RUNTIME DECIDES                      │
│                                                             │
│   [ 11-Stage Policy Gate ] ─── ALLOW / DENY / ASK           │
│   [ Process Confinement  ] ─── Sandboxes, cgroups, limits   │
│   [ Atomic Checkpoints   ] ─── Two-phase SQLite persistence │
│   [ Evidence Verifier    ] ─── Builds, tests, diagnostics   │
└──────────────────────────────┬──────────────────────────────┘
                               │ Verified Side Effects
                               ▼
┌─────────────────────────────────────────────────────────────┐
│                   YOUR LOCAL REPOSITORY                     │
└─────────────────────────────────────────────────────────────┘
```

---

## Visual Demo

### Interactive Terminal Cockpit (`m31a tui`)

M31A features a conversation-first terminal cockpit built on [Ratatui](https://github.com/ratatui/ratatui) with calm semantic design tokens, quiet status headers, and borderless conversation streams:

```
 M31A · nvidia/nemotron-3-ultra-550b-a55b · master · active                08:30:15 
────────────────────────────────────────────────────────────────────────────────────
 You
 Refactor authentication service to use Argon2 password hashing and add tests.

 M31A
 I will inspect the current authentication module, add the argon2 dependency,
 update the hash and verify methods, and run the test suite to verify.

 ┌─ Plan (DAG) ───────────────────────────────────────────────────────────────────┐
 │ [✓] 1. Inspect src/auth/service.rs and current hash implementations            │
 │ [✓] 2. Add argon2 dependency to Cargo.toml                                     │
 │ [●] 3. Refactor hash_password and verify_password to Argon2id                  │
 │ [ ] 4. Run test suite: cargo test --test auth_tests                            │
 └────────────────────────────────────────────────────────────────────────────────┘

 Tool · write_file(src/auth/service.rs)
 ┌─ Diff Preview ─────────────────────────────────────────────────────────────────┐
 │ - let salt = salt::generate();                                                │
 │ - sha256_hash(password, &salt)                                                 │
 │ + let salt = SaltString::generate(&mut OsRng);                                 │
 │ + Argon2::default().hash_password(password.as_bytes(), &salt)                 │
 └────────────────────────────────────────────────────────────────────────────────┘

 Verification · cargo test --test auth_tests
 running 6 tests
 test tests::test_hash_and_verify ... ok
 test tests::test_invalid_password_rejected ... ok
 test tests::test_argon2id_algorithm_identifier ... ok
 test result: ok. 6 passed; 0 failed; finished in 0.42s
────────────────────────────────────────────────────────────────────────────────────
 > Refactor complete and verified. Ready for review.█
────────────────────────────────────────────────────────────────────────────────────
 [Esc] Navigation  [Enter] Submit  [/] Slash Commands  [^C] Cancel
```

### Headless Autonomous Execution (`m31a mission run`)

Prefer single-command execution in scripts or CI? Launch missions directly from your shell:

```bash
$ m31a mission run "Refactor database queries to parameterized statements" --profile coding

[INFO] Initializing runtime: workspace=/home/user/project channel=production
[INFO] Synthesizing execution plan...
[PLAN] Generated DAG with 3 tasks (topological wave execution enabled)
[STEP 1/3] Analyze SQL query usage in src/db/
       -> Policy: tool_call 'read_file' -> ALLOW
[STEP 2/3] Replace string concatenations with sqlx query! macro
       -> Policy: tool_call 'edit_file' -> ALLOW
       -> Applier: atomic 2-file mutation staged
[STEP 3/3] Empirical Verification Gate
       -> Executing: cargo check --all-targets ... PASS
       -> Executing: cargo test --test db_tests ... PASS (14/14 tests)
[EVIDENCE] Verification passed: 0 regressions, all criteria satisfied
[MISSION COMPLETE] ID: 0192a6b2-8c11-7390-9941-2a6288ff3001 (duration: 18.4s)
```

---

## Why M31A?

| Feature | Conventional AI Coding Scripts | M31A Autonomous Runtime |
|:---|:---|:---|
| **Authority** | The LLM executes arbitrary shell commands. | **The model proposes; the runtime decides.** |
| **Safety & Policy** | None or simple regex blacklists. | **11-stage non-bypassable policy evaluation gate.** |
| **Completion Criteria** | Model declares "I'm done" (hallucinated). | **Empirical verification only** (compilers, test passes, linters). |
| **Interruption & Crash Recovery** | Session lost; repository left in broken state. | **Two-phase atomic SQLite checkpoints & differential replanning.** |
| **Execution Confinement** | Unrestricted host filesystem and network. | **Sandboxed processes, path containment, and OS confinement.** |
| **Secret Management** | API keys and tokens exposed to model logs. | **Automated zero-leak regex credential redactor.** |
| **Runtime Dependencies** | Node.js, Python, heavy Docker images, or GPU. | **Single clean Rust binary with zero foreign runtime dependencies.** |

---

## Core Capabilities

- **Acyclic DAG Task Planning**: Natural language goals are decomposed into deterministic topological directed acyclic graphs (DAGs) with wave parallelization, concurrency limits, and critical path scheduling.
- **Conversation-First Terminal Cockpit**: Clean, typographic TUI built on Ratatui with subtle role labels (`You`, `M31A`, `Tool`, `Verification`), interactive diff inspection, approval prompts, and autocomplete slash commands (`/`).
- **11-Stage Non-Bypassable Policy Gate**: Evaluates every side effect against a comprehensive rule matrix (`ALLOW`, `DENY`, `ASK`, `ESCALATE`). Dangerous commands and sensitive file modifications are stopped before execution.
- **Empirical Multi-Tier Verification**: Built-in verification engine executes compiler passes, unit tests, and security checks. When builds fail, the diagnostician correlates errors to root causes and proposes heuristic repairs.
- **Two-Phase Crash Recovery**: System state, task progress, and artifacts are committed to SQLite via two-phase atomic transactions. If a run is interrupted, M31A automatically detects the crash on startup and safely resumes.
- **Isolated Git Attribution**: Work happens in dedicated worktrees. Commits include verifiable attribution metadata (`M31A-Mission`, `M31A-Task`, `M31A-Agent`) without polluting your main working branch until approved.
- **Zero-Leak Telemetry & Auditing**: Telemetry streams (NDJSON) and compact SQLite event logs sanitize credentials (NVIDIA keys, JWTs, private keys, database connection strings) before anything touches disk.

---

## Quickstart

### 1. Installation

#### One-Liner Install Script

On **Linux** and **macOS**:
```bash
curl -fsSL https://raw.githubusercontent.com/eshanized/M31A/master/scripts/install.sh | bash
```

On **Windows** (PowerShell):
```powershell
irm https://raw.githubusercontent.com/eshanized/M31A/master/scripts/install.ps1 | iex
```

#### Pre-Built Standalone Binaries

Download official standalone release archives directly from [GitHub Releases](https://github.com/eshanized/M31A/releases/latest):

| Platform / Architecture | Target Triple | Archive |
|:---|:---|:---|
| **Linux (x86_64)** | `x86_64-unknown-linux-gnu` | [`m31a-0.1.5-linux-x64.tar.gz`](https://github.com/eshanized/M31A/releases/latest) |
| **Linux (ARM64)** | `aarch64-unknown-linux-gnu` | [`m31a-0.1.5-linux-arm64.tar.gz`](https://github.com/eshanized/M31A/releases/latest) |
| **macOS (Apple Silicon)** | `aarch64-apple-darwin` | [`m31a-0.1.5-darwin-arm64.tar.gz`](https://github.com/eshanized/M31A/releases/latest) |
| **macOS (Intel)** | `x86_64-apple-darwin` | [`m31a-0.1.5-darwin-x64.tar.gz`](https://github.com/eshanized/M31A/releases/latest) |
| **Windows (x86_64)** | `x86_64-pc-windows-msvc` | [`m31a-0.1.5-windows-x64.zip`](https://github.com/eshanized/M31A/releases/latest) |
| **Windows (ARM64)** | `aarch64-pc-windows-msvc` | [`m31a-0.1.5-windows-arm64.zip`](https://github.com/eshanized/M31A/releases/latest) |

*Each release archive includes the standalone binary, documentation, license, SHA-256 checksum, and deployment manifest.*

#### Build from Source

Requirements: Rust 1.85+ (`cargo`) and `make`.

```bash
git clone https://github.com/eshanized/M31A.git
cd M31A
make bootstrap
make release
make install
```

---

### 2. Configure Model Credentials

M31A uses **NVIDIA NIM** as its high-performance production inference provider:

```bash
# Set your NVIDIA NIM API key
export NVIDIA_API_KEY="nvapi-..."

# Or persist locally in your workspace (.m31a/config.toml, chmod 0600):
m31a config set-credential nvidia_nim "nvapi-..."
```

*(Optional)* Override the active model (default: `nvidia/nemotron-3-ultra-550b-a55b`):
```bash
export M31A_MODEL="meta/llama-3.3-70b-instruct"
# or pass --model flag on any command
```

---

### 3. Launch

```bash
# Launch interactive terminal cockpit
m31a tui

# Or start an interactive session directly
m31a

# Or execute a focused autonomous mission from CLI
m31a mission run "Add integration tests for user registration endpoint" --profile coding

# Run system diagnostic checks
m31a doctor
```

---

## Example Mission

Here is a step-by-step example of executing an autonomous refactoring mission:

```bash
m31a mission run \
  "Extract database configuration into strongly typed Settings struct and validate with tests" \
  --profile coding
```

### What M31A does automatically:
1. **Genesis & Repo Scan**: Scans repository AST and workspace structure to build an in-memory knowledge graph of dependencies and callers.
2. **DAG Planning**: Decomposes the mission into discrete tasks:
   - Task A: Create `src/config/settings.rs` with `Settings` struct and serde validation.
   - Task B: Update `src/db/pool.rs` to consume `Settings`.
   - Task C: Add unit tests in `tests/config_test.rs`.
3. **Policy Gate Evaluation**: Each tool request passes through policy. Reading files is granted (`ALLOW`); modifying source code is verified within workspace boundaries.
4. **Sandboxed Mutation**: Edits are staged atomically. If an edit fails or introduces syntax errors, changes are rolled back.
5. **Continuous Verification**: M31A runs `cargo check` and `cargo test`. If a type mismatch occurs, the diagnostician extracts compiler errors, feeds structured diagnostics to the agent, and repairs the discrepancy.
6. **Completion Evidence**: When tests pass with zero regressions, the mission commits changes to a clean git branch with full telemetry provenance.

---

## How It Works: The Execution Loop

Every mission follows a deterministic 4-stage lifecycle:

```
    ┌──────────┐
    │  01 PLAN │  Acyclic DAG Decomposition & Wave Scheduling
    └────┬─────┘
         ▼
    ┌──────────┐
    │ 02 APPLY │  Sandboxed Execution & 11-Stage Policy Gate
    └────┬─────┘
         ▼
    ┌──────────┐
    │ 03 CHECK │  Empirical Verification (Compiler, Tests, Lints)
    └────┬─────┘
         ▼
    ┌──────────┐
    │ 04 STATE │  Two-Phase SQLite Checkpoint & Git Attribution
    └──────────┘
```

1. **Plan (DAG Decomposition)**: The planner analyzes constraints, risks, and requirements, producing a dependency graph where independent tasks run in parallel waves.
2. **Execute (Sandboxed Confinement)**: Agents invoke tools from a catalog of 28 core tools. Each action must pass the 11-stage policy gate before any side effect occurs.
3. **Verify (Empirical Evidence)**: The runtime executes compiler checks, automated unit/integration tests, and security scans. Model assertions of completion are ignored without hard evidence.
4. **Recover (Atomic Checkpoints)**: Completed tasks are sealed with SHA-256 manifests. In the event of a system crash, power failure, or process cancellation, the runtime resumes cleanly without re-executing verified tasks.

---

## Architecture (L0–L9)

M31A enforces a strict downward-dependency layered hierarchy. Lower layers never import or depend on higher layers; the TUI and CLI are pure projections of authoritative SQLite state:

```
L9  User Interface    CLI Commands, Interactive Ratatui TUI Cockpit
L8  Autonomy          Mission Orchestrator, Halting Evaluator, Loop Detector
L7  Verification      Test Runners, Compiler Diagnostician, Crash Recovery
L6  Execution         Job Engine, Worktrees, Artifact Storage
L5  Planning          Candidate DAG, Topological Sorter, CPM Reconciler
L4  Agents            Role State Machines (Planner, Coder, Reviewer, Tester)
L3  Intelligence      Prompt Envelopes, Delimiter Escaping, Context Memory
L2  Capabilities      Tool Registry (28 tools), Process Confinement, Sandboxes
L1  Security          11-Stage Policy Gate, ASVS Hardening, Secret Redactor
L0  Kernel            Strongly-Typed IDs, Foundation Types, Error Infrastructure
```

---

## Security & Governance

M31A is engineered under the assumption that external inputs (repository contents, web pages, and LLM responses) are potentially adversarial:

- **11-Stage Policy Gate**: Every file write, terminal command, and git commit is evaluated against precedence rules: Workspace containment &rarr; Sensitive path denial (`.git/`, `.env`, credentials) &rarr; Tool safety limits &rarr; Human approval.
- **Prompt Injection Defense**: All untrusted external text (file contents, compiler logs, git diffs) is wrapped in structured XML envelopes with strictly escaped delimiters to prevent prompt injection.
- **Zero-Leak Redactor**: Telemetry and audit streams scrub credentials using real-time regex matchers for API keys, tokens, SSH keys, and connection strings.
- **Bounded Resource Budgets**: 10 distinct budget dimensions (execution time, tokens, process count, file mutations) enforce hard limits to prevent runaway loops.

---

## Platform & Provider Support

### Platform Qualification Matrix

| Platform | Target | Status | Confinement Mechanism |
|:---|:---|:---|:---|
| **Linux (x86_64)** | `x86_64-unknown-linux-gnu` | **Primary / Fully Qualified** | cgroups v2, POSIX rlimits, namespaces |
| **Linux (ARM64)** | `aarch64-unknown-linux-gnu` | **Pre-Built / CI Verified** | POSIX rlimits, path containment |
| **macOS (Apple Silicon)** | `aarch64-apple-darwin` | **Pre-Built / CI Verified** | Seatbelt (`sandbox-exec`), POSIX rlimits |
| **macOS (Intel)** | `x86_64-apple-darwin` | **Pre-Built / CI Verified** | Seatbelt (`sandbox-exec`), POSIX rlimits |
| **Windows (x86_64)** | `x86_64-pc-windows-msvc` | **Pre-Built / CI Verified** | Windows Job Objects, ACL containment |
| **Windows (ARM64)** | `aarch64-pc-windows-msvc` | **Pre-Built / CI Verified** | Windows Job Objects, ACL containment |

*For complete details on platform capability guarantees and sandbox isolation tiers, see [`docs/PLATFORM-SUPPORT.md`](docs/PLATFORM-SUPPORT.md).*

### Model Provider Support

- **Production Provider**: **NVIDIA NIM** (`nvidia_nim`). Supports state-of-the-art models including `nvidia/nemotron-3-ultra-550b-a55b`, `meta/llama-3.3-70b-instruct`, and `deepseek-ai/deepseek-r1`.
- Dynamic model discovery automatically queries available models on startup.
- Retired legacy provider identifiers are rejected deterministically with clear remediation guidance.

---

## Documentation

Full architectural manuals, subsystem guides, and specifications:

| Guide | Document | Description |
|:---|:---|:---|
| **Runtime Architecture** | [`docs/architecture/ARCHITECTURE.md`](docs/architecture/ARCHITECTURE.md) | Complete layered architecture, state machines, and dependency rules. |
| **Runtime Authority** | [`docs/architecture/RUNTIME-AUTHORITY.md`](docs/architecture/RUNTIME-AUTHORITY.md) | Authoritative runtime ownership and non-delegable execution principles. |
| **Prompt Authority** | [`docs/architecture/PROMPT-AUTHORITY.md`](docs/architecture/PROMPT-AUTHORITY.md) | Delimiter smuggling defense, trust envelopes, and PromptOS v2 contracts. |
| **CLI & Telemetry** | [`docs/subsystems/CLI.md`](docs/subsystems/CLI.md) | Full CLI reference, subcommand hierarchy, flags, and exit codes. |
| **Autonomy Engine** | [`docs/subsystems/AUTONOMY.md`](docs/subsystems/AUTONOMY.md) | Autonomy levels, sliding-window loop detection, and budget enforcers. |
| **Policy System** | [`docs/subsystems/POLICY.md`](docs/subsystems/POLICY.md) | 11-stage policy gate, decision matrix (`ALLOW`/`DENY`/`ASK`), and escalation rules. |
| **Security & Sandbox** | [`docs/subsystems/SECURITY.md`](docs/subsystems/SECURITY.md) | ASVS L1 compliance, threat model, secret redactor, and process isolation. |
| **Tool Catalog** | [`docs/subsystems/TOOLS.md`](docs/subsystems/TOOLS.md) | Complete reference for all 28 core tools, schemas, and risk tiers. |
| **Recovery & Checkpoints** | [`docs/subsystems/RECOVERY.md`](docs/subsystems/RECOVERY.md) | Two-phase atomic checkpoints, startup crash scanner, and differential replanning. |
| **Testing Strategy** | [`docs/subsystems/TESTING.md`](docs/subsystems/TESTING.md) | Verification suites, security regression tests, and evaluation harnesses. |
| **Platform Support** | [`docs/PLATFORM-SUPPORT.md`](docs/PLATFORM-SUPPORT.md) | Authoritative cross-platform qualification and capability parity matrix. |
| **Release Engineering** | [`docs/RELEASE.md`](docs/RELEASE.md) | Release validation gates, artifact verification, and deployment pipelines. |

---

## Project Status

M31A is an active, production-focused open-source runtime with core Phases 1–36 implemented and continuously verified.

- **Current Release**: `v0.1.5`
- **Test Suite**: >820 deterministic unit and integration tests passing in CI across all platforms.
- **License**: Dual-licensed under MIT or Apache 2.0.

---

## Contributing

We welcome contributions from the systems programming and AI engineering communities!

- Review our **[Contributing Guide](CONTRIBUTING.md)** for coding standards, architectural rules, and local development workflows.
- Check our **[Code of Conduct](CODE_OF_CONDUCT.md)** for community guidelines.
- To run the full verification suite locally:
  ```bash
  cargo fmt --check
  cargo check --all-targets
  cargo clippy --all-targets -- -D warnings
  cargo test
  ```

---

## Security

Security vulnerabilities are handled with highest priority. Please do **NOT** file public GitHub issues for security disclosures.

Refer to our **[Security Policy](SECURITY.md)** for instructions on responsible, confidential disclosure.

---

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE) or http://opensource.org/licenses/MIT)

at your option.

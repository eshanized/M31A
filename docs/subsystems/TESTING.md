# M31A Verification & Testing Strategy

This document describes the multi-layered testing strategy, subsystem integration test matrix, security hardening suite, evaluation harness, acceptance scenarios A through H, and continuous verification gates in M31 Autonomous (M31A).

## Core Verification Principle

> **"Completion requires evidence. Never fake success; test real production paths with zero mock bypasses."**

M31A is verified through rigorous, defense-in-depth testing layers ranging from fast unit tests to deterministic threat-vector security audits and isolated E2E acceptance benchmarks.

---

## Multi-Layer Verification Hierarchy

```
Layer 5: Continuous CI Verification Gates (fmt, clippy, check, test)
  ↑
Layer 4: Golden Acceptance Evaluation Harness (Scenarios A–H in Temp Fixtures)
  ↑
Layer 3: Adversarial Security Hardening Suite (11 ASVS Threat Vectors)
  ↑
Layer 2: Subsystem Integration Tests (Phases 01 through 12 in tests/)
  ↑
Layer 1: Unit & Component Contract Tests (src/**/*.rs)
```

---

## Subsystem Integration Test Matrix (`tests/`)

The M31A integration test suite comprehensively exercises every subsystem through end-to-end integration contracts:

| Integration Test File | Primary Subsystem | Coverage Highlights |
|:---|:---|:---|
| `phase_01_events_ids_kernel.rs` | L0 Kernel | Strongly typed IDs (`MissionId`, `TaskId`), event envelopes, error contracts. |
| `phase_01_persistence_streams_artifacts.rs` | L0/L6 Persistence | SQLite connection pooling, append-only streams, artifact store. |
| `phase_01_state_machines.rs` | L0/L4 State Machines | Task, Agent, and Mission state transitions and valid event triggers. |
| `phase_02_intake_completion_recovery.rs` | L8 Intake & Gates | `MissionIntake` validation, `CompletionGate`, recovery budget trackers. |
| `phase_02_service_bus_transactions.rs` | L0 Event Bus | Broadcast event bus, subscriber filters, transaction atomicity. |
| `phase_03_autonomy_controller.rs` | L8 Autonomy Loop | 12-stage mission controller loop, state progression, and exit states. |
| `phase_04_planning_authority.rs` | L5 Planning | `CandidatePlan` validation, non-empty tasks, acyclic DAG verification. |
| `phase_05_replan_reconciliation.rs` | L5 DAG Reconciler | Task completion preservation, differential DAG reconciliation. |
| `phase_05_resource_manager.rs` | L2/L8 Resources | Concurrency limits, step budgeting, resource tracking. |
| `phase_05_scheduler_concurrency.rs` | L5 Scheduler | Topological task scheduling, parallel wave execution. |
| `phase_05_task_dag_persistence.rs` | L5 DAG Persistence | SQLite task graph serialization, dependency edges, status queries. |
| `phase_06_agent_runtime.rs` | L4 Agent Runtime | Multi-agent coordination, role-specific prompts, context compilation. |
| `phase_07_models_context_repo.rs` | L3 Intelligence | XML trust envelopes, prompt compilers, repository AST queries. |
| `phase_08_capability_registry_seams.rs` | L2 Capabilities | 15 capability families, provider bindings, availability checks. |
| `phase_08_execution_pipeline.rs` | L6 Execution | Tool execution pipeline, streaming output interception. |
| `phase_08_process_jobs_supervisor.rs` | L6 Job Supervisor | Child process group supervision, signal escalation, output spooling. |
| `phase_08_tool_catalog_schemas.rs` | L2 Tool Catalog | Parameter validation for all 28 core tools against JSON schemas. |
| `phase_09_interactive_approval_grants.rs` | L1 Approval System | `ApprovalCoordinator`, durable SQLite policy grants, session resolution. |
| `phase_09_policy_precedence_invariants.rs` | L1 Policy Precedence | 10-tier layer precedence, non-weakening decision merger. |
| `phase_09_process_jobs_recovery.rs` | L6/L7 Job Recovery | Abandoned process detection, spool sealing with `[INTERRUPTED]`. |
| `phase_09_sandbox_isolation_confinement.rs`| L1/L2 Sandbox | Path traversal blocking, POSIX rlimits, workspace sandboxing. |
| `phase_10_checkpoints_crash_recovery.rs` | L7 Checkpoints | Two-phase checkpoint commits, startup crash recovery scanner. |
| `phase_10_failure_recovery_loop.rs` | L7 Fault Recovery | 15 failure classifications, loop detection, differential replanning. |
| `phase_10_verification_hierarchy.rs` | L7 Verification | 7-tier verification hierarchy, semantic reviews, evidence linking. |
| `phase_11_cli.rs` | L9 CLI Interface | CLI subcommand parsing, exit codes, and JSON output formatting. |
| `phase_11_config.rs` | L9 Configuration | 7-tier configuration precedence, platform-aware paths, profiles. |
| `phase_11_git.rs` | L2/L6 Git Operations | Worktree management, staging, diffs, RFC-compliant commit trailers. |
| `phase_11_plugins_hooks.rs` | L2/L9 Plugins & Hooks | In-process plugin traits, 4-stage lifecycle hooks, policy subordination. |
| `phase_11_tui.rs` | L9 TUI Cockpit | Ratatui terminal cockpit, telemetry rendering, interactive screens. |
| `phase_12_completion_reports.rs` | L7/L8 Reporting | 16-field `CompletionReport`, dual Markdown/JSON projection, evidence locators. |
| `phase_12_resource_budgets.rs` | L2/L8 Resource Budgets | 10-dimensional budget model, two-phase reservation/settlement, quotas. |
| `phase_12_skills_profiles.rs` | L2/L4 Skills & Profiles | Declarative `SKILL.toml`, multi-tier discovery, 7 canonical profiles. |
| `phase_12_telemetry_correlation.rs` | L0/L9 Telemetry | W3C correlation context, multi-tier secret redaction, CLI inspect. |
| `phase_12_security_hardening.rs` | L1 Security Hardening | Comprehensive 11-threat matrix verification suite (ASVS L1). |
| `phase_12_eval_harness.rs` | L9 Evaluation Harness | Golden acceptance scenarios A–H and scorecard generation in temp fixtures. |
| `isolation_boundary_hardening.rs` | L1/Git Authority | Worktree isolation defaults ("required" vs "best_effort"), fail-closed on missing git or worktree failure, isolated execution. |

---

## Adversarial Security Hardening Suite (`phase_12_security_hardening.rs`)

A dedicated integration test suite explicitly asserts runtime boundaries across 11 canonical threat vectors:
1. **Root-scoped filesystem traversal escapes**: `..`, redundant slashes, symlink escapes strictly blocked.
2. **Direct `execve` injection**: Chained commands (`;`, `&&`) and dangerous dynamic loaders (`LD_PRELOAD`, `NODE_OPTIONS`) stripped.
3. **Secret leakage prevention**: Scrubbing of API keys, bearer tokens, AWS credentials, and private keys.
4. **Prompt injection isolation**: XML trust envelopes escape closing tags, defeating delimiter smuggling.
5. **Unattended ASK fail-closed**: Approval prompts convert to DENY in non-interactive autonomy modes.
6. **Plugin sandbox subordination**: In-process plugins are strictly evaluated against `PolicyGate`.
7. **Terminal escape sanitization**: ANSI cursor repositioning and OSC titles stripped from output.
8. **Process cancellation**: Process groups are reaped completely with signal escalation; zero zombie leaks.
9. **Corrupted checkpoint recovery**: Missing or tampered artifacts fail closed to `Corrupt` or `Ambiguous`.
10. **Future schema tampering**: Unsupported schema versions (e.g. `version = 999`) rejected fail-closed.
11. **Resource exhaustion clamping**: Streaming byte limits and budget exhaustion boundaries strictly enforced.

---

## Autonomous Evaluation Harness (`m31a eval run`)

The autonomous evaluation harness executes canonical acceptance scenarios A through H against pristine, isolated temporary Git fixtures (`FixtureRepoBuilder`):

- **Scenario A (Simple Bug Fix)**: Isolates failing test, modifies single file, verifies fix passes, records commit trailers.
- **Scenario B (Multi-File Feature)**: Implements cross-cutting caching across multiple modules, verifies clean worktree.
- **Scenario C (Failure and DAG Replan)**: Detects compilation error, classifies as `Compilation`, triggers `DifferentialReplanEngine`, records replan in SQLite, and applies corrective fix.
- **Scenario D (Policy Denial)**: Evaluates unauthorized credential read (`/etc/shadow`, `~/.ssh/id_rsa`), returns `DENY` fail-closed with zero mutations.
- **Scenario E (Unattended ASK Fail-Closed)**: Dispatches interactive approval in unattended mode; strictly converts to `DENY`.
- **Scenario F (Crash Recovery & Resume)**: Simulates runtime crash mid-execution; scanner recovers state to `SafeToResume`, preserves Task 1, and reschedules in-flight Task 2.
- **Scenario G (Malicious Prompt Injection)**: Quarantines adversarial prompt injection in README; defeats delimiter smuggling; policy blocks secret extraction.
- **Scenario H (Huge Output Artifact Quota)**: Clamps runaway compiler diagnostics with `StreamingQuotaWriter`, preserving protected verification artifacts.

### Running Evaluations

```bash
# Run all acceptance scenarios and format as markdown summary
m31a eval run --all

# Run a single targeted scenario
m31a eval run --scenario a

# Output machine-readable JSON scorecard
m31a eval run --all --output json
```

---

## Continuous Verification Build Gates

Before any commit or release, the following 4 build commands MUST pass cleanly in order:

```bash
cargo fmt --check
cargo check
cargo clippy --all-targets -- -D warnings   # then repeat with --features development
cargo test-min # or ./scripts/test-minimal.sh
```

---

## Minimal Resource Testing System

M31A contains over 100 integration test binaries under `tests/`. Unconstrained test execution (`cargo test --all-targets`) compiles and links dozens of test binaries in parallel with full debug symbols, which can consume tens of gigabytes of RAM and lock all CPU cores.

To prevent out-of-memory errors and processor starvation, M31A implements a multi-tiered Minimal Resource Testing System:

### 1. Build Profile Optimization (`Cargo.toml`)
- `[profile.test]` sets `debug = 1` (line-tables-only). This preserves file and line numbers for panic backtraces while slashing debug symbol size by ~85-90%, preventing multi-gigabyte linker memory bloat.
- Incremental compilation is disabled (`incremental = false`) for the test profile to eliminate disk cache thrashing and memory overhead during batch test execution.

### 2. Workspace Cargo Configuration (`.cargo/config.toml`)
- `[build] jobs = 2`: Limits parallel `rustc` compiler jobs to 2.
- `[env] RUST_TEST_THREADS = "2"`: Constrains the standard test runner thread pool to 2 concurrent threads across all test executions.
- `[alias]`: Provides fast shortcuts:
  - `cargo test-min`: Executes all library unit tests with minimal resources.
  - `cargo test-minimal`: Runs library tests bounded to 2 jobs and 2 test threads.
  - `cargo test-all-min`: Runs all tests under bounded concurrency.

### 3. Runtime Verification Bounding (`src/verification/`)
- Both `LocalVerificationProvider` and `TestRunner` automatically inject `RUST_TEST_THREADS=2` and `CARGO_BUILD_JOBS=2` into child process environments, ensuring that automated verification tasks spawned by autonomous agents never freeze the host system.

### 4. Dedicated Test Runner Script (`scripts/test-minimal.sh`)
- Executable helper script providing CPU niceness (`nice -n 19`), resource bounds, and execution modes:
  ```bash
  ./scripts/test-minimal.sh              # Library unit tests (default, ~7s, <100MB RAM)
  ./scripts/test-minimal.sh --test <NAME> # Targeted integration test
  ./scripts/test-minimal.sh --all        # Unit tests + critical integration suites
  ./scripts/test-minimal.sh --check      # cargo check + tests
  ```

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

The M31A integration test suite comprehensively exercises every subsystem through end-to-end integration contracts organized into functional subsystem domains:

### 1. Runtime Kernel, Events, IDs, and State Machines
| Integration Test File | Primary Subsystem | Coverage Highlights |
|:---|:---|:---|
| `runtime_kernel_events_and_ids.rs` | L0 Kernel | Strongly typed IDs (`MissionId`, `TaskId`), event envelopes, error contracts. |
| `runtime_state_machines.rs` | L0/L4 State Machines | Task, Agent, and Mission state transitions and valid event triggers. |
| `event_bus_transactions.rs` | L0 Event Bus | Broadcast event bus, subscriber filters, transaction atomicity. |

### 2. Persistence, Repositories, and Transactions
| Integration Test File | Primary Subsystem | Coverage Highlights |
|:---|:---|:---|
| `persistence_streams_and_artifacts.rs` | L0/L6 Persistence | SQLite connection pooling, append-only streams, artifact store. |
| `task_graph_persistence.rs` | L5 DAG Persistence | SQLite task graph serialization, dependency edges, status queries. |

### 3. Mission Lifecycle and Autonomy
| Integration Test File | Primary Subsystem | Coverage Highlights |
|:---|:---|:---|
| `mission_intake_completion_and_recovery.rs` | L8 Intake & Gates | `MissionIntake` validation, `CompletionGate`, recovery tracking. |
| `autonomy_controller.rs` | L8 Autonomy Loop | 12-stage mission controller loop, state progression, and exit states. |
| `mission_lifecycle_governance_and_integrity.rs` | L8 Governance | Mission lifecycle governance, state integrity invariants, transition validations. |
| `autonomous_mission_production_proof.rs` | L8 Autonomy | Multi-turn autonomous mission proof, journey tests A–E, live cancellation/resume honesty. |
| `autonomous_engineering_loop_lifecycle.rs` | L8 Lifecycle | End-to-end autonomous software engineering loop, multi-step orchestration. |

### 4. Planning, Task Graphs, and Scheduling
| Integration Test File | Primary Subsystem | Coverage Highlights |
|:---|:---|:---|
| `planning_authority.rs` | L5 Planning | `CandidatePlan` validation, non-empty tasks, acyclic DAG verification. |
| `task_graph_reconciliation.rs` | L5 DAG Reconciler | Task completion preservation, differential DAG reconciliation. |
| `resource_manager_limits.rs` | L2/L8 Resources | Concurrency limits, resource limits, resource tracking. |
| `scheduler_concurrency.rs` | L5 Scheduler | Topological task scheduling, parallel wave execution. |
| `scheduler_state_machine_and_cancellation.rs` | L5 Scheduler | Task state machine transitions, retryable failure, cancellation cascade. |
| `adaptive_planning_engine_and_quality_metrics.rs` | L5 Planning | Adaptive planning engine, PlanQualityMetrics, ReplanScope classification. |
| `planning_retry_and_replan_limits.rs` | L5 Planning | Adaptive replanning limits, bounded planning retries, fail-closed replan exhaustion. |

### 5. Agent Runtime, Delegation, and Handoff
| Integration Test File | Primary Subsystem | Coverage Highlights |
|:---|:---|:---|
| `agent_runtime.rs` | L4 Agent Runtime | Multi-agent coordination, role-specific prompts, context compilation. |
| `agentic_core_coordination_and_dispatch.rs` | L4 Agent Core | Unified agent runner, role handoffs, execution envelopes. |
| `agent_role_envelopes_and_collaboration.rs` | L4 Collaboration | Role envelope isolation, multi-agent collaboration, delegation boundaries. |
| `runtime_handoff_and_revision_binding.rs` | L4/L5 Handoff | Revision binding between plan, task, and authorization; stale state defense. |
| `research_ownership_and_prerequisite_evidence.rs` | L4 Research | Research role ownership, prerequisite evidence generation before coding tasks. |

### 6. Model Integration and Repository Intelligence
| Integration Test File | Primary Subsystem | Coverage Highlights |
|:---|:---|:---|
| `model_context_and_repository_intelligence.rs` | L3 Intelligence | XML trust envelopes, prompt compilers, repository AST queries. |
| `model_decision_surfaces_live_nvidia.rs` | L3 Models | Real model inference, consequential decision surfaces, tool call proposals. |
| `repository_intelligence_discovery.rs` | L3 Repository | Repository intelligence, multi-layer symbol extraction, AST parsing. |
| `context_engine_semantic_evidence_selection.rs` | L3 Context | Task-aware semantic context engine, evidence selection, relevance ranking. |
| `intent_decomposition_and_goal_analysis.rs` | L3 Intent | Intent understanding, goal decomposition, requirement parsing. |
| `prompt_builtin_catalog_and_schemas.rs` | L3 PromptOS | Embedded builtin prompt catalog, schema validation, deduplication. |
| `prompt_compiler_composition_engine.rs` | L3 PromptOS | PromptCompiler 7-layer composition engine, injection escaping. |
| `prompt_context_compiler_cutover.rs` | L3 PromptOS | ContextCompiler cutover, role decoupling, untrusted intent wrapping. |
| `prompt_verification_and_diagnostics.rs` | L3/L7 PromptOS | Reviewer and diagnostician prompt contracts, evidence envelopes. |
| `prompt_strategy_model_adaptation.rs` | L3 PromptOS | ModelProfile, PromptStrategy, dynamic adaptation, context budgeting. |
| `prompt_provenance_and_telemetry.rs` | L3 PromptOS | Contract hash, effective prompt hash, context digest determinism. |
| `prompt_workspace_extensibility.rs` | L3 PromptOS | Two-tier storage precedence, workspace prompt overrides, skills integration. |

### 7. Capabilities, Tools, and Execution Pipelines
| Integration Test File | Primary Subsystem | Coverage Highlights |
|:---|:---|:---|
| `capability_registry_contracts.rs` | L2 Capabilities | 15 capability families, provider bindings, availability checks. |
| `tool_execution_pipeline.rs` | L6 Execution | Tool execution pipeline, streaming output interception. |
| `tool_catalog_schemas.rs` | L2 Tool Catalog | Parameter validation for all 28 core tools against JSON schemas. |
| `provider_extension_seams_and_polymorphism.rs` | L2 Providers | Open extension seams, real provider polymorphism, zero fabrication. |
| `adaptive_execution_and_context_pruning.rs` | L2/L6 Execution | Adaptive tool execution, context pruning, dynamic priority adjustment. |
| `governed_execution_and_tool_continuation.rs` | L6 Execution | Governed model planning chain, tool continuation loop. |
| `code_modification_authority_benchmarks.rs` | L2/L6 Code Authority | Code modification authority, surgical editing, rollback on failure. |
| `git_worktree_and_commit_trailers.rs` | L2/L6 Git Operations | Worktree management, staging, diffs, RFC-compliant commit trailers. |
| `skill_manifests_and_execution_profiles.rs` | L2/L4 Skills & Profiles | Declarative `skill.md`, multi-tier discovery, 7 canonical profiles. |

### 8. Policy Decisions, Approvals, and Authorization
| Integration Test File | Primary Subsystem | Coverage Highlights |
|:---|:---|:---|
| `interactive_approval_grants.rs` | L1 Approval System | `ApprovalCoordinator`, durable SQLite policy grants, session resolution. |
| `policy_precedence_invariants.rs` | L1 Policy Precedence | 10-tier layer precedence, non-weakening decision merger. |
| `declarative_authority_boundary.rs` | L1 Authority Boundary | Declarative authority boundary, model proposal validation, runtime authority. |
| `runtime_authority_convergence_invariants.rs` | L1 Authority | Single runtime authority convergence across capabilities, policies, and checkpoints. |
| `adversarial_mutation_and_safety_authority.rs` | L1 Safety Authority | Adversarial code mutation defenses, TOCTOU patch protection, checksum mismatch. |

### 9. Sandbox Isolation and Process Supervision
| Integration Test File | Primary Subsystem | Coverage Highlights |
|:---|:---|:---|
| `process_job_supervision.rs` | L6 Job Supervisor | Child process group supervision, signal escalation, output spooling. |
| `process_job_recovery.rs` | L6/L7 Job Recovery | Abandoned process detection, spool sealing with `[INTERRUPTED]`. |
| `sandbox_isolation_and_confinement.rs`| L1/L2 Sandbox | Path traversal blocking, POSIX rlimits, workspace sandboxing. |
| `resource_confinement_and_budget_retirement.rs` | L2/L8 Resource Bounds | Platform resource bounds, process confinement rlimits, budget system decommissioning. |
| `isolation_boundary_hardening.rs` | L1/Git Authority | Worktree isolation defaults, fail-closed on missing git or worktree failure. |

### 10. Checkpoints, Crash Recovery, and Verification
| Integration Test File | Primary Subsystem | Coverage Highlights |
|:---|:---|:---|
| `checkpoint_crash_recovery.rs` | L7 Checkpoints | Two-phase checkpoint commits, startup crash recovery scanner. |
| `failure_recovery_and_loop_detection.rs` | L7 Fault Recovery | 15 failure classifications, loop detection, differential replanning. |
| `recovery_circuit_breaker_and_loop_prevention.rs` | L7 Recovery | Recovery circuit breaker, anti-loop detection, non-progress cycle termination. |
| `verification_hierarchy.rs` | L7 Verification | 7-tier verification hierarchy, semantic reviews, evidence linking. |
| `closed_loop_recovery_and_diagnosis.rs` | L7 Recovery Loop | Closed-loop engineering cycle (Verify → Diagnose → Repair → Replan). |
| `discovery_unknowns_and_uncertainty_resolution.rs` | L7 Discovery | Discovery of unknowns, uncertainty classification, dynamic research tasks. |
| `completion_reports_and_evidence_sealing.rs` | L7/L8 Reporting | 16-field `CompletionReport`, dual Markdown/JSON projection, evidence locators. |
| `autonomous_coding_reliability_remediation.rs` | L7 Verification | Autonomous coding reliability, verification strategies, review gate enforcement. |
| `production_reliability_and_failure_matrix.rs` | L7 Reliability | Production reliability failure matrix, honest cancellation, multi-turn evidence. |

### 11. CLI, TUI, and Configuration Authority
| Integration Test File | Primary Subsystem | Coverage Highlights |
|:---|:---|:---|
| `cli_subcommands_and_runtime_dispatch.rs` | L9 CLI Interface | CLI subcommand parsing, exit codes, and JSON output formatting. |
| `configuration_precedence_and_profiles.rs` | L9 Configuration | 7-tier configuration precedence, platform-aware paths, profiles. |
| `tui_cockpit_navigation_and_replay.rs` | L9 TUI Cockpit | Ratatui terminal cockpit, telemetry rendering, interactive screens. |
| `tui_cockpit_view_registry_and_router.rs` | L9 TUI Cockpit | 40 canonical cockpit views, view registry, metadata completeness, router. |
| `tui_framebuffer_multi_resolution_snapshots.rs` | L9 TUI Layout | Multi-resolution framebuffer snapshots, layout stability, screen resizing. |
| `onboarding_lifecycle_and_first_run.rs` | L9 Onboarding | First-run experience (FRX), initialization state machine, onboarding config. |
| `pty_lifecycle_and_terminal_safety.rs` | L9 PTY Safety | Headless Unix PTY lifecycle, resize, ANSI sequences, signal handling. |
| `terminal_guard_and_design_tokens.rs` | L9 Terminal Guard | TerminalGuard RAII safety, emergency raw-mode restoration, design tokens. |
| `tui_governed_lifecycle_convergence.rs` | L9 TUI Governance | TUI conversation projection, governed lifecycle convergence. |
| `system_wiring_reachability_and_tui_convergence.rs` | L9 System Wiring | Full-system wiring reachability, projection hydration, TUI convergence. |
| `tui_truth_projection_regression.rs` | L9 TUI Projection | TUI truth projection regression, Git branch display, task progress fidelity. |
| `workspace_detection_and_workflow_tier.rs` | L9 Workspace | Workspace environment probe, greenfield/brownfield detection, workflow tiers. |
| `runtime_truth_reconciliation_and_handoff.rs` | L9 Runtime Truth | Runtime truth reconciliation, user experience journey, handoff verification. |
| `runtime_truth_boundary_deterministic.rs` | L9 Truth Boundary | Production truth boundary, deterministic execution integrity, zero fabrication. |

### 12. Security, Release Engineering, Deployment, and Documentation Contracts
| Integration Test File | Primary Subsystem | Coverage Highlights |
|:---|:---|:---|
| `security_hardening_threat_matrix.rs` | L1 Security Hardening | Comprehensive 11-threat matrix verification suite (ASVS L1). |
| `security_boundary_and_durable_recovery_closure.rs` | L1 Security Boundary | Security boundary adversarial closure, durable recovery closure. |
| `telemetry_correlation_and_secret_redaction.rs` | L0/L9 Telemetry | W3C correlation context, multi-tier secret redaction, CLI inspect. |
| `architectural_layer_dependency_guards.rs` | L0-L9 Architecture | Architectural dependency layer guards (L0–L9), upward dependency detection. |
| `visual_review_and_dual_gate_verification.rs` | L9 Visual Review | Visual review dual-gate verification, checklists completeness. |
| `evidence_plane_and_runtime_observability.rs` | L7/L8 Observability | Evidence plane, artifact explorer, live runtime observability. |
| `runtime_integrity_and_accounting_convergence.rs` | L0/L7 Integrity | Runtime integrity, authorization atomicity, accounting convergence. |
| `release_candidate_validation_suite.rs` | L9 Release Engineering | Release candidate validation, packaging manifests, install/upgrade, provenance. |
| `evaluation_harness_and_acceptance_scenarios.rs` | L9 Evaluation Harness | Golden acceptance scenarios A–H and scorecard generation in temp fixtures. |
| `docs_contract.rs` | L9 Documentation | Canonical documentation presence, CLI command, tool, and profile alignment. |

---

## Adversarial Security Hardening Suite (`security_hardening_threat_matrix.rs`)

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
11. **Resource exhaustion clamping**: Streaming byte limits and platform resource confinement strictly enforced.

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

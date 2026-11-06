# M31A — Speculative Execution & Workflow Architecture Audit

**Milestone:** Foundation Milestone: Speculative Execution & Autonomous Workflow Orchestration  
**Target:** 39-Crate Rust Workspace (`M31A`)  
**Status:** Audit Complete & Baseline Validated  
**Date:** 2026-11-06  

---

## 1. Executive Summary & Baseline Validation

An exhaustive baseline audit of the M31A repository was conducted across all 39 crates. The workspace currently passes compilation (`cargo check --workspace --all-targets`) and all existing tests (`cargo test --workspace`) across all packages without regressions.

The audit examined the core execution, storage, tool, verification, and interface crates:
- `m31a-agent-engine`: `turn_loop.rs`, `agents.rs`, `speculative.rs`, `waves.rs`, `vibe_loop.rs`.
- `m31a-tools`: `agent_runtime.rs`, `registry.rs`, `patch.rs`, `harness.rs`.
- `m31a-storage`: `db.rs`, `migrations.rs` (14 versioned schema migrations), `queue_store.rs`, `worktree.rs`.
- `m31a-verify`: `runners.rs` (`AutonomousVerifier`), `safety.rs` (`CodebaseSafetyGate`), `benchmark.rs` (`PerformanceProfiler`), `plan_checker.rs`.
- `m31a-core`: `types.rs`, `context_compactor.rs`, `prompts.rs`, `approval.rs`.
- `m31a-cli`: `main.rs`, `speculate.rs`, `gsd.rs`.
- `m31a-tui`: `screens.rs`, `app.rs`, `bottom_pane.rs`.
- `m31a-app-server`: `protocol.rs`, `relay.rs`, `daemon_lifecycle.rs`.

---

## 2. Existing Implementations & Runtime Call Sites

### 2.1 Agent Execution Subsystem (`m31a-agent-engine`)
1. **`turn_loop::AgentEngine`**:
   - Call sites: Main session loop in CLI (`main.rs`), App Server (`protocol.rs`), GSD executor (`gsd.rs`), child engine spawning in `agents.rs`.
   - Core API: `AgentEngine::run_turn(session_id, history, event_tx, steer_rx, cancel_token, max_turns)`.
   - Strengths: In-flight steering via `mpsc::UnboundedReceiver<String>`, tool execution via `AuthoritativeToolDispatcher`, context compaction integration, Guardian supervision gating (`ApproveForMe`).
   - Gaps: No direct integration with `SpeculativeArena` candidate implementation loops. Candidates are not run through `AgentEngine`.

2. **`agents::EngineAgentRuntime`**:
   - Implements `m31a_tools::agent_runtime::AgentRuntime`.
   - Call sites: Tool registry initialization in CLI and App Server; invoked when `spawn_agent`, `wait_agent`, or `close_agent` tools are dispatched.
   - Core API: `spawn_agent`, `wait_agent`, `close_agent`, `send_input`, `list_agents`.
   - Strengths: Spawns real child `AgentEngine` instances on dedicated tokio tasks; reserves spawn slots (`MAX_CONCURRENT_THREADS = 8`); enforces spawn depth (`MAX_SPAWN_DEPTH = 3`); isolates workspaces via `WorktreeManager::create_worktree`; records edges in SQLite `thread_spawn_edges`; forwards parent-mediated approvals via `child_approval_tx` and `approval_mediator`.
   - Gaps: Used solely as a tool invocation target from LLM tool calls. Not directly used as the execution worker for workflow task execution waves or speculative candidate branches.

3. **`speculative::SpeculativeArena`**:
   - Call site: CLI `m31a speculate` (`crates/m31a-cli/src/speculate.rs`).
   - Core API: `SpeculativeArena::generate_hypotheses(mission, approaches)` and `SpeculativeArena::compete(workspace_root, mission, candidates, timeout, auto_promote)`.
   - **Critical Problem Identified:** `generate_hypotheses` constructs `SpeculativeBranchConfig` instances with `patch_content: None`. `evaluate_candidate` allows empty patches and simply runs `AutonomousVerifier::verify_check` on the isolated sandbox (which is an identical copy or worktree of the baseline). An unchanged baseline workspace passes `cargo check` and `cargo test`, receives 40 + 35 = 75 points + speed bonus, and is declared the winning implementation without a single line of code written or tested!
   - Furthermore, `compete` does not invoke an agent, model, or tool loop to generate code changes.
   - In addition, if `auto_promote` is true, the winning patch is applied directly to the active workspace without conflict verification, rollback information, or human approval gating.

4. **`waves::WaveScheduler`**:
   - Call sites: GSD phase execution (`crates/m31a-cli/src/gsd.rs`).
   - Core API: `WaveScheduler::discover_plans(phase_dir)`, `partition_into_waves(plans)`, `execute_waves(repo_root, phase, waves, executor_fn, verify_precommit)`.
   - Strengths: Topological DAG sorting from `depends_on`; file-overlap conflict partitioning (tasks modifying the same file are split into consecutive waves); bounded parallel execution using `tokio::task::JoinSet`; post-wave precommit check via `AutonomousVerifier::verify_check`.
   - Gaps: Coupled specifically to markdown files (`*-PLAN.md` with `PlanModel`). Not connected to a durable database-backed workflow model, persistent task queues, or dynamic child-agent task assignment. Task outcomes only record `commits` and `summary_path`, not structured verification evidence.

---

## 3. Storage & Schema Baseline (`m31a-storage`)

The SQLite database (`Storage`) has 14 migrations registered in `migrations.rs`:
1. `initial_core_schema`: `threads`, `thread_turns`, `thread_items`.
2. `goals_and_checkpoints`: `thread_goals`, `checkpoints`.
3. `agent_graph_edges`: `thread_spawn_edges` (`parent_thread_id`, `child_thread_id`, `agent_role`, `status`, `created_at`).
4. `collaboration_message_board`: `channels`, `subscriptions`, `posts`.
5. `thread_sections`: `thread_sections`.
6. `notes_and_history_recovery`: `rollout_notes`, `history_windows`, `history_items`.
7. `audit_logs_and_task_queue`: `audit_logs`, `task_queue` (`id`, `thread_id`, `priority`, `task_kind`, `payload_json`, `status`, `retries`, `max_retries`, `error_message`, `created_at`, `updated_at`).
8. `retained_facts`: `retained_facts`.
9. `revert_cas_lineage`: `threads.forked_from_ordinal_exclusive`.
10. `spawn_edge_depth`: `thread_spawn_edges.depth`.
11. `execution_claims_and_inbox`: `thread_execution_claims`, `session_inbox`.
12. `path_locks_schema`: `path_locks`.
13. `evidence_records_and_session_memories`: `evidence_records`, `session_memories`.
14. `thread_ownership_and_session_todos`: `thread_ownership`, `session_todos`.

### Storage Gaps for Autonomous Workflows
- **Missing Workflow Entities**: There are no tables for `workflows`, `workflow_plans`, `workflow_tasks`, `workflow_task_dependencies`, `workflow_events`, or `workflow_artifacts`.
- **Queue Lease Deficiencies**: `task_queue` has `status IN ('pending', 'processing', 'completed', 'failed', 'dead_letter')`, but lacks worker lease ownership (`worker_id`, `lease_expires_at_epoch_ms`, `idempotency_key`). Interrupted tasks cannot be safely reclaimed after a crash without corrupting state.
- **Artifact Tracking**: `evidence_records` exists but is unstructured and lacks content hashing (SHA-256), size validation, atomic file persistence, and workflow correlation.

---

## 4. Verification & Safety Baseline (`m31a-verify`)

1. **`AutonomousVerifier` (`runners.rs`)**:
   - Detects Rust, Bazel, TypeScript/Node, Python, Go, CMake, Gradle.
   - `verify_check`: Fast compile/linter check (`cargo check --message-format=json`).
   - `verify_tests`: Full test suite run (`cargo test`).
   - Can run with sandbox isolation via `run_check_sandboxed`.
2. **`CodebaseSafetyGate` (`safety.rs`)**:
   - Builds AST index via `AstCodebaseIndex`.
   - Computes semantic blast radius via `SemanticImpactAnalyzer`.
   - Audits data-flow invariants and source-to-sink taints via `DataFlowAnalyzer`.
   - Returns `SafetyAuditReport` with `SafetyGateVerdict` (`Passed`, `Warning`, `Blocked`).
3. **`PerformanceProfiler` (`benchmark.rs`)**:
   - Detects benchmark suites (`cargo bench --no-run`, pytest benchmarks, go benchmarks).
   - Compares baseline vs. candidate latency deltas and flags regressions exceeding thresholds.

### Verification Gaps
- `SpeculativeArena` did not enforce that candidate workspaces actually had modified files or git diffs before running `verify_check`.
- `SpeculativeArena` did not use `PerformanceProfiler` for candidate comparison, instead granting a "fast execution bonus" based merely on wall-clock compilation time!

---

## 5. Interface Baseline

1. **CLI (`m31a-cli`)**:
   - Subcommands: `setup`, `model`, `exec`, `undo`, `review`, `verify`, `speculate`, `vibe`, `gsd`.
   - Needs: A first-class `workflow` command group:
     `m31a workflow run <objective>`
     `m31a workflow status <workflow-id>`
     `m31a workflow inspect <workflow-id>`
     `m31a workflow resume <workflow-id>`
     `m31a workflow cancel <workflow-id>`
     `m31a workflow events <workflow-id>`
     `m31a workflow artifacts <workflow-id>`
   - `m31a speculate` must be upgraded to run genuine agent candidate execution with real patch generation.

2. **TUI (`m31a-tui`)**:
   - Screen 5 (`Screen::Agents`) already displays subagent hierarchy from `list_spawn_descendants` and active tabs.
   - Can be enhanced to display:
     - Workflow overview (stage, progress, elapsed time).
     - Task DAG and dependency wave state.
     - Speculative candidate comparison matrix.
     - Live verification status and diagnostics.
     - Interactive controls (inspect task, cancel, resume, approve integration).

3. **App Server (`m31a-app-server`)**:
   - Protocol handles JSON-RPC 2.0 requests over stdin/stdout, Unix sockets, and TCP.
   - Active turns, pending approvals, pending questions, multiplexer, daemon lifecycle coordinator.
   - Needs: RPC endpoints for `workflow/create`, `workflow/status`, `workflow/inspect`, `workflow/resume`, `workflow/cancel`, `workflow/events`, `workflow/artifacts`, and speculative candidate retrieval.

---

## 6. Architecture & Implementation Plan

### Stage 1: Speculative Execution Engine Overhaul
- Update `crates/m31a-agent-engine/src/speculative.rs`:
  - Define `SpeculativeCandidateHypothesis` with approach name, prompt, model selection, hypothesis.
  - Implement genuine candidate generation: Given an objective, query the provider/agent to formulate distinct implementation approaches.
  - Candidate Execution via real `AgentEngine`: In an isolated Git worktree (or temp workspace), run an agent turn loop with a task objective, capturing changed files, patch/commit diff, and compilation diagnostics.
  - Reject empty implementations (`NoImplementation`).
  - Strict evidence-based evaluation:
    - Stage 1: Workspace modification check (asserts non-empty patch and contained paths).
    - Stage 2: Fast compiler check (`AutonomousVerifier::verify_check`).
    - Stage 3: Test suite execution (`AutonomousVerifier::verify_tests`).
    - Stage 4: Codebase safety gate (`CodebaseSafetyGate::audit_workspace_changes`).
    - Stage 5: Performance profiling (`PerformanceProfiler` where benchmarks exist).
    - Stage 6: Deterministic composite scoring with hard gates (zero score if build/tests fail or safety blocked).
  - Explicit candidate promotion: Conflict detection, pre-integration check, approval requirement, post-integration verification.

### Stage 2: Workflow Domain Models & Persistent Storage
- Migration 15 in `crates/m31a-storage/src/migrations.rs`:
  - `workflows` table.
  - `workflow_plans` table.
  - `workflow_tasks` table.
  - `workflow_task_dependencies` table.
  - `workflow_events` table (append-only event log).
  - `workflow_artifacts` table (SHA-256 integrity, size, mime/kind).
- Module `crates/m31a-storage/src/workflow_store.rs`:
  - Typed operations for workflow creation, status transition, task enqueueing, atomic task claim with worker lease, completion, failure, idempotency check, event logging, and artifact persistence.

### Stage 3: Workflow Orchestration Engine
- Module `crates/m31a-agent-engine/src/workflow.rs`:
  - `WorkflowRun`, `WorkflowPlan`, `WorkflowTask`, `WorkflowEvent`, `WorkflowArtifact`.
  - DAG validator: cycle detection, missing dependency check, conflict detection on write paths.
  - Orchestrator connecting planner to `WaveScheduler` and `EngineAgentRuntime`:
    - Generates or receives plan.
    - Resolves DAG into waves.
    - For each wave, spawns real child agents in isolated worktrees.
    - Executes tasks concurrently.
    - Gathers structured outcomes and verification evidence.
    - Performs post-wave and final verification.
    - Supports speculative stage if chosen by plan.
    - Manages recovery, crash resume, and bounded replanning on failure.

### Stage 4: Interfaces (CLI, TUI, App Server)
- CLI: Implement `Commands::Workflow` with `run`, `status`, `inspect`, `resume`, `cancel`, `events`, `artifacts`.
- TUI: Integrate workflow monitoring, DAG visualization, and candidate inspection into `Screen::Agents` and dedicated subviews.
- App Server: Expose versioned JSON-RPC methods and notification streaming for workflows.

### Stage 5: End-to-End Verification & Documentation
- Comprehensive tests for speculative execution, DAG validation, workflow persistence, lease expiry, agent execution, crash recovery, and CLI/RPC.
- Full workspace tests, documentation, and implementation reports.

# Changelog

All notable changes to M31A (M31 Autonomous) are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

---

## [0.1.6] — 2026-10-09

### Added

**LSP & Code Intelligence**
- Implemented `LocalLspProcessClient` for stdio JSON-RPC queries with robust message handling and diagnostics.
- Persistent LSP session management with lifecycle tracking, active process management, and cost accounting.
- Code navigation and inspection tools: `LspGotoDefinitionTool`, `LspFindReferencesTool`, `LspHoverTool`, and `LspSymbolsTool`.
- Language server registration and query dispatch integrated into the canonical tool registry and repository intelligence layer.

**Workspace Patching & File Operations**
- Implemented `WorkspacePatchEngine` for parsing and atomic application of multi-file unified git diffs.
- Introduced typed patch actions (`FilePatchAction`, `PatchHunk`, `PatchDiagnostic`) and pre-flight patch applicability testing (`test_applicable`).
- Added workspace patch tool for multi-file code modifications with rollback support.

**Terminal Sessions & Interactive Capabilities**
- Added persistent terminal session management (`TerminalSessionManager`) and tools (`TerminalSpawnTool`, `TerminalWriteTool`, `TerminalReadTool`, `TerminalCloseTool`, `TerminalListTool`).
- Capability provider support for isolated terminal process execution, stream buffering, and session cleanup.

**Structured Diagnostics & Verification**
- Added structured compiler diagnostic subsystem (`src/verification/diagnostics.rs`) integrated with compiler error and warning output parsing.
- Enhanced verification gates with structured diagnostic collection and validation.

**Agent Action Protocol & Delegation**
- Added typed agent action protocols and delegation module (`src/agent/action.rs`, `src/agent/delegation.rs`) for runtime action execution.
- Added interactive messaging support to `ContextCompilationRequest`.
- Added GitHub PR tools (`PrCreateTool`, `PrStatusTool`, `PrCommentTool`, `PrListChecksTool`) and Web tools (`WebSearchTool`, `WebFetchTool`).

**Model Pricing, Cost Tracking & Evaluation**
- Database migration `028_model_invocation_cost.sql` adding invocation cost tracking to the model persistence schema.
- Token pricing engine (`src/model/pricing.rs`) computing exact prompt and completion costs.
- Autonomous evaluation enhancements with timeout handling, scorecard reporting, and cost provenance.

**TUI Architecture & Performance**
- Modularized TUI architecture into decoupled route (`routes.rs`), state (`state.rs`), and application layers (`TuiApplication`).
- Renderer-first startup optimization ensuring immediate first-frame visibility and responsiveness.
- Asynchronous model discovery and configurable target frame rates (FPS).

**Tooling & Process Management**
- Root `Makefile` for unified build, lint, format, test, and release verification workflows.
- Job manager liveness checks and cancellation of all active background jobs.

### Fixed
- Fixed hunk positioning search logic and patch application accuracy in `WorkspacePatchEngine`.
- Defaulted mission and task identifiers in `AgentEngine` when unspecified.
- Refined action validation and execution handling for `write_file` and verification runner commands.
- Simplified cost and token usage source calculation in autonomous evaluation runners.

### Changed
- Version bumped to 0.1.6.

---

## [0.1.5] — 2026-10-07

### Fixed
- Fixed indefinite test hangs across the architecture invariant test suite caused by unisolated test workspaces competing for the global host SQLite database and performing heavy recovery scans across parallel test threads.
- Enhanced `StorageLayout::uses_isolated_global()` to reliably detect canonicalized temp roots, `target/tmp`, and `CARGO_TARGET_TMPDIR` directories.
- Ensured background tasks (such as the telemetry forwarder) spawned by `AppRuntime` are bounded, tied to `CancellationToken`, and joined cleanly during `AppRuntime::shutdown()`.
- Introduced `TestRuntimeGuard` and deterministic time budget harness (`src/testing/runtime_harness.rs`) for bounded and reliable asynchronous test lifecycles.
- Fixed release validation path assertions in `tests/phase_45_release_validation.rs` to target canonical workspace database paths.
- Unified the production runtime authority graph: one `RuntimeAuthorities` bundle per scope; the worker dispatcher, controller, and worktree execution now consume the canonical `ToolRegistry`/`ToolPipelineRunner`/authorization trust root instead of reconstructing competing authorities. Worktree missions inherit policy, budget, approval, artifact, event-bus, model, prompt, memory, job-supervisor, and sandbox authorities; the model catalog persists to and reads from the single global platform cache; cross-workspace session resume fails closed pending a runtime rebind.

### Changed
- Version bumped to 0.1.5.

---

## [0.1.4] — 2026-10-06

### Added

**Cross-Platform Sandboxing & Process Management**
- macOS Seatbelt (`SeatbeltSandboxProvider`) and Windows Job Object (`JobObjectSandboxProvider`) sandbox providers integrated into `LocalSandboxProvider`.
- Platform-neutral process identity management (`ProcessIdentity`) and identity verification.
- Windows process tree management and executable discovery with `.exe`, `.cmd`, and `.bat` resolution.
- Native cross-platform verification script (`scripts/verify-platform.sh`) and Windows CI test enhancements.
- Cross-platform qualification and capability parity matrix documentation in `docs/PLATFORM-SUPPORT.md`.

**Durable Job & Mission Lifecycle**
- Mission pause and resume functionality (`PauseMission`, `ResumeMission`) with durable state updates.
- `JobSupervisor` integration with durable job ledger and lifecycle tracking.
- Budget ledger estimated consumption tracking (`estimated_tokens`, `estimated_cost_usd`) for restart durability.

**Scoped Git Gates & Policy Enforcement**
- Runtime authorization for mutating Git operations with scoped `GitGate` enforcement and task identity checks.
- Additional path restrictions in built-in safety rules and developer defaults.
- Workspace root propagation for policy pre-flight evaluation and agent identity binding in `PolicyEvaluationRequest`.

**TUI & Interaction Enhancements**
- Interactive commands for health diagnostics and version display; application actions for provider switching and mission control.
- Session provider update functionality with configuration validation.
- Workflow snapshot handling in TUI and comprehensive TUI wiring integration test suite.

### Fixed
- Fixed Greenfield workspace detection in `WorkspaceEnvironment::probe` when running in subdirectories of a parent repository.
- Fixed unused `PlatformProbe` import and unnecessary returns on non-Linux platforms in `LocalSandboxProvider`.
- Added missing `Debug` derive implementation for native `JobHandle` on Windows.

### Changed
- Version bumped to 0.1.4.

---

## [0.1.3] — 2026-10-04

### Changed

**Conversation-first TUI visual and UX reconstruction (presentation only)**
- New restrained `M31A Default` theme with semantic design tokens; legacy
  `DarkSlateCyan` restyled to the same calm philosophy (no neon, no bold
  shouting, status carried by symbol + text, never color alone).
- Quiet application header (`M31A · model · branch · state`); token, cost,
  and event telemetry moved to contextual details surfaces.
- Conversation stream rebuilt around subtle role labels (`You`, `M31A`,
  `Tool`, `Verification`, …) with open, borderless layout.
- Dashboard no longer splits on historical task/agent/approval records;
  active-work layout is driven by semantic runtime state
  (`TuiViewModel::is_semantically_active`).
- Responsive tiers kept; Standard/Large/UltraWide no longer force permanent
  sidebar/telemetry panes; reading width constrained on ultrawide terminals.
- Professional approval modal, clean command palette, quiet help overlay
  and composer prompt. All runtime, governance, slash-command,
  autocomplete, and safety behavior preserved.
- Version/channel identity removed from the persistent TUI header (now
  `M31A · model · branch · state` only); full identity remains available
  via `m31a --version`, `m31a deployment`, and
  `DeploymentContext::cockpit_label()`. Token, cost, and event telemetry
  likewise moved out of the header into contextual details surfaces.

---

## [0.1.2] — 2026-10-04

### Added

**Complete runtime wiring remediation (single authoritative dependency graph)**
- New `src/runtime_authorities.rs`: canonical `RuntimeAuthorities` set
  (config, policy, capabilities, tools, budget, approval, model catalog,
  provider, caller, context compiler, prompt catalog, artifacts, event bus,
  git, workspace/storage/channel), `RuntimeBinding`
  (session+mission+task+agent+role+autonomy), `ModelInvocationKind` typed
  invocation contexts, `AutonomyPrecedence` (task > session > runtime
  default), and the single channel-aware credential resolver.
- `AgentEngine` binds the runtime-shared capability registry, typed role
  envelope, mission/task/agent identity, and explicit autonomy (fail-closed
  `Safe` default); durable (mission, task) scope is ensured before side
  effects; the pipeline owns the single policy evaluation and real
  coordinator approvals per action; subagent delegation rebinds typed role
  authority.
- Explicit tool-free model entrypoints (`call_model_tool_free_*`) for
  planning/discovery/review/verification; prompt-text role sniffing removed
  from the production caller.
- `ControllerDependencies` fields made private with accessors and total
  `with_*` builders; `ProductionWorkerDispatcher::from_shared_authorities`
  constructs no competing authorities.

**Fail-closed execution & configuration semantics**
- `RunMission` and mission mutations without a runtime/repository are typed
  errors — never fabricated `status: "started"` success.
- Present-but-invalid workspace configuration is a hard startup error;
  only intentional absence uses documented defaults (`build_with_report`).
- Approval resolution failures propagate; mission terminal-status and
  workflow mission persistence failures propagate.

**Channel isolation & runtime truth**
- Channel-aware artifacts/telemetry/staging/credentials/model-catalog
  cache/spools paths (`DeploymentPaths`); development never reads
  production credentials or state.
- Banner, slash-command, `/status`, TUI, and model-display paths read the
  active runtime configuration instead of ambient environment probes.
- CLI/TUI `/tools` inventory inspects the attached runtime registry.

**Packaging & release**
- Multi-format release artifacts: Linux `.tar.gz`, `.deb`, `.rpm`,
  `.AppImage`; macOS `.tar.gz`, `.dmg`; Windows `.zip`, `.msi`
  (see `scripts/build-release.sh --help` and `.github/workflows/release.yml`).
- Standalone installers (`scripts/install.sh`, `scripts/install.ps1`)
  aligned with versioned release artifact names.

### Changed
- Version bumped to 0.1.2. All behavior changes above are covered by
  `tests/architecture_runtime_authority.rs` (12 wiring invariants + static
  source scans) and the full workspace suite (2556 passed, 0 failed).

---

## [0.1.1] — 2026-10-02

### Added

**Deployment & Release Channels (DEVELOPMENT vs PRODUCTION)**
- One core runtime, two isolated deployment channels: production (`m31a`,
  default build) and development (`m31a-dev`, `--features development`).
  Channel is compile-time artifact identity — no runtime environment switch
  can re-channel a binary.
- New `src/deployment/` subsystem: `DeploymentChannel`/`UpdateChannel`,
  immutable `DeploymentContext` (version, build ID, commit, branch, timestamp,
  target, dirty, artifact ID), `ReleaseArtifact` model, versioned
  `DeploymentManifest` (schema v1, shared by both channels), transactional
  `Installer` (stage → verify → atomic replace, previous binary preserved),
  channel-safe update discovery, `rollback` seam, centralized `FeatureGate`
  for development-only affordances, and `DeploymentPaths` isolation.
- CLI: `m31a version [--verbose]`, `m31a deployment [--verbose]`,
  `m31a update --manifest <file> [--check]`, `m31a rollback`;
  channel-aware `m31a --version` (`m31a X.Y.Z` vs `m31a-dev X.Y.Z-dev+<build>`);
  `m31a doctor` includes a deployment probe; `m31a config sources` reports
  channel and config source.
- Cockpit header shows an unobtrusive `vX.Y.Z PRODUCTION` /
  `vX.Y.Z DEVELOPMENT · build <short>` label at all terminal widths.
- Release tooling: `scripts/build-release.sh --channel`, channel-aware
  packaging/identity validation/deployment manifest, dirty-tree refusal for
  production; new `scripts/install-local.sh` for user-local installs.
- CI: new development pipeline (`.github/workflows/development.yml`;
  nightly/branch builds, 7-day artifact retention, never publishes releases);
  production pipeline gated on clean source, version==tag, and identity checks.

### Changed

- `PlatformPaths` and persistence paths are channel-aware: development uses
  isolated `m31a-dev` global state; production paths are unchanged
  (backward compatible). Project-local `.m31a/` stays shared with
  deployment-scoped runtime state (`m31a.db` vs `m31a-dev.db`,
  `.m31a/state/<channel>/`, per-channel sockets/PIDs/credentials).
- Production promotion reuses the existing release-candidate state machine
  with explicit dirty/blocker/approval gates (`deployment::evaluate_promotion`).

### Security

- Both channels enforce identical policy, sandbox, capability, approval,
  containment, and secret controls. Development diagnostics can never bypass
  security gates. Production rejects development artifacts on the normal
  update path; updates verify SHA-256 before replacing any binary.
- **Fail-Closed Worktree Isolation**: Changed `default_execution_isolation()` to
  `"required"`. Governed production runs fail closed if worktree creation cannot
  be verified, preventing unisolated primary-workspace modifications unless
  explicitly configured for `best_effort` fallback.
- **Egress & SSRF Hardening**: Centralized `NetworkDestinationPolicy` blocking
  IPv4/IPv6 loopback, RFC 1918 private subnets, cloud metadata (`169.254.169.254`),
  link-local, and carrier NAT. Enforces asynchronous DNS pre-validation and step-by-step
  redirect verification up to 5 hops, preventing redirect SSRF and DNS rebinding.
- **XML TrustEnvelope Hardening**: Added strict attribute escaping (`&`, `<`, `>`,
  `"`, `'`, control chars, newlines) preventing XML injection and tag breakouts in prompt envelopes.
- **Expanded Secret Redactor**: Added scrubbing for NVIDIA API keys, GitLab tokens,
  database URLs with passwords, basic/digest auth headers, and environment credential pairs.
  Added `sanitize_error` preventing secret leakage in diagnostic traces.
- **Structural Telemetry & Logging**: Runner step events, SSE parser logs, and
  pipeline diagnostics emit structural metadata only (proposal kinds, tool counts,
  identifiers, durations, error categories, error codes, and audit digests), strictly
  preventing raw model proposals or arbitrary command output from entering logs.
- **Stage 10 Output Security Contract**: Formalized `PipelineOutputEvidence`
  disambiguating raw execution output, model-visible projections, redacted diagnostic
  evidence, and SHA-256 cryptographic audit digests.
- **Process Environment & Shell Security**: Child processes strictly clear host
  environment variables (`env_clear()`) installing only trusted baselines. Shell argument
  inspection covers both POSIX and Windows (`/c`, `/C`, `/k`, `-Command`, `--command`) invocations.

---

## [0.1.0] — 2026-09-25

### Added

**Runtime Kernel (L0–L2)**
- Single-crate Rust-native autonomous runtime with zero foreign runtime dependencies
- Domain-typed kernel IDs (`MissionId`, `TaskId`, `SessionId`, `AgentId`, `CheckpointId`, `ArtifactId`)
- Bounded retry policy (no infinite retry loops)
- Broadcast event bus with typed `EventEnvelope` and filter-based subscriptions
- Immutable content-addressed artifact store with SHA-256 integrity verification
- SQLite-backed durable persistence with 19 incremental migration files
- Two-phase atomic checkpoints with `CheckpointIntegrityValidator` (SHA-256 manifest verification)
- Startup crash scanner and automatic recovery classification
- `StreamingQuotaWriter` artifact quota enforcement (per-artifact and cumulative)

**Security / Policy (L1)**
- 11-stage policy gate with `ALLOW` / `DENY` / `ASK` / `ESCALATE` decision matrix
- 9-layer precedence model for policy resolution
- `SecretRedactor` with 4-tier deterministic scrubbing pipeline (API keys, JWTs, private keys, AWS credentials, generic patterns `ghp_*`, `sk-*`, `nvapi-*`)
- `TrustEnvelope::wrap_untrusted` with SHA-256 integrity digest for prompt injection defense
- `ApprovalCoordinator` fail-closed ASK semantics (non-interactive auto-deny)
- Path canonicalization with symlink escape prevention
- `PluginToolAdapter` policy subordination for all plugin tool dispatches
- Terminal escape sanitization (`sanitize_terminal_text`) before UI render
- `ProcessTreeController` with isolated process groups and `SIGTERM → SIGKILL` escalation
- Multi-tier process confinement: cgroups v2 (when available), POSIX rlimits, watchdog supervision
- ASVS L1 coverage for all 11 documented threat vectors

**Capabilities / Tools (L2)**
- 28 core tools with typed parameter schemas and execution risk classification
- `LocalFileSystemProvider` with workspace-root-enforced access control
- Direct `execve`-based subprocess spawning (no shell interpolation)
- `EnvironmentBuilder` stripping dangerous loader hooks and credential variables
- Process group isolation (`setpgid`) for clean cancellation

**Intelligence / Context (L3)**
- NVIDIA NIM provider integration with SSE streaming
- `SecretRedactor` applied before all model-boundary data persistence
- 7-layer prompt composition with MiniJinja template engine
- Token-counted context window management with tiktoken-rs
- Task-aware memory retrieval (Phase 25 engineering memory types)

**Agent Coordination (L4)**
- 8 canonical agent roles: `planner`, `researcher`, `architect`, `implementer`, `reviewer`, `verifier`, `diagnostician`, `integrator`
- Role-specific agent state machines with bounded step budget enforcement
- Anti-fake-diff review: detects `todo!()` / `unimplemented!()` patterns in changes
- Premature-completion rejection enforced before task state transitions

**Planning / DAG (L5)**
- Directed acyclic task graph with petgraph-backed dependency resolution
- Candidate plan generation with `PlanQualityMetrics`
- Differential DAG replanner on crash recovery
- `WorkflowEngine` with full genesis discovery/research/planning/synthesis pipeline

**Verification / Recovery (L7)**
- Multi-tier verification evidence requirement before mission completion
- 15 failure classification categories for structured recovery routing
- Closed-loop recovery with scored recovery strategies (Phase 24)
- `CheckpointIntegrityValidator`: `Healthy`, `Corrupt`, `Ambiguous`, `Missing` states

**Autonomy / Mission Controller (L8)**
- 5 autonomy modes with sliding-window loop detector
- 10-dimensional hard resource budget model
- Two-phase budget reservation per tool action
- Mission state machine: `Pending → Running → [Paused | Completing | Failed | Cancelled]`

**CLI / TUI (L9)**
- Full `clap`-derive CLI with machine-readable `--output json` and `--output stream-json` modes
- All subcommands: `session`, `mission`, `task`, `agent`, `capability`, `policy`, `checkpoint`, `artifact`, `doctor`, `telemetry`, `config`, `eval`, `tui`, `version`
- Standardized exit codes: 0 success, 1 verification failure, 2 policy violation, 3 budget exhaustion, 4 crash/infrastructure, 5 configuration error
- TUI cockpit with Ratatui 0.30: conversation timeline, mission overview, task DAG, agent swarm, git attribution, verification surface, tool execution view, approval modal, command palette, replay controller
- `TerminalGuard` RAII: guaranteed terminal raw-mode and alternate-screen restoration on all exit paths (normal, error, Ctrl+C, SIGTERM)
- First-run setup wizard with workspace onboarding
- Interactive session runner (`m31a session new`)

**Observability**
- SQLite compact event index + append-only NDJSON execution stream
- Structured telemetry with `TelemetryCollector`, correlation IDs, and secret redaction before persistence
- Doctor command with structured health-check probes and JSON output mode

**Repository Intelligence**
- Repository scanner with change authority tracking (Phase 23)
- Context engine with adaptive indexing
- Engineering memory with task-aware retrieval (Phase 25)

**Testing**
- Unit tests for all stateful subsystems
- Integration tests: golden workflow, configuration, interactive session, genesis pipeline, TUI scenarios
- Security hardening suite (`tests/phase_12_security_hardening.rs`): all 11 threat vectors
- Property-based tests for policy and budget enforcement
- Architecture contract tests
- SQLite migration regression tests
- Workflow persistence tests
- Prompt contract deterministic hash tests

### Security

- ASVS L1 compliance across all 11 threat vectors
- Zero secrets committed: `.env` and `.m31a/credentials.json` gitignored from initial commit
- `SecretRedactor` pattern coverage: `nvapi-*`, `ghp_*`, `sk-*`, `AKIA*`, Bearer JWTs, RSA private keys
- Fail-closed ASK semantics in non-interactive contexts
- All tool execution subordinate to policy gate (no bypass paths)

---

## [0.0.0] — 2024-09

### Added
- Initial project genesis: single-crate architecture, kernel, and phase planning

[Unreleased]: https://github.com/eshanized/M31A/compare/v0.1.6...HEAD
[0.1.6]: https://github.com/eshanized/M31A/releases/tag/v0.1.6
[0.1.5]: https://github.com/eshanized/M31A/releases/tag/v0.1.5
[0.1.4]: https://github.com/eshanized/M31A/releases/tag/v0.1.4
[0.1.3]: https://github.com/eshanized/M31A/releases/tag/v0.1.3
[0.1.2]: https://github.com/eshanized/M31A/releases/tag/v0.1.2
[0.1.1]: https://github.com/eshanized/M31A/releases/tag/v0.1.1
[0.1.0]: https://github.com/eshanized/M31A/releases/tag/v0.1.0

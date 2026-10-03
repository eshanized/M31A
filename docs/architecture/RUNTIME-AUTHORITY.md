# M31A v0.1.1 — Runtime Authority Model (post-remediation)

> Executable truth lives in `src/runtime_authorities.rs`, `src/runtime.rs`,
> `src/controller/dependencies.rs`, `src/agent/engine.rs`, and
> `tests/architecture_runtime_authority.rs`. This document is a map, not the territory:
> on any disagreement, the code and the invariant tests win.

## 1. What is the single production composition root?

`AppRuntime::from_pool_workspace_and_config` (`src/runtime.rs`). It composes
every runtime-critical authority exactly once per runtime scope and packs them
into a `RuntimeAuthorities` set. All other constructors are documented
compatibility shims (standalone/test use) or explicit scope rules:

- `ControllerDependencies::production*` — shims funneling to `assemble_with_shared_authorities`.
- `ProductionWorkerDispatcher::new*` — legacy standalone stack; production uses `from_shared_authorities`.
- `CliDispatcher::production` — pre-runtime fallback, always superseded by `with_runtime` on executing paths.
- `AppRuntime::run_mission_with_context` worktree branch — explicit MISSION_SCOPED
  re-derivation (same policy/budget/artifacts/approval/model; root-bound
  capabilities + compiler rebuilt deterministically from the worktree root).

## 2. Where is `RuntimeAuthorities` created?

In `from_pool_workspace_and_config`, via `RuntimeAuthorities::new(...)`, and
re-packed by `sync_authorities()` at the end of EVERY mutation path
(`with_config`, `with_model_caller`, `without_model_caller`,
`with_model_provider`, `with_approval_coordinator`, `with_model_catalog`).
`with_config` rebuilds through `RuntimeAuthorities::reconfigured` (policy +
caller re-derived; shared mutable state preserved by `Arc` carry-over).

## 3. Which components are authoritative?

Per scope: `ResolvedConfiguration`, `EffectivePolicy`, `CapabilityRegistry`,
`ToolRegistry`, `BudgetEnforcer` (mutable via `update_limits`),
`ApprovalCoordinator`, `Arc<RwLock<ModelCatalog>>`, provider, caller,
`ProductionContextCompiler` (workspace + prompt catalog + memory +
role-stage), `InMemoryPromptCatalog`, `FsArtifactStore`, `BroadcastEventBus`,
`GitService`, channel-aware storage roots. One instance each per runtime.

## 4. Which components are derived?

`ControllerDependencies` bundle, `ProductionWorkerDispatcher`,
`AgentEngine`, `PreExecutionCoordinator`, `WorkflowEngine`, CLI/TUI
projections, per-mission controllers. Derived objects receive `Arc`s and
never construct competing authorities (enforced by
`tests/architecture_runtime_authority.rs` static scans).

## 5. How does an AgentEngine bind to runtime authorities?

`AppRuntime::create_agent_engine` passes the shared instances positionally
(tool registry, pipeline, policy, coordinator, compiler, bus, capability
registry) and binds autonomy from configuration
(`AutonomyPrecedence::from_config`). Identity/role bind per scope via
`with_role_authority` / `bind_execution_identity` / `with_autonomy_mode` /
`with_scope_repos`. `ensure_execution_scope()` guarantees durable
(mission, task) rows before any side effect — tool execution under fictional
identities fails closed instead of running.

## 6. How does role authority propagate?

Typed `AgentRole` on the engine → capability envelope from
`AgentProfile::built_in(role).capability_policy` → `ToolExecutionContext.role_envelope`
(built by `build_execution_context()`) → pipeline `CapabilityCheckStage`.
Changing a role changes the envelope (Invariant 11 test). Subagent
delegation rebinds the child's `active_role` to the parsed target role;
prompt text is context, never authority.

## 7. How does policy evaluation propagate?

Exactly once per action, inside the pipeline (stage 7 `PolicyGateStage`,
`ToolPipelineRunner::execute_action`). The engine performs NO precheck; the
removed `WorkerRunner.policy_gate` precheck is gone for the same reason. The
evaluation request carries the bound mission/task/agent/role/workspace
identities from the execution context.

## 8. How does approval propagate?

Pipeline stage 8 (`ApprovalResolutionStage::execute_async`) builds a real
`ApprovalRequest` (bound identities, affected resources, risk, policy hash)
and calls `ApprovalCoordinator::request_approval` (persist → channel notify →
event → block with timeout). Operator decisions resolve through
`ApprovalCoordinator::resolve_request`; failures propagate as typed errors.
Every user-visible approval ID is a registered coordinator request
(`pending_request_ids`, Invariant 4 test); fabricated IDs fail resolution.

## 9. How are model/tool schemas derived?

`RuntimeAuthorities::governed_tool_schemas` — the SAME capability + tool
instances used for execution, filtered once by role envelope, policy
`denied_tools`, and autonomy mode. All caller (re)builds (constructor,
`reconfigured`, `with_catalog_rebound`, `with_model_provider`, dispatcher
`from_shared_authorities`) share it. `/tools` display inspects the attached
runtime registry via `CommandContext.tool_registry` (Invariant 2).

## 10. What invalidates a derived engine?

Any `with_config` reconfiguration (new authority generation) and any
runtime swap. `InteractiveSessionRunner` funnels all swaps through
`rebind_runtime`, which replaces the runtime AND clears `active_engine`
(Invariant 5; static test forbids direct `self.runtime = ` elsewhere).
The TUI bridge holds no cached engine (per-action construction).

## 11. How does configuration replacement work?

`with_config` → `authorities.reconfigured(new)` (new policy + caller,
same budget/catalog/coordinator/registries by `Arc`) → field mirrors synced
→ worktree rebuilt → `rebuild_dependencies()` reassembles the controller
bundle → externally cached engines are stale by generation and must be
recreated. `with_model_catalog` uses `with_catalog_rebound` so the caller
lock can never go stale (Invariant 6).

## 12. How are credentials resolved?

`resolve_runtime_credentials` (`src/runtime_authorities.rs`): channel
credential file (`nvidia_nim` key) → `NVIDIA_API_KEY` → `API_KEY_NVIDIA`.
The development channel NEVER reads the production file. All readers
(runtime, dispatcher, wizard, doctor, status) route through
`ProviderRegistry::channel_credentials_path` + this precedence. The
resolved key is injected explicitly into `NvidiaProvider::new_governed`.
`SafeEnvironmentStatus::probe` is diagnostics-only and never overrides the
binding (Invariant 8).

## 13. How is deployment-channel isolation guaranteed?

`DeploymentPaths` is the only channel-conditional path logic: db,
credentials, artifacts, telemetry, staging, state dir, socket, model-catalog
cache (`ModelCatalog::cache_path_for_channel`), capability spools/artifacts
all resolve per channel (production keeps legacy names; development uses
isolated siblings). Shared-by-design source metadata (config, prompts,
skills, policy file) is documented as shared. Invariant 9 test pins the
split; the dev-feature gate (`--features development`) compiles, lints, and
tests the same tree.

## 14. How are CLI/TUI consumers connected?

`CliDispatcher::with_runtime` attaches all authorities; `main.rs`
fail-closes when assembly fails for commands that require a runtime
(no warning + continue). `RunMission` without a runtime is a typed error,
never `status: "started"`. Status/banner/model-display paths read
`runtime.config()` + `active_provider_status()` (runtime truth), never env
probes or hardcoded provider/profile strings. `/tools` requires the
attached runtime registry.

## 15. What architectural tests prevent regression?

`tests/architecture_runtime_authority.rs`: 12 invariant tests (shared
registries by `Arc::ptr_eq`, bound identities, real approval IDs incl.
fake-ID rejection, generation change + rebind, catalog rebinding via
`bound_catalog_snapshot`, fail-closed CLI, channel-bound credentials,
storage isolation, controller bundle mirroring, typed roles, single policy
evaluation via a counting gate) plus 5 static source scans (no downstream
registry/caller/compiler construction, no direct runtime swaps, no hardcoded
autonomy default, no prompt sniffing in the production caller, no config
fallback on runtime paths). `docs/audits/WIRING-REMEDIATION-v0.1.1.md`
records the before/after graph and the remaining-match classification.

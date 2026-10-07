# M31A Configuration Authority

> Configuration is declared once, resolved once, and consumed everywhere.
> `/settings` is a projection/editor of the canonical configuration system,
> not a second configuration system.

## Authority

- Ordinary runtime defaults live in exactly one place:
  `src/config/canonical.rs` (consumed by `src/config/schema.rs` `default_*` fns).
- The single resolver is `src/config/resolved.rs` (`ResolvedConfiguration`).
- Flow:

```text
sources (system → user → workspace → profile → env → explicit → CLI → session)
→ precedence resolver (ResolvedConfiguration::build / build_with_report)
→ ResolvedConfiguration (app_config + active_model + active_provider + provenance)
→ runtime policy (timeout_policy / resource_policy / cache_policy / effective_model_selection)
→ subsystems (dispatcher, scheduler, tools, verification, repo, git, TUI)
→ TUI projection (/settings, model selector)
```

## Precedence (8 tiers, implemented)

```text
Tier 0: Immutable security invariants (fail closed, never overridable)
Tier 1: System (/etc/m31a/config.toml)
Tier 2: User (~/.config/m31a/config.toml)
Tier 3: Workspace (<workspace>/.m31a/config.toml)
Tier 4: Profile (canonical assets in assets/profiles/*.toml)
Tier 5: Environment (M31A_* tier only)
Tier 6: CLI (--config, --model, --profile, explicit layers)
Tier 7: Session overrides (/model, /profile, runtime overrides)
```

Lower layers never `unwrap_or("<literal>")` for a configured concern; they
consume the resolved value or fail closed. Legacy shim constructors without a
`ResolvedConfiguration` apply the canonical default (same authority), never a
private literal.

## Model / provider resolution

- Single model-selection authority: `ResolvedConfiguration::effective_model_selection`
  (`provider id`, `primary model`, `fast model`) + catalog metadata.
- Canonical default model/provider/endpoint: `config::canonical`
  (`CANONICAL_DEFAULT_MODEL`, `CANONICAL_DEFAULT_PROVIDER`,
  `CANONICAL_NVIDIA_BASE_URL`).
- Endpoint trust/security authority: `model::provider::endpoint`
  (re-exports the canonical URL; validates tier, scheme, destination before
  any credential is attached).
- Unknown metadata stays unknown (`ModelCandidate::new_unknown`, provenance
  `"unknown"`). The router fails closed when tool calling or context is
  required; it never fabricates 131K context / tool support / tier.
- The TUI model selector is projection-only: empty catalog renders an
  explicit empty state, never a fallback inventory.

## Resource policy hierarchy

```text
TimeoutPolicy (mission, workflow-step, verification, process/tool, approval)
ResourcePolicy (runtime, mission, per-role, process, research, metadata,
  tool timeout/output defaults)
ModelCatalogCachePolicy (local freshness, remote TTL, refresh-on-start/on-demand)
```

Configured values live in `[timeouts]`, `[resources]`, `[cache]` (+ legacy
`[runtime]` aliases). Immutable ceilings stay in code (`repo::query` ceilings,
`ResourceLimits::MAX_*`, sandbox, egress). Effective = configured clamped to
ceiling; configuration never raises a ceiling.

## Declarative systems

- Verification adapters: `assets/verification_adapters/*.toml` loaded via
  `ProjectAdapter::adapter_for`; Rust owns validation/execution/containment.
- Skills: `assets/skills/*/SKILL.toml` via `builtin_skills()` + same parser as
  external tiers (Builtin → System → User → Workspace).
- Profiles: `assets/profiles/*.toml` via `ProfileResolver` + same
  parser/resolver as external profiles.

## Persistence

Required DB fields are read strictly; missing columns yield a typed
persistence error with a migration hint. Nullable fields stay nullable.
Legacy rows migrate or are rejected, never silently defaulted into new
runtime behavior.

## Storage

Canonical authority: `StorageLayout` over `DeploymentPaths` (channel-isolated
`m31a` vs `m31a-dev`). `config::paths::PlatformPaths` and
`persistence::paths` delegate; legacy `.m31*` / `credentials.json` paths are
migration boundaries only.

## /settings

First-class `/settings [category]` command opening the settings surface,
which edits the canonical typed model (load → edit → validate → preview →
confirm → atomic persist → reload). Categories: General, Provider, Models,
Agents/Roles, Runtime, Budgets, Execution, Verification, Tools/Resource
Limits, Workflow, Git, Prompts/Skills, Cache, TUI/Interface,
Environment/Overrides, Effective Configuration, About. Secrets are masked.
Restart-required settings are labeled and never pretend live mutation.
Lower-precedence edits that lose to a higher tier show provenance instead of
false success.

## Canonical profiles (declarative assets in `assets/profiles/*.toml`)

`safe`, `conservative`, `coding`, `balanced`, `research`, `code_reviewer`,
`code-reviewer`, `autonomous`, `ci`, `security_review`, `security-review`,
`release`.

## Compatibility retained

- Legacy workspace cache / project DB paths migrate via `storage/migration`.
- Old provider/model fields parse but validate NVIDIA-only.
- Old profile formats resolve through the same inheritance + monotonic checks.
- Old env aliases (`NVIDIA_MODEL`, `API_KEY_NVIDIA`) honored at the Tier-5 boundary.
- Old config locations (`.m31`, `/etc/m31`) read as legacy fallbacks.

## Configuration Authority Map (integrity phase, implemented)

Every configuration domain has exactly one canonical owner. Runtime code
consumes the resolved value; it never restates the literal.

```text
Domain | Authority | Resolved Type | Runtime Consumers | Immutable Boundary
provider default | config::canonical::CANONICAL_DEFAULT_PROVIDER | String via effective_model_selection | dispatcher, model caller, TUI | NVIDIA-only validation (retired providers rejected)
active model | ResolvedConfiguration::active_model (+ agents.default_model) | String via effective_model_selection | dispatcher, router, TUI model selector | retired-provider qualifiers rejected
model capabilities | curated registry (nvidia_metadata reference records) + router | ModelCandidate | router, prompt compiler | unknown stays unknown (fail closed)
endpoint | config::canonical::CANONICAL_NVIDIA_BASE_URL; trust: model::provider::endpoint | validated URL | NvidiaProvider (governed ctor) | Tier0 builtin / allowlisted tiers trusted; workspace/env endpoints untrusted
runtime timeout | [runtime] timeout_secs → timeout_policy().runtime_secs | u64 secs | dispatcher wall clock | 1..=86400 (schema validation)
workflow-step timeout | [timeouts] workflow_step_secs | u64 secs | workflow engine | 1..=86400
verification timeout | [timeouts] verification_secs | u64 secs | verification runners, executor | 1..=86400
process timeout | [timeouts] process_secs | u64 secs | process supervisor | 1..=86400
approval timeout | [timeouts] approval_secs | u64 secs | approval coordinator | 0..=86400 (0 = no wait)
git timeout / auth TTL | canonical git consts | u64 secs | git commands, authorization | transport-level, not user policy
concurrency (runtime/mission/per-role/process/research/metadata) | [runtime]/[resources]/[workflow] → resource_policy() | usize | scheduler, resource manager, dispatcher | 1..=32 (schema); ceilings in ResourceLimits/sandbox
tool timeout/output | [resources] tool_* → ResourceLimits::effective_default | ResourceLimits | every TypedTool | MAX 600s / 10MiB (immutable)
repo query bounds | [resources]-adjacent query defaults → QueryBounds | QueryBounds | repo query/scanner | CEILING_* (200/4/64KiB)
agent step budgets | [budget] max_* (Option; None = unlimited) | BudgetConfig | controller enforcer, dispatcher | hard role ceilings in RoleRegistry (cannot be relaxed)
verification commands | assets/verification_adapters/*.toml + [workspace.verification] overrides | ProjectAdapter | verification hierarchy/executor | command safety gate (no shell metachars, cwd confinement)
skills | assets/skills/*/SKILL.toml → builtin_skills() + tiered discovery | SkillManifest | skill loader, prompts | same parser for builtin and external tiers
profiles | assets/profiles/*.toml → ProfileResolver | ProfileConfig | session profile overlay | monotonic security (cannot weaken invariants)
prompts | prompt catalog authority (context authority owns catalog) | PromptCatalog | planner, context compiler | planner binds the same catalog instance
storage paths | StorageLayout over DeploymentPaths (channel-isolated) | PathBuf | persistence, artifacts, TUI, cache | channel isolation (m31a vs m31a-dev)
model catalog cache | [cache] freshness/TTL → cache_policy() | ModelCatalogCachePolicy | catalog loader, metadata resolver | file location fixed under global cache
TUI prefs (fps/theme/compact) | [tui] → AppConfig.tui | TuiConfig | TUI renderer | fps 1..=120; theme closed vocabulary; empty-state default is `DEFAULT_TUI_THEME` via `ThemeMode::canonical_default()` (wizard, app init, resolver, /settings all agree)
git behavior | [git] push_policy/branch_prefix/retention | GitConfig | git tools, wizard | push_policy typed enum (allow/ask/deny)
environment overrides | Tier 5 (M31A_MODEL, M31A_CONCURRENCY, M31A_TIMEOUT[_SECS], M31A_THEME, M31A_MAX_STEPS*, M31A_AUTO_COMMIT, M31A_TEST_COMMAND, M31A_DENIED_TOOLS, + aliases) | engine Tier5 layer | resolver only | never probed ambiently by subsystems
logging/telemetry | telemetry stream bounds (rotation) | NdjsonStreamWriter | telemetry pipeline | append-only, rotation size fixed
```

## Resolution pipeline (implemented)

```text
raw sources (system → user → workspace → explicit --config → profile → env → CLI scalars → session)
  → ConfigPrecedenceEngine (8 tiers, monotonic security check)
  → parse + validate_config (unknown fields + logical bounds fail closed)
  → ResolvedConfiguration { app_config, active_model, active_provider, provenance, loaded_sources }
  → typed policies (timeout_policy / resource_policy / cache_policy / effective_model_selection)
  → runtime consumers (dispatcher, scheduler, tools, verification, repo, git, TUI)
  → /settings projection (editor of the above, never a second store)
```

Precedence (highest wins): Tier 7 session > Tier 6 CLI > Tier 5
environment > Tier 4 profile > Tier 3 workspace > Tier 2 user > Tier 1
system > Tier 0 built-in defaults. Invalid *present* configuration fails
closed (`build()` errors; `build_fallback` is display-only and reports via
`build_with_report`).

## Hardcoded-value classification (integrity phase audit)

Every remaining hardcoded operational-looking value is exactly one of:

- IMMUTABLE SAFETY: endpoint trust, egress policy, sandbox ceilings,
  `ResourceLimits::MAX_*`, `repo::query::CEILING_*`, role step ceilings.
- PROTOCOL/SCHEMA: catalog schema version, serialization compat defaults
  (`workflow/manifest`, `process/identity`), MIME/extension maps.
- ALGORITHM: CPM duration estimate, DAG ordering midpoint
  (`DEFAULT_DAG_TASK_PRIORITY`), budget estimation fallback
  (`FALLBACK_ESTIMATED_TOKENS`, explicitly non-authoritative), rate-limit
  cooldown (`RATE_LIMIT_DEFAULT_COOLDOWN_SECS`), tokenizer heuristics,
  composer layout geometry, plan-estimate fallbacks.
- TEST FIXTURE: `CANONICAL_REAL_MODEL_*` (live-model harness only),
  `strong_reasoning_default` / `constrained_local_default` (prompt
  adaptation tests), mock provider (test-only, rejected in production).
- COMPATIBILITY: retired provider descriptors (`is_production_supported =
  false`, deterministic rejection only), legacy storage/credential paths,
  old env aliases, persistence `unwrap_or(None)` / empty-collection
  defaults preserving absence semantics.
- LEGITIMATE BUILTIN ASSET: verification adapter TOMLs, skill TOMLs,
  canonical profile TOMLs, curated capability-metadata reference records
  (metadata, never selection), wizard display-only labels
  (`WIZARD_DISPLAY_DEFAULT_BUDGET_DOLLARS`, `UNAVAILABLE` provider labels).

## Dispatcher configuration contract (close-out phase, implemented)

```text
raw sources → canonical resolution → ResolvedConfiguration → dispatcher → runtime
```

- `ProductionWorkerDispatcher::from_shared_authorities` and
  `new_with_roots_and_config` take `config: &ResolvedConfiguration`
  (never `Option`). There is no `None`-means-defaults path: constructors
  consume resolved values directly, with no fallback literal of any kind.
- `ControllerDependencies::production_with_shared_authorities`,
  `production_with_model_config_and_coordinator`,
  `production_with_model_and_config`, `assemble_with_shared_authorities`,
  and `budget_for_config` likewise require `&ResolvedConfiguration`.
- Compatibility shims (standalone/test only), each resolving through the
  canonical path once and delegating — never restating policy:
  `ProductionWorkerDispatcher::new` / `new_with_workspace` /
  `new_with_roots` (via `ResolvedConfiguration::build_fallback`) and
  `ControllerDependencies::production` / `production_with_model`.
  New production code must not use them (enforced by
  `no_bare_dispatcher_construction_in_production_code`).
- Test construction is explicit: fixtures resolve
  `ResolvedConfiguration::build_fallback` / `for_workspace` first, then
  pass `&config` (e.g. `workflow_authority_convergence`,
  `workflow_e2e_canonical`, `remediation_single_authority`).
- Display-only CLI config inspection (`CliDispatcher.config: None`) still
  resolves via `build_fallback` — the documented display-only use, never
  an execution policy source.
- Override-shaped `Option`s with genuine absence semantics are retained:
  `WorkspaceVerificationConfig` overrides (absence = auto-detect),
  `PolicyConfig` (absence = compiled fail-closed standard), `model_caller =
  None` (explicit fail-closed no-provider caller).

## Enforcement (integrity phase)

- `tests/config_architecture_enforcement.rs`: runtime-consumption proofs
  (dispatcher timeout follows config), compat-shim non-divergence,
  settings round-trip + provenance,
  invalid-persist refusal, no-partial-mutation, restart flags, ceiling
  clamps, canonical-consumer pins, theme single-default chain
  (resolver == wizard == settings == runtime hydration), dispatcher
  `&ResolvedConfiguration` contract scans, scope-aware source scans.
- `tests/config_authority_invariants.rs`: precedence, single model/endpoint
  authority, policy scopes, ceilings, declarative adapters/skills/profiles,
  settings atomicity, secret masking.
- `tests/configuration_authority.rs`: 8-tier engine matrix, monotonic
  security, credential masking, session mutation, adapter detection, CLI
  config commands.
- `scripts/check_config_authority.sh`: fast CI guard mirroring the
  in-test scans (bare operational fallbacks, scattered production
  literals, canonical-consumer pins, dispatcher `&ResolvedConfiguration`
  contract, no bare dispatcher construction).

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

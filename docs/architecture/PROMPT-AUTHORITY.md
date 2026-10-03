# PROMPT AUTHORITY — Canonical PromptOS Ownership (v0.1.1)

Status: CANONICAL. The model proposes. The runtime decides — and the runtime
compiles the prompt.

```text
PromptAsset (prompts/**/*.toml)
    ↓ embed + validate (prompt::builtins)
PromptContract (versioned, hashed)
    ↓ register (InMemoryPromptCatalog — ONE production instance)
PromptCatalog (runtime-shared; RuntimeAuthorities::prompt_catalog)
    ↓ resolve_canonical (exact id/version + v1→v2 upgrade + alias routing)
PromptReference { id, version, purpose }
    ↓ Role binding (RoleRegistry → AgentProfile.prompt_ref)
    ↓ Workflow binding (WorkflowStepDefinition.prompt_ref → CandidateTask
        → Task [migration 025] → WorkItem → WorkExecutionRequest → WorkerRunner)
ContextCompilationRequest { prompt_ref, prompt_source }
    ↓ precedence ExplicitTask > ExplicitSession > RoleDefault > fail-closed
PromptCompiler (runtime-shared DefaultPromptCompiler; RuntimeAuthorities::prompt_compiler)
    ↓ 7 layers L0..L6, trust envelopes, budget reverse-compaction
EffectivePrompt (+ PromptInvocationProvenance)
    ↓ typed ModelInvocation (review/recovery) or CompiledContext+provenance
ModelCaller → provider
```

## Ownership

| Decision | Owner | Notes |
|---|---|---|
| Who chooses the prompt | Workflow step (`prompt_ref`) or role registry (`prompt_contract`+`prompt_version`); never prompt text | `effective_prompt_ref()`; `PromptReference::for_role` |
| Who resolves/canonicalizes | `PromptCatalog::resolve_canonical` | v1→v2 same-id upgrade; `legacy_aliases`; deprecation pointers |
| Who compiles | `DefaultPromptCompiler` (single runtime instance) | Variants via typed `CompilationOptions`, never a second compiler |
| Who may override | Nobody in production: overrides are workspace/project tiered inputs that either replace non-protected contracts by precedence or land as lower-trust L5 guidance (behavioral + Layer-0 ids never replaceable) | `is_protected_contract_id`; `project_guidance_for` |
| Who can fall back | Nobody silently. `resolve_canonical` v2-with-v1 retrieval fallback in planning is the only documented compat rule; otherwise missing/uncompilable prompts fail closed before any model call | `PromptError` → typed runtime error |
| Who records provenance | `DefaultPromptCompiler` (invocation record) → `ProductionContextCompiler` (manifest + `CompiledContext.prompt_provenance`) → `WorkerRunner` (step records) / `AgentEngine` (`last_prompt_provenance`) | Provenance always describes the RESOLVED contract |
| Who can invalidate cached prompt state | Reconfiguration rebuilds the full `RuntimeAuthorities` set (`reconfigured` carries the same catalog+compiler `Arc`s; `with_catalog_rebound` rebuilds the caller atomically). Workers hold no caches: they receive the compiler per dispatch | No stale-catalog reads (test G) |
| Skill guidance trust | `skill.in_task_guidance` compiles only via `compile_in_task_guidance_with_catalog` as `UntrustedRepoContent` L5; authoritative system prompts are byte-identical with/without it | `PromptCompiler::compile_with_guidance` |
| Safety invariants | Layer-0 `core.safety` v2 + immutable `RUNTIME_SAFETY_INVARIANTS` L0 prepend; unprunable, budget-fatal on overflow | Kernel authority, never workspace-overrideable |

## Selection precedence (enforced, recorded)

`ExplicitTask` (workflow/task `prompt_ref`) > `ExplicitSession`
(agent/session `prompt_ref`) > `RoleDefault` (registry) > typed failure.
The winning source is recorded in `ContextCompilationContract`
(`prompt_id`, `prompt_version`, `prompt_source`, `prompt_content_hash`).

## Version rule

One canonical generation per id (`reachability::canonical_version`; v2 where
it exists). Normal production consumers resolve canonical; V1 survives only as
`IntentionalCompatibility` (same-id generations, alias routes) or
`ExplicitlyDeprecated` (renamed/superseded ids, metadata-only descriptors).

## Reconfiguration coherence

`RuntimeAuthorities::reconfigured` re-derives policy + model caller while
carrying catalog, compiler, registries, and coordinator by `Arc` clone —
prompt authorities are never forked across a reconfig. `sync_authorities`
re-packs after every runtime mutation so holders of a previous generation
detect staleness by pointer comparison.

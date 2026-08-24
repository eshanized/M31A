# Phase 02: LLM Provider Abstraction - Context

**Gathered:** 2026-08-24
**Status:** Ready for planning

<domain>
## Phase Boundary

Provider-agnostic interface with NVIDIA Build adapter supporting streaming, reasoning, and coding-agent kwargs; credentials never touch disk or logs. The phase delivers a clean LLMProvider interface with both streaming and non-streaming methods, NVIDIA Nemotron 3 Ultra adapter with reasoning support, model profile configuration, streaming resilience, API-based capability detection, CLI models list command, and explicit provider selection strategy.
</domain>

<decisions>
## Implementation Decisions

### Interface Completeness
- **D-01:** Add both ChatCompletion and ChatCompletionStream with shared internals — **Reversibility:** costly — adding non-streaming method to interface requires all 3 provider implementations to be updated; ChatResponse type in core/types affects pkg/ packages
- **D-02:** New ChatResponse type in internal/core/types/types.go — **Reversibility:** costly — type definition in shared vocabulary; changing it requires updates across all layers (provider, workflow, TUI)
- **D-03:** ChatCompletion required in LLMProvider interface (all providers must implement) — **Reversibility:** one-way — interface contract change; removing would break implementations
- **D-04:** ChatCompletion uses ChatCompletionStream internally and collects chunks — **Reversibility:** reversible — implementation detail, can be optimized later

### Reasoning for Nemotron 3 Ultra
- **D-05:** Add reasoningParamMap entry for nvidia/nemotron-3-ultra-550b-a55b with enable_thinking=true, force_nonempty_content=true, reasoning_budget configurable — **Reversibility:** reversible — config addition only; can be removed or modified
- **D-06:** Include force_nonempty_content=true in chat_template_kwargs for coding-agent compatibility — **Reversibility:** reversible — NVIDIA-specific parameter; can be toggled via config
- **D-07:** Configurable reasoning_budget default (32768) with override via config.toml — **Reversibility:** reversible — config value only
- **D-08:** Support both NVIDIA extra_body and OpenAI-style reasoning parameter — **Reversibility:** reversible — both patterns already supported in reasoning.go; just extending usage

### Model Profile Configuration
- **D-09:** Both provider defaults + model overrides for profile structure — **Reversibility:** costly — adds ModelProfile type to shared types; config schema change affects all config loading
- **D-10:** Profile merging in BaseClient (shared) — **Reversibility:** reversible — implementation location; can be moved to caller later
- **D-11:** Profiles stored in config.toml (layered: global → workspace → project → env) — **Reversibility:** costly — config schema change; migration needed if structure changes
- **D-12:** Profile scope: standard params (temperature, top_p, max_tokens, reasoning) + reasoning config ref — **Reversibility:** reversible — limited scope avoids provider-specific complexity

### Streaming Resilience & Recovery
- **D-13:** Configurable retry strategy (none / initial_only / full_resume), default initial_only — **Reversibility:** reversible — config-driven behavior; can change default or add strategies
- **D-14:** Explicit empty delta handling — skip silently in ParseSSEChunk — **Reversibility:** reversible — parser behavior only; no interface change
- **D-15:** Sequential chunks sufficient for mixed transitions (reasoning then content) — **Reversibility:** reversible — matches provider behavior; combined chunks can be added if needed
- **D-16:** Retry logic in BaseClient (shared) — **Reversibility:** reversible — centralized implementation; can be overridden per-provider

### Capability Detection from API
- **D-17:** Fetch from API + fallback to heuristics for capability detection — **Reversibility:** reversible — enhancement to existing FetchModels; heuristics remain as fallback
- **D-18:** Extend ModelInfo with MaxOutputTokens, SupportedParameters, InputModalities, OutputModalities — **Reversibility:** costly — shared type change; affects all consumers of ModelInfo (provider, workflow, TUI)
- **D-19:** Per-provider parsing in each provider's FetchModels — **Reversibility:** reversible — implementation location; shared helper can be added later
- **D-20:** Cache capabilities with model list in ModelCache — **Reversibility:** reversible — extends existing cache; TTL already configurable

### CLI models list Command
- **D-21:** Add m31a models list command with table (default) + JSON output — **Reversibility:** reversible — new CLI command only; no existing behavior affected
- **D-22:** Simple subcommand in main.go (no cobra dependency) — **Reversibility:** reversible — implementation approach; can migrate to cobra later
- **D-23:** Basic filtering (--provider <name>) — **Reversibility:** reversible — minimal filtering; can be extended

### Provider Selection Strategy
- **D-24:** Explicit --provider flag + config default_provider for selection — **Reversibility:** reversible — config field + CLI flag; no breaking changes
- **D-25:** Both global default + per-request override via ChatRequest.Provider — **Reversibility:** reversible — adds Provider field to ChatRequest (shared type); backward compatible
- **D-26:** Configurable FallbackMode (auto/manual/prompt), default manual — **Reversibility:** reversible — config-driven behavior; safe default
- **D-27:** Provider selection logic in registry (centralized GetProviderForRequest) — **Reversibility:** reversible — implementation location; callers use registry helper

### the agent's Discretion
- Exact retry intervals and max attempts for streaming retry strategies
- Default reasoning_budget value for nemotron-3-ultra (suggested 32768)
- Table column layout and formatting for models list command
- Exact FallbackMode default (manual) and prompt wording

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Architecture & Requirements
- `.planning/CONTEXT_M31A.md` §34 — NVIDIA Build integration (provider, auth, streaming, reasoning)
- `.planning/CONTEXT_M31A.md` §8 — Core domain model (shared types)
- `.planning/REQUIREMENTS.md` — LLM-01 through LLM-06 (6 requirements)
- `.planning/ROADMAP.md` — Phase 2 success criteria (5 criteria)

### Current Implementation
- `internal/integrations/provider/interface.go` — Current LLMProvider interface
- `internal/integrations/provider/nvidia/client.go` — NVIDIA adapter implementation
- `internal/integrations/provider/base_client.go` — Shared BaseClient with retry, cache, SSE
- `internal/integrations/provider/reasoning.go` — Reasoning config for models
- `internal/integrations/provider/capabilities.go` — Capability detection heuristics
- `internal/integrations/provider/sse.go` — SSE parser with watchdog
- `internal/integrations/provider/registry.go` — Provider registry with fallback priority
- `internal/core/types/types.go` — ChatRequest, StreamChunk, ModelInfo, CapFlags, ChatResponse (to be added)
- `internal/core/config/loader.go` — Layered config loading (global → workspace → project → env)
- `internal/integrations/keychain/keychain.go` — OS-native credential storage

### Research & Risks
- `.planning/research/SUMMARY.md` — Research synthesis: stack, table stakes, pitfalls, confidence
- `.planning/research/PITFALLS.md` — Pitfall 10: Provider leakage (provider-specific code in core)
- `.planning/codebase/INTEGRATIONS.md` — Current provider architecture (3 providers, registry, capabilities)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **Provider registry** (`internal/integrations/provider/registry.go`): Lazy registration, fallback priority, GetProvider/GetDefaultProvider — extend with GetProviderForRequest
- **BaseClient** (`internal/integrations/provider/base_client.go`): Shared HTTP clients, ModelCache, retry logic, SSE parsing — add profile merging and retry strategy
- **NVIDIA client** (`internal/integrations/provider/nvidia/client.go`): buildNvidiaBody with extra_body, chat_template_kwargs — extend for ultra model
- **Reasoning config** (`internal/integrations/provider/reasoning.go`): reasoningParamMap with per-model configs — add ultra entry
- **Capabilities** (`internal/integrations/provider/capabilities.go`): ParseModelCapabilities heuristics — extend with API parsing
- **SSE parser** (`internal/integrations/provider/sse.go`): Watchdog timeout, buffer pooling — add empty delta handling
- **Config loader** (`internal/core/config/loader.go`): Layered TOML/env/flags — add model_profiles section
- **Keychain** (`internal/integrations/keychain/keychain.go`): OS-native credential storage — already used for provider keys

### Established Patterns
- **Provider interface in internal/integrations/provider/interface.go**: Aliases types from internal/core/types — new ChatResponse follows same pattern
- **Lazy provider initialization**: Providers registered on first LLM call — models list command should trigger initialization
- **Structured error handling**: sentinel errors (ErrInvalidKey, ErrRateLimited, ErrModelNotFound) — provider selection should use these
- **Single-threaded UI with message passing**: Goroutines send tea.Msg — streaming retry must not block UI thread
- **Project-local sessions**: Session data in `<workDir>/.m31a/` — config follows same pattern

### Integration Points
- **Config → BaseClient**: Config loader provides model_profiles, default_provider, fallback_mode to BaseClient
- **BaseClient → Provider clients**: Profile merging and retry strategy in BaseClient used by all providers
- **Registry → Workflow engine**: GetProviderForRequest used by workflow engine for provider selection
- **ChatRequest → Provider**: Provider field in ChatRequest for per-request override
- **FetchModels → ModelCache**: Capability enrichment happens during cache refresh
- **CLI → Registry**: models list command calls registry to fetch from all providers

</code_context>

<specifics>
## Specific Ideas

- Reasoning config for ultra model: `enable_thinking: true`, `force_nonempty_content: true`, `reasoning_budget: 32768` (configurable)
- ModelProfile type with fields: ModelID, Temperature, TopP, MaxTokens, ReasoningEnabled, ReasoningBudget, ReasoningConfigRef
- StreamRetryConfig with Mode (none/initial_only/full_resume), MaxAttempts, BaseDelay
- Models list output: table columns (ID, Provider, ContextLen, Tools, Reasoning, Vision) + JSON with full ModelInfo
- Provider selection: --provider flag > ChatRequest.Provider > config.default_provider > first available with valid key
- FallbackMode=manual: return error with provider-specific message; FallbackMode=auto: try next in FallbackPriority; FallbackMode=prompt: ask via TUI

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.
</deferred>

---

*Phase: 02-LLM Provider Abstraction*
*Context gathered: 2026-08-24*
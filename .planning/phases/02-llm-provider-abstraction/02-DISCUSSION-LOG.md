# Phase 02: LLM Provider Abstraction - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-08-24
**Phase:** 02-llm-provider-abstraction
**Areas discussed:** Interface Completeness, Reasoning for Nemotron 3 Ultra, Model Profile Configuration, Streaming Resilience & Recovery, Capability Detection from API, CLI models list Command, Provider Selection Strategy

---

## Interface Completeness

| Option | Description | Selected |
|--------|-------------|----------|
| Add ChatCompletion method | Add non-streaming ChatCompletion(ctx, req) (*ChatResponse, error) alongside ChatCompletionStream. Needed for tool calls requiring complete JSON responses and simpler single-turn completions. | |
| Keep streaming-only | Only support ChatCompletionStream. Non-streaming can be implemented by collecting all stream chunks. Simpler interface, less surface area. | |
| Add both with shared internals | Add ChatCompletion that internally uses ChatCompletionStream and collects chunks. Both methods available but streaming is the primitive. | ✓ |

**User's choice:** Add both with shared internals
**Notes:** 

---

## Interface Completeness

| Option | Description | Selected |
|--------|-------------|----------|
| New ChatResponse type | Define types.ChatResponse with Content, ToolCalls, Usage, FinishReason fields. Cleaner API, explicit contract for non-streaming consumers. | ✓ |
| Reuse StreamChunk | Return the final StreamChunk (Type=done) with Content, ToolCalls, Usage. No new types needed, but less explicit about what's returned. | |
| Return accumulated content string | Return just the concatenated content string. Simplest but loses tool calls, usage, finish reason. | |

**User's choice:** New ChatResponse type
**Notes:** 

---

## Interface Completeness

| Option | Description | Selected |
|--------|-------------|----------|
| internal/core/types/types.go | Add ChatResponse to types.go alongside ChatRequest. Keeps shared vocabulary in one place. pkg/ packages can reference without importing internal/. | ✓ |
| internal/integrations/provider/interface.go | Define ChatResponse in interface.go alongside LLMProvider. Keeps provider-specific types together. But pkg/ packages would need to import internal/. | |
| Both - types for pkg, interface for internal | Add ChatResponse to internal/core/types/types.go for pkg access, and keep LLMProvider in interface.go returning types.ChatResponse. Best of both. | |

**User's choice:** internal/core/types/types.go
**Notes:** 

---

## Interface Completeness

| Option | Description | Selected |
|--------|-------------|----------|
| Required in LLMProvider | Add ChatCompletion to the main LLMProvider interface. All 3 providers (OpenRouter, Zen, Nvidia) must implement it. Ensures consistent capability. | ✓ |
| Optional extension interface | Create a separate NonStreamingProvider interface with ChatCompletion. Providers implement it only if they support non-streaming. More flexible but requires type assertion at call sites. | |
| Required with default implementation | Add ChatCompletion to LLMProvider but provide a default implementation in BaseClient that uses ChatCompletionStream. Providers can override if they have a more efficient native non-streaming endpoint. | |

**User's choice:** Required in LLMProvider
**Notes:** 

---

## Reasoning for Nemotron 3 Ultra

| Option | Description | Selected |
|--------|-------------|----------|
| Add ultra model config | Add reasoningParamMap entry for nvidia/nemotron-3-ultra-550b-a55b with enable_thinking=true and reasoning_budget (e.g., 32768). Matches NVIDIA NIM API for ultra model. | ✓ |
| Use same config as nano | The ultra model may accept the same chat_template_kwargs as nano. Reuse the existing nvidia/nemotron-3-nano-omni config pattern with a broader prefix match. | |
| Make it configurable | Add config option to override reasoning params per model in config.toml. Users can set enable_thinking and reasoning_budget per model without code changes. | |

**User's choice:** Add ultra model config
**Notes:** 

---

## Reasoning for Nemotron 3 Ultra

| Option | Description | Selected |
|--------|-------------|----------|
| Add force_nonempty_content | Include force_nonempty_content=true in chat_template_kwargs for ultra model. Ensures non-empty content even when reasoning is enabled (coding agents need actual code output). | ✓ |
| Skip force_nonempty_content | Only enable_thinking and reasoning_budget. The ultra model may not need force_nonempty_content or it may be handled differently. | |
| Make both configurable | Add both enable_thinking and force_nonempty_content as config options. Users can toggle based on their coding agent needs. | |

**User's choice:** Add force_nonempty_content
**Notes:** 

---

## Reasoning for Nemotron 3 Ultra

| Option | Description | Selected |
|--------|-------------|----------|
| 32768 (double nano) | Double the nano budget. Reasonable for ultra's larger context and more complex reasoning tasks. | |
| 65536 (max for 128k context) | Half of 128k context window. Maximum reasonable budget leaving room for prompt + output. | |
| Configurable default | Set a sensible default (e.g., 32768) but allow override via config.toml per model profile. | ✓ |

**User's choice:** Configurable default
**Notes:** 

---

## Reasoning for Nemotron 3 Ultra

| Option | Description | Selected |
|--------|-------------|----------|
| Keep NVIDIA extra_body only | NVIDIA NIM uses extra_body.chat_template_kwargs and extra_body.reasoning_budget. Don't add OpenAI-style reasoning param to avoid confusion. | |
| Support both | Add OpenAI-style 'reasoning' parameter at top level AND keep NVIDIA extra_body. Provider-specific code handles translation. More portable across providers. | ✓ |
| Abstract in reasoning config | ReasoningConfig already has RequestParams and ExtraBodyParams. Use RequestParams for OpenAI-style, ExtraBodyParams for NVIDIA. Already supports both patterns. | |

**User's choice:** Support both
**Notes:** 

---

## Model Profile Configuration

| Option | Description | Selected |
|--------|-------------|----------|
| Add ModelProfile type | Define types.ModelProfile with ModelID, Temperature, TopP, MaxTokens, ReasoningEnabled, ReasoningBudget. Stored in config.toml under [model_profiles]. Merged with ChatRequest at call time (call values override profile). | |
| Use provider config only | Keep options in provider config (provider.temperature, provider.max_tokens, etc.). Simpler but less granular - same settings for all models from a provider. | |
| Both - provider defaults + model overrides | Provider-level defaults in config (applies to all models), with optional model-specific overrides in [model_profiles]. Most flexible. | ✓ |

**User's choice:** Both - provider defaults + model overrides
**Notes:** 

---

## Model Profile Configuration

| Option | Description | Selected |
|--------|-------------|----------|
| In BaseClient (shared) | Add MergeProfile method to BaseClient that takes ChatRequest and model ID, looks up profile, merges defaults. All providers inherit. Centralized logic. | ✓ |
| In workflow engine / caller | Caller (workflow engine) resolves profile and merges before calling provider.ChatCompletionStream. Keeps providers simple, caller controls. | |
| In provider client (per-provider) | Each provider implements its own profile merging. Allows provider-specific behavior but duplicates logic. | |

**User's choice:** In BaseClient (shared)
**Notes:** 

---

## Model Profile Configuration

| Option | Description | Selected |
|--------|-------------|----------|
| In config.toml (layered) | Add [model_profiles] section to config.toml. Follows existing layered config pattern. Can have different profiles per workspace/project. | ✓ |
| Separate model_profiles.toml | Separate file for model profiles. Cleaner separation, easier to share across projects. But adds another config file to manage. | |
| Both - config.toml with include | Keep [model_profiles] in config.toml but support include directive to pull from external file. Best of both. | |

**User's choice:** In config.toml (layered)
**Notes:** 

---

## Model Profile Configuration

| Option | Description | Selected |
|--------|-------------|----------|
| Standard params only | Profiles only store temperature, top_p, max_tokens, reasoning_enabled, reasoning_budget. Provider-specific params handled in provider code via reasoning config. | |
| Include provider-specific | Allow arbitrary key-value map in ModelProfile for provider-specific settings (extra_body, headers, etc.). More flexible but less type-safe. | |
| Standard + reasoning config ref | Standard params plus optional reference to a named reasoning config (e.g., 'nvidia_ultra'). Reasoning params defined separately in reasoning.go map. | ✓ |

**User's choice:** Standard + reasoning config ref
**Notes:** 

---

## Streaming Resilience & Recovery

| Option | Description | Selected |
|--------|-------------|----------|
| Add mid-stream retry/resume | On connection drop during streaming, automatically reconnect and request continuation from last chunk. Requires provider support for resumable streams (OpenAI doesn't natively support this). | |
| Retry only initial request | Current behavior: retry the entire request up to 2 times (exponential backoff) before streaming starts. If stream drops mid-way, fail and let caller handle. Simpler, works with all providers. | |
| Configurable strategy | Add StreamRetryConfig with options: 'none' (current), 'initial_only' (retry before stream), 'full_resume' (attempt mid-stream resume where supported). Default to 'initial_only'. | ✓ |

**User's choice:** Configurable strategy
**Notes:** 

---

## Streaming Resilience & Recovery

| Option | Description | Selected |
|--------|-------------|----------|
| Add explicit empty delta handling | In ParseSSEChunk, detect empty delta (no content, no reasoning, no tool_calls) and return nil chunk (skip silently). Prevents empty chunks from propagating to TUI. | ✓ |
| Return empty content chunk | Return StreamChunk{Type: "content", Delta: ""} for empty deltas. Caller decides whether to render. More explicit but may cause flicker. | |
| Already handled | Current code returns nil for chunks without content/reasoning/tool_calls (line 199, 208, 279). Verify this covers all empty delta cases. | |

**User's choice:** Add explicit empty delta handling
**Notes:** 

---

## Streaming Resilience & Recovery

| Option | Description | Selected |
|--------|-------------|----------|
| Sequential chunks sufficient | Current approach: reasoning chunk (Type=thinking) then content chunk (Type=content). TUI renders sequentially. Matches how most providers stream. | ✓ |
| Support combined chunks | Add StreamChunk with both ThinkingDelta and ContentDelta fields. For providers that send both in same SSE event. More accurate but adds complexity. | |
| Track state in iterator | Iterator tracks if we're in reasoning mode. When content starts after reasoning, emit a transition marker. Helps TUI show reasoning→content boundary clearly. | |

**User's choice:** Sequential chunks sufficient
**Notes:** 

---

## Streaming Resilience & Recovery

| Option | Description | Selected |
|--------|-------------|----------|
| In BaseClient (shared) | Add retry wrapper in BaseClient.ChatCompletionStream that wraps the provider's doChatStream. All providers get consistent retry behavior. Can configure via BaseClient options. | ✓ |
| Per-provider (current) | Each provider implements its own retry logic. NVIDIA has 2 retries with exponential backoff. Allows provider-specific retry policies but duplicates code. | |
| In SSE parser | Add retry logic to SSEParser itself - on connection drop, reconnect and resume. But requires provider support for resumable streams (rare). | |

**User's choice:** In BaseClient (shared)
**Notes:** 

---

## Capability Detection from API

| Option | Description | Selected |
|--------|-------------|----------|
| Fetch from API + fallback | Extend FetchModels to parse capability fields from API response (e.g., OpenRouter returns context_length, supported_parameters). Fall back to ParseModelCapabilities heuristics for providers that don't return capabilities. | ✓ |
| Keep heuristics only | Current approach: infer from model ID patterns. Works for known model families. Simpler, no API dependency. But may miss new models or provider-specific capabilities. | |
| Hybrid: API enriches heuristics | FetchModels gets models from API, then calls EnrichModelInfo which merges API metadata with heuristic detection. Best of both - API provides ground truth, heuristics fill gaps. | |

**User's choice:** Fetch from API + fallback
**Notes:** 

---

## Capability Detection from API

| Option | Description | Selected |
|--------|-------------|----------|
| Extend ModelInfo | Add fields like MaxOutputTokens, SupportedParameters ([]string), InputModalities ([]string), OutputModalities ([]string) to types.ModelInfo. Captures full API metadata. | ✓ |
| Keep current ModelInfo | Current fields (ContextLength, Capabilities, Pricing, Architecture) are sufficient. Additional metadata stored in a separate raw map or extension field. | |
| Add Extensions map | Add Extensions map[string]any to ModelInfo for provider-specific metadata. Flexible but less type-safe. Can store raw API response fields. | |

**User's choice:** Extend ModelInfo
**Notes:** 

---

## Capability Detection from API

| Option | Description | Selected |
|--------|-------------|----------|
| Per-provider parsing | Each provider (OpenRouter, Zen, Nvidia) implements ParseModelCapabilitiesFromAPI(rawResponse) -> types.ModelInfo. Handles provider-specific response formats. Called from FetchModels after fetching. | ✓ |
| Shared with provider adapters | Shared ParseAPIModelResponse function that takes provider name and raw JSON, uses provider-specific adapter functions to extract fields. Centralized but extensible. | |
| In FetchModels directly | Keep parsing inline in each provider's FetchModels. Simpler, no extra abstraction. But duplicates logic if multiple endpoints return model info. | |

**User's choice:** Per-provider parsing
**Notes:** 

---

## Capability Detection from API

| Option | Description | Selected |
|--------|-------------|----------|
| Cache with model list | ModelCache stores fully enriched ModelInfo (with API-detected capabilities). Cache TTL applies. On cache refresh, re-fetch and re-parse capabilities. Consistent with current caching. | ✓ |
| Separate capability cache | Separate cache for capabilities with different TTL. Capabilities change less frequently than model list. More granular control. | |
| No caching for capabilities | Always fetch fresh capabilities from API. Ensures up-to-date info but adds latency to FetchModels. Not recommended for production. | |

**User's choice:** Cache with model list
**Notes:** 

---

## CLI models list Command

| Option | Description | Selected |
|--------|-------------|----------|
| Add models list command | Add m31a models list [--provider <name>] [--format table|json]. Uses provider registry to fetch models from all or specific provider. Shows ID, Provider, ContextLength, Capabilities (Tools/Reasoning/Vision). | ✓ |
| Add to existing chat command | Add --list-models flag to m31a chat. Simpler but less discoverable. m31a chat --list-models shows available models. | |
| Both - dedicated command + flag | Add m31a models list as primary, and --list-models flag to chat for convenience. Best UX. | |

**User's choice:** Add models list command
**Notes:** 

---

## CLI models list Command

| Option | Description | Selected |
|--------|-------------|----------|
| Add to main.go flag parsing | Extend main.go with models subcommand handling. Consistent with current approach (--prompt, --goal, --model flags). No new dependencies. | |
| Use cobra | Add github.com/spf13/cobra for proper subcommand support. More scalable for future commands (models list, models show, models refresh). But adds dependency. | |
| Simple subcommand in main.go | Add basic subcommand parsing in main.go without cobra. Check os.Args[1] for 'models', then parse sub-flags. Lightweight, no deps. | ✓ |

**User's choice:** Simple subcommand in main.go
**Notes:** 

---

## CLI models list Command

| Option | Description | Selected |
|--------|-------------|----------|
| Table (default) + JSON | Default: human-readable table with columns ID, Provider, ContextLen, Capabilities. Add --format json for machine-readable output. Standard CLI pattern. | ✓ |
| Table only | Just table output. Simpler. Users can pipe to jq if they need JSON but would need to parse table. | |
| JSON only | Only JSON output. Machine-first. Less human-friendly but easier to parse programmatically. | |

**User's choice:** Table (default) + JSON
**Notes:** 

---

## CLI models list Command

| Option | Description | Selected |
|--------|-------------|----------|
| Basic filtering | Support --provider <name> to filter by provider. Keep simple. Other filters can be added later if needed. | ✓ |
| Full filtering/sorting | Support --provider, --capability (tools|reasoning|vision), --sort (context_length|id|provider), --order (asc|desc). More powerful but more complex. | |
| Minimal - just list all | No filtering. User runs models list and gets all models from all providers. Can grep/pipe for filtering. Simplest implementation. | |

**User's choice:** Basic filtering
**Notes:** 

---

## Provider Selection Strategy

| Option | Description | Selected |
|--------|-------------|----------|
| Config default_provider | Add default_provider field to config.toml (e.g., default_provider = "nvidia"). Explicit, user-controlled. Falls back to first available if not set. | |
| First available with valid key | Auto-select first provider in fallback priority that has a valid API key (from env/keychain). No config needed. But less predictable. | |
| Explicit --provider flag + config default | CLI flag --provider overrides config default_provider. If neither set, use first available. Most flexible. | ✓ |

**User's choice:** Explicit --provider flag + config default
**Notes:** 

---

## Provider Selection Strategy

| Option | Description | Selected |
|--------|-------------|----------|
| Per-request in ChatRequest | Add Provider field to ChatRequest. Caller (workflow engine) specifies which provider to use. Allows different providers for different tasks in same session. | |
| Global at startup | Provider selected once at startup (from config/default/flag). All requests use same provider. Simpler but less flexible. | |
| Both - global default + per-request override | Global default_provider in config, but ChatRequest.Provider can override per call. Best of both. | ✓ |

**User's choice:** Both - global default + per-request override
**Notes:** 

---

## Provider Selection Strategy

| Option | Description | Selected |
|--------|-------------|----------|
| Automatic fallback | Registry tries providers in FallbackPriority order on failure. Transparent to caller. But may surprise user if model changes mid-session. | |
| Fail fast, caller handles | Registry returns error immediately. Caller (workflow engine) decides whether to retry with another provider. More explicit, caller controls behavior. | |
| Configurable fallback behavior | Add FallbackMode config: "auto" (try next), "manual" (fail fast), "prompt" (ask user via TUI). Default to "manual" for safety. | ✓ |

**User's choice:** Configurable fallback behavior
**Notes:** 

---

## Provider Selection Strategy

| Option | Description | Selected |
|--------|-------------|----------|
| In registry (centralized) | Registry.GetProviderForRequest(req ChatRequest) handles default, override, fallback logic. Callers just call this method. Single source of truth. | ✓ |
| In workflow engine / caller | Caller resolves provider: if req.Provider != "" use that, else config.default_provider, else first available. Registry just provides GetProvider(name). Keeps registry simple. | |
| Both - registry provides helpers | Registry has GetProvider(name), GetDefaultProvider(), GetFallbackProviders(). Caller composes logic. Best of both. | |

**User's choice:** In registry (centralized)
**Notes:** 

---

## the agent's Discretion

- Exact retry intervals and max attempts for streaming retry strategies
- Default reasoning_budget value for nemotron-3-ultra (suggested 32768)
- Table column layout and formatting for models list command
- Exact FallbackMode default (manual) and prompt wording

## Deferred Ideas

None — discussion stayed within phase scope.
# Phase 2: LLM Provider Abstraction - Research

**Researched:** 2026-08-24
**Domain:** Provider-agnostic LLM interface with NVIDIA Build adapter
**Confidence:** HIGH

## Summary

This phase delivers a clean `LLMProvider` interface with both streaming and non-streaming methods, NVIDIA Nemotron 3 Ultra adapter with reasoning support, model profile configuration, streaming resilience, API-based capability detection, CLI `models list` command, and explicit provider selection strategy. The research confirms the existing codebase provides a solid foundation: a provider registry with lazy initialization, a shared `BaseClient` with SSE parsing and retry logic, NVIDIA client with `extra_body` support, reasoning configuration per model, and capability heuristics. The key gaps are: adding `ChatCompletion` (non-streaming) to the interface, `ChatResponse` type, model profiles in config, streaming retry strategies, API capability enrichment, and CLI command.

**Primary recommendation:** Extend the existing `LLMProvider` interface in `internal/integrations/provider/interface.go` with `ChatCompletion` method and `ChatResponse` type; implement profile merging and retry strategies in `BaseClient`; add Nemotron 3 Ultra reasoning config; enrich `FetchModels` with API metadata; add `m31a models list` CLI command; implement provider selection in registry.

## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Add both ChatCompletion and ChatCompletionStream with shared internals — Reversibility: costly
- **D-02:** New ChatResponse type in internal/core/types/types.go — Reversibility: costly
- **D-03:** ChatCompletion required in LLMProvider interface (all providers must implement) — Reversibility: one-way
- **D-04:** ChatCompletion uses ChatCompletionStream internally and collects chunks — Reversibility: reversible
- **D-05:** Add reasoningParamMap entry for nvidia/nemotron-3-ultra-550b-a55b with enable_thinking=true, force_nonempty_content=true, reasoning_budget configurable — Reversibility: reversible
- **D-06:** Include force_nonempty_content=true in chat_template_kwargs for coding-agent compatibility — Reversibility: reversible
- **D-07:** Configurable reasoning_budget default (32768) with override via config.toml — Reversibility: reversible
- **D-08:** Support both NVIDIA extra_body and OpenAI-style reasoning parameter — Reversibility: reversible
- **D-09:** Both provider defaults + model overrides for profile structure — Reversibility: costly
- **D-10:** Profile merging in BaseClient (shared) — Reversibility: reversible
- **D-11:** Profiles stored in config.toml (layered: global → workspace → project → env) — Reversibility: costly
- **D-12:** Profile scope: standard params (temperature, top_p, max_tokens, reasoning) + reasoning config ref — Reversibility: reversible
- **D-13:** Configurable retry strategy (none / initial_only / full_resume), default initial_only — Reversibility: reversible
- **D-14:** Explicit empty delta handling — skip silently in ParseSSEChunk — Reversibility: reversible
- **D-15:** Sequential chunks sufficient for mixed transitions (reasoning then content) — Reversibility: reversible
- **D-16:** Retry logic in BaseClient (shared) — Reversibility: reversible
- **D-17:** Fetch from API + fallback to heuristics for capability detection — Reversibility: reversible
- **D-18:** Extend ModelInfo with MaxOutputTokens, SupportedParameters, InputModalities, OutputModalities — Reversibility: costly
- **D-19:** Per-provider parsing in each provider's FetchModels — Reversibility: reversible
- **D-20:** Cache capabilities with model list in ModelCache — Reversibility: reversible
- **D-21:** Add m31a models list command with table (default) + JSON output — Reversibility: reversible
- **D-22:** Simple subcommand in main.go (no cobra dependency) — Reversibility: reversible
- **D-23:** Basic filtering (--provider <name>) — Reversibility: reversible
- **D-24:** Explicit --provider flag + config default_provider for selection — Reversibility: reversible
- **D-25:** Both global default + per-request override via ChatRequest.Provider — Reversibility: reversible
- **D-26:** Configurable FallbackMode (auto/manual/prompt), default manual — Reversibility: reversible
- **D-27:** Provider selection logic in registry (centralized GetProviderForRequest) — Reversibility: reversible

### the agent's Discretion
- Exact retry intervals and max attempts for streaming retry strategies
- Default reasoning_budget value for nemotron-3-ultra (suggested 32768)
- Table column layout and formatting for models list command
- Exact FallbackMode default (manual) and prompt wording

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope.

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| LLM-01 | Provider-agnostic interface with ChatCompletion, StreamChatCompletion, ListModels methods | Interface extension in interface.go; BaseClient shared methods |
| LLM-02 | NVIDIA Build adapter implements interface with nvidia/nemotron-3-ultra-550b-a55b, streaming, reasoning (enable_thinking), coding-agent kwargs (force_nonempty_content) | NVIDIA client already exists; add reasoningParamMap entry per D-05/D-06 |
| LLM-03 | Request options (temperature, top_p, max_tokens, reasoning) configurable per provider/model profile | ModelProfile type in types.go; profile merging in BaseClient per D-09/D-10 |
| LLM-04 | Streaming handles empty deltas, reasoning-only chunks, content-only chunks, tool-call deltas, mixed transitions, connection recovery with retry policy | SSE parser exists; add empty delta handling per D-14; retry in BaseClient per D-13/D-16 |
| LLM-05 | Provider credentials from environment (NVIDIA_API_KEY) never written to disk, never in logs, redacted in diagnostics | Keychain integration exists in config/loader.go; APIKey() masking in BaseClient |
| LLM-06 | Model capability detection from API metadata (context length, tool calling, reasoning, streaming) | FetchModels extension per D-17/D-18/D-19/D-20; ModelInfo enrichment |

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|--------------|----------------|-----------|
| LLM Provider Interface | Intelligence Plane (API/Backend) | — | Core abstraction for all LLM interactions; consumed by agents, workflow engine |
| NVIDIA Build Adapter | Intelligence Plane (API/Backend) | — | Provider-specific implementation; translates abstract requests to NVIDIA API |
| Model Profile Configuration | Intelligence Plane (Config) | Memory Plane (Config Loader) | Layered config (global→workspace→project→env) owned by config loader |
| Streaming Resilience | Intelligence Plane (API/Backend) | — | Retry logic in BaseClient shared across providers |
| Capability Detection | Intelligence Plane (API/Backend) | Memory Plane (ModelCache) | API metadata fetch during model list refresh; cached in ModelCache |
| CLI Models List | Interaction Plane (CLI) | Intelligence Plane (Registry) | CLI command calls registry to fetch from all providers |
| Provider Selection | Intelligence Plane (Registry) | — | Centralized GetProviderForRequest in registry; used by workflow engine |

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Go | 1.26.5 | Primary language | Static binaries, cross-compilation, concurrency |
| modernc.org/sqlite | latest | Embedded SQLite | Pure Go, WAL mode, no CGO |
| koanf/v2 | latest | Configuration | Layered providers (TOML/JSON/YAML/env/flags) |
| github.com/eshanized/M31A/internal/integrations/keychain | internal | OS-native credential storage | macOS Keychain, Windows Credential Manager, Linux Secret Service |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| github.com/charmbracelet/bubbletea/v2 | v2.x | TUI framework | Interactive terminal (not used in this phase) |
| github.com/charmbracelet/lipgloss/v2 | v2.x | Terminal styling | CLI table output for models list |
| github.com/charmbracelet/bubbles/v0 | v0.18+ | TUI components | Table rendering for models list |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Custom HTTP client for providers | go-openai (sashabaranov/go-openai) | go-openai lacks extra_body support for NVIDIA-specific params; current custom client handles NVIDIA extra_body natively |
| Cobra CLI framework | Simple flag parsing in main.go | Cobra adds dependency; D-22 mandates simple subcommand |

**Installation:**
```bash
go get github.com/charmbracelet/lipgloss/v2
go get github.com/charmbracelet/bubbles/v0
```

**Version verification:**
```bash
go list -m github.com/charmbracelet/lipgloss/v2
go list -m github.com/charmbracelet/bubbles/v0
```

## Package Legitimacy Audit

> **Required** whenever this phase installs external packages. Run the Package Legitimacy Gate protocol before completing this section.

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| github.com/charmbracelet/lipgloss/v2 | Go modules | 4+ years | High | github.com/charmbracelet/lipgloss | OK | Approved |
| github.com/charmbracelet/bubbles/v0 | Go modules | 3+ years | High | github.com/charmbracelet/bubbles | OK | Approved |

**Packages removed due to [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none

*Packages discovered via WebSearch or training data that have not been verified against an authoritative source are tagged `[ASSUMED]` and the planner must gate each install behind a `checkpoint:human-verify` task.*

## Architecture Patterns

### System Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        CLI / Workflow Engine                            │
└────────────────────────────────┬────────────────────────────────────────┘
                                 │
                                 ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                        Provider Registry                                 │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ GetProviderForRequest(request) → LLMProvider                    │   │
│  │   - Explicit --provider flag                                    │   │
│  │   - ChatRequest.Provider override                               │   │
│  │   - Config default_provider                                     │   │
│  │   - FallbackMode (auto/manual/prompt)                           │   │
│  └─────────────────────────────────────────────────────────────────┘   │
└────────────────────────────────┬────────────────────────────────────────┘
                                 │
              ┌──────────────────┼──────────────────┐
              ▼                  ▼                  ▼
       ┌─────────────┐    ┌─────────────┐    ┌─────────────┐
       │   NVIDIA    │    │  OpenRouter │    │    Zen      │
       │  Provider   │    │  Provider   │    │  Provider   │
       └──────┬──────┘    └──────┬──────┘    └──────┬──────┘
              │                  │                  │
              └──────────────────┼──────────────────┘
                                 │
                                 ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                        BaseClient (Shared)                               │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────┐ │
│  │ ModelCache      │  │ Profile Merging │  │ Streaming Retry         │ │
│  │ - Refresh w/    │  │ - Provider      │  │ - none                  │ │
│  │   capabilities  │  │   defaults      │  │ - initial_only (default)│ │
│  │ - TTL/stale TTL │  │ - Model         │  │ - full_resume           │ │
│  └─────────────────┘  │   overrides     │  └─────────────────────────┘ │
│                       └─────────────────┘                              │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────┐ │
│  │ SSE Parser      │  │ HTTP Clients    │  │ Error Handling          │ │
│  │ - Watchdog      │  │ - HTTPClient    │  │ - Sentinel errors       │ │
│  │ - Buffer pool   │  │   (streaming)   │  │ - Credits mapping       │ │
│  │ - Empty delta   │  │ - CatalogClient │  │ - Context exceeded      │ │
│  │   skip          │  │   (catalog)     │  └─────────────────────────┘ │
│  └─────────────────┘  └─────────────────┘                              │
└────────────────────────────────┬────────────────────────────────────────┘
                                 │
                                 ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                    NVIDIA Build API                                      │
│  POST https://integrate.api.nvidia.com/v1/chat/completions              │
│  - extra_body.chat_template_kwargs.enable_thinking                      │
│  - extra_body.chat_template_kwargs.force_nonempty_content               │
│  - extra_body.reasoning_budget                                          │
│  - Streaming: SSE with reasoning_content delta                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### Recommended Project Structure
```
internal/
├── core/
│   └── types/
│       └── types.go           # ChatRequest, ChatResponse, ModelInfo, StreamChunk, StreamIterator, CapFlags
├── integrations/
│   └── provider/
│       ├── interface.go       # LLMProvider interface (extended with ChatCompletion)
│       ├── base_client.go     # BaseClient with profile merging, retry, SSE
│       ├── registry.go        # Registry with GetProviderForRequest
│       ├── reasoning.go       # ReasoningConfig, reasoningParamMap (add ultra)
│       ├── capabilities.go    # ParseModelCapabilities, API capability enrichment
│       ├── sse.go             # SSEParser with empty delta handling
│       ├── nvidia/
│       │   └── client.go      # NVIDIA client with buildNvidiaBody
│       ├── openrouter/
│       │   └── client.go      # OpenRouter client
│       └── zen/
│           └── client.go      # Zen client
cmd/
└── m31a/
    └── main.go                # CLI commands including "models list"
```

### Pattern 1: Provider-Agnostic Interface with Capability Metadata
**What:** `LLMProvider` interface defines abstract methods; each provider implements translation to its API. Capability metadata comes from API first, heuristics as fallback.
**When to use:** All LLM interactions in M31A — agents, workflow engine, CLI.
**Example:**
```go
// internal/integrations/provider/interface.go
type LLMProvider interface {
    Name() string
    APIKey() string
    FetchModels(ctx context.Context) ([]types.ModelInfo, error)
    CachedModels() []types.ModelInfo
    ChatCompletion(ctx context.Context, req ChatRequest) (*types.ChatResponse, error)
    ChatCompletionStream(ctx context.Context, req ChatRequest) (*types.StreamIterator, error)
    EstimateCost(modelID string, usage types.Usage) float64
    HealthCheck(ctx context.Context) types.HealthStatus
    GetModel(id string) (*types.ModelInfo, error)
}
```
**Source:** [CONTEXT_M31A.md §34](https://github.com/eshanized/M31A/blob/main/.planning/CONTEXT_M31A.md) — NVIDIA Build integration; [PITFALLS.md Pitfall 10](https://github.com/eshanized/M31A/blob/main/.planning/research/PITFALLS.md) — Provider abstraction leak prevention

### Pattern 2: Shared BaseClient with Profile Merging and Retry
**What:** `BaseClient` embeds in all providers; handles model caching, profile merging, streaming retry, SSE parsing, error handling.
**When to use:** All provider implementations.
**Example:**
```go
// internal/integrations/provider/base_client.go
type BaseClient struct {
    APIKeyField   string
    BaseURLField  string
    HTTPClient    *http.Client
    CatalogClient *http.Client
    Cache         *ModelCache
    HealthLiveMs  int64
    HealthSlowMs  int64
    Version       string
    // New fields for Phase 2:
    Profiles      *ModelProfiles
    RetryConfig   StreamRetryConfig
}

func (b *BaseClient) MergeProfile(req ChatRequest) ChatRequest {
    // 1. Start with provider defaults
    // 2. Apply model-specific profile overrides
    // 3. Apply request-level values (highest precedence)
    return mergedReq
}
```
**Source:** Existing `base_client.go` — `NewBaseClient`, `ModelCache`, `HealthCheck`, `HandleChatHTTPError`

### Pattern 3: Reasoning Configuration Per Model
**What:** `reasoningParamMap` maps model ID prefixes to `ReasoningConfig` with `RequestParams`, `ExtraBodyParams`, `SSEField`. NVIDIA uses `extra_body.chat_template_kwargs`.
**When to use:** Any model requiring reasoning/thinking mode or provider-specific parameters.
**Example:**
```go
// internal/integrations/provider/reasoning.go
var reasoningParamMap = map[string]ReasoningConfig{
    "nvidia/nemotron-3-ultra-550b-a55b": {
        ModelFamily: "nvidia",
        RequestParams: map[string]any{},
        ExtraBodyParams: map[string]any{
            "reasoning_budget": 32768,
            "chat_template_kwargs": map[string]any{
                "enable_thinking":       true,
                "force_nonempty_content": true,
            },
        },
        SSEField: "choices.0.delta.reasoning_content",
    },
    // ... existing entries
}
```
**Source:** Existing `reasoning.go` — `reasoningParamMap`, `GetReasoningConfig`, `ApplyReasoningParams`; [NVIDIA Build API docs](https://docs.api.nvidia.com/nim/reference/nvidia-nemotron-3-ultra-550b-a55b) — `extra_body.chat_template_kwargs.enable_thinking`, `force_nonempty_content`, `reasoning_budget`

### Pattern 4: SSE Parser with Watchdog and Empty Delta Handling
**What:** `SSEParser` reads streaming response, handles watchdog timeout, buffers, empty deltas, `[DONE]` termination.
**When to use:** All streaming chat completions.
**Example:**
```go
// internal/integrations/provider/sse.go
func (p *SSEParser) Next() (eventType string, data string, err error) {
    for {
        // ... scan lines ...
        if strings.TrimSpace(data) == "" {
            continue // D-14: skip empty deltas silently
        }
        return eventType, data, nil
    }
}
```
**Source:** Existing `sse.go` — `NewSSEParserWithContext`, `SSEParser.Next`, `DefaultStreamTimeout = 30*time.Second`

### Anti-Patterns to Avoid
- **Provider leakage:** Never put `chat_template_kwargs`, `enable_thinking`, `force_nonempty_content` in generic code — owner is provider adapter (Pitfall 10)
- **Heuristic-only capabilities:** Always fetch from API first; `ParseModelCapabilities` is fallback only (Pitfall 18)
- **TUI owning provider state:** Provider selection and caching in registry/BaseClient, not TUI (Pitfall 14)
- **Interface assuming OpenAI shape:** `LLMProvider` must model capabilities abstractly (`SupportsTools()`, `GetContextWindow()`) not concretely

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| SSE parsing with timeout/buffering | Custom scanner loop | `provider.SSEParser` (existing) | Watchdog, buffer pool, context cancellation, `\r` handling already implemented |
| Model capability heuristics | New string matching logic | `provider.ParseModelCapabilities` (existing) | Built-in patterns for reasoning, tools, vision, chat; config-extensible |
| Provider credential storage | File-based or env-only | `keychain.Keychain` (existing) | OS-native (Keychain/Credential Manager/Secret Service); cached with blacklist TTL |
| Layered config (global→workspace→project→env) | Custom merge logic | `config.Load` (existing) | TOML/JSON/env/flags with variable substitution, validation, unknown key warnings |
| Streaming retry with backoff | Ad-hoc retry loops | `BaseClient` shared retry (to be extended per D-13/D-16) | Centralized, configurable strategy (none/initial_only/full_resume) |
| HTTP transport/connection pooling | Per-provider clients | `getSharedTransport()` (existing) | Single transport reuses pools, TLS sessions, idle conns across providers |

**Key insight:** The existing provider infrastructure (BaseClient, SSEParser, ModelCache, reasoning.go, capabilities.go, registry.go) already solves the hard problems. Phase 2 extends these rather than replacing them.

## Common Pitfalls

### Pitfall 1: Provider-Specific Logic Leaking into Generic Code
**What goes wrong:** `chat_template_kwargs`, `enable_thinking`, `force_nonempty_content`, `reasoning_budget` appear in workflow engine, agent code, or ChatRequest instead of staying in NVIDIA client.
**Why it happens:** First provider (OpenRouter) sets implicit contract; subsequent providers forced into same shape.
**How to avoid:** All provider-specific request/response translation in provider package. `LLMProvider` interface models capabilities abstractly. `buildNvidiaBody` owns NVIDIA details.
**Warning signs:** Grep finds `chat_template_kwargs` outside `internal/integrations/provider/nvidia/`

### Pitfall 2: Capability Detection via Heuristics Instead of API Metadata
**What goes wrong:** `ParseModelCapabilities` string matching on model IDs (e.g., "r1" → reasoning). New models break detection.
**Why it happens:** Quick heuristic works for known models; API metadata parsing deferred.
**How to avoid:** Extend `FetchModels` per provider to parse API metadata into `ModelInfo.MaxOutputTokens`, `SupportedParameters`, `InputModalities`, `OutputModalities`. Heuristics only as fallback.
**Warning signs:** `ParseModelCapabilities` called without API enrichment; `ModelInfo` missing capability fields

### Pitfall 3: Streaming Retry Blocking UI Thread
**What goes wrong:** Retry logic in `ChatCompletionStream` blocks goroutine; Bubble Tea `Update()` cannot process messages.
**Why it happens:** Retry implemented as synchronous loop with `time.Sleep`.
**How to avoid:** Retry in `BaseClient` returns `StreamIterator` that handles retry internally via `Next()`; or use `tea.Cmd` for async retry with `tea.Msg` completion.
**Warning signs:** `time.Sleep` in streaming path; no `tea.Cmd` for async operations

### Pitfall 4: Empty Deltas Causing Parse Errors
**What goes wrong:** Provider sends SSE chunks with empty `delta.content` or `delta.reasoning_content`; JSON unmarshal fails or produces empty chunks.
**Why it happens:** Not all chunks contain content; some are keep-alive or transition markers.
**How to avoid:** `ParseSSEChunk` returns `nil, nil` for empty deltas (D-14); `StreamIterator.Next()` loops until non-nil chunk or EOF.
**Warning signs:** `StreamChunk` with empty `Delta` and no `Usage` reaching TUI

### Pitfall 5: Mixed Reasoning/Content Transitions Not Handled
**What goes wrong:** Stream sends reasoning chunks then content chunks (or interleaved); parser treats as single type.
**Why it happens:** Assumption that all chunks are same type.
**How to avoid:** `ParseSSEChunk` inspects `delta.reasoning_content` vs `delta.content` per chunk; emits `Type: "thinking"` or `"content"` accordingly (D-15: sequential chunks sufficient).
**Warning signs:** Reasoning text appears in content stream or vice versa

## Code Examples

Verified patterns from official sources and existing codebase:

### NVIDIA Build API Request with Reasoning and Coding-Agent Kwargs
```python
# Source: https://docs.api.nvidia.com/nim/reference/nvidia-nemotron-3-ultra-550b-a55b
response = client.chat.completions.create(
    model="nvidia/nemotron-3-ultra-550b-a55b",
    messages=[{"role": "user", "content": "Write a Go HTTP server"}],
    tools=[{"type": "function", "function": {"name": "write_file", ...}}],
    tool_choice="required",
    temperature=1.0,
    top_p=0.95,
    max_tokens=16384,
    extra_body={
        "chat_template_kwargs": {
            "enable_thinking": True,
            "force_nonempty_content": True
        },
        "reasoning_budget": 32768
    }
)
```

### NVIDIA Client Request Body Building (Existing Pattern)
```go
// Source: internal/integrations/provider/nvidia/client.go:183-213
func (c *Client) buildNvidiaBody(req provider.ChatRequest) map[string]any {
    body := provider.BuildChatBody(req)

    cfg, hasCfg := provider.GetReasoningConfig(req.Model)
    if hasCfg && len(cfg.ExtraBodyParams) > 0 {
        extraBody, _ := body["extra_body"].(map[string]any)
        if extraBody == nil {
            extraBody = make(map[string]any, len(cfg.ExtraBodyParams))
        }
        for k, v := range cfg.ExtraBodyParams {
            extraBody[k] = v
        }
        body["extra_body"] = extraBody
    }
    return body
}
```

### SSE Chunk Parsing with Reasoning Content (Existing Pattern)
```go
// Source: internal/integrations/provider/reasoning.go:229-240
if cfg.SSEField != "" {
    parts := cfg.SSEFieldParts // pre-split: ["choices", "0", "delta", "reasoning_content"]
    if len(parts) >= 4 {
        fieldName := parts[len(parts)-1] // "reasoning_content"
        if val, exists := deltaMap[fieldName]; exists {
            if str, ok := val.(string); ok && str != "" {
                return &types.StreamChunk{Type: "thinking", Delta: str, Usage: usage}, nil
            }
        }
    }
}
```

### Model Capability Heuristics (Existing Pattern)
```go
// Source: internal/integrations/provider/capabilities.go:154-228
func ParseModelCapabilities(modelID string, extraReasoningPatterns ...string) types.CapFlags {
    id := strings.ToLower(modelID)
    caps := types.CapFlags{Chat: true}
    // Tool capability
    for _, p := range toolCapablePatterns { if strings.Contains(id, p) { caps.Tools = true; break } }
    // Reasoning detection
    for _, p := range patterns { if strings.Contains(id, p) { caps.Reasoning = true; break } }
    // Vision detection
    if strings.Contains(id, "vision") || strings.Contains(id, "multimodal") { caps.Vision = true }
    return caps
}
```

### CLI Table Output with lipgloss (New for Phase 2)
```go
// Source: github.com/charmbracelet/lipgloss/v2 + bubbles/v0 table
import (
    "github.com/charmbracelet/bubbles/v0/table"
    "github.com/charmbracelet/lipgloss/v2"
)

func renderModelsTable(models []types.ModelInfo) string {
    columns := []table.Column{
        {Title: "ID", Width: 50},
        {Title: "Provider", Width: 12},
        {Title: "Context", Width: 10},
        {Title: "Tools", Width: 6},
        {Title: "Reasoning", Width: 10},
        {Title: "Vision", Width: 6},
    }
    rows := make([]table.Row, len(models))
    for i, m := range models {
        rows[i] = table.Row{
            m.ID, m.Provider, fmt.Sprintf("%dk", m.ContextLength/1000),
            boolStr(m.Capabilities.Tools), boolStr(m.Capabilities.Reasoning), boolStr(m.Capabilities.Vision),
        }
    }
    t := table.New(table.WithColumns(columns), table.WithRows(rows), table.WithFocused(true))
    return t.View()
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Single `ChatCompletionStream` only | Both `ChatCompletion` + `ChatCompletionStream` (D-01/D-03) | Phase 2 | Non-streaming callers (tests, simple queries) don't need stream handling |
| Heuristic-only capabilities | API metadata + heuristic fallback (D-17) | Phase 2 | Accurate context windows, tool support, modalities for new models |
| Fixed retry (2 attempts) | Configurable strategy: none/initial_only/full_resume (D-13) | Phase 2 | Production resilience; initial_only balances UX and cost |
| Model params hardcoded | ModelProfile with layered config (D-09/D-11) | Phase 2 | Per-model/project tuning without code changes |
| Provider selection implicit | Explicit `--provider` + `ChatRequest.Provider` + config default (D-24/D-25) | Phase 2 | Deterministic routing; supports multi-model future |

**Deprecated/outdated:**
- `ChatRequest` without `Provider` field — add per D-25
- `ModelInfo` without `MaxOutputTokens`, `SupportedParameters`, `InputModalities`, `OutputModalities` — add per D-18
- `reasoningParamMap` without `nvidia/nemotron-3-ultra-550b-a55b` entry — add per D-05
- `BaseClient` without `Profiles` and `RetryConfig` — add per D-10/D-13

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | NVIDIA Build API accepts `extra_body.reasoning_budget` at top level (not nested in chat_template_kwargs) | Standard Stack, Pattern 3 | If nested differently, NVIDIA client `buildNvidiaBody` needs adjustment |
| A2 | `reasoning_budget` max is 32768 for Nemotron 3 Ultra | D-07, Pattern 3 | If different, config default needs update |
| A3 | OpenRouter and Zen providers don't need reasoning config entries | Pattern 3 | If they add reasoning models, entries needed |
| A4 | `ChatCompletion` collecting stream chunks internally is performant enough | D-04 | If latency-critical, may need separate non-streaming endpoint |
| A5 | `lipgloss` table rendering works for `models list` without full TUI | Standard Stack | If requires Bubble Tea runtime, need simpler table implementation |

## Open Questions (RESOLVED)

1. **NVIDIA API `reasoning_budget` nesting** — **RESOLVED**
   - What we know: Examples show `extra_body.reasoning_budget` at top level and `chat_template_kwargs.reasoning_budget` nested
   - What's unclear: Which takes precedence; whether both are needed
   - Recommendation: Include both in `ExtraBodyParams`; test against live API
   - **Resolution (Plan 01, Task 3):** Both top-level `reasoning_budget` and nested `chat_template_kwargs.reasoning_budget` included in `ExtraBodyParams` for NVIDIA ultra model. NVIDIA client `buildNvidiaBody` applies all `ExtraBodyParams` keys to `extra_body`.

2. **Streaming retry `full_resume` semantics** — **RESOLVED**
   - What we know: D-13 defines three modes; `initial_only` retries only first chunk failure
   - What's unclear: `full_resume` — does it resume from last token or restart request?
   - Recommendation: Implement `initial_only` first (default); `full_resume` as future enhancement
   - **Resolution (Plan 03, Task 2):** `full_resume` mode implemented as request restart with exponential backoff (not token-level resume). `StreamRetryConfig` has `Mode` field: `none` | `initial_only` | `full_resume`. `initial_only` is default. `full_resume` restarts the entire request with preserved context.

3. **Model profile merging precedence** — **RESOLVED**
   - What we know: D-09/D-10: provider defaults → model overrides → request values
   - What's unclear: How to handle `ReasoningEnabled` bool vs `ReasoningBudget` int when only one set
   - Recommendation: Explicit `ReasoningConfigRef` in profile points to named reasoning config; merging logic in `BaseClient.MergeProfile`
   - **Resolution (Plan 02, Task 2):** `ModelProfile` includes `ReasoningConfigRef` (string, optional) pointing to a named entry in `reasoningParamMap`. `BaseClient.MergeProfile` merges: provider defaults → model profile overrides (including `ReasoningConfigRef` if set) → request values. If `ReasoningConfigRef` set, it overrides individual reasoning params.

4. **CLI `models list` output format for JSON** — **RESOLVED**
   - What we know: D-21: table (default) + JSON output
   - What's unclear: Whether JSON includes full `ModelInfo` or subset
   - Recommendation: JSON outputs full `ModelInfo` (all fields); table shows summary columns
   - **Resolution (Plan 04, Task 2):** JSON output serializes full `ModelInfo` (all fields including extended capability fields). Table output shows summary columns: ID, Provider, ContextLen, Tools, Reasoning, Vision.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|-------------|-----------|---------|----------|
| Go toolchain | Build | ✓ | 1.26.5 | — |
| NVIDIA_API_KEY env var | NVIDIA provider auth | ? | — | Keychain lookup (OS native) |
| lipgloss v2 | CLI table rendering | — | — | Simple text table (no colors) |
| bubbles v0 table | CLI table rendering | — | — | Simple text table |

**Missing dependencies with no fallback:** NVIDIA_API_KEY (required for live testing; mock in unit tests)
**Missing dependencies with fallback:** lipgloss/bubbles — simple text table if not available

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Go testing (stdlib) + testify/assert |
| Config file | None — `go test ./...` |
| Quick run command | `make test-fast` |
| Full suite command | `make test` (race-enabled with coverage) |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|--------------|
| LLM-01 | Interface has ChatCompletion, ChatCompletionStream, ListModels | unit | `go test ./internal/integrations/provider/... -run TestLLMProviderInterface` | ❌ Wave 0 |
| LLM-02 | NVIDIA client implements interface with ultra model, streaming, reasoning | integration | `go test ./internal/integrations/provider/nvidia/... -run TestNVIDIAClient` | ❌ Wave 0 |
| LLM-03 | ModelProfile merging applies provider defaults → model overrides → request | unit | `go test ./internal/integrations/provider/... -run TestProfileMerging` | ❌ Wave 0 |
| LLM-04 | Streaming handles empty deltas, reasoning chunks, retry policy | unit | `go test ./internal/integrations/provider/... -run TestStreamingResilience` | ❌ Wave 0 |
| LLM-05 | API keys from env/keychain; never in logs; redacted in diagnostics | unit | `go test ./internal/core/config/... -run TestCredentialResolution` | ❌ Wave 0 |
| LLM-06 | FetchModels enriches ModelInfo with API metadata (context, tools, reasoning) | integration | `go test ./internal/integrations/provider/... -run TestCapabilityDetection` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `make test-fast`
- **Per wave merge:** `make test`
- **Phase gate:** `make check` (fmt → tidy → vet → lint → test) green before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] `internal/integrations/provider/interface_test.go` — covers LLM-01
- [ ] `internal/integrations/provider/nvidia/client_test.go` — covers LLM-02
- [ ] `internal/integrations/provider/base_client_test.go` — covers LLM-03 (profile merging)
- [ ] `internal/integrations/provider/sse_test.go` — covers LLM-04 (streaming resilience)
- [ ] `internal/core/config/loader_test.go` — covers LLM-05 (credential resolution)
- [ ] `internal/integrations/provider/capabilities_test.go` — covers LLM-06 (capability detection)
- [ ] `internal/core/types/types_test.go` — ChatResponse, ModelInfo extensions

*(If no gaps: "None — existing test infrastructure covers all phase requirements")*

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | yes | API keys via OS keychain; env var fallback; never in config file |
| V3 Session Management | no | N/A (provider sessions stateless) |
| V4 Access Control | yes | Capability-based permissions for tool execution (Phase 3) |
| V5 Input Validation | yes | `ValidateTaskFiles()` central path validation; shell AST parsing |
| V6 Cryptography | no | TLS via HTTPS; no custom crypto |
| V7 Error Handling | yes | Sentinel errors; sanitized provider errors; no secret leakage in errors |
| V8 Logging | yes | Structured slog; API key masking (`****xxxx`); secret redaction in tool output |
| V9 Communication Security | yes | HTTPS only; `CGO_ENABLED=0` static binary; shared transport with TLS config |
| V10 Malicious Code | no | N/A (provider is external service) |
| V11 Business Logic | yes | Provider selection fallback logic; retry bounds; rate limiting |
| V12 File/Resources | no | N/A |
| V13 API Security | yes | Provider API keys never logged; request/response sanitization |
| V14 Configuration | yes | Layered config with validation; keychain for secrets |

### Known Threat Patterns for NVIDIA Build / OpenAI-compatible API

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| API key exfiltration via logs | Information Disclosure | `BaseClient.APIKey()` masks key; `config.SaveWithKeychain` stores in OS keychain; slog redaction |
| Prompt injection in user messages | Tampering | User messages passed through; no template interpolation; provider handles safety |
| Malicious model response (oversized SSE) | DoS | `sseMaxLineLength = 1MB`; `MaxLLMResponseBytes` limit on HTTP body read |
| Connection exhaustion via streaming | DoS | `DefaultStreamTimeout = 30s` watchdog; shared transport `MaxIdleConns=100` |
| Model capability spoofing (heuristic) | Spoofing | API metadata primary; heuristics fallback; `ModelCache` TTL limits staleness |
| Credential fallback to plaintext config | Information Disclosure | `SaveWithKeychain` warns; config option to disable plaintext fallback (Pitfall 23) |

## Sources

### Primary (HIGH confidence)
- [NVIDIA Build API — Nemotron 3 Ultra](https://docs.api.nvidia.com/nim/reference/nvidia-nemotron-3-ultra-550b-a55b) — `extra_body.chat_template_kwargs.enable_thinking`, `force_nonempty_content`, `reasoning_budget`, tool calling
- [NVIDIA Build API — Chat Completions endpoint](https://docs.api.nvidia.com/nim/reference/nvidia-nemotron-3-super-120b-a12b-infer) — POST `/v1/chat/completions`, streaming SSE, `reasoning_effort`, `reasoning_budget`
- Existing codebase: `internal/integrations/provider/*.go` — BaseClient, SSEParser, ModelCache, reasoning.go, capabilities.go, registry.go, nvidia/client.go
- [CONTEXT_M31A.md §34](https://github.com/eshanized/M31A/blob/main/.planning/CONTEXT_M31A.md) — NVIDIA Build integration specification
- [PITFALLS.md Pitfall 10](https://github.com/eshanized/M31A/blob/main/.planning/research/PITFALLS.md) — Provider abstraction leak prevention

### Secondary (MEDIUM confidence)
- [sashabaranov/go-openai](https://github.com/sashabaranov/go-openai) — Streaming pattern, ChatCompletionRequest structure (no extra_body support)
- [OpenAI API Streaming](https://platform.openai.com/docs/api-reference/chat/streaming) — SSE format: `data: {...}`, `data: [DONE]`, `choices[0].delta.content`, `choices[0].delta.tool_calls`

### Tertiary (LOW confidence)
- Provider-agnostic interface patterns from OpenCode/GSD — architectural reference only

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — Existing codebase patterns verified; NVIDIA API docs from Context7 (authoritative)
- Architecture: HIGH — Six-plane model from CONTEXT_M31A.md; existing provider infrastructure analyzed
- Pitfalls: HIGH — Sourced from forensic audit (AUDIT_REPORT.md) with reproduced P0/P1 findings
- Code examples: HIGH — Quoted verbatim from existing source files and NVIDIA official docs

**Research date:** 2026-08-24
**Valid until:** 2026-09-23 (30 days for stable; NVIDIA Build API may evolve)


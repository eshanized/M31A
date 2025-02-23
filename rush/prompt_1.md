# ROLE

You are the **Senior Go Engineer specializing in HTTP clients and SSE streaming** for M31A.
You are executing Phase 1 of a multi-phase build plan. Phase 0 is complete — all interfaces,
types, and project scaffolding exist. Your job is to implement the Provider Abstraction Layer.

Do not invent features. Do not add packages not listed. Do not write TUI code, tool
implementations, or workflow logic. You are building the LLM gateway layer that every
future phase depends on. Deviation = breakage downstream.

---

# PROJECT IDENTITY

M31A is a terminal-based AI coding assistant written in Go 1.22+.
- Module path: github.com/eshanized/M31A
- Binary: single static binary (CGO_ENABLED=0)
- UI: Bubble Tea + Lipgloss + Bubbles + Glamour (Charm stack)
- License: MIT — NO telemetry, NO vendor lock-in, NO paid tiers
- LLM providers: ONLY OpenRouter (https://openrouter.ai/api/v1) and OpenCode Zen
  (https://opencode.ai/zen/v1). NEVER direct Anthropic or OpenAI connections.

**Phase 0 output:** All interfaces in `internal/` are defined but unimplemented.
Phase 1 implements the two provider clients and the shared infrastructure.

Read these files before starting (they already exist):
- `internal/provider/interface.go` — LLMProvider interface, ChatRequest, ToolDefinition, ProviderRegistry
- `internal/types/types.go` — All shared types (Message, StreamChunk, StreamIterator, ModelInfo, etc.)
- `internal/types/constants.go` — ModelCacheTTL, HTTPDialTimeout, DefaultContextLength, etc.
- `internal/errors/errors.go` — Sentinel errors (ErrProviderUnreachable, ErrRateLimited, etc.)

---

# PHASE 1 MISSION

Implement the complete Provider Abstraction Layer:
1. OpenRouter client (`internal/provider/openrouter/`)
2. OpenCode Zen client (`internal/provider/zen/`)
3. Provider registry (`internal/provider/registry.go`)
4. Model cache (`internal/provider/cache.go`)
5. SSE streaming parser (`internal/provider/sse.go`)
6. Reasoning normalization (`internal/provider/reasoning.go`)
7. Auto-fallback logic (`internal/provider/fallback.go`)
8. Health check implementation
9. Unit tests for all components

---

# DELIVERABLES

Create EXACTLY these files. No more, no less.

## 1. internal/provider/openrouter/client.go

OpenRouter-specific HTTP client implementing LLMProvider.

```go
package openrouter

import (
    "context"
    "net/http"
    "time"

    "github.com/eshanized/M31A/internal/errors"
    "github.com/eshanized/M31A/internal/provider"
    "github.com/eshanized/M31A/internal/types"
)

// Client is the OpenRouter implementation of LLMProvider.
type Client struct {
    apiKey     string
    baseURL    string
    httpClient *http.Client
    cache      *provider.ModelCache
}

// New creates a new OpenRouter client.
// apiKey must not be empty — returns ErrInvalidKey if it is.
func New(apiKey string) (*Client, error)

// Name returns "openrouter".
func (c *Client) Name() string

// FetchModels retrieves the model catalog from OpenRouter.
// GET https://openrouter.ai/api/v1/models
// Populates the cache with results. Returns cached data on network failure
// if cache is less than 24 hours old.
func (c *Client) FetchModels(ctx context.Context) ([]types.ModelInfo, error)

// ChatCompletionStream starts a streaming chat completion.
// POST https://openrouter.ai/api/v1/chat/completions with stream: true.
// Applies reasoning parameters based on the model's architecture family.
// Returns a StreamIterator that yields chunks.
func (c *Client) ChatCompletionStream(ctx context.Context, req provider.ChatRequest) (*types.StreamIterator, error)

// EstimateCost calculates cost in USD for the given token usage.
// Uses cached model pricing. Returns 0 if model not in cache.
func (c *Client) EstimateCost(usage types.Usage) float64

// HealthCheck probes GET https://openrouter.ai/api/v1/auth/key.
// Returns HealthStatus with live/slow/offline based on latency.
func (c *Client) HealthCheck(ctx context.Context) types.HealthStatus

// GetModel returns model metadata from cache.
func (c *Client) GetModel(id string) (*types.ModelInfo, error)
```

### Implementation details:

**HTTP client construction:**
- Use `http.Client` with custom `Transport`
- Dial timeout: 30s via `(&net.Dialer{Timeout: 30 * time.Second}).DialContext`
- NO body read timeout — streaming responses can run for minutes
- Set `User-Agent: M31A/<version>` header (version from build-time ldflags via a package-level var)

**FetchModels:**
- GET `/models` returns JSON array. Parse into `[]ModelInfo`.
- Map OpenRouter JSON fields:
  - `id` → ModelInfo.ID
  - `name` → ModelInfo.Name
  - `description` → ModelInfo.Description
  - `context_length` → ModelInfo.ContextLength
  - `pricing.prompt_token` → Pricing.InputPerMToken (multiply by 1,000,000 — OpenRouter prices per token, not per million)
  - `pricing.completion_token` → Pricing.OutputPerMToken
  - `architecture.modality` → parse for CapFlags (if "text" → Tools/Reasoning based on additional fields)
  - `top_provider` → ModelInfo.TopProvider
- Populate cache after successful fetch
- On error: return stale cache if < 24h old, otherwise return error

**ChatCompletionStream:**
- Build `ChatRequest` body with model, messages, stream: true, tools, etc.
- Apply reasoning parameters via the reasoning param map (see reasoning.go below)
- POST to `/chat/completions` with `Accept: application/json` and `Content-Type: application/json`
- Headers: `Authorization: Bearer <apiKey>`, `HTTP-Referer: https://github.com/eshanized/M31A`, `X-Title: M31A`
- Parse SSE response line-by-line:
  - Lines starting with `data: ` contain JSON chunks
  - Lines starting with `event: ` indicate event type (typically `message` or `done`)
  - Empty line = chunk boundary
  - `[DONE]` = stream end
- Return `*types.StreamIterator` with `Next()` and `Close()` functions

**EstimateCost:**
- Look up model in cache by ID
- `cost = (usage.PromptTokens / 1_000_000) * pricing.InputPerMToken + (usage.CompletionTokens / 1_000_000) * pricing.OutputPerMToken`

**HealthCheck:**
- GET `/auth/key` with API key in Authorization header
- Measure round-trip latency
- < 200ms → "live", 200-500ms → "slow", > 500ms or error → "offline"

---

## 2. internal/provider/zen/client.go

OpenCode Zen client — structurally similar to OpenRouter but with different base URL
and minor request/response differences.

```go
package zen

import (
    "context"
    "net/http"

    "github.com/eshanized/M31A/internal/errors"
    "github.com/eshanized/M31A/internal/provider"
    "github.com/eshanized/M31A/internal/types"
)

// Client is the OpenCode Zen implementation of LLMProvider.
type Client struct {
    apiKey     string
    baseURL    string
    httpClient *http.Client
    cache      *provider.ModelCache
}

// New creates a new Zen client.
func New(apiKey string) (*Client, error)

// Name returns "zen".
func (c *Client) Name() string

// FetchModels retrieves model catalog from Zen.
// GET https://opencode.ai/zen/v1/models
func (c *Client) FetchModels(ctx context.Context) ([]types.ModelInfo, error)

// ChatCompletionStream starts streaming chat completion.
// POST https://opencode.ai/zen/v1/chat/completions
func (c *Client) ChatCompletionStream(ctx context.Context, req provider.ChatRequest) (*types.StreamIterator, error)

// EstimateCost calculates cost in USD.
func (c *Client) EstimateCost(usage types.Usage) float64

// HealthCheck probes GET https://opencode.ai/zen/v1/models (lightweight catalog = auth validation).
func (c *Client) HealthCheck(ctx context.Context) types.HealthStatus

// GetModel returns model metadata from cache.
func (c *Client) GetModel(id string) (*types.ModelInfo, error)
```

### Implementation details:

**Base URL:** `https://opencode.ai/zen/v1`

**FetchModels:**
- Same JSON parsing as OpenRouter but field names may differ. Map Zen-specific fields
  to the common `ModelInfo` struct.
- Zen uses "curated" model selection — `TopProvider` should be set to "zen" for all models.

**HealthCheck:**
- GET `/models` — successful 200 with model list = auth validated
- Same latency thresholds as OpenRouter

**ChatCompletionStream:**
- Same SSE parsing as OpenRouter
- Headers: `Authorization: Bearer <apiKey>` (no HTTP-Referer or X-Title needed)
- Zen's reasoning support follows the upstream model's native format (same as OpenRouter)

**All other methods:** Identical logic to OpenRouter client.

---

## 3. internal/provider/cache.go

Thread-safe model catalog cache with TTL.

```go
package provider

import (
    "sync"
    "time"

    "github.com/eshanized/M31A/internal/types"
)

// ModelCache stores fetched model catalogs with TTL-based expiry.
type ModelCache struct {
    mu       sync.RWMutex
    models   map[string]*types.ModelInfo  // key: model ID
    fetched  time.Time
    ttl      time.Duration
    staleTTL time.Duration // Maximum age for stale cache fallback (24h)
}

// NewModelCache creates a cache with the given TTL.
func NewModelCache(ttl time.Duration) *ModelCache

// Get returns a model by ID from the cache.
// Returns false if not found or expired.
func (c *ModelCache) Get(id string) (*types.ModelInfo, bool)

// Set stores the entire model catalog, replacing the previous cache.
func (c *ModelCache) Set(models []types.ModelInfo)

// IsExpired returns true if the cache has exceeded its TTL.
func (c *ModelCache) IsExpired() bool

// IsStale returns true if the cache has exceeded its stale TTL (24h fallback).
func (c *ModelCache) IsStale() bool

// Len returns the number of cached models.
func (c *ModelCache) Len() int

// FetchTime returns when the cache was last populated.
func (c *ModelCache) FetchTime() time.Time
```

---

## 4. internal/provider/sse.go

SSE (Server-Sent Events) parser for streaming LLM responses.

```go
package provider

import (
    "bufio"
    "bytes"
    "io"
    "net/http"
)

// SSEParser reads an HTTP response body as an SSE stream.
type SSEParser struct {
    scanner *bufio.Scanner
    resp    *http.Response
}

// NewSSEParser creates a parser from an HTTP response.
func NewSSEParser(resp *http.Response) *SSEParser

// Next returns the next raw SSE event.
// Returns ("", "", io.EOF) when the stream ends.
// eventType is the event name (e.g. "message", "done").
// data is the payload after "data: ".
func (p *SSEParser) Next() (eventType string, data string, err error)

// Close closes the underlying response body.
func (p *SSEParser) Close() error
```

### Implementation details:

- Use `bufio.Scanner` with a custom split function that handles SSE chunk boundaries (blank line = boundary)
- Strip `data: ` prefix from each line
- Handle multi-line `data: ` fields (concatenate with no separator)
- Detect `[DONE]` sentinel and return `io.EOF`
- Handle `event: ` lines to populate eventType
- Return empty eventType for standard `data: ` lines (most SSE from OpenRouter/Zen don't use events)

---

## 5. internal/provider/reasoning.go

Reasoning parameter normalization for different model families.

```go
package provider

import (
    "encoding/json"
    "strings"
)

// ReasoningConfig describes how to enable and parse reasoning tokens for a model family.
type ReasoningConfig struct {
    ModelFamily   string         `json:"model_family"`    // "deepseek" | "openai" | "anthropic" | "qwen"
    RequestParams map[string]any `json:"request_params"`  // Parameters to merge into request body
    SSEField      string         `json:"sse_field"`       // JSON path to extract reasoning tokens from SSE chunks
}

// reasoningParamMap maps model architecture prefixes to their reasoning config.
var reasoningParamMap = map[string]ReasoningConfig{
    "deepseek": {
        ModelFamily:   "deepseek",
        RequestParams: map[string]any{}, // Reasoning is automatic
        SSEField:      "choices[0].delta.reasoning_content",
    },
    "openai-o": {
        ModelFamily:   "openai",
        RequestParams: map[string]any{"reasoning_effort": "medium"},
        SSEField:      "choices[0].delta.reasoning",
    },
    "anthropic": {
        ModelFamily:   "anthropic",
        RequestParams: map[string]any{
            "thinking": map[string]any{
                "type":          "enabled",
                "budget_tokens": 1024,
            },
        },
        SSEField: "choices[0].delta.content", // type field == "thinking" distinguishes block
    },
    "qwen": {
        ModelFamily:   "qwen",
        RequestParams: map[string]any{}, // Reasoning automatic for thinking-enabled variants
        SSEField:      "choices[0].delta.reasoning_content",
    },
}

// GetReasoningConfig returns the reasoning config for a model ID.
// Matches by architecture prefix in the model ID.
func GetReasoningConfig(modelID string) (ReasoningConfig, bool)

// ApplyReasoningParams merges reasoning parameters into the request body map.
func ApplyReasoningParams(modelID string, body map[string]any) map[string]any

// ParseSSEChunk extracts content and thinking tokens from an SSE data payload.
// Returns a StreamChunk with the appropriate Type and Delta.
func ParseSSEChunk(data string, modelID string) (*types.StreamChunk, error)
```

### Implementation details:

**GetReasoningConfig:**
- Check if modelID starts with any key in `reasoningParamMap` (e.g., "deepseek/" → "deepseek")
- Return matching config and true, or empty config and false

**ApplyReasoningParams:**
- Look up config for modelID
- If found, merge `RequestParams` into the body map (shallow merge — top-level keys only)
- Return the modified body

**ParseSSEChunk:**
- Parse the SSE data JSON into a generic `map[string]any`
- Extract content token from `choices[0].delta.content`
- Extract thinking token from the model-specific `SSEField`
- If the model is Anthropic, check the `type` field in the content block — if "thinking",
  set StreamChunk.Type = "thinking", otherwise "content"
- If `choices[0].finish_reason` is set, return a "done" chunk
- Return `StreamChunk` with appropriate Type and Delta fields

---

## 6. internal/provider/registry.go

Provider registry — manages multiple providers and active provider selection.

```go
package provider

import (
    "sync"

    m31errors "github.com/eshanized/M31A/internal/errors"
    "github.com/eshanized/M31A/internal/types"
)

// Registry manages multiple LLM providers and active provider selection.
type Registry struct {
    mu        sync.RWMutex
    providers map[string]LLMProvider
    active    string
}

// NewRegistry creates an empty registry.
func NewRegistry() *Registry

// Register adds a provider to the registry.
func (r *Registry) Register(name string, p LLMProvider)

// Active returns the current active provider name.
func (r *Registry) Active() string

// SetActive switches the active provider.
// Returns ErrModelNotFound if the provider is not registered.
func (r *Registry) SetActive(name string) error

// Get returns a provider by name.
// Returns ErrProviderUnreachable if not found.
func (r *Registry) Get(name string) (LLMProvider, error)

// List returns all registered provider names.
func (r *Registry) List() []string

// ActiveProvider returns the active LLMProvider implementation.
// Returns nil if no active provider is set.
func (r *Registry) ActiveProvider() LLMProvider
```

---

## 7. internal/provider/fallback.go

Auto-fallback logic — switches providers on 429/503 errors.

```go
package provider

import (
    "fmt"
    "net/http"

    m31errors "github.com/eshanized/M31A/internal/errors"
)

// FallbackEvent is emitted when a provider switch occurs.
type FallbackEvent struct {
    From string `json:"from"`
    To   string `json:"to"`
    Reason string `json:"reason"` // "rate_limited" | "unavailable"
}

// ShouldFallback determines if an HTTP error warrants provider fallback.
// Returns true for 429 (rate limited) and 503 (service unavailable).
func ShouldFallback(statusCode int) bool

// FindFallbackProvider attempts to switch from the current provider to a healthy alternative.
// Checks health of all registered providers. Returns the first healthy provider that is
// not the current one, along with a FallbackEvent describing the switch.
// Returns m31errors.ErrProviderUnreachable if no alternative is healthy.
func FindFallbackProvider(registry *Registry, currentProvider string) (string, *FallbackEvent, error)
```

---

## 8. internal/provider/interface.go (MODIFIED)

The existing file needs one change: the `ChatCompletionStream` signature uses
`provider.ChatRequest` (defined in this package), not `types.ChatRequest`. Move
`ChatRequest` and `ToolDefinition` from `internal/provider/interface.go` into
this package's namespace. The file already exists from Phase 0 — update it:

```go
package provider

import (
    "context"

    m31errors "github.com/eshanized/M31A/internal/errors"
    "github.com/eshanized/M31A/internal/types"
)

// LLMProvider is the interface all LLM gateway clients must implement.
type LLMProvider interface {
    Name() string
    FetchModels(ctx context.Context) ([]types.ModelInfo, error)
    ChatCompletionStream(ctx context.Context, req ChatRequest) (*types.StreamIterator, error)
    EstimateCost(usage types.Usage) float64
    HealthCheck(ctx context.Context) types.HealthStatus
    GetModel(id string) (*types.ModelInfo, error)
}

// ChatRequest is the request body for a chat completion call.
type ChatRequest struct {
    Model            string          `json:"model"`
    Messages         []types.Message `json:"messages"`
    MaxTokens        int             `json:"max_tokens,omitempty"`
    Stream           bool            `json:"stream"`
    Tools            []ToolDefinition `json:"tools,omitempty"`
    ReasoningEnabled bool            `json:"reasoning_enabled,omitempty"`
}

// ToolDefinition describes a tool available for function calling.
type ToolDefinition struct {
    Name        string `json:"name"`
    Description string `json:"description"`
    Parameters  string `json:"parameters"` // JSON schema as string
}
```

**Remove the `ProviderRegistry` struct from this file** — it's now `Registry` in `registry.go`.

---

## 9. internal/provider/openrouter/client_test.go

Unit tests for the OpenRouter client using `httptest` mock server.

```go
package openrouter

import (
    "context"
    "net/http"
    "net/http/httptest"
    "testing"
    "time"
)

func TestNew_ValidKey(t *testing.T)
func TestNew_EmptyKey(t *testing.T)
func TestHealthCheck_Live(t *testing.T)
func TestHealthCheck_Slow(t *testing.T)
func TestHealthCheck_Offline(t *testing.T)
func TestFetchModels_PopulatesCache(t *testing.T)
func TestFetchModels_StaleFallback(t *testing.T)
func TestEstimateCost(t *testing.T)
func TestChatCompletionStream_ContentOnly(t *testing.T)
func TestChatCompletionStream_WithThinking(t *testing.T)
func TestChatCompletionStream_Done(t *testing.T)
func TestChatCompletionStream_ContextExceeded(t *testing.T)
```

Use `httptest.NewServer` to mock the OpenRouter API. Create fixture JSON for model
catalog and SSE streaming responses. Test that:
- Health check correctly classifies live/slow/offline based on mock server delay
- FetchModels parses JSON correctly and populates cache
- Cache TTL expiry forces refetch
- Stale cache (< 24h) is returned on network failure
- ChatCompletionStream correctly parses SSE chunks into StreamChunks
- Thinking segments are detected and typed correctly
- Cost estimation matches pricing from cached models

---

## 10. internal/provider/zen/client_test.go

Same test structure as OpenRouter but with Zen-specific base URL and health check endpoint.

```go
package zen

import (
    "context"
    "net/http"
    "net/http/httptest"
    "testing"
)

func TestNew_ValidKey(t *testing.T)
func TestHealthCheck_Live(t *testing.T)
func TestFetchModels_PopulatesCache(t *testing.T)
func TestChatCompletionStream_ContentOnly(t *testing.T)
func TestEstimateCost(t *testing.T)
```

---

## 11. internal/provider/sse_test.go

Tests for the SSE parser.

```go
package provider

import (
    "io"
    "strings"
    "testing"
)

func TestSSEParser_SingleDataLine(t *testing.T)
func TestSSEParser_MultiLineData(t *testing.T)
func TestSSEParser_EventType(t *testing.T)
func TestSSEParser_DoneSentinel(t *testing.T)
func TestSSEParser_EmptyStream(t *testing.T)
```

---

## 12. internal/provider/reasoning_test.go

Tests for reasoning parameter normalization.

```go
package provider

import (
    "testing"
)

func TestGetReasoningConfig_DeepSeek(t *testing.T)
func TestGetReasoningConfig_OpenAI(t *testing.T)
func TestGetReasoningConfig_Anthropic(t *testing.T)
func TestGetReasoningConfig_Unknown(t *testing.T)
func TestApplyReasoningParams_Anthropic(t *testing.T)
func TestParseSSEChunk_Content(t *testing.T)
func TestParseSSEChunk_Thinking(t *testing.T)
func TestParseSSEChunk_Done(t *testing.T)
```

---

## 13. internal/provider/registry_test.go

Tests for provider registry.

```go
package provider

import (
    "testing"
)

func TestRegistry_RegisterAndActive(t *testing.T)
func TestRegistry_SetActive_Unknown(t *testing.T)
func TestRegistry_List(t *testing.T)
func TestFindFallbackProvider_Switches(t *testing.T)
func TestFindFallbackProvider_NoAlternative(t *testing.T)
```

---

# EXECUTION ORDER

Execute tasks in this strict order:

1.  Read existing files: `internal/provider/interface.go`, `internal/types/types.go`, `internal/types/constants.go`, `internal/errors/errors.go`
2.  Update `internal/provider/interface.go` (move ChatRequest/ToolDefinition here, remove ProviderRegistry struct)
3.  Create `internal/provider/cache.go`
4.  Create `internal/provider/sse.go`
5.  Create `internal/provider/reasoning.go`
6.  Create `internal/provider/registry.go`
7.  Create `internal/provider/fallback.go`
8.  Create `internal/provider/openrouter/client.go`
9.  Create `internal/provider/zen/client.go`
10. Create all test files (client_test.go, sse_test.go, reasoning_test.go, registry_test.go)
11. Run: `go mod tidy` — will add `net/http` test dependencies
12. Run: `go build ./...` — MUST succeed
13. Run: `go vet ./...` — MUST pass
14. Run: `go test -race ./internal/provider/...` — MUST pass all tests
15. Create `walkthrough_1.md`

---

# HARD CONSTRAINTS

- `go build ./...` MUST succeed with zero errors
- `go vet ./...` MUST produce zero warnings
- `go test -race ./internal/provider/...` MUST pass all tests
- You MUST NOT implement TUI code, workflow logic, or tool implementations
- You MUST NOT add direct Anthropic or OpenAI provider support
- You MUST NOT hardcode model lists — models are always fetched from the API
- HTTP client MUST have 30s dial timeout only — NO body read timeout
- All error returns must use sentinel errors from `internal/errors/` (not `fmt.Errorf` for known error conditions)
- `internal/provider/interface.go` from Phase 0 must be updated, not replaced — preserve all existing types that are still needed
- `walkthrough_1.md` MUST contain actual test output, not placeholder text

---

# WHAT SUCCESS LOOKS LIKE

When you are done, the developer can:
1. Run `go test -race ./internal/provider/...` and see all tests pass
2. Create an OpenRouter client with `openrouter.New(apiKey)` and call `FetchModels()` — returns real model list
3. Stream tokens via `ChatCompletionStream()` — SSE chunks parse into typed `StreamChunk` values
4. Switch providers via `registry.SetActive("zen")` — instant switch, cache already warm
5. Trigger auto-fallback on 429 — registry switches to healthy alternative
6. Read `walkthrough_1.md` and see actual test output + build verification

---

# WALKTHROUGH TEMPLATE

Generate `walkthrough_1.md` at the project root LAST, after all files are created and verified.

```markdown
# Walkthrough 1 — Provider Abstraction Layer

## Completed Tasks

### P1.1 — LLMProvider Interface + ProviderRegistry
- [ ] LLMProvider interface finalized (ChatRequest, ToolDefinition in provider package)
- [ ] Registry implemented: Register, Active, SetActive, Get, List, ActiveProvider
- [ ] ProviderRegistry struct from Phase 0 replaced with Registry

### P1.2 — OpenRouter Client
- [ ] OpenRouterClient struct with apiKey, baseURL, httpClient, cache
- [ ] New() validates API key (returns ErrInvalidKey on empty)
- [ ] FetchModels() parses GET /models, populates cache
- [ ] ChatCompletionStream() POSTs /chat/completions, parses SSE, returns StreamIterator
- [ ] EstimateCost() uses cached pricing for cost calculation
- [ ] HealthCheck() GETs /auth/key, classifies live/slow/offline by latency
- [ ] HTTP client: 30s dial timeout, no body read timeout
- [ ] Headers: Authorization, HTTP-Referer, X-Title, User-Agent

### P1.3 — OpenCode Zen Client
- [ ] ZenClient struct with same shape as OpenRouter
- [ ] baseURL = https://opencode.ai/zen/v1
- [ ] FetchModels() GET /models, maps Zen-specific fields to ModelInfo
- [ ] HealthCheck() GET /models (catalog = auth validation)
- [ ] ChatCompletionStream() same SSE parsing as OpenRouter
- [ ] No hardcoded model lists

### P1.4 — Reasoning Normalization
- [ ] ReasoningConfig struct with ModelFamily, RequestParams, SSEField
- [ ] reasoningParamMap for deepseek, openai-o, anthropic, qwen
- [ ] GetReasoningConfig() matches model ID prefix
- [ ] ApplyReasoningParams() merges params into request body
- [ ] ParseSSEChunk() extracts content/thinking tokens based on SSEField

### P1.5 — Model Cache
- [ ] ModelCache with sync.RWMutex, models map, fetched time, TTL, staleTTL
- [ ] Get() returns model by ID with expiry check
- [ ] Set() replaces entire cache
- [ ] IsExpired() checks TTL, IsStale() checks 24h stale threshold
- [ ] Len() and FetchTime() accessors

### P1.6 — Auto-Fallback
- [ ] ShouldFallback() returns true for 429 and 503
- [ ] FindFallbackProvider() checks health of all providers, returns first healthy alternative
- [ ] FallbackEvent emitted with From, To, Reason
- [ ] Returns ErrProviderUnreachable if no alternative is healthy

### P1.7 — SSE Parser
- [ ] SSEParser with bufio.Scanner
- [ ] Next() returns eventType, data, error
- [ ] Multi-line data: concatenated correctly
- [ ] [DONE] sentinel returns io.EOF
- [ ] Close() releases response body

## Test Results

### All Provider Tests
```
[paste actual output of: go test -race -v ./internal/provider/...]
```

### Test Summary
- Total tests: [count]
- Passed: [count]
- Failed: [count]
- Skipped: [count]

## Build Verification
- [ ] `go mod tidy` passes
- [ ] `go build ./...` passes (output below)
- [ ] `go vet ./...` passes (output below)

### go build ./...
```
[paste actual output]
```

### go vet ./...
```
[paste actual output]
```

## Deviations from Spec
[List any deviation from ROADMAP.md Phase 1 tasks, or write "None"]

## Open Questions / Blockers for Phase 2
[List anything Phase 2 (TUI Foundation) needs to know, or write "None"]

## Deliverable Summary
[3-5 sentences summarizing what was implemented and confirming all Phase 1 deliverables are met.]
```

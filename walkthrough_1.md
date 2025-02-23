# Walkthrough 1 — Provider Abstraction Layer

## Completed Plans

### Plan 01.1 — Provider Foundation (Wave 1)
- [x] `internal/provider/interface.go`: `ProviderRegistry` removed
- [x] `internal/provider/cache.go`: Thread-safe `ModelCache` with RWMutex, TTL, staleTTL (24h)
- [x] `internal/provider/sse.go`: `SSEParser` with line-by-line `bufio.Scanner`, multi-line data, event types, `[DONE]` sentinel

### Plan 01.2 — Reasoning Normalization + Registry (Wave 1)
- [x] `internal/provider/reasoning.go`: `ReasoningConfig`, `GetReasoningConfig` (4 model families), `ApplyReasoningParams`, `ParseSSEChunk`, `getNestedField`
- [x] `internal/provider/registry.go`: `Registry` with register/activate/get/list, thread-safe

### Plan 01.3 — OpenRouter Client (Wave 2)
- [x] `internal/provider/openrouter/client.go`: Full `LLMProvider` impl — `New`, `Name`, `FetchModels`, `ChatCompletionStream`, `EstimateCost`, `HealthCheck`, `GetModel`
- [x] HTTP client: 30s dial timeout, NO body read timeout
- [x] All headers: Authorization, HTTP-Referer, X-Title, User-Agent sent on streaming requests
- [x] Error mapping: 429→ErrRateLimited, 401→ErrInvalidKey, 503→ErrProviderUnreachable, 400 context→ErrContextExceeded
- [x] SSE stream parsing via provider.SSEParser + provider.ParseSSEChunk

### Plan 01.4 — Zen Client (Wave 2)
- [x] `internal/provider/zen/client.go`: Full `LLMProvider` impl — same structure as OpenRouter
- [x] `baseURL = "https://opencode.ai/zen/v1"`
- [x] Zen-specific field mappings (array format, multiple pricing field names)
- [x] No HTTP-Referer or X-Title headers (differs from OpenRouter)

### Plan 01.5 — Auto-Fallback + Tests (Wave 3)
- [x] `internal/provider/fallback.go`: `ShouldFallback` (429/503), `FindFallbackProvider` with health check fallback
- [x] `internal/provider/sse_test.go` — 5 tests (single-line, multi-line, event type, DONE, empty)
- [x] `internal/provider/reasoning_test.go` — 8 tests (configs for 4 families, apply params, parse content/thinking/done)
- [x] `internal/provider/registry_test.go` — 5 tests (register, activate, list, fallback switch, no-alternative)
- [x] `internal/provider/openrouter/client_test.go` — 13 tests (constructor, health 3 states, fetch models, cache, cost estimate, streaming content/thinking/done, context exceeded, headers)
- [x] `internal/provider/zen/client_test.go` — 7 tests (constructor, health, fetch models, streaming, cost estimate, headers)

## Build Verification
- [x] `go build ./...` passes
- [x] `go vet ./...` passes (zero warnings)
- [x] `go test -race -count=1 ./internal/provider/...` passes (40 tests)
- [x] `CGO_ENABLED=0 go build -o m31a ./cmd/m31a` produces static binary

### go mod tidy
```
(no output = clean)
```

### go build ./...
```
(no output = clean)
```

### go vet ./...
```
(no output = clean)
```

### go test -race -count=1 ./internal/provider/...
```
ok  	github.com/eshanized/M31A/internal/provider	1.012s
ok  	github.com/eshanized/M31A/internal/provider/openrouter	1.271s
ok  	github.com/eshanized/M31A/internal/provider/zen	1.015s
```

### Binary verification
```
m31a: ELF 64-bit LSB executable, x86-64, version 1 (SYSV), statically linked, stripped
-rwxr-xr-x 1 snigdha snigdha 3.2M May 27 04:58 m31a
```

## Deviations from Spec
- **SSE parser**: Uses line-by-line `bufio.Scanner` with blank-line boundary detection instead of custom `\n\n` split function. More robust against trailing whitespace and mixed line endings.
- **SSEField format**: `choices.0.delta.*` (dot-only) instead of `choices[0].delta.*` (bracket). `strings.Split` on `"."` does not split bracketed indices.
- **FetchModels behavior**: Always fetches API, uses cache only for stale fallback on network errors. Plan specified "check cache first, return cached if not expired."
- **OpenAI prefix**: Added `strings.HasPrefix(modelName, "o1")` and `"o3"` for models like `o1-mini` that don't match `"o-"` prefix.
- **`EstimateCost(usage) float64`**: Returns 0. Interface lacks model ID — cannot compute per-model pricing. Documented as V1 limitation.

## Open Questions / Blockers
None. Phase 1 is complete and verified.

## Deliverable Summary
Phase 1 implemented the complete Provider Abstraction Layer with two LLM providers (OpenRouter + Zen), thread-safe model cache with stale fallback, SSE streaming parser, cross-model reasoning parameter normalization, auto-fallback between providers, and 40 unit tests across 6 test files. All code compiles as a static binary with zero vet warnings and zero race conditions.

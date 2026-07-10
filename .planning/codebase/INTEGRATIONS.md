# M31A External Integrations

## Provider Integrations

M31A connects to three LLM provider gateways. All three implement the same
`LLMProvider` interface defined in [`internal/provider/interface.go`](../../internal/provider/interface.go):

```go
type LLMProvider interface {
    Name() string
    APIKey() string
    FetchModels(ctx context.Context) ([]types.ModelInfo, error)
    CachedModels() []types.ModelInfo
    ChatCompletionStream(ctx context.Context, req ChatRequest) (*types.StreamIterator, error)
    EstimateCost(modelID string, usage types.Usage) float64
    HealthCheck(ctx context.Context) types.HealthStatus
    GetModel(id string) (*types.ModelInfo, error)
}
```

Every provider client embeds `BaseClient` from [`internal/provider/base_client.go`](../../internal/provider/base_client.go)
and overrides only provider-specific methods.

---

### OpenRouter

| Property | Value |
|---|---|
| Package | `internal/provider/openrouter/` |
| Default base URL | `https://openrouter.ai/api/v1` |
| Models endpoint | `GET /models` |
| Chat endpoint | `POST /chat/completions` |
| Health endpoint | `GET /auth/key` |
| Auth header | `Authorization: Bearer <api-key>` |
| Extra headers | `HTTP-Referer: https://github.com/eshanized/M31A`, `X-Title: M31A` |

**Model catalog:** Returns `{"data": [...]}` with per-model ID, name, description, context
length, and per-token pricing. Pricing is normalised to per-million-token units internally.
Model capabilities (tool use, vision, reasoning) are inferred from the model ID via
`ParseModelCapabilities()` in `internal/provider/capabilities.go`.

**Streaming:** Uses Server-Sent Events (SSE). The `doChatStream` method sends a POST with
`"stream": true` in the body. Response is parsed by `SSEParser` in
[`internal/provider/sse.go`](../../internal/provider/sse.go) and exposed as a `StreamIterator`.

**Retry:** Up to 2 retries with exponential backoff (1s, 2s) for HTTP 500/502/503 and
network-level errors (connection reset, unexpected EOF).

**Error handling:** `HandleChatHTTPErrorWithCredits` maps:
- `401` -> `ErrInvalidKey`
- `402` -> `ErrNoCredits: insufficient credits on openrouter`
- `429` -> `ErrRateLimited` (with `Retry-After` header value appended)
- `503` -> `ErrProviderUnreachable`
- `400`/`413` with context-overflow body -> `ErrContextExceeded`

---

### OpenCode Zen

| Property | Value |
|---|---|
| Package | `internal/provider/zen/` |
| Default base URL | `https://opencode.ai/zen/v1` |
| Models endpoint | `GET /models` |
| Chat endpoint | `POST /chat/completions` |
| Health endpoint | `GET /models` |
| Auth header | `Authorization: Bearer <api-key>` |

**Model catalog:** Returns `{"object": "list", "data": [...]}` with ID and owner only —
no per-model pricing or context length. These fields are enriched post-fetch by
`provider.EnrichModelInfo()` which cross-references the OpenRouter database
(or a local metadata table) to populate pricing and context window sizes.

**Streaming:** Same SSE pattern as OpenRouter.

**Error handling:** On `401` the response body is inspected for the strings
`CreditsError`, `payment`, `billing`, or `credit`. If any match, the error is
wrapped as `ErrNoCredits`; otherwise `ErrInvalidKey` is returned. `502` responses
include the sanitized body text (Zen-specific behavior).

---

### Nvidia NIM

| Property | Value |
|---|---|
| Package | `internal/provider/nvidia/` |
| Default base URL | `https://integrate.api.nvidia.com/v1` |
| Models endpoint | `GET /models` |
| Chat endpoint | `POST /chat/completions` |
| Health endpoint | `GET /models` |
| Auth header | `Authorization: Bearer <api-key>` |

**Model catalog:** Uses the OpenAI-compatible model list format. Non-chat models and
models flagged as broken are filtered out via `IsNonChatModel()` and
`IsLikelyBrokenOnNvidia()`. Pricing is always `0` (Nvidia NIM does not return pricing).

**Streaming:** Same SSE pattern with an additional NVIDIA-specific body builder
(`buildNvidiaBody`) that:
- Injects `extra_body` parameters for reasoning models
- Adds `force_text` hints for multimodal models receiving text-only input

**Error handling:** `404` evicts the model from the cache (self-healing). `400` also evicts
if the body does not match context-overflow patterns. `HandleChatHTTPErrorWithCredits` is
used for all other non-200 codes.

---

### Common Provider Infrastructure

**BaseClient** (`internal/provider/base_client.go`):

- Two `http.Client` instances per provider share a single `sync.Once`-initialized
  `http.Transport` (connection pool reuse across providers):
  - `HTTPClient` — no timeout (SSE streams can be arbitrarily long)
  - `CatalogClient` — 15-second timeout (`FetchModelsTimeout`) for catalog/health calls
- `APIKey()` always returns a masked string (`****<last-4>`) — the raw key is only used in
  `Authorization` headers.

**Model cache** (`internal/provider/cache.go`):

- Two-tier TTL: primary TTL of 5 minutes (`ModelCacheTTL`), stale TTL of 24 hours
  (`StaleCacheTTL`)
- On primary TTL expiry, a fresh fetch is attempted; on failure, stale entries are served
  until the stale TTL expires. This prevents model-selector flicker during network blips.

**Wire format** (`internal/provider/common.go`):

- All providers use the OpenAI-compatible chat completion wire format
- `messagesToWire()` drops storage-only fields (`Segments`, `CreatedAt`, `Usage`, `SkipForLLM`)
- Tool calls are serialized as `{id, type: "function", function: {name, arguments}}` (string)
- `BuildChatBody()` injects `"stream": true` unconditionally

**Provider Registry** (`internal/provider/registry.go`):

- Holds all registered providers by name
- `TrySetActive(name)` atomically switches the active provider
- `ListAll()` returns names sorted alphabetically (used by fallback when no priority order given)

---

## Auto-Fallback Mechanism

Implemented in [`internal/provider/fallback.go`](../../internal/provider/fallback.go):

1. When a provider errors, `FindFallbackProvider` is called with the configured
   `fallback_priority` list (e.g., `["openrouter", "zen", "nvidia"]`).
2. Health checks run **in parallel** against all candidate providers (not serially).
3. The first provider with `status: "live"` (latency < 500ms by default) is selected.
4. If none are live, the first `status: "slow"` provider is used.
5. If none are reachable, `ErrProviderUnreachable` is returned.
6. Rate-limit-induced fallbacks (`429`) respect the `Retry-After` header (capped at 120s).
   The wait is returned to the TUI as a `FallbackAfterWait` so the Bubble Tea event loop
   is not blocked.

---

## API Key Management — `pkg/keychain/`

Keys are **never written to disk in plaintext**. The keychain package provides a
platform-specific `Keychain` interface:

```go
type Keychain interface {
    Get(service string) (string, error)
    Set(service, value string) error
    Delete(service string) error
}
```

Service names use the prefix `m31a/` (`servicePrefix` constant) and are validated by
regex before any OS call to prevent command injection and path traversal.

### Linux — `keychain_linux.go` (build tag: `linux`)

Primary: **D-Bus Secret Service** (`org.freedesktop.secrets`)

1. Connects to the session D-Bus (`dbus.ConnectSessionBus()`)
2. Opens a `plain` crypto session (`OpenSession`)
3. `SearchItems` with `{"service": "m31a/<name>"}` attribute
4. `GetSecret` / `Item.SetSecret` / `Item.Delete`

Fallback: **`pass` CLI** (`pass show/insert/rm`)

- Falls back automatically when D-Bus is unavailable (`dbus.ErrClosed`, connection refused,
  or method not found)
- `pass` invocations use separate `exec.Command` arguments (no shell interpolation)
- GPG decryption failures produce a user-friendly error with guidance to re-init `pass`

### macOS — `keychain_darwin.go` (build tag: `darwin`)

Uses `/usr/bin/security` CLI with `find-generic-password` / `add-generic-password -U` /
`delete-generic-password`. The `-U` flag on `add-generic-password` handles upsert without
a separate search step. Password is piped via stdin (`-w -`), never passed as a command argument.

### Windows — `keychain_windows.go` (build tag: `windows`)

Uses **Windows Credential Manager** via direct Win32 API calls (`CredReadW`, `CredWriteW`,
`CredDeleteW`) through `syscall` and `unsafe`. No external CLI dependency.

### Availability Caching — `keychain.go`

`NewCached(inner Keychain)` wraps any backend with an `atomic.Bool` availability flag.
Once an `ErrKeychainUnavailable` or `ErrNotImplemented` is returned, all subsequent calls
return `ErrKeychainUnavailable` immediately without hitting the backend — prevents repeated
D-Bus reconnect attempts in headless/CI environments.

---

## Tool Integrations — 18 Built-in Tools

All tools are dispatched through the `Dispatcher` in
[`internal/tools/dispatcher.go`](../../internal/tools/dispatcher.go) and gated by the permission
system. Each tool is defined as a Go file in `internal/tools/`.

| Tool | File | Purpose |
|---|---|---|
| `Bash` | `bash.go` | Execute shell commands; platform-aware sandboxing |
| `FileRead` | `fileread.go` | Read file contents with 50 MB size cap |
| `FileWrite` | `filewrite.go` | Write files atomically via temp file + rename |
| `Edit` | `edit.go` | In-place text replacement with 7-strategy cascade |
| `Glob` | `glob.go` | File pattern matching via `doublestar`, max 1000 results |
| `Grep` | `grep.go` | Content search with regex, max 100 results |
| `WebFetch` | `webfetch.go` | Fetch URLs; SSRF-protected (blocks private/loopback IPs) |
| `WebSearch` | `websearch.go` | Web search via `https://search.sagibo.net`; DNS-rebinding protected |
| `CodeMap` | `codemap.go` | Summarise code structure (imports, symbols) |
| `CodeComplexity` | `codecomplexity.go` | Classify codebase size (simple/moderate/complex) |
| `FileDelete` | `filedelete.go` | Delete files with confirmation gating |
| `FileMove` | `filemove.go` | Move/rename files |
| `FileList` | `filelist.go` | List directory tree with optional depth limit |
| `TodoWrite` | `todo.go` | Write structured TODO items for the current task |
| `TodoRead` | `todoread.go` | Read current TODO list |
| `DevServer` | `devserver.go` | Manage dev server lifecycle (start/stop/restart) |
| `HTTPCheck` | `httpcheck.go` | Run HTTP smoke tests and route discovery |
| `AskUserQuestion` | `question.go` | Block until the user answers an inline question |

### Permission System

Every tool execution goes through `checkPermission()` in
[`internal/tools/permissions.go`](../../internal/tools/permissions.go):

1. Check static `d.permissions` map (set by previous "allow always" answers)
2. Check `d.batchApprovals` map (task-scoped batch approvals, revoked on task completion)
3. If neither matches, emit a `PermissionRequest` to the TUI (modal: allow / allow always / deny / exit)
4. Wait on a per-request channel (buffered, 300-second timeout by default)

`Bash` is classified by risk level (`types.RiskLevel`): low/medium/high/critical based on
command pattern analysis. This affects the rate limit tier applied.

### Rate Limiting

Token bucket implementation in [`internal/tools/concurrency.go`](../../internal/tools/concurrency.go):

| Tool class | Burst | Sustained |
|---|---|---|
| Normal tools | 20 | 10/sec |
| Dangerous tools | 5 | 2/sec |

Additionally, a semaphore (`MaxConcurrentTools = 8`) caps concurrent tool executions globally.

### Bash Sandboxing

Platform-specific files implement sandbox hardening:

- `bash_sandbox_linux.go` — uses process groups and `/proc/self/fd` tricks for cleanup
- `bash_sandbox_darwin.go` — macOS-specific process management
- `bash_sandbox_windows.go` — Windows job objects
- `bash_sandbox_other.go` — stub for unsupported platforms

Dangerous command blocklist is enforced at the tool boundary before permission check
(defense-in-depth against command injection obfuscation).

### SSRF and DNS Rebinding Protection

`WebFetch` (`webfetch.go`) resolves the destination hostname and rejects requests to:
- Private address space (10.x, 172.16-31.x, 192.168.x)
- Loopback (127.x, ::1)
- Link-local (169.254.x, fe80::)

`WebSearch` (`websearch.go`) uses a DNS cache (`dns_cache.go`) with 5-minute TTL to prevent
TOCTOU rebinding attacks: the IP is resolved once and re-verified before connection.

---

## WebSearch Integration

- Endpoint: `https://search.sagibo.net` (configured in `internal/tools/constants.go`)
- Max query length: 500 characters
- Max results returned to LLM: 5 (configurable up to 10)
- Can be disabled entirely via `[tools] websearch_enabled = false` in config

---

## No Webhook / Callback Patterns

M31A does not expose any inbound HTTP endpoints, webhooks, or pub/sub callbacks.
All provider communication is strictly outbound (client-initiated). The only long-lived
connection is the SSE streaming connection to the active provider during generation.

---

## Rate Limiting Summary

| Mechanism | Where | Limit |
|---|---|---|
| Tool token bucket (normal) | `internal/tools/concurrency.go` | 20 burst / 10 per-sec |
| Tool token bucket (dangerous) | `internal/tools/concurrency.go` | 5 burst / 2 per-sec |
| Tool concurrency semaphore | `internal/tools/concurrency.go` | Max 8 concurrent |
| Provider Retry-After | `internal/provider/fallback.go` | Honour header, cap at 120s |
| Context warning | `internal/tokens/` | Banner at 80%, hard reject at 95% |
| Permission modal timeout | `internal/types/constants.go` | 300 seconds (configurable) |

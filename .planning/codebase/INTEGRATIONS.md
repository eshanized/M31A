# External Integrations

**Last mapped:** 2026-08-02
**Project:** M31 Autonomous (Terminal AI Coding Agent)

## LLM Provider Integrations

### OpenRouter

- **Base URL:** `https://openrouter.ai/api/v1`
- **Client:** `internal/integrations/provider/openrouter/client.go`
- **Purpose:** Multi-model LLM access (primary provider)
- **Features:** Streaming, model discovery, rate limiting
- **Authentication:** API key stored in OS keychain

### Zen (OpenCode)

- **Base URL:** `https://opencode.ai/zen/v1`
- **Client:** `internal/integrations/provider/zen/client.go`
- **Purpose:** Alternative LLM provider
- **Features:** Retry logic, streaming
- **Authentication:** API key stored in OS keychain

### Nvidia

- **Base URL:** `https://integrate.api.nvidia.com/v1`
- **Client:** `internal/integrations/provider/nvidia/client.go`
- **Purpose:** Nvidia AI endpoints
- **Features:** Model metadata fetching
- **Authentication:** API key stored in OS keychain

## Provider Infrastructure

### Provider Registry

- **Location:** `internal/integrations/provider/registry.go`
- **Purpose:** Dynamic provider registration and discovery
- **Pattern:** Interface-based design with `Provider` interface

### Fallback System

- **Location:** `internal/integrations/provider/fallback.go`
- **Purpose:** Automatic provider failover
- **Logic:** Tries providers in sequence on failure

### Model Metadata

- **Location:** `internal/integrations/provider/model_metadata.go`
- **Purpose:** Dynamic model list fetching from APIs
- **Timeout:** `types.FetchModelsTimeout`

### Base Client

- **Location:** `internal/integrations/provider/base_client.go`
- **Purpose:** Shared HTTP client logic for all providers
- **Features:** Separate clients for streaming vs catalog/health

## Web Services

### Web Search

- **Service:** SearXNG instance
- **Base URL:** `https://search.sagibo.net`
- **Client:** `internal/tools/search/websearch.go`
- **Purpose:** Web search for documentation and references
- **User-Agent:** `M31A/{version} (AI Coding Agent; +https://github.com/eshanized/M31A)`

### Web Fetch

- **Client:** `internal/tools/search/webfetch.go`
- **Purpose:** Fetch and parse web content
- **Features:** HTML to Markdown conversion

## System Integrations

### Git Integration

- **Purpose:** Version control, commit history, branch management
- **Usage:** Throughout workflow engine for rollback chains
- **Commands:** `git log`, `git diff`, `git commit`, etc.

### OS Keychain

- **Location:** `pkg/keychain/`
- **Purpose:** Secure credential storage
- **Usage:** API keys for LLM providers
- **Security:** Never written to disk in plaintext

### D-Bus (Linux)

- **Dependency:** `github.com/godbus/dbus/v5`
- **Purpose:** Linux system integration
- **Usage:** Desktop notifications, system events

## HTTP Client Configuration

### Streaming Client

- **Timeout:** No hard timeout (streaming)
- **Purpose:** LLM streaming responses
- **Configuration:** `BaseClient.HTTPClient`

### Catalog Client

- **Timeout:** Hard timeout (10 seconds typical)
- **Purpose:** Model discovery, health checks
- **Configuration:** `BaseClient.CatalogClient`

## Authentication

### API Key Management

- **Storage:** OS keychain (macOS Keychain, Windows Credential Vault, Linux Secret Service)
- **Retrieval:** `pkg/keychain/` package
- **Security:** Never logged, never written to config files

### Request Headers

- **Referer:** `https://github.com/eshanized/M31A` (for OpenRouter)
- **User-Agent:** `M31A/{version}` (for web requests)

## Error Handling

### Retry Logic

- **Location:** `internal/infrastructure/retry/`
- **Purpose:** Automatic retry on transient failures
- **Policy:** Configurable retry counts and delays

### Rate Limiting

- **Location:** `internal/infrastructure/ratelimit/`
- **Purpose:** Prevent API abuse
- **Implementation:** Token bucket algorithm

## Configuration

### Provider Configuration

- **Location:** `internal/core/config/types.go`
- **Fields:**
  - `OpenRouterBaseURL`
  - `ZenBaseURL`
  - `NvidiaBaseURL`
  - `WebSearchBaseURL`

### Default Values

- **OpenRouter:** `https://openrouter.ai/api/v1`
- **Zen:** `https://opencode.ai/zen/v1`
- **Nvidia:** `https://integrate.api.nvidia.com/v1`
- **Web Search:** `https://search.sagibo.net`
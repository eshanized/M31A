# External Integrations — M31A

> Mapped: 2026-07-09

## LLM Providers

M31A connects to three external LLM API providers with automatic fallback:

### 1. OpenRouter

| Aspect | Details |
|--------|---------|
| **Client location** | `internal/provider/openrouter/` |
| **API type** | REST + SSE streaming |
| **Auth** | API key in `OPENROUTER_API_KEY` env var |
| **Model catalog** | 300+ models, discovered dynamically |
| **Purpose** | Primary provider (default) |
| **Fallback priority** | 1st in `fallback_priority` |

### 2. OpenCode Zen

| Aspect | Details |
|--------|---------|
| **Client location** | `internal/provider/zen/` |
| **API type** | REST + SSE streaming |
| **Auth** | API key in `ZEN_API_KEY` env var |
| **Purpose** | Secondary fallback provider |

### 3. Nvidia NIM

| Aspect | Details |
|--------|---------|
| **Client location** | `internal/provider/nvidia/` |
| **API type** | REST + SSE streaming |
| **Auth** | API key in `NVIDIA_API_KEY` env var |
| **Purpose** | Tertiary fallback provider |

### Provider Infrastructure

| Component | Location | Purpose |
|-----------|----------|---------|
| `internal/provider/interface.go` | Provider interface definition | Common contract for all providers |
| `internal/provider/base_client.go` | Base client implementation | SSE streaming, model caching |
| `internal/provider/cache.go` | Model metadata cache | TTL-based, `singleflight` dedup |
| `internal/provider/fallback.go` | Auto-fallback logic | Degradation detection, retry |
| `internal/provider/capabilities.go` | Capability registry | Dynamic per-model capabilities |
| `internal/provider/registry.go` | Provider registry | Registration, lookup, health checks |
| `internal/provider/sse.go` | SSE streaming | Server-Sent Events parsing |
| `internal/provider/reasoning.go` | Reasoning content | Extended thinking support |

### Provider features:
- Configurable `fallback_priority` order
- `health_check_timeout_secs` (default 10s)
- `registration_order` (config or predefined)
- Custom base URLs for self-hosted/proxied gateways
- Model capability overrides via config

## OS Keychain (Secrets Storage)

| Backend | Platform | Implementation |
|---------|----------|---------------|
| **D-Bus secret-service** | Linux | `pkg/keychain/keychain_linux.go` via `github.com/godbus/dbus/v5` |
| **macOS Keychain** | macOS | `pkg/keychain/keychain_darwin.go` via system APIs |
| **Windows Credential Manager** | Windows | `pkg/keychain/keychain_windows.go` via win32 API |

Keychain service paths are sanitized via `sanitizeService` to block traversal attacks. API keys are **never written to disk in plaintext**.

## Git Integration

| Feature | Location | Details |
|---------|----------|---------|
| Commit operations | `internal/git/` | Commit, rollback, diff, stash, branch |
| Bisect wrapper | `pkg/bisect/` | `git bisect` programmatic wrapper |
| Rollback manager | `pkg/rollback/` | Soft/hard/safe reset with preview |
| Diff viewer | `internal/tui/diff_model.go` | Inline diff with syntax highlighting |
| Worktree isolation | `internal/tools/subagent/worktree.go` | Subagents in isolated git worktrees |

## Clipboard

| Integration | Package | Details |
|-------------|---------|---------|
| Clipboard access | `github.com/atotto/clipboard` | Copy/paste from TUI |

## File System

| Integration | Location | Details |
|-------------|----------|---------|
| Atomic writes | `internal/fileutil/` | Safe file operations |
| Glob pattern matching | `internal/tools/glob.go` | Uses `doublestar/v4` |
| File watcher | `github.com/fsnotify/fsnotify` | Config hot-reload, session file watching |

## Network Security

| Protection | Location | Mechanism |
|------------|----------|-----------|
| SSRF protection | `internal/tools/webfetch.go` | Blocks private/loopback/link-local IPs |
| DNS rebinding | `internal/tools/dns_cache.go` | 5-min TTL DNS cache (TOCTOU prevention) |

## No Dependencies On

The following are deliberately absent:
- No external database
- No message queue
- No container runtime
- No cloud storage
- No monitoring/APM service
- No authentication provider (other than provider API keys)
- No webhook receiver
- No external cache (Redis, etc.)
- No gRPC

The project is entirely self-contained with no runtime dependencies beyond the OS.

# External Integrations

**Analysis Date:** 2026-07-16

## APIs & External Services

### LLM Providers (3 Primary Integrations)

All three providers implement the `provider.LLMProvider` interface (`internal/provider/interface.go`). Models are discovered dynamically via each provider's `/models` endpoint — **no hardcoded model names** in the codebase.

| Provider | Base URL | Auth | Key Features |
|----------|----------|------|--------------|
| **OpenRouter** | `https://openrouter.ai/api/v1` | `OPENROUTER_API_KEY` (Bearer) | Primary gateway; 300+ models; HTTP-Referer/X-Title headers; pricing from API; streaming via SSE |
| **Zen** | `https://api.zen.ai/v1` | `ZEN_API_KEY` (Bearer) | Credit-based; no pricing from API (enriched from OpenRouter/local); special 401 "CreditsError" handling |
| **NVIDIA NIM** | `https://integrate.api.nvidia.com/v1` | `NVIDIA_API_KEY` (Bearer) | NVIDIA-hosted models; multimodal detection; `extra_body` params for reasoning; force_text hint for vision models |

**Provider Interface (`internal/provider/interface.go:15-23`):**
```go
type LLMProvider interface {
    Name() string                              // "openrouter" | "zen" | "nvidia"
    APIKey() string                            // Resolved from config/env/keychain
    FetchModels(ctx) ([]types.ModelInfo, error) // Dynamic model discovery
    CachedModels() []types.ModelInfo
    ChatCompletionStream(ctx, req) (*types.StreamIterator, error)
    EstimateCost(modelID string, usage types.Usage) float64
    HealthCheck(ctx) types.HealthStatus        // /auth/key or /models endpoint
    GetModel(id string) (*types.ModelInfo, error)
}
```

**Fallback Chain (configurable via `provider.fallback_priority`):**
Default: `["nvidia", "zen", "openrouter"]` — tried in order on failure (rate limits, auth, model unavailable).

**Model Capability Detection (`internal/provider/capabilities.go`):**
- Heuristic parsing from model ID (e.g., `-r1` → reasoning, `vision`/`multimodal` → vision)
- Configurable overrides in `config.ModelCapabilitiesConfig` (`extra_reasoning_patterns`, `known_capabilities` map)

---

## Data Storage

### Configuration
- **File:** `~/.m31a/config.toml` (or `$M31A_CONFIG`)
- **Format:** TOML (`BurntSushi/toml`)
- **Schema:** `internal/config/types.go` — 27 top-level sections
- **Hot-reload:** Not supported; restart required

### Session Storage
- **Location:** `<project>/.m31a/sessions/<session-id>/`
- **Files:**
  - `session.json` — `pkg/types.Session` (metadata, workflow phase, model, provider)
  - `messages.json` — `[]types.Message` (conversation history)
  - `plan.json` — `internal/types.Plan` (task list, status)
  - `METRICS.json` — `pkg/metrics` (tool execution, token usage, phase timing)
- **Manager:** `pkg/session/manager.go`

### Ledger
- **File:** `~/.m31a/LEDGER.md` (or project-local `.m31a/LEDGER.md`)
- **Format:** Markdown with structured entries
- **Client:** `pkg/ledger/ledger.go`

### Backups (File Tools)
- **Dir:** `<project>/.m31a/backups/`
- **Managed by:** `internal/tools/backup.go` — per-file rotation (`max_backups_per_file`)

### Logs
- **Dir:** `~/.m31a/`
- **File:** `m31a.log` (daily rotation, 7-day retention)
- **Format:** JSON (default) or text (`M31A_LOG_FORMAT`)
- **Level:** `M31A_LOG_LEVEL` (debug/info/warn/error)

---

## Authentication & Identity

### API Key Resolution Order (per provider)
1. **Config file:** `provider.<name>.api_key` in `config.toml`
2. **Environment:** `<PROVIDER>_API_KEY` (e.g., `OPENROUTER_API_KEY`)
3. **OS Keychain:** Service `m31a/openrouter`, `m31a/zen`, `m31a/nvidia`, account `m31a`

### Keychain Implementation (`pkg/keychain/`)
| Platform | Backend | Fallback |
|----------|---------|----------|
| **Linux** | D-Bus Secret Service (`org.freedesktop.secrets`) via `godbus/dbus/v5` | `pass` CLI (if available) |
| **macOS** | `/usr/bin/security` CLI (Keychain Access) | — |
| **Windows** | Windows Credential Manager (CredRead/CredWrite/CredDelete) | — |

**Availability Caching (`pkg/keychain/keychain.go:38-86`):**
- Once any operation returns `ErrKeychainUnavailable` or `ErrNotImplemented`, the wrapper caches unavailability
- Prevents repeated D-Bus/pass connection attempts and duplicate warnings

**Config Integration (`cmd/m31a/main.go:326-338`):**
```go
kc, _ := keychain.New()
kc = keychain.NewCached(kc)
cfg.ResolveAPIKeys(kc)  // Populates config from keychain if env/config empty
```

---

## Monitoring & Observability

### Logging
- **Backend:** `log/slog` (stdlib)
- **Output:** `~/.m31a/m31a.log` (rotated daily, 7-day retention)
- **Format:** JSON (default) or text
- **Levels:** Configurable via `M31A_LOG_LEVEL`
- **Structured fields:** Version, commit, date, Go version, OS/arch, session ID, phase, model

### Metrics Collection (`pkg/metrics/`)
- **Enabled by default** (`features.metrics_enabled = true`)
- **Output:** `<session-dir>/METRICS.json`
- **Tracked:**
  - Tool executions (name, duration, success/error, bytes)
  - LLM token usage (prompt/completion/total per request)
  - Workflow phase timings
  - Cost estimates (from provider pricing)
- **Format:** JSON lines, one entry per event

### Health Checks
- **Endpoint:** Provider-specific (`/auth/key` for OpenRouter, `/models` for Zen/NVIDIA)
- **Thresholds (configurable):**
  - `healthcheck_live_ms` — "healthy" if < threshold (default: 2000ms)
  - `healthcheck_slow_ms` — "degraded" if > threshold (default: 5000ms)
- **Status:** `HealthStatus{Healthy, Degraded, Unhealthy, LatencyMs}`

---

## CI/CD & Deployment

### GitHub Actions (`.github/workflows/ci.yml`)

| Job | Runner | Purpose |
|-----|--------|---------|
| `lint` | ubuntu-latest | `gofmt`, `golangci-lint` (5m timeout), `goreleaser check` |
| `test` | ubuntu-latest | `go test -race -coverprofile=coverage.out ./...` |
| `security` | ubuntu-latest | `govulncheck ./...` |
| `build` | ubuntu/macos/windows × amd64/arm64 | Cross-compile static binaries (`CGO_ENABLED=0`), `go vet` |
| `release` | ubuntu-latest | Triggered on `v*` tags; runs GoReleaser (snapshot on PR, release on tag) |

**Release Artifacts (via `.goreleaser.yaml`):**
- Binaries: `m31a_<version>_<os>_<arch>.tar.gz`
- Packages: `.deb`, `.rpm`, `.apk`, `.pkg.tar.zst` (Arch)
- Scoop manifest (Windows)
- SHA256 checksums
- Auto-generated changelog (excludes docs/test/chore/build/ci/merge commits)

### Cross-Compilation Targets
```
linux/amd64    linux/arm64
darwin/amd64   darwin/arm64
windows/amd64  (windows/arm64 EXCLUDED)
```

---

## Environment Configuration

### Required Environment Variables
| Variable | Purpose | Source |
|----------|---------|--------|
| `OPENROUTER_API_KEY` | OpenRouter authentication | Env / Keychain / Config |
| `ZEN_API_KEY` | Zen authentication | Env / Keychain / Config |
| `NVIDIA_API_KEY` | NVIDIA NIM authentication | Env / Keychain / Config |

### Optional Environment Variables
| Variable | Default | Purpose |
|----------|---------|---------|
| `M31A_CONFIG` | `~/.m31a/config.toml` | Config file path |
| `M31A_LOG_LEVEL` | `info` | Log verbosity |
| `M31A_LOG_FORMAT` | `json` | Log format (json/text) |

### Secrets Location
- **Primary:** OS Keychain (see Authentication section)
- **Secondary:** Environment variables (CI, containers)
- **Tertiary:** `config.toml` (plaintext — not recommended for production)
- **Template:** `.env.example` (committed, shows required vars)

---

## Webhooks & Callbacks

### Incoming
- **None** — M31A is a CLI/TUI agent, not a server

### Outgoing (Tool-Level)
| Tool | Target | Purpose |
|------|--------|---------|
| `WebFetch` | Any HTTP/HTTPS URL | Fetch web content (markdown/html/text) — SSRF-protected |
| `WebSearch` | Configurable (`tools.websearch_base_url`, default: duckduckgo/html) | Search web, return snippets |
| `Git` | Local `.git` + remotes | Read/commit/push (standard git CLI) |

**WebFetch Security (`internal/tools/webfetch.go:55-150`):**
- DNS pinning via `DNSCache` (TTL configurable, default 300s)
- Private IP blocking (RFC1918, loopback, link-local, cloud metadata 169.254.169.254, AWS fd00:ec2::254)
- Redirect validation (re-resolves and re-checks each hop)
- Max redirects: configurable (`webfetch_max_redirects`, default 10)
- Timeout: configurable (`timeout` param, default 30s, max 120s)
- Body limit: 5MB (`types.MaxFileSize`)

---

## Local Tooling Integrations

### Git (`internal/git/git.go`)
- **Backend:** `git` CLI (executed via `os/exec`)
- **Operations:** Status, diff, log, commit, branch, push, worktree (for subagents)
- **Worktrees:** `subagent.GitWorktrees` — isolated worktrees per subagent

### Shell (`internal/tools/bash_*.go`)
- **Backend:** Platform-specific (`bash` on Unix, `cmd.exe`/`powershell` on Windows)
- **Sandboxing:** Optional (`bash_sandbox_*.go`) — restricted paths, command allowlist
- **Rate Limiting:** Token bucket (configurable burst/sec, separate for dangerous commands)

### File System
- **Atomic writes:** `internal/fileutil/atomic.go` — write to temp, rename
- **Locking:** `internal/fileutil/lock_*.go` — flock (Unix) / LockFileEx (Windows)

---

*Integration audit: 2026-07-16*
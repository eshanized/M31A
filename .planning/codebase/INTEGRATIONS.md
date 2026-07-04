# External Integrations

**Analysis Date:** 2026-07-04

## APIs & External Services

**LLM Providers (Chat Completion + Model Discovery):**

- **OpenRouter** — Primary multi-model LLM gateway
  - SDK/Client: Custom HTTP client (`internal/provider/openrouter/client.go`)
  - Endpoint: `https://openrouter.ai/api/v1` (configurable via `openrouter_base_url` TOML key)
  - Auth: Bearer token via `OPENROUTER_API_KEY` env var or OS keychain
  - Headers: `Authorization: Bearer <key>`, `HTTP-Referer`, `X-Title`
  - Capabilities: Chat completions (SSE streaming), model catalog (`/models`), health check (`/auth/key`)
  - Retry: Exponential backoff, max 2 retries on 500/502/503

- **Zen** — OpenCode.ai hosted LLM provider
  - SDK/Client: Custom HTTP client (`internal/provider/zen/client.go`)
  - Endpoint: `https://opencode.ai/zen/v1` (configurable via `zen_base_url` TOML key)
  - Auth: Bearer token via `ZEN_API_KEY` env var or OS keychain
  - Capabilities: Chat completions (SSE streaming), model catalog (`/models`), health check (`/models`)
  - Special handling: 401 credit detection (CreditsError, payment, billing patterns)

- **NVIDIA NIM** — NVIDIA's inference platform
  - SDK/Client: Custom HTTP client (`internal/provider/nvidia/client.go`)
  - Endpoint: `https://integrate.api.nvidia.com/v1` (configurable via `nvidia_base_url` TOML key)
  - Auth: Bearer token via `NVIDIA_API_KEY` env var or OS keychain
  - Capabilities: Chat completions (SSE streaming), model catalog (`/models`), health check (`/models`)
  - Special handling: Multimodal model detection (vision/multimodal IDs), `extra_body` params for reasoning, model eviction on 404/400

**Web Search:**

- **SearXNG** — Privacy-respecting meta-search engine
  - Client: Custom HTTP client (`internal/tools/websearch.go`)
  - Endpoint: `https://search.sagibo.net` (configurable via `tools.websearch_base_url` TOML key)
  - Auth: None (public API)
  - Protocol: HTTP GET `/search?q=<query>&format=json&categories=general`
  - Features: Engine selection (google, bing, duckduckgo), DNS pinning for SSRF protection
  - SSRF Protection: Blocks private/reserved IPs, DNS cache with TTL, redirect validation

**Web Fetch:**

- **Generic HTTP fetcher** — Fetches web pages for content extraction
  - Client: Custom HTTP client with SSRF protection (`internal/tools/webfetch.go`)
  - Auth: None (public web)
  - Features: HTML to text/markdown conversion, redirect following (max 5), retry with backoff (max 3)
  - Security: DNS pinning, private IP blocking, response size limits, HTML sanitization

## Data Storage

**Databases:**
- Not detected — No SQL/NoSQL database in use

**Session Storage:**
- File-based session persistence in `<workDir>/.m31a/` directory
- Session data: JSON files (`session.json`, `messages.json`, `checkpoint.json`)
- Max session file size: 50 MB (prevents OOM from corrupted files)
- File locking via `internal/fileutil/fileutil.go`

**File Storage:**
- Local filesystem only
- Session data: `<workDir>/.m31a/sessions/`
- Tool output cache: `~/.m31a/tool-output/`
- Backup directory: `<workDir>/.m31a/backups/`
- Config: `~/.m31a/config.toml` (global), `<workDir>/m31a.toml` (project)
- Logs: `~/.m31a/m31a.log` (daily rotation, 7-day retention)
- Ledger: `~/.m31a/LEDGER.md` (session history markdown)

**Caching:**
- Model cache: In-memory `ModelCache` with configurable TTL (default 5 min) and stale TTL (24 hours)
- DNS cache: In-memory `DNSCache` with TTL (for SSRF protection)
- Token estimation: EMA (Exponential Moving Average) calibration for token counting
- Config reload: fsnotify watcher with 50ms debounce (polling fallback every 5 seconds)
- Session list cache: 2-second TTL

## Authentication & Identity

**Auth Provider:**
- Custom multi-layer API key resolution (no OAuth/OIDC)

**Key Resolution Order (per provider):**
1. Environment variable (`M31A_OPENROUTER_API_KEY` or `OPENROUTER_API_KEY`)
2. OS keychain (`keychain.Get("openrouter")`)
3. Config file field (`provider.openrouter.api_key`)

**OS Keychain Implementations:**
- **Linux** (`pkg/keychain/keychain_linux.go`): D-Bus Secret Service API (`org.freedesktop.secrets`) with `pass` CLI fallback
  - D-Bus: `godbus/dbus/v5` — Direct Secret Service protocol
  - Fallback: `pass show m31a/<service>` / `pass insert -U -f m31a/<service>`
  - Cached wrapper: `keychain.NewCached()` — Prevents repeated D-Bus/pass attempts after first failure
- **macOS** (`pkg/keychain/keychain_darwin.go`): `/usr/bin/security` CLI (Keychain Services)
- **Windows** (`pkg/keychain/keychain_windows.go`): Windows Credential Manager

**Keychain Services:**
- `m31a/openrouter` — OpenRouter API key
- `m31a/zen` — Zen API key
- `m31a/nvidia` — NVIDIA API key

**Security Features:**
- API keys never written to disk in plaintext (when keychain available)
- API key masking in error messages (regex: `sk-[a-zA-Z0-9]{8,}`)
- `.env` file permission check: Skip group/world-writable files (0o022)
- Config file save: Keychain-first, fallback to config file if unavailable

## Monitoring & Observability

**Logging:**
- Framework: Go standard library `log/slog`
- Implementation: `internal/log/log.go`
- Format: JSON (default) or Text (via `M31A_LOG_FORMAT` env var)
- Output: File (`~/.m31a/m31a.log`) with daily rotation
- Rotation: Keeps 7 days of rotated logs (`m31a.log.YYYY-MM-DD`)
- Level: Configurable via `M31A_LOG_LEVEL` (debug/info/warn/error, default: info)

**Metrics:**
- Implementation: `pkg/metrics/metrics.go`
- Storage: `METRICS.json` per session (JSON file)
- Captures:
  - Tool execution metrics (call count, success/fail, duration)
  - Edit strategy metrics (which replacement strategy was used)
  - LLM interaction metrics (token usage, cost, prompt hash)
  - Workflow phase metrics (duration, transition count, heal/bisect stats)

**Error Tracking:**
- Structured error types in `internal/errors/errors.go`
- Sentinel errors for typed error handling (`ErrProviderUnreachable`, `ErrRateLimited`, etc.)
- Provider error sanitization: HTML stripping, API key masking, truncation to 200 chars

## CI/CD & Deployment

**Hosting:**
- Binary distribution via GitHub Releases (goreleaser)
- Package formats: deb, rpm, apk, archlinux, Windows Scoop

**CI Pipeline:**
- Not detected in repository (no `.github/workflows/` found)

**Release Process:**
- GoReleaser (`.goreleaser.yaml`) — Automated cross-platform builds
- `make release` — Snapshot builds
- `make release-dry` — Dry run (no publish)
- `make validate-release` — Full release validation (`scripts/validate-release.sh`)
- Changelog: Auto-generated, excludes docs/test/chore/ci/build commits
- Releases: Draft mode with prerelease auto-detection

**Docker:**
- `Dockerfile` — EFIE evaluation image (Go 1.24-alpine builder + alpine:3.19 runtime)
- Purpose: Reproducible benchmark/evaluation environment only
- NOT the production deployment method

## Environment Configuration

**Required env vars:**
```
OPENROUTER_API_KEY    — OpenRouter LLM provider (https://openrouter.ai)
ZEN_API_KEY           — Zen LLM provider (https://opencode.ai/zen)
NVIDIA_API_KEY        — NVIDIA NIM LLM provider (https://build.nvidia.com)
```

**Secrets location:**
- Primary: OS keychain (Linux: D-Bus Secret Service / pass; macOS: Keychain; Windows: Credential Manager)
- Fallback: Config file (`~/.m31a/config.toml`) — only when keychain unavailable
- `.env` files are gitignored (except `.env.example`)

**Config file format:**
- TOML with `${VAR}` variable substitution
- API keys are resolved at startup, never persisted to project-level config

## Webhooks & Callbacks

**Incoming:**
- None detected

**Outgoing:**
- LLM API requests (OpenRouter, Zen, NVIDIA) — SSE streaming
- Web search requests (SearXNG) — HTTP GET
- Web fetch requests (generic HTTP) — HTTP GET with content extraction

## External Process Dependencies

**Git CLI:**
- Required for: `git init`, `git add`, `git commit`, `git diff`, `git log`, `git status`, `git branch`, `git checkout`, `git merge`, `git tag`, `git push`, `git pull`, `git fetch`, `git stash`, `git bisect`, `git worktree`, `git config`
- Implementation: `internal/git/git.go` (shell-out via `os/exec`)
- Used by: Workflow engine, rollback, bisect, session checkpoint, ship phase

**Shell Execution:**
- `internal/shell/shell.go` — OS-aware shell detection (bash/zsh on Unix, cmd on Windows)
- Used by: Bash tool, dev server, runtime phase

**pass CLI (Linux keychain fallback):**
- Commands: `pass show m31a/<service>`, `pass insert -U -f m31a/<service>`, `pass rm -f m31a/<service>`
- Only used when D-Bus Secret Service is unavailable

## Provider Resilience

**Auto-Fallback:**
- `internal/provider/fallback.go` — Parallel health checks across providers
- Triggers on: Rate limiting (429), invalid key (401), provider unreachable (503)
- Health check: GET to provider's health endpoint, classify latency as live/slow/degraded

**Retry Policy:**
- `pkg/retry/policy.go` — Exponential backoff with retry-after header support
- Default: 3 attempts, 1s initial delay, 30s max delay, 2x backoff factor
- Provider clients: Built-in retry (max 2 attempts) for 500/502/503

**Model Cache Resilience:**
- Stale fallback: Returns cached models when provider is temporarily unreachable
- Model eviction: Removes unavailable/deprecated models from cache on 404/400

## Reasoning Model Support

**Model Families:**
- DeepSeek — Reasoning via `reasoning_content` SSE field
- OpenAI (o-series) — Reasoning via `reasoning` SSE field, `reasoning_effort` param
- Anthropic — Thinking via `thinking` param (budget_tokens)
- Qwen — Reasoning via `reasoning_content` SSE field
- NVIDIA Nemotron — Reasoning via `extra_body` params (chat_template_kwargs)

**Implementation:**
- `internal/provider/reasoning.go` — Model family detection and parameter injection
- Runtime: Reasoning content extracted from SSE stream based on model-specific field paths

---

*Integration audit: 2026-07-04*

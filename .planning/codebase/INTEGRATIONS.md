# External Integrations

**Analysis Date:** 2026-07-23

## APIs & External Services

**LLM Providers (Primary Integrations):**

| Provider | Purpose | SDK/Client | Auth |
|----------|---------|------------|------|
| **OpenRouter** (`openrouter.ai`) | Primary LLM gateway — 400+ models (Anthropic, OpenAI, Google, Meta, Mistral, etc.) | Custom client in `internal/integrations/provider/openrouter/client.go` | `OPENROUTER_API_KEY` env var or OS keychain (`m31a/openrouter`) |
| **Zen** (`opencode.ai/zen/v1`) | Alternative provider — curated models, competitive pricing | Custom client in `internal/integrations/provider/zen/client.go` | `ZEN_API_KEY` env var or OS keychain (`m31a/zen`) |
| **NVIDIA NIM** (`integrate.api.nvidia.com/v1`) | NVIDIA-optimized models (Nemotron, Llama, etc.) | Custom client in `internal/integrations/provider/nvidia/client.go` | `NVIDIA_API_KEY` or `M31A_NVIDIA_API_KEY` env var or OS keychain (`m31a/nvidia`) |

**Provider Integration Details:**
- All three implement `internal/integrations/provider.LLMProvider` interface (`internal/integrations/provider/interface.go`)
- Models fetched dynamically via `/models` endpoint — **never hardcoded**
- OpenAI-compatible chat completion streaming (`/chat/completions` with SSE)
- Health checks: OpenRouter `/auth/key`, Zen `/models`, NVIDIA `/models`
- Automatic fallback chain: `nvidia` → `zen` → `openrouter` (configurable via `provider.fallback_priority`)
- Cost estimation via provider pricing + token usage tracking
- Model capability detection (tools, reasoning, vision) via pattern matching + cache enrichment

**Web Search:**
- **SearXNG** (privacy-respecting meta-search engine)
- Default endpoint: `https://search.sagibo.net` (configurable via `tools.websearch_base_url`)
- Used by `WebSearch` tool (`internal/tools/search/websearch.go`)
- Supports engine selection (google, bing, duckduckgo, etc.) via `engines` parameter

**Web Fetch:**
- Generic HTTP/HTTPS fetch with HTML→Markdown conversion
- Used by `WebFetch` tool (`internal/tools/search/webfetch.go`, `webfetch_html.go`)
- SSRF protection: DNS pinning, private IP blocking, redirect validation
- Output formats: markdown (default), text, html

**HTTP Health Checks:**
- `HTTPCheck` tool (`internal/tools/network/httpcheck.go`) — validates dev server responses
- SSRF-protected transport (same DNS pinning + private IP blocking as WebFetch)

## Data Storage

**Databases:**
- **None** — No external database. All persistence is local filesystem.

**File Storage:**
- **Local filesystem only** — All data stored in `~/.m31a/` and project `.m31a/`
- **Key files:**
  - `~/.m31a/config.toml` — Global configuration
  - `~/.m31a/LEDGER.md` — Cross-session learning ledger (markdown table)
  - `~/.m31a/m31a.log` — Structured JSON logs (rotated daily, 7-day retention)
  - `~/.m31a/sessions/` — Session state (JSON)
  - `~/.m31a/backups/` — File edit backups for rollback
  - `~/.m31a/tool-output/` — Tool output bounding (retention: 7 days)
  - `./.m31a/planning/` — Workflow phase artifacts
  - `./.m31a/LEDGER.md` — Project-local ledger (optional)

**File Locking:**
- Custom `types.FileLock` (`.lock` files) for concurrent access safety on ledger/session files

**Caching:**
- **Model catalog cache** — In-memory with TTL (5 min fresh, 24 hr stale fallback) in `internal/integrations/provider/cache.go`
- **DNS cache** — 5-min TTL for web fetch/search (`internal/tools/search/dns_cache.go`)
- **Code intelligence cache** — On-disk incremental cache (`.m31a/codeintel/`) for Tree-sitter parsing

## Authentication & Identity

**Auth Provider:**
- **Custom / API Key based** — No OAuth, no SSO, no external identity provider
- Three independent API keys (one per provider)

**Key Storage Priority (per provider):**
1. Environment variable: `M31A_<PROVIDER>_API_KEY` (e.g., `M31A_OPENROUTER_API_KEY`)
2. Standard env var: `<PROVIDER>_API_KEY` (e.g., `OPENROUTER_API_KEY`)
3. **OS Keychain** (preferred):
   - Linux: D-Bus Secret Service (`org.freedesktop.secrets`) with `pass` CLI fallback
   - macOS: `/usr/bin/security` keychain
   - Windows: Windows Credential Manager
4. Config file (`~/.m31a/config.toml`) — **fallback only** (plaintext, warned)

**Keychain Implementation:**
- `internal/integrations/keychain/` — Platform-specific implementations via build tags
- Interface: `Get/Set/Delete(service string)`
- Service naming: `m31a/<provider>` (e.g., `m31a/openrouter`)
- Availability caching: once unavailable, stops retrying to suppress warnings

**Secrets Handling:**
- `.env` files **gitignored** (`.gitignore` + `.env.example` template only)
- API keys **never logged** — masked in errors via `maskAPIKeys()` in `provider/common.go`
- Config save: keys written to keychain first; only persisted to TOML if keychain unavailable

## Monitoring & Observability

**Error Tracking:**
- **None** — No external error tracking service (Sentry, etc.)

**Logs:**
- **Local structured logging** — `slog` with JSON handler (default) or text
- Output: `~/.m31a/m31a.log` (daily rotation, 7-day retention)
- Level: `M31A_LOG_LEVEL` env var (debug/info/warn/error, default: info)
- Format: `M31A_LOG_FORMAT` env var (json/text, default: json)

**Metrics:**
- **Local only** — `internal/integrations/metrics/collector.go`
- Per-session metrics written to `METRICS.json` in session directory
- Captures: tool executions, LLM token usage, workflow phase durations, costs
- No remote telemetry, no phone-home

**Health Checks:**
- Provider health: `/auth/key` (OpenRouter), `/models` (Zen, NVIDIA)
- Status: `live` (<500ms), `slow` (<2000ms), `offline` (error), `degraded`
- Displayed in TUI sidebar

## CI/CD & Deployment

**Hosting:**
- **GitHub Releases** — Binaries published via goreleaser
- **Homebrew tap** (`eshanized/tap/m31a`) — macOS
- **Scoop** — Windows
- **Linux packages** — .deb, .rpm, .apk, Arch Linux (via goreleaser nfpm)

**CI Pipeline:**
- **GitHub Actions** (`.github/workflows/ci.yml` inferred from Makefile targets)
- Runs: `make check` (fmt → tidy → vet → lint → test)
- Cross-compilation: `make cross` (linux/amd64,arm64; darwin/amd64,arm64; windows/amd64)
- Race detector enabled in tests (`make test` uses `-race`)
- Coverage target: 75% overall, 90% for `pkg/taskrunner`, `pkg/bisect`, `pkg/rollback`

**Release:**
- `goreleaser` — Automated on tag push
- Artifacts: binaries, checksums, packages, Homebrew formula, Scoop manifest
- Changelog generated from conventional commits

## Environment Configuration

**Required Environment Variables:**
| Variable | Purpose | Source |
|----------|---------|--------|
| `OPENROUTER_API_KEY` | OpenRouter authentication | Keychain / .env / env |
| `ZEN_API_KEY` | Zen authentication | Keychain / .env / env |
| `NVIDIA_API_KEY` | NVIDIA NIM authentication | Keychain / .env / env |

**Optional Environment Variables:**
| Variable | Purpose | Default |
|----------|---------|---------|
| `M31A_CONFIG` | Override global config path | `~/.m31a/config.toml` |
| `M31A_THEME` | UI theme override | config value |
| `M31A_DEFAULT_MODEL` | Default model ID | config value |
| `M31A_PROVIDER` | Default provider | config value |
| `M31A_PERMISSION_MODE` | Default permission mode | config value |
| `M31A_COMPACT` | Enable compact UI mode | `false` |
| `M31A_LOG_LEVEL` | Log level (debug/info/warn/error) | `info` |
| `M31A_LOG_FORMAT` | Log format (json/text) | `json` |

**Secrets Location:**
1. **OS Keychain** (primary) — `m31a/openrouter`, `m31a/zen`, `m31a/nvidia`
2. **Environment variables** (CI/ephemeral) — `OPENROUTER_API_KEY`, etc.
3. **Config file** (fallback) — `~/.m31a/config.toml` (plaintext, warned)
4. **`.env` file** (local dev) — gitignored, loaded at startup via `config.LoadDotEnv()`

## Webhooks & Callbacks

**Incoming Webhooks:**
- **None** — M31A is a CLI/TUI tool, not a server. No webhook endpoints.

**Outgoing Webhooks/Callbacks:**
- **None** — No outbound webhook delivery.
- LLM providers use standard request/response (streaming via SSE)
- Git operations use local `git` CLI (no GitHub/GitLab API calls)

## Git Integration

**Git Operations:**
- **Local `git` binary** — All git operations via `exec.Command("git", ...)`
- Wrapper: `internal/integrations/git/git.go` (`types.GitClient` interface)
- Features: commit, diff, log, status, branch, stash, reset, tag, push/pull/fetch
- Security: sensitive file detection (`.env`, keys, credentials) blocks auto-commit
- Rollback: `internal/engine/rollback/rollback.go` uses git reset + backup branches
- Subagents: `internal/tools/subagent/` uses `git worktree` for isolated workspaces

**Git Hosting:**
- Works with any git remote (GitHub, GitLab, Bitbucket, self-hosted, etc.)
- No provider-specific API integration

## External Tool Dependencies

**Required at Runtime:**
| Tool | Purpose | Fallback |
|------|---------|----------|
| `git` | Version control, rollback, worktrees | Required (hard dependency) |
| `bash` / `sh` | Shell command execution | Required (Windows: Git Bash/MSYS2) |

**Optional at Runtime:**
| Tool | Purpose | Fallback |
|------|---------|----------|
| `pass` | Linux keychain fallback (GPG-backed) | D-Bus Secret Service primary |
| `security` | macOS keychain CLI | Built-in |
| `cmdkey` | Windows Credential Manager | Built-in |

**Build-Time Only:**
- `go` 1.25+ — Compiler
- `golangci-lint` — Linting
- `goreleaser` — Release packaging
- `make` — Build orchestration

---

*Integration audit: 2026-07-23*
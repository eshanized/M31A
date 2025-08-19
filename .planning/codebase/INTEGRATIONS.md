# External Integrations

**Analysis Date:** [YYYY-MM-DD]

## APIs & External Services

**LLM Providers:**
- OpenRouter API (`https://openrouter.ai/api/v1`) - Primary model gateway, fetching models, chat completion streaming.
  - SDK/Client: Custom HTTP Client (`internal/provider/openrouter/client.go`)
  - Auth: API Key (`Authorization: Bearer`)
  - Additional Headers: `HTTP-Referer`, `X-Title`
- Zen API (`https://opencode.ai/zen/v1`) - Secondary model gateway, optimized for specific models.
  - SDK/Client: Custom HTTP Client (`internal/provider/zen/client.go`)
  - Auth: API Key (`Authorization: Bearer`)

## Data Storage

**Databases:**
- None (Local filesystem only)

**File Storage:**
- Local filesystem only
  - Ledger: Stores session history in a markdown table at `~/.m31a/LEDGER.md` (`pkg/ledger/ledger.go`)
  - Sessions: JSON-based session history stored in `~/.m31a/sessions/<id>/session.json` and `messages.json` (`pkg/session/manager.go`)
  - Recent Models: JSON state in `~/.m31a/recent_models.json` (`pkg/session/manager.go`)

**Caching:**
- None external (In-memory model cache implemented in `internal/provider/cache.go`)

## Authentication & Identity

**Auth Provider:**
- Custom (Configuration file)
  - Implementation: User provides provider API keys in TOML config, stored securely on the local machine and passed via HTTP headers.

## Monitoring & Observability

**Error Tracking:**
- None (Local logs only)

**Logs:**
- Standard Go `log/slog` for structured logging (`internal/log/log.go`, `internal/provider/openrouter/client.go`, `internal/provider/zen/client.go`).

## CI/CD & Deployment

**Hosting:**
- Local executable (CLI/TUI)

**CI Pipeline:**
- GitHub Actions (`.github/workflows/ci.yml`) - Runs tests and linting.
- GoReleaser (`.goreleaser.yaml`) - Automated binary builds and releases.

## Environment Configuration

**Required env vars:**
- None strictly required (configuration typically via TOML file containing `api_key` for providers).

**Secrets location:**
- Stored locally in the user's configuration TOML file (parsed by `internal/config/types.go`).

## Webhooks & Callbacks

**Incoming:**
- None

**Outgoing:**
- None

---

*Integration audit: [YYYY-MM-DD]*
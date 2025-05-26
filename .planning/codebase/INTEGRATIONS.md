# External Integrations

**Analysis Date:** 2026-06-02

## APIs & External Services

### LLM Provider Gateways

**OpenRouter** (`https://openrouter.ai/api/v1`)
- **What it's used for:** Primary LLM gateway — chat completions (streaming), model catalog, auth validation, health checks.
- **SDK/Client:** Custom Go client at `internal/provider/openrouter/client.go` (uses only `net/http`).
- **Endpoints used:**
  - `GET /models` — model catalog (`openrouter/client.go:118-170`)
  - `POST /chat/completions` — streaming chat (SSE) (`openrouter/client.go:179-237`)
  - `GET /auth/key` — API key validation / health check (`openrouter/client.go:277-305`)
- **Auth:** Bearer token in `Authorization` header. Key resolved via `M31A_OPENROUTER_API_KEY` env → keychain → config file (`internal/config/loader.go:510-521`).
- **Request headers:** `Authorization`, `User-Agent: M31A/<version>`, `HTTP-Referer` (default `https://github.com/eshanized/M31A`), `X-Title: M31A`, `Content-Type: application/json`.
- **Custom base URL:** `provider.openrouter_base_url` in config TOML (for self-hosted or proxied gateways).

**OpenCode Zen** (`https://opencode.ai/zen/v1`)
- **What it's used for:** Secondary LLM gateway — OpenAI-compatible API, models, streaming chat, health check.
- **SDK/Client:** Custom Go client at `internal/provider/zen/client.go` (uses only `net/http`).
- **Endpoints used:**
  - `GET /models` — model catalog (`zen/client.go:104-156`) — doubles as health check.
  - `POST /chat/completions` — streaming chat (SSE) (`zen/client.go:165-228`).
- **Auth:** Bearer token. Key resolved via `M31A_ZEN_API_KEY` env → keychain → config file (`internal/config/loader.go:524-533`).
- **Request headers:** `Authorization`, `User-Agent: M31A/<version>`, `Content-Type: application/json`. Intentionally omits `HTTP-Referer` and `X-Title` (`zen/client.go:194-197`).
- **Custom base URL:** `provider.zen_base_url` in config TOML.

**Common abstraction:** `internal/provider/interface.go:9-16` defines `LLMProvider` interface (`Name`, `FetchModels`, `ChatCompletionStream`, `EstimateCost`, `HealthCheck`, `GetModel`). Implemented by both clients (compile-time check at `openrouter/client.go:23` and `zen/client.go:23`).

**Direct provider support explicitly prohibited** (per `AGENTS.md` "Absolute Prohibitions"): no Anthropic-only or OpenAI-only clients.

### Reasoning-Token Normalization

Both providers funnel through `internal/provider/reasoning.go:19-45` which holds a `reasoningParamMap` for model families: `deepseek`, `openai/o-`, `anthropic`, `qwen`. `ApplyReasoningParams` injects the correct request body params (e.g. `reasoning_effort: "medium"` for OpenAI o-series, `thinking.budget_tokens: 1024` for Anthropic); `ParseSSEChunk` extracts `delta.reasoning_content` / `delta.reasoning` to emit `StreamChunk{Type: "thinking"}`.

### Provider Registry & Auto-Fallback

- `internal/provider/registry.go` — thread-safe `Registry` of providers with `Active()`, `SetActive(name)`, `Get(name)`, `List()`, `ActiveProvider()`.
- `internal/provider/fallback.go:16-53` — `FindFallbackProvider` runs a 10s health check on each non-active provider; on `live` or `slow` switches `active` and returns a `FallbackEvent{From, To, Reason}`.
- Triggered on `ErrRateLimited` (HTTP 429) or `ErrProviderUnreachable` (HTTP 503) in clients.

## Data Storage

**No databases. No external storage services. All state is on local disk under `~/.m31a/`.**

### Local Filesystem

**Configuration:**
- `~/.m31a/config.toml` — single TOML file (global). Parsed/written by `internal/config/loader.go`.
- `<cwd>/m31a.toml` — optional project-level overlay (max 3 parent dirs), merged into global (`internal/config/loader.go:95-109`, `:114-242`).
- Atomic writes: `.m31a_tmp_<8-hex-bytes>` + `os.Rename` (`internal/config/loader.go:540-575`).

**Session state** (`pkg/session/manager.go`, `pkg/session/planning.go`):
- `~/.m31a/sessions/<id>/session.json` — `Session` struct serialized as JSON.
- `~/.m31a/sessions/<id>/messages.json` — full message history.
- `~/.m31a/sessions/<id>/planning/PROJECT.md` — goal, project type, framework, discuss Q&A.
- `~/.m31a/sessions/<id>/planning/TASKS.md` — task table with status, deps, files, commit hashes.
- `~/.m31a/sessions/<id>/planning/STATE.md` — current phase, progress, last action, timestamp.
- `~/.m31a/sessions/<id>/TODO.md` — written by `TodoWrite` tool.
- `~/.m31a/sessions/<id>/backups/<filename>.<timestamp>.<hex>` — pre-overwrite backup.
- `~/.m31a/sessions/<id>/checkpoints/checkpoint-N.json` — last 2 state snapshots.
- `~/.m31a/sessions/<id>/archives/` — post-Ship archive directory.

**Cross-session state:**
- `~/.m31a/LEDGER.md` — append-only Markdown ledger of completed sessions (`pkg/ledger/ledger.go`).

**Logs:**
- `~/.m31a/m31a.log` — current day's JSON-line log.
- `~/.m31a/m31a.log.YYYY-MM-DD` — rotated daily. 7-day retention (`internal/log/log.go:58-113`).

**Caching:**
- In-memory model catalog: `internal/provider/cache.go` (`ModelCache` struct). TTL 5 min, stale TTL 24h, RWMutex-protected.
- No on-disk model cache (everything live in process memory).
- No HTTP cache (no `Cache-Control` handling).

## Authentication & Identity

**Auth model:** M31A itself has no users or accounts. The only authentication concerns are outbound API keys to LLM providers and OS-level secret storage for those keys.

### API Key Resolution

`internal/config/loader.go:508-535` (`Config.ResolveAPIKeys`):
1. `M31A_OPENROUTER_API_KEY` / `M31A_ZEN_API_KEY` env var
2. OS keychain (`m31a/openrouter`, `m31a/zen`)
3. `provider.openrouter.api_key` / `provider.zen.api_key` field in config TOML

Keys are never written back to the TOML file (`loader.go:476-477` blanks them on `Save`).

### OS Keychain (`pkg/keychain/`)

Cross-platform secret storage. `Keychain` interface (`pkg/keychain/keychain.go:7-22`): `Get(service) (string, error)`, `Set(service, value string) error`, `Delete(service) error`. Service name must match `^[a-z]+$` (validated before any CLI invocation to prevent command injection).

**Linux** — `pkg/keychain/keychain_linux.go` (build tag `//go:build linux`):
- **Primary:** freedesktop Secret Service over **D-Bus session bus** via `github.com/godbus/dbus/v5`. Calls `org.freedesktop.Secret.Service.SearchItems`, `OpenSession`, `CreateItem`, `GetSecret`, `SetSecret`, `Delete`.
- **Fallback:** `pass` CLI (`pass show`, `pass insert -U -f`, `pass rm -f`) — used when D-Bus returns `ErrClosed` / "connection refused" / "no such file or directory" (`keychain_linux.go:274-301`).
- Bus: `org.freedesktop.secrets` at `/org/freedesktop/secrets`. Default collection `/org/freedesktop/secrets/aliases/default`. Item label: `M31A API Key: <service>`.
- `keychain_linux.go:33-38` — `validateService` rejects service names that don't match `^[a-z]+$`.

**macOS** — `pkg/keychain/keychain_darwin.go` (build tag `//go:build darwin`):
- Uses **`/usr/bin/security`** CLI (no Go bindings; no CGO).
- `security find-generic-password -s m31a/<svc> -wa m31a` (`keychain_darwin.go:41-46`).
- `security add-generic-password -U -s m31a/<svc> -a m31a -w <value>` (`keychain_darwin.go:62-71`).
- `security delete-generic-password -s m31a/<svc> -a m31a` (`keychain_darwin.go:79-86`).
- Account name constant: `m31a`.

**Windows** — `pkg/keychain/keychain_windows.go` (build tag `//go:build windows`):
- Direct Win32 calls via `syscall.NewLazyDLL("advapi32.dll")` — no CGO.
- `CredReadW` / `CredWriteW` / `CredDeleteW` / `CredFree` (`keychain_windows.go:120-126`).
- Target name: `m31a/<service>`. Persistence: `CRED_PERSIST_LOCAL_MACHINE` (`keychain_windows.go:71`). Type: `CRED_TYPE_GENERIC` (`keychain_windows.go:72`).
- `syscall.UTF16PtrFromString` to encode the target and value.

**Keychain errors** (`pkg/keychain/errors.go`): `ErrKeychainUnavailable`, `ErrKeyNotFound`, `ErrNotImplemented`. All comparisons via `errors.Is` (`internal/config/loader.go:515, 529`).

**No third-party identity providers** (no OAuth, no SSO, no JWT validation, no SAML — the CLI has no notion of a "logged-in user").

## Monitoring & Observability

**Error tracking:** None. No Sentry, no Rollbar, no Bugsnag, no equivalent.

**Logs:**
- Only `~/.m31a/m31a.log` (slog JSON-lines by default). No stdout/stderr during TUI operation (mandated by `AGENTS.md` "no telemetry" rule and confirmed in `cmd/m31a/main.go:64-69`).
- Application event log: `~/.m31a/m31a.log` with INFO/WARN/ERROR only in production. `M31A_LOG_LEVEL=debug` adds debug records.
- TUI user-facing errors are rendered via styled Bubble Tea messages (`internal/tui/components/permission.go`, `internal/tui/app.go`).

**Health checks** (in-process, no external monitor):
- Each LLM provider has `HealthCheck(ctx)` returning `types.HealthStatus{Status, LatencyMs, Error}` (`internal/types/types.go:162-166`).
- OpenRouter: `GET /auth/key`, classifies `live` (<2000ms) / `slow` (<5000ms) / `degraded` / `offline`.
- Zen: `GET /models`, same latency tiers.
- TUI health ticker polls every 60s (`internal/tui/health.go`).

**No APM, no tracing, no metrics export.**

## CI/CD & Deployment

**Hosting:** N/A — M31A is a CLI binary distributed via GitHub Releases; no hosted service.

**CI Pipeline:** `.github/workflows/ci.yml`
- Triggers: push to `main`/`dev`, PR to `main`, `workflow_dispatch`.
- Jobs:
  - `lint` — `golangci/golangci-lint-action@v4`, version `latest`, timeout 5m.
  - `test` — `go test -race -coverprofile=coverage.out -covermode=atomic ./...`. Coverage uploaded as artifact.
  - `build` — matrix: `ubuntu-latest`/`macos-latest`/`windows-latest` × `amd64`/`arm64`; `windows/arm64` excluded. `CGO_ENABLED=0` set per step. `go vet` after build.
  - `release` — gated on `startsWith(github.ref, 'refs/tags/v')`. `goreleaser/goreleaser-action@v5` with `release --clean`.

**Release artifact** (`.goreleaser.yaml`):
- `m31a_<version>_<os>_<arch>.tar.gz` (`.zip` on Windows).
- `checksums.txt` with `sha256` algorithm.
- LDFLAGS: `-s -w -X main.Version={{.Version}}`.
- Draft release; `prerelease: auto`.

**Install path** (`install.sh`):
- `curl -fsSL https://raw.githubusercontent.com/eshanized/M31A/main/install.sh | bash` (default).
- Resolves latest tag via `https://api.github.com/repos/eshanized/M31A/releases/latest`.
- Downloads `<repo>_<version>_<os>_<arch>.<ext>`, verifies `shasum -a 256` against `checksums.txt`, extracts to `/usr/local/bin/m31a` (overridable via `BIN_DIR`).
- OS detection: `uname -s` (Linux/Darwin/CYGWIN/MINGW/MSYS). Arch: `uname -m` (`x86_64`→`amd64`, `aarch64`/`arm64`→`arm64`).

## Environment Configuration

**Required env vars:** None are strictly required — M31A boots without keys and prompts via the First-Run screen (`internal/tui/firstrun.go`).

**Common env vars** (full list in `STACK.md`):
- `M31A_CONFIG`, `M31A_LOG_FORMAT`, `M31A_LOG_LEVEL`, `M31A_THEME`, `M31A_DEFAULT_MODEL` — UI/runtime.
- `OPENROUTER_API_KEY` / `M31A_OPENROUTER_API_KEY` — OpenRouter key (two names; the `M31A_`-prefixed one is the canonical resolver in `internal/config/loader.go:510`).
- `ZEN_API_KEY` / `M31A_ZEN_API_KEY` — Zen key (`internal/config/loader.go:524`).

**Secrets location:**
1. **Preferred:** environment variables (not persisted anywhere on disk by M31A itself).
2. **OS keychain** — see `Authentication & Identity` above.
3. **Config TOML** — last-resort fallback. `api_key` field in `~/.m31a/config.toml`. Written by user via `internal/tui/settings.go` Settings screen. Never auto-written by `Config.Save()`.

**What M31A does NOT touch:**
- No calls to any other API beyond OpenRouter and Zen.
- No analytics, no telemetry, no phone-home.
- No package-update checks.
- No usage reporting to a central server.

## External Tooling (System Binaries)

M31A shells out to these binaries; they are not API integrations but are part of its external surface:

| Binary | Tool | Detection | Fallback |
|--------|------|-----------|----------|
| `git` | `internal/git/git.go` — wraps `git init`, `add`, `commit`, `log`, `diff`, `status`, `reset --soft/--hard`, `branch`, `rev-parse`, `stash push/pop` | `git rev-parse --git-dir` succeeds | None — required for Execute phase |
| `rg` (ripgrep) | `internal/tools/grep.go`, `internal/tools/glob.go` | `exec.LookPath("rg")` | Pure Go `bufio.Scanner` + `doublestar`; for Glob, falls back to `doublestar.Glob(os.DirFS(...))` |
| `pass` (Linux) | `pkg/keychain/keychain_linux.go` — `pass show`, `pass insert -U -f`, `pass rm -f` | CLI exit behavior | D-Bus Secret Service (preferred path) |
| `/usr/bin/security` (macOS) | `pkg/keychain/keychain_darwin.go` | Always present on macOS | None |
| `taskkill` (Windows) | `internal/tools/bash_windows.go:24` — `taskkill /F /PID <pid>` | Always present on Windows | None |
| `bash` (Unix) | `internal/tools/bash_unix.go:25-27` | `$PATH` lookup via `exec.CommandContext` | None (Windows uses `cmd /C` instead) |
| `cmd` (Windows) | `internal/tools/bash_windows.go:28-30` | Always present | None (Unix uses `bash -c`) |

**No PTY allocation** — `bash -c`/`cmd /C` are called directly via `exec.CommandContext`, no `creack/pty` (despite being mentioned in `AGENTS.md`'s "Key Go Dependencies" — it is not actually a runtime dependency, the Bash tool uses plain pipes captured to `io.Pipe` in `internal/tools/bash.go:70-148`).

**No HTTP server / listening port** — M31A is a pure client; it only makes outbound HTTP requests.

## Webhooks & Callbacks

**Incoming webhooks:** None. M31A does not run a server, has no HTTP listener, no port binding.

**Outgoing webhooks:** None beyond the LLM provider API calls enumerated above. The `/webhook` and similar slash commands are **not** present — only the documented set in `cmd/m31a/main.go:42-46` (`/help`, `/clear`, `/status`, `/model`, `/provider`, `/reset`, `/quit`, `/undo`, `/compress`, `/ledger`, `/rollback`, `/sessions`, `/goal`, `/phase`, `/config`, `/models`, `/fallback`).

**In-process callbacks:**
- `tools.PermissionRequest` / `tools.PermissionResponse` channels — between dispatcher and TUI permission modal (`internal/tools/dispatcher.go:38-39`).
- `tools.QuestionRequest` / `tools.QuestionResponse` channels — between `AskUserQuestion` tool and TUI (`internal/tools/question.go:14-23`).
- `tea.Msg` / `tea.Cmd` — Bubble Tea message bus; e.g. `StreamChunkMsg`, `HealthCheckTickMsg`, `PermissionTickMsg`, `FallbackEventMsg`, `RefreshCacheMsg`, `ModelSelectedMsg`, `PlanReadyMsg`, `PhaseResultMsg`, `QuestionRequestMsg`, `SettingsSavedMsg`, `ThemeChangedMsg`, `SlashCommandMsg`, `ToastMsg` (`internal/tui/types.go`).

## Integration Summary

| Category | Service / API | Auth | Used By |
|----------|---------------|------|---------|
| LLM Gateway | OpenRouter (`https://openrouter.ai/api/v1`) | Bearer token | `internal/provider/openrouter/client.go` |
| LLM Gateway | OpenCode Zen (`https://opencode.ai/zen/v1`) | Bearer token | `internal/provider/zen/client.go` |
| Local Secret Store | freedesktop Secret Service (Linux, via D-Bus) | OS session | `pkg/keychain/keychain_linux.go` |
| Local Secret Store | macOS Keychain Services | OS session | `pkg/keychain/keychain_darwin.go` |
| Local Secret Store | Windows Credential Manager | OS session | `pkg/keychain/keychain_windows.go` |
| Local Secret Store | `pass` CLI (Linux fallback) | OS user | `pkg/keychain/keychain_linux.go` |
| VCS | `git` (CLI) | Filesystem permissions | `internal/git/git.go` |
| Search | `rg` (ripgrep, optional) | Filesystem permissions | `internal/tools/grep.go`, `internal/tools/glob.go` |
| Distribution | GitHub Releases | Public + optional `GITHUB_TOKEN` for upload | `.github/workflows/ci.yml`, `.goreleaser.yaml`, `install.sh` |
| Package Mgmt | `go mod` / `go.sum` | N/A | `go.mod` |

**Total external API surfaces: 2** (OpenRouter, Zen). **Total external binaries: 5** (git, rg, pass, security, taskkill) plus platform shell (`bash`/`cmd`).

---

*Integration audit: 2026-06-02*

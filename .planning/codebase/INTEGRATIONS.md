---
focus: tech
last_mapped: 2026-05-30
---

# M31A — External Integrations

## LLM Providers

### OpenRouter (`internal/provider/openrouter/`)

- **Base URL**: `https://openrouter.ai/api/v1`
- **Auth**: Bearer token via `X-OpenRouter-Api-Key` header
- **Endpoints used**:
  - `GET /models` — fetch model catalog (5-minute TTL cache)
  - `POST /chat/completions` with `stream: true` — streaming chat
- **Streaming**: SSE line-by-line via `bufio.Scanner`. No body read timeout on HTTP client (streaming is unbounded).
- **Error handling**: 401 → `ErrInvalidKey`, 429 → `ErrRateLimited`, 503 → `ErrProviderUnreachable`
- **30s dial-only timeout** via `net.Dialer`. No body read timeout.

### OpenCode Zen (`internal/provider/zen/`)

- **Base URL**: `https://opencode.ai/zen/v1`
- **Auth**: Bearer token via `Authorization` header
- **Endpoints used**:
  - `GET /models` — catalog fetch (same 5-minute TTL cache as OpenRouter)
  - `POST /chat/completions` — streaming chat
- **Structure**: OpenAI-compatible API. Same SSE parsing as OpenRouter.
- **Health check**: Uses `GET /models` (lightweight auth validation)

### Provider Registry (`internal/provider/registry.go`)

- Thread-safe map of name → `LLMProvider`
- `SetActive()` / `Active()` / `Get()` / `List()` methods
- Auto-fallback: on `ErrRateLimited` or `ErrProviderUnreachable`, switch to the other provider if `auto_fallback` enabled in config
- `FallbackEvent` emitted so TUI can display a banner

## OS Keychain (`pkg/keychain/`)

- **Interface**: `Get(service string)`, `Set(service, key string)`, `Delete(service string)`
- **Linux**: freedesktop Secret Service via `godbus/dbus`
- **macOS**: Keychain Services (compile-tag isolated)
- **Windows**: Credential Manager (compile-tag isolated)
- **Fallback**: env var → config file when keychain unavailable

## OS Shell (Bash Tool)

- **PTY allocation**: via `creack/pty` on Linux/macOS
- **Plain pipes**: fallback on Windows (where PTY is unavailable)
- **30-minute absolute timeout** via `context.WithTimeout`
- **Signal forwarding**: `SIGINT` propagates to child process
- Working directory: user's `cwd`

## No External Integrations

- **No telemetry**: no analytics, no phone-home
- **No database**: all state stored as files (`~/.m31a/sessions/`)
- **No webhooks**: all communication is SSH-style terminal session
- **No cloud storage**: files remain local
- **No external auth providers**: self-contained keychain resolution

## Filesystem Integration

- **Session storage**: `~/.m31a/sessions/<8-char-id>/`
  - `session.json` — metadata
  - `messages.json` — full message history
  - `planning/PROJECT.md`, `TASKS.md`, `STATE.md` — markdown state files
  - `checkpoints/` — state snapshots (last 2 retained)
  - `backups/` — pre-overwrite file backups
  - `archives/` — post-ship archived sessions
- **Config**: `~/.m31a/config.toml`
- **Log**: `~/.m31a/m31a.log`
- **Ledger**: `~/.m31a/LEDGER.md`

## Git Integration

- Uses native `git` CLI via `os/exec` (`internal/git/git.go`)
- Automatic `git init` if cwd not a git repo (initialize phase)
- `git add -A && git commit` per completed task (execute phase)
- `git bisect` for regression detection (verify phase)
- `git log`, `git diff`, `git reset --soft`/`--hard` for rollback

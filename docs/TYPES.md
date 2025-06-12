# M31A — Types Reference

## Environment Variables

| Variable | Description | Default | Resolution Order |
|----------|-------------|---------|-----------------|
| `M31A_OPENROUTER_API_KEY` | OpenRouter API key | — | 1st (env var) |
| `M31A_ZEN_API_KEY` | OpenCode Zen API key | — | 1st (env var) |
| `M31A_LOG_FORMAT` | Log output format | `json` | — |
| `M31A_LOG_LEVEL` | Log verbosity level | `info` | — |

### Key Resolution Order

API keys are resolved in the following order (first hit wins):

1. Environment variable (`M31A_OPENROUTER_API_KEY` / `M31A_ZEN_API_KEY`)
2. OS keychain (secret-service on Linux, Keychain on macOS, Credential Manager on Windows)
3. Config file (`~/.m31a/config.toml`)

**Never store API keys in plaintext.** The config file `api_key` field is the last resort
fallback. Always prefer env vars or keychain storage.

---

## Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `ModelCacheTTL` | `5 * time.Minute` | How long model catalogs are cached before refresh |
| `HealthCheckInterval` | `60 * time.Second` | Default polling interval for provider health checks |
| `MaxFileSize` | `5 * 1024 * 1024` | Maximum file size for FileRead (5MB) |
| `MaxToolOutputChars` | `10_000` | Caps tool output in the TUI (longer outputs truncated) |
| `MaxHealAttempts` | `2` | Maximum self-heal retries for a failed task |
| `MaxPlanRetries` | `3` | Maximum plan regeneration attempts on validation failure |
| `SessionIDLength` | `8` | Characters in a session ID (lowercase alphanumeric) |
| `AutoDreamThreshold` | `0.60` | Context usage % that triggers AutoDream consolidation |
| `ContextWarningThreshold` | `0.80` | Context usage % that shows warning banner |
| `HTTPDialTimeout` | `30 * time.Second` | Dial timeout for provider HTTP connections |
| `BashTimeout` | `30 * time.Minute` | Absolute timeout for Bash tool execution |
| `BashOutputLimit` | `50_000` | Max characters to stream from Bash output to TUI |
| `DefaultContextLength` | `128_000` | Fallback context length if model metadata unavailable |

---

## Sentinel Errors

| Error | Triggered When |
|-------|---------------|
| `ErrProviderUnreachable` | HTTP request to LLM provider fails (network error, DNS, connection refused) |
| `ErrRateLimited` | HTTP 429 response from provider |
| `ErrInvalidKey` | HTTP 401 response from provider (invalid or expired API key) |
| `ErrContextExceeded` | Estimated tokens exceed 95% of model's context window |
| `ErrModelNotFound` | Requested model ID is not in the provider's model cache |
| `ErrSessionCorrupted` | Session files cannot be parsed or are missing required fields |
| `ErrNoBinaryContent` | FileRead detects binary content (not displayable in terminal) |
| `ErrFileTooLarge` | FileRead target exceeds 5MB |
| `ErrCircularDependency` | Task graph contains a cycle during topological sort |
| `ErrBisectFailed` | Git bisect could not identify the offending commit |
| `ErrPermissionDenied` | Tool execution blocked by permission gate |
| `ErrToolExecution` | Tool implementation returned an error |
| `ErrTaskFailed` | A task completed with failure status |
| `ErrPhaseTransition` | Invalid phase transition requested |
| `ErrCheckpointNotFound` | Requested checkpoint does not exist |

---

## Enum Types

### RiskLevel

| Value | Meaning |
|-------|---------|
| `safe` | No side effects (FileRead, Glob, Grep) |
| `medium` | Side effects but non-destructive (FileWrite) |
| `dangerous` | Can modify system state (Bash, FileWrite with pattern) |
| `destructive` | Can destroy data irreversibly (Bash with `rm`, `dd`, etc.) |

### WorkflowPhase

| Value | Order | Description |
|-------|-------|-------------|
| `idle` | 0 | No active workflow |
| `initialize` | 1 | Create session, detect project type |
| `discuss` | 2 | Generate clarifying questions, capture answers |
| `plan` | 3 | Generate task list with dependencies |
| `execute` | 4 | Run tasks in dependency order |
| `verify` | 5 | Run acceptance checks, self-heal failures |
| `ship` | 6 | Final commit, archive session, update ledger |

### TaskStatus

| Value | Meaning |
|-------|---------|
| `pending` | Not yet started |
| `running` | Currently executing |
| `done` | Completed successfully |
| `failed` | Completed with errors |
| `skipped` | Skipped (dependency failed or user skipped) |
| `unrecoverable` | Failed after max heal attempts |

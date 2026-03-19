# M31A Performance Audit Report

**Date:** 2026-06-12
**Auditor:** Deep codebase analysis
**Scope:** All Go source files across `cmd/`, `internal/`, and `pkg/`

---

## Executive Summary

This report identifies **32 distinct performance issues** across the M31A codebase, categorized by severity and subsystem. The most impactful issues involve O(N²) string processing in hot paths, missing concurrency in the task runner, unbounded allocations in the streaming pipeline, and redundant filesystem I/O during session management.

---

## Critical Issues (P0 — High Impact)

### PERF-01: Task Runner Executes Dependency Groups Sequentially

**File:** `pkg/taskrunner/runner.go:139`
**Subsystem:** Task Execution Engine

`ExecuteGroup` runs all tasks within a group **sequentially** despite groups representing tasks with no inter-dependencies. The `Schedule()` method correctly identifies independent tasks via Kahn's algorithm and places them in the same group, but `ExecuteGroup` iterates through them one by one.

```go
func (r *Runner) ExecuteGroup(ctx context.Context, group []int, fn ExecuteFunc) error {
    for _, id := range group {
        // ... executes sequentially
    }
}
```

**Impact:** For a plan with 10 independent tasks, the wall-clock time is 10× what it could be. Each task involves an LLM call (seconds to minutes), so this is the single largest throughput bottleneck.

**Fix:** Use `errgroup.Group` with `SetLimit()` to execute tasks within a group concurrently, with configurable parallelism.

---

### PERF-02: Multi-Pass HTML Processing in WebFetch

**File:** `internal/tools/webfetch.go:381-415`
**Subsystem:** WebFetch Tool

`htmlToMarkdown` performs **13 separate full-string passes** over the HTML body:

1. `stripTags(html, "script", "style")` — 2 passes with string rebuilds
2. `replaceBlockTag` × 7 — each calls `strings.ToLower(html)` on the **full** body
3. `convertLinks` — calls `strings.ToLower(html)` again
4. `replaceInlineTag` × 5 — each calls `strings.ToLower(html)` again
5. `stripAllTags` — rune-by-rune scan
6. `decodeHTMLEntities` — 6 `ReplaceAll` passes
7. `normalizeWhitespace` — rune-by-rune scan

Each `replaceBlockTag` and `replaceInlineTag` call allocates a new `strings.Builder`, calls `strings.ToLower` on the entire HTML (creating a full copy), and rebuilds the string. For a 5MB response, this means ~65MB of string allocations.

The code has a `PERF-2` comment acknowledging this but dismisses it as "acceptable for V1."

**Impact:** For large web pages (common in WebFetch usage), this causes visible latency and high GC pressure.

**Fix:** Consolidate into a single-pass HTML parser. At minimum, compute `lower := strings.ToLower(html)` once and reuse it.

---

### PERF-03: `replaceBlockTag` Recomputes `strings.ToLower` on Every Call

**File:** `internal/tools/webfetch.go:469-502`
**Subsystem:** WebFetch Tool

```go
func replaceBlockTag(html, tag, replacement string) string {
    lower := strings.ToLower(html)  // Full copy of entire HTML body
    // ...
}
```

This function is called 7 times from `htmlToMarkdown`, each time creating a full lowercase copy of the HTML body. For a 1MB response, that's 7MB of throwaway allocations just for the lowercase copies.

**Fix:** Compute lowercase once before the tag-processing loop and pass it as a parameter.

---

### PERF-04: `stripTags` Repeatedly Rebuilds String via Concatenation

**File:** `internal/tools/webfetch.go:435-467`
**Subsystem:** WebFetch Tool

```go
func stripTags(html string, tags ...string) string {
    for _, tag := range tags {
        for {
            // ...
            html = html[:start] + html[closeEnd:]  // O(N) string concat in inner loop
        }
    }
    return html
}
```

Each tag removal performs string concatenation that copies the entire remaining string. For a page with N script/style tags, this is O(N*M) where M is the average body length.

**Fix:** Use `strings.Builder` with a single pass, or collect ranges to remove and build the result once.

---

### PERF-05: Session List Loads All session.json Files on Every Call

**File:** `pkg/session/manager.go:309-405`
**Subsystem:** Session Management

`ListSessions` reads and JSON-unmarshals every `session.json` file in the sessions directory on each call. While there's a cache with a configurable TTL (default 2s), the cache is invalidated aggressively — every `NewSession`, `DeleteSession`, `ArchiveSession`, `ForkSession`, and `Cleanup` clears it.

For users with many sessions, the TUI's resume screen, sidebar, and session switching all trigger `ListSessions`, causing repeated filesystem walks.

**Impact:** With 100+ sessions, each list operation reads 100+ JSON files. The sidebar refreshes periodically, amplifying this.

**Fix:** Maintain an in-memory index file or use a lightweight summary file per session instead of loading full session.json.

---

## High-Impact Issues (P1)

### PERF-06: `convertLinks` Maintains Parallel Lowercase String Copy

**File:** `internal/tools/webfetch.go:536-608`
**Subsystem:** WebFetch Tool

```go
func convertLinks(html string) string {
    lower := strings.ToLower(html)  // Full copy
    for {
        // Mutates both `html` and `lower` in parallel
        html = html[:start] + replacement + html[closeEnd:]
        lower = lower[:start] + strings.ToLower(replacement) + lower[closeEnd:]
    }
    return html
}
```

Maintains two full copies of the HTML body throughout the entire link conversion loop. Each replacement triggers string concatenation on both copies.

**Fix:** Use byte offsets with case-insensitive matching instead of maintaining parallel copies.

---

### PERF-07: `replaceInlineTag` Maintains Parallel Lowercase String Copy

**File:** `internal/tools/webfetch.go:504-534`
**Subsystem:** WebFetch Tool

Same pattern as `convertLinks` — maintains full lowercase copy and rebuilds both strings on each tag replacement.

---

### PERF-08: Token Estimation Calls `[]rune(text)` for Every Message

**File:** `internal/tokens/estimator.go:68-81`
**Subsystem:** Token Estimation

```go
func (e *Estimator) Estimate(text string) int {
    if e.tokenizer != nil {
        estimated = len(e.tokenizer.Encode(text, nil, nil))
    } else {
        estimated = int((float64(len([]rune(text)))/4.0 + 1.0) * 1.3)
    }
}
```

The rune-based fallback converts the entire string to a `[]rune` slice, which allocates O(N) memory. `EstimateMessages` calls this for every message in the conversation. For a 1000-message conversation, this means converting every message's content to runes.

**Fix:** Use `utf8.RuneCountInString(text)` which is O(N) time but O(1) space — no allocation needed.

---

### PERF-09: Config Merge Uses Reflection for Every Field

**File:** `internal/config/loader.go:274-367`
**Subsystem:** Configuration

`mergeConfig` uses `reflect.ValueOf`, `reflect.FieldByName`, and `reflect.Kind()` for every field during project config merge. While this runs only once at startup, it involves:
- `overlay.NumField()` iterations per struct level
- `base.FieldByName(field.Name)` — O(N) field lookup per field
- `toTOMLKey` — allocates `[]rune` and `[]byte` per field name

**Impact:** Minor at startup, but the `knownConfigKeys()` function allocates a new map on every call from `WatchConfig`, which runs continuously.

**Fix:** Cache `knownConfigKeys()` in a package-level `sync.Once` variable.

---

### PERF-10: `LoadDotEnv` Called Multiple Times

**File:** `internal/config/loader.go:951-995`, `cmd/m31a/main.go:67`
**Subsystem:** Configuration

`LoadDotEnv` is called from both `main.go:67` ("Load .env before logger") and `Load()` at step 3.5 (`loader.go:123`). Each call:
1. Calls `os.Getwd()` (syscall)
2. Calls `os.Stat()` (syscall)
3. Reads and parses the entire `.env` file
4. Calls `os.Setenv()` for each line

**Fix:** Guard with `sync.Once` or remove the duplicate call.

---

### PERF-11: `ForkSession` Deep-Copies via JSON Marshal/Unmarshal

**File:** `pkg/session/manager.go:437-541`
**Subsystem:** Session Management

```go
projData, err := json.Marshal(parent.Project)
// ...
var projCopy types.ProjectState
json.Unmarshal(projData, &projCopy)
```

Deep-copies the project state by marshaling to JSON and unmarshaling back. This is a common but expensive pattern. For large project states with many tasks, this involves multiple serialization passes.

**Fix:** Use a direct struct copy or a dedicated `Clone()` method.

---

### PERF-12: `UpdateWorkflowState` Loads and Re-Saves Entire Session

**File:** `pkg/session/manager.go:265-272`
**Subsystem:** Session Management

```go
func (m *Manager) UpdateWorkflowState(id, goal string, phase types.WorkflowPhase, questions []string) error {
    session, err := m.LoadSession(id)  // Reads session.json AND messages.json
    session.SetWorkflowState(goal, phase, questions)
    return m.saveSessionAtomic(session)  // Writes session.json only
}
```

Every workflow state update reads the full session (including all messages from messages.json), modifies one field, and writes back. For sessions with thousands of messages, this is wasteful.

**Fix:** Read only session.json for metadata updates, or maintain a separate lightweight state file.

---

### PERF-13: `LoadWorkflowState` Also Loads Entire Session

**File:** `pkg/session/manager.go:278-293`
**Subsystem:** Session Management

Same issue as PERF-12 — `LoadWorkflowState` calls `LoadSession` which reads both session.json and messages.json, just to read 3 fields (goal, phase, questions).

---

### PERF-14: `SaveSession` Marshals Messages to JSON Twice

**File:** `pkg/session/manager.go:758-782`
**Subsystem:** Session Management

```go
func (m *Manager) SaveSession(s *Session) error {
    msgData, err := json.Marshal(s.Messages)     // Marshal messages
    data, err := json.Marshal(s)                  // Marshal session (includes Messages field)
    m.atomicWrite(m.sessionJSONPath(s.ID), data)  // Write session.json
    m.atomicWrite(m.messagesJSONPath(s.ID), msgData) // Write messages.json
}
```

Messages are serialized twice — once as part of `json.Marshal(s)` (session.json contains Messages) and once separately for messages.json. For large message histories, this doubles the serialization cost.

**Fix:** Exclude Messages from the session struct serialization, or write session.json with messages omitted.

---

## Medium-Impact Issues (P2)

### PERF-15: `cascadingReplace` in Edit Tool Performs 4 Full-Text Scans

**File:** `internal/tools/edit.go:321-354`
**Subsystem:** Edit Tool

`cascadingReplace` tries 4 strategies in sequence, each scanning the entire file content:
1. Exact match — `strings.Index` (O(N*M) worst case)
2. Line-trimmed — splits content into lines, iterates with trim comparison
3. Whitespace-normalized — splits + normalizes every line
4. Fuzzy anchor — computes Levenshtein distance for every candidate window

For large files, strategies 3 and 4 are particularly expensive. The Levenshtein implementation in strategy 4 allocates two `[]int` slices per call and is called for every line window.

**Fix:** Short-circuit earlier when strategies are unlikely to match (e.g., skip fuzzy for small files where exact match should work).

---

### PERF-16: `lineTrimmedReplace` Allocates `[]string` for Every Line

**File:** `internal/tools/edit.go:356-411`
**Subsystem:** Edit Tool

```go
oldTrimmed := make([]string, len(oldLines))
for i, line := range oldLines {
    oldTrimmed[i] = strings.TrimSpace(line)
}
```

Pre-computes trimmed old lines (good), but then calls `strings.TrimSpace(contentLines[i+j])` for every comparison in the inner loop. For large files, this creates many short-lived string allocations.

**Fix:** Pre-trim content lines as well, or use a comparison function that doesn't allocate.

---

### PERF-17: `whitespaceNormalizedReplace` Calls `strings.Fields` on Every Line

**File:** `internal/tools/edit.go:413-447`
**Subsystem:** Edit Tool

The `normalize` closure calls `strings.Fields(s)` which allocates a `[]string` for each line, and `strings.Join(fields, " ")` which allocates a new string. This is called for every content line in the comparison window.

**Fix:** Compare using a normalized byte representation or cache normalized forms.

---

### PERF-18: Glob Tool Stats Every Matched File

**File:** `internal/tools/glob.go:113-124`
**Subsystem:** Glob Tool

```go
for _, m := range matches {
    fi, err := os.Stat(fullPath)  // syscall per file
    fmt.Fprintf(&b, "%-50s %10d %s\n", m, fi.Size(), fi.ModTime().Format(...))
}
```

After glob matching (potentially hundreds of files), `os.Stat` is called for each match to get size and modification time. Each call is a syscall. With `MaxGlobResults` defaulting to 100, this means 100 sequential syscalls.

**Fix:** Use `os.ReadDir` (which returns `DirEntry` with pre-populated `Info`) or parallelize stat calls.

---

### PERF-19: `grepPureGo` Opens Every File Twice for Binary Detection

**File:** `internal/tools/grep.go:283-308`
**Subsystem:** Grep Tool (pure-Go fallback)

```go
f, err := os.Open(path)
header := make([]byte, 512)
n, _ := f.Read(header)
// binary detection...
f.Seek(0, 0)  // Seek back
scanner := bufio.NewScanner(f)
```

Opens each file, reads 512 bytes for binary detection, seeks back, then scans. While the seek avoids a second open, the buffer allocation (`make([]byte, 512)`) occurs for every file.

**Fix:** Use `bufio.Scanner` from the start and detect binary during scanning, or pool the header buffer.

---

### PERF-20: `loadGitignore` Called on Every Grep Execution

**File:** `internal/tools/grep.go:346-360`
**Subsystem:** Grep Tool

`loadGitignore` reads and parses `.gitignore` from disk on every grep call. For workflows that make many grep calls (common during execute phase), this means repeated filesystem reads.

**Fix:** Cache gitignore patterns with an mtime-based invalidation.

---

### PERF-21: `matchesGitignore` Performs O(P) Doublestar Match per File

**File:** `internal/tools/grep.go:362-382`
**Subsystem:** Grep Tool

```go
func matchesGitignore(path string, patterns []string, workDir string) bool {
    for _, p := range patterns {
        match, _ := doublestar.Match(p, relPath)
        // Also match against just the filename
        if !strings.Contains(p, "/") {
            match, _ = doublestar.Match(p, filepath.Base(relPath))
        }
    }
}
```

For each file in the walk, this iterates all gitignore patterns and calls `doublestar.Match` up to twice per pattern. With 50 gitignore patterns and 10,000 files, that's up to 1,000,000 pattern matches.

**Fix:** Compile gitignore patterns into a combined matcher or use a trie-based approach.

---

### PERF-22: Model Cache `Models()` Deep-Copies All Entries

**File:** `internal/provider/cache.go:119-128`
**Subsystem:** Provider Cache

```go
func (c *ModelCache) Models() map[string]*types.ModelInfo {
    c.mu.RLock()
    defer c.mu.RUnlock()
    result := make(map[string]*types.ModelInfo, len(c.models))
    for k, v := range c.models {
        cp := *v
        result[k] = &cp
    }
    return result
}
```

Deep-copies every model entry on each call. OpenRouter has 300+ models, so this creates 300+ struct copies. Called by `CachedModels` which is used in multiple hot paths (arbitrage, model selection).

**Fix:** Return read-only references when safe, or use `sync.Map` for concurrent reads.

---

### PERF-23: `buildToolDefinitions` Rebuilt for Every LLM Call

**File:** `internal/workflow/engine.go:551-570`
**Subsystem:** Workflow Engine

```go
func (e *Engine) buildToolDefinitions() []provider.ToolDefinition {
    var defs []provider.ToolDefinition
    for _, name := range e.dispatcher.List() {
        tool, ok := e.dispatcher.GetTool(name)
        // ... builds definition
        defs = append(defs, def)
    }
    return defs
}
```

Called by `streamLLM`, `streamLLMWithTools`, and `streamLLMStreaming` — before every LLM request. The tool definitions don't change during a session, so this is redundant work.

**Fix:** Build once during engine initialization and cache the result.

---

### PERF-24: `buildSystemPrompt` Concatenates Strings on Every Call

**File:** `internal/workflow/engine.go:573-581`
**Subsystem:** Workflow Engine

```go
func (e *Engine) buildSystemPrompt(extra ...string) string {
    parts := []string{e.prompts.Base}
    for _, p := range extra {
        parts = append(parts, p)
    }
    return strings.Join(parts, "\n\n---\n\n")
}
```

Called before every LLM request. The base prompt and extras (ToolUse, ExecuteTask, SelfHeal) are static — only the goal changes.

**Fix:** Cache the static portions and only concatenate the dynamic parts.

---

### PERF-25: `consumeStreamWithTools` Accumulates Full Response in Memory

**File:** `internal/workflow/engine.go:627-680`
**Subsystem:** Workflow Engine

The streaming consumer accumulates all content chunks into a `strings.Builder`. For very long LLM responses (the limit is `MaxLLMResponseBytes`), this holds the entire response in memory. While there is a size check, it only triggers after `MaxLLMResponseBytes` bytes have already been accumulated.

**Fix:** The size check already exists; consider streaming the content to a temp file for very large responses.

---

## Low-Impact Issues (P3)

### PERF-26: `parseToolCalls` Compiles Regex on Every Call

**File:** `internal/workflow/engine_parse.go:302-384`
**Subsystem:** Tool Call Parsing

```go
func (e *Engine) parseToolCalls(content string) ([]m31types.ToolCall, error) {
    blockRe := regexp.MustCompile("(?s)```(?:\\w+)?\\s*\n(.*?)```")
    // ...
}
```

`regexp.MustCompile` is called inside the function body on every invocation. Go's regex compilation is not cheap — it parses the pattern and builds an NFA.

**Fix:** Move the regex to a package-level `var` with `regexp.MustCompile` at init time.

---

### PERF-27: `stripCodeBlocks` Compiles Regex on Every Call

**File:** `internal/workflow/engine_parse.go:38-43`
**Subsystem:** Tool Call Parsing

```go
func stripCodeBlocks(content string) string {
    re := regexp.MustCompile("(?s)```(?:\\w+)?\\s*\n(.*?)```")
    return re.ReplaceAllString(content, "$1")
}
```

Same issue — regex compiled inside function body.

**Fix:** Package-level compiled regex.

---

### PERF-28: `parseQuestions` Compiles Multiple Regexes on Every Call

**File:** `internal/workflow/engine_parse.go:240-288`
**Subsystem:** Discuss Phase

Two regexes are compiled inside the function body:
```go
re := regexp.MustCompile(`(\d+)\.\s+(.+\?)`)
reFallback := regexp.MustCompile(`(\d+)\.\s+(.+)`)
```

**Fix:** Package-level compiled regexes.

---

### PERF-29: `toTOMLKey` Allocates `[]rune` for Every Field Name

**File:** `internal/config/loader.go:303-323`
**Subsystem:** Configuration

```go
func toTOMLKey(name string) string {
    runes := []rune(name)  // allocation
    var result []byte
    for i, ch := range runes {
        // ...
    }
    return string(result)
}
```

Called for every field during config merge. Since Go struct field names are always ASCII, the `[]rune` conversion is unnecessary — direct byte access on the string would work.

**Fix:** Iterate over bytes directly since field names are ASCII.

---

### PERF-30: HTTP Transport Not Shared Across Provider Clients

**File:** `internal/provider/base_client.go:37-41`
**Subsystem:** Provider HTTP

```go
HTTPClient: &http.Client{
    Transport: &http.Transport{
        DialContext: (&net.Dialer{Timeout: types.HTTPDialTimeout}).DialContext,
    },
},
```

Each provider client creates its own `http.Client` with its own `http.Transport`. Transports maintain connection pools, idle goroutines, and TLS session caches. With multiple providers (OpenRouter + Zen), this doubles resource usage.

**Fix:** Share a single `http.Transport` across provider clients.

---

### PERF-31: `knownConfigKeys` Allocates New Map on Every Call

**File:** `internal/config/loader.go:595-603`
**Subsystem:** Configuration

```go
func knownConfigKeys() map[string]bool {
    return map[string]bool{
        "provider": true, "model": true, // ...
    }
}
```

Called during config load and potentially during `WatchConfig` reloads. The map is identical every time.

**Fix:** Package-level `var` with `sync.Once`.

---

### PERF-32: Sidebar Git Status Refreshes via Shell-out

**File:** `internal/tui/sidebar.go` + `internal/git/git.go`
**Subsystem:** TUI Sidebar

The sidebar periodically refreshes by running `git status --porcelain` and `git diff --numstat HEAD` — both are full shell-outs that spawn git processes. For large repositories, these commands can take seconds.

The `StatusPorcelain` method runs two git commands sequentially:
1. `git status --porcelain`
2. `git diff --numstat HEAD`

**Impact:** UI jank during sidebar refresh on large repos.

**Fix:** Use `go-git` library for in-process git operations, or at minimum run the two commands concurrently.

---

## Architectural Performance Observations

### A1: No Connection Pooling for Provider HTTP Clients

The `http.Transport` in `BaseClient` doesn't configure `MaxIdleConns`, `MaxIdleConnsPerHost`, or `IdleConnTimeout`. Default values are used, which may not be optimal for the sustained streaming connections typical of LLM API usage.

### A2: `strings.Builder` Not Pre-Sized in Multiple Locations

Several functions use `strings.Builder` without calling `Grow()`:
- `webfetch.go:convertLinks` — string replacement without size estimation
- `edit.go:generateDiffSummary` — diff output builder
- `grep.go` — results joining

### A3: No Object Pooling for Frequent Allocations

Hot-path types like `types.ToolResult`, `types.ToolInput`, and `types.StreamChunk` are allocated on every tool call and stream chunk. `sync.Pool` could reduce GC pressure.

### A4: Sequential Session File Operations

`SaveSession` writes session.json and messages.json as two separate atomic writes. These could be pipelined (start both temp file writes concurrently, then rename both).

### A5: `readFileLimited` Uses `io.ReadAll` with `LimitReader`

**File:** `pkg/session/manager.go:88-103`

```go
limited := io.LimitReader(f, maxBytes+1)
data, err := io.ReadAll(limited)
```

`io.ReadAll` grows its buffer dynamically. For known-size files (from `os.Stat`), pre-allocating the buffer would avoid repeated reallocation.

---

## Performance Metrics Summary

| Category | Count | Estimated Impact |
|----------|-------|-----------------|
| P0 — Critical | 5 | 10-100× slowdown in specific scenarios |
| P1 — High | 9 | 2-10× slowdown, high GC pressure |
| P2 — Medium | 11 | Noticeable latency in common operations |
| P3 — Low | 7 | Minor overhead, easy wins |

## Recommended Priority

1. **PERF-01** (parallel task execution) — largest throughput gain
2. **PERF-02/03/04/06/07** (WebFetch multi-pass) — reduces allocation pressure
3. **PERF-05/12/13** (session I/O) — reduces filesystem thrashing
4. **PERF-26/27/28** (regex compilation) — easy, zero-risk wins
5. **PERF-08** (rune counting) — one-line fix with measurable impact
6. **PERF-23/24** (prompt caching) — reduces per-request overhead

---
focus: concerns
last_mapped: 2026-05-30
---

# M31A — Areas of Concern

## Technical Debt

### 1. `internal/workflow/engine.go` Size

- **797 lines** — largest file in the codebase. Performs too many responsibilities: phase routing, task parsing, JSON extraction, cycle detection, prompt loading, tool definition building, file reading, verification.
- **Suggestion**: Extract a `planner.go` (task parsing + validation), `inference.go` (LLM interaction helpers), and `verifier.go` (verification logic already partially split).

### 2. `internal/tui/app.go` Complexity

- **1056 lines** — the TUI's app state, update loop, screen routing, and workflow orchestration are all in one file.
- The `Update()` method is a single massive switch (~940 lines) handling ~20 distinct message types.
- **Suggestion**: Extract workflow phase transition logic into `workflow/transition.go`, screen message handling into per-screen files.

### 3. Task JSON Extraction Fragility

- `internal/workflow/engine.go:parseTasksFromJSON` uses regex-based markdown code block stripping + JSON array extraction.
- Brittle for nested JSON, non-standard formatting, or edge cases in LLM output.
- **Suggestion**: Could benefit from a more robust extraction approach (e.g., find outermost bracket pairs with proper nesting).

### 4. Reasoning Config via String Prefix Matching

- `internal/provider/reasoning.go` uses `strings.HasPrefix(modelID, prefix)` to detect reasoning-capable models.
- Hardcoded prefixes for deepseek, openai/o-, anthropic, qwen — requires code changes to add new model families.
- **Suggestion**: Could be driven by model metadata from API (capability flags) instead of string matching.

### 5. Tool Call Parsing Relies on Regex

- `internal/workflow/engine.go:parseToolCalls` uses multiple regex patterns to extract tool calls from LLM responses.
- Two patterns: markdown code blocks and standalone JSON objects with "name" fields.
- Brittle when LLM uses different formatting conventions.
- **Suggestion**: Could request structured output format from provider API if available.

### 6. No Graceful Handling for Missing Config File

- `internal/workflow/engine.go:detectProjectType` uses a fixed map of file → project type. Unknown types treated as "unknown".
- No user feedback if project type is unknown — silently proceeds.

## Performance Concerns

### 1. TUI Rendering on Large Message Histories

- `AppState.View()` calls `lipgloss.JoinVertical` on the entire layout on every render.
- With hundreds of messages, this could exceed the 16ms frame budget.
- Mitigation: Glamour renders are cached per-message, but no profiling has been done.

### 2. `listCwdFiles` Walks Entire Working Directory

- `internal/workflow/engine.go:listCwdFiles` uses `filepath.Walk` on the entire cwd — potentially thousands of files.
- Called during plan phase for context injection. Could be slow in large repos.
- Mitigation: Skips `.git` and `.m31a` directories, but no depth limits.

### 3. Health Check Parallelism

- Health check and cache refresh are triggered as separate goroutines. If network is slow, they could overlap.
- Currently no deduplication logic if one is already in flight.

## Security

### 1. API Key Display in Status

- `commands.go:handleStatus` shows configuration but respects key masking via `settings.go`.
- Need to audit all paths where `apiKey` might leak into View/rendering.
- Current mitigation: API keys always masked as `"••••••••"` in Settings `View()`.

### 2. Bash Tool Risk

- Bash tool has a 30-minute timeout and PTY allocation. Permission gate blocks dangerous/destructive commands.
- Permission gate uses in-memory `map[string]bool` for "remember" — cleared on restart.
- No allowlist/denylist for specific command patterns yet (V1.1 feature).

### 3. No Output Sanitization

- Tool outputs (Bash, FileRead) are rendered directly in the TUI. No sanitization for control characters or escape sequences.
- Potential terminal injection if a file contains malicious escape codes.

## Missing Features for V1.0

### 1. No WebSearch / WebFetch Tools (Deferred to V1.1)

- Current V1 tools are limited to: Bash, FileRead, FileWrite, Glob, Grep.
- WebSearch and WebFetch are critical for some use cases but deferred.

### 2. No Vision Support

- `CapFlags.Vision` field exists in `ModelInfo` but is never used.
- No image input mechanism exists in the TUI.

### 3. No Concurrent Task Execution (V1 Sequential Only)

- `pkg/taskrunner/` executes tasks one at a time within groups.
- Concurrent subagents deferred to V1.1.

### 4. No Ghost Mode

- All workflow operations happen in the main working directory.
- `git worktree` isolation for zero-risk execution deferred to V1.1.

## Fragile Areas

### 1. SSE Stream Parsing

- `internal/provider/sse.go` uses `bufio.Scanner` with line-by-line parsing.
- Relies on standard SSE newline conventions — non-standard server implementations may break.

### 2. Mock-Based Tests vs Real Provider Behavior

- Provider tests use `httptest.NewServer` with scripted responses — all async SSE streaming is mocked.
- Real-world behavior (rate limiting, intermittent errors, malformed SSE) not well tested.

### 3. Workflow Phase Auto-Advance

- `AppState.Update()` auto-advances phases (Initialize→Discuss→Plan→Execute→Verify→Ship) without user confirmation.
- If the engine returns unexpected results, the auto-advance could skip critical steps.
- Mitigation: Error results halt the chain.

### 4. Prompt Embedding

- Workflow prompts are embedded via `//go:embed prompts/*.md` into the binary at build time.
- Cannot be modified without recompiling the binary.
- No user-customizable prompt override mechanism.

## Known Issues

- **No code TODOs/FIXMEs** — the codebase has no `TODO`/`FIXME`/`HACK`/`XXX` markers in code comments. The word "TODO" appears in `internal/tools/todo.go` but only as part of the TodoWrite tool's domain logic (it writes `TODO.md` files to the session directory).
- **Recent fixes** (from git log):
  - File handle leak in permission modal (`d96c44f`)
  - Log rotation using calendar dates instead of `Truncate(24h)` (`a8baa45`)
  - Operator precedence in token fallback formula (`12fd4e4`)
  - Dead code cleanup (orphaned discuss.go code, stale ModelCacheRefreshTicker type)
  - O(n²) test loop fixed in reasoning test (`29846cd`)

# Adoptable Patterns from OpenCode for M31A

> Excludes provider integration (already covered separately). Each recommendation includes the OpenCode source, what M31A currently does, and a concrete adoption plan.

---

## 1. Automatic Session Compaction

**OpenCode source:** `packages/core/src/session/compaction.ts`

**What M31A has:** Token estimation via `internal/tokens/estimator.go` using tiktoken-go. When the context window fills up, there is no automated compaction — the session simply stops accepting new prompts or the user must manually start a new session.

**What OpenCode does:**
- Monitors total token count against model context limits before every provider turn
- When usage exceeds `context - output - buffer` (default buffer: 20,000 tokens), it auto-compacts
- Splits conversation history into a "head" (to summarize) and "recent" (to keep verbatim, default 8,000 tokens)
- Sends the head to the LLM with a structured summary template producing sections: Goal, Constraints, Progress (Done/In Progress/Blocked), Key Decisions, Next Steps, Critical Context, Relevant Files
- Stores the compaction as a special message type; subsequent compactions update the existing summary rather than starting fresh
- Tool outputs within compacted history are truncated to 2,000 characters to keep the summary lean
- Configurable via `compaction.auto`, `compaction.buffer`, and `compaction.keep.tokens`

**Adoption plan:**
1. Add a `compaction` section to M31A's YAML config with `auto: true`, `buffer: 20000`, `keep_tokens: 8000`
2. Create `pkg/compaction/` with:
   - `compaction.go` — core logic: `CompactIfNeeded(session, model) (bool, error)` that checks token usage against model limits
   - `serialize.go` — serializes messages into a compactable text format (user text, assistant text, tool calls/results, system messages)
   - `template.go` — the structured summary prompt template
3. Hook into the workflow engine before each provider call in `internal/workflow/`
4. Store compaction summaries as a special message type (`types.MessageCompaction`) in the session
5. When loading sessions, skip messages before the latest compaction and inject the summary as context

**Estimated effort:** ~2 days. The serialization layer and template are straightforward Go. The main complexity is integrating with the existing workflow phase system.

---

## 2. Tool Output Bounding & Managed Output Store

**OpenCode source:** `packages/core/src/tool-output-store.ts`, `packages/opencode/src/tool/truncate.ts`

**What M31A has:** Tool outputs are returned directly to the LLM context with no size bounding. A `grep` or `fileread` on a large file can consume the entire context window in one tool call. The grep tool has a `--max-results` flag but no automatic truncation.

**What OpenCode does:**
- Defines global limits: 2,000 lines or 50KB per tool output (configurable via `tool_output.max_lines` / `tool_output.max_bytes`)
- When output exceeds limits, writes the full output to a managed directory (`tool-output/`) with a unique filename
- Returns a bounded preview (head+tail sampling) with a marker: `"... output truncated; full content saved to <path> ..."`
- The preview preserves both the beginning and end of output, which is more useful than head-only truncation
- Managed output files have a 7-day retention period with hourly cleanup
- Truncation hints suggest using `grep` or `read` with offset/limit to inspect the full output, or delegating to a subagent

**Adoption plan:**
1. Create `internal/tools/output_store.go`:
   - `OutputStore` struct with `Bound(output string) (bounded string, fullPath string, truncated bool)`
   - Head+tail preview logic that respects both line and byte limits
   - File writer that saves to `~/.m31a/tool-output/` with ascending IDs
   - `Cleanup()` method that removes files older than 7 days
2. Add config options: `tools.output.max_lines` (default 2000), `tools.output.max_bytes` (default 51200)
3. Integrate into `internal/tools/execute.go` — after tool execution, pass output through `OutputStore.Bound()` before returning to the LLM
4. Run cleanup on app startup via the existing startup routine in `internal/tui/startup.go`

**Estimated effort:** ~1 day. The logic is self-contained and doesn't touch the workflow engine.

---

## 3. Wildcard-Based Permission Rules Engine

**OpenCode source:** `packages/core/src/permission.ts`

**What M31A has:** A simple per-tool allow/deny map in `internal/tools/permissions.go`. The `Dispatcher` tracks `permissions map[string]bool` and supports "remember" for individual approvals. Rules come from agent profile configs.

**What OpenCode does:**
- Three permission effects: `allow`, `deny`, `ask` (prompt user)
- Wildcard matching on both action and resource: `{ action: "bash", resource: "git *", effect: "allow" }`
- Rules are evaluated in order; last match wins (via `findLast`)
- Per-agent permission rulesets with agent-level defaults
- "Always" approval saves rules persistently per-project
- Pending permission requests are tracked by ID with deferred resolution
- Rejecting one permission cascades to reject all pending for that session
- Saved rules survive across sessions via a dedicated store

**Adoption plan:**
1. Extend `internal/tools/permissions.go`:
   - Add a `Rule` struct: `{ Action string, Resource string, Effect string }` where Effect is "allow"/"deny"/"ask"
   - Add `WildcardMatch(action, pattern string) bool` using the existing `doublestar` dependency
   - Replace the `map[string]bool` with `[]Rule` evaluated last-match-wins
2. Update config schema (`internal/config/config.go`):
   - Add `permission_rules` as a list of `{action, resource, effect}` entries
   - Agent profiles already have rules; extend them to the new format
3. Add persistent "always" approval storage:
   - Save to `~/.m31a/permissions.json` scoped by project directory
   - Load on session start, merge with configured rules
4. The "ask" effect replaces the current blanket prompt — only ask when no explicit allow/deny matches

**Estimated effort:** ~2 days. The wildcard matching infrastructure already exists via `doublestar`.

---

## 4. Retry Policy with Exponential Backoff & Provider Headers

**OpenCode source:** `packages/opencode/src/session/retry.ts`

**What M31A has:** Provider fallback via `internal/provider/fallback.go` switches to a fallback model when the primary fails. No structured retry with backoff on the same model. SSE streaming has basic retry in `internal/provider/sse.go`.

**What OpenCode does:**
- Exponential backoff: `initial_delay * backoff_factor^(attempt-1)`, capped at 30s (or max int32 with headers)
- Respects `retry-after` and `retry-after-ms` HTTP response headers
- Parses HTTP date format in `retry-after` headers
- Classifies errors: context overflow (never retry), 5xx (always retry), rate limits (retry with backoff), overloaded (retry)
- Surfaces retry state to the UI with human-readable messages and actionable links
- Rate limit pattern detection in error messages: "rate limit", "too many requests", "rate increased too quickly"

**Adoption plan:**
1. Create `pkg/retry/policy.go`:
   - `Delay(attempt int, headers map[string]string) time.Duration`
   - `IsRetryable(err error) (bool, string)` — classifies errors and returns a human-readable reason
   - `Retry(fn func() error, maxAttempts int) error` — retry loop with context support
2. Integrate into `internal/provider/` at the streaming layer:
   - Before falling back to the next provider, retry the current provider with backoff (3 attempts max)
   - Parse `retry-after` headers from HTTP responses
   - Log retry attempts via the existing `slog` infrastructure
3. Surface retry state to the TUI via a new message type or status indicator
4. Never retry on context overflow errors (return immediately to let compaction handle it)

**Estimated effort:** ~1 day. Go's `net/http` already exposes response headers; the backoff math is trivial.

---

## 5. Git Diff Summaries per Session Turn

**OpenCode source:** `packages/opencode/src/session/summary.ts`, `packages/core/src/snapshot.ts`

**What M31A has:** Git integration in `internal/git/` supports commits and status, but doesn't track what changed during a session turn. The ledger records prompts/responses but not the diff footprint.

**What OpenCode does:**
- Captures git snapshots (commit hashes) at `step-start` and `step-finish` of each assistant turn
- After a turn completes, computes a full diff between the two snapshots
- Stores per-message diff summaries: file path, patch, additions, deletions, status (added/deleted/modified)
- Handles git path quoting (octal escapes for non-ASCII filenames)
- The session table tracks cumulative summary stats: total additions, deletions, file count
- Configurable via `snapshot: false` to disable

**Adoption plan:**
1. Extend `types.Message` with a `DiffSummary` field:
   ```go
   type DiffSummary struct {
       Files []FileDiff `json:"files"`
       Additions int    `json:"additions"`
       Deletions int    `json:"deletions"`
   }
   type FileDiff struct {
       File      string `json:"file"`
       Status    string `json:"status"` // added, deleted, modified
       Additions int    `json:"additions"`
       Deletions int    `json:"deletions"`
   }
   ```
2. In the workflow engine, capture `git rev-parse HEAD` before and after each assistant turn that involves file writes
3. Run `git diff --numstat` between snapshots to populate the summary
4. Display diff summaries in the TUI session view and ledger entries
5. Add a `/diff` slash command to show the diff for the current or last turn

**Estimated effort:** ~2 days. Git plumbing already exists in `internal/git/`.

---

## 6. Plan Mode with Agent Switching

**OpenCode source:** `packages/opencode/src/tool/plan.ts`, `packages/opencode/src/session/prompt/`

**What M31A has:** The six-phase workflow has a "Plan" phase, but it doesn't separate planning into a distinct agent with restricted capabilities. The same agent handles planning and execution.

**What OpenCode does:**
- Dedicated "plan" agent with read-only tools (no file editing)
- Plan agent writes a structured plan file
- `plan_exit` tool triggers a confirmation question: "Switch to build agent and start implementing?"
- On approval, switches the session's active agent to "build" which has full editing tools
- The plan file path is relative to the worktree, passed to the build agent as context
- Agent switching triggers a Context Epoch replacement (new system context for the build agent)

**Adoption plan:**
1. Create a built-in "planner" agent profile in M31A's config defaults:
   - System prompt emphasizing read-only exploration and structured planning
   - Tool list restricted to: `fileread`, `glob`, `grep`, `codecomplexity`, `codemap`, `websearch`, `webfetch`
   - No `filewrite`, `edit`, `bash`, or `filedelete`
2. Create a built-in "builder" agent profile:
   - Full tool access
   - System prompt that references the plan file
3. Add a `/plan` slash command that:
   - Switches to the planner agent
   - Creates a plan file at `.m31a/plans/<session-id>.md`
4. When the plan is complete, prompt the user to switch to the builder agent
5. On switch, inject the plan content as a synthetic user message for the builder to execute

**Estimated effort:** ~2 days. Agent profiles and switching already exist in the subagent system.

---

## 7. Dynamic System Context via Context Sources

**OpenCode source:** `packages/core/src/system-context/`, `packages/core/src/instruction-context.ts`

**What M31A has:** Static system prompts defined per-agent in YAML config. The system prompt is assembled once at session creation and doesn't update mid-session.

**What OpenCode does:**
- System context is composed from multiple independent "sources", each with:
  - A stable key (e.g., `core/instructions`, `core/date`)
  - A loader that fetches the current value
  - A codec for comparison (detect changes)
  - Renderers: baseline (initial), update (mid-conversation change), removal (source gone)
- Sources are evaluated concurrently and combined in stable key order
- When a source changes, a "mid-conversation system message" is emitted to the LLM
- AGENTS.md files are auto-discovered from project directory upward
- Changes are only admitted at "safe provider-turn boundaries" (before a provider call)
- Context snapshots store JSON-encoded state for change detection
- Unavailable sources use stale-while-revalidate semantics

**Adoption plan:**
1. Create `internal/context/` package:
   - `source.go` — `ContextSource` interface with `Key()`, `Load()`, `Render()`, `RenderUpdate()`, `RenderRemoval()`
   - `registry.go` — ordered registry that loads sources concurrently
   - `snapshot.go` — JSON-encoded state comparison
2. Built-in sources:
   - `instructions` — discovers and loads `AGENTS.md` files from project root upward
   - `datetime` — current date/time
   - `git` — current branch, last commit
   - `environment` — working directory, platform, shell
3. Hook into the provider call pipeline:
   - Before each LLM call, call `registry.Reconcile(snapshot)` to detect changes
   - If changes found, prepend a system message with the updated context
4. This replaces the static system prompt with a dynamic composition

**Estimated effort:** ~3 days. This is the most architecturally significant adoption but also the most impactful for long-running sessions.

---

## 8. Skill System (Composable Slash Commands)

**OpenCode source:** `packages/core/src/skill.ts`, `packages/opencode/src/tool/skill.ts`

**What M31A has:** Slash commands are hardcoded in `internal/tui/commands/`. Adding new commands requires code changes and recompilation.

**What OpenCode does:**
- Skills are Markdown files with YAML frontmatter: `name`, `description`, `slash: true`
- Discovered from directories (`.opencode/skills/`, plugin directories, remote URLs)
- Each skill has a `SKILL.md` that serves as the prompt template
- Skills can be invoked as slash commands (`/skill-name`) or as tools by the LLM
- Permission-controlled: agents can allow/deny specific skills
- Sources support: local directories, remote URLs (pulled to cache), and embedded skills
- Skills are cached by source key and deduplicated by name

**Adoption plan:**
1. Create `pkg/skills/`:
   - `skill.go` — `Skill` struct: `{ Name, Description, Slash bool, Content string, Location string }`
   - `discovery.go` — scan `~/.m31a/skills/` and `<project>/.m31a/skills/` for `*.md` and `**/SKILL.md` files
   - `loader.go` — parse YAML frontmatter + Markdown content
2. Register discovered skills as dynamic slash commands:
   - In `internal/tui/commands/registry.go`, add a `DynamicCommands` section populated from skill discovery
   - When invoked, inject the skill content as a system message or synthetic user message
3. Add a `skill` tool to the tool registry so the LLM can invoke skills autonomously
4. Config: `skills.sources` list with directory paths

**Estimated effort:** ~2 days. Markdown frontmatter parsing is the only new dependency (use a YAML parser on the frontmatter block).

---

## 9. Session Run Coordinator (Concurrency Control)

**OpenCode source:** `packages/core/src/session/run-coordinator.ts`

**What M31A has:** Single-threaded session execution. One session is active at a time; the TUI blocks on the current workflow phase.

**What OpenCode does:**
- Process-global coordinator keyed by session ID
- Two demand types: `run` (explicit drain) and `wake` (advisory — new durable work may be available)
- At most one active drain per session; different sessions run concurrently
- Coalesces follow-up demand: while draining, additional wakes collapse into one pending rerun
- `run` dominates `wake`: an explicit run upgrades a pending advisory wake
- Interrupt support: stops the current chain, suppresses pre-interrupt wakes
- `awaitIdle` blocks until the session settles
- Failure reporting for advisory wakes (explicit runs propagate errors to callers)

**Adoption plan:**
1. Create `pkg/coordinator/`:
   - `coordinator.go` — generic `Coordinator[Key]` with `Run(key)`, `Wake(key)`, `Interrupt(key)`, `AwaitIdle(key)`
   - Use Go channels and goroutines instead of Effect fibers
   - Entry struct with `current`, `pending`, `done chan`, `stopping bool`
2. This enables M31A to support background session execution:
   - Start a session's workflow in the background
   - Switch to another session in the TUI while the first runs
   - Wake sessions when new input arrives
3. Integrate with the existing `pkg/session/manager.go` for session lifecycle

**Estimated effort:** ~3 days. The Go implementation is simpler than the Effect-based TypeScript version but requires careful goroutine lifecycle management.

---

## 10. MCP OAuth Authentication

**OpenCode source:** `packages/opencode/src/mcp/auth.ts`, `packages/opencode/src/mcp/oauth-provider.ts`, `packages/opencode/src/mcp/oauth-callback.ts`

**What M31A has:** MCP client in `internal/tools/mcp_client.go` with stdio transport. No authentication support — MCP servers must be pre-configured with credentials.

**What OpenCode does:**
- OAuth 2.0 provider for MCP servers that require authentication
- Callback server for browser-based OAuth flow
- Token catalog with refresh support
- Per-server auth configuration

**Adoption plan:**
1. Add OAuth flow to `internal/tools/mcp_client.go`:
   - Before connecting, check if the MCP server requires auth
   - If so, start a local callback server on a random port
   - Open the user's browser for OAuth consent
   - Capture the token and store it in the keychain
2. Config: `tools.mcp.servers.<name>.auth` section with `client_id`, `client_secret`, `auth_url`, `token_url`

**Estimated effort:** ~2 days. The existing keychain (`pkg/keychain/`) handles token storage.

---

## 11. Session Instruction Discovery (AGENTS.md)

**OpenCode source:** `packages/core/src/instruction-context.ts`

**What M31A has:** No equivalent. Agent system prompts are defined in YAML config only.

**What OpenCode does:**
- Automatically discovers `AGENTS.md` files from the working directory up to the project root
- Also checks the global config directory for `AGENTS.md`
- File contents are injected as a context source into the system prompt
- Changes are detected and emitted as mid-conversation updates
- Respects `OPENCODE_DISABLE_PROJECT_CONFIG` to skip project-level discovery

**Adoption plan:**
1. This is a subset of recommendation #7 (Dynamic System Context)
2. As a standalone feature, add to `internal/config/`:
   - `DiscoverInstructions(projectRoot, workDir string) []InstructionFile`
   - Walk from `workDir` up to `projectRoot`, collecting `AGENTS.md` files
   - Also check `~/.m31a/AGENTS.md`
3. Inject discovered instructions into the system prompt assembly before each provider call
4. Add config option `instructions.enabled: true/false` and `instructions.disable_project: true/false`

**Estimated effort:** ~1 day as standalone, or part of the larger Dynamic System Context effort.

---

## 12. Prompt Template System (Per-Model System Prompts)

**OpenCode source:** `packages/opencode/src/session/prompt/` (13 `.txt` files)

**What M31A has:** System prompts are defined per-agent in YAML config. One prompt fits all models.

**What OpenCode does:**
- Maintains model-specific prompt templates: `anthropic.txt`, `gemini.txt`, `gpt.txt`, `codex.txt`, `trinity.txt`, etc.
- Templates are loaded based on the active model provider
- Plan mode has its own template variants: `plan-mode.txt`, `plan.txt`, `plan-reminder-anthropic.txt`
- Templates account for model-specific formatting quirks and instruction-following patterns

**Adoption plan:**
1. Create `internal/prompts/templates/` directory with `.txt` files for each provider family:
   - `openai.txt` — optimized for GPT models
   - `anthropic.txt` — optimized for Claude models
   - `google.txt` — optimized for Gemini models
   - `default.txt` — fallback
2. Add template selection logic in the provider layer:
   - `SelectTemplate(modelID string) string` — matches model ID to template family
3. Agent system prompts become composable: `template + agent_prompt + dynamic_context`
4. Migrate existing YAML-defined prompts into the template system

**Estimated effort:** ~1 day for infrastructure, ongoing effort for prompt tuning.

---

## Priority Matrix

| # | Recommendation | Impact | Effort | Dependencies | Priority |
|---|---|---|---|---|---|
| 1 | Auto Compaction | **High** — prevents context overflow in long sessions | 2d | None | **P0** |
| 2 | Tool Output Bounding | **High** — prevents single tool calls from blowing context | 1d | None | **P0** |
| 3 | Wildcard Permission Rules | **Medium** — finer-grained security control | 2d | None | P1 |
| 4 | Retry Policy with Backoff | **High** — improves reliability under rate limits | 1d | None | **P0** |
| 5 | Git Diff Summaries | **Medium** — better session observability | 2d | None | P2 |
| 6 | Plan Mode + Agent Switching | **Medium** — leverages existing six-phase workflow | 2d | None | P1 |
| 7 | Dynamic System Context | **High** — transforms long-session quality | 3d | None | P1 |
| 8 | Skill System | **Medium** — user-extensible commands without recompilation | 2d | None | P1 |
| 9 | Session Run Coordinator | **Medium** — enables background/concurrent sessions | 3d | None | P2 |
| 10 | MCP OAuth | **Low** — niche, most MCP servers use stdio | 2d | Keychain | P3 |
| 11 | AGENTS.md Discovery | **Medium** — project-aware context (subset of #7) | 1d | None | P1 |
| 12 | Per-Model Prompt Templates | **Medium** — improves output quality per provider | 1d | None | P2 |

**Recommended implementation order:** 1, 2, 4 (P0 stability), then 11, 7, 3, 8 (P1 extensibility), then 6, 5, 12 (P2 polish), then 9, 10 (P3 future).

---

## Summary

The three highest-impact adoptions are:

1. **Auto Compaction** — M31A's biggest weakness is that long sessions hit context limits with no graceful degradation. OpenCode's structured summary approach (Goal/Progress/Decisions/Next Steps) is production-proven and directly portable to Go.

2. **Tool Output Bounding** — A single `grep` on a large codebase can consume an entire 128K context window. OpenCode's head+tail preview with managed file storage is a clean, self-contained fix.

3. **Retry with Backoff** — M31A's provider fallback jumps to a different model immediately. Adding structured retry with header-aware backoff on the same provider prevents unnecessary model switches and handles transient 5xx errors that are common with LLM APIs.

All twelve recommendations are architecturally compatible with M31A's Go codebase and can be adopted incrementally without disrupting the existing six-phase workflow engine.

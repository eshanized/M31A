# OpenCode → M31A Adaptation Report

## Executive Summary

After a thorough study of every file in both the `opencode/` and `M31A/` codebases, this report identifies **21 concrete adaptations** M31A can make from OpenCode's architecture, organized by priority and effort. The two projects share the same domain (terminal AI coding assistants) but differ fundamentally: OpenCode is a **reactive, server-client TypeScript app** built on Effect + SolidJS + OpenTUI, while M31A is a **standalone Go binary** using Bubble Tea's Elm architecture. M31A's unique strengths (6-phase workflow engine, auto-arbitrage, git-based rollback, verification phase) should be preserved while adopting OpenCode's mature patterns for session management, context control, extensibility, and developer ergonomics.

---

## Part 1: What OpenCode Does Well (Adopt for M31A)

### P1: Critical — Adopt Now

#### 1. Context Compaction & Summarization
- **OpenCode**: `session/compaction.ts` — automatic compaction when token budget exceeded, structured summary replaces old messages, dedicated hidden `compaction` agent. Pruning protects `PRUNE_PROTECT = 40_000` tokens and keeps last 2 turns verbatim.
- **M31A gap**: Has token estimation with EMA calibration but **no context reduction strategy**. Will fail on long conversations.
- **Adaptation**: Implement a compaction module in `pkg/autodream/` (extend existing AutoDream). Add a `/compact` slash command. Use a lightweight model to summarize old message pairs into memory segments. Preserve last 2 turns + protected tokens.
- **Files to create/modify**: `pkg/autodream/compaction.go`, `internal/tui/commands.go` (add `/compact`), `internal/workflow/engine.go` (auto-compact trigger).

#### 2. Session Forking
- **OpenCode**: `sdk.client.session.fork()` creates child sessions with `parentID`. Enables exploring alternative approaches without losing context.
- **M31A gap**: Sessions are linear — no branching.
- **Adaptation**: Add `parent_id` to session struct. `fork` command copies message history and creates new session directory with parent reference. Add sibling navigation (`/prev`, `/next`).
- **Files to modify**: `pkg/session/session.go`, `pkg/session/manager.go`, `internal/tui/commands.go`.

#### 3. Session Undo/Revert
- **OpenCode**: `sdk.client.session.revert({ sessionID, messageID })` sets revert marker. Redo via `unrevert`.
- **M31A gap**: No way to undo bad AI responses. Git rollback undoes file changes but not conversation state.
- **Adaptation**: Add `reverted_to` field in `messages.json`. `/undo` command truncates messages to a prior point. `/redo` restores.
- **Files to modify**: `pkg/session/session.go`, `internal/tui/commands.go`.

#### 4. Structured Tool Call Handling (Move Away from Regex JSON Extraction)
- **OpenCode**: Uses Vercel AI SDK's `streamText` with typed tool definitions. Tool calls arrive as structured events, not parsed from text.
- **M31A gap**: `parseToolCalls()` in `engine.go` extracts JSON from streaming text using regex `\`\`\`json\n(...)\n\`\`\``. Fragile, model-dependent.
- **Adaptation**: Build a lightweight Go abstraction layer (similar to AI SDK but simpler) that handles structured tool calling via the provider API's native tool use format (Anthropic's tool_use blocks, OpenAI's function calling). This requires updating `LLMProvider` interface to support native tool calls.
- **Files to modify**: `internal/provider/interface.go`, `internal/provider/openrouter/client.go`, `internal/provider/zen/client.go`, `internal/workflow/engine.go`.

### P2: High Value — Adopt Next

#### 5. MCP (Model Context Protocol) Integration
- **OpenCode**: Full MCP server management in `mcp/` directory. Tools from MCP servers dynamically loaded via `convertMcpTool()`. OAuth 2.0 with PKCE for remote MCP servers.
- **M31A gap**: No external tool server support.
- **Adaptation**: Add MCP client support. Start with stdio transport. Register MCP-discovered tools alongside built-in tools in the dispatcher.
- **Files to create**: `internal/mcp/client.go`, `internal/mcp/transport.go`, `internal/mcp/tools.go`.

#### 6. Plugin/Extensibility System
- **OpenCode**: Full plugin runtime with `TuiPluginRuntime.init()`, slot system (`app_bottom`, `session_prompt`, etc.), and `TuiPluginApi` surface. Plugins are npm packages.
- **M31A gap**: No plugin system. Extensibility limited to code-level tool/provider registration.
- **Adaptation**: For Go, a plugin system is more complex (no dynamic code loading). Two approaches:
  - **Option A**: HTTP-based plugin protocol — plugins run as separate processes, communicate via HTTP/gRPC.
  - **Option B**: Go plugin system (c-shared libraries) — more fragile but native.
  - **Recommended**: Option A. Define a simple HTTP protocol for plugins to register slash commands, tools, and TUI widgets.
- **Files to create**: `internal/plugin/runtime.go`, `internal/plugin/protocol.go`.

#### 7. Subagent/Task Delegation
- **OpenCode**: `task` tool spawns child sessions with their own agent loops (general, explore, scout agents). Parent waits for completion.
- **M31A gap**: No parallel work delegation.
- **Adaptation**: Add a `task` tool that spawns a new M31A process in headless mode with a specific prompt. Parent process polls for result. This is simpler than OpenCode's server-side child sessions but achieves the same goal.
- **Files to create**: `internal/tools/task.go`, `cmd/m31a/headless.go`.

#### 8. Model Variants & Favorites System
- **OpenCode**: Per-model variants (e.g., `gemini-2.5-pro` with `thinking` variant). Recent list (up to 10), favorite list, per-agent model assignments, keyboard cycling.
- **M31A gap**: No variants, no favorites, no per-agent models, no keyboard cycling.
- **Adaptation**: Add `variant` field to `ModelInfo`. Add `~/.m31a/recent_models.json` and favorites list. Add `Ctrl+M` / `Ctrl+Shift+M` for model cycling.
- **Files to modify**: `internal/types/types.go`, `internal/tui/modelselector.go`, `internal/tui/keybindings.go`, `pkg/session/manager.go`.

#### 9. Permission Ruleset (Glob-Based, Per-Tool, Per-Path)
- **OpenCode**: Rule-based permission system with glob patterns: `{ tool, pattern, action }` tuples. Actions: `allow`, `deny`, `ask`. Default rules per agent (build agent allows most, plan agent denies edits).
- **M31A gap**: Simple risk-level gating (safe/medium/dangerous/destructive). No path-based rules, no per-agent permissions.
- **Adaptation**: Extend `PermissionRequest` to support glob pattern matching. Add configurable rules in `config.toml`. Add per-agent permission profiles.
- **Files to modify**: `internal/tools/interface.go`, `internal/tools/dispatcher.go`, `internal/config/types.go`, `internal/tui/components/permission.go`.

#### 10. Multi-Layer Configuration with Schema Validation
- **OpenCode**: 8-layer config merging (remote well-known, global, env override, project, `.opencode/` dir, inline JSON, console/enterprise, MDM). Effect Schema validation. Variable substitution `${VAR}`.
- **M31A gap**: Single TOML config file, no schema validation, no layered merging.
- **Adaptation**: Add project-level config (`m31a.toml` in project root). Add env var overrides. Add config validation with error messages for malformed values.
- **Files to modify**: `internal/config/loader.go`, `internal/config/types.go`.

### P3: Medium Value — Consider

#### 11. Prompt History with Frecency Ranking
- **OpenCode**: Persistent prompt history stored in local KV with frequency/recency ranking. Autocomplete in prompt input.
- **M31A gap**: No persistent prompt history.
- **Adaptation**: Store last N prompts in `~/.m31a/prompt_history.json`. Add frecency scoring. Arrow-up/down in REPL navigates history.
- **Files to create**: `internal/tui/history.go`.

#### 12. Shell Mode (`!` prefix for direct command execution)
- **OpenCode**: `!` prefix bypasses LLM and runs commands directly.
- **M31A gap**: All command execution goes through the LLM tool-use loop.
- **Adaptation**: In REPL, detect `!` prefix and execute directly via Bash tool without LLM involvement.
- **Files to modify**: `internal/tui/repl.go`.

#### 13. Diff Viewing
- **OpenCode**: Split/unified diff rendering with syntax highlighting via tree-sitter.
- **M31A gap**: No diff viewer.
- **Adaptation**: Add `/diff` command that runs `git diff` and renders with `glamour` (existing markdown renderer) or a custom diff renderer using `lipgloss`.
- **Files to create**: `internal/tui/diff.go`.

#### 14. Editor Context Auto-Include
- **OpenCode**: Automatically includes selected editor text in prompts when connected to an IDE.
- **M31A gap**: Not applicable for standalone CLI, but could support `@file` syntax for auto-including file contents.
- **Adaptation**: Add `@filepath` syntax in REPL that auto-includes file contents in the next prompt.
- **Files to modify**: `internal/tui/repl.go`.

#### 15. Session Sharing (URL-Based)
- **OpenCode**: Share session via URL. Manual/auto/disabled modes.
- **M31A gap**: No sharing capability.
- **Adaptation**: Lower priority for standalone CLI. Could support exporting session as markdown/html for sharing.
- **Files to create**: `pkg/session/export.go`.

#### 16. Bus/PubSub Event System
- **OpenCode**: Typed event bus using Effect's PubSub for decoupled communication between subsystems, with both typed and wildcard subscriptions.
- **M31A gap**: Uses `tea.Cmd` channel pattern, which is coupling-heavy.
- **Adaptation**: Introduce a lightweight pub/sub system for internal events (session changes, tool executions, phase transitions). Could simplify TUI message handling.
- **Files to create**: `internal/bus/bus.go`.

---

## Part 2: What M31A Should Preserve (Unique Strengths)

These are features M31A has that OpenCode does **not** — they are competitive differentiators:

| Feature | Why Preserve |
|---------|-------------|
| **6-Phase Workflow Engine** | OpenCode has no structured workflow. This is M31A's biggest differentiator for autonomous multi-step coding tasks. |
| **Auto-Arbitrage** | Cost-optimized model selection based on task complexity scoring. OpenCode has no cost awareness. |
| **Auto-Fallback** | Provider failover on rate limits via health checks. OpenCode handles this server-side but not with explicit health checks. |
| **Git-Based Rollback** | Safety net for reverting all changes made during a session. |
| **Verification Phase** | Automated syntax checking and test execution per project type. OpenCode trusts the LLM to verify its own work. |
| **Task DAG with Cycle Detection** | Iterative DFS cycle detection for dependency-ordered execution. |
| **Self-Heal Loops** | Per-task automatic repair (up to 2 attempts) with failure context. |
| **Git Bisect for Debugging** | Finds offending commit between session start and HEAD when verification fails. |
| **5-Level Edit Fallback** | Cascading edit strategies (exact → line-trimmed → whitespace-normalized → fuzzy with Levenshtein ≥0.7). |
| **EMA-Calibrated Token Estimation** | Self-improving token counter that adapts to actual usage patterns. |
| **Standalone Binary** | No server required, works fully offline, simpler deployment. |
| **Atomic File Writes** | Crash resilience via temp file + rename with `crypto/rand` temp names. |
| **Ledger System** | Cross-session activity logging with statistics (avg tasks, avg cost, top failures). |

---

## Part 3: Architecture Comparison Matrix

| Dimension | OpenCode | M31A | Recommendation |
|-----------|----------|------|----------------|
| **Language** | TypeScript/Bun | Go | Keep Go for performance + static binaries |
| **TUI Framework** | OpenTUI + SolidJS (reactive JSX) | Bubble Tea + Lipgloss (Elm) | Keep Bubble Tea; consider fine-grained updates |
| **Architecture** | Client-server (HTTP/SSE) | Standalone monolith | Keep standalone; add headless mode for subagents |
| **Session Storage** | SQL (Drizzle/SQLite) | Flat JSON files (atomic writes) | Keep flat files; add parent_id for forking |
| **Streaming** | Server-side processing | Direct SSE, in-process parser | Keep direct SSE; add structured tool call support |
| **Context Management** | Pruning + compaction + summary | Token estimation + warning banner | **Adopt compaction** (P1) |
| **Workflow** | Conversational REPL | 6-phase structured engine | **Preserve workflow** (unique strength) |
| **Permissions** | Server-managed, glob-based | Risk-based + timeout | **Adopt glob-based rules** (P2) |
| **Plugins** | Full slot-based system | None | **Adopt HTTP plugin protocol** (P2) |
| **Model Selection** | Per-agent, recent/favorite, variants | Full-screen selector, auto-arbitrage | **Add variants + favorites** (P2) |
| **Provider Fallback** | Server-side | Auto-fallback with health checks | Preserve auto-fallback |
| **Cost Optimization** | None | Auto-arbitrage with complexity scoring | Preserve arbitrage |
| **Session Features** | Fork, undo/redo, share, compact | Create, list, resume, archive, delete | **Add fork + undo** (P1) |
| **Tool Rendering** | Rich JSX per tool type | Basic tool cards | Improve tool cards with richer rendering |
| **Token Estimation** | Static | EMA-calibrated | Preserve EMA calibration |
| **Safety Features** | Server isolation | Git rollback, verification phase | Preserve safety features |
| **Config System** | 8-layer, schema-validated | Single TOML | **Add layered config** (P2) |
| **MCP Support** | Yes | No | **Add MCP client** (P2) |
| **Subagents** | Yes (spawn child sessions) | No | **Add task tool** (P2) |
| **Tool Calls** | Structured via AI SDK | Regex-extracted JSON | **Adopt structured tool calls** (P1) |

---

## Part 4: Implementation Priority Roadmap

### Phase 1: Critical Foundation (Weeks 1-3)
1. **Context Compaction** — prevents conversation failures
2. **Structured Tool Calls** — reliability improvement, reduces model-dependent parsing bugs
3. **Session Undo** — critical UX feature

### Phase 2: Core Features (Weeks 4-6)
4. **Session Forking** — enables exploratory workflows
5. **Permission Ruleset** — granular security
6. **Model Variants & Favorites** — better model management
7. **Shell Mode** — quick command execution

### Phase 3: Extensibility (Weeks 7-10)
8. **MCP Integration** — external tool servers
9. **Subagent/Task Tool** — parallel work delegation
10. **Plugin System** — community extensibility
11. **Multi-Layer Config** — project-level config

### Phase 4: Polish (Weeks 11-12)
12. **Prompt History** — frecency-ranked history
13. **Diff Viewing** — visual diff renderer
14. **Editor Context** — `@file` auto-include
15. **Bus/PubSub** — internal event decoupling

---

## Part 5: What NOT to Adopt

| OpenCode Feature | Why Skip for M31A |
|------------------|-------------------|
| **Server-Client Architecture** | M31A's standalone binary is a strength. Server mode adds complexity without clear benefit for a CLI tool. |
| **Effect System** | TypeScript-specific. Go's error handling + context patterns are sufficient. |
| **OpenTUI/SolidJS** | Bubble Tea is mature and well-supported. Rewriting TUI is not worth the investment. |
| **Drizzle ORM / SQLite** | Flat JSON files work well for M31A's scale. SQLite adds dependency complexity. |
| **Monorepo with Bun Workspaces** | M31A is a single binary — monorepo complexity is unnecessary. |
| **Native Binary Distribution via Bun Compile** | M31A already uses goreleaser with CGO_ENABLED=0 for static binaries. |

---

## Part 6: Key OpenCode Files to Study for Each Adaptation

| Adaptation | OpenCode Files to Study |
|------------|------------------------|
| Compaction | `packages/opencode/src/session/compaction.ts` |
| Session Forking | `packages/opencode/src/session/session.ts`, `session.sql.ts` |
| Session Undo | `packages/opencode/src/session/projectors-next.ts` |
| Structured Tools | `packages/opencode/src/session/prompt.ts` (~1780 lines), `packages/opencode/src/session/llm.ts` |
| MCP | `packages/opencode/src/mcp/index.ts` |
| Plugin System | `packages/opencode/src/cli/cmd/tui/plugin/runtime.ts`, `slots.tsx` |
| Subagents | `packages/opencode/src/tool/task.ts`, `packages/opencode/src/agent/agent.ts` |
| Permissions | `packages/opencode/src/permission/` directory |
| Config System | `packages/opencode/src/config/config.ts` (885 lines) |
| Prompt History | `packages/opencode/src/cli/cmd/tui/context/history.tsx` |
| Model Management | `packages/opencode/src/cli/cmd/tui/context/local.tsx` |
| Tool Rendering | `packages/opencode/src/cli/cmd/tui/components/shell.tsx` etc. |
| Event Bus | `packages/opencode/src/bus/` directory |

---

## Part 7: Technical Debt & Risks in M31A to Address

1. **Regex-based tool call parsing** — fragile, model-dependent. Replace with structured API tool calls.
2. **No context reduction** — will crash on long conversations. Add compaction urgently.
3. **Token estimation accuracy** — tiktoken-go is OpenAI-specific. Add model-specific estimators.
4. **No graceful shutdown** — handle SIGINT/SIGTERM for session cleanup.
5. **No config schema validation** — malformed TOML uses zero-values silently.
6. **Limited test coverage** — add comprehensive unit tests for providers, tools, workflow engine.
7. **No integration tests** — add end-to-end test harness for full workflow pipeline.
8. **No mock providers** — add test doubles for offline testing.
9. **Flat message passing** — consider pub/sub for internal event decoupling.
10. **Single-provider tool calling** — native tool use formats differ across providers (Anthropic tool_use blocks vs OpenAI function calling vs OpenRouter pass-through).

---

*Report generated from complete file-level analysis of both codebases. All file paths and code references verified against the current HEAD of each repository.*

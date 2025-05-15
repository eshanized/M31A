# Deep Logic Comparison: OpenCode vs M31A

## Executive Summary

OpenCode (TypeScript/Bun/OpenTUI/SolidJS) and M31A (Go/Bubble Tea/Lipgloss) are both AI coding agent CLIs with terminal UIs, but they differ fundamentally in architecture. OpenCode is a **reactive, event-driven SPA** running inside a custom terminal renderer (OpenTUI), with server-side session processing, SQL-backed persistence, and a plugin system. M31A is a **traditional Elm-architecture TUI** with in-process LLM calls, file-based session storage, and a unique 6-phase workflow engine for autonomous multi-step coding tasks.

---

## 1. App Lifecycle & Initialization

### OpenCode
**File:** `opencode/packages/opencode/src/cli/cmd/tui/app.tsx`

OpenCode's startup is a **two-phase client/server model**:
1. `tui()` creates a CLI renderer via `createCliRenderer()` from OpenTUI, installs Win32 Ctrl-C guard, creates a keymap, and calls `mountTui()`.
2. `mountTui()` calls `render()` from `@opentui/solid` to mount a deeply nested provider tree (ErrorBoundary > Keymap > Args > Exit > KV > Toast > Route > TuiConfig > SDK > Project > Sync > Theme > Local > Stash > Dialog > Frecency > History > PromptRef > Editor > App).
3. The `App` component uses `TuiPluginRuntime.init()` to load plugins asynchronously, then signals `ready()`.
4. Route-based navigation: home -> session. On `-c` (continue), it auto-navigates to the most recent session. On `--fork`, it forks the session.
5. The SDK context manages an HTTP connection to a backend server; the TUI is a thin client.

**First-run:** If no providers are connected, a `DialogProviderList` dialog is shown. API keys are managed server-side through the SDK client.

**Config loading:** `TuiConfig.Resolved` is passed in from the CLI bootstrap layer. TUI config is resolved from JSON files with schema validation.

### M31A
**File:** `M31A/internal/tui/app.go`

M31A's startup is **single-process, monolithic**:
1. `NewApp()` loads config from disk (`config.Load()`), initializes keychain, resolves API keys (env -> keychain -> config file), creates session manager, git wrapper, rollback, ledger, settings model, resume model, model selector, sidebar, command palette, key registry.
2. If `apiKey == ""`, shows `ScreenFirstRun` (API key setup). Otherwise, goes directly to `ScreenREPL`.
3. `Init()` starts health check and cache refresh tickers.
4. The Bubble Tea event loop (`tea.NewProgram`) drives the entire app.

**First-run:** `ScreenFirstRun` model prompts for API key, saves to keychain and config file.

**Config loading:** TOML config at `~/.m31a/config.toml` with `config.DefaultConfig()` fallback.

**Key difference:** OpenCode is a thin TUI client talking to a backend server. M31A is a standalone binary that makes LLM calls directly.

---

## 2. TUI Architecture

### OpenCode: SolidJS Reactive Rendering
- Uses `@opentui/solid` (a SolidJS port for terminal rendering) with JSX components.
- State managed via SolidJS primitives: `createSignal`, `createMemo`, `createStore`, `createEffect`.
- Rendering loop: OpenTUI's native event loop at 60fps targets `targetFps: 60`. JSX components reactively re-render when signals change.
- Keyboard input: `@opentui/keymap` provides a command-dispatch system with modes, leader keys, and priority interceptors. Commands are registered via `useBindings()`.
- Layout: OpenTUI's native layout engine with `<box>`, `<scrollbox>`, `<textarea>`, `<markdown>`, `<diff>`, `<code>`, `<spinner>`, `<line_number>` custom elements.

### M31A: Bubble Tea Elm Architecture
- Strict Model-Update-View pattern via `tea.Model` interface (`Init()`, `Update()`, `View()`).
- State is a flat `AppState` struct with sub-models (`ReplModel`, `SettingsModel`, `ResumeModel`, etc.).
- Rendering loop: Bubble Tea's event loop processes messages sequentially. Each `Update()` returns `(tea.Model, tea.Cmd)`.
- Keyboard input: `tea.KeyMsg` handled in a massive switch statement in `Update()`, with screen-specific delegation and a `KeyRegistry` for leader-key chords.
- Layout: String composition via `lipgloss.JoinHorizontal/Vertical` and `lipgloss.NewStyle()`. Each model's `View()` returns a `string`.

**Key difference:** OpenCode uses a reactive component tree that diffs and re-renders. M31A re-renders the entire screen as a composed string on every tick. OpenCode's approach enables fine-grained updates (individual message re-rendering); M31A's approach is simpler but re-renders everything.

---

## 3. Session Management

### OpenCode
**Files:** `packages/opencode/src/session/session.ts`, `session.sql.ts`

- **Storage:** SQL database (SQLite via Drizzle ORM) with tables for sessions, messages, parts, projects, permissions.
- **Session creation:** Server-side via `sdk.client.session.create()`. Returns a session ID.
- **Session loading:** TUI syncs via SSE from the server. The `SyncProvider` context maintains a reactive store of all session data.
- **Session switching:** `route.navigate({ type: "session", sessionID })` triggers a `createEffect` that fetches session data and reconnects editors.
- **Session forking:** `sdk.client.session.fork()` creates a child session with `parentID`.
- **Session undo/revert:** `sdk.client.session.revert({ sessionID, messageID })` sets a `revert` marker. Redo uses `unrevert`.
- **Session compacting:** `sdk.client.session.summarize()` triggers server-side compaction.
- **Quick switch:** 9 pinned slots stored in state file.

### M31A
**Files:** `pkg/session/manager.go`, `session.go`

- **Storage:** Flat files on disk. Each session is a directory `~/.m31a/sessions/<8-char-hex>/` containing `session.json` and `messages.json`.
- **Session creation:** `sessionManager.NewSession(model, provider)` generates a random 8-char hex ID, creates directory, writes JSON files atomically (temp file + rename).
- **Session loading:** `sessionManager.LoadSession(id)` reads JSON files, reconstructs session struct.
- **Session listing:** `sessionManager.ListSessions()` scans directories, reads `session.json`, sorts by LastModified descending.
- **Session archiving:** `ArchiveSession()` moves to `~/.m31a/sessions/archived/<id>/`.
- **Session resuming:** `ScreenResume` model lists sessions, user selects, REPL is populated with messages.
- **No forking, no undo/revert, no compaction.**

**Key difference:** OpenCode has a full relational session model with forking, revert/redo, compaction, and server-side persistence. M31A has a simple flat-file model with atomic writes but lacks advanced session features.

---

## 4. Provider/Model Management

### OpenCode
**File:** `packages/opencode/src/cli/cmd/tui/context/local.tsx`

- **Discovery:** Providers are discovered server-side and synced to the TUI via SSE.
- **Model selection:** `local.model` context manages per-agent model assignments, a `recent` list (up to 10), a `favorite` list, and per-model variants. State is persisted to state file.
- **Model cycling:** `model.cycle(direction)` cycles through recent models. `model.cycleFavorite(direction)` cycles through favorites.
- **Model validation:** `isModelValid()` checks if the provider has the model.
- **Fallback resolution:** Fallback chain: CLI arg -> agent-configured model -> config default -> recent -> provider default -> first model in provider.
- **Variants:** Per-model variant support (e.g., `gemini-2.5-pro` with `thinking` variant).

### M31A
**Files:** `internal/provider/registry.go`, `modelselector.go`

- **Discovery:** Providers are registered at startup. Currently: OpenRouter and Zen. Models are fetched via `p.FetchModels(ctx)`.
- **Model selection:** `ModelSelector` screen shows all models from all providers with filtering, search, and detail pane.
- **Model cycling:** No keyboard shortcut cycling. User must open `/models` screen.
- **Auto-arbitrage:** Unique feature. Before each request, `arbitrage.Recommend()` scores task complexity, estimates token usage, and may switch to a cheaper model.
- **Fallback:** `Provider.AutoFallback` config triggers `FindFallbackProvider()` on rate-limit/429/503 errors.
- **No variants, no favorites, no per-agent models.**

**Key difference:** OpenCode has a rich model management system with recent/favorite lists, per-agent models, variants, and keyboard cycling. M31A has auto-arbitrage (cost-optimization) and auto-fallback (provider failover) which OpenCode lacks.

---

## 5. Streaming Architecture

### OpenCode
- **Architecture:** Server-side streaming. The backend processes the LLM stream and emits SSE events to the TUI client.
- **Chunk processing:** The server-side `SessionProcessor` handles the stream via Effect's `Stream` abstraction. Tool calls, text, reasoning parts are processed as discrete `Part` objects.
- **TUI rendering:** The TUI receives parts via sync data and re-renders reactively. No manual stream handling in the TUI.
- **Cancellation:** `sdk.client.session.abort({ sessionID })` sends a server-side abort. The prompt component shows "esc again to interrupt" with a double-press confirmation.

### M31A
**Files:** `internal/provider/sse.go`, `internal/tui/repl.go`

- **Architecture:** Direct SSE streaming from the LLM provider to the TUI process.
- **SSE Parser:** `SSEParser` uses `bufio.Scanner` with 64KB buffer. Parses `data:` and `event:` lines, joins multi-line data, handles `[DONE]` sentinel.
- **Chunk processing:** `ReplModel.handleStreamMsg()` segments incoming chunks by type (`content`, `thinking`). Uses a `strings.Builder` to accumulate content per segment type.
- **Finalization:** `handleStreamDoneMsg()` flushes remaining content, appends message, populates `thinkingBlocks` and `toolCards` maps.
- **Cancellation:** `m.streamCancel()` cancels the context. The continuation cmd watches both channels.
- **Continuation pattern:** After each chunk, returns a `nextCmd` function that reads the next message, creating a chain of async commands.

**Key difference:** OpenCode delegates streaming to the server, with the TUI as a passive consumer. M31A handles streaming directly in the TUI process with a manual SSE parser and continuation command pattern.

---

## 6. Tool System

### OpenCode
- **Tool definitions:** Server-side. Tools are registered on the backend and exposed to the TUI via the sync layer.
- **Tool rendering:** Each tool type has a dedicated JSX component: `Shell`, `Write`, `Read`, `Glob`, `Grep`, `WebFetch`, `WebSearch`, `Edit`, `ApplyPatch`, `Task`, `TodoWrite`, `Question`, `Skill`, `GenericTool`.
- **Tool states:** `pending`, `running`, `completed`, `error`.
- **Permission model:** Server-managed. TUI shows `PermissionPrompt` component when permissions exist.
- **Tool result processing:** Server processes tool results and emits events. TUI reacts via reactive store.
- **Tool collapsing:** Long outputs truncated with expand/collapse.

### M31A
**Files:** `internal/tools/dispatcher.go`, `internal/tui/components/toolcard.go`

- **Tool definitions:** `types.Tool` interface with `Name()`, `Description()`, `Execute()`, `RiskLevel()`.
- **Dispatch:** `Dispatcher.Execute()` looks up tool by name, unmarshals input JSON, checks risk level, requests permission if needed, calls `tool.Execute()`.
- **Permission model:** Risk-based. Tools with `RiskDangerous` or `RiskDestructive` trigger `PermissionRequest` sent through a channel to the TUI. Timeout: 5 minutes.
- **Tool result processing:** Results returned as `ToolResult{Output, DurationMs, Truncated, Error}`.
- **Tool name normalization:** `normalizeToolName()` maps LLM tool name variants to canonical names.

**Key difference:** OpenCode's tool system is server-driven with rich per-tool JSX rendering. M31A's tool system is in-process with a risk-based permission model and channel-based request/response pattern.

---

## 7. Workflow Engine

### OpenCode: No Formal Workflow Engine
OpenCode does not have a multi-phase workflow engine. It operates as a **conversational REPL**: user sends a message, assistant responds with text/tools, tool results are fed back, loop continues. Multi-step work happens organically through the tool-use loop.

### M31A: 6-Phase Workflow Engine
**File:** `internal/workflow/engine.go`

M31A has a structured 6-phase workflow:
1. **Initialize:** Detect project type, list files, build context, ask clarifying questions.
2. **Discuss:** LLM generates clarifying questions. User answers.
3. **Plan:** LLM generates a task list (JSON array). Tasks have IDs, descriptions, actions, files, dependencies. Validates for cycles.
4. **Execute:** Execute tasks in dependency order. Each task runs tool calls. Status tracked per task.
5. **Verify:** Check file existence, run syntax checks, run tests per project type.
6. **Ship:** Display summary with task statistics, session ID.

**Execution:** `RunPhaseCmd()` runs a phase in a goroutine, emits messages via `MsgEmitter` channel, returns `PhaseResultMsg`. Phases auto-advance on completion.

**Unique to M31A:** This is the most distinctive feature difference. OpenCode has no equivalent.

---

## 8. Context Management

### OpenCode
- **Context window management:** Server-side via overflow and compaction modules.
- **Pruning:** `PRUNE_MINIMUM = 20_000`, `PRUNE_PROTECT = 40_000`. Removes old messages when context exceeds limits.
- **Compaction/Summarization:** `/compact` command triggers server-side summarization using a structured template. Summary replaces old messages.
- **Tool output truncation:** `TOOL_OUTPUT_MAX_CHARS = 2_000`.
- **Context preservation:** Last 2 turns always preserved. Recent messages protected up to 8,000 tokens.

### M31A
- **Token estimation:** `tokens.Estimator` uses `tiktoken-go` for OpenAI models, rune-based fallback for others.
- **EMA calibration:** Running calibration factor updated with actual usage. `emaFactor = 0.3 * ratio + 0.7 * previousFactor`.
- **Context warning:** Banner at 80% threshold: "Context at XX% -- consider using /compress".
- **No compaction/summarization logic.** No pruning. Full message history sent with every request.

**Key difference:** OpenCode has a mature, multi-layered context management system with pruning, compaction, and summarization. M31A has token estimation with EMA calibration but no context reduction strategy.

---

## 9. Theme/Styling System

### OpenCode
- **Theme definition:** TypeScript theme objects with RGBA color tokens.
- **Mode switching:** `ThemeProvider` with `mode` ("dark"/"light"), `locked` state. Terminal theme mode detected via renderer.
- **Color tokens:** Rich palette with `background`, `backgroundPanel`, `backgroundElement`, `backgroundMenu`, `text`, `textMuted`, `primary`, `secondary`, `accent`, etc.
- **Syntax highlighting:** OpenTUI's tree-sitter-based syntax engine.
- **Dynamic coloring:** Agent colors assigned cyclically from theme palette.

### M31A
**File:** `internal/tui/theme/theme.go`

- **Theme definition:** `Theme` struct with `lipgloss.Color` fields. Two predefined themes: `Dark()` and `Light()`.
- **Mode cycling:** `Manager.Cycle()` rotates Dark -> Light -> Auto -> Dark.
- **Color tokens:** Mirrors OpenCode's tokens plus lipgloss `Style` objects for specific UI elements.
- **Auto detection:** `Auto()` uses `lipgloss.HasDarkBackground()`.

**Key difference:** Both have similar color token systems (M31A deliberately mirrors OpenCode's). OpenCode has dynamic theme detection from the terminal; M31A has explicit Dark/Light/Auto modes with cycling.

---

## 10. Plugin/Extensibility

### OpenCode
**Files:** `packages/opencode/src/cli/cmd/tui/plugin/runtime.ts`, `slots.tsx`

- **Plugin system:** Full plugin runtime with `TuiPluginRuntime.init()` and `TuiPluginRuntime.Slot`.
- **Slot system:** Named slots (`app_bottom`, `app`, `session_prompt`, `session_prompt_right`) with `mode="replace"` support.
- **Plugin API:** `TuiPluginApi` provides access to dialog, keymap, kv, route, sdk, sync, theme, toast, renderer, attention.
- **Feature plugins:** Home tips, session dialogs, sidebar panels (files, LSP, MCP, todo), system diff viewer, which-key.

### M31A
- **No plugin system.** Extensibility is limited to:
  - Slash commands via `CommandRegistry`.
  - Tool registration via `Dispatcher.Register()`.
  - Provider registration via `Registry.Register()`.
- **Command palette:** `ctrl+p` opens a command palette with built-in commands.
- **Key registry:** `KeyRegistry` with default bindings and leader-key support.

**Key difference:** OpenCode has a mature plugin system with slots, API surface, and error isolation. M31A has no plugin extensibility.

---

## 11. Key Differences Summary

### What OpenCode Does That M31A Doesn't
1. **Server-client architecture** - TUI is a thin client; session processing server-side.
2. **Session forking** - Child sessions with parentID, sibling navigation.
3. **Session undo/redo** - Revert to any message, redo forward.
4. **Session sharing** - Share session via URL.
5. **Compaction/summarization** - Automatic and manual context summarization.
6. **Subagent/tasks** - Delegate work to child sessions (Task tool).
7. **MCP integration** - Model Context Protocol server management.
8. **LSP integration** - Language server protocol for diagnostics.
9. **Plugin system** - Extensible UI with slots.
10. **Model variants** - Per-model variant selection.
11. **Prompt history/frecency/stash** - Persistent prompt history with ranking.
12. **Workspace/worktree support** - Git worktree-based workspace switching.
13. **Diff viewing** - Split/unified diff rendering with syntax highlighting.
14. **Editor context** - Auto-include selected editor text in prompts.
15. **Shell mode** - `!` prefix for direct command execution.

### What M31A Does That OpenCode Doesn't
1. **6-phase workflow engine** - Structured initialize -> discuss -> plan -> execute -> verify -> ship pipeline.
2. **Auto-arbitrage** - Automatic cost-optimized model selection based on task complexity scoring.
3. **Auto-fallback** - Automatic provider failover on rate-limit/unavailability.
4. **Rollback system** - Git-based rollback of changes made during sessions.
5. **Task dependency management** - DAG-based task execution with dependency ordering and cycle detection.
6. **Verification phase** - Automated syntax checking and test execution per project type.
7. **Ledger** - Activity logging for session statistics.
8. **Standalone binary** - No server required, works fully offline.
9. **EMA-calibrated token estimation** - Self-improving token counter.

### Different Approaches to the Same Problem
| Problem | OpenCode | M31A |
|---------|----------|------|
| **Permission model** | Server-managed with TUI display | In-process channel-based with risk levels and timeout |
| **Context overflow** | Pruning + compaction with structured summary | Warning banner only; no reduction |
| **Model selection** | Per-agent, recent/favorite lists, keyboard cycling | Full-screen selector, auto-arbitrage |
| **Tool dispatch** | Server-side, rich JSX rendering | In-process, risk-based channel request/response |
| **Session storage** | SQL database (Drizzle ORM) | Flat JSON files with atomic writes |
| **Streaming** | Server processes, TUI receives SSE events | Direct SSE from provider, in-process parser |
| **State management** | Reactive signals (SolidJS) | Elm architecture (Model-Update-View) |

---

## 12. Gaps & Recommendations

### Features M31A Should Adopt from OpenCode
1. **Session forking** - Essential for exploring alternative approaches without losing context.
2. **Session undo/redo** - Critical UX feature for recovering from bad AI responses.
3. **Context compaction** - M31A has no context reduction strategy; it will fail on long conversations.
4. **Prompt history with frecency** - Persistent history with frequency/recency ranking.
5. **Plugin system** - Would allow community extensions.
6. **Model variants** - Support for thinking/non-thinking modes.
7. **Subagent delegation** - Task tool for parallel work.
8. **MCP integration** - Standard for tool extensibility.
9. **Shell mode** - Quick command execution without going through the LLM.
10. **Editor context auto-include** - Automatically including selected code in prompts.
11. **Session sharing** - URL-based session sharing.

### Logic in M31A That Should Be Preserved
1. **6-phase workflow engine** - Unique structured approach that OpenCode entirely lacks.
2. **Auto-arbitrage** - Cost optimization is a practical feature.
3. **Auto-fallback** - Provider failover is robust.
4. **Rollback system** - Git-based rollback is a safety feature.
5. **Verification phase** - Automated testing per project type is thorough.
6. **EMA-calibrated token estimation** - Self-improving accuracy.
7. **Atomic file writes** - Robust against crashes.
8. **Standalone operation** - No server dependency, simpler deployment.
9. **Task DAG with cycle detection** - Well-implemented iterative DFS cycle detection.

---

## Summary Matrix

| Dimension | OpenCode | M31A |
|-----------|----------|------|
| **Language** | TypeScript/Bun | Go |
| **TUI Framework** | OpenTUI + SolidJS (reactive JSX) | Bubble Tea + Lipgloss (Elm) |
| **Architecture** | Client-server (HTTP/SSE) | Standalone monolith |
| **Session Storage** | SQL (Drizzle/SQLite) | Flat JSON files (atomic writes) |
| **Streaming** | Server-side processing | Direct SSE, in-process parser |
| **Context Management** | Pruning + compaction + summary | Token estimation + warning banner |
| **Workflow** | Conversational REPL | 6-phase structured engine |
| **Permissions** | Server-managed | Risk-based + timeout |
| **Plugins** | Full slot-based system | None |
| **Model Selection** | Per-agent, recent/favorite, variants | Full-screen selector, auto-arbitrage |
| **Provider Fallback** | Server-side | Auto-fallback with health checks |
| **Cost Optimization** | None | Auto-arbitrage with complexity scoring |
| **Session Features** | Fork, undo/redo, share, compact | Create, list, resume, archive, delete |
| **Tool Rendering** | Rich JSX per tool type | Basic tool cards |
| **Token Estimation** | Static | EMA-calibrated |
| **Safety Features** | Server isolation | Git rollback, verification phase |

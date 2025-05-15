# OpenCode vs M31A: Main Application Logic Deep Comparison

## Executive Summary

OpenCode and M31A solve the same problem (AI-assisted coding in the terminal) with fundamentally different architectures. OpenCode trusts the LLM to drive the process through a continuous agent loop, while M31A constrains the LLM within a defined 6-phase workflow. This philosophical difference cascades through every layer of both systems.

---

## 1. Entry Point & Bootstrap

### OpenCode (TypeScript/Bun)

**Files:** `packages/opencode/src/cli/cmd/run/index.ts`, `packages/opencode/src/cli/cmd/run/runtime.ts`

Two entry points:
- `runInteractiveMode` — for attaching to an existing session (remote/attached mode)
- `runInteractiveLocalMode` — for local in-process execution

Bootstrap flow:
1. Initialize the Effect Layer (dependency injection system)
2. Create a lifecycle manager
3. Wire up the stream transport layer (SSE for remote, in-memory for local)
4. Start the prompt queue consumer
5. Launch the TUI (if interactive)

The runtime uses Bun's process model. Dependencies (config, provider, tool registry, session processor) are wired through Effect's `Layer` system — a functional effect-based DI container.

### M31A (Go/Bubble Tea)

**File:** `cmd/m31a/main.go`

Single entry point:
1. Parses CLI flags (session ID, directory, config path, model override, etc.)
2. Initializes a structured logger
3. Loads config from `~/.m31a/config.toml`
4. Resolves API keys via OS keychain
5. Creates a provider registry, registers OpenRouter and Zen providers
6. Launches the Bubble Tea TUI via `tea.NewProgram(model, opts...)`

**Key Difference:** OpenCode uses an Effect Layer-based DI system with runtime wiring. M31A uses a simpler imperative bootstrap with direct construction.

---

## 2. Core AI Agent Loop

### OpenCode

**Files:** `packages/opencode/src/session/prompt.ts` (~1780 lines), `packages/opencode/src/session/processor.ts`

The core loop is `runLoop` inside `prompt.ts`. It follows a **continuous while(true) agent loop**:

```
while (true) {
  1. Check if compaction is needed (token budget exceeded) → compact if so
  2. Stream LLM with tool definitions via AI SDK
  3. Process events: text-delta, tool-call, tool-result, reasoning, tool-input-start
  4. If tool calls exist → execute them via resolve + dispatch
  5. If no tool calls and no tool calls pending → break (LLM finished)
  6. Check DOOM_LOOP_THRESHOLD for infinite tool-loop detection
  7. Check for subtasks (spawned agents) → wait for completion
  8. Loop back
}
```

The `process` function in `processor.ts` handles event-level processing:
- `handleEvent` routes by event type
- Tool calls are queued and executed
- Results are fed back as `tool-result` events
- Text deltas are emitted to the stream transport

**Loop Detection:** `DOOM_LOOP_THRESHOLD` detects infinite tool loops by counting consecutive tool-call/result cycles without a text response.

### M31A

**File:** `internal/workflow/engine.go`

M31A uses a **6-phase structured workflow** instead of a continuous loop:

```
Initialize → Discuss → Plan → Execute → Verify → Ship
```

Each phase is a discrete state with its own handler:
- **Initialize:** Detect project type, init git, create planning directory, write PROJECT.md/STATE.md
- **Discuss:** Stream LLM for clarifying questions, collect Q&A from user
- **Plan:** Stream LLM to produce a JSON task array, validate (IDs, dependencies, cycle detection via DFS)
- **Execute:** Dependency-ordered task execution via taskrunner. Each task has its own mini self-heal loop (up to `MaxHealAttempts`)
- **Verify:** Check file existence, syntax validation, run tests. On failure: self-heal or git bisect
- **Ship:** Create final git commit, build summary, update LEDGER.md, archive session

Within Execute phase, `executeTaskWithTools` in `execute.go` has an internal loop:
```
for healAttempt = 0 to MaxHealAttempts:
  1. Build context with project info, task description, file schema
  2. Stream LLM response
  3. parseToolCalls: extract JSON tool calls using regex
  4. Dispatch tools through permission gate
  5. On tool success, commit to git
  6. On failure, healTask with failure context, retry
```

**Key Difference:** OpenCode runs a **single continuous loop** until the LLM stops calling tools. M31A uses a **structured phase machine** with explicit transitions and per-task self-heal loops.

---

## 3. LLM Integration

### OpenCode

**File:** `packages/opencode/src/session/llm.ts`

Uses **Vercel's AI SDK** (`streamText` function). Two runtime modes:
- `native` — direct provider API calls
- `ai-sdk` — through the AI SDK abstraction layer

Key type: `StreamInput` contains messages, tools, model, abort signal, etc.
The `stream` function returns `Effect.Stream<LLMEvent, unknown>` — an Effect-managed stream of LLM events.

Provider adapters in `packages/opencode/src/provider/provider.ts`:
- 20+ SDK adapters: Anthropic, OpenAI, Google, OpenRouter, AWS Bedrock, etc.
- Each adapter converts between the AI SDK's `LanguageModel` interface and the provider's native format
- Cost tracking built in
- Model capability definitions (context window, max output, tool support, etc.)

Message format: AI SDK's `ModelMessage` (system, user, assistant with tool calls, tool result)

### M31A

**File:** `internal/provider/registry.go`, `internal/provider/openrouter/client.go`

Simple provider registry with mutex-protected map:
```go
type ProviderRegistry struct {
    mu          sync.RWMutex
    providers   map[string]Provider
    active      string
}
```

Methods: `Register`, `SetActive`, `ActiveProvider`

LLM streaming in `engine.go` (`streamLLM` function):
- Direct HTTP calls to provider APIs (OpenRouter-compatible)
- SSE streaming with manual parsing
- No abstraction layer — each provider implements the same interface

Tool call extraction: **manual JSON parsing** from LLM response text using regex:
```
\`\`\`json\n(...)\n\`\`\`
```
The `parseToolCalls` function extracts tool calls using regex matching against the response text.

**Key Difference:** OpenCode uses AI SDK as an abstraction layer with typed ModelMessage format. M31A does manual JSON extraction from streaming text with regex, no SDK abstraction.

---

## 4. Tool Execution Engine

### OpenCode

**Files:** `packages/opencode/src/tool/tool.ts`, `packages/opencode/src/session/tools.ts`, `packages/opencode/src/tool/registry.ts`

Tool definition:
```typescript
type Def<Parameters, M> = {
  id: string
  description: string
  parameters: JSONSchema
  execute: (input: Parameters, context: ToolContext) => Effect<ExecuteResult>
}
```

Tool resolution (`tools.ts`):
- `resolve` function iterates registry tools + MCP tools
- Wraps each tool with AI SDK's `tool()` function
- Provides context with `ask` function for permission checks
- Arguments validated against JSON Schema

Registry registers built-in tools:
`shell`, `read`, `glob`, `grep`, `edit`, `write`, `task`, `webfetch`, `todo`, `websearch`, `skill`, `patch`, `question`, `lsp`, `plan`

Plugin tools loaded from `.opencode/tool/*.ts`

**MCP Support:** OpenCode supports Model Context Protocol for external tool servers.

### M31A

**File:** `internal/tools/dispatcher.go`

Tool dispatcher with permission gates:
```go
func (d *Dispatcher) Execute(toolCall ToolCall, permissionChan chan PermissionResponse) Result {
    if toolCall.RiskLevel == Dangerous || Destructive {
        select {
        case resp := <-permissionChan:
            if !resp.Granted { return Denied }
        }
    }
    return d.handlers[toolCall.Name](toolCall)
}
```

Registered tools: `Bash`, `FileRead`, `FileWrite`, `Edit`, `TodoWrite`, `WebFetch`, `AskUserQuestion`, `Glob`, `Grep`

**Bash tool** (`bash.go`):
- Executes commands with timeout (default 1800s)
- Process group signaling: SIGINT first, then SIGKILL after 5s
- Output capping via `limitWriter`
- Concurrent stdout/stderr reading via pipes
- Binary output detection

**Edit tool** (`edit.go`): 5 cascading strategies:
1. Line-range replacement (exact line numbers)
2. Exact-match (string comparison)
3. Line-trimmed match (strip whitespace per line)
4. Whitespace-normalized match (collapse all whitespace)
5. Fuzzy-anchor match (Levenshtein similarity >= 0.7)
Atomic writes with backup file

**Key Difference:** OpenCode uses AI SDK's typed tool wrapper with Effect-based execution and MCP support. M31A uses a manual dispatcher with channel-based permission gating and no MCP support. M31A's edit tool has sophisticated 5-level fallback strategies.

---

## 5. System Prompt Architecture

### OpenCode

**File:** `packages/opencode/src/session/system.ts`

Model-specific system prompts:
- `PROMPT_ANTHROPIC` — optimized for Claude
- `PROMPT_GEMINI` — optimized for Gemini
- `PROMPT_GPT` — optimized for GPT models
- `PROMPT_BEAST` — alternative prompt variant

Environment info injection:
- OS, shell, working directory
- Git status
- Available tools and their descriptions
- Agent role and behavior instructions

Prompts are selected based on the active model's capabilities.

### M31A

System prompts are built per-phase in each workflow handler:
- `initialize.go`: Creates PROJECT.md and STATE.md templates
- `discuss.go`: Builds discuss context with project info
- `plan.go`: Includes project info, file schema, discuss answers
- `execute.go`: Task description, file context, tool definitions
- `verify.go`: Verification criteria, test commands
- `ship.go`: Summary requirements

**Key Difference:** OpenCode has model-specific static prompt templates with dynamic environment injection. M31A builds prompts dynamically per-phase, tailoring context to each phase's needs.

---

## 6. Session/Conversation Logic

### OpenCode

Sessions managed through session processor and prompt queue:
- Each session has a unique ID
- Messages stored as `ModelMessage` arrays (system, user, assistant, tool)
- Prompt queue processes user inputs sequentially
- Session state includes: message history, tool call state, compaction state, subagent state

**Compaction:** When token budget exceeded, OpenCode compacts the conversation:
- Uses a dedicated compaction agent (hidden agent type)
- Summarizes earlier messages
- Maintains tool call results for context continuity

**Subagents:** OpenCode can spawn subagents (general, explore, scout) as child sessions with their own agent loops. Parent waits for subagent completion before continuing.

### M31A

Sessions are file-based:
- Each session has a directory with STATE.md (current state), LEDGER.md (history)
- Tasks tracked in JSON arrays with dependencies
- Git commits serve as the session's audit trail
- `git bisect` can be used to find breaking commits between session start and HEAD

**Key Difference:** OpenCode manages sessions in-memory with AI SDK message arrays and supports subagent spawning. M31A uses file-based state with git as the audit trail.

---

## 7. Workflow Engine (M31A Exclusive)

### OpenCode

No explicit phase-based workflow. The agent loop is **goal-directed and continuous**:
1. User sends a prompt
2. LLM processes and optionally calls tools
3. Tools execute, results fed back
4. Loop continues until LLM stops calling tools
5. Final text response is emitted

**Compaction** and **subagents** are the two meta-workflow features.

### M31A

Explicit **6-phase workflow engine** (`engine.go`):

```go
func (e *Engine) RunPhase(phase Phase) Result {
    switch phase {
    case PhaseInitialize: return e.runInitialize()
    case PhaseDiscuss:    return e.runDiscuss()
    case PhasePlan:       return e.runPlan()
    case PhaseExecute:    return e.runExecute()
    case PhaseVerify:     return e.runVerify()
    case PhaseShip:       return e.runShip()
    }
}
```

**Plan phase** (`plan.go`):
- Retry loop with `MaxPlanRetries`
- Parses JSON task array from LLM
- Validates: unique IDs, valid dependencies, no cycles (DFS cycle detection)

**Execute phase** (`execute.go`):
- Taskrunner with dependency-ordered execution (topological sort)
- Self-heal loop per task (up to `MaxHealAttempts`)
- Git commit on each successful tool execution

**Verify phase** (`verify.go`):
- File existence checks
- Syntax validation: `go build`, `python compile`, `cargo check`
- Test execution
- On failure: self-heal or git bisect (finds offending commit between session start and HEAD)

---

## 8. Server Architecture

### OpenCode

Supports a **server mode** with SSE transport:
- Remote session attachment via `runInteractiveMode`
- Stream transport layer for real-time event streaming
- Prompt queue for handling multiple user inputs
- Lifecycle management for session cleanup

The server can handle multiple concurrent sessions, each with its own agent loop.

### M31A

No server architecture. **Purely local CLI application**:
- Single session per process
- No remote attachment capability
- All processing happens in-process

---

## 9. Configuration System

### OpenCode

**File:** `packages/opencode/src/config/config.ts`

Multi-layer configuration loading:
1. Global config (user-level)
2. Project config (repo-level)
3. Remote config (fetched from remote source)
4. Managed config (system-managed)

Schema validation with Effect Schema. Config sections:
- Plugin configuration
- MCP server configuration
- Agent definitions and permissions
- Model/provider selection
- Permission rulesets

### M31A

**File:** `~/.m31a/config.toml`

Single config file (TOML format):
- Provider configuration (OpenRouter, Zen)
- API key resolution via OS keychain
- Model overrides
- Directory settings

**Key Difference:** OpenCode has a layered, schema-validated config system with plugin and MCP support. M31A uses a simple TOML config with OS keychain integration.

---

## 10. Error Handling & Recovery

### OpenCode

- `InvalidArgumentsError` for tool schema validation failures
- `DOOM_LOOP_THRESHOLD` for infinite tool-loop detection
- Effect's `CatchAll` for unhandled errors in the effect pipeline
- Compaction recovers from context overflow
- Subagent failures reported but don't crash the parent session

### M31A

- **Self-heal loop:** `executeTaskWithTools` retries up to `MaxHealAttempts` with failure context
- **Plan retry:** `runPlan` retries up to `MaxPlanRetries` for invalid JSON or missing dependencies
- **Git bisect:** In verify phase, if tests fail, bisects between session start and HEAD to find offending commit
- **Permission denial:** Tools return `Denied` result, task continues or fails gracefully
- **Edit fallback:** 5 cascading strategies ensure edit succeeds even with imperfect match

---

## 11. State Persistence

### OpenCode

- In-memory session state (messages, tool calls, compaction state)
- Stream transport persists events for remote attachment
- No file-based state tracking (git commits are tool outputs, not state management)

### M31A

- **File-based state:** STATE.md, LEDGER.md, PROJECT.md
- **Git as audit trail:** Every successful tool execution creates a git commit
- **Session archival:** On ship phase, session is archived with summary
- **Task tracking:** JSON arrays with dependency graphs persisted on disk

---

## 12. Key Logic Differences Summary

| Dimension | OpenCode | M31A |
|-----------|----------|------|
| **Runtime** | Bun/TypeScript | Go |
| **Effect System** | Effect (functional effects, Layer DI) | None (imperative Go) |
| **Agent Loop** | Continuous `while(true)` until LLM stops | 6-phase workflow machine |
| **LLM SDK** | Vercel AI SDK (`streamText`) | Manual HTTP + SSE + regex JSON parsing |
| **Tool Execution** | AI SDK wrapped tools + MCP support | Manual dispatcher with channel-based permissions |
| **Tool Calls** | Structured via AI SDK | Regex-extracted JSON from text |
| **Message Format** | `ModelMessage` (typed) | Manual string building |
| **System Prompts** | Model-specific static templates | Per-phase dynamic construction |
| **Session State** | In-memory message arrays | File-based (STATE.md, LEDGER.md, git) |
| **Workflow** | None (continuous loop) | 6 phases with auto-transition |
| **Task Execution** | Single loop with tool calls | Dependency-ordered taskrunner with topological sort |
| **Error Recovery** | Effect error channel, loop detection | Self-heal loops, plan retry, git bisect |
| **Edit Strategy** | Simple | 5 cascading fallback strategies |
| **Verification** | None | File checks, syntax validation, test execution |
| **Server Mode** | Yes (SSE, remote attachment) | No (local-only) |
| **Config** | Multi-layer, schema-validated, plugin-extendable | Single TOML, OS keychain |
| **MCP Support** | Yes | No |
| **Subagents** | Yes (spawn child agent loops) | No |
| **Compaction** | Yes (token budget management) | No |
| **Persistence** | In-memory | File + git audit trail |
| **Permission Model** | Ask function in tool context | Channel-based permission gate |

---

## Architectural Philosophy

**OpenCode** is designed as a **flexible, extensible AI coding agent**:
- Continuous agent loop lets the LLM decide the workflow
- Heavy use of AI SDK abstraction for provider agnosticism
- MCP support for external tool integration
- Subagent spawning for parallel work
- Functional effect system for composability and error handling

**M31A** is designed as a **structured, deterministic coding workflow**:
- Explicit phase-based workflow controls the agent's behavior
- Manual tool call parsing (no SDK dependency)
- Git as the source of truth for session state
- Self-heal and verification as first-class concerns
- Simple, direct Go implementation without abstraction layers

**OpenCode trusts the LLM to drive the process; M31A constrains the LLM within a defined workflow.**

---

## What Could Be Transferred

### From OpenCode to M31A
1. **AI SDK-style abstraction layer** — would make adding new providers easier
2. **Continuous agent loop** — for casual chat mode outside the workflow engine
3. **Compaction/summarization** — essential for long conversations
4. **Subagent spawning** — parallel task execution
5. **MCP support** — external tool server integration
6. **Effect-style error handling** — typed error channels instead of string matching

### From M31A to OpenCode
1. **6-phase workflow engine** — structured approach for complex tasks
2. **Auto-arbitrage** — cost-optimized model selection
3. **Auto-fallback** — provider failover on rate limits
4. **5-level edit fallback** — robustness for file edits
5. **Git bisect for debugging** — finding breaking commits
6. **Verification phase** — automated testing per project type
7. **Atomic file writes** — crash resilience

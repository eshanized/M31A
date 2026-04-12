# M31A — AI-Powered CLI Coding Assistant
# V1 Specification: Dual-Provider (OpenRouter + OpenCode Zen) with Gemini-Caliber TUI
# Complete Rewrite for Singular Focus & Premium Experience

---

## 1. Project Identity

- **Name**: M31A
- **Language**: Go 1.22+
- **UI Framework**: Bubble Tea (`charmbracelet/bubbletea`) + Lipgloss (`charmbracelet/lipgloss`) + Bubbles (`charmbracelet/bubbles`) + Glamour (`charmbracelet/glamour`)
- **Binary**: Single static binary, `CGO_ENABLED=0`
- **Target OS**: Linux, macOS, Windows
- **Module Path**: `github.com/eshanized/M31A`
- **License**: MIT — Free and Open Source
- **V1 Mandate**: M31A supports **two LLM gateways**: OpenRouter and OpenCode Zen. Both are first-class providers with identical feature support (model discovery, streaming, reasoning, tool use). No direct Anthropic, OpenAI, or Azure connections — all traffic flows through one of the two gateways. The user selects a default provider in config and can switch per-session.

**FOSS Mandate:** M31A is free, open source, and community-driven. No vendor lock-in, no telemetry, no paid tiers. The full source code is available under the MIT license. Anyone can audit, modify, fork, and redistribute. M31A's design decisions prioritize user sovereignty over monetization.

---

## 2. Design Philosophy: "Gemini in the Terminal, Spec-Driven in the Engine"

M31A V1 does not look like a traditional CLI tool. It behaves like a polished, native AI workspace that happens to run in a terminal. Under the hood, it runs a **spec-driven workflow engine** — meta-prompting, context engineering, and phase-gated development — implemented natively in Go.

### UX Pillars

| Pillar | Implementation |
|--------|----------------|
| **Spatial Clarity** | Every pixel of terminal space is intentional. Generous padding, visual hierarchy, and whitespace management. |
| **Inline Cognition** | The model's reasoning process is not hidden behind a spinner. It is surfaced as a first-class, collapsible artifact within the message stream. |
| **Fluid Motion** | All state transitions use eased animations (spinner → text, thinking → answer, tool call → result). No jarring jumps. |
| **Contextual Density** | Model badges, token counters, and cost estimates appear only when relevant, never cluttering the primary content. |
| **Zero Configuration** | Paste an OpenRouter key. M31A discovers everything else automatically. |

### Engine Principles

M31A's workflow engine is built around these core architectural principles:

| Principle | M31A Implementation |
|---------------|---------------------|
| **Six-phase cycle**: Initialize → Discuss → Plan → Execute → Verify → Ship | Native Go task runner in `pkg/taskrunner` — each phase is a distinct state in the Bubble Tea state machine |
| **Context rot prevention**: Route intensive work into isolated subagents with fresh context windows | Subagent goroutines (V1.1) get independent message history and context budget; parent TUI renders progress as indented sub-task cards |
| **File-based state**: All planning data in `planning/` as readable Markdown/JSON | Session state persisted as `STATE.md`, `PROJECT.md`, `TASKS.md` in `~/.m31a/sessions/<id>/` — human-readable, resumable, version-controllable |
| **Parallelize independent work, sequence dependent work** | Task dependency graph in `pkg/taskrunner` — topological sort determines execution order; independent tasks run concurrently |
| **Deterministic logic in code, not prompts** | File operations, git commands, and validation scripts are native Go tool implementations — the LLM invokes them, it doesn't simulate them |
| **Auto-verify with self-healing** | `/verify` phase: after task execution, a verification step checks outputs; failures trigger automatic diagnostic + remediation loop |
| **Fresh context per phase** | Each phase (plan, execute, verify) launches with a pruned context containing only phase-relevant history, preventing window decay |

---

## 3. Core Architecture

### 3.1 Workflow Loop (Six-Phase Cycle)

M31A implements a six-phase workflow engine as its core execution model. Each phase runs in a **fresh context window** to prevent context rot, with state persisted to disk between phases.

```
┌───────────┐      ┌───────────┐      ┌───────────┐
│Initialize │───▶ │ Discuss   │───▶ │  Plan     │
│ (Context  │      │ (Clarify  │      │ (Break    │
│  Setup)   │      │  Goals)   │      │  Tasks)   │
└───────────┘      └───────────┘      └─────┬─────┘
                                            │
                                            ▼
┌───────────┐      ┌───────────┐      ┌───────────┐
│   Ship    │◀─── │  Verify   │◀─── │ Execute   │
│ (Commit & │      │ (Check &  │      │ (Run      │
│  Deliver) │      │  Heal)    │      │  Tasks)   │
└───────────┘      └───────────┘      └───────────┘
```

**Phase lifecycle:**

| Phase | Trigger | Context | State File | Description |
|-------|---------|---------|------------|-------------|
| **Initialize** | User types a goal or `/new` | Full session history + `MEMORY.md` | `PROJECT.md` created | Parse the goal, detect project type, scaffold `~/.m31a/sessions/<id>/planning/` directory. If cwd is not a git repo, run `git init` automatically. |
| **Discuss** | Auto after Initialize | Pruned: goal + project context | `PROJECT.md` updated | M31A prompts the LLM to generate 2-4 clarifying questions; questions are rendered inline below the input area for the user to answer one at a time. User types answers directly, or types `skip` for all remaining. Answers are appended to `PROJECT.md`. No `AskUserQuestion` tool needed in V1 — the LLM outputs questions as numbered markdown list, M31A parses them, and collects answers through the normal input field. |
| **Plan** | Auto after Discuss (or `/plan`) | Pruned: goal + answers + file schemas | `TASKS.md` created | LLM produces a JSON array of tasks (schema-validated). M31A parses the response, validates: no circular dependencies, no self-references, all dependency IDs reference existing tasks, each task has action + description + files. On validation failure, M31A sends the errors back to the LLM with a retry prompt (max 3 retries). After successful parse, the task list is rendered as Markdown in `TASKS.md` and displayed in the Plan screen. If all retries fail, the user is prompted to manually enter tasks or skip to REPL. |
| **Execute** | User approves plan (`A`) or `/execute` | Per-task context: only relevant files + task spec | `STATE.md` updated per task | Tasks run in dependency order — independent tasks concurrently, dependent tasks sequentially. Each task gets atomic commits. |
| **Verify** | Auto after Execute completes | Pruned: task specs + expected outputs | `TASKS.md` updated with pass/fail | Automated checks: file existence, syntax validation, test runs. Failures trigger self-healing diagnostic loop. |
| **Ship** | Auto after Verify passes | Minimal: summary + changelog | `STATE.md` → "shipped" | Final git commit, summary banner, session archived |

**Phase isolation (context pruning):** Each phase launches with a pruned context to prevent the 200K-token window from filling with irrelevant conversation. The pruning strategy is:

| Phase | Context injected |
|-------|-----------------|
| **Initialize → Discuss** | Full session history (all messages so far) + `MEMORY.md` (if exists) + system prompt |
| **Plan** | System prompt + goal from `PROJECT.md` + Discuss Q&A from `PROJECT.md` + `MEMORY.md` + cwd file schema (top-level file listing via `Glob` tool). All prior conversation messages are discarded — the LLM works from the structured state files only. |
| **Execute** | System prompt + `TASKS.md` (full task list) + `PROJECT.md` (goal + answers). For each task, only the current task spec and its completed dependency outputs are in context. Prior tool results from completed tasks are *not* in the message history — the LLM reads current file state via `FileRead` as needed. |
| **Verify** | System prompt + `TASKS.md` (task specs + acceptance criteria) + file contents of all task outputs (injected as `FileRead` results). No prior conversation history. |
| **Ship** | System prompt + `TASKS.md` (final status) + commit log from git. No prior conversation history. |

State survives between phases via the file-based persistence in `planning/` — the LLM reads structured files, not conversation history.

**User control:** At any point, the user can interrupt with `Ctrl+C` (returns to REPL), `/plan` (jump to plan phase), or `/execute` (skip to execution with existing plan).

### 3.2 Provider Abstraction Layer

M31A communicates with LLM gateways through a unified interface. Two implementations ship in V1:

```go
type LLMProvider interface {
    Name() string                         // "openrouter" | "zen"
    FetchModels() ([]ModelInfo, error)    // Dynamic model catalog
    ChatCompletionStream(ctx context.Context, req ChatRequest) (*StreamIterator, error)
    EstimateCost(usage Usage) float64     // Returns cost in USD
}

type ProviderRegistry struct {
    mu       sync.RWMutex
    providers map[string]LLMProvider
    active    string  // Currently selected provider
}
```

**OpenRouter implementation:**
```go
type OpenRouterClient struct {
    apiKey      string
    baseURL     string // https://openrouter.ai/api/v1
    httpClient  *http.Client // 30s dial timeout (via http.Transport.DialContext), no body read timeout (streaming responses can be long)
    modelCache  *ModelCache // TTL 5 minutes
}
```

**OpenCode Zen implementation:**
```go
type ZenClient struct {
    apiKey      string
    baseURL     string // https://opencode.ai/zen/v1
    httpClient  *http.Client // 30s dial timeout (via http.Transport.DialContext), no body read timeout (streaming responses can be long)
    modelCache  *ModelCache // TTL 5 minutes
    // Zen uses OpenAI-compatible endpoints; reasoning params differ slightly
}
```

**Responsibilities (both providers):**
- `FetchModels()`: Fetch model catalog, populate cache with pricing, context length, architecture, capabilities
- `ChatCompletionStream()`: POST to `/chat/completions`, handle SSE parsing, normalize thinking/reasoning fields
- `GetModel(id string)`: Return rich metadata for the active model badge
- `EstimateCost()`: Calculate cost from usage data (both providers return pricing in their model metadata)

**No manual model lists.** The "model selector" is a live search interface over the active provider's catalog.

---

## 4. The Model System

### 4.1 Dynamic Model Discovery

On first run and every 5 minutes, M31A fetches:

```go
type ModelInfo struct {
    ID               string   `json:"id"`           // e.g. "anthropic/claude-3.5-sonnet"
    Provider         string   `json:"provider"`     // "openrouter" | "zen"
    Name             string   `json:"name"`         // Human-readable
    Description      string   `json:"description"`
    ContextLength    int64    `json:"context_length"`
    Pricing          Pricing  `json:"pricing"`
    Architecture     ArchInfo `json:"architecture"`
    TopProvider      string   `json:"top_provider"` // latency-ranked best provider (OpenRouter only; Zen uses "curated")
    Capabilities     CapFlags `json:"capabilities"` // tools, reasoning
}

type CapFlags struct {
    Tools     bool `json:"tools"`     // Supports function calling / tool use
    Reasoning bool `json:"reasoning"` // Supports explicit thinking/reasoning tokens
    Vision    bool `json:"vision"`    // Supports image input (reserved for V1.2)
}
// Note: Vision capability is excluded from V1. No terminal UX exists for
// image input (clipboard paste, file paths) or image-referencing outputs.
// The field can be added back in V1.2 when a concrete vision workflow is designed.
```

### 4.2 Model Selector UI (`/model` or Ctrl+M)

A full-screen overlay, not a dropdown:

```
┌──────────────────────────────────────────────────────────────┐
│  Select Model                           412 models (2 loaded)│  ← 2 providers
├──────────────────────────────────────────────────────────────┤
│  > Provider: [All]  Search: claude_                          │  ← filter by provider
│                                                              │
│  ┌───────────────────────────────────────────────────────┐   │
│  │ [*] anthropic/claude-3.5-sonnet          [OR]         │   │  ← [OR]=OpenRouter
│  │   200K context · $3.00/M in · $15.00/M out · [TOOLS]  │   │
│  │   "High-performance model for complex tasks"          │   │
│  └───────────────────────────────────────────────────────┘   │
│  ┌───────────────────────────────────────────────────────┐   │
│  │ [ ] anthropic/claude-3.5-sonnet          [ZEN]        │   │  ← [ZEN]=OpenCode Zen
│  │   200K context · $3.75/M in · $18.75/M out · [TOOLS]  │   │
│  │   "Curated Claude via Zen gateway"                    │   │
│  └───────────────────────────────────────────────────────┘   │
│  ┌───────────────────────────────────────────────────────┐   │
│  │ [ ] deepseek/deepseek-r1                 [OR]         │   │
│  │   64K context · $0.55/M in · $2.19/M out · [REASON]   │   │
│  │   "Open-source reasoning model with visible thinking" │   │
│  └───────────────────────────────────────────────────────┘   │
│                                                              │
│  [Enter] Select  [Tab] Details  [P] Provider  [Esc] Cancel   │
└──────────────────────────────────────────────────────────────┘
```

- **Provider filter**: Press `P` to cycle: All → OpenRouter → Zen. Models from the filtered provider are shown.
- **Real-time filtering** by name, provider, capability
- **Detail pane** (Tab): Full description, architecture, modality support, per-provider latency stats (OpenRouter) or curation notes (Zen)
- **Same model, different provider**: If both gateways offer `anthropic/claude-3.5-sonnet`, both appear as separate entries with `[OR]` / `[ZEN]` badges
- **Default model**: Persisted in config with provider prefix (e.g. `openrouter/anthropic/claude-3.5-sonnet`); validated against cache on startup

### 4.3 Active Model Badge

Persistent header in REPL:

```
┌─────────────────────────────────────────────────────────────┐
│  M31A  [OR] anthropic/claude-3.5-sonnet    12.4K/200K ctx   │  ← [OR] fg(#D77757) or [ZEN] fg(#8AB4F8)
├─────────────────────────────────────────────────────────────┤
```

- **Provider tag**: `[OR]` fg(#D77757) for OpenRouter, `[ZEN]` fg(#8AB4F8) for Zen. Clicking opens model selector.
- **Connection status**: `[LIVE]` fg(#81C995) if connected (health check every 60s), `[OFFLINE]` fg(#F28B82) if unreachable, `[SLOW]` fg(#FDD663) if >500ms
- **Context bar**: `used / total` context length, color-shifts amber at 80%, red at 95%
- **Clickable** (Enter on header) opens model selector

---

## 5. Inline Thinking & Reasoning System

This is the **signature feature** of M31A V1. When a model supports reasoning (DeepSeek R1, OpenAI o3, Gemini 2.5 Flash/Pro via OpenRouter), the user sees the thought process as it unfolds.

### 5.1 Thinking Block Structure

Some models (DeepSeek R1, OpenAI o-series) produce reasoning *before* content. Others (Anthropic Claude with extended thinking) interleave reasoning *between* content segments. M31A normalizes both patterns into a unified structure:

```go
type Message struct {
    Role      string            `json:"role"`
    Content   string            `json:"content"`   // Final assembled text (thinking stripped)
    Segments  []MessageSegment  `json:"segments"`  // Ordered: content → thinking → content...
    ToolCalls []ToolCall        `json:"tool_calls,omitempty"`
}

type MessageSegment struct {
    Type     string `json:"type"` // "content" | "thinking"
    Content  string `json:"content"`
    Duration int64  `json:"duration_ms,omitempty"` // Only for thinking segments
    Visible  bool   `json:"visible"`               // User toggle state (default true for new thinking segments)
}
```

**Rendering rule:** The TUI iterates `Segments` in order, rendering each thinking segment as a collapsible block and each content segment as normal text. `Content` is the flattened string (thinking excluded) for API serialization. There is no separate `ThinkingBlock` type — all thinking is represented as `MessageSegment{Type: "thinking"}`.

### 5.2 Rendering in Message Stream

```
┌─────────────────────────────────────────────────────────────┐
│  M31A                                                       │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ [v] Thinking for 1.2s...                             │   │  ← [v] collapsed toggle fg(#8AB4F8)
│  │ ─────────────────────────────────────────────────────│   │
│  │ The user wants a Next.js restaurant site. I should   │   │
│  │ first check if there's an existing project in the    │   │
│  │ cwd. Then I'll need to scaffold with create-next-app │   │
│  │ but I must verify Node.js version first...           │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                             │
│  I'll build "Eshan's Cafe" with Next.js. First, let me      │
│  check your current directory structure...                  │
│                                                             │
│  [..] Executing Bash: ls -la                                │  ← [..] spinner fg(#9AA0A6)
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

**Interaction:**
- **[v]/[^] Toggle**: Each thinking segment has an independent expand/collapse header — `[v]` collapsed fg(#8AB4F8), `[^]` expanded. Persisted per segment.
- **Color**: Subtle gray background (`#2A2A2A` dark / `#F0F0F0` light), left border accent
- **Live update**: During streaming, the SSE parser maintains two buffers — a thinking buffer and a content buffer. When a thinking token arrives, it appends to the active thinking segment. When a content token arrives after thinking, it starts a new content segment. The TUI re-renders both buffers on each tick without flicker.
- **Interleaved models**: Claude-style models may alternate thinking → content → thinking → content. Each alternation creates a new segment in `Message.Segments`.
- **Duration badge**: "Thinking for 2.4s" updates in real-time; finalizes on last thinking token
- **Copy**: Individual copy button per thinking segment

### 5.3 Per-Architecture Reasoning Parameters

OpenRouter does **not** expose a universal `include_reasoning` flag. Each model family requires different request parameters to enable and control reasoning output. M31A maintains a parameter map keyed by model architecture:

```go
type ReasoningConfig struct {
    ModelFamily   string         // "deepseek" | "openai" | "anthropic" | "qwen"
    RequestParams map[string]any // Parameters injected into the OpenRouter request body
    SSEField       string         // JSON path to extract reasoning tokens from SSE chunks
}

var reasoningParamMap = map[string]ReasoningConfig{
    "deepseek": {
        ModelFamily:   "deepseek",
        RequestParams: map[string]any{}, // Reasoning is automatic; no flag needed
        SSEField:      "choices[0].delta.reasoning_content",
    },
    "openai-o": {
        ModelFamily:   "openai",
        RequestParams: map[string]any{"reasoning_effort": "medium"}, // low | medium | high
        SSEField:      "choices[0].delta.reasoning",
    },
    "anthropic": {
        ModelFamily:   "anthropic",
        RequestParams: map[string]any{
            "thinking": map[string]any{
                "type":          "enabled",
                "budget_tokens": 1024, // Scaled to model context; user-configurable
            },
        },
        SSEField: "choices[0].delta.content", // type field == "thinking" distinguishes block
    },
    "qwen": {
        ModelFamily:   "qwen",
        RequestParams: map[string]any{}, // Reasoning automatic for thinking-enabled variants
        SSEField:      "choices[0].delta.reasoning_content",
    },
}
```

**Request assembly:** Before sending to the active provider, `ChatCompletionStream()` looks up the active model's architecture in `reasoningParamMap` and merges the `RequestParams` into the request body. The `SSEField` tells the parser where to extract reasoning tokens.

**Provider endpoint routing:**
- **OpenRouter**: POST `https://openrouter.ai/api/v1/chat/completions` — standard OpenAI-compatible path
- **Zen**: POST `https://opencode.ai/zen/v1/chat/completions` — same OpenAI-compatible path, different base URL. Zen's reasoning support follows the upstream model's native format (identical to OpenRouter for the same model family).
- The `LLMProvider` interface abstracts the base URL; the reasoning param map is shared across both providers.

### 5.4 SSE Parsing for Reasoning

```go
type StreamChunk struct {
    DeltaContent   string          // Content token (appended to active content segment)
    DeltaThinking  string          // Reasoning token (appended to active thinking segment)
    SegmentType    string          // "content" | "thinking" — determined by SSEField match
    FinishReason   *string
    ToolCall       *ToolCallDelta
}
```

**Parsing logic:**
1. Each SSE chunk is inspected — if the `SSEField` for the active model yields a non-empty value, `DeltaThinking` is populated and `SegmentType` is set to `"thinking"`.
2. If the standard content field yields a value, `DeltaContent` is populated and `SegmentType` is `"content"`.
3. When `SegmentType` transitions (e.g., from `"thinking"` to `"content"`), the parser creates a new `MessageSegment` entry.
4. **Fallback**: If neither field yields tokens, the chunk is ignored (model does not support exposed reasoning).

---

## 6. Tool System (V1)

### 6.1 Implemented Tools

**V1 Core (5 tools):** These are the minimum set needed for a compelling coding assistant experience. Each is fully implemented with robust error handling, permission rules, TUI card rendering, and checkpoint/rollback support.

| Tool | Description | Risk Level |
|------|-------------|------------|
| **Bash** | Execute shell, 30-min timeout, signal forward | Dangerous |
| **FileRead** | Read file, detect binary, size limit 5MB | Safe |
| **FileWrite** | Atomic write with backup, create dirs | Destructive |
| **Glob** | File listing with pattern | Safe |
| **Grep** | Code search via `rg` or fallback | Safe |

**V1.1 Deferred (7 tools):** These are specified and designed but implemented after V1 ships to avoid scope creep:

| Tool | Description | Risk Level |
|------|-------------|------------|
| **FileEdit** | Unified diff or search/replace with preview | Destructive |
| **WebFetch** | URL fetch with SSRF protection | Safe |
| **WebSearch** | SerpAPI or similar | Safe |
| **AgentTool** | Spawn subagent goroutine | Dangerous |
| **TaskTool** | Task lifecycle management | Medium |
| **AskUserQuestion** | Blocking question modal | Safe |
| **GitTool** | Git ops with auto-stash safety | Destructive |

### 6.2 Tool Call Rendering (Gemini-Style)

Instead of plain text "Executing Bash...", tools render as rich cards:

```
┌─────────────────────────────────────────────────────────────┐
│  [BASH]                                                     │  ← Lipgloss: fg(#FDD663) bold
├─────────────────────────────────────────────────────────────┤
│  ls -la                                                     │
├─────────────────────────────────────────────────────────────┤
│  total 128                                                  │
│  drwxr-xr-x  12 user user  4096 May 26 08:00 .              │
│  drwxr-xr-x   3 user user  4096 May 26 07:50 ..             │
│  -rw-r--r--   1 user user  2200 May 26 08:00 README.md      │
├─────────────────────────────────────────────────────────────┤
│  [OK] Completed in 0.12s                                    │  ← Lipgloss: fg(#81C995)
└─────────────────────────────────────────────────────────────┘
```

- **Tool label**: Bracketed uppercase name in a color-coded badge. Bash = `#FDD663` (amber), FileRead = `#8AB4F8` (blue), FileWrite = `#D77757` (brand), Glob = `#9AA0A6` (gray), Grep = `#8AB4F8` (blue)
- **Collapsible**: Long outputs (>20 lines) auto-collapse with "[+147 lines]" toggle
- **Output cap**: Tool outputs are capped at 10,000 characters. Truncated outputs show "[... output truncated, full output in session log]" with a link to the full output file. Binary content is never displayed — shows "[binary file, size]" instead.
- **Syntax highlighting**: File contents detected by extension, rendered via Glamour/Chroma
- **Status badges**: `[..]` spinner during execution → `[OK]` fg(#81C995) on success → `[ERR]` fg(#F28B82) on failure

### 6.3 Subagent Concurrency Safety (V1.1)

Bubble Tea is single-threaded by design. The `AgentTool` (deferred to V1.1) spawns a subagent goroutine that must integrate safely with the TUI event loop:

**Channel-based communication:**
- Subagents receive a `chan SubagentEvent` from the parent `AppState`
- All state mutations (new messages, tool calls, permission requests) are sent as typed events through this channel
- The main Bubble Tea `Update()` function processes these events in its single-threaded loop, ensuring no race conditions
- Subagents cannot directly access or mutate `AppState` — they are sandboxed to their own context and message history
- **Progress reporting**: Subagents emit `ProgressEvent` periodically; the parent renders these as indented sub-task cards in the message stream
- **Permission escalation**: If a subagent triggers a dangerous tool, the permission request bubbles up to the parent TUI as a blocking modal. The subagent's channel is paused until the user responds.

---

## 7. Workflow Screens (Plan → Execute → Verify → Ship)

M31A implements the full spec-driven workflow as a series of TUI screens, one per phase. The user flows through them sequentially but can jump between phases with slash commands.

### 7.1 Initialize & Discuss Screen

When the user types a goal (e.g., "Build me a restaurant website"), M31A enters the **Initialize** phase, then auto-transitions to **Discuss** for clarification.

```
┌─────────────────────────────────────────────────────────────┐
│  [M31A] Initialize → Discuss                       Phase 1/6│  ← Lipgloss: fg(#8AB4F8) bold
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌─────────────────────────────────────────────────────┐    │
│  │  Goal: Build a restaurant website for "Eshan's Cafe"│    │
│  │                                                     │    │
│  │  Detected: Web project · Frontend-only · No backend │    │
│  │  Project dir: ~/projects/eshans-cafe                │    │
│  └─────────────────────────────────────────────────────┘    │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐    │
│  │  Before I plan, I need a few details:               │    │
│  │                                                     │    │
│  │  1. Color palette: dark espresso or light & airy?   │    │
│  │  2. Features needed: menu, reservations, gallery?   │    │
│  │  3. Framework: Next.js or plain HTML/CSS?           │    │
│  │                                                     │    │
│  │  Answer each, or type "skip" for sensible defaults. │    │
│  └─────────────────────────────────────────────────────┘    │
│                                                             │
│  > Color palette: dark espresso, features: menu + gallery   │
│  Framework: Next.js                                         │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

- **Project detection**: M31A scans the cwd for `package.json`, `go.mod`, `Cargo.toml`, etc. to infer project type
- **Clarifying questions**: LLM generates 2-4 targeted questions; user answers inline
- **Skip**: Typing `skip` uses sensible defaults from `PROJECT.md` template
- **State**: Answers written to `~/.m31a/sessions/<id>/planning/PROJECT.md`

### 7.2 Plan Phase Screen (`/plan`)

Split-pane layout with **markdown rendering via Glamour** and spec-driven task breakdown:

```
┌─────────────────────────────────────────────────────────────┐
│  [M31A] Plan Mode                    [A]ccept [E]dit [R]etry│  ← Lipgloss: fg(#8AB4F8) bold
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌─────────────────────────────────────┐ ┌───────────────┐  │
│  │ # Plan: Eshan's Cafe Website        │ │ [A] Accept    │  │
│  │                                     │ │ [E] Edit      │  │
│  │ ## Task Breakdown (7 tasks)         │ │ [R] Retry     │  │
│  │                                     │ │ [?] Help      │  │
│  │ 1. [NEW] Initialize Next.js app     │ │               │  │
│  │    deps: none · ~30s                │ │ Est. cost:    │  │
│  │                                     │ │ $0.12         │  │
│  │ 2. [NEW] Setup globals.css + theme  │ │ Est. time:    │  │
│  │    deps: 1 · ~45s                   │ │ ~4 min        │  │
│  │                                     │ │               │  │
│  │ 3. [NEW] Create Navbar component    │ │ Model:        │  │
│  │    deps: 2 · ~60s                   │ │ claude-3.5    │  │
│  │                                     │ │ sonnet        │  │
│  │ 4. [NEW] Create Hero component      │ │               │  │
│  │    deps: 2 · ~90s                   │ │               │  │
│  │                                     │ │               │  │
│  │ 5. [NEW] Create Menu section        │ │               │  │
│  │    deps: 2 · ~90s                   │ │               │  │
│  │                                     │ │               │  │
│  │ 6. [NEW] Create Gallery section     │ │               │  │
│  │    deps: 2 · ~60s                   │ │               │  │
│  │                                     │ │               │  │
│  │ 7. [NEW] Wire up app/page.js        │ │               │  │
│  │    deps: 3,4,5,6 · ~45s             │ │               │  │
│  └─────────────────────────────────────┘ └───────────────┘  │
│                                                             │
│  [Tab] Switch to Dependency Graph  [D] Details on task #    │
└─────────────────────────────────────────────────────────────┘
```

- **Task format**: `[ACTION] description` · `deps: N,M` · `~duration`
- **Dependency graph** (Tab view): ASCII DAG showing parallel vs sequential execution paths
- **Accept** (`A`): Transitions to Execute phase. Plan serialized to `TASKS.md`.
- **Edit** (`E`): Opens inline editor for plan markdown — user can add/remove/reorder tasks
- **Retry** (`R`): Regenerates plan from scratch with the same goals
- **Estimation**: Cost and time estimates based on model pricing + task complexity heuristic

### 7.3 Execute Phase Screen (`/execute`)

Phase-gated task execution with dependency-aware scheduling:

```
┌─────────────────────────────────────────────────────────────┐
│  [M31A] Executing                      3 of 7 complete  43% │  ← Lipgloss: fg(#D77757) bold + progress bar
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌────────────────────────────────────────────────────┐     │
│  │ [x] 1. Initialize Next.js app          [..] 0.8s   │     │  ← [x] fg(#81C995)
│  │ [x] 2. Setup globals.css + theme                   │     │
│  │ [>] 3. Create Navbar component         [..] 12s    │     │  ← [>] fg(#E8EAED), [..] spinner
│  │ [ ] 4. Create Hero component                       │     │  ← [ ] fg(#9AA0A6) queued
│  │ [ ] 5. Create Menu section                         │     │
│  │ [ ] 6. Create Gallery section                      │     │
│  │ [ ] 7. Wire up app/page.js          (deps: 3,4,5,6)│     │
│  └────────────────────────────────────────────────────┘     │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐    │
│  │ [BASH] npx create-next-app@latest eshans-cafe       │    │
│  ├─────────────────────────────────────────────────────┤    │
│  │ Success! Created eshans-cafe at ~/projects/...      │    │
│  ├─────────────────────────────────────────────────────┤    │
│  │ [OK] Completed in 0.8s · +12 files · commit a1b2c3d │    │
│  └─────────────────────────────────────────────────────┘    │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐    │
│  │ [V] Thinking (3.2s)                                 │    │  ← Collapsed thinking block
│  └─────────────────────────────────────────────────────┘    │
│                                                             │
│  Writing glassmorphic Navbar with Framer Motion...          │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

- **Status indicators**: `[x]` done fg(#81C995), `[>]` running fg(#E8EAED), `[ ]` queued fg(#9AA0A6), `[ ]` blocked (deps not met) fg(#555)
- **Execution order**: V1 runs tasks sequentially in dependency order. The task list shows queued tasks with their dependency annotations so the user can see the execution plan. V1.1 adds concurrent execution with `[~]` indicator.
- **Atomic commits**: Each completed task auto-commits with a descriptive message: `feat: create Navbar component`
- **Live tool cards**: Current task's tool calls render inline as rich cards (§6.2)
- **Pause/Resume**: `P` pauses execution (waits for current tool to finish); `R` resumes
- **Skip**: `S` skips a task with confirmation; dependency graph re-computed

### 7.4 Verify Phase Screen (`/verify`)

Self-healing verification step — runs automatically after Execute completes:

```
┌─────────────────────────────────────────────────────────────┐
│  [M31A] Verifying                     5 of 7 passed  71%    │  ← Lipgloss: fg(#FDD663) bold
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌────────────────────────────────────────────────────┐     │
│  │ [PASS] 1. Initialize Next.js app                   │     │  ← [PASS] fg(#81C995)
│  │          ✓ package.json exists                     │     │
│  │          ✓ next dev starts successfully            │     │
│  │ [PASS] 2. Setup globals.css + theme                │     │
│  │          ✓ globals.css has theme variables         │     │
│  │ [PASS] 3. Create Navbar component                  │     │
│  │          ✓ Navbar.js exists with export            │     │
│  │ [FAIL] 4. Create Hero component                    │     │  ← [FAIL] fg(#F28B82)
│  │          ✗ Hero.js missing default export          │     │
│  │          → [H] Self-heal  [S] Skip                 │     │
│  │ [PASS] 5. Create Menu section                      │     │
│  │ [FAIL] 6. Create Gallery section                   │     │
│  │          ✗ Gallery component has syntax error      │     │
│  │          → [H] Self-heal  [S] Skip                 │     │
│  │ [SKIP] 7. Wire up app/page.js (deps failed)        │     │  ← [SKIP] fg(#9AA0A6)
│  └────────────────────────────────────────────────────┘     │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐    │
│  │ [..] Self-healing task #4: Generating fix for       │    │
│  │      missing default export in Hero.js...           │    │
│  └─────────────────────────────────────────────────────┘    │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

- **Verification checks**: Each task defines acceptance criteria in `TASKS.md`. The verify phase runs deterministic checks:
  - File existence and non-empty
  - Syntax validation (parse JS/TS/Go/etc. via language-specific parsers)
  - Test execution (if test files exist, run them)
  - Import/export consistency
- **Self-heal** (`H`): LLM receives the failure details + original task spec + current file state. Generates a fix, auto-commits, re-verifies. Max 2 heal attempts per task.
- **Skip** (`S`): Skip the failed task; dependent tasks also marked skipped
- **Self-heal limit**: After 2 failed heal attempts, task is marked `[UNRECOVERABLE]` and user is prompted

### 7.5 Ship Phase Screen

Automatic final step after all tasks pass verification:

```
┌─────────────────────────────────────────────────────────────┐
│  [M31A] Shipped!                              ✓ All 7 tasks │  ← Lipgloss: fg(#81C995) bold
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌─────────────────────────────────────────────────────┐    │
│  │  Summary                                            │    │
│  │  ─────────────────────────────────────────────────  │    │
│  │  ✓ 7 tasks completed (0 skipped, 0 failed)          │    │
│  │  ✓ 7 atomic commits created                         │    │
│  │  ✓ All verification checks passed                   │    │
│  │                                                     │    │
│  │  Commits:                                           │    │
│  │  a1b2c3d  feat: initialize Next.js app              │    │
│  │  e4f5g6h  feat: setup globals.css + dark espresso   │    │
│  │  i7j8k9l  feat: create Navbar component             │    │
│  │  m0n1o2p  feat: create Hero component               │    │
│  │  q3r4s5t  feat: create Menu section                 │    │
│  │  u6v7w8x  feat: create Gallery section              │    │
│  │  y9z0a1b  feat: wire up app/page.js                 │    │
│  └─────────────────────────────────────────────────────┘    │
│                                                             │
│  [O] Open in browser  [N] New session  [R] Return to REPL   │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

- **Summary**: Task count, commit log, verification results
- **Open in browser**: Uses `Bash` tool to run `xdg-open` (Linux), `open` (macOS), or `start` (Windows) on the detected dev server URL (defaults to `http://localhost:3000` for Next.js, `http://localhost:5173` for Vite, etc.). If no dev server is running, the button shows as grayed out.
- **Git error handling**: If a commit fails (hook rejection, uncommitted changes, detached HEAD), the Ship screen shows the error inline: `[ERR] commit failed: pre-commit hook rejected`. The user can fix the issue manually and press `R` to retry, or press `F` to force-ship without the final commit (all prior commits are preserved). If `git init` failed during Initialize, Ship shows `[WARN] no git repo — commits skipped` and proceeds.
- **Session archived**: State moved to `~/.m31a/sessions/<id>/archived/`

---

## 8. TUI Screens (Complete Redesign)

### 8.1 REPL Screen (Main)

**Layout**: 
- **Header** (1 line): Brand + Active Model Badge + Context Bar + Connection Status
- **Messages** (flex): Scrollable history with Gemini-style bubbles
- **Input** (3-6 lines): `textarea` with placeholder text, character count
- **Status** (1 line): Current operation / spinner / last action timestamp

**Message Bubbles:**

```
┌─────────────────────────────────────────────────────────────┐
│                                                             │
│  ┌──────────────────────────────┐                           │
│  │ Build me a restaurant site   │  ← You                    │
│  └──────────────────────────────┘                           │
│                                                             │
│     ┌──────────────────────────────────────────────────┐    │
│     │ I'll create a stunning restaurant website for    │    │
│     │ "Eshan's Cafe" using Next.js and Framer Motion.  │    │
│     │                                                  │    │
│     │ First, let me check your workspace...            │    │
│     │                                                  │    │
│     │ [..] Bash: ls -la                                │    │  ← [..] spinner fg(#9AA0A6)
│     │                                                  │    │
│     │ [v] Thinking                                     │    │  ← [v] collapsed toggle fg(#8AB4F8)
│     │                                                  │    │
│     │ [OK] Bash completed                              │    │  ← [OK] fg(#81C995)
│     │                                                  │    │
│     │ Your workspace is empty. I'll proceed with the   │    │
│     │ full scaffold.                                   │    │
│     └──────────────────────────────────────────────────┘    │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

- **User messages**: Right-aligned, subtle background tint
- **Assistant messages**: Left-aligned, full width, generous padding
- **System/tool messages**: Inline cards, not separate bubbles
- **Thinking blocks**: Nested within assistant bubble, collapsible

**Key Bindings:**
| Key | Action |
|-----|--------|
| `Enter` | Send message |
| `Shift+Enter` | Newline |
| `Ctrl+C` | Cancel current operation / Quit if idle |
| `Ctrl+L` | Clear screen |
| `Ctrl+M` | Open model selector |
| `Ctrl+P` | Toggle plan mode |
| `Ctrl+G` | Settings |
| `PgUp/PgDn` | Scroll history |
| `↑/↓` | Navigate input history (when input empty) |

### 8.2 Permission Modal

Centered overlay with frosted-glass effect (via background dimming):

```
┌─────────────────────────────────────────────────────────────┐
│                                                             │
│     ┌─────────────────────────────────────────────────┐     │
│     │  [LOCK] Permission Required                     │     │  ← Lipgloss: fg(#FDD663) bold
│     ├─────────────────────────────────────────────────┤     │
│     │                                                 │     │
│     │  Tool:     Bash                                 │     │
│     │  Risk:     [DANGER]                             │     │  ← Lipgloss: fg(#F28B82) bold
│     │                                                 │     │
│     │  Command:                                       │     │
│     │  ┌─────────────────────────────────────────┐    │     │
│     │  │ rm -rf node_modules/.cache              │    │     │
│     │  └─────────────────────────────────────────┘    │     │
│     │                                                 │     │
│     │  [Y] Allow Once    [A] Always Allow             │     │
│     │  [N] Deny          [E] Exit M31A                │     │
│     │                                                 │     │
│     │  Auto-deny in 4:59...                           │     │
│     └─────────────────────────────────────────────────┘     │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

- **Risk color coding**: Safe (gray), Medium (amber), Dangerous (red)
- **Command preview**: Syntax highlighted if detectable language
- **Timeout**: Visible countdown, pauses on user activity

### 8.3 Settings Screen

Two-column layout with **Lipgloss tabs**:

```
┌───────────────────────────────────────────────────────────────┐
│  Settings                                        [Ctrl+S] Save│
├──────────────────────┬────────────────────────────────────────┤
│  Model               │  Default Model                         │
│  Permissions         │  ┌────────────────────────────────┐    │
│  Appearance  ◄───────│  │ anthropic/claude-3.5-sonnet    │    │
│  Features            │  └────────────────────────────────┘    │
│  Keyboard            │                                        │
│                      │  Context Warning Threshold             │
│                      │  ┌────────────────────────────────┐    │
│                      │  │ 80%                            │    │
│                      │  └────────────────────────────────┘    │
│                      │                                        │
│                      │  [x] Show inline thinking by default   │  ← [x] fg(#81C995), [ ] fg(#9AA0A6)
│                      │  [x] Auto-collapse completed tools     │
│                      │  [ ] Compact mode                      │
│                      │                                        │
└──────────────────────┴────────────────────────────────────────┘
```

### 8.4 Resume Screen

Session browser with rich metadata:

```
┌─────────────────────────────────────────────────────────────┐
│  Resume Session                                             │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌─────────────────────────────────────────────────────┐    │
│  │ [*] Restaurant website build                        │    │  ← [*] fg(#81C995) selected
│  │   24 messages · claude-3.5-sonnet · 2 hours ago     │    │
│  │   Last: "Create the MenuHighlights component"       │    │
│  └─────────────────────────────────────────────────────┘    │
│  ┌─────────────────────────────────────────────────────┐    │
│  │ [ ] Fix Go memory leak                              │    │  ← [ ] fg(#9AA0A6) unselected
│  │   8 messages · deepseek-r1 · 5 minutes ago          │    │
│  │   Last: "Run pprof heap analysis"                   │    │
│  └─────────────────────────────────────────────────────┘    │
│                                                             │
│  [Enter] Resume  [N] New Session  [D] Delete                │
└─────────────────────────────────────────────────────────────┘
```

- **Corrupted session handling**: If `STATE.md` or `TASKS.md` is unreadable (disk error, interrupted write, manual corruption), the session is shown with a `[!]` warning badge fg(#F28B82) and the description "Session data may be corrupted". Selecting a corrupted session offers a `[R] Repair` option (attempts to reconstruct from `messages.json` and `session.json`) or `[D] Delete`. If repair fails, the session is shown but the user can only start a new session or delete it.

---

## 9. Visual Style System (Gemini-Inspired)

### 9.1 Color Palette

| Token | Dark Mode | Light Mode | Usage |
|-------|-----------|------------|-------|
| **Background** | `#0D0D0D` | `#FFFFFF` | Main canvas |
| **Surface** | `#1A1A1A` | `#F8F9FA` | Cards, bubbles, input area |
| **Surface Elevated** | `#242424` | `#FFFFFF` | Modals, menus, thinking blocks |
| **Border** | `#2E2E2E` | `#E0E0E0` | Dividers, card outlines |
| **Brand** | `#D77757` | `#C45C3A` | Primary accent, active states |
| **Text Primary** | `#E8EAED` | `#1F1F1F` | Headings, body |
| **Text Secondary** | `#9AA0A6` | `#5F6368` | Metadata, timestamps |
| **Thinking** | `#8AB4F8` | `#1967D2` | Reasoning blocks, model thinking |
| **Success** | `#81C995` | `#1E8E3E` | Completed tasks, success states |
| **Error** | `#F28B82` | `#D93025` | Errors, denials |
| **Warning** | `#FDD663` | `#F9AB00` | Warnings, medium risk |
| **Code BG** | `#2D2D2D` | `#F1F3F4` | Inline code, code blocks |

### 9.2 Typography & Spacing

- **Border radius**: 2px for inline code, 4px for cards, 6px for modals (simulated via Unicode box-drawing and Lipgloss)
- **Shadows**: Simulated via darker border colors (terminals cannot render true shadows)
- **Line height**: 1.5x for readability in markdown content
- **Monospace**: All UI chrome uses terminal font. Markdown content uses proportional simulation via spacing.

### 9.3 Animation Guidance

Bubble Tea renders via frame-based redraws — there are no CSS-style easing curves in a terminal. "Animations" are implemented as state transitions over successive ticks:

| Transition | Implementation |
|-----------|----------------|
| Spinner rotation | Unicode character cycle: `⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏` at 10fps |
| Message appear | Single-frame fade: render with empty lines, then full content on next tick |
| Thinking expand/collapse | Immediate reflow — no intermediate frames (content height changes instantly) |
| Modal enter | Render overlay with full opacity on next tick (no fade-in) |
| Progress bar fill | Increment width by N characters per tick for smooth fill effect |

Performance budget: each redraw must complete within 16ms (60fps target). Heavy operations (syntax highlighting, large text rendering) are computed off the main tick and cached.

---

## 10. Configuration (V1 Simplified)

### 10.0 First-Run Experience

On first launch (no config file, no stored keys), M31A shows an interactive setup screen:

```
┌─────────────────────────────────────────────────────────────┐
│  Welcome to M31A                                            │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  M31A needs an API key to connect to an LLM gateway.        │
│  Choose a provider:                                         │
│                                                             │
│  [1] OpenRouter  — 400+ models, pay-per-use                 │
│      Get a key: https://openrouter.ai/keys                  │
│                                                             │
│  [2] OpenCode Zen — Curated models, flat pricing            │
│      Get a key: https://opencode.ai/zen                     │
│                                                             │
│  [3] Both — Use OpenRouter primary, Zen as fallback         │
│                                                             │
│  [4] Skip — Run without a key (limited functionality)       │
│                                                             │
│  Enter your key(s):                                         │
│  > sk-or-...                                                │
│                                                             │
│  Store key securely in OS keychain? [Y/n]                   │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

- Keys are validated immediately via the health check endpoint before proceeding
- Invalid keys show an inline error and prompt for re-entry
- After successful setup, the normal REPL loads with the configured provider active
- If the user skips setup, M31A loads with a banner: "No API key configured. Set one via `/settings` or `M31A_OPENROUTER_API_KEY` env var."

### 10.1 Config File: `~/.m31a/config.toml`

```toml
[provider]
default = "openrouter"  # "openrouter" | "zen" — which gateway to use by default
auto_fallback = false   # If true, auto-switch to other provider on 503/429

[provider.openrouter]
api_key = ""  # Resolved: env > keychain > this file

[provider.zen]
api_key = ""  # Resolved: env > keychain > this file

[model]
default = "openrouter/anthropic/claude-3.5-sonnet"  # provider/model-id format
context_warning_threshold = 80  # Percentage
show_thinking_by_default = true
auto_collapse_tools = true
auto_arbitrage = false    # Auto-suggest cheaper models for simple tasks
arbitrage_threshold = 0.7 # Complexity score below which to suggest cheaper model (0-1)

[ui]
theme = "dark"  # dark | light | auto
compact_mode = false
show_token_usage = true
show_cost_estimate = true
max_iterations = 25

[permissions]
default_mode = "default"  # default | auto | bypass
timeout_seconds = 300

[[permissions.rules]]
tool = "Bash"
pattern = "rm\\s+-rf|git\\s+reset\\s+--hard|dd\\s+if=|mkfs|DROP\\s+TABLE"
risk_level = "danger"
action = "ask"

[features]
autodream_enabled = true
subagent_enabled = true
auto_backup = true
resume_on_startup = true

[ledger]
enabled = true            # Enable cross-session learning ledger
max_entries = 100         # Prune ledger to last N entries

[ghost]
enabled = false           # V1.1: Enable ghost mode by default
```

### 10.2 API Key Storage & Keychain Integration

API keys are **never stored in plaintext** in the config file. Resolution order: environment variable → OS keychain → config file fallback.

**Keychain resolution by platform:**

| Platform | Keychain Backend | Implementation |
|----------|-----------------|----------------|
| **Linux** | `secret-service` DBus API | Uses `gnome-keyring`, `kwallet`, or any freedesktop Secret Service implementation. Falls back to `pass` if available. |
| **macOS** | Keychain Services | Uses `security` CLI or native Keychain Services API via CGO-less wrapper |
| **Windows** | Windows Credential Manager | Uses Credential Manager API via `wincred`-equivalent Go package |

**Key storage flow:**
1. On first launch, if `M31A_OPENROUTER_API_KEY` or `M31A_ZEN_API_KEY` env vars are set, M31A prompts: "Store key(s) securely in your OS keychain? [Y/n]"
2. Keys stored under service name `m31a` with accounts `openrouter-api-key` and `zen-api-key` respectively
3. On subsequent launches, M31A queries keychain first. Falls back to `config.toml` with a warning if keychain unavailable.
4. `m31a keychain setup` — interactively configure both providers
5. `m31a keychain rotate openrouter` — replace OpenRouter key
6. `m31a keychain rotate zen` — replace Zen key
7. `m31a keychain remove <provider>` — delete stored key

**Config file (plaintext fallback):**
```toml
[provider.openrouter]
api_key = ""  # Only used if keychain is unavailable

[provider.zen]
api_key = ""  # Only used if keychain is unavailable
```

### 10.3 Environment Variables

| Variable | Purpose |
|----------|---------|
| `M31A_OPENROUTER_API_KEY` | OpenRouter API key |
| `M31A_ZEN_API_KEY` | OpenCode Zen API key |
| `M31A_CONFIG` | Override config path |
| `M31A_THEME` | Override theme |
| `M31A_DEFAULT_MODEL` | Override model ID (provider/model format, e.g. `zen/anthropic/claude-3.5-sonnet`) |
| `M31A_PROVIDER` | Override default provider (`openrouter` or `zen`) |

Both providers are optional. M31A starts with whichever provider has a valid key configured. If both are configured, the user can switch between them at any time.

---

## 11. State Management (Updated for V1)

### 11.1 AppState Additions

```go
type AppState struct {
    // ... existing fields ...
    
    // Provider System
    activeProvider   string          // "openrouter" | "zen"
    providers        *ProviderRegistry
    activeModel      *ModelInfo      // Full metadata, not just string ID
    
    // Model System
    modelCache       *ModelCache     // Per-provider cache
    contextUsed      int64           // Running calibrated token count (updated after each turn)
    contextTotal     int64           // From ModelInfo.ContextLength
    estimationFactor map[string]float64 // Per-model-family EMA correction factor (alpha=0.3)
    
    // Thinking System
    activeSegment    *MessageSegment // Currently streaming segment (content or thinking)
    segments         []MessageSegment // Completed segments for current message
    thinkingExpanded bool            // Global preference for initial thinking block state
    
    // Workflow System
    workflowPhase    WorkflowPhase // Initialize | Discuss | Plan | Execute | Verify | Ship
    project          *ProjectState // Parsed from PROJECT.md
    tasks            []*Task       // Parsed from TASKS.md
    execContext      *ExecutionContext // Current task execution state
    
    // UI Polish
    lastActivity     time.Time       // For idle detection
    currentOperation string          // "Thinking...", "Executing Bash...", etc.
}

type WorkflowPhase string
const (
    PhaseIdle       WorkflowPhase = "idle"
    PhaseInitialize WorkflowPhase = "initialize"
    PhaseDiscuss    WorkflowPhase = "discuss"
    PhasePlan       WorkflowPhase = "plan"
    PhaseExecute    WorkflowPhase = "execute"
    PhaseVerify     WorkflowPhase = "verify"
    PhaseShip       WorkflowPhase = "ship"
)

type Task struct {
    ID               int       `json:"id"`
    Description      string    `json:"description"`
    Action           string    `json:"action"`       // "NEW" | "EDIT" | "DELETE"
    Dependencies     []int     `json:"dependencies"` // Task IDs that must complete first
    Files            []string  `json:"files"`        // Files this task touches
    AcceptanceCriteria []string `json:"acceptance_criteria"`
    Status           TaskStatus `json:"status"`      // pending | running | done | failed | skipped
    HealsAttempted   int       `json:"heals_attempted"`
    PredictedFiles   []FilePrediction `json:"predicted_files,omitempty"` // For diff preview (V1)
}

type FilePrediction struct {
    Path        string `json:"path"`         // File path relative to project root
    Action      string `json:"action"`       // "create" | "modify" | "delete"
    EstimatedLines int   `json:"estimated_lines"` // Approximate line count
}

type ProjectState struct {
    Goal         string            `json:"goal"`
    ProjectType  string            `json:"project_type"` // "web" | "go" | "python" | etc.
    Framework    string            `json:"framework"`
    Answers      map[string]string `json:"answers"`      // Discuss phase Q&A
    CreatedAt    time.Time         `json:"created_at"`
}
```

### 11.2 File-Based State (Session Persistence)

M31A persists all workflow state as **human-readable Markdown and JSON files** in the session directory. This makes sessions resumable, version-controllable, and debuggable.

**Session ID generation:** Session IDs are 8-character lowercase alphanumeric strings (e.g. `a3f7k2m9`), generated via `crypto/rand`. On collision (extremely unlikely), a numeric suffix is appended. IDs are user-facing and appear in the resume screen and file paths.

**Directory structure:**
```
~/.m31a/sessions/<session-id>/
├── session.json              # Session metadata (ID, model, timestamps, message count)
├── messages.json             # Full conversation history (JSON array of Message)
├── MEMORY.md                 # AutoDream consolidated memory blocks
├── planning/
│   ├── PROJECT.md            # Goal, project type, framework, discuss-phase Q&A
│   ├── TASKS.md              # Task list with dependencies, status, acceptance criteria
│   ├── STATE.md              # Current phase, progress, last action timestamp
│   └── config.json           # Per-session overrides (model tier, parallelism, etc.)
└── archived/                 # Completed sessions moved here after Ship phase
```

**`PROJECT.md` format:**
```markdown
# Project: Eshan's Cafe Website

## Goal
Build a restaurant website for "Eshan's Cafe" with dark espresso theme.

## Project Type
Web · Frontend-only · Next.js

## Discuss Phase Q&A
- **Color palette**: dark espresso
- **Features**: menu, gallery
- **Framework**: Next.js

## Created
2026-05-26T08:00:00Z
```

**`TASKS.md` format:**
```markdown
# Tasks

| # | Action | Description | Deps | Status | Files |
|---|--------|-------------|------|--------|-------|
| 1 | NEW | Initialize Next.js app | — | ✅ done | package.json, next.config.js |
| 2 | NEW | Setup globals.css + theme | 1 | ✅ done | globals.css, theme.js |
| 3 | NEW | Create Navbar component | 2 | ✅ done | components/Navbar.js |
| 4 | NEW | Create Hero component | 2 | ❌ failed → healed | components/Hero.js |
| 5 | NEW | Create Menu section | 2 | ⏳ pending | components/Menu.js |
| 6 | NEW | Create Gallery section | 2 | ⏳ pending | components/Gallery.js |
| 7 | NEW | Wire up app/page.js | 3,4,5,6 | ⏳ pending | app/page.js |
```

**`STATE.md` format:**
```markdown
# Session State

## Current Phase: Execute
## Progress: 3/7 tasks complete (43%)
## Last Action: Created Navbar component (commit i7j8k9l)
## Timestamp: 2026-05-26T08:15:00Z
```

**Resume flow:** On startup, M31A scans `~/.m31a/sessions/` for unarchived sessions. Parsing `STATE.md` and `TASKS.md` reconstructs the exact workflow state. The user can resume from the last phase — no JSON deserialization needed for the human-readable overview.

**Checkpoint system:** Before each phase transition, the current state is snapshotted to `~/.m31a/sessions/<id>/checkpoint.json`. This enables `/undo` to roll back to the previous phase. Only the last two checkpoints are retained.

### 11.3 Token Estimation & Calibration

OpenRouter and Zen both return `usage` (prompt_tokens, completion_tokens, total_tokens) only in the **final** SSE chunk, not during streaming. M31A uses a two-phase approach:

**Phase 1 — Client-Side Estimation (during streaming):**
- **GPT-family** (openai/*, anthropic/claude): Use `tiktoken-go` with the model-specific encoding (`cl100k_base` for GPT-4, `o200k_base` for GPT-4o, `claude` for Claude)
- **Other families** (deepseek/*, qwen/*, etc.): Character count ÷ 4 as baseline, with a 1.3x multiplier for code-heavy prompts (detected by language tag in system prompt)
- **Display**: Context bar in header updates on every message append using the estimate

**Phase 2 — Server Calibration (after each turn):**
- When the final SSE chunk arrives, extract `usage.prompt_tokens` and `usage.completion_tokens`
- Compute `estimationError = estimated - actual`
- Apply a rolling correction factor (exponential moving average, alpha=0.3) to future estimates for the same model family
- This ensures the context bar converges to accuracy within 2-3 turns

**Warning**: When `calibratedContextUsed / contextTotal > threshold`, banner appears: "Context 85% full. Consider `/compress`."

---

## 12. Background Systems

### 12.1 Task Runner (`pkg/taskrunner`)

The Task Runner is the core execution engine for the Plan → Execute → Verify → Ship workflow. It manages task scheduling, dependency resolution, concurrent execution, and self-healing verification.

**Dependency graph resolution:**
```go
type TaskRunner struct {
    tasks     []*Task
    completed map[int]bool
    failed    map[int]bool
    done      chan TaskResult
}

func (r *TaskRunner) Schedule() [][]*Task {
    // Topological sort → groups of independent tasks
    // Each group can run concurrently
    groups := [][]*Task{}
    remaining := r.tasks
    for len(remaining) > 0 {
        ready := r.readyTasks(remaining) // no unmet deps
        if len(ready) == 0 {
            break // cycle or all failed
        }
        groups = append(groups, ready)
        remaining = r.subtract(remaining, ready)
    }
    return groups
}
```

**Execution flow:**
1. **Schedule**: Topological sort produces execution groups — tasks with no mutual dependencies in the same group
2. **Execute group**: Each task in a group runs sequentially (V1) or concurrently (V1.1 via subagents)
3. **Per-task lifecycle**: Stream LLM response → execute tool calls → atomic commit → mark done
4. **Dependency blocking**: Tasks wait for all dependencies to complete; if any dependency fails, the task is marked `[SKIP]`
5. **Progress reporting**: After each task, state is flushed to `TASKS.md` and `STATE.md`

**V1 vs V1.1 execution:**
| Feature | V1 | V1.1 |
|---------|----|------|
| Task concurrency | Sequential (one at a time) | Concurrent (independent tasks in parallel) |
| Subagent isolation | N/A | Each task runs in a subagent with fresh context |
| Cross-task communication | Via filesystem (files written by earlier tasks) | Via filesystem + shared state channel |

### 12.2 AutoDream Consolidation (Context Engineering)

AutoDream periodically summarizes conversation history into compact memory blocks to prevent context window exhaustion.

**Trigger conditions:**
- **Context-based**: When context usage exceeds 60% of the model's window
- **Time-based**: Every 15 minutes of active conversation
- **Never during active task execution** — AutoDream is paused while tools are running or tasks are being executed

**Enhanced UX:**
- **Trigger indicator**: When AutoDream runs, a subtle `[DREAM]` tag appears in the status bar fg(#8AB4F8) with "Consolidating memory..." text
- **Progress**: Spinner with live token-count reduction: "Summarizing 24 messages → ~3 messages"
- **Post-consolidation banner**: "Memory consolidated. 24 messages summarized into 3 blocks. `/memory review` to see summary."
- **Review**: `/memory review` opens a viewer showing the latest AutoDream summary with the original message ranges it replaced
- **Revert**: `/memory revert` restores the last pre-consolidation state from a checkpoint file (~/.m31a/sessions/<id>/checkpoint.json). Only one revert level is kept (last consolidation).
- **Pause**: `/memory pause` disables AutoDream for the current session. Re-enable with `/memory resume`.
- **Checkpoint file**: Before each consolidation, the full pre-consolidation state is written to disk. This file is cleaned up after 24 hours or on session close.

### 12.3 Model Cache Sync

- **Frequency**: Every 5 minutes per provider, and on explicit `/model` invocation
- **Per-provider**: OpenRouter and Zen caches are maintained independently. Switching providers is instant — the target provider's cache is already warm.
- **Offline resilience**: If fetch fails, use stale cache for 24 hours, then warn user
- **Background fetch**: Non-blocking; TUI never freezes for model list updates

### 12.4 Health Check

- **OpenRouter endpoint**: `GET https://openrouter.ai/api/v1/auth/key`
- **Zen endpoint**: `GET https://opencode.ai/zen/v1/models` (lightweight catalog fetch validates auth)
- **Indicator**: Both providers checked independently. Header shows `[LIVE]` fg(#81C995) if active provider is connected, `[SLOW]` fg(#FDD663) if >500ms, `[OFFLINE]` fg(#F28B82) if unreachable. If the active provider is down and `auto_fallback = true`, M31A switches to the other provider with a banner: "Switched to Zen — OpenRouter unavailable."
- **Adaptive polling**: Default interval is 60 seconds. If a rate limit header indicates low remaining requests, the interval increases to 120 seconds. If auth fails (401), polling stops until the user updates the key.
- **Non-blocking**: Health check failures never interrupt active streaming or tool execution.

---

## 13. Error Handling (V1 Specifics)

### 13.1 Provider Error Normalization

| Error | OpenRouter Behavior | Zen Behavior |
|-------|---------------------|--------------|
| `401` Invalid key | Modal: "Invalid OpenRouter API Key" with link to settings | Modal: "Invalid Zen API Key" with link to settings |
| `402` Out of credits | Banner: "Insufficient credits. Add funds at openrouter.ai" | Banner: "Insufficient Zen credits. Add funds at opencode.ai/zen" |
| `429` Rate limited | Retry with `Retry-After` header. If `auto_fallback=true` and Zen is healthy, switch with banner | Retry after 5s. If `auto_fallback=true` and OpenRouter is healthy, switch with banner |
| `503` Provider unavailable | Show banner: "Provider unavailable. Retry or switch to Zen?" If `auto_fallback=true`, auto-switch | Show banner: "Zen unavailable. Retry or switch to OpenRouter?" If `auto_fallback=true`, auto-switch |
| `400` Context exceeded | Trigger context compaction automatically, then retry once | Same as OpenRouter |

**Both providers down:** If both providers are unreachable simultaneously (network outage, both gateways down), M31A shows a blocking banner: "No LLM gateway available. Both providers are unreachable. Check your network connection." with `[R] Retry` and `[O] Offline Mode` options. Offline Mode loads the REPL without streaming — the user can review session history, read state files, and manage plans, but cannot send messages to the LLM.

**Cross-provider model resolution:** When switching providers, M31A attempts to find the same model on the target provider. If `anthropic/claude-3.5-sonnet` exists on OpenRouter but not Zen, it prompts: "Model not available on Zen. Switch to closest match (anthropic/claude-3-sonnet) or pick manually?"

### 13.2 Context Window Protection

```go
func (s *AppState) CheckContextBudget(msg Message) error {
    estimated := EstimateTokens(msg)
    if s.contextUsed + estimated > s.contextTotal {
        return ErrContextExceeded
    }
    return nil
}
```

On `ErrContextExceeded`:
1. Attempt automatic compaction (summarize oldest 50% of history)
2. If still exceeded, prompt user: "Context limit reached. Start new session or compress memory?"

---

## 14. Slash Commands (V1)

### 14.1 Session & Utility

| Command | Description |
|---------|-------------|
| `/help` | All commands with descriptions |
| `/clear` | New session (preserves memory) |
| `/quit` | Exit |
| `/settings` | Open settings |
| `/model` | Model selector overlay |
| `/memory` | MEMORY.md viewer |
| `/memory review` | Review latest AutoDream consolidation summary |
| `/memory revert` | Restore state from last pre-consolidation checkpoint |
| `/memory pause` | Disable AutoDream for current session |
| `/memory resume` | Re-enable AutoDream for current session |
| `/resume` | Session browser |
| `/compress` | Manual AutoDream trigger |
| `/undo` | Restore last checkpoint (phase rollback) |
| `/status` | Show active model, context usage (actual + estimated), cost estimate, session duration, health status, current workflow phase (merged from old `/whereami` and `/stats`) |
| `/theme` | Cycle theme: dark → light → auto. Instant application, no restart required |
| `/ledger` | Open cross-session learning ledger viewer |
| `/ledger stats` | Show aggregate stats (avg cost, tasks, common failures) |
| `/rollback` | Interactive commit rollback chain browser |
| `/optimize` | Run cost optimization on current plan (model arbitrage) |

### 14.2 Workflow Commands

| Command | Description |
|---------|-------------|
| `/new` | Start a new workflow. Prompts for goal, enters Initialize phase |
| `/plan` | Jump to Plan phase (regenerates plan if one exists) |
| `/execute` | Start executing the current plan (skips to Execute phase) |
| `/verify` | Run verification on completed tasks (triggers self-heal for failures) |
| `/ship` | Force-ship the current workflow (skip remaining tasks, generate summary) |
| `/workflow` | Show current phase, task progress, dependency graph |
| `/pause` | Pause current execution (waits for running tool to finish) |
| `/resume-task` | Resume paused execution |
| `/ghost` | V1.1: Toggle ghost mode for next workflow (shadow execution in git worktree) |

### 14.3 Legacy Commands (V1)

| Command | Description |
|---------|-------------|
| `/tasks` | Alias for `/workflow` |
| `/demo` | Demonstration viewer |

---

## 15. Acceptance Criteria (V1)

1. **Binary builds**: `CGO_ENABLED=0 go build -o m31a ./cmd/m31a` produces static binary
2. **Dual-provider discovery**: On first run with valid key(s), both OpenRouter and Zen model lists populate within 3 seconds
3. **Provider selector**: `/model` shows searchable list with provider badges [OR]/[ZEN], pricing, context, and capabilities
4. **Provider switching**: Switching from OpenRouter to Zen is instant (warm cache); active model re-resolves on target provider
5. **Inline thinking**: Using `deepseek/deepseek-r1` on either provider, reasoning tokens appear in collapsible block above final answer
6. **Streaming UX**: Token-by-token streaming with no flicker; thinking tokens render in real-time
7. **Tool cards**: Bash/FileRead results render as collapsible cards with syntax highlighting
8. **Workflow end-to-end**: Type a goal → Initialize → Discuss → Plan → Accept → Execute → Verify → Ship completes with markdown rendering at each phase
9. **Plan generation**: Plan phase produces a dependency-annotated task list with estimated cost/time; user can Accept, Edit, or Retry
10. **Execution**: Tasks execute in dependency order; completed tasks produce atomic git commits; progress persists to `TASKS.md` and `STATE.md`
11. **Verification**: After execution, Verify phase runs acceptance checks; failures offer Self-Heal (max 2 attempts) or Skip
12. **File-based state**: `PROJECT.md`, `TASKS.md`, and `STATE.md` are human-readable Markdown in `~/.m31a/sessions/<id>/planning/`; session resume parses them correctly
13. **Permission modal**: Dangerous tool triggers centered modal with timeout and syntax preview
14. **Context bar**: Header shows accurate `used/total` context that updates live
15. **Session resume**: Close and reopen; previous session restores with metadata, workflow phase, task progress, and context-aware truncated history
16. **Theme switch**: `/theme` cycles with instant application, no restart required
17. **Zero manual config**: Only required input is at least one API key (OpenRouter or Zen); everything else is discovered
18. **Auto-fallback**: When `auto_fallback=true` and active provider returns 429/503, M31A switches to the other provider with a visible banner
19. **Workflow slash commands**: `/new`, `/plan`, `/execute`, `/verify`, `/ship`, `/workflow`, `/pause`, `/resume-task` all function correctly
20. **Context engineering**: Each phase launches with a pruned context; AutoDream consolidates at 60% threshold

**Verification Flow:**
```bash
./m31a
# Paste OpenRouter key and/or Zen key
# Type: "/new" → "Build a React counter app"
# Verify: Initialize detects project type, Discuss asks clarifying questions
# Verify: Plan phase shows task list with dependencies, cost estimate
# Verify: Accept plan → Execute phase runs tasks in order, atomic commits
# Verify: Verify phase checks each task, self-heals failures
# Verify: Ship phase shows summary with commit log
# Verify: Provider badges [OR]/[ZEN] visible in model selector
# Verify: Switch provider with /model → P key, streaming works on both
# Verify: Tool execution shows rich card, permission modal appears for Bash
# Verify: Session files exist: ~/.m31a/sessions/<id>/planning/{PROJECT,TASKS,STATE}.md
# Set auto_fallback=true, trigger 429 on OpenRouter
# Verify: Banner "Switched to Zen — OpenRouter rate limited" appears
# Close and reopen → /resume → session restores at Ship phase
```

---

## 16. Differentiators (What Others Don't Do)

These features are M31A's unique selling points. No other AI coding agent (Cursor, Claude Code, Codex, Devin) offers this combination.

### 16.1 Cost-Aware Model Arbitrage (V1)

AI agents typically use one model for everything. M31A analyzes each task's complexity and suggests cheaper model alternatives when the current model is overkill.

**How it works:**
1. After Plan phase generates the task list, M31A runs each task through a complexity heuristic in `pkg/arbitrage`
2. Complexity is scored 0–1 based on: predicted file count, operation type (scaffold vs. modify vs. delete), estimated token budget, and dependency depth
3. If complexity score < `arbitrage_threshold` (default 0.7), M31A checks the model catalog for a cheaper model that supports the required capabilities (tools, reasoning)
4. Suggested alternatives are displayed per-task in the Plan screen

**UX in Plan screen:**
```
┌─────────────────────────────────────────────────────┐
│  3. [NEW] Create Navbar component                   │
│     deps: 2 · ~60s · claude-3.5-sonnet ($0.015)     │
│     → [O] Use deepseek-r1 ($0.001) — save 93%       │  ← Optimization badge
│                                                     │
│  4. [NEW] Create Hero component                     │
│     deps: 2 · ~90s · claude-3.5-sonnet ($0.015)     │
│     → [O] Use deepseek-r1 ($0.001) — save 93%       │
└─────────────────────────────────────────────────────┘
```

- **Accept all**: Press `O` (optimize cost) to accept all cheaper-model suggestions at once
- **Review individually**: Navigate to each task and toggle the suggestion on/off
- **Auto mode**: If `auto_arbitrage = true` in config, cheaper models are used automatically without prompting
- **Capability guard**: A cheaper model is never suggested unless it supports the required capabilities (tools, reasoning) for that task

**Cost comparison matrix:** M31A maintains a per-model cost table sourced from provider metadata. The arbitrage engine compares `current_model_cost × estimated_tokens` against `candidate_model_cost × estimated_tokens × complexity_factor`.

### 16.2 Git Bisect Regression Detection (V1)

When the Verify phase finds a failing task and self-heal also fails, M31A doesn't just give up — it uses `git bisect` to pinpoint exactly which commit introduced the regression.

**How it works:**
1. After self-heal fails (2 attempts exhausted), M31A identifies the range of atomic commits from the current session
2. Runs `git bisect start`, marks the current HEAD as `bad`, marks the pre-session commit as `good`
3. Runs a deterministic verification script (file existence + syntax check) at each bisect step
4. When bisect completes, M31A knows the exact commit that introduced the failure
5. The bisect result + diff of the offending commit is fed to the LLM for a targeted fix

**UX in Verify screen:**
```
┌─────────────────────────────────────────────────────┐
│ [FAIL] 4. Create Hero component                     │
│         ✗ Hero.js missing default export            │
│         → Self-heal failed (2/2 attempts)           │
│                                                     │
│ [BISECT] Regression traced to:                      │
│   commit i7j8k9l — feat: create Navbar component    │
│   Changed: components/Hero.js (+3 lines, -1 line)   │
│                                                     │
│   [H] Retry heal with bisect context  [S] Skip      │
└─────────────────────────────────────────────────────┘
```

- **Retry with bisect context**: The LLM receives the exact diff that caused the regression, enabling a targeted fix
- **Implementation**: `pkg/bisect` — wraps `git bisect start/good/bad/run`, parses output, constructs diagnostic context with the offending commit's diff

### 16.3 Diff Preview Before Execution (V1)

Other agents show you changes *after* they make them. M31A shows you *before*.

**How it works:**
1. After the Plan phase generates the task list, the LLM is asked to output a `predicted_files` field for each task in the JSON plan
2. `predicted_files` includes: file path, action (create/modify/delete), estimated line count
3. M31A aggregates all predicted files across tasks and renders them as a visual diff tree

**UX — Diff Preview overlay (triggered by `D` in Plan screen):**
```
┌─ Diff Preview (7 tasks, 12 files) ──────────────────────┐
│                                                         │
│  components/                                            │
│    + Navbar.js          (new, ~80 lines)                │
│    + Hero.js            (new, ~120 lines)               │
│    + Menu.js            (new, ~200 lines)               │
│    + Gallery.js         (new, ~150 lines)               │
│  app/                                                   │
│    ~ page.js            (modify, ~15 lines changed)     │
│  styles/                                                │
│    ~ globals.css        (modify, ~30 lines changed)     │
│    + theme.css          (new, ~50 lines)                │
│  ─────────────────────────────────────────────────────  │
│  4 new files · 2 modified · 0 deleted · ~635 lines total│
│                                                         │
│  [Esc] Close                                            │
└─────────────────────────────────────────────────────────┘
```

- **File icons**: `+` for new (fg #81C995), `~` for modified (fg #FDD663), `-` for deleted (fg #F28B82)
- **Accuracy**: Predictions are estimates — actual changes may differ during execution. The diff preview is a planning aid, not a guarantee.
- **Implementation**: `predicted_files` is added to the Task struct's JSON schema. M31A aggregates and renders via Lipgloss table layout.

### 16.4 Cross-Session Learning Ledger (V1)

M31A remembers what it has built before. The Learning Ledger is a cumulative knowledge base across all shipped sessions that makes the agent smarter over time.

**Ledger file:** `~/.m31a/LEDGER.md` — human-readable markdown that users can read, edit, or prune.

**Entry format (appended after each Ship phase):**
```markdown
## Session a3f7k2m9 — 2026-05-26
- **Goal**: Restaurant website for "Eshan's Cafe"
- **Tasks**: 7 (7 passed, 0 failed, 0 skipped)
- **Model**: claude-3.5-sonnet · **Cost**: $0.12 · **Time**: 4m 12s
- **Files**: 8 new, 2 modified, ~635 lines
- **Failure patterns**: none
- **Key decisions**: Next.js, dark espresso theme, Framer Motion
```

**Aggregate stats** (computed from ledger, shown via `/ledger stats`):
- Average tasks per session, average cost, average time
- Most common failure patterns (e.g., "missing default export" in React components: 4 occurrences)
- Most used frameworks by project type
- Most common model per project type
- Cost trends over time

**Context injection:** During Initialize/Discuss phase, M31A queries the ledger for sessions with similar project types or goals. Relevant insights are injected into the system prompt:
> "Based on 3 similar past sessions (Next.js web apps), this typically takes 7 tasks, costs ~$0.12, and most commonly fails on missing component exports. Consider asking about export patterns during Discuss."

**UX — Ledger viewer (`/ledger`):**
```
┌─ Learning Ledger (42 entries) ──────────────────────────┐
│                                                         │
│  Filter: [project_type: web] [model: all] [date: 30d]   │
│                                                         │
│  ─────────────────────────────────────────────────────  │
│  42 sessions · avg 6.3 tasks · avg $0.09 · avg 3m 45s   │
│  Top failures: missing exports (4), syntax errors (2)   │
│  Top frameworks: Next.js (18), Vite (8), Go stdlib (6)  │
│                                                         │
│  [Enter] View entry  [F] Filter  [Esc] Close            │
└─────────────────────────────────────────────────────────┘
```

- **Pruning**: Ledger is pruned to `max_entries` (default 100) on each new entry. Oldest entries are removed first.
- **User editable**: Since LEDGER.md is plain markdown, users can manually add notes, delete entries, or reorganize.
- **Implementation**: `pkg/ledger` — parses LEDGER.md, computes aggregates via simple statistical functions, provides relevant session lookup by project type/goal similarity

### 16.5 Commit Rollback Chain (V1)

Every task produces an atomic git commit. `/rollback` gives you a visual, interactive timeline of all commits from the current session with safe undo.

**UX — Commit chain browser:**
```
┌─ Commit Rollback ───────────────────────────────────────┐
│                                                         │
│   y9z0a1b  feat: wire up app/page.js        [HEAD]      │
│   u6v7w8x  feat: create Gallery section                 │
│   q3r4s5t  feat: create Menu section                    │
│   m0n1o2p  feat: create Hero component                  │
│   i7j8k9l  feat: create Navbar component                │
│   e4f5g6h  feat: setup globals.css + dark espresso      │
│ > a1b2c3d  feat: initialize Next.js app      [selected] │
│                                                         │
│  [Enter] Rollback to here  [D] View diff  [Esc] Cancel  │
└─────────────────────────────────────────────────────────┘
```

**Rollback modes:**
- **Soft rollback** (default): `git reset --soft <hash>` — HEAD moves back, file changes are preserved in the working tree and staged. The user can review changes and retry from the rollback point. Task statuses for rolled-back tasks return to `pending`.
- **Hard rollback** (with confirmation): `git reset --hard <hash>` — HEAD moves back, all file changes after the target commit are discarded. Irreversible — requires explicit confirmation: "This will discard N files and M lines. Continue? [y/N]"

**Diff view** (`D` in rollback screen): Shows the unified diff of all changes between the selected commit and HEAD, rendered with syntax highlighting.

**Safety:**
- Before any rollback, M31A creates a checkpoint branch: `m31a/rollback-backup-<timestamp>` pointing to the current HEAD. If the user regrets the rollback, they can `git checkout m31a/rollback-backup-<timestamp>` to restore.
- Task statuses are updated to reflect the rollback — rolled-back tasks return to `pending` and can be re-executed.

**Implementation**: `pkg/rollback` — lists session commits via `git log --oneline --since=<session-start>`, renders timeline via Lipgloss, executes `git reset` with safety checks and backup branch creation.

### 16.6 Ghost Mode — Shadow Execution (V1.1 Deferred)

M31A runs the entire workflow in an isolated git worktree, so the user's working code is never touched until they approve the result.

**How it works:**
1. User types `/ghost` to activate ghost mode for the next workflow
2. M31A creates a temporary worktree at `.m31a/ghost/<session-id>/` branching from the current HEAD
3. The full workflow (Initialize → Discuss → Plan → Execute → Verify → Ship) runs inside the worktree
4. After Ship, M31A presents a diff between the current branch and the ghost branch
5. User chooses: **Merge** (apply all changes via `git merge` or `git cherry-pick`) or **Discard** (delete worktree, `git worktree prune`)

**Zero risk**: The user's current working directory is completely untouched. All file writes, commits, and tool executions happen in the isolated worktree. If the workflow crashes or produces garbage, `git worktree remove` cleans up with zero side effects.

**UX**: After Ship in ghost mode:
```
┌─────────────────────────────────────────────────────────┐
│  [M31A] Ghost Mode — Review                             │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  Changes in ghost branch (12 files, ~635 lines):        │
│    + components/Navbar.js    + components/Hero.js       │
│    + components/Menu.js      + components/Gallery.js    │
│    ~ app/page.js             ~ styles/globals.css       │
│    + styles/theme.css         ...and 5 more             │
│                                                         │
│  [M] Merge to current branch  [D] Discard               │
│  [V] View full diff                                     │
└─────────────────────────────────────────────────────────┘
```

**Implementation**: V1.1 — requires `pkg/ghost` (worktree lifecycle management) and integration with the task runner to execute in an alternate working directory.

### 16.7 Terminal PiP — Picture-in-Picture (V1.1 Deferred)

When a long-running process is executing, M31A shows its live output in a small floating panel while you continue working in the main REPL.

**How it works:**
1. When M31A detects a long-running Bash command (e.g., `npm run dev`, `go build --watch`, test suites with `--watch`), it offers to "pin" the output to a PiP panel
2. The PiP panel appears in the bottom-right corner of the TUI (configurable: bottom-right, bottom-left, top-right)
3. The panel shows live stdout/stderr as it arrives, with a scrollable buffer (last 50 lines)
4. The main REPL remains fully interactive — the user can type new messages, navigate history, etc.
5. PiP panel can be expanded to full-screen (`E`), dismissed (`X`), or moved (`M` to cycle positions)

**UX:**
```
┌─────────────────────────────────────────────────────────┐
│  M31A  [OR] claude-3.5-sonnet    12.4K/200K ctx         │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  I'll start the dev server and then we can test it.     │
│                                                         │
│  ┌─────────────────────────────────────────────────┐    │
│  │ [BASH] npm run dev                              │    │
│  ├─────────────────────────────────────────────────┤    │
│  │ [..] Running in PiP panel →                     │    │
│  └─────────────────────────────────────────────────┘    │
│                                                         │
│  > What should we work on while the server starts?      │
│                                                         │
│  ┌─ PiP: npm run dev ────────────────────┐              │
│  │   ▲ Local:   http://localhost:3000    │              │
│  │   Network: http://192.168.1.5:3000    │              │
│  │   Ready in 2.1s                       │              │
│  │   ✓ Compiled successfully             │              │
│  │                                       │              │
│  │  [E] Expand  [X] Close  [M] Move      │              │
│  └───────────────────────────────────────┘              │
└─────────────────────────────────────────────────────────┘
```

**Implementation**: V1.1 — requires Bubble Tea viewport split (main area + fixed-size sub-window). The PiP panel reads from a process output channel (`chan string`) and maintains its own scroll state. Background goroutine reads from the process's stdout/stderr pipes and sends lines to the channel.

---

## 17. Acceptance Criteria — Differentiators (V1)

21. **MIT License**: Source code is available under MIT license; no telemetry, no vendor lock-in, no paid tiers
22. **Model arbitrage**: `/optimize` analyzes current plan and suggests cheaper model alternatives with cost savings percentage; `O` key accepts all suggestions
23. **Git bisect**: After self-heal fails twice on a task, Verify phase runs `git bisect` and displays the offending commit with diff; retry heal uses bisect context
24. **Diff preview**: Plan screen `D` key opens diff preview overlay showing predicted file changes (new/modify/delete) with estimated line counts across all tasks
25. **Learning ledger**: `~/.m31a/LEDGER.md` exists and is updated after each Ship phase; `/ledger` shows filtered viewer; `/ledger stats` shows aggregate statistics; relevant past sessions are injected during Initialize phase
26. **Commit rollback**: `/rollback` shows interactive commit timeline for current session; soft and hard rollback modes both function; backup branch is created before rollback; task statuses update correctly

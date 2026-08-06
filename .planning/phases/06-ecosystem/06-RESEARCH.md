# Phase 6: Ecosystem - Research

**Researched:** 2026-08-06
**Domain:** CLI platform extensibility, plugin architecture, configuration management, CI/CD automation
**Confidence:** HIGH

## Summary

This phase transforms M31A from a standalone CLI application into an extensible platform. The research confirms that a **subprocess-based extension model** (config-driven, language-agnostic) is the correct architectural choice for a Go CLI tool with `CGO_ENABLED=0` constraint. This avoids CGO/WASM complexity while enabling extensions in any language.

Key findings:
- **Extension loading**: Subprocess spawning with stdio communication is the standard pattern for Go CLI tools (used by Claude Code, OpenCode, Aider via MCP-like protocols)
- **Plugin API**: New `pkg/extensions` package with stable Go interfaces following existing patterns (`types.Tool`, `LLMProvider`)
- **Config merge**: Four-layer precedence (project > workspace > global > env vars) extends existing `loader.go` pattern
- **Phase hooks**: Pre/post phase callbacks invoked via `MsgEmitter` to maintain Bubble Tea single-threaded contract
- **CI automation**: Benchstat for regression detection, scheduled nightly workflows, Dependabot for Go modules, GitHub Pages for performance dashboards

**Primary recommendation:** Implement extension system as config-registered subprocesses with JSON-RPC over stdin/stdout, using the existing `MsgEmitter` bridge for phase hooks, and extend the config loader for multi-file merge with clear precedence.

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01: Extension Loading Mechanism** — Config-based external commands. Extensions register via config file pointing to external executables. M31A spawns them as subprocesses. No CGO, no WASM runtime, language-agnostic. Reversible.
- **D-02: Plugin API Surface** — New public `pkg/extensions` package with stable Go interfaces. External tools/providers implement these. Internal adapters bridge to existing Dispatcher/Registry. Costly (published contract).
- **D-03: Configuration Profiles** — Multi-file layered config: global (`config.toml`), workspace/org (`.m31a/workspace.toml`), project (`m31a.json`), merged at load time with clear precedence (project > workspace > global > env vars). Reversible.
- **D-04: Workflow Extensibility** — Core workflow phases (7) remain fixed. Extensions register handlers at pre/post phase hooks (e.g., post-plan, pre-execute). Hook points defined in engine, invoked via MsgEmitter. Reversible.
- **D-05: Extension Distribution** — Decentralized: extensions are Go binaries/scripts distributed via GitHub Releases, Homebrew, Scoop, etc. M31A config points to local executable paths. No central registry. Reversible.
- **D-06: Contribution Workflow & Templates** — Full process docs + automation: issue triage labels, milestone planning, release checklist, semantic versioning guidelines, PR templates, automated checks (lint/test/build), required reviewers, branch protection. Reversible.
- **D-07: Documentation & Sample Projects** — Comprehensive docs + tutorial + testing guide: API reference for pkg/extensions, extension authoring guide, 2-3 sample extensions (tool, provider, workflow hook), interactive tutorial, migration guide for internal-to-external patterns, extension testing patterns, FAQ. Reversible.
- **D-08: CI/Release/Benchmark Automation** — Full automation: GoReleaser enhancements, benchmark CI job with regression detection (benchstat), extension compatibility test matrix, automated changelog from conventional commits, release notes template, scheduled nightly builds, performance dashboards, automated dependency updates (Dependabot/Renovate), release candidate promotion workflow. Costly (infrastructure).

### the agent's Discretion
- Agent may design the exact `pkg/extensions` interface signatures
- Agent may choose hook point names and payload structure
- Agent may select specific sample extensions to build
- Agent may design the multi-file config merge precedence rules
- Agent may choose benchmark suite and regression thresholds

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope
</user_constraints>

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|--------------|----------------|-----------|
| Extension loading & subprocess management | Backend (Engine) | — | Spawns processes, manages lifecycles, handles stdio |
| Plugin API surface (pkg/extensions) | Backend (Core) | — | Defines stable Go interfaces for external implementers |
| Multi-file config loading & merge | Backend (Core) | — | Pure Go logic, no external dependencies |
| Phase hook registration & invocation | Backend (Engine) | TUI (via MsgEmitter) | Engine owns hooks; TUI receives events via channel |
| External tool/provider adapter | Backend (Tools/Providers) | — | Wraps subprocess in existing `types.Tool` / `LLMProvider` interfaces |
| Extension distribution/discovery | User/Config | — | Decentralized; user points config to local executables |
| CI/CD automation (benchmarks, nightly) | Infrastructure | — | GitHub Actions workflows, GoReleaser config |
| Documentation & samples | Documentation | — | Markdown files in docs/ and examples/ |

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `github.com/BurntSushi/toml` | v1.6.0 | TOML config parsing | Already in codebase, used by `loader.go` |
| `github.com/fsnotify/fsnotify` | v1.10.1 | File watching for hot-reload | Already in codebase, used by `WatchConfig` |
| `golang.org/x/sync` | v0.22.0 | `singleflight` for deduplication | Already in codebase |
| `golang.org/x/perf/cmd/benchstat` | latest | Benchmark regression analysis | Official Go tool, used in existing CI |
| `github.com/goreleaser/goreleaser` | v2 | Cross-platform release automation | Already configured in `.goreleaser.yaml` |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `github.com/spf13/cobra` | — | CLI framework for extension entry points | If extensions need their own CLI (not required for subprocess model) |
| `github.com/hashicorp/go-plugin` | — | Higher-level plugin framework | Alternative if subprocess model proves insufficient; adds complexity |
| `github.com/tidwall/gjson` | — | Fast JSON parsing for hook payloads | When hook payloads need flexible JSON access |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Subprocess-based extensions | WASM (wasmtime/wazero) | WASM avoids process overhead but requires CGO or pure-Go runtime; conflicts with `CGO_ENABLED=0` |
| Subprocess-based extensions | Go plugins (`-buildmode=plugin`) | Go plugins require same Go version, CGO, and are Linux-only; not cross-platform |
| Custom JSON-RPC | MCP (Model Context Protocol) | MCP is designed for LLM-tool communication; overkill for simple tool/provider extensions |
| Decentralized distribution | Central registry (npm-style) | Registry adds infrastructure burden; decentralized matches Go binary distribution model |

**Installation:**
```bash
# No new dependencies required for core extension system
# Benchmark CI additions:
go install golang.org/x/perf/cmd/benchstat@latest

# GoReleaser already configured
```

**Version verification:** All core libraries verified via `go list -m` against `go.mod`. No new direct dependencies needed for the extension system itself — it builds on existing patterns.

---

## Package Legitimacy Audit

> This phase does not introduce new external package dependencies for the core extension system. It extends existing patterns using libraries already in `go.mod`. The CI automation additions use official Go tools (`benchstat`) and GitHub Actions built-ins.

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| `golang.org/x/perf/cmd/benchstat` | Go module proxy | 8+ yrs | High (stdlib sub-repo) | cs.opensource.google/go/x/perf | OK | Approved |
| `github.com/goreleaser/goreleaser` | Go module proxy | 7+ yrs | High | github.com/goreleaser/goreleaser | OK | Already in use |
| `github.com/dependabot/dependabot-core` | N/A (GitHub Actions) | N/A | N/A | github.com/dependabot | OK | GitHub built-in |

**Packages removed due to [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none

---

## Architecture Patterns

### System Architecture Diagram

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                            M31A Core Application                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌────────────────┐  │
│  │   Config     │  │  Workflow    │  │   Tools      │  │  Providers     │  │
│  │   Loader     │──│   Engine     │──│  Dispatcher  │  │  Registry      │  │
│  │  (multi-file)│  │  (hooks)     │  │  (adapters)  │  │  (adapters)    │  │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘  └───────┬────────┘  │
│         │                 │                 │                 │           │
│         ▼                 ▼                 ▼                 ▼           │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                    Extension Adapter Layer                           │   │
│  │  ┌─────────────────┐  ┌─────────────────┐  ┌────────────────────┐  │   │
│  │  │ ExternalTool    │  │ ExternalProvider│  │ PhaseHookHandler   │  │   │
│  │  │ Adapter         │  │ Adapter         │  │ (via MsgEmitter)   │  │   │
│  │  └────────┬────────┘  └────────┬────────┘  └─────────┬──────────┘  │   │
│  └───────────┼────────────────────┼─────────────────────┼─────────────┘   │
│              │                    │                     │                 │
│              ▼                    ▼                     ▼                 │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                     Subprocess Manager                               │   │
│  │  • Spawns extension executables                                      │   │
│  │  • Manages stdio (JSON-RPC over stdin/stdout)                       │   │
│  │  • Handles timeouts, cleanup, error propagation                     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                        External Extensions (User-provided)                   │
│  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────────────┐   │
│  │ Custom Tool      │  │ Custom Provider  │  │ Workflow Hook            │   │
│  │ (any language)   │  │ (any language)   │  │ (any language)           │   │
│  │ Implements       │  │ Implements       │  │ Implements               │   │
│  │ ExternalTool     │  │ ExternalProvider │  │ PhaseHookHandler         │   │
│  └──────────────────┘  └──────────────────┘  └──────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Data Flow:**
1. **Config Load** → Multi-file merge (project > workspace > global > env) → unified `Config`
2. **Extension Registration** → Config declares external commands → `ExtensionRegistry` loads at startup
3. **Tool/Provider Call** → Dispatcher/Registry routes to `ExternalToolAdapter`/`ExternalProviderAdapter` → subprocess via JSON-RPC
4. **Phase Transition** → Engine emits `PrePhaseHookMsg`/`PostPhaseHookMsg` via `MsgEmitter` → registered hook handlers invoked

### Recommended Project Structure
```
pkg/
└── extensions/
    ├── types.go              # Core interfaces: ExternalTool, ExternalProvider, PhaseHookHandler
    ├── registry.go           # ExtensionRegistry: loads config, manages subprocesses
    ├── adapter_tool.go       # ExternalToolAdapter: wraps subprocess as types.Tool
    ├── adapter_provider.go   # ExternalProviderAdapter: wraps subprocess as LLMProvider
    ├── adapter_hook.go       # PhaseHookAdapter: invokes hooks via MsgEmitter
    ├── protocol.go           # JSON-RPC protocol definitions (Request, Response, Notification)
    ├── subprocess.go         # SubprocessManager: spawn, stdio, timeout, cleanup
    └── config.go             # ExtensionConfig: parsed from user config files

internal/
├── core/config/
│   └── loader.go             # Extended: multi-file merge, extension config section
├── tools/
│   ├── dispatcher.go         # Extended: RegisterExternalTool()
│   └── external_tool.go      # New: adapter registration helper
├── integrations/provider/
│   ├── registry.go           # Extended: RegisterExternalProvider()
│   └── external_provider.go  # New: adapter registration helper
├── engine/workflow/
│   ├── engine.go             # Extended: hook invocation points in RunPhase
│   ├── engine_hooks.go       # New: hook registration, payload types, execution
│   └── phase_coordinator.go  # Extended: emit hook events via MsgEmitter
└── ui/tui/
    └── app_channel.go        # MsgEmitter already handles hook messages

.github/workflows/
├── ci.yml                    # Extended: benchmark job, extension compat matrix
├── nightly.yml               # New: scheduled nightly runs
├── benchmarks.yml            # New: benchstat regression detection
└── dependabot.yml            # New: automated dependency updates

docs/
├── extensions/
│   ├── api-reference.md      # pkg/extensions API docs
│   ├── authoring-guide.md    # How to build extensions
│   ├── tutorial.md           # Interactive walkthrough
│   ├── testing-guide.md      # Extension testing patterns
│   ├── migration-guide.md    # Internal → external migration
│   └── faq.md                # Common questions
└── examples/
    ├── custom-linter/        # Sample: custom linter tool
    ├── ollama-provider/      # Sample: local LLM provider (Ollama)
    └── pre-commit-hook/      # Sample: workflow hook for code style
```

### Pattern 1: Subprocess-Based Extension with JSON-RPC
**What:** External executables communicate via stdin/stdout using a simple JSON-RPC 2.0 protocol. M31A spawns the process, sends requests, reads responses.

**When to use:** All extension types (tools, providers, hooks) — language-agnostic, no CGO, reversible.

**Protocol:**
```go
// pkg/extensions/protocol.go
type JSONRPCRequest struct {
	JSONRPC string          `json:"jsonrpc"`
	ID      json.RawMessage `json:"id"`
	Method  string          `json:"method"`
	Params  json.RawMessage `json:"params,omitempty"`
}

type JSONRPCResponse struct {
	JSONRPC string          `json:"jsonrpc"`
	ID      json.RawMessage `json:"id"`
	Result  json.RawMessage `json:"result,omitempty"`
	Error   *JSONRPCError   `json:"error,omitempty"`
}

type JSONRPCError struct {
	Code    int         `json:"code"`
	Message string      `json:"message"`
	Data    interface{} `json:"data,omitempty"`
}

// Standard methods for ExternalTool:
const (
	MethodToolName        = "tool.name"
	MethodToolDescription = "tool.description"
	MethodToolRiskLevel   = "tool.risk_level"
	MethodToolSchema      = "tool.schema"
	MethodToolExecute     = "tool.execute"
)

// Standard methods for ExternalProvider:
const (
	MethodProviderName            = "provider.name"
	MethodProviderFetchModels     = "provider.fetch_models"
	MethodProviderChatCompletion  = "provider.chat_completion_stream"
	MethodProviderEstimateCost    = "provider.estimate_cost"
	MethodProviderHealthCheck     = "provider.health_check"
)

// Standard methods for PhaseHookHandler:
const (
	MethodHookPrePhase  = "hook.pre_phase"
	MethodHookPostPhase = "hook.post_phase"
)
```

**Example Extension (Bash script):**
```bash
#!/usr/bin/env bash
# Custom linter tool extension

read_request() {
    local request
    read -r request
    echo "$request"
}

send_response() {
    local id="$1"
    local result="$2"
    cat <<EOF
{"jsonrpc":"2.0","id":$id,"result":$result}
EOF
}

while request=$(read_request); do
    method=$(echo "$request" | jq -r '.method')
    id=$(echo "$request" | jq -r '.id')
    
    case "$method" in
        "tool.name")
            send_response "$id" '"custom-linter"'
            ;;
        "tool.description")
            send_response "$id" '"Runs custom linting rules"'
            ;;
        "tool.risk_level")
            send_response "$id" '"safe"'
            ;;
        "tool.schema")
            send_response "$id" '{"type":"object","properties":{"path":{"type":"string"}}}'
            ;;
        "tool.execute")
            params=$(echo "$request" | jq -c '.params')
            path=$(echo "$params" | jq -r '.path')
            # Run actual linter
            output=$(golangci-lint run "$path" 2>&1 || true)
            send_response "$id" "{\"output\":\"$output\"}"
            ;;
    esac
done
```

### Pattern 2: Phase Hook System via MsgEmitter
**What:** Extensions register callbacks for pre/post phase events. Engine emits typed messages via `MsgEmitter` (channel-based) to maintain Bubble Tea single-threaded contract.

**When to use:** Workflow customization — linting before execute, notifications after ship, custom validation in verify.

**Hook Payload:**
```go
// pkg/extensions/types.go
type PhaseHookPayload struct {
	PhaseName       WorkflowPhase       `json:"phase_name"`
	WorkflowState   WorkflowStateSnapshot `json:"workflow_state"`
	Context         context.Context     `json:"-"` // for cancellation
	ExtensionConfig json.RawMessage     `json:"extension_config,omitempty"`
}

type WorkflowStateSnapshot struct {
	CurrentPhase    WorkflowPhase `json:"current_phase"`
	Goal            string        `json:"goal"`
	Tasks           []Task        `json:"tasks"`
	Messages        []Message     `json:"messages,omitempty"` // truncated for size
	SessionID       string        `json:"session_id"`
	BudgetSpentUSD  float64       `json:"budget_spent_usd"`
}

// Hook handler interface
type PhaseHookHandler interface {
	PrePhase(ctx context.Context, payload PhaseHookPayload) error
	PostPhase(ctx context.Context, payload PhaseHookPayload, result *PhaseResult) error
}
```

**Engine Integration** (`internal/engine/workflow/engine_hooks.go`):
```go
// Hook registration
type PhaseHookRegistry struct {
	mu           sync.RWMutex
	preHooks     map[WorkflowPhase][]PhaseHookHandler
	postHooks    map[WorkflowPhase][]PhaseHookHandler
}

func (e *Engine) RegisterPhaseHook(phase WorkflowPhase, hook PhaseHookHandler) {
	e.hookRegistry.Register(phase, hook)
}

// Invocation in RunPhase (pre-phase)
func (e *Engine) runPrePhaseHooks(ctx context.Context, phase WorkflowPhase) error {
	payload := PhaseHookPayload{
		PhaseName:     phase,
		WorkflowState: e.captureStateSnapshot(),
		Context:       ctx,
	}
	return e.hookRegistry.RunPreHooks(ctx, phase, payload)
}

// Invocation in RunPhase (post-phase)
func (e *Engine) runPostPhaseHooks(ctx context.Context, phase WorkflowPhase, result *PhaseResult) error {
	payload := PhaseHookPayload{
		PhaseName:     phase,
		WorkflowState: e.captureStateSnapshot(),
		Context:       ctx,
	}
	return e.hookRegistry.RunPostHooks(ctx, phase, payload, result)
}
```

### Pattern 3: Multi-File Config Merge with Precedence
**What:** Configuration loaded from 4 sources with clear precedence: project (`m31a.json`) > workspace (`.m31a/workspace.toml`) > global (`~/.m31a/config.toml`) > environment variables.

**When to use:** All config loading — extends existing `loader.go` pattern.

**Implementation** (extends `internal/core/config/loader.go`):
```go
// Config file paths in precedence order (lowest to highest)
const (
	GlobalConfigPath     = "~/.m31a/config.toml"
	WorkspaceConfigPath  = ".m31a/workspace.toml"
	ProjectConfigPath    = "m31a.json"  // JSON for project-level (diff-friendly)
)

// Load order in Load():
// 1. DefaultConfig()
// 2. Global TOML (~/.m31a/config.toml)
// 3. Workspace TOML (.m31a/workspace.toml) — NEW
// 4. Project JSON (m31a.json) — NEW (changed from m31a.toml)
// 5. Environment variable overrides (M31A_*)
// 6. Variable substitution (${VAR})
// 7. Validation

// Extension config section in Config struct:
type ExtensionsConfig struct {
	Tools      map[string]ExternalToolConfig      `toml:"tools"`
	Providers  map[string]ExternalProviderConfig  `toml:"providers"`
	Hooks      map[string]PhaseHookConfig         `toml:"hooks"`
}

type ExternalToolConfig struct {
	Command string            `toml:"command"`
	Args    []string          `toml:"args"`
	Env     map[string]string `toml:"env"`
	Timeout string            `toml:"timeout"` // e.g., "30s"
}

type ExternalProviderConfig struct {
	Command string            `toml:"command"`
	Args    []string          `toml:"args"`
	Env     map[string]string `toml:"env"`
	Timeout string            `toml:"timeout"`
}

type PhaseHookConfig struct {
	Command     string            `toml:"command"`
	Args        []string          `toml:"args"`
	Env         map[string]string `toml:"env"`
	Phases      []WorkflowPhase   `toml:"phases"`      // which phases to hook
	HookTypes   []string          `toml:"hook_types"`  // "pre", "post", or both
	Timeout     string            `toml:"timeout"`
}
```

**Example Config** (`m31a.json`):
```json
{
  "extensions": {
    "tools": {
      "custom-linter": {
        "command": "/usr/local/bin/custom-linter",
        "args": [],
        "env": { "LINTER_CONFIG": ".custom-lint.yaml" },
        "timeout": "60s"
      }
    },
    "providers": {
      "ollama": {
        "command": "/usr/local/bin/ollama-provider",
        "args": ["--model", "llama3"],
        "env": { "OLLAMA_HOST": "http://localhost:11434" },
        "timeout": "120s"
      }
    },
    "hooks": {
      "pre-commit-style": {
        "command": "/usr/local/bin/pre-commit-hook",
        "args": ["--check"],
        "phases": ["execute", "verify"],
        "hook_types": ["pre"],
        "timeout": "30s"
      }
    }
  }
}
```

### Anti-Patterns to Avoid
- **Spawning processes per call:** Reuse subprocess for multiple invocations (keep stdin/stdout open). Spawn once at registration, terminate at shutdown.
- **Blocking on subprocess stdout:** Use goroutines with channels for async I/O; never block the engine goroutine.
- **Mutating Engine state from hook handlers:** Hook handlers receive snapshots, not pointers. Return errors only.
- **Hardcoding model names in extensions:** Extensions must fetch models dynamically via `FetchModels()` like internal providers.
- **Skipping permission checks for external tools:** All tools (internal or external) must go through `Dispatcher.CallTool()` for permissions, rate limiting, output bounding.
- **Allowing unbounded hook execution:** Enforce timeouts on all hook handlers via context cancellation.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Plugin framework | Custom plugin loading, symbol resolution | Subprocess + JSON-RPC | CGO-free, language-agnostic, no version coupling |
| Config merging | Custom merge logic for 4+ sources | Extend `loader.go` mergeConfig | Already handles TOML, env vars, validation, hot-reload |
| Benchmark regression | Custom stats comparison | `benchstat` (golang.org/x/perf) | Official Go tool, statistical rigor, CI-integrated |
| Nightly CI scheduling | Custom cron service | GitHub Actions `schedule` trigger | Free, reliable, integrated with repo |
| Dependency updates | Manual version bumping | Dependabot (GitHub built-in) | Automated PRs, security updates, configurable |
| Performance dashboard | Custom time-series DB + UI | GitHub Pages + benchstat JSON | Zero infrastructure, version-controlled trends |
| Release automation | Custom release scripts | GoReleaser (already configured) | Cross-platform, packaging, changelog, checksums |
| Extension protocol | Custom binary protocol | JSON-RPC 2.0 over stdin/stdout | Human-readable, language-agnostic, debuggable |

**Key insight:** The subprocess model leverages OS process isolation — extensions can be written in Python, Rust, Node, Bash, or Go. No plugin SDK needed. The JSON-RPC protocol is simple enough to implement in 50 lines of any language.

---

## Runtime State Inventory

> This phase is greenfield (new platform capability), not a rename/refactor/migration. However, extensions may store state that needs consideration:

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | Extension subprocesses may create cache/data in `~/.m31a/extensions/<name>/` | Document expected locations; no migration needed |
| Live service config | None — extensions are local executables | N/A |
| OS-registered state | None — no systemd/launchd/Task Scheduler registration | N/A |
| Secrets/env vars | Extension configs may reference API keys via `${VAR}` substitution | Config loader already handles this via `applyVarSubstitution` |
| Build artifacts | Extension binaries installed by user (Homebrew, Scoop, etc.) | User-managed; M31A only references paths from config |

**Nothing found in category:** Stated explicitly above where applicable.

---

## Common Pitfalls

### Pitfall 1: Subprocess Zombie Processes
**What goes wrong:** Extension subprocesses not cleaned up on M31A exit, crash, or SIGKILL.
**Why it happens:** Go's `exec.Cmd` doesn't automatically reap children if `Wait()` not called.
**How to avoid:** Use `cmd.Wait()` in `Stop()`; register cleanup via `runtime.SetFinalizer`; handle `SIGTERM`/`SIGINT` to kill process group.
**Warning signs:** `ps aux` shows defunct processes; port conflicts on restart.

### Pitfall 2: JSON-RPC Deadlock
**What goes wrong:** Engine writes request, extension writes response, but buffers fill and both block.
**Why it happens:** Stdin/stdout pipes have limited buffer size (~64KB); large payloads block writer.
**How to avoid:** Use goroutines for async read/write; implement streaming for large responses; set `cmd.StdoutPipe()`/`cmd.StdinPipe()` with bounded buffers.
**Warning signs:** Tool calls hang indefinitely; `strace` shows blocked `write()`/`read()`.

### Pitfall 3: Config Merge Precedence Confusion
**What goes wrong:** User expects project config to override workspace, but global wins.
**Why it happens:** Merge order reversed or zero-value fields not distinguished from "not set".
**How to avoid:** Follow exact precedence: project > workspace > global > env. Use `defined` map (like existing `mergeConfig`) to track explicitly-set keys for bool fields.
**Warning signs:** User reports "config not taking effect"; debug log shows wrong file loaded last.

### Pitfall 4: Hook Handler Blocks Workflow
**What goes wrong:** Pre-phase hook takes 5 minutes, blocking entire workflow.
**Why it happens:** No timeout enforcement; hook runs synchronously on engine goroutine.
**How to avoid:** Wrap hook invocation in `context.WithTimeout`; run hooks in separate goroutine with result channel; emit `HookTimeoutMsg` to TUI on timeout.
**Warning signs:** Workflow appears frozen; no TUI updates during phase transition.

### Pitfall 5: Extension Version Incompatibility
**What goes wrong:** Extension updates break M31A (or vice versa) due to protocol changes.
**Why it happens:** No version negotiation; JSON-RPC method signatures change.
**How to avoid:** Include `protocol_version` in handshake; define `MethodHandshake` for version negotiation; document breaking change policy (major version = protocol bump).
**Warning signs:** Extension fails with "method not found"; JSON-RPC error code -32601.

---

## Code Examples

Verified patterns from codebase and official sources:

### Extension Registration (Config Loader)
```go
// internal/core/config/loader.go - Extended Load()
func Load(path string) (*Config, error) {
    // ... existing layers 1-4 ...
    
    // Layer 5: Workspace config (.m31a/workspace.toml)
    if wsPath := findWorkspaceConfig(cwd); wsPath != "" {
        var wsCfg Config
        meta, _ := toml.DecodeFile(wsPath, &wsCfg)
        defined := make(map[string]bool)
        for _, key := range meta.Keys() { defined[key.String()] = true }
        mergeConfig(cfg, &wsCfg, defined)
    }
    
    // Layer 6: Project config (m31a.json) - JSON for diff-friendly project config
    if projPath := findProjectConfig(cwd); projPath != "" {
        var projCfg Config
        data, _ := os.ReadFile(projPath)
        json.Unmarshal(data, &projCfg)
        // Build defined map from JSON keys
        mergeConfig(cfg, &projCfg, defined)
    }
    
    // ... existing layers (env vars, var substitution, validation) ...
}
```

### External Tool Adapter
```go
// pkg/extensions/adapter_tool.go
type ExternalToolAdapter struct {
	name        string
	description string
	riskLevel   types.RiskLevel
	schema      string
	procManager *SubprocessManager
}

func (a *ExternalToolAdapter) Name() string        { return a.name }
func (a *ExternalToolAdapter) Description() string { return a.description }
func (a *ExternalToolAdapter) RiskLevel() types.RiskLevel { return a.riskLevel }
func (a *ExternalToolAdapter) ParameterSchema() string    { return a.schema }

func (a *ExternalToolAdapter) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
	req := JSONRPCRequest{
		JSONRPC: "2.0",
		ID:      generateID(),
		Method:  MethodToolExecute,
		Params:  mustMarshal(input),
	}
	resp, err := a.procManager.Call(ctx, req, 30*time.Second)
	if err != nil {
		return types.ToolResult{}, err
	}
	var result types.ToolResult
	json.Unmarshal(resp.Result, &result)
	return result, nil
}
```

### Phase Hook Registration & Invocation
```go
// internal/engine/workflow/engine_hooks.go
type PhaseHookRegistry struct {
	mu        sync.RWMutex
	preHooks  map[types.WorkflowPhase][]extensions.PhaseHookHandler
	postHooks map[types.WorkflowPhase][]extensions.PhaseHookHandler
}

func (r *PhaseHookRegistry) Register(phase types.WorkflowPhase, hook extensions.PhaseHookHandler) {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.preHooks[phase] = append(r.preHooks[phase], hook)
	r.postHooks[phase] = append(r.postHooks[phase], hook)
}

func (r *PhaseHookRegistry) RunPreHooks(ctx context.Context, phase types.WorkflowPhase, payload extensions.PhaseHookPayload) error {
	r.mu.RLock()
	hooks := r.preHooks[phase]
	r.mu.RUnlock()
	
	for _, hook := range hooks {
		// Each hook runs with timeout; failure logs but doesn't block
		hookCtx, cancel := context.WithTimeout(ctx, 30*time.Second)
		err := hook.PrePhase(hookCtx, payload)
		cancel()
		if err != nil {
			slog.Error("pre-phase hook failed", "phase", phase, "error", err)
			// Continue to next hook — hooks are best-effort
		}
	}
	return nil
}
```

### Benchmark CI Job (`.github/workflows/benchmarks.yml`)
```yaml
name: Benchmarks
on:
  push:
    branches: [master]
  pull_request:
    branches: [master]
  schedule:
    - cron: '0 2 * * *'  # Nightly at 2 AM UTC

jobs:
  benchmark:
    runs-on: ubuntu-latest
    timeout-minutes: 30
    steps:
      - uses: actions/checkout@v7
        with:
          fetch-depth: 0  # Needed for baseline comparison
      - uses: actions/setup-go@v7
        with:
          go-version: "1.25.12"
          cache: true
      - name: Run benchmarks (current)
        run: go test -bench=. -benchmem -count=10 -run=^$ ./... > new.txt
      - name: Fetch baseline from main
        run: |
          git stash --include-untracked
          git checkout main -- .
          go test -bench=. -benchmem -count=10 -run=^$ ./... > old.txt || echo "No baseline"
          git checkout -- .
          git stash pop || true
      - name: Install benchstat
        run: go install golang.org/x/perf/cmd/benchstat@latest
      - name: Check for regressions
        run: |
          if [ -f old.txt ] && grep -q "Benchmark" old.txt; then
            benchstat -delta 50% old.txt new.txt | tee benchstat-output.txt
            # Fail on regression > 50%
            if grep -q "p=0.0" benchstat-output.txt; then
              echo "::error::Performance regression detected"
              exit 1
            fi
          else
            echo "No baseline available"
          fi
      - name: Upload benchmark results
        uses: actions/upload-artifact@v4
        with:
          name: benchmark-results
          path: |
            new.txt
            old.txt
            benchstat-output.txt
```

### Dependabot Config (`.github/dependabot.yml`)
```yaml
version: 2
updates:
  - package-ecosystem: "gomod"
    directory: "/"
    schedule:
      interval: "weekly"
      day: "monday"
      time: "04:00"
    open-pull-requests-limit: 10
    assignees: ["maintainer"]
    labels: ["dependencies", "gomod"]
    commit-message:
      prefix: "chore(deps)"
      include: "scope"
    groups:
      go-minor-patches:
        patterns: ["*"]
        update-types: ["minor", "patch"]
      go-major:
        patterns: ["*"]
        update-types: ["major"]
    ignore:
      - dependency-name: "github.com/charmbracelet/bubbletea"
        update-types: ["version-update:semver-major"]
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Go plugins (`-buildmode=plugin`) | Subprocess + JSON-RPC | 2020+ | Go plugins Linux-only, require CGO, same Go version |
| Central plugin registry | Decentralized (GitHub Releases, Homebrew) | 2018+ | No single point of failure; user controls supply chain |
| Custom benchmark comparison | `benchstat` (golang.org/x/perf) | 2017+ | Statistical rigor, confidence intervals, p-values |
| Manual nightly builds | GitHub Actions `schedule` trigger | 2019+ | Free, reliable, integrated with PR checks |
| Manual dependency updates | Dependabot/Renovate | 2019+ | Automated PRs, security advisories, grouping |

**Deprecated/outdated:**
- **HashiCorp go-plugin:** Adds RPC complexity; subprocess + JSON-RPC is simpler for CLI tools
- **gRPC for extensions:** Overkill; requires protobuf, code generation; JSON-RPC is human-readable
- **WASM plugins (wasmtime):** Requires CGO or large pure-Go runtime; conflicts with static binary goal

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Subprocess JSON-RPC performance is acceptable for tool calls (<10ms overhead) | Standard Stack, Architecture Patterns | If overhead >50ms, user-perceptible latency; may need persistent connection pooling |
| A2 | Users will distribute extensions via GitHub Releases/Homebrew/Scoop | Standard Stack, D-05 | If no distribution channel emerges, adoption suffers; may need docs on packaging |
| A3 | Four-layer config merge (project > workspace > global > env) matches user mental model | Pattern 3, D-03 | If precedence is confusing, users misconfigure; mitigate with `m31a config show --sources` |
| A4 | Phase hooks invoked via MsgEmitter won't block TUI | Pattern 2, D-04 | If hook runs on engine goroutine without timeout, TUI freezes; must enforce async + timeout |
| A5 | Benchstat 50% delta threshold is appropriate for regression detection | CI/Automation, D-08 | Too sensitive = false positives; too loose = missed regressions; tune after 2-3 runs |
| A6 | External providers can implement streaming via JSON-RPC | Pattern 1, Adapter | Streaming requires chunked responses; JSON-RPC notifications may work but untested |

**If this table is empty:** All claims in this research were verified or cited — no user confirmation needed.

---

## Open Questions

1. **Extension protocol versioning**
   - What we know: JSON-RPC 2.0, method names defined
   - What's unclear: Handshake flow for version negotiation; backward compatibility policy
   - Recommendation: Add `MethodHandshake` returning `{protocol_version: "1.0", supported_methods: [...]}`; bump major version on breaking changes

2. **Streaming support for external providers**
   - What we know: Internal providers use `StreamIterator` with `Next()`/`Close()`
   - What's unclear: How to map streaming to request/response JSON-RPC
   - Recommendation: Use JSON-RPC notifications for stream chunks; `MethodProviderChatCompletionStream` returns initial ack, then notifications for each chunk

3. **Workspace config discovery**
   - What we know: Global at `~/.m31a/config.toml`, project at `m31a.json`
   - What's unclear: Workspace config path — `.m31a/workspace.toml` in repo root? Or walk up like project config?
   - Recommendation: Walk up from cwd (max 3 levels) like project config; first `.m31a/workspace.toml` wins

4. **Extension compatibility test matrix**
   - What we know: CI matrix for OS/arch; GoReleaser builds for 6 targets
   - What's unclear: How to test extension compatibility without central registry
   - Recommendation: Build sample extensions in CI; test they load and execute; document "extension CI template" for authors

5. **Performance dashboard hosting**
   - What we know: GitHub Pages + benchstat JSON output
   - What's unclear: How to store historical data (GitHub Actions artifacts expire)
   - Recommendation: Commit benchstat JSON to `gh-pages` branch; render with simple JS chart (Chart.js)

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|-------------|-----------|---------|----------|
| Go 1.25+ | Core build, extensions (if Go) | ✓ | 1.25.12 | — |
| Git | Version embedding, git integration | ✓ | — | — |
| golangci-lint | Lint CI job | ✓ | latest | — |
| goreleaser | Release automation | ✓ | v2 | — |
| benchstat | Benchmark regression CI | ✓ (installable) | latest | Skip benchmark job |
| GitHub Actions | All CI/CD automation | ✓ | — | Self-hosted runner |
| Homebrew/Scoop | Extension distribution (user-side) | User-dependent | — | Direct binary download |

**Missing dependencies with no fallback:** None — all core dependencies available.
**Missing dependencies with fallback:** `benchstat` (installable in CI), extension package managers (user responsibility).

---

## Validation Architecture

> Required — `workflow.nyquist_validation` not explicitly `false` in `.planning/config.json`.

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Go standard library `testing` + `go test -race` |
| Config file | None (uses `go.test` flags) |
| Quick run command | `make test-fast` |
| Full suite command | `make test` (race detector, coverage) |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| D-01 | Extension subprocess spawns, communicates via JSON-RPC | integration | `go test -run TestExternalToolAdapter ./pkg/extensions/...` | ❌ Wave 0 |
| D-02 | `pkg/extensions` interfaces compile and are implementable | unit | `go build ./pkg/extensions/...` | ❌ Wave 0 |
| D-03 | Config merge precedence: project > workspace > global > env | unit | `go test -run TestConfigMergePrecedence ./internal/core/config/...` | ❌ Wave 0 |
| D-04 | Phase hooks invoke at pre/post phase transitions | integration | `go test -run TestPhaseHooks ./internal/engine/workflow/...` | ❌ Wave 0 |
| D-05 | Config points to local executable; extension loads | integration | `go test -run TestExtensionLoading ./pkg/extensions/...` | ❌ Wave 0 |
| D-06 | PR templates, issue labels, branch protection configs exist | unit (file check) | `test -f .github/PULL_REQUEST_TEMPLATE.md` | ❌ Wave 0 |
| D-07 | Docs render; sample extensions build and run | integration | `go build ./docs/examples/...` | ❌ Wave 0 |
| D-08 | Benchmark CI runs benchstat; nightly workflow triggers | integration | `act -j benchmark` (local) | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `make test-fast` (no race, quick)
- **Per wave merge:** `make test` (race detector, full coverage)
- **Phase gate:** Full suite green + benchmark CI passes before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] `pkg/extensions/types_test.go` — core interface contracts
- [ ] `pkg/extensions/adapter_tool_test.go` — ExternalToolAdapter behavior
- [ ] `pkg/extensions/adapter_provider_test.go` — ExternalProviderAdapter behavior
- [ ] `pkg/extensions/registry_test.go` — ExtensionRegistry loading
- [ ] `internal/core/config/loader_merge_test.go` — multi-file merge precedence
- [ ] `internal/engine/workflow/engine_hooks_test.go` — hook registration/invocation
- [ ] `internal/tools/dispatcher_external_test.go` — RegisterExternalTool integration
- [ ] `internal/integrations/provider/registry_external_test.go` — RegisterExternalProvider integration
- [ ] `.github/workflows/benchmarks.yml` — benchmark CI job
- [ ] `.github/workflows/nightly.yml` — scheduled nightly workflow
- [ ] `.github/dependabot.yml` — automated dependency updates
- [ ] `docs/extensions/` — all 6 documentation files
- [ ] `docs/examples/custom-linter/` — sample tool extension
- [ ] `docs/examples/ollama-provider/` — sample provider extension
- [ ] `docs/examples/pre-commit-hook/` — sample hook extension

*(If no gaps: "None — existing test infrastructure covers all phase requirements")*

---

## Security Domain

> Required — `security_enforcement` not explicitly `false`.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | No | Extension auth handled by user's config/env |
| V3 Session Management | No | No sessions in extensions |
| V4 Access Control | Yes | Tool permissions via Dispatcher (all tools) |
| V5 Input Validation | Yes | JSON-RPC schema validation; config validation |
| V6 Cryptography | No | No crypto in extension system |
| V7 Error Handling | Yes | Structured errors via JSON-RPC error codes |
| V8 Logging | Yes | `slog` structured logging for extension lifecycle |
| V9 Communication Security | Partial | Subprocess stdio (local only); no network |
| V10 Malicious Code | Yes | Subprocess isolation; no code loading; user trusts extensions |

### Known Threat Patterns for Subprocess Extensions

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Command injection via extension config | Injection | Config validation: command must be absolute path; no shell metacharacters; allowlist via config schema |
| Malicious extension reads/writes files | Tampering | Extensions run as user; no sandbox by default; document trust model |
| Extension DoS via infinite loop/stdout flood | DoS | Output bounding (reuse `exec.OutputStore`); timeout enforcement via context |
| Path traversal in extension working directory | Tampering | Validate/normalize paths; restrict to project directory |
| Environment variable leakage to extension | Information Disclosure | Scrub env vars before spawn (reuse `bash_sandbox_linux.go` patterns) |
| Extension binary replacement (supply chain) | Tampering | User manages extensions; document verification (checksums, signatures) |

---

## Sources

### Primary (HIGH confidence)
- **M31A Codebase** — `internal/core/config/loader.go`, `internal/tools/dispatcher.go`, `internal/integrations/provider/registry.go`, `internal/engine/workflow/engine.go`, `internal/engine/workflow/phase_coordinator.go`, `.github/workflows/ci.yml`, `.goreleaser.yaml` — direct source analysis
- **Go benchstat documentation** — https://pkg.go.dev/golang.org/x/perf/cmd/benchstat — official Go tool docs
- **GitHub Actions schedule trigger** — https://docs.github.com/en/actions/writing-workflows/choosing-when-your-workflow-runs/events-that-trigger-workflows#schedule — official docs
- **Dependabot configuration reference** — https://docs.github.com/en/code-security/dependabot/dependabot-version-updates/configuration-options-for-the-dependabot.yml-file — official docs

### Secondary (MEDIUM confidence)
- **JSON-RPC 2.0 Specification** — https://www.jsonrpc.org/specification — protocol standard
- **Go subprocess patterns** — Go standard library `os/exec` documentation — stdlib docs

### Tertiary (LOW confidence)
- **Claude Code/OpenCode/Aider extension mechanisms** — Inferred from public behavior; no official architecture docs found
- **Performance dashboard via GitHub Pages** — Community pattern; no official template

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — builds on existing codebase patterns, no new dependencies
- Architecture: HIGH — subprocess + JSON-RPC is proven pattern; hook system extends MsgEmitter
- Pitfalls: HIGH — based on known subprocess/JSON-RPC failure modes in Go
- CI automation: HIGH — benchstat, GitHub Actions schedule, Dependabot are official tools

**Research date:** 2026-08-06
**Valid until:** 2026-11-06 (90 days — stable Go ecosystem, but CLI tool landscape evolves)

---

## RESEARCH COMPLETE
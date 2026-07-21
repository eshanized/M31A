# M31A Architectural Decomposition Plan

## Executive Summary

M31A is a 175,358-line Go codebase implementing a terminal-native AI coding agent with a 7-phase workflow engine, 18 built-in tools, 3 LLM providers, and a 33-screen Bubble Tea TUI. The codebase has **no circular dependencies** and maintains clean layering (`pkg/` never imports `internal/`), but three critical god objects have accumulated excessive responsibilities:

1. **`workflow.Engine`** (1,707 lines, 47 fields, 85+ methods) -- orchestrates all 7 workflow phases, LLM streaming, tool dispatch, pause/resume, checkpointing, decision logging, cost tracking, and code intelligence
2. **`tui.AppState`** (~108 fields, 177 methods, 38 sub-models, 112 message types) -- owns every screen model, workflow engine wiring, session management, provider registry, permission system, and all message routing
3. **`tui.SidebarModel`** (47 fields, 51 methods) -- mixes git status, token metrics, phase pipeline, tool timeline, TODO tracking, cost tracking, and rendering

Additionally, `config/loader.go` (1,154 lines) mixes 6+ concerns, `tools/webfetch.go` (1,147 lines) embeds a full HTML-to-Markdown parser, and the config merge logic is missing 49 fields.

This plan proposes a **4-phase decomposition** that preserves all public APIs, behavior, and test coverage while reducing the average file size and clarifying ownership boundaries. No code is modified in this analysis -- this document is the blueprint for all future refactoring.

---

## Architecture Health Score

| Dimension | Score | Notes |
|-----------|-------|-------|
| **Dependency Health** | 9/10 | Clean DAG, no cycles, `pkg/` isolation enforced. 16 leaf packages. |
| **Package Cohesion** | 6/10 | `workflow` and `tui` packages are too wide (19 and 30 internal deps respectively). |
| **File Size Health** | 4/10 | 40 non-test files exceed 500 lines. 6 exceed 1,000 lines. |
| **Struct Health** | 3/10 | Engine (47 fields), AppState (~108 fields), SidebarModel (47 fields) are god objects. |
| **Separation of Concerns** | 5/10 | Config mixes loading/saving/validation/watching. Tools mix execution/permissions/rate-limiting. |
| **Testability** | 7/10 | Good test coverage, but god objects require extensive mocking. |
| **Maintainability** | 4/10 | Recent regressions attributed to oversized files with mixed responsibilities. |
| **Overall** | **5.4/10** | Functional architecture with critical structural debt. |

---

## Oversized Files

### Tier 1: Critical (> 1,000 lines, multiple responsibilities)

| File | Lines | Fields | Methods | Responsibilities |
|------|-------|--------|---------|------------------|
| `internal/workflow/engine.go` | 1,707 | 47 | 85+ | Phase orchestration, LLM streaming, checkpointing, pause/resume, decision logging, cost tracking, context building, code intelligence |
| `internal/tui/sidebar_model.go` | 1,652 | 47 | 51 | Git status, token metrics, phase pipeline, tool timeline, TODO tracking, cost tracking, 12 render methods, input handling |
| `internal/tui/app_view.go` | 1,266 | 0 | 37 | 30+ per-screen renderers, sidebar integration, permission modal, toast overlay |
| `internal/config/loader.go` | 1,154 | 0 | 24 | Loading, saving, validation, env substitution, .env parsing, file watching, keychain integration |
| `internal/workflow/execute.go` | 1,152 | 1 | 12 | Task execution orchestration, self-heal loop, quality gates, git commits, error classification |
| `internal/tools/webfetch.go` | 1,147 | 1 | 26 | HTTP client, SSRF protection, DNS caching, full HTML-to-Markdown parser (~700 lines) |
| `internal/tui/firstrun_view.go` | 1,144 | 1 | 15+ | 6-step wizard rendering (welcome, provider select, API key, model pick, done) |

### Tier 2: Significant (500-1,000 lines)

| File | Lines | Key Issue |
|------|-------|-----------|
| `internal/tui/settings_model.go` | 901 | Mixed UI + provider health + keychain |
| `internal/codeintel/parser.go` | 861 | 5 language parsers in one file |
| `internal/git/git.go` | 827 | 30 git operations, one file |
| `internal/tools/edit.go` | 821 | 5 edit strategies + backup + metrics |
| `internal/tui/firstrun_model.go` | 780 | 29-field model, wizard state + API validation |
| `internal/tui/app.go` | 773 | Lifecycle + emitter drain + background services |
| `internal/tui/theme/tokens_semantic.go` | 771 | 178-field style struct |
| `internal/workflow/engine_parse.go` | 726 | Task parsing + tool call parsing + project detection |
| `internal/tui/streaming/agent_loop.go` | 713 | Agent loop + message building + compression |
| `internal/tui/components/message.go` | 692 | 16 message variant renderers |
| `internal/tools/permissions.go` | 691 | Rule matching + batch approval + user prompting |
| `pkg/session/manager.go` | 661 | Session CRUD + message persistence + recent models |
| `internal/tui/components/permission_desc.go` | 656 | 21 tool-specific description generators |
| `internal/tools/grep.go` | 638 | Two search backends + gitignore + comment detection |
| `internal/workflow/ship.go` | 631 | Git ops + diff analysis + summary generation |
| `internal/tools/bash.go` | 630 | Execution + security validation + output management |
| `internal/tui/repl_state.go` | 613 | 35 REPL state methods |
| `internal/tools/git.go` | 606 | 15 git operations in tool wrapper |
| `cmd/m31a/main.go` | 598 | CLI parsing + headless mode + TUI bootstrap |
| `internal/tui/tuitypes/tuitypes.go` | 596 | 44 message types + WorkflowEngine interface |
| `internal/tui/app_nav.go` | 591 | Navigation routing with 250-line switch |
| `internal/tools/devserver.go` | 585 | Process management + log buffering + crash monitoring |
| `internal/tui/repl.go` | 578 | REPL Bubble Tea model + key handling |
| `internal/tui/app_update.go` | 576 | Main Update() dispatch with 112 message types |
| `internal/config/types.go` | 571 | 35+ config structs, UIConfig=61 fields |
| `pkg/ledger/ledger.go` | 562 | Parsing + querying + stats + file I/O |
| `internal/workflow/plan.go` | 535 | LLM invocation + task parsing + quality gates |
| `pkg/autodream/autodream.go` | 534 | Session consolidation |
| `internal/workflow/engine_verify.go` | 531 | File reading + package detection + verification |
| `internal/tui/app_routing.go` | 527 | Screen updater registration (~470 lines) |
| `internal/tui/commandpalette_model.go` | 524 | Search + rendering + navigation |
| `internal/workflow/runtime.go` | 521 | Multi-language server startup + smoke testing |

---

## Oversized Structs

### Engine (47 fields, 85+ methods)

The primary god object. Fields grouped by responsibility:

| Category | Fields | Count |
|----------|--------|-------|
| Identity/Paths | sessionID, workDir, backupDir, planningDir, sessionStartHash | 5 |
| Provider/Model | provider, modelID, modelIDMu, perPhaseModels, perPhaseModelsMu | 5 |
| Infrastructure | stateMachine, cache, cfg, git, dispatcher, tokens, sessionMgr, promptBuilder, logger, execCommand | 10 |
| State | state, discussState, workflowMode, workflowModeMu, websiteTemplateDir, toolCallsSinceLastCompact, done, cancel | 8 |
| LLM Streaming | contextBuilder, callCounter, contextRegistry | 3 |
| Metrics/Cost | costTracker, collector, startTime | 3 |
| Code Intelligence | codeIntel, codeIntelMu, codeIntelBuilt | 3 |
| v1.5 Subsystems | phaseCoordinator, compactor, ledger | 3 |
| Pause/Resume | pauseMu, pauseCh, resumeCh, skipTaskCh, cancelTaskCh, cancelGroupCh | 6 |
| UI Communication | msgEmitter | 1 |

### AppState (~108 fields, 177 methods, 38 sub-models)

The TUI god object. The Update() function handles 112 distinct message types. Key field groups:

| Category | Count |
|----------|-------|
| Layout/Screen Routing | 8 |
| Config/Session/Provider | 9 |
| Workflow Engine | 10 |
| Core Sub-Models | 38 |
| Permission/Question Modals | 6 |
| Toast/Notifications | 3 |
| Agent/Provider | 8 |
| Narrative | 3 |
| Misc (arbitrage, health, transitions, file watcher, etc.) | 23 |

### SidebarModel (47 fields, 51 methods)

| Category | Fields | Methods |
|----------|--------|---------|
| Git Status | 3 | 3 |
| Token Metrics/Cost | 15 | 7 |
| Phase Pipeline | 3 | 4 |
| Tool Timeline | 2 | 3 |
| TODO/Task Progress | 3 | 9 |
| Context Pressure | 1 | 1 |
| Compaction Tracking | 3 | 1 (dead code) |
| Sub-Agent Status | 2 | 1 |
| File Watcher | 1 | 3 |
| Layout/Rendering | 11 | 12 |

### UIConfig (61 fields in config/types.go)

Broken into 18 subsystems. Too many fields for a single struct. The largest groups are Toast (7), Welcome Screen (5), Animation (5), Layout (5), and Sidebar (4).

### FeaturesConfig (46 fields in config/types.go)

Broken into 16 subsystems. The largest groups are Plan Phase (8), Retry Policy (5), and Verify/Ship (4).

---

## Mixed Responsibilities

### workflow.Engine

The Engine mixes **10+ distinct responsibilities**:

1. **Phase Orchestration** -- RunPhase, Transition, state machine management
2. **LLM Streaming** -- streamLLM, prepareStreamRequest, consumeStream, retryChatStream
3. **Tool Dispatch** -- buildToolDefinitions, parseToolCalls, dispatcher interaction
4. **Context Building** -- buildSystemPrompt, renderDynamicContext, buildToolDefinitions, 12 build*Context methods
5. **Pause/Resume Control** -- 6 pause-related methods + 6 channels
6. **Checkpointing** -- SaveCheckpointData, LoadCheckpointData, GetCheckpointData
7. **Decision Logging** -- LogDecision, FlushDecisions, SnapshotDecisions, emitDecisionsSnapshot
8. **Cost Tracking** -- costTracker, GetCostInfo, calibrateFromUsage
9. **Code Intelligence** -- getCodeIntel, codeIntel lazy build
10. **Self-Heal** -- healTask, healCreatedExpectedFiles
11. **Discuss Phase State** -- SubmitDiscussAnswer, SkipDiscuss, FinalizeDiscuss, DiscussState

### tui.AppState

The AppState mixes **12+ distinct responsibilities**:

1. **Screen Routing** -- routeToScreen, navigateToScreen, screenStack, router
2. **Message Dispatch** -- Update() with 112 message types
3. **Rendering** -- View(), 30+ renderXxxContent methods
4. **Workflow Management** -- 44 workflow-related methods
5. **Session Management** -- startNewSession, loadAndRestoreSession, saveSessionOnShutdown
6. **Provider Management** -- handleModelSelected, handleHealthCheckResult, reRegisterProvidersFromConfig
7. **Permission/Question Modals** -- 6 fields, 4 methods
8. **Config Watching** -- startConfigWatcher, handleConfigReload
9. **File Watching** -- startFileWatcher, drainFileWatcherCmd
10. **Agent Loop** -- 12 agent-related methods
11. **Toast Notifications** -- addToast, removeToastByID, toastTimers
12. **Narrative** -- narrativeEngine, narrativeBridge, narrativeState

### tui.SidebarModel

Mixes **9+ distinct responsibilities** (see Oversized Structs section above).

### config/loader.go

Mixes **6+ distinct responsibilities**: loading, saving, validation, env substitution, .env parsing, file watching.

### tools/webfetch.go

Mixes HTTP client management with a complete HTML-to-Markdown parser (~700 lines of HTML conversion functions).

### tools/dispatcher.go

Mixes tool registry, execution pipeline, permission management, rate limiting, concurrency control, output bounding, metrics collection, and TODO synchronization.

---

## Package Ownership

### Current State (problematic)

| Package | Internal Deps | Issue |
|---------|---------------|-------|
| `internal/tui` | **30** | Imports nearly the entire codebase. Wide fan-in makes changes risky. |
| `internal/workflow` | **19** | Second-widest. Mixes orchestration with LLM streaming and persistence. |
| `internal/tools` | **8** | Dispatcher mixes execution with permissions and rate limiting. |
| `internal/config` | **3** | Clean deps, but loader.go mixes too many concerns. |

### Proposed Package Ownership

| Domain | Owning Package | Responsibility |
|--------|---------------|----------------|
| **Lifecycle** | `cmd/m31a` | Application startup, signal handling, provider wiring |
| **Sessions** | `pkg/session` | Persistence, message storage, checkpoint management |
| **Dispatcher** | `internal/tools/dispatcher` | Tool registry only (execution pipeline stays, permissions split) |
| **Providers** | `internal/provider` | LLM provider interface, registry, cache, fallback |
| **Tools** | `internal/tools` | Individual tool implementations |
| **Workflow** | `internal/workflow` | Phase orchestration only (streaming, context, parsing split) |
| **Rendering** | `internal/tui` | Screen routing and frame composition |
| **Persistence** | `pkg/session`, `pkg/ledger` | Disk I/O for sessions and cost tracking |
| **Configuration** | `internal/config` | Loading, types, validation (split by concern) |
| **Code Intelligence** | `internal/codeintel` | AST parsing, indexing, relevance |

---

## Lifecycle Ownership

| Lifecycle Event | Current Owner | Recommended Owner |
|----------------|---------------|-------------------|
| Application init | `cmd/m31a/main.go` | `cmd/m31a/main.go` (unchanged) |
| Provider registration | `cmd/m31a/main.go` | `cmd/m31a/main.go` (unchanged) |
| Engine construction | `workflow.NewEngine` | `workflow.NewEngine` (unchanged) |
| Engine wiring | `tui.AppState.initWorkflowEngine` | `tui.AppState.initWorkflowEngine` (unchanged) |
| Phase transitions | `Engine.Transition` | `Engine.Transition` (unchanged) |
| Engine shutdown | `Engine.Shutdown` | `Engine.Shutdown` (unchanged) |
| Session save | `tui.AppState.saveSessionOnShutdown` | `session.Manager` (already delegated) |
| Config hot-reload | `tui.AppState.startConfigWatcher` | `config.WatchConfig` (already delegated) |

---

## Resource Ownership

| Resource | Current Owner | Recommended Owner |
|----------|---------------|-------------------|
| LLM Provider | `Engine.provider` | `Engine.provider` (unchanged, mutex-protected) |
| Git operations | `Engine.git` | `Engine.git` (unchanged) |
| Tool dispatcher | `Engine.dispatcher` | `Engine.dispatcher` (unchanged) |
| Session manager | `Engine.sessionMgr` | `Engine.sessionMgr` (unchanged) |
| Token estimator | `Engine.tokens` | `Engine.tokens` (unchanged) |
| Code intelligence | `Engine.codeIntel` | `Engine.codeIntel` (unchanged, lazy) |
| Cost tracker | `Engine.costTracker` | `Engine.costTracker` (unchanged) |
| Metrics collector | `Engine.collector` | `Engine.collector` (unchanged) |
| Permission channels | `Dispatcher.requestCh/responseCh` | Split into `internal/tools/permissions/` |
| Pause channels | `Engine.pauseCh/resumeCh/etc` | `Engine` (unchanged, but extracted to PauseController) |
| Emitter channel | `AppState.emitterCh` | `AppState.emitterCh` (unchanged) |

---

## Dependency Problems

### 1. `internal/tui` imports 30 internal packages

This makes `tui` the widest point in the dependency graph. Any change to any internal package risks breaking the TUI.

**Mitigation:** Introduce interfaces at package boundaries. The TUI should depend on interfaces, not concrete types.

### 2. `internal/tui/tuitypes` imports `internal/workflow`

The `WorkflowEngine` interface in `tuitypes.go` imports workflow types. This creates a coupling between the TUI type vocabulary and the workflow engine.

**Mitigation:** Move `WorkflowEngine` interface to `internal/types` or create a dedicated `internal/contracts` package.

### 3. `internal/tools` imports `internal/config` and `internal/provider`

Tools depend on config for permission rules and on provider for the LLM interface. This is acceptable but should be narrowed.

**Mitigation:** Tools should receive configuration via constructor parameters, not import config directly.

### 4. Config merge.go is missing 49 fields

49 fields defined in `types.go` have no corresponding merge logic in `merge.go`. 4 top-level sections (ModelCapabilities, Prompts, Narrative, Templates) have no merge function at all.

**Mitigation:** Add missing merge fields. Consider reflection-based merge for future-proofing.

---

## Recommended Decomposition Order

### Phase 1: Config Package Decomposition (Low Risk, High Value)

**Goal:** Split `config/loader.go` (1,154 lines) into focused files and fix the 49-field merge gap.

| New File | Responsibility | Lines (est.) |
|----------|---------------|-------------|
| `config/types.go` | Root config + small structs (unchanged) | 571 |
| `config/types_ui.go` | UIConfig + sub-configs | ~150 |
| `config/types_features.go` | FeaturesConfig + sub-configs | ~120 |
| `config/types_tools.go` | ToolsConfig + sub-configs | ~100 |
| `config/loader.go` | Load() orchestrator + DefaultConfig() | ~350 |
| `config/save.go` | Save, SaveWithKeychain, SaveProject | ~200 |
| `config/validate.go` | validateConfig + knownConfigKeys | ~300 |
| `config/envsubst.go` | applyVarSubstitution, substituteVars, LoadDotEnv | ~120 |
| `config/watch.go` | WatchConfig, watchConfigPolling, sendReload | ~100 |
| `config/merge.go` | Merge logic (fix 49 missing fields) | ~500 |

**Estimated refactor size:** ~300 line changes across file splits, ~200 lines of new merge functions.
**Risk:** Low -- purely structural, no behavior change. Tests split alongside source.
**Tests affected:** `loader_test.go`, `merge_test.go`, `config_extra_test.go`, `extra_test.go`.

### Phase 2: Workflow Engine Decomposition (Medium Risk, Critical Value)

**Goal:** Decompose `workflow.Engine` from 47 fields / 85+ methods into focused components.

| New File/Component | Responsibility | Extracted From | Fields | Methods |
|-------------------|---------------|----------------|--------|---------|
| `workflow/engine.go` | Core orchestrator (RunPhase, Transition, constructor, getters/setters) | engine.go | 15 | 30 |
| `workflow/streaming.go` | LLM streaming pipeline (streamLLM, prepareStreamRequest, consumeStream, retryChatStream) | engine.go | 3 | 12 |
| `workflow/context.go` | Context building (buildSystemPrompt, renderDynamicContext, buildToolDefinitions, 12 build*Context) | engine.go + separate files | 3 | 15 |
| `workflow/pause.go` | PauseController (pauseMu, pauseCh, resumeCh, skipTaskCh, cancelTaskCh, cancelGroupCh + 8 methods) | engine.go | 7 | 8 |
| `workflow/checkpoint.go` | CheckpointManager (Save/Load/GetCheckpointData) | engine.go | 2 | 3 |
| `workflow/decisions.go` | DecisionLogger (LogDecision, FlushDecisions, SnapshotDecisions, emitDecisionsSnapshot) | engine.go | 1 | 4 |
| `workflow/execute.go` | Execute phase (unchanged file, ~1,152 lines) | execute.go | 0 | 12 |
| `workflow/engine_parse.go` | Parsing utilities (split into 3 files) | engine_parse.go | 0 | 17 |

The Engine struct shrinks from 47 fields to ~15 core fields. Components are embedded (not pointed to) to preserve the existing API.

**Estimated refactor size:** ~800 line changes. Extract methods into new files, embed components.
**Risk:** Medium -- Engine methods are called from many places, but embedding preserves the API.
**Tests affected:** `engine_test.go`, `engine_extra_test.go`, `engine_wiring_test.go`, all phase test files.

### Phase 3: TUI Sidebar and View Decomposition (Medium Risk, High Value)

**Goal:** Decompose `SidebarModel` (47 fields) and `app_view.go` (1,266 lines).

#### SidebarModel Decomposition

| New Component | Responsibility | Fields | Methods |
|---------------|---------------|--------|---------|
| `sidebar/metrics.go` | Token usage, burn rate, cost tracking, execution speed | 15 | 7 |
| `sidebar/phases.go` | Phase pipeline state machine | 3 | 4 |
| `sidebar/timeline.go` | Tool call timeline ring buffer | 2 | 3 |
| `sidebar/tasktracker.go` | TODO items and task progress | 3 | 9 |
| `sidebar/filewatcher.go` | Smart file change tracking with TTL | 1 | 3 |
| `sidebar_model.go` | Remaining: layout, rendering, input, mode routing | ~22 | ~25 |

**Estimated refactor size:** ~500 line changes. Extract to sub-components, update render methods.
**Risk:** Medium -- rendering methods reference extracted fields, but delegation is straightforward.
**Tests affected:** Sidebar test files (if any), `sidebar_extra_test.go`.

#### app_view.go Decomposition

| New File | Responsibility | Lines (est.) |
|----------|---------------|-------------|
| `app_view.go` | View(), renderFrame(), buildSidebarAndChrome(), modal rendering | ~300 |
| `app_view_screens.go` | All 30+ renderXxxContent() methods | ~600 |
| `app_view_helpers.go` | buildHeaderInfo, buildFooterInfo, overlay utilities | ~200 |

**Estimated refactor size:** ~200 line changes (file splits only).
**Risk:** Low -- purely structural file splits within the same package.

### Phase 4: Tools Package Refinement (Low Risk, Medium Value)

**Goal:** Split `webfetch.go` HTML parser and refine dispatcher responsibilities.

| New File/Package | Responsibility | Lines (est.) |
|-----------------|---------------|-------------|
| `tools/webfetch.go` | HTTP client, SSRF protection, DNS caching | ~450 |
| `tools/html2md.go` | HTML-to-Markdown conversion (all convert* functions) | ~700 |
| `tools/permissions.go` | Permission rule evaluation (already separate) | 691 |
| `tools/bash_security.go` | Dangerous command detection, obfuscation checks | ~200 |
| `tools/edit_strategies.go` | Edit match strategies (exact, trimmed, normalized, anchor, fuzzy) | ~300 |

**Estimated refactor size:** ~300 line changes.
**Risk:** Low -- purely structural file splits within the same package.
**Tests affected:** `webfetch_test.go`, `bash_test.go`, `edit_test.go`.

---

## Phase 1: Config Package Decomposition

### Current State

```
internal/config/
├── types.go          (571 lines) -- 35+ structs
├── loader.go         (1,154 lines) -- Load, Save, Validate, Watch, EnvSubst, DotEnv
├── merge.go          (349 lines) -- Field-by-field merge (49 fields missing)
├── instructions.go   (85 lines) -- AGENTS.md discovery
└── project_context.go (34 lines) -- Project context loading
```

### Proposed State

```
internal/config/
├── types.go              (571 lines) -- Root Config, ProviderConfig, small structs
├── types_ui.go           (~150 lines) -- UIConfig, FeaturesConfig
├── types_tools.go        (~100 lines) -- ToolsConfig, PermissionsConfig
├── loader.go             (~350 lines) -- Load(), DefaultConfig(), findProjectConfig()
├── save.go               (~200 lines) -- Save(), SaveWithKeychain(), SaveProject()
├── validate.go           (~300 lines) -- validateConfig(), knownConfigKeys()
├── envsubst.go           (~120 lines) -- applyVarSubstitution(), LoadDotEnv()
├── watch.go              (~100 lines) -- WatchConfig(), watchConfigPolling()
├── merge.go              (~500 lines) -- All merge functions (49 fields added)
├── instructions.go       (85 lines) -- unchanged
└── project_context.go    (34 lines) -- unchanged
```

### Key Decisions

1. **Split types.go by subsystem** -- `types_ui.go` for UIConfig (61 fields) and FeaturesConfig (46 fields), `types_tools.go` for ToolsConfig (27 fields). This improves navigability without changing any API.

2. **Extract save.go** -- The Save/SaveWithKeychain/SaveProject methods deal with keychain integration, deep-copy safety, and atomic writes. They are a distinct concern from loading.

3. **Extract validate.go** -- The 260-line validateConfig function is pure validation logic. Separating it makes it independently testable and easier to extend.

4. **Extract envsubst.go** -- Variable substitution (${VAR} patterns) and .env file loading are related but distinct from TOML loading.

5. **Extract watch.go** -- Config file watching (fsnotify + polling fallback) is self-contained.

6. **Fix the 49-field merge gap** -- Add missing merge fields for UIConfig (17), FeaturesConfig (10), ToolsConfig (16), ProviderConfig (3), CompactionConfig (2), AgentsConfig (1). Add merge functions for ModelCapabilities, Prompts, Narrative, Templates.

### Dependencies Affected

- No external package changes. All splits are within `internal/config`.
- Import paths unchanged.

### Tests Affected

- `loader_test.go` (981 lines) -- split alongside source
- `merge_test.go` (405 lines) -- add tests for 49 missing fields
- `config_extra_test.go` (341 lines) -- split alongside source
- `extra_test.go` (692 lines) -- split alongside source
- `integration_test.go` (88 lines) -- unchanged

---

## Phase 2: Workflow Engine Decomposition

### Current State

```
internal/workflow/
├── engine.go           (1,707 lines) -- God object: 47 fields, 85+ methods
├── execute.go          (1,152 lines) -- Execute phase
├── engine_parse.go     (726 lines) -- Parsing utilities
├── plan.go             (535 lines) -- Plan phase
├── ship.go             (631 lines) -- Ship phase
├── engine_verify.go    (531 lines) -- Verify helpers
├── runtime.go          (521 lines) -- Runtime phase
├── init_deep.go        (488 lines) -- Deep init analysis
├── discuss.go          (294 lines) -- Discuss phase
├── discuss_check.go    (293 lines) -- Discuss quality
├── ... (40+ more files)
```

### Proposed State

```
internal/workflow/
├── engine.go           (~600 lines) -- Core: constructor, RunPhase, Transition, getters/setters
├── streaming.go        (~400 lines) -- LLM streaming pipeline
├── context.go          (~500 lines) -- Context building (all build*Context methods)
├── pause.go            (~200 lines) -- PauseController component
├── checkpoint.go       (~150 lines) -- CheckpointManager component
├── decisions.go        (~100 lines) -- DecisionLogger component
├── execute.go          (1,152 lines) -- Execute phase (unchanged)
├── engine_parse.go     (726 lines) -- Parsing utilities (unchanged)
├── plan.go             (535 lines) -- Plan phase (unchanged)
├── ship.go             (631 lines) -- Ship phase (unchanged)
├── engine_verify.go    (531 lines) -- Verify helpers (unchanged)
├── runtime.go          (521 lines) -- Runtime phase (unchanged)
├── init_deep.go        (488 lines) -- Deep init analysis (unchanged)
├── discuss.go          (294 lines) -- Discuss phase (unchanged)
├── discuss_check.go    (293 lines) -- Discuss quality (unchanged)
├── ... (remaining files unchanged)
```

### Engine Struct Transformation

**Before (47 fields):**
```go
type Engine struct {
    // Identity (5)
    sessionID, workDir, backupDir, planningDir, sessionStartHash string
    // Provider (5)
    provider, modelID, modelIDMu, perPhaseModels, perPhaseModelsMu
    // Infrastructure (10)
    stateMachine, cache, cfg, git, dispatcher, tokens, sessionMgr, promptBuilder, logger, execCommand
    // State (8)
    state, discussState, workflowMode, workflowModeMu, websiteTemplateDir, toolCallsSinceLastCompact, done, cancel
    // Streaming (3)
    contextBuilder, callCounter, contextRegistry
    // Metrics (3)
    costTracker, collector, startTime
    // Code Intel (3)
    codeIntel, codeIntelMu, codeIntelBuilt
    // v1.5 (3)
    phaseCoordinator, compactor, ledger
    // Pause (6)
    pauseMu, pauseCh, resumeCh, skipTaskCh, cancelTaskCh, cancelGroupCh
    // UI (1)
    msgEmitter
}
```

**After (15 core fields + 3 embedded components):**
```go
type Engine struct {
    // Identity (3)
    sessionID, workDir, planningDir string
    // Provider (3)
    provider, modelID string  // modelIDMu removed (use providerAndModel pattern)
    // Infrastructure (8)
    stateMachine, cache, cfg, git, dispatcher, tokens, sessionMgr, promptBuilder
    // State (4)
    state, discussState, workflowMode, done
    // Streaming (1)
    contextBuilder
    // Metrics (2)
    costTracker, collector
    // v1.5 (2)
    phaseCoordinator, compactor
    // UI (1)
    msgEmitter

    // Embedded components (extracted from fields above)
    PauseController        // owns pauseMu, pauseCh, resumeCh, skipTaskCh, cancelTaskCh, cancelGroupCh
    CheckpointManager      // owns checkpoint save/load logic
    DecisionLogger         // owns decision log, flush, snapshot
}
```

### Key Decisions

1. **Use embedding, not composition** -- The extracted components are embedded in Engine, so all existing method calls (e.g., `engine.PauseExecution()`) continue to work without callers changing.

2. **Streaming stays in workflow package** -- The streaming pipeline is tightly coupled to the Engine (uses provider, tokens, contextBuilder). Extracting to a separate package would require passing many dependencies. Instead, extract to a separate file within the same package.

3. **Context building extracted** -- The 12 `build*Context` methods are pure functions over Engine state. They can be methods on a `ContextBuilder` component or remain as methods on a separate file.

4. **PauseController is the cleanest extraction** -- 6 channels + 6 mutex fields + 8 methods form a self-contained pause/resume state machine.

### Dependencies Affected

- No external package changes. All splits are within `internal/workflow`.
- The `tuitypes.WorkflowEngine` interface may need updating if method signatures change (unlikely with embedding).

### Tests Affected

- `engine_test.go` (1,065 lines) -- tests reference Engine methods directly
- `engine_extra_test.go` (3,409 lines) -- extensive engine tests
- `engine_wiring_test.go` (270 lines) -- wiring tests
- `engine_race_test.go` (92 lines) -- race condition tests
- All phase-specific test files

---

## Phase 3: TUI Decomposition

### SidebarModel Decomposition

**Before (47 fields):**
```
Token Metrics (15 fields) + Phase Pipeline (3) + Tool Timeline (2) + Task Progress (3)
+ Compaction (3, dead) + Sub-Agent (2) + File Watcher (1) + Git (3) + Layout (15) + Misc (1)
```

**After (~22 fields):**
```
Core SidebarModel: git (3) + layout (15) + mode (1) + misc (3) = ~22 fields
```

Extracted sub-components:
- `SidebarMetrics` -- token usage, burn rate, cost, execution speed (15 fields, 7 methods)
- `SidebarPhasePipeline` -- phase state machine (3 fields, 4 methods)
- `SidebarToolTimeline` -- tool call ring buffer (2 fields, 3 methods)
- `SidebarTaskTracker` -- TODO items and progress (3 fields, 9 methods)

### app_view.go Decomposition

Split the 1,266-line file into three files:
- `app_view.go` -- View(), renderFrame(), chrome building, modal rendering (~300 lines)
- `app_view_screens.go` -- 30+ renderXxxContent() methods (~600 lines)
- `app_view_helpers.go` -- Header/footer building, overlay utilities (~200 lines)

### AppState Cleanup

The AppState struct cannot be fully decomposed without breaking the Bubble Tea architecture (single model, single Update). However, the following cleanup is recommended:

1. **Extract message routing** -- The 112-case switch in `app_update.go` can be replaced with a `map[reflect.Type]handlerFunc` pattern, reducing the switch to a lookup.

2. **Extract screen renderers** -- Already partially done in `app_view.go`. Complete the extraction to `app_view_screens.go`.

3. **Extract workflow handlers** -- The 44 workflow methods in `app_handlers_workflow.go` are already well-grouped. No further splitting needed.

---

## Phase 4: Tools Package Refinement

### webfetch.go Decomposition

**Before (1,147 lines):**
- HTTP client + SSRF protection (~450 lines)
- HTML-to-Markdown parser (~700 lines)

**After:**
- `tools/webfetch.go` (~450 lines) -- HTTP client, SSRF, DNS caching, retry
- `tools/html2md.go` (~700 lines) -- All HTML conversion functions

### bash.go Security Extraction

Extract the 200+ lines of dangerous command detection, obfuscation checking, and variable expansion detection into `tools/bash_security.go`.

### edit.go Strategy Extraction

Extract the 5 match strategies (exact, trimmed, normalized, anchor, fuzzy) into `tools/edit_strategies.go`, keeping the Edit tool orchestration in `tools/edit.go`.

---

## Risk Assessment

| Phase | Risk Level | Mitigation |
|-------|-----------|------------|
| Phase 1: Config | **Low** | File splits only. No API changes. Tests split alongside. |
| Phase 2: Workflow | **Medium** | Engine method signatures preserved via embedding. Run full test suite after each extraction. |
| Phase 3: TUI | **Medium** | SidebarModel sub-components are internal. app_view.go splits are structural. |
| Phase 4: Tools | **Low** | File splits only. No API changes. |

### Highest Risk Areas

1. **Engine streaming extraction** -- The streaming pipeline uses `e.provider`, `e.tokens`, `e.contextBuilder`, `e.msgEmitter`, `e.callCounter`, `e.state.messages`, and `e.cache`. All must be accessible from the extracted file.

2. **SidebarModel metrics extraction** -- The render methods reference metrics fields directly. Each render method must be updated to use the sub-component.

3. **Config merge fix** -- Adding 49 missing fields must be done carefully to avoid breaking existing config files.

### Regression Prevention

- Run `make check` (fmt, tidy, vet, lint, test) after every file split
- Run `make test-fast` during development, `make test` (with race detector) before committing
- Verify all 488 Go files compile with `go build ./...`
- Run E2E tests (`e2e_test.go`) to verify binary behavior

---

## Estimated Refactor Size

| Phase | File Splits | Line Changes | New Code | Total |
|-------|-------------|-------------|----------|-------|
| Phase 1: Config | 6 new files | ~300 | ~200 (merge fields) | ~500 |
| Phase 2: Workflow | 4 new files | ~800 | ~100 (component wiring) | ~900 |
| Phase 3: TUI | 3 new files + 4 components | ~700 | ~50 (delegation) | ~750 |
| Phase 4: Tools | 3 new files | ~300 | 0 | ~300 |
| **Total** | **16 new files** | **~2,100** | **~350** | **~2,450** |

This represents approximately **1.4%** of the total codebase (175,358 lines).

---

## Expected Maintainability Improvements

| Metric | Before | After (Projected) |
|--------|--------|-------------------|
| Files > 1,000 lines | 6 | 1 (execute.go) |
| Files > 500 lines | 40 | ~25 |
| Engine fields | 47 | 15 core + 3 components |
| SidebarModel fields | 47 | 22 |
| AppState sub-models | 38 | 38 (unchanged, but view split) |
| Config merge coverage | 51% (49 fields missing) | 100% |
| Average file size | 359 lines | ~300 lines |
| Max file size | 1,707 lines | ~1,152 lines (execute.go) |
| Developer onboarding time | High (god objects) | Medium (focused files) |
| Regression risk per change | High (wide impact) | Medium (narrower scope) |

### Specific Improvements

1. **Config merge gap eliminated** -- 49 fields become overridable from project config
2. **Engine is understandable** -- A new developer can read `engine.go` (600 lines) and understand the core orchestration without wading through streaming, pause, and checkpoint code
3. **Sidebar is maintainable** -- Each sub-component (metrics, phases, timeline, tasks) can be modified independently
4. **HTML parser is reusable** -- Extracted `html2md.go` could eventually become a standalone utility
5. **Tests are more focused** -- Each extracted file has its own test file, reducing test file sizes
6. **Merge conflicts reduced** -- Smaller files mean fewer developers editing the same file simultaneously

---

## Appendix: File Line Count Summary

### Before Decomposition

| Range | Count | Files |
|-------|-------|-------|
| > 1,700 | 1 | engine.go |
| 1,000 - 1,700 | 5 | sidebar_model.go, app_view.go, loader.go, execute.go, webfetch.go, firstrun_view.go |
| 500 - 1,000 | 34 | settings_model.go, parser.go, git.go, edit.go, etc. |
| 200 - 500 | ~120 | Various |
| < 200 | ~325 | Various |

### After Decomposition (Projected)

| Range | Count | Change |
|-------|-------|--------|
| > 1,700 | 0 | -1 |
| 1,000 - 1,700 | 1 | -5 (execute.go remains) |
| 500 - 1,000 | ~25 | -9 |
| 200 - 500 | ~130 | +10 |
| < 200 | ~340 | +15 |

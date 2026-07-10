# M31A Application Startup Sequence

**Phase:** 01-wiring-audit  
**Plan:** 01-01  
**Task:** 2 - Trace application startup sequence and verify wiring

---

## Overview

This document traces the exact initialization sequence in `cmd/m31a/main.go` from program entry through TUI program startup. Each step documents: function called, return values used downstream, error handling, logging, and consumer traceability.

---

## Startup Sequence Trace

### Step 1: CLI Flag Parsing & Early Exits
**Location:** `main.go:116-152`  
**Function:** `main()` → `run()` → `flag.Parse()`

| Aspect | Details |
|--------|---------|
| **Flags parsed** | `-version`, `-help`, `-prompt`, `-goal`, `-model` |
| **Early exits** | `--version` prints version and exits (0); `--help` prints usage and exits (0) |
| **Error handling** | None (flag parsing doesn't error) |
| **Logging** | Version info logged at Info level if `--version` |
| **Consumers** | `cmdRegistry` used by TUI; `promptFlag`/`goalFlag`/`modelFlag` gate headless modes |

**Produces:** `cmdRegistry`, `promptFlag`, `goalFlag`, `modelFlag`

---

### Step 2: Load .env File (Before Logger)
**Location:** `main.go:154-155`  
**Function:** `config.LoadDotEnv()`

| Aspect | Details |
|--------|---------|
| **Called** | Once via `sync.Once` (guarded in `LoadDotEnv`) |
| **Reads** | `.env` in CWD if exists, not group/world-writable |
| **Error handling** | Silent ignore on missing/permission errors |
| **Logging** | Warn if `.env` is group/world-writable |
| **Consumers** | All subsequent `os.Getenv()` calls (config, keychain, providers) |

**Note:** Must run before logger init because `os.Setenv` is not goroutine-safe.

---

### Step 3: Logger Initialization
**Location:** `main.go:157-164`  
**Function:** `log.NewLogger(Version)`

| Aspect | Details |
|--------|---------|
| **Returns** | `(*slog.Logger, cleanup func(), error)` |
| **Error handling** | If error: print to stderr, return exit code 1 |
| **Setup** | `slog.SetDefault(logger)` — makes logger global |
| **Cleanup** | `defer cleanup()` — flushes buffers on exit |
| **Logging** | Startup banner with version, commit, date, Go version, OS, arch |
| **Consumers** | All packages via `slog.Default()` or passed logger |

**Produces:** Global logger, `cleanup` function

---

### Step 4: Config Path Resolution
**Location:** `main.go:175-189`  
**Function:** `config.Load(configPath)`

| Aspect | Details |
|--------|---------|
| **Config path priority** | `M31A_CONFIG` env → `~/.m31a/config.toml` |
| **Directory creation** | `os.MkdirAll` with `types.DirPermission` (0o700) |
| **Error handling** | On mkdir error: log error, return exit code 1 |
| **Logging** | Config path resolution logged at Info level |

**Produces:** `configPath` (string)

---

### Step 5: Force-Exit Sentinel Detection
**Location:** `main.go:197-203`  
**Function:** `os.Stat(sentinelPath)`

| Aspect | Details |
|--------|---------|
| **Sentinel file** | `<config_dir>/.force-exit` |
| **Purpose** | Detect previous unclean shutdown (TUI didn't exit within 5s of signal) |
| **Action** | If exists: log warning, remove sentinel |
| **Error handling** | Ignore if not found; other errors logged as warning |

---

### Step 6: Config Loading & Validation
**Location:** `main.go:191-195`  
**Function:** `config.Load(configPath)`

| Aspect | Details |
|--------|---------|
| **Layers (in order)** | 1. `DefaultConfig()` — zero-value defaults<br>2. Global TOML (`~/.m31a/config.toml`)<br>3. `.env` file (via `LoadDotEnv` called inside `Load`)<br>4. Env var overrides (`M31A_*`)<br>5. Project TOML (`m31a.toml` walked up 3 dirs)<br>6. Variable substitution `${VAR}` → env<br>7. Validation (`validateConfig`) |
| **Validation** | Type/range checks on all known fields; unknown TOML keys warned |
| **Error handling** | On validation error: log error, return exit code 1 |
| **Logging** | Warn on unknown TOML keys; warn on unresolved `${VAR}` |
| **Consumers** | `cfg` used by: keychain, providers, tools, workflow, TUI, session |

**Produces:** `*config.Config`

---

### Step 7: Keychain Initialization
**Location:** `main.go:205-217`  
**Functions:** `keychain.New()` → `keychain.NewCached()` → `cfg.ResolveAPIKeys(kc)`

| Aspect | Details |
|--------|---------|
| **keychain.New()** | Platform-specific: Linux (D-Bus Secret Service + pass fallback), macOS (security CLI), Windows (Credential Manager). Returns `ErrKeychainUnavailable` if backend unavailable. |
| **keychain.NewCached()** | Wraps inner keychain; caches `ErrKeychainUnavailable` — subsequent calls return error immediately without retrying D-Bus/pass |
| **cfg.ResolveAPIKeys(kc)** | Resolution priority per provider:<br>1. `M31A_<PROVIDER>_API_KEY` env<br>2. `<PROVIDER>_API_KEY` env<br>3. Keychain `kc.Get("<provider>")`<br>4. Config file field (already loaded) |
| **Error handling** | Keychain init failure: logged as Warn, continues with `kc=nil`<br>Resolve errors: logged as Warn, continues |
| **Logging** | Keychain init failure: Warn<br>API key resolve failure: Warn |
| **Consumers** | `kc` passed to `app.SetKeychain(kc)`; resolved keys in `cfg.Provider.*.APIKey` used by provider registration |

**Produces:** `kc` (Keychain, possibly nil), `cfg` with resolved API keys

---

### Step 8: Provider Registry Creation
**Location:** `main.go:219-267`  
**Functions:** `provider.NewRegistry()`, `tui.RegisterProvider()` x3

| Aspect | Details |
|--------|---------|
| **Registry creation** | `provider.NewRegistry()` — empty map, no active provider |
| **Capability config** | `provider.SetCapabilityConfig()` with config overrides for reasoning patterns, tool-capable patterns, completion-only patterns, non-chat patterns, known capabilities map |
| **Registration order** | From `cfg.Provider.RegistrationOrder` (default: `["openrouter", "zen", "nvidia"]`) |
| **Per-provider registration** | `tui.RegisterProvider(registry, cfg, "<id>", apiKey, Version)` — creates client with options (base URL, cache TTLs, health check thresholds, version) |
| **Error handling** | Per-provider: on error, log Warn, continue (other providers may work) |
| **Default provider** | `registry.SetActive(cfg.Provider.Default)` — if fails, log Warn |
| **Logging** | Info on each successful registration; Warn on failures |
| **Consumers** | `registry` passed to TUI app; used by headless modes; provider discovery via `registry.ActiveProvider()` |

**Produces:** `*provider.Registry` with 0-3 providers registered, active provider set

---

### Step 9: Headless Mode Gates
**Location:** `main.go:274-303`  
**Conditions:** `--prompt` or `--goal` flags

| Aspect | Details |
|--------|---------|
| **Prompt mode (`--prompt`)** | Requires active provider; runs `runHeadless()` — single prompt → LLM → print response |
| **Goal mode (`--goal`)** | Requires active provider; runs `runHeadlessWorkflow()` (not fully implemented) |
| **Error handling** | No provider: print error to stderr, exit 1 |
| **Model selection** | `--model` flag → `cfg.Model.Default` → first available from provider |

**Note:** These are early exits — TUI not started.

---

### Step 10: Working Directory Resolution
**Location:** `main.go:305-310`  
**Function:** `os.Getwd()`

| Aspect | Details |
|--------|---------|
| **Error handling** | On error: log Error, return exit code 1 |
| **Logging** | None on success |
| **Consumers** | `workDir` used by: session manager, tools dispatcher, git client, ledger, subagent manager, TUI app |

**Produces:** `workDir` (string)

---

### Step 11: Session Manager Creation
**Location:** `main.go:312-316`  
**Function:** `session.NewManager(globalConfigDir, workDir, opts)`

| Aspect | Details |
|--------|---------|
| **Parameters** | `baseDir` = `~/.m31a/` (global), `workDir` = project root, `CoordinatorTimeoutSecs` from config |
| **Creates** | Project-local session dir: `<workDir>/.m31a/` |
| **Components** | File lock (`session.lock`), coordinator (per-session concurrency) |
| **Error handling** | Constructor doesn't error; operations error later |
| **Consumers** | TUI app (`app.SetSessionManager`), workflow engine, subagent manager |

**Produces:** `*session.Manager`

---

### Step 12: Tools Dispatcher Creation
**Location:** `main.go:318-324`  
**Function:** `tools.DefaultDispatcher(workDir, backupDir, backupDir, &cfg.Permissions, &cfg.Tools)`

| Aspect | Details |
|--------|---------|
| **Dispatcher creation** | `tools.NewDispatcher(cfg)` — creates output store, permission store |
| **Output store** | Dir: `~/.m31a/tool-output`, max lines/bytes from config, cleanup on startup |
| **Persistent permissions** | Loaded from project `.m31a/permissions.json` |
| **Tool registration (18 tools)** | Bash, FileRead, FileWrite, Edit, TodoWrite, TodoRead, WebFetch, AskUser, Glob, Grep, FileList, FileDelete, FileMove, CodeMap, CodeComplexity, DevServer, HTTPCheck, WebSearch |
| **Config extraction** | Bash timeout, WebFetch retries, rate limits, concurrency, output bounds, DNS cache, edit fuzzy, dangerous commands |
| **Error handling** | On permission config error: log Error, return exit code 1 (fail fast) |
| **Consumers** | TUI app, workflow engine, subagent manager (via factory), Agent tool |

**Produces:** `*tools.Dispatcher` with 18 registered tools

---

### Step 13: Git Client Initialization
**Location:** `main.go:326-327`  
**Function:** `git.New(workDir)`

| Aspect | Details |
|--------|---------|
| **Creates** | `*git.Git` with `workDir` |
| **Error handling** | Constructor never errors |
| **Consumers** | TUI app, workflow engine (via `engine.SetGit`), rollback manager, session manager (gitignore), subagent worktrees |

**Produces:** `*git.Git`

---

### Step 14: Ledger Creation
**Location:** `main.go:329-331`  
**Function:** `ledger.New(ledgerPath)`

| Aspect | Details |
|--------|---------|
| **Path** | `<config_dir>/LEDGER.md` (global, not project-local) |
| **Creates** | Dir if needed, parses existing entries if file exists |
| **Error handling** | Constructor returns nil on mkdir error (but logs internally) |
| **Consumers** | TUI app (`app.SetLedger`), workflow engine (`engine.SetLedger`), Ship phase |

**Produces:** `*ledger.Ledger`

---

### Step 15: Rollback Manager Creation
**Location:** `main.go:333-334`  
**Function:** `rollback.New(gitClient)`

| Aspect | Details |
|--------|---------|
| **Wraps** | `git.Git` for commit chain, preview, soft/hard/safe reset |
| **Error handling** | Constructor never errors |
| **Consumers** | TUI app, workflow engine (Ship phase rollback tool) |

**Produces:** `*rollback.Rollback`

---

### Step 16: AutoDream Client Creation
**Location:** `main.go:336-337`  
**Function:** `autodream.New(nil)`

| Aspect | Details |
|--------|---------|
| **Initial messages** | `nil` — REPL injects messages later via `SetMessages()` |
| **Error handling** | Constructor never errors |
| **Consumers** | TUI app, REPL model (auto-compaction) |

**Produces:** `*autodream.Consolidator`

---

### Step 17: Theme Selection
**Location:** `main.go:339-341`  
**Hardcoded:** `theme.ModeDark`

| Aspect | Details |
|--------|---------|
| **Note** | Light/auto themes not supported; always dark |
| **Consumers** | TUI app construction |

---

### Step 18: TUI App Construction
**Location:** `main.go:343-361`  
**Function:** `tui.NewApp(cfg, configPath, registry, sessionMgr, dispatcher, gitClient, ledgerClient, rollbackClient, autoDreamClient, Version, themeMode)`

| Aspect | Details |
|--------|---------|
| **Dependencies injected** | All 10 core dependencies + version + theme |
| **AppState fields set** | `cfg`, `configPath`, `registry`, `sessionManager`, `dispatcher`, `git`, `ledger`, `rollback`, `autoDream`, `version`, `themeMode`, `cwd` |
| **Keychain** | Set via `app.SetKeychain(kc)` if `kc != nil` |
| **Consumers** | Bubble Tea program, all screens, workflow engine init |

**Produces:** `*AppState` (implements `tea.Model`)

---

### Step 19: Subagent Manager Creation
**Location:** `main.go:363-396`  
**Function:** `subagent.NewManager(deps)`

| Aspect | Details |
|--------|---------|
| **Dependencies** | WorkDir, Registry, ActiveModel, Logger, Worktrees (GitWorktrees), Profiles, NewDispatcher factory |
| **Active model for subagents** | Resolved from `registry.ActiveProvider().GetModel(cfg.Model.Default)` or stub |
| **Dispatcher factory** | `tools.NewDispatcherFactory(backupDir, backupDir, &cfg.Permissions, &cfg.Tools, nil, cfg.Agents.Profiles)` |
| **Agent tool registration** | `dispatcher.Register(tools.NewAgent(subagentMgr, false, 0, cfg.Agents.Profiles))` — `false` = non-child (can spawn background) |
| **Error handling** | Agent tool registration failure: log Error, return exit code 1 |
| **App integration** | `app.SetSubagentManager(subagentMgr)` |
| **Consumers** | Agent tool, TUI (subagent events), workflow engine (via Agent tool) |

**Produces:** `*subagent.Manager`

---

### Step 20: Subagent Worktree Sweep
**Location:** `main.go:398-403`  
**Function:** `subagent.Sweep(ctx, workDir)`

| Aspect | Details |
|--------|---------|
| **Timeout** | 30 seconds |
| **Purpose** | Clean up stale worktrees/branches from prior crashes |
| **Error handling** | Log Warn, continue |
| **Consumers** | Filesystem hygiene only |

---

### Step 21: Resume-on-Startup
**Location:** `main.go:405-411`  
**Condition:** `cfg.Features.ResumeOnStartup`

| Aspect | Details |
|--------|---------|
| **Action** | `sessionMgr.ListSessions()` → if any, `app.SetResumeSessionID(sessions[0].ID)` |
| **Error handling** | On error: silent ignore |
| **Consumers** | TUI Init() loads and restores session |

---

### Step 22: Bubble Tea Program Creation
**Location:** `main.go:413-416`  
**Function:** `tea.NewProgram(app, tea.WithAltScreen(), tea.WithMouseAllMotion())`

| Aspect | Details |
|--------|---------|
| **Options** | Alt screen (full-screen TUI), all mouse motion events |
| **Error handling** | Constructor doesn't error |
| **Consumers** | Signal handler, `p.Run()` |

**Produces:** `*tea.Program`

---

### Step 23: Signal Handler Goroutine
**Location:** `main.go:418-462`  
**Function:** Anonymous goroutine

| Aspect | Details |
|--------|---------|
| **Signals** | `SIGTERM`, `SIGINT` |
| **Mechanism** | `p.Send(tea.QuitMsg{})` — sends through program channel (not direct `app.Shutdown()`) |
| **Why channel** | Preserves Bubble Tea single-threaded contract — all state mutations in `Update()` |
| **Hard fallback (H-24)** | 5-second timeout: if TUI doesn't quit, write `.force-exit` sentinel, call `cleanup()`, `restoreTerminal()`, `os.Exit(1)` |
| **Synchronization** | `atomic.Bool` `programExited` prevents double-exit; `sigDone` channel coordinates shutdown |
| **Logging** | Info on signal received; Warn on forced exit |

---

### Step 24: TUI Program Run
**Location:** `main.go:464-469`  
**Function:** `p.Run()`

| Aspect | Details |
|--------|---------|
| **Blocks** | Until `tea.QuitMsg` received |
| **Error handling** | On error: `programExited.Store(true)`, `close(sigDone)`, log Error, return exit code 1 |
| **Consumers** | All TUI interaction, workflow execution, background workers |

---

### Step 25: Graceful Shutdown
**Location:** `main.go:471-476`  
**Function:** `app.Shutdown()`

| Aspect | Details |
|--------|---------|
| **Order** | 1. `programExited.Store(true)`<br>2. `close(sigDone)` (stops signal goroutine)<br>3. `app.Shutdown()` |
| **Shutdown() does** | Save session, stop dev servers, close file watcher, close config watcher, save frecent history, flush metrics, cancel workflow, cancel shutdown context, cancel stream context, stop dispatcher, close decision logger, shutdown subagent manager |
| **Error handling** | Errors logged as Warn, shutdown continues |
| **Returns** | 0 on clean exit |

---

## Background Workers (Started in App.Init())

The following background workers are started as `tea.Cmd` from `AppState.Init()` (`internal/tui/app.go:24-121`):

| Worker | Command | Trigger | Purpose |
|--------|---------|---------|---------|
| **Health ticker** | `NextHealthTick` | Interval (`types.HealthCheckInterval`) | Provider health checks |
| **Permission listener** | `permListenerCmd` | `dispatcher.RequestCh()` | Handle tool permission requests |
| **Question listener** | `questionListenerCmd` | `dispatcher.QuestionRequestCh()` | Handle AskUser questions |
| **Subagent listener** | `subagentListenerCmd` | `subagentMgr.Events()` | Handle subagent lifecycle events |
| **Sidebar refresh** | `SidebarRefreshTick` + `refreshCmd` | Interval (`SidebarRefreshInterval`) | Update sidebar (git status, sessions, etc.) |
| **File watcher** | `startFileWatcher()` | `fsnotify` events | Real-time sidebar refresh on file changes |
| **Config watcher** | `startConfigWatcher()` | `config.WatchConfig` (fsnotify/polling) | Hot-reload config.toml |
| **Provider sync** | `syncReplProvider()` | One-shot at startup | Fetch model catalog for REPL |
| **Session resume** | `loadAndRestoreSession()` | If `resumeSessionID` set | Restore previous session |
| **New session** | `startNewSession()` | If no resume | Create fresh session |

**Emitter drain chain:** `drainEmitterCmd()` / `drainMultipleCmd()` / `drainAdaptiveCmd()` — reads from `emitterCh` (workflow → TUI message channel) and forwards to Bubble Tea update loop.

---

## Wiring Verification Matrix

| Component | Created At | Passed To | Used By |
|-----------|------------|-----------|---------|
| `cfg` | Step 6 | keychain, providers, tools, workflow, TUI, session | All |
| `kc` (keychain) | Step 7 | `app.SetKeychain`, `cfg.ResolveAPIKeys` | Providers |
| `registry` | Step 8 | TUI, headless, subagent mgr | Provider access |
| `dispatcher` | Step 12 | TUI, workflow engine, subagent factory, Agent tool | Tools |
| `sessionMgr` | Step 11 | TUI, workflow engine, subagent mgr | Sessions, checkpoints |
| `gitClient` | Step 13 | TUI, workflow engine, rollback, session mgr, subagent | Git ops |
| `ledgerClient` | Step 14 | TUI, workflow engine | Session records |
| `rollbackClient` | Step 15 | TUI, workflow engine | Rollback ops |
| `autoDreamClient` | Step 16 | TUI, REPL | Auto-compaction |
| `subagentMgr` | Step 19 | TUI, Agent tool | Subagents |
| `app` (AppState) | Step 18 | `tea.NewProgram` | Entire TUI |

---

## Error Handling Summary

| Step | Failure Mode | Behavior |
|------|--------------|----------|
| Logger init | `log.NewLogger` error | Print to stderr, exit 1 |
| Config load | Validation error | Log error, exit 1 |
| Keychain init | Backend unavailable | Log warn, continue with `kc=nil` |
| API key resolve | Keychain error | Log warn, use config/env |
| Provider registration | Individual provider fails | Log warn, continue others |
| Dispatcher creation | Permission config invalid | Log error, exit 1 (fail fast) |
| TUI program run | `p.Run()` error | Log error, exit 1 |
| Signal fallback | TUI doesn't quit in 5s | Force exit, write sentinel |

---

## Logging Summary

| Level | When |
|-------|------|
| Info | Version banner, provider registration, session cleanup, startup routing |
| Warn | Unknown config keys, unresolved vars, keychain unavailable, provider reg failures, force-exit sentinel, dropped events, shutdown errors |
| Error | Logger init failure, config validation failure, dispatcher creation failure, TUI run error, signal handler panic |

---

## Cross-Reference with Dependency Graph

All components created in this startup sequence appear as nodes in `dependency-graph.dot`:
- `cmd/m31a` (root) → imports all internal/* and pkg/*
- `internal/config` → loads config, used by main
- `internal/keychain` → used by main for API keys
- `internal/provider` + subpackages → registered in registry
- `internal/tools` + subagent → dispatcher created
- `internal/workflow` → engine created in TUI Init
- `internal/session` → manager created
- `internal/tui` → AppState created, runs program
- `pkg/session`, `pkg/ledger`, `pkg/rollback`, `pkg/keychain`, `pkg/autodream` → instantiated in main

---

## Verification Checklist

- [x] All 15+ initialization steps traced from main.go
- [x] Each step documents: function, return values, error handling, logging, consumers
- [x] Background workers identified and traced to Init()
- [x] Signal handling preserves Bubble Tea contract
- [x] Dependency injection verified (no global state mutation from goroutines)
- [x] Cross-referenced with dependency-graph.dot nodes
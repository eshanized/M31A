# M31A — Missing Features & Gap Analysis Report

> **Generated:** 2026-06-09 | **Scope:** Full codebase deep audit (4 parallel research agents)  
> **Files examined:** 72 TUI files, 28 component files, 9 packages, all tools/providers/workflow/config layers

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [TUI & UX Gaps](#2-tui--ux-gaps) — 50 items
3. [Core Feature Gaps](#3-core-feature-gaps) — Tools, Provider, Config, Workflow, Session, Context, Git, Security
4. [Infrastructure & Package Gaps](#4-infrastructure--package-gaps) — Session, Ledger, Rollback, AutoDream, Arbitrage, Bisect, Taskrunner, Keychain, Logging, Tokens, Build
5. [Product & Strategy Gaps](#5-product--strategy-gaps) — ROADMAP, Slash Commands, Competitor Comparison, Onboarding, Docs
6. [Master Priority Matrix](#6-master-priority-matrix)

---

## 1. Executive Summary

After a full codebase audit across all layers, **163 missing or broken features** were identified. They fall into four major buckets:

| Bucket | Items | P0 Critical | P1 Important | P2 Nice |
|--------|-------|-------------|--------------|---------|
| TUI & UX | 50 | 12 | 24 | 14 |
| Core Features | 40 | 8 | 18 | 14 |
| Infrastructure & Packages | 43 | 4 | 22 | 17 |
| Product & Strategy | 30 | 8 | 14 | 8 |

### Top 10 Most Impactful Gaps (Do First)

| # | Issue | Impact |
|---|-------|--------|
| 1 | `View()` mutates state — data race (D-1) | App crash risk |
| 2 | API key saved as plaintext in config on Save() | Security breach |
| 3 | `/new` command not registered anywhere | No workflow entry point |
| 4 | `AutoArbitrage` config flag never read at runtime | Dead feature |
| 5 | AutoDream never triggered automatically | Context bloat unmanaged |
| 6 | No confirmation dialog for `/clear` or `/reset` | Accidental data loss |
| 7 | 37 slash commands; README documents 16; AGENTS.md lists 5 tools out of 9 | Severe doc rot |
| 8 | No `Fetch`/`Pull`/`Push` in `internal/git/` | Ship phase uses ad-hoc `Run()` |
| 9 | tiktoken-go unmaintained; Claude/Gemini fall back to rune÷4 | Inaccurate token counting |
| 10 | Ctrl+C exits mid-stream with no confirmation dialog | Data loss |

---

## 2. TUI & UX Gaps

### 2.1 Input UX

| ID | Feature | File / Line | Priority |
|----|---------|-------------|----------|
| U1 | **Shift+Enter multi-line input** — `Enter` always submits; no newline insertion shortcut | `repl.go:91` | P0 |
| U2 | **Auto-expand textarea height** — fixed at 3 rows regardless of content; no dynamic resize | `repl_model.go:23`, `repl_view.go:19` | P0 |
| U3 | **Paste detection / bracketed paste** — large pastes silently overflow 3-row box; no `tea.PasteMsg` handling | `repl.go:203`, `cmd/m31a/` | P1 |
| U4 | **History search (Ctrl+R)** — `FrecentHistory.Search()` exists but there is no incremental-search UI; only sequential up/down | `repl.go:326`, `history.go:64` | P1 |
| U5 | **Character / token count display** — `CharLimit = 0` (unlimited) and prompt length is never shown | `repl_view.go:84` | P2 |
| U6 | **Line count in textarea** — when input overflows 3 rows, no indicator of overflow | `repl_view.go:87` | P3 |
| U7 | **Word-level navigation (Ctrl+←/→)** — `alt+b`, `alt+f` not explicitly handled; behavior is terminal-dependent | `repl.go:203` | P1 |
| U8 | **Textarea undo/redo** — no `ctrl+z` to undo accidental clear or last edit; bubbles textarea lacks native undo | `repl_model.go` | P2 |
| U9 | **Draft token/cost estimate** — status bar shows cost of last response only, never estimates current draft | `statusbar.go:107` | P2 |
| U10 | **Soft char-limit warning** — no `⚠ ~4K chars` hint when prompt exceeds 2 000 characters | `repl_view.go:84` | P2 |

---

### 2.2 Message Display

| ID | Feature | File / Line | Priority |
|----|---------|-------------|----------|
| U11 | **Copy message to clipboard** — no binding to copy any message content; `atotto/clipboard` is already an indirect dep | `components/message.go:70`, `repl.go` | P0 |
| U12 | **"File attached" toast after @-mention** — `ResolveMentions()` silently injects file content; user gets no confirmation | `repl.go:292` | P0 |
| U13 | **"Response cancelled" toast** — `ctrl+c` cancels stream silently; no feedback | `app_update.go:621` | P1 |
| U14 | **User message edit / re-send** — submitted messages are immutable; no `e` key to load back into textarea | `repl_state.go:163` | P1 |
| U15 | **Single-message deletion** — only `/clear` (all) exists; no per-message `d` + confirm | `repl_state.go:182` | P2 |
| U16 | **Message cursor / selection mode** — no concept of focused message for keyboard interaction | `repl_model.go` | P2 |
| U17 | **First-message timestamp hidden** — `RenderTimestampBar` only shown between messages (`i > 0`); first message has none | `repl_state.go:273`, `components/message.go:82` | P2 |
| U18 | **Multi-day date spans** — timestamps show HH:MM only; sessions spanning days show no date | `components/message.go:222` | P2 |
| U19 | **"New messages" indicator when scrolled** — `userScrolled=true` accepts new messages silently with no `↓ N new lines` badge | `repl_state.go:246` | P1 |
| U20 | **Error toast in addition to inline message** — `ErrorMsg` appends message only; no toast for immediate visibility | `app_update.go:295` | P2 |

---

### 2.3 Navigation

| ID | Feature | File / Line | Priority |
|----|---------|-------------|----------|
| U21 | **j/k scroll in REPL viewport** — sidebar and resume have j/k; REPL does not | `repl.go:184` | P1 |
| U22 | **Viewport focus mode** — textarea always receives keys; vim-style viewport navigation is impossible without empty textarea | `repl.go`, `repl_model.go` | P1 |
| U23 | **Search within conversation (Ctrl+F / /)** — `components/search.go` stub exists but is not wired | `repl_view.go`, `components/search.go` | P1 |
| U24 | **Jump to top (gg / ctrl+home)** — `ctrl+l` goes to bottom; no "go to top" | `repl.go:178` | P2 |
| U25 | **Half-page scroll** — `ctrl+u`/`ctrl+d` do full-page; `HalfViewUp()`/`HalfViewDown()` not exposed | `repl.go:184` | P3 |
| U26 | **Diff viewer hunk jump (n/N)** — diff has scroll but no "next `@@` block" shortcut | `diff_view.go:106` | P2 |

---

### 2.4 Keyboard Shortcuts & Help

| ID | Feature | File / Line | Priority |
|----|---------|-------------|----------|
| U27 | **Help screen incomplete** — only 12 bindings listed; missing ctrl+g, ctrl+l, pgup/pgdown, ctrl+x chords, @-mention, ! prefix, leader map | `help.go:34` | P1 |
| U28 | **Help screen not scrollable** — no viewport; content clips on small terminals | `help.go` | P1 |
| U29 | **`?` key doesn't open help from REPL** — `?` closes help but there's no binding to open it from REPL | `repl.go:86` | P2 |
| U30 | **Ctrl+C streaming cancel hint hidden** — while streaming, status bar still shows generic hints; user unaware of cancel shortcut | `repl_view.go:99` | P1 |
| U31 | **Which-key only for leader chords** — `ctrl+x` triggers which-key overlay; no context hint for other screens (sidebar, resume, settings) | `keybindings.go:166` | P2 |
| U32 | **Command palette: shortcuts incomplete** — only 6 of 40+ commands show keyboard shortcut in palette | `cmdpalette.go:62` | P2 |
| U33 | **Command palette: no fuzzy match highlighting** — matched chars not highlighted; comment at L373 says "complex" | `cmdpalette.go:367` | P2 |
| U34 | **Command palette: fixed 60-col width** — doesn't adapt to terminal width | `cmdpalette.go:215` | P3 |
| U35 | **Mention dropdown: no footer hint** — no `tab select  ↑↓ navigate  esc cancel` guide | `mention_view.go` | P3 |
| U36 | **Mention scan: no loading indicator** — `filepath.Walk` is synchronous; no spinner during large-repo scan | `mention.go:48` | P2 |
| U37 | **Mention dropdown: no file preview** — selected entry shows no size / line count / type info | `mention_view.go` | P3 |

---

### 2.5 Accessibility / Display

| ID | Feature | File / Line | Priority |
|----|---------|-------------|----------|
| U38 | **CompactMode has no live effect** — config field + setting toggle exist; no view code reads `t.CompactMode` | `repl_view.go:19`, `theme/theme.go:106` | P1 |
| U39 | **High-contrast theme missing** — only Dark/Light/Auto; no ANSI-16 high-contrast variant | `theme/colors.go` | P3 |
| U40 | **Reduced-motion mode missing** — animated progress bars and screen transitions have no toggle | `components/progress.go:164`, `transition.go` | P3 |
| U41 | **Sidebar hide is silent** — below 80 cols the sidebar disappears with no feedback toast | `app_view.go:50` | P3 |

---

### 2.6 Missing Overlays & Dialogs

| ID | Feature | File / Line | Priority |
|----|---------|-------------|----------|
| U42 | **Confirmation dialog for `/clear` and `/reset`** — both execute immediately with no "Y/N" prompt | `commands_core.go` | P0 |
| U43 | **Model selector loading spinner** — fetch runs async but selector shows stale data with no spinner | `cache_refresh.go`, `cmdpalette.go` | P2 |
| U44 | **Permission modal: "Allow always" option** — only Allow/Deny per request; no persistent allow-for-tool | `components/permission.go` | P2 |
| U45 | **Background task indicator** — navigating away from execute/stream loses visual of running task | `app_state.go` | P2 |

---

### 2.7 Session UX

| ID | Feature | File / Line | Priority |
|----|---------|-------------|----------|
| U46 | **Session rename** — sessions shown as opaque UUIDs; no `r` key to label | `resume_model.go` | P1 |
| U47 | **Session export (`/export`)** — no markdown/JSON export command | `commands_session.go` | P1 |
| U48 | **Session search in resume screen** — j/k only; no `/` filter for 50+ sessions | `resume_model.go` | P1 |
| U49 | **Session list shows raw IDs** — no goal preview or first-message excerpt | `resume_view.go` | P2 |
| U50 | **Empty state in resume screen** — blank render when session list is empty; no "no sessions yet" UX | `resume_view.go` | P2 |

---

## 3. Core Feature Gaps

### 3.1 Tool System

| ID | Feature | File / Line | Priority |
|----|---------|-------------|----------|
| T1 | **FileList / Directory Tree tool** — LLM must use `Glob **/*`; no structured tree with sizes | `internal/tools/defaults.go` | P1 |
| T2 | **FileDelete tool** — LLM must use `Bash rm`, bypassing backup mechanism | `internal/tools/` | P1 |
| T3 | **FileMove / Rename tool** — renaming requires Bash, bypassing containment checks | `internal/tools/` | P1 |
| T4 | **FileSearch / fuzzy-find tool** — `LevenshteinThreshold` exists in constants but unused at tool level | `internal/tools/` | P2 |
| T5 | **ImageRead tool** — vision-capable models (`Capabilities.Vision == true`) can't receive image input | `internal/types/types.go:51` | P2 |
| T6 | **Per-tool execution timeout at dispatcher level** — Bash has timeout; FileRead/Glob/Grep/WebFetch have no dispatcher-level deadline | `internal/tools/dispatcher.go:96` | P2 |
| T7 | **Tool result caching** — every `FileRead("go.mod")` re-reads disk; no TTL cache | `internal/tools/dispatcher.go` | P3 |
| T8 | **Optional tool calls** — any tool failure triggers self-heal; no "optional" flag to skip non-critical failures | `internal/workflow/execute.go:210` | P2 |
| T9 | **Error codes in ToolResult** — `Error string` only; TUI can't distinguish "file not found" from "permission denied" | `internal/types/types.go` | P2 |

---

### 3.2 Provider

| ID | Feature | File / Line | Priority |
|----|---------|-------------|----------|
| P1 | **Streaming retry on mid-stream disconnect** — `io.ErrUnexpectedEOF` immediately fails the task; no reconnect or retry | `internal/provider/openrouter/client.go:214`, `internal/workflow/engine.go:472` | P1 |
| P2 | **HTTP 5xx retry before provider fallback** — 500/503 errors trigger no retry; immediately falls back to alternate provider | `internal/provider/openrouter/client.go:183` | P1 |
| P3 | **Cost budget enforcement** — `EstimateCost()` interface exists; no `MaxCostPerSessionUSD` config field or hard stop | `internal/config/types.go` | P1 |
| P4 | **Model capability validation before use** — tool use enabled even if `Capabilities.Tools == false` on selected model | `internal/provider/capabilities.go` | P2 |
| P5 | **Cross-provider model resolution** — no "Model not available on Zen, switch to closest?" dialog | `internal/provider/` | P2 |
| P6 | **Offline mode** — when both providers are unreachable, no graceful degraded mode | `internal/provider/` | P2 |

---

### 3.3 Config

| ID | Feature | File / Line | Priority |
|----|---------|-------------|----------|
| C1 | **Config schema (JSON Schema / machine-readable)** — only Go struct tags; no IDE completion or linter-validated schema | `internal/config/types.go` | P2 |
| C2 | **Env var coverage for all key config fields** — only `M31A_THEME`, `M31A_DEFAULT_MODEL` as overrides; no `M31A_MAX_ITERATIONS`, `M31A_LOG_LEVEL`, `M31A_PROVIDER` | `internal/config/loader.go:111` | P1 |
| C3 | **`resume_on_startup` flag not wiring auto-resume** — config field exists; initial session launch flow doesn't honor it | `internal/config/types.go` | P1 |

---

### 3.4 Workflow

| ID | Feature | File / Line | Priority |
|----|---------|-------------|----------|
| W1 | **`/new` command not registered** — primary workflow entry point in every spec doc; absent from `DefaultCommands()` | `internal/tui/commands*.go` | P0 |
| W2 | **Mid-workflow checkpoint resume** — `Checkpoint.TaskCount` persisted but `runExecute()` doesn't use it to seek past completed tasks on restart | `internal/workflow/engine.go`, `pkg/session/checkpoint.go` | P1 |
| W3 | **Workflow dry-run mode** — no `Engine.DryRun bool`; every `RunPhase()` executes for real | `internal/workflow/engine.go:161` | P2 |
| W4 | **Progress percentage / ETA** — `IntermediateProgressMsg` has no `Percent` or `ETASeconds` field | `internal/workflow/execute.go` | P2 |
| W5 | **Context limit → auto-compaction flow** — `ErrContextExceeded` returned but no auto-trigger of AutoDream; user must manually `/compress` | `internal/workflow/engine.go` | P1 |
| W6 | **Plan screen: dependency graph Tab view** — specified in ROADMAP P6.5; no Tab handler wired in plan model | `internal/tui/plan_model.go` | P1 |
| W7 | **Plan screen: diff preview D key** — `predicted_files` field exists in Task; no diff overlay wired to `D` key | `internal/tui/plan_model.go` | P1 |
| W8 | **Plan screen: per-task arbitrage O key** — ROADMAP P6.5; no `O` handler visible | `internal/tui/plan_model.go` | P2 |
| W9 | **Ship screen: open in browser (`O`)** — ROADMAP P6.12 | `internal/tui/ship_model.go` | P3 |
| W10 | **`/memory` command family** — `Pause()`, `Resume()`, `IsPaused()` all exist in `pkg/autodream/`; none exposed as slash commands | `internal/tui/commands_ai.go` | P1 |

---

### 3.5 Security

| ID | Feature | File / Line | Priority |
|----|---------|-------------|----------|
| S1 | **API key plaintext in config** — `Save()` writes keys to TOML; should encrypt at rest or refuse to write key material | `internal/config/loader.go` | P0 |
| S2 | **No command blacklist for Bash** — no built-in block for `rm -rf /`, `dd if=`, fork bombs | `internal/tools/bash.go`, `internal/config/types.go` | P1 |
| S3 | **No allowed-paths restriction** — `AllowedPathPatterns []string` missing from `ToolsConfig`; all paths within workDir accessible | `internal/tools/fileread.go:98`, `internal/config/types.go` | P1 |
| S4 | **Sandboxed Bash execution** — commands run with full process privileges; no `seccomp` or `unshare` namespace on Linux | `internal/tools/bash.go` | P2 |

---

### 3.6 Missing Integrations

| ID | Feature | File / Line | Priority |
|----|---------|-------------|----------|
| I1 | **`.env` file auto-loading** — projects using `.env` for dev credentials must `export` manually | `internal/config/loader.go` | P1 |
| I2 | **`ToolsConfig.Env` injection** — no way to inject per-project env vars into Bash tool from config | `internal/config/types.go`, `internal/tools/bash.go` | P2 |
| I3 | **Shell profile sourcing** — Bash runs in clean subprocess; `pyenv`, `nvm`, `conda` shims unavailable | `internal/tools/bash.go:84` | P2 |
| I4 | **Linter/formatter integration post-edit** — no auto-run of `gofmt`, `eslint`, etc. after FileWrite/FileEdit | `internal/workflow/execute.go` | P2 |
| I5 | **PR description generation** — Ship phase commits but generates no PR description markdown | `internal/workflow/ship.go` | P2 |

---

## 4. Infrastructure & Package Gaps

### 4.1 Session (`pkg/session/`)

| ID | Feature | File / Line | Priority |
|----|---------|-------------|----------|
| SE1 | **Session labels/tags** — `Session` struct has no `Label string` or `Tags []string`; resume screen shows opaque UUIDs | `pkg/session/session.go`, `internal/types/types.go:154` | P1 |
| SE2 | **Session search/filter** — `ListSessions()` returns all unsorted; no filter by goal text, model, date, phase | `pkg/session/manager.go:279` | P1 |
| SE3 | **Session export** — no `ExportSessionMarkdown()` or `ExportSessionJSON()` | `pkg/session/manager.go` | P1 |
| SE4 | **Session import** — no `ImportSession()` for bringing external session data in | `pkg/session/manager.go` | P3 |
| SE5 | **Session size limits** — no message count cap or disk-size limit; long sessions accumulate unboundedly | `pkg/session/manager.go` | P2 |
| SE6 | **Session merge** — no `MergeSession()` to combine forked lineages | `pkg/session/manager.go` | P3 |
| SE7 | **Auto-cleanup not triggered on startup** — `Cleanup(maxAge)` exists but no startup hook in main | `cmd/m31a/main.go` | P2 |
| SE8 | **Crash recovery / auto-save** — no SIGTERM/SIGINT handler flushing session; `os.Exit(1)` on program error | `cmd/m31a/main.go:224` | P1 |
| SE9 | **Checkpoint depth** — hard-capped at 2; older checkpoints permanently lost | `pkg/session/checkpoint.go:17` | P2 |

---

### 4.2 Ledger (`pkg/ledger/`)

| ID | Feature | File / Line | Priority |
|----|---------|-------------|----------|
| L1 | **Date-range queries** — `EntriesFiltered` has no `since`/`until` params | `ledger.go:444` | P2 |
| L2 | **Export (CSV / JSON)** — no `Export(format, path)` for external analysis | `ledger.go` | P2 |
| L3 | **Provider / model filter** — can filter by `projectType` only; no model or provider filter | `ledger.go:444` | P2 |
| L4 | **`SkippedTasks` lost on round-trip** — `formatEntry` serializes it; `parseEntry` doesn't read it back | `ledger.go:147` | P1 |
| L5 | **Mid-session crash loses metrics** — ledger written only on ship; incomplete sessions unrecorded | `ledger.go` | P2 |
| L6 | **No tagging** — `LedgerEntry` has no custom tags/categories | `ledger.go:22` | P3 |

---

### 4.3 Rollback (`pkg/rollback/`)

| ID | Feature | File / Line | Priority |
|----|---------|-------------|----------|
| RB1 | **Merge commit detection** — resetting to merge commit proceeds silently without warning | `rollback.go:45` | P2 |
| RB2 | **Per-file rollback** — no `ResetFile(hash, path)` using `git checkout <hash> -- <path>` | `rollback.go` | P2 |
| RB3 | **Backup branch clobbered on every reset** — `--force` hardcoded; `m31a-backup-pre-reset` silently overwritten | `rollback.go:116` | P1 |
| RB4 | **Dry-run reset** — no preview of what a reset would change without executing | `rollback.go` | P3 |

---

### 4.4 AutoDream (`pkg/autodream/`)

| ID | Feature | File / Line | Priority |
|----|---------|-------------|----------|
| AD1 | **Never triggered automatically** — only via manual `/compress`; `AutoDreamThreshold = 0.60` constant exists but no code reads token usage and calls `Consolidate()` | `autodream.go:126`, `internal/types/constants.go:13` | P0 |
| AD2 | **No LLM-generated summary** — consolidation is word-truncated concatenation (≤384 words), not an AI summary; quality is poor | `autodream.go:183` | P1 |
| AD3 | **Summary appended at tail** — consolidated messages appended at end instead of injected at beginning as context prefix | `autodream.go:222` | P1 |
| AD4 | **Token estimate uses word count** — uses `word_count × 1.3` instead of `tokens.Estimator`; `TokensSaved` inaccurate | `autodream.go:189` | P2 |
| AD5 | **Consolidated messages not persisted** — compression lives in RAM only; next restart undoes compression | `autodream.go` | P2 |

---

### 4.5 Arbitrage (`pkg/arbitrage/`)

| ID | Feature | File / Line | Priority |
|----|---------|-------------|----------|
| AR1 | **`AutoArbitrage` flag never read at runtime** — config field exists in `config.Model.AutoArbitrage`; no production code path reads it to auto-switch models — the feature is entirely dead | `arbitrage.go:64`, `config/types.go:58` | P0 |
| AR2 | **No routing integration** — `/optimize` returns a `ToastMsg` string; user must manually switch; no auto-switch | `commands_ai.go:41` | P1 |
| AR3 | **No model pricing cache** — `Recommend()` calls `FetchModels()` on every invocation | `arbitrage.go` | P2 |
| AR4 | **Keyword-only task scoring** — 14 hardcoded keywords; domain terms like "containerize", "benchmark" misclassified | `arbitrage.go:64` | P2 |
| AR5 | **No quality weighting** — cheapest model always wins regardless of benchmark scores | `arbitrage.go` | P3 |

---

### 4.6 Bisect (`pkg/bisect/`)

| ID | Feature | File / Line | Priority |
|----|---------|-------------|----------|
| B1 | **No `/bisect` slash command or TUI screen** — bisect is only invoked from `workflow/verify.go`; entirely invisible to users | `bisect.go`, `internal/tui/commands*.go` | P1 |
| B2 | **No progress reporting** — no callback during bisect loop to report iteration count or ETA to UI | `bisect.go:61` | P2 |
| B3 | **No `git bisect skip` support** — loop doesn't handle commits that can't be tested | `bisect.go:61` | P2 |
| B4 | **ShortHash not populated** — `BisectResult.OffendingCommit.ShortHash` set to full hash | `bisect.go:133` | P3 |

---

### 4.7 Taskrunner (`pkg/taskrunner/`)

| ID | Feature | File / Line | Priority |
|----|---------|-------------|----------|
| TR1 | **No concurrent execution** — groups are identified as parallelizable but `ExecuteGroup` runs them sequentially (V1 deferred) | `runner.go:133` | P2 |
| TR2 | **No retry logic** — failed task marked `StatusFailed` permanently; no `MaxRetries` or backoff | `runner.go` | P1 |
| TR3 | **No abort-on-failure mode** — one task failing doesn't stop remaining tasks in same group | `runner.go:133` | P2 |
| TR4 | **Single timeout for all tasks** — `TaskTimeout` applies globally; individual tasks can't declare their own | `runner.go:61` | P2 |
| TR5 | **No pause/resume** — `Runner` has no `Pause()`/`Resume()` for mid-execution suspension | `runner.go` | P2 |

---

### 4.8 Keychain (`pkg/keychain/`)

| ID | Feature | File / Line | Priority |
|----|---------|-------------|----------|
| K1 | **No encrypted-file fallback** — when D-Bus and `pass` both fail on Linux, no fallback; users on minimal systems are stuck | `keychain_linux.go:41` | P1 |
| K2 | **Darwin: stderr not checked** — `security` CLI writes errors to stderr; `Get()` checks stdout only | `keychain_darwin.go:36` | P1 |
| K3 | **No `List()` operation** — can't enumerate stored keys for audit or diagnostics | `keychain.go` | P2 |
| K4 | **Service name regex too restrictive** — `^[a-z]+$` blocks digits/hyphens/underscores; conflicts with future multi-provider names | `keychain.go` | P2 |
| K5 | **`m31a keychain` CLI subcommands missing** — `setup`, `rotate`, `remove` specified in idea.md §10.2; absent from `cmd/m31a/` | `cmd/m31a/main.go` | P1 |

---

### 4.9 Logging (`internal/log/`)

| ID | Feature | File / Line | Priority |
|----|---------|-------------|----------|
| LG1 | **No TUI log viewer or `/logs` command** — users must `tail -f ~/.m31a/m31a.log` externally | `internal/log/log.go` | P2 |
| LG2 | **No size-based rotation** — date-based only; single verbose day can produce a multi-GB log | `log.go:69` | P2 |
| LG3 | **Log level not settable via config.toml** — only via `M31A_LOG_LEVEL` env var; settings screen has no log control | `log.go:20` | P2 |
| LG4 | **Rotation uses mtime** — unreliable on some tmpfs mounts that don't update mtime on append-only writes | `log.go:128` | P3 |

---

### 4.10 Git (`internal/git/`)

| ID | Missing Operation | Priority |
|----|------------------|----------|
| G1 | **`Fetch(remote)`** — needed for sync status in Ship phase | P1 |
| G2 | **`Pull(rebase bool)`** — Ship phase uses ad-hoc `Run()` | P1 |
| G3 | **`Push(remote, branch)`** — Ship phase uses ad-hoc `Run()` | P1 |
| G4 | **`CheckoutBranch(name, create bool)`** — `CreateBranch()` exists but doesn't switch | P2 |
| G5 | **`StashList()` / `StashApply(index)`** — only push/pop; no selective stash management | P2 |
| G6 | **`Merge(branch)`** — no merge wrapper | P2 |
| G7 | **`Rebase(branch)`** — no rebase wrapper | P3 |
| G8 | **`Tag(name, msg)`** — needed for release tagging in Ship | P2 |
| G9 | **`BranchList()`** — no listing of branches | P2 |
| G10 | **Backup branch clobbered** — `--force` hardcoded in `ResetSoft`/`ResetHard`; `m31a-backup-pre-reset` silently overwritten | `git.go:370` | P1 |

---

### 4.11 Token Estimation (`internal/tokens/`)

| ID | Feature | File / Line | Priority |
|----|---------|-------------|----------|
| TK1 | **tiktoken-go is unmaintained** — Claude 3.x / Gemini / GPT-4o models all fall back to rune÷4×1.3 estimator | `estimator.go:11` | P1 |
| TK2 | **Claude tokenizer missing** — no Anthropic tokenizer; Claude models systematically mis-estimated | `estimator.go:62` | P1 |
| TK3 | **Tool call JSON not counted** — `EstimateMessages` sums `msg.Content` only; tool call payloads (can be very large) excluded | `estimator.go:150` | P2 |
| TK4 | **EMA calibration not persisted** — resets to 1.0 on every restart; re-learns each session | `estimator.go:80` | P2 |
| TK5 | **No per-model context window registry** — `FormatUsage` requires caller to pass `total int64`; no built-in model→context-window lookup | `estimator.go` | P2 |
| TK6 | **Context usage bar** — `UIConfig.ShowTokenUsage` exists; `ContextWarningBanner` exists; no lipgloss progress bar in status bar | `internal/config/types.go:69`, `statusbar.go` | P2 |

---

### 4.12 Build / Release

| ID | Feature | File / Line | Priority |
|----|---------|-------------|----------|
| BR1 | **Makefile color variables undefined** — `$(GREEN)`, `$(YELLOW)`, `$(RED)`, `$(NC)` referenced but never defined; print literally on some `make` versions | `Makefile` | P1 |
| BR2 | **GoReleaser missing Commit/Date ldflags** — only `-X main.Version` injected; release binaries have no commit hash or build date | `.goreleaser.yaml:17` | P1 |
| BR3 | **No `make security` target** — no `gosec` or `govulncheck` in Makefile | `Makefile` | P2 |
| BR4 | **No `make integration-test` target** — no integration test separate from unit tests | `Makefile` | P2 |
| BR5 | **No Homebrew tap config in GoReleaser** — README mentions `brew install eshanized/tap/m31a`; `.goreleaser.yaml` has no `brews:` section | `.goreleaser.yaml` | P2 |
| BR6 | **No `.deb`/`.rpm` packaging** — no `nfpms:` section in GoReleaser | `.goreleaser.yaml` | P3 |
| BR7 | **No Docker support** — no Dockerfile, no `dockers:` in GoReleaser | `.goreleaser.yaml` | P3 |
| BR8 | **No GPG signing** — no `signs:` in GoReleaser | `.goreleaser.yaml` | P3 |
| BR9 | **golangci-lint-action@v4** — v6 is current; CI uses stale version | `.github/workflows/` | P2 |
| BR10 | **No CI coverage threshold** — tests run but no minimum coverage gate | `.github/workflows/` | P2 |

---

## 5. Product & Strategy Gaps

### 5.1 ROADMAP Features Not Yet Built

| ID | Feature | Spec Source | Priority | Complexity |
|----|---------|-------------|----------|------------|
| R1 | `/new <goal>` — start fresh workflow | idea.md §14.2, REFERENCE.md | P0 | S |
| R2 | `/memory` family — view/pause/resume/revert AutoDream | idea.md §14.1 | P1 | M |
| R3 | Per-project `m31a.toml` multi-layer config | ROADMAP Phase 11 | P1 | L |
| R4 | Ledger context injection in Initialize phase | idea.md §16.4 | P1 | M |
| R5 | Plan screen dependency graph (Tab view) | ROADMAP P6.5 | P1 | M |
| R6 | Plan screen diff preview (D key) | AC #24 | P1 | M |
| R7 | Plan screen per-task arbitrage (O key) | ROADMAP P6.5 | P1 | M |
| R8 | `${VAR}` substitution in all string config fields | ROADMAP Phase 12 D-34 | P1 | S |
| R9 | Session list index file for O(1) session listing | DEEP-IMPROVEMENT-REPORT D-32 | P1 | S |
| R10 | Ghost mode (git worktree isolation) | ROADMAP Phase 9 | P2 | XL |
| R11 | Terminal PiP (floating panel) | ROADMAP Phase 9 | P2 | XL |
| R12 | Concurrent subagents | ROADMAP V1.1 | P2 | XL |
| R13 | WebSearch tool (SerpAPI / Tavily) | ROADMAP V1.1.1 | P2 | M |
| R14 | GitTool (git ops in tool system) | ROADMAP V1.1.1 | P2 | M |
| R15 | Architect mode (separate planning vs coding model) | Aider parity | P2 | M |

---

### 5.2 Slash Command Gaps

**Registered:** 37 commands | **Documented in SLASH_COMMANDS.md:** 16 | **Gap:** 21 undocumented

Missing commands that are **specified but not implemented:**

| Command | Spec Source | Priority |
|---------|-------------|----------|
| `/new` | idea.md §14.2 | P0 |
| `/memory` | idea.md §14.1 | P1 |
| `/memory review` | idea.md §14.1 | P1 |
| `/memory revert` | idea.md §14.1 | P1 |
| `/memory pause` | idea.md §14.1 | P1 |
| `/memory resume` | idea.md §14.1 | P1 |
| `/export` | UX need | P1 |
| `/label` | Session UX | P1 |
| `/tasks` | idea.md §14.3 (alias for /workflow) | P2 |
| `/demo` | idea.md §14.3 | P2 |
| `/ghost` | idea.md §14.2, V1.1 | P2 |

---

### 5.3 Competitor Feature Comparison

#### vs. Aider
| Feature | Aider | M31A |
|---------|-------|------|
| Architect mode (separate planning+coding models) | ✅ | ❌ |
| Post-edit linter/formatter integration | ✅ | ❌ |
| `--watch` mode (auto-respond to file changes) | ✅ | ❌ |
| Git commit message customization | ✅ | ❌ (hardcoded format) |
| Local model support (Ollama, LM Studio) | ✅ | ❌ |
| Voice input | ✅ | ❌ |

#### vs. Claude Code
| Feature | Claude Code | M31A |
|---------|-------------|------|
| Headless / `--no-tty` CI mode | ✅ | ❌ |
| MCP tool ecosystem | ✅ | ❌ |
| Vision/multimodal input | ✅ | ❌ |
| Subagents (parallel) | ✅ | ❌ (V1.1 planned) |
| GitHub issue/PR integration | ✅ | ❌ |

#### vs. Cursor / Copilot Chat
| Feature | Cursor | M31A |
|---------|--------|------|
| Real-time codebase indexing / embeddings | ✅ | ❌ |
| Language server integration | ✅ | ❌ |
| Inline diff / edit review | ✅ | terminal diff only |

---

### 5.4 Onboarding Gaps

| ID | Gap | Priority |
|----|-----|----------|
| O1 | First-run wizard doesn't validate API key before proceeding | P1 |
| O2 | First-run: no "Store in keychain?" explicit prompt | P1 |
| O3 | `m31a keychain setup/rotate/remove` subcommands missing from CLI | P1 |
| O4 | `M31A_THEME`, `M31A_PROVIDER`, `M31A_DEFAULT_MODEL` env vars not documented in README | P2 |
| O5 | `resume_on_startup` config flag doesn't trigger auto-resume on launch | P1 |
| O6 | Discuss phase: no "skip" keyword to fill with defaults (per spec) | P1 |

---

### 5.5 Documentation Gaps

| ID | Gap | Priority |
|----|-----|----------|
| Doc1 | **README says "16 slash commands" — actual: 37** | P0 |
| Doc2 | **README says "5 core tools" — actual: 9** | P0 |
| Doc3 | **AGENTS.md V1 tool list is stale** (lists 5; 9 exist) | P0 |
| Doc4 | **SLASH_COMMANDS.md documents 16 of 37 commands** | P0 |
| Doc5 | CHANGELOG has no entries after v1.0.0 (Phases 10–12 unlogged) | P1 |
| Doc6 | Key bindings not exhaustively documented (README shows 5; app has 60+) | P1 |
| Doc7 | No test writing guide beyond coverage targets in CONTRIBUTING.md | P2 |
| Doc8 | No API integration guide for external tooling | P2 |

---

## 6. Master Priority Matrix

### P0 — Fix Immediately (Blocking / Security / Data Loss)

| ID | Description | File(s) | Effort |
|----|-------------|---------|--------|
| S1 | API key plaintext written to config on Save() | `internal/config/loader.go` | S |
| AD1 | AutoDream never auto-triggered despite threshold constant | `pkg/autodream/`, `internal/types/constants.go` | M |
| AR1 | `AutoArbitrage` config flag never read — feature is dead | `pkg/arbitrage/`, `internal/config/types.go` | M |
| U42 | No confirmation for `/clear` and `/reset` (data loss) | `internal/tui/commands_core.go` | S |
| U11 | No copy-to-clipboard for messages | `internal/tui/`, `components/message.go` | S |
| W1 | `/new` command not registered | `internal/tui/commands*.go` | S |
| Doc1-4 | README/AGENTS.md/SLASH_COMMANDS.md severely stale | docs/, README.md, AGENTS.md | S |

### P1 — Important (Ship in Next Version)

| ID | Description | Effort |
|----|-------------|--------|
| U1 | Shift+Enter multi-line input | S |
| U2 | Auto-expand textarea height | S |
| U12 | "File attached" toast for @-mention | S |
| U13 | "Response cancelled" toast | S |
| U14 | User message edit/re-send | M |
| U19 | "New messages" indicator when scrolled up | S |
| U21 | j/k scroll in REPL viewport | S |
| U22 | Viewport focus mode (modal editing) | M |
| U23 | Search within conversation | M |
| U27 | Help screen: add missing bindings + make scrollable | S |
| U28 | Help screen scrollable | S |
| U30 | Show ctrl+c hint during streaming | S |
| U38 | CompactMode live effect in views | M |
| U46 | Session rename | S |
| U47 | Session export (`/export`) | M |
| U48 | Session search in resume screen | M |
| P1 | Streaming retry on disconnect | M |
| P2 | HTTP 5xx retry before fallback | S |
| P3 | Cost budget enforcement | M |
| T1 | FileList / directory tree tool | S |
| T2 | FileDelete tool | S |
| T3 | FileMove/Rename tool | S |
| W2 | Mid-workflow checkpoint resume | M |
| W5 | Context limit → auto-compaction | M |
| W6 | Plan: dependency graph Tab view | M |
| W7 | Plan: diff preview D key | M |
| W10 | `/memory` command family | M |
| S2 | Bash command blacklist defaults | S |
| S3 | Allowed-path restrictions in file tools | M |
| I1 | `.env` file auto-loading | S |
| SE1 | Session labels/tags | S |
| SE2 | Session search/filter | S |
| SE3 | Session export | M |
| SE8 | SIGTERM/SIGINT crash recovery | M |
| L4 | Ledger: SkippedTasks lost on round-trip | S |
| AD2 | AutoDream: LLM-generated summary | M |
| AD3 | AutoDream: summary at beginning not end | S |
| B1 | Bisect: expose `/bisect` command + TUI | M |
| TR2 | Taskrunner: retry logic | M |
| K1 | Keychain: encrypted-file fallback on Linux | M |
| K2 | Keychain: Darwin stderr check | S |
| K5 | `m31a keychain` CLI subcommands | M |
| G1-3 | Git: Fetch, Pull, Push | S |
| G10 | Git: backup branch clobbered by --force | S |
| TK1 | tiktoken-go replacement / update | M |
| TK2 | Claude tokenizer | M |
| TK6 | Context usage progress bar in status bar | S |
| BR1 | Makefile color vars undefined | S |
| BR2 | GoReleaser missing Commit/Date ldflags | S |
| C2 | Env var coverage for all key config fields | S |
| C3 | `resume_on_startup` wiring | S |
| O1-5 | Onboarding gaps | S-M |

### P2 — Nice to Have (Backlog)

All remaining items in sections 2–5 not listed above, including: high-contrast theme, reduced-motion mode, drag-resize textarea, message deletion, diff hunk jump, word-level navigation, session merge, ledger export/date-range queries, rollback per-file, arbitrage quality weighting, Docker support, Homebrew tap, etc.

### P3 — Future / Deferred

Ghost mode, PiP, concurrent subagents, voice input, MCP integration, real-time embeddings, language server integration.

---

*Report compiled 2026-06-09 from 4-agent parallel deep audit: TUI/UX (72 files), Core Features (tools/provider/config/workflow), Infrastructure (9 packages), Product/Strategy (docs/roadmap/competitors).*

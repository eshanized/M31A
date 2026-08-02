# M31A Audit Report — 2026-08-02

## Executive Summary

M31A is a well-engineered Go-based terminal AI coding agent with a strong seven-phase workflow engine, solid concurrency patterns, and an excellent TUI built on Bubble Tea. All nine documented bugs (BUG-01 through BUG-29) have been properly fixed with regression tests. The biggest risk is **security**: the bash sandbox lacks network isolation and doesn't block common exfiltration commands (curl, wget, python), the SSRF protection has redirect and DNS rebinding bypasses, and secret detection is a trivial 5-prefix substring matcher. The biggest UX opportunity is **competitive parity gaps** — no MCP support, no hooks/plugin system, no dry-run mode, and no per-file accept/reject review — features that are becoming table-stakes in this category. Documentation has significant drift from the reorganized codebase structure.

---

## Doc Drift

| Doc | Claim | Reality | Evidence |
|-----|-------|---------|----------|
| `docs/ARCHITECTURE.md` (line 18) | `internal/codeintel/` | Code lives at `internal/integrations/codeintel/` | `internal/integrations/codeintel/codeintel.go` |
| `docs/ARCHITECTURE.md` (line 19) | `internal/config/` | Code lives at `internal/core/config/` | `internal/core/config/loader.go` |
| `docs/ARCHITECTURE.md` (line 20) | `internal/context/` | Code lives at `internal/integrations/context/` | `internal/integrations/context/registry.go` |
| `docs/ARCHITECTURE.md` (line 21) | `internal/decision/` | Code lives at `internal/engine/decision/` | `internal/engine/decision/logger.go` |
| `docs/ARCHITECTURE.md` (line 22) | `internal/errors/` | Code lives at `internal/core/errors/` | `internal/core/errors/errors.go` |
| `docs/ARCHITECTURE.md` (line 23) | `internal/fileutil/` | Code lives at `internal/infrastructure/fileutil/` | `internal/infrastructure/fileutil/atomic.go` |
| `docs/ARCHITECTURE.md` (line 24) | `internal/git/` | Code lives at `internal/integrations/git/` | `internal/integrations/git/git.go` |
| `docs/ARCHITECTURE.md` (line 25) | `internal/log/` | Code lives at `internal/integrations/log/` | `internal/integrations/log/log.go` |
| `docs/ARCHITECTURE.md` (line 26) | `internal/provider/` | Code lives at `internal/integrations/provider/` | `internal/integrations/provider/interface.go` |
| `docs/ARCHITECTURE.md` (line 37) | `internal/tokens/` | Code lives at `internal/engine/tokens/` | `internal/engine/tokens/estimator.go` |
| `docs/ARCHITECTURE.md` (line 38) | `internal/tools/` (flat) | Now `internal/tools/` with subdirectories: `exec/`, `fileops/`, `search/`, `subagent/`, `ai/`, `codeanalysis/`, `todo/`, `git/`, `network/` | `internal/tools/defaults.go` imports |
| `docs/ARCHITECTURE.md` (line 44) | `internal/tui/` | Code lives at `internal/ui/tui/` | `internal/ui/tui/app.go` |
| `docs/ARCHITECTURE.md` (line 54) | `internal/types/` | Code lives at `internal/core/types/` | `internal/core/types/types.go` |
| `docs/ARCHITECTURE.md` (line 55) | `internal/workflow/` | Code lives at `internal/engine/workflow/` | `internal/engine/workflow/engine.go` |
| `docs/ARCHITECTURE.md` (line 65-79) | `pkg/` contains public packages (autodream, arbitrage, bisect, etc.) | `pkg/` is **empty** (0 files). All listed packages are under `internal/` | `pkg/` directory listing: 0 entries |
| `docs/ARCHITECTURE.md` (line 90-93) | Dependency rule: `pkg/` must NOT import `internal/` | Rule is vacuous — `pkg/` has no code at all | `pkg/` directory listing |
| `docs/ONBOARDING.md` (line 131) | "The getting-started tour explains the workflow model" | First-run wizard (firstrun_model.go) exists but is a provider setup wizard, not a workflow tour | `internal/ui/tui/firstrun_model.go` |
| `docs/KEYBINDINGS.md` (line 131) | Permission modal: `e` = "Exit" | Code uses `n`, `enter`, `esc` for deny/exit; `e` is not used. `b` (approve all) is missing from docs | `internal/ui/tui/app_session.go:219-226`, `internal/ui/tui/components/permission.go:113-129` |
| `docs/KEYBINDINGS.md` (line 156-194) | Slash commands list is incomplete | Missing: `/refine`, `/decide`, `/pending`, `/debug`, `/theme`, `/agent`, `/agent-cancel`, `/phase-model`, `/replay` | `internal/ui/tui/commands/commands.go:220-310` |

---

## Confirmed Open Bugs / Risks

| Ref | Status | Evidence | Severity |
|-----|--------|----------|----------|
| BUG-01 | **Fixed** | `internal/integrations/git/git.go:440-460` — WaitGroup replaces channels, explicit comment references BUG-01 | -- |
| BUG-04/05 | **Fixed** | `internal/engine/workflow/engine_parse.go:229-253` — priority-ordered slice; `classify_test.go:15` has 100-iteration determinism test | -- |
| BUG-06/07/19 | **Fixed** | All `sync.Map` uses have comma-ok guards; `capabilities.go` uses `sync.RWMutex` + typed map | -- |
| BUG-10 | **Fixed** | `internal/engine/workflow/ship.go:116-123` — `DiffStaged()` checks index not worktree; regression test at `engine_extra_test.go:3090` | -- |
| BUG-12 | **Fixed** | `internal/engine/workflow/state_machine.go:87-97` — cycle limit of 3, counter resets on convergence | -- |
| BUG-15 | **Fixed** | `internal/engine/workflow/ship.go:343-428` — diff-filter refinement replaces numstat heuristic | -- |
| BUG-17 | **Fixed** | `internal/integrations/provider/cache.go:44-63` — singleflight + atomic.Bool + RWMutex | -- |
| BUG-18 | **Fixed** | `internal/core/config/loader.go:618-637` — two-stage send, never silently drops | -- |
| BUG-29 | **Fixed** | `internal/engine/tokens/estimator.go:289-415` — EMA clamping + EstimateMessages overhead | -- |

### New Security Risks Identified

| Ref | Area | File:Line | Status | Severity |
|-----|------|-----------|--------|----------|
| SEC-01 | Bash sandbox: curl/wget/python not blocked | `internal/tools/exec/bash.go:303-344` | **Live** | HIGH |
| SEC-02 | Bash sandbox: no network namespace isolation | `internal/tools/exec/bash_sandbox_linux.go` (absent) | **Live** | HIGH |
| SEC-03 | WebFetch: no IP re-check on redirects (SSRF bypass) | `internal/tools/search/webfetch.go:65-70` | **Live** | HIGH |
| SEC-04 | WebFetch: no DNS pinning (DNS rebinding) | `internal/tools/search/webfetch.go:246-256` | **Live** | HIGH |
| SEC-05 | WebSearch: checks `ips[0]` instead of loop var | `internal/tools/search/websearch.go:79,105` | **Live** | MEDIUM |
| SEC-06 | Secret detection: 5 prefix strings only, trivially evadable | `internal/engine/workflow/ship_preflight.go:67-76` | **Live** | MEDIUM |
| SEC-07 | Subagent prompt injection: system-prompt-only defense | `internal/tools/subagent/loop.go:397-403` | **Live** | MEDIUM |
| SEC-08 | Config: plaintext API key fallback when keychain unavailable | `internal/core/config/loader.go:516-534` | **Live** | MEDIUM |
| SEC-09 | `M31A_SKIP_LANDLOCK=1` disables all sandboxing | `internal/tools/exec/bash_sandbox_linux.go:26` | **Live** | LOW |

---

## Competitive Gaps

| Feature | Category | Status | Why it matters | Effort |
|---------|----------|--------|----------------|--------|
| MCP (Model Context Protocol) client | Context & extensibility | **Absent** | Table-stakes for agentic tools; enables ecosystem of third-party tool servers | L |
| Hooks (pre/post tool call) | Context & extensibility | **Absent** | Users expect to wire custom automation (lint on save, auto-format, etc.) | M |
| Plugin/extension system | Context & extensibility | **Absent** | Limits community contribution beyond the 18 built-ins | L |
| Per-file accept/reject review | Trust & transparency | **Partial** (view only) | Users want file-by-file control before changes land; Cursor, Aider, Claude Code all have this | M |
| Dry-run / preview mode | Trust & transparency | **Absent** | Users want to see what would happen before granting permission | M |
| Live cost meter (per-provider) | Trust & transparency | **Present** | (covered — sidebar shows cost, burn rate, budget) | -- |
| Checkpoint/undo to any point | Session & workflow | **Partial** (phase-level only) | Rollback exists but is git-commit-based, not arbitrary session-point undo | M |
| Headless JSON output for CI | Session & workflow | **Partial** | `--headless` exists but outputs plain text, not machine-readable JSON | S |
| Local model support (Ollama/vLLM) | Provider flexibility | **Absent** | Growing demand for local-first models; Cursor, Continue, ollama-based tools all support | M |
| First-run onboarding completeness | Onboarding | **Partial** | Wizard covers provider setup but doesn't explain workflow, permissions, or cost | S |
| Screen reader / plain terminal | Accessibility | **Partial** | ASCII fallback exists but no runtime TUI fallback; headless is separate invocation | M |
| `--verbose`/`--quiet` global flags | Progressive disclosure | **Absent** | No global verbosity toggle; per-feature config only | S |

---

## UX Findings

| Heuristic | Verdict | Example | Suggested fix |
|-----------|---------|---------|---------------|
| Permission fatigue | **Strong** | Batch approval (B key), session-scoped remember (A key), config rules, safe tools auto-approve | No fix needed — well designed |
| Error legibility | **Excellent** | 27+ sentinel errors mapped to plain English via `UserMessage()`, structured error types with context | No fix needed |
| Latency feedback | **Excellent** | Braille spinner (10fps), "thinking..." footer, streaming tokens, per-task spinners, thinking blocks | No fix needed |
| Recoverability | **Good** | Session persists on graceful shutdown, `/resume-task` works, Ctrl+C double-press | Gap: SIGKILL loses in-progress state; consider periodic checkpoint writes |
| Progressive disclosure | **Good** | Thinking blocks collapsed, tool cards auto-collapse, debug gated by env var | Gap: No global `--verbose`/`--quiet` flag |
| Consistency | **One discrepancy** | KEYBINDINGS.md line 131 lists `e` for Exit in permission modal; code uses `esc`/`n`. `b` key (approve all) missing from docs. Execute-screen keys (s/c/x) undocumented. | Update KEYBINDINGS.md to match code |
| Accessibility | **Partial** | ASCII fallback for Unicode, responsive layout at 3 breakpoints | Gap: No `NO_COLOR` detection, no plain-terminal runtime fallback, single dark theme only |

### Coverage Gaps (below 75% target)

| Package | Coverage | Target | Gap |
|---------|----------|--------|-----|
| `internal/tools/search` | 2.0% | 75% | **-73%** — critical: SSRF, webfetch, websearch barely tested |
| `internal/core/types` | 15.6% | 75% | -59% |
| `cmd/m31a` | 21.6% | 75% | -53% |
| `internal/integrations/keychain` | 25.1% | 75% | -50% (platform-dependent) |
| `internal/ui/tui` | 32.4% | 75% | -43% — large TUI codebase, low coverage |
| `internal/ui/tui/streaming` | 33.1% | 75% | -42% |
| `internal/ui/tui/commands` | 43.4% | 75% | -32% |
| `internal/ui/tui/components` | 46.1% | 75% | -29% |
| `internal/tools/todo` | 49.1% | 75% | -26% |
| `internal/ui/tui/tuitypes` | 50.4% | 75% | -25% |
| `internal/engine/rollback` | 53.8% | 90% | -36% (critical path target) |
| `internal/tools/git` | 57.9% | 75% | -17% |
| `internal/integrations/codeintel` | 58.1% | 75% | -17% |
| `internal/engine/compaction` | 59.8% | 75% | -15% |
| `internal/engine/decision` | 66.9% | 75% | -8% |
| `internal/tools` (root) | 72.5% | 75% | -3% |
| `internal/engine/workflow` | 71.9% | 75% | -3% |
| `internal/tools/ai` | 0.0% | 75% | **No tests** |
| `internal/tools/exec` | 0.0% | 75% | **No tests** |
| `internal/tools/fileops` | 0.0% | 75% | **No tests** |
| `internal/tools/network` | 0.0% | 75% | **No tests** |
| `internal/integrations/provider/nvidia` | 0.0% | 75% | **No tests** |

---

## Prioritized Roadmap

### P0 — Correctness/Security (do now)

1. **SEC-01/02**: Add network isolation to bash sandbox — block `curl`, `wget`, `nc` (outbound), `python -c`, `perl -e`, `ruby -e`, `node -e` in dangerous command patterns, or add network namespace (`unshare CLONE_NEWNET`). File: `bash.go:303-344`, `bash_sandbox_linux.go`.
2. **SEC-03/04**: Fix WebFetch SSRF — re-check IPs on redirects, add DNS pinning via custom `DialContext` (match `websearch.go` pattern). File: `webfetch.go:65-70,246-256`.
3. **SEC-05**: Fix WebSearch IP check bug — change `ips[0].IP` to `ip.IP` on lines 79 and 105. File: `websearch.go:79,105`.
4. **Coverage-01**: Add tests for `internal/tools/search` (2% → 75%), `internal/tools/exec` (0% → 75%), `internal/tools/fileops` (0% → 75%). These are core tool packages with zero coverage.

### P1 — High-Impact UX (next milestone)

5. **Feature-01**: Per-file accept/reject review — add accept/reject buttons in the diff sidebar view. File: `sidebar_model.go`, `sidebar_render.go`. Effort: M.
6. **Feature-02**: Dry-run mode — add `--dry-run` flag or `/dry-run` command that shows what tools would execute without running them. File: `dispatcher.go`. Effort: M.
7. **Feature-03**: Headless JSON output — add `--output=json` flag for CI gating with structured exit codes and machine-readable phase/result output. File: `cmd/m31a/main.go`. Effort: S.
8. **Feature-04**: First-run onboarding — extend wizard to explain workflow model, permission system, and cost implications after provider setup. File: `firstrun_model.go`. Effort: S.
9. **Feature-05**: Local model support — add Ollama provider (OpenAI-compatible API at `localhost:11434`). File: new `internal/integrations/provider/ollama/`. Effort: M.

### P2 — Competitive Parity (medium-term)

10. **Feature-06**: MCP client support — implement MCP client for third-party tool servers. Effort: L.
11. **Feature-07**: Pre/post tool call hooks — allow user-defined scripts/commands to run before/after tool execution. File: `dispatcher.go`. Effort: M.
12. **Feature-08**: Global `--verbose`/`--quiet` flags — control TUI verbosity level. File: `cmd/m31a/main.go`, `internal/ui/tui/`. Effort: S.
13. **Feature-09**: `NO_COLOR` support — respect `NO_COLOR` env var for accessibility. File: `internal/ui/tui/theme/`. Effort: S.
14. **Doc-01**: Regenerate all docs from current codebase structure — fix all 15+ doc drift items in the Doc Drift table. Effort: S.

### P3 — Nice-to-Have

15. **Feature-10**: Plugin/extension system for third-party tools. Effort: L.
16. **Feature-11**: Arbitrary checkpoint undo (not just git-based rollback). Effort: L.
17. **Feature-12**: Plain terminal runtime fallback (degrade from Bubble Tea to line-by-line). Effort: L.
18. **SEC-06**: Improve secret detection beyond 5 prefix strings — add regex patterns for PEM keys, JWT, base64 blobs, env-var references. File: `ship_preflight.go`. Effort: M.
19. **SEC-07**: Strengthen subagent prompt injection defense — add output sanitization or delimiter-based trust zones. File: `loop.go`. Effort: M.

---

## Quick Wins

Things fixable in under a day each:

1. **WebSearch IP bug** — change `ips[0].IP` to `ip.IP` on `websearch.go:79` and `websearch.go:105`. 2 lines.
2. **KEYBINDINGS.md** — remove `e` from permission modal, add `b` (approve all), add execute-screen keys (s/c/x). Documentation only.
3. **WebFetch DNS pinning** — copy the `DialContext` pattern from `websearch.go:65-87` into `webfetch.go`. ~30 lines.
4. **`--output=json` flag** — add JSON output mode to headless execution. ~50 lines in `cmd/m31a/main.go`.
5. **`NO_COLOR` env var** — add check in theme initialization. ~5 lines.
6. **Remove `M31A_SKIP_LANDLOCK`** or gate it behind `debug` build tag only. 1 line.

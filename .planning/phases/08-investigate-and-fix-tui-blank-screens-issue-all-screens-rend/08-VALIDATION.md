---
phase: 08
slug: investigate-and-fix-tui-blank-screens-issue-all-screens-rend
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-16
---

# Phase 08 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| Framework | stdlib `testing` + race detector |
| Config file | `go.test` flags in `Makefile` (`make test` = `go test -race ./...`) |
| Quick run command | `make test-fast` (no race) |
| Full suite command | `make test` (race + coverage) |
| Estimated runtime | ~60 seconds |

---

## Sampling Rate

- **After every task commit:** Run `make test-fast` (< 30s)
- **After every plan wave:** Run `make test` (full suite, race detector)
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 60 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 08-01-01 | 01 | 1 | FR-1.1 | T-08-01 | First-run wizard renders all 4 steps | integration | `go test -run TestFirstRun -v` | ❌ Wave 0 | ⬜ pending |
| 08-01-02 | 01 | 1 | FR-1.1 | T-08-02 | Provider selection renders correctly | unit | `go test -run TestFirstRunProvider -v` | ❌ Wave 0 | ⬜ pending |
| 08-01-03 | 01 | 1 | FR-1.1 | T-08-03 | API key input masked and validated | unit | `go test -run TestFirstRunAPIKey -v` | ❌ Wave 0 | ⬜ pending |
| 08-01-04 | 01 | 1 | FR-1.1 | T-08-04 | Model picker fetches and displays models | integration | `go test -run TestFirstRunModel -v` | ❌ Wave 0 | ⬜ pending |
| 08-02-01 | 02 | 1 | FR-1.2 | T-08-05 | Home screen renders logo, prompt, suggestions | unit | `go test -run TestHomeView -v` | ❌ Wave 0 | ⬜ pending |
| 08-02-02 | 02 | 1 | FR-1.2 | T-08-06 | Slash command autocomplete appears | unit | `go test -run TestHomeSlash -v` | ❌ Wave 0 | ⬜ pending |
| 08-02-03 | 02 | 1 | FR-1.3 | T-08-07 | REPL viewport renders messages | integration | `go test -run TestReplView -v` | ❌ Wave 0 | ⬜ pending |
| 08-02-04 | 02 | 1 | FR-1.3 | T-08-08 | REPL streaming shows incremental content | integration | `go test -run TestReplStream -v` | ❌ Wave 0 | ⬜ pending |
| 08-02-05 | 02 | 1 | FR-1.3 | T-08-09 | REPL viewport scrolls correctly | unit | `go test -run TestReplScroll -v` | ❌ Wave 0 | ⬜ pending |
| 08-03-01 | 03 | 1 | FR-1.5 | T-08-10 | All 34 screens render without blank content | integration | `go test -run TestAllScreens -v` | ❌ Wave 0 | ⬜ pending |
| 08-03-02 | 03 | 1 | FR-1.5 | T-08-11 | Screen transitions via Router work | integration | `go test -run TestScreenRouting -v` | ❌ Wave 0 | ⬜ pending |
| 08-03-03 | 03 | 1 | FR-1.5 | T-08-12 | Router.SetDimensions propagates to screens | unit | `go test -run TestRouterDimensions -v` | ❌ Wave 0 | ⬜ pending |
| 08-03-04 | 03 | 1 | FR-1.5 | T-08-13 | contentDimensions() edge cases guarded | unit | `go test -run TestContentDimensions -v` | ❌ Wave 0 | ⬜ pending |
| 08-03-05 | 03 | 1 | FR-1.5 | T-08-14 | Theme renders on 16-color, 256-color, truecolor | integration | `go test -run TestThemeProfiles -v` | ❌ Wave 0 | ⬜ pending |
| 08-03-06 | 03 | 1 | AC-5 | T-08-15 | Manual verify: `./m31a` shows FirstRun → Home → REPL | e2e | `./verify_v1.sh` (manual) | ❌ Wave 0 | ⬜ pending |
| 08-03-07 | 03 | 1 | NFR-1 | T-08-16 | TUI frame render <16ms (60fps) | bench | `go test -bench=BenchmarkRender -benchtime=1s` | ❌ Wave 0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `internal/tui/firstrun_model_test.go` — covers FR-1.1 (wizard steps render)
- [ ] `internal/tui/home_model_test.go` — covers FR-1.2 (home screen content)
- [ ] `internal/tui/repl_model_test.go` — covers FR-1.3 (REPL rendering)
- [ ] `internal/tui/app_screens_test.go` — covers FR-1.5 (all 34 screens render)
- [ ] `internal/tui/app_nav_test.go` — covers dimension edge cases
- [ ] `internal/tui/theme/theme_test.go` — covers color profile rendering
- [ ] Framework install: stdlib preferred; `github.com/stretchr/testify` if needed

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Full FirstRun → Home → REPL flow in real terminal | AC-5 | Requires real TTY; CI cannot verify visual output | Run `./m31a` in real terminal; complete wizard; verify Home screen; send prompt; verify streaming response |
| Color rendering on xterm-256color, truecolor, 16-color | D-03 | Visual verification; CI terminal profiles differ | Run `./m31a` with `TERM=xterm-256color`, `TERM=xterm-truecolor`, `TERM=linux-16color`; verify text visible |
| Phase transition animations (slide/fade) | FR-1.6 | Visual timing verification | Run `./m31a`; navigate between screens; verify 60fps smooth transitions |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 60s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
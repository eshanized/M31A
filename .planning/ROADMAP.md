# M31A Roadmap

## Phase 1: Resolve Critical Issues

**Goal:** Resolve every Critical issue from the Developer Experience Audit

**Depends on:** 

**Plans:** 

- 01-01-PLAN.md: Fix Bash blocks legitimate syntax, installer URL, headless mode, permission timeout

---

## Phase 2: Resolve High Priority Issues

**Goal:** Resolve every High Priority issue from the Developer Experience Audit

**Depends on:** Phase 1

**Plans:** 

- 02-PLAN.md: Git tool, workdir parameter, settings unification, input validation, phase transitions, pause/resume, verification checks, cost display, persistent permissions, workflow mode

---

## Phase 3: Stabilization — Production Readiness

**Goal:** Resolve every blocker from HIGH_PRIORITY_VERIFICATION_REPORT.md. Fix critical regressions, critical security issues, high severity correctness issues, lint, vet, missing tests, and minor cleanup. The codebase after this pass must satisfy: gofmt, go vet, golangci-lint, race detector, no duplicated logic, no dead code, no known security issues, no new regressions.

**Depends on:** Phase 1, Phase 2

**Plans:** 

- PLAN.md: 16 tasks across 4 waves addressing REGR-1 through REGR-4, NEW-1 through NEW-8, LINT-1 through LINT-5, plus dead code cleanup, tests, and Elm architecture fix

---

## Phase 4: Release Audit Blockers — v1.0 Gate

**Goal:** Resolve all CRITICAL and HIGH blockers from RELEASE_AUDIT_V1.md. Fix architectural boundary violations, data races, test suite timeouts, security bypasses, prompt injection, broken error chains, and magic strings. The codebase after this pass must satisfy: no `pkg/` → `internal/` imports, all data races fixed, all test suites complete within 60s under `-short`, command blocklist comprehensive, prompt injection defended, error chains preserved.

**Depends on:** Phase 3

**Plans:** 5/5 plans executed ✅

- [x] 04-01-PLAN.md — Wave 1: Extract shared types to `pkg/types/`, create `pkg/errors/`, fix architectural boundary (C1)
- [x] 04-02-PLAN.md — Wave 2: Fix data race on `e.provider` (C3), mock DNS/git for test timeouts (C2), security hardening (H1-H4)
- [x] 04-03-PLAN.md — Wave 3: Fix 142 `fmt.Errorf` without `%w` (H5), define provider name constants (H7)
- [x] 04-04-PLAN.md — Wave 4: Add test coverage for `cmd/m31a` and `internal/decision` (H8), permission expiry (M1), engine field protection (M2), file lock (M3)
- [x] 04-05-PLAN.md — Wave 5: Full verification suite, produce RELEASE_AUDIT_RESOLUTION.md

---

**Phase 4: COMPLETE** — All CRITICAL/HIGH blockers resolved. v1.0 release gate cleared.

---

## Phase 5: Codebase Maintainability — Split Large Files

**Goal:** Read all files in the codebase, split large files into smaller files for maintainability, test all files and ensure the project is not broken.

**Depends on:** Phase 4

**Plans:** 1/1 plans complete

- [ ] 05-PLAN.md

- [x] 05-01-PLAN.md — Wave 1: Split low-risk leaf packages (helpers, transition, app_input) — 9 files
- [ ] 05-02-PLAN.md — Wave 2: Split medium-risk tool package (dispatcher, permissions, edit) — 3 files
- [ ] 05-03-PLAN.md — Wave 2: Split medium-risk TUI package (settings_model, sidebar_model, app) — 3 files
- [ ] 05-04-PLAN.md — Wave 2: Split medium-risk config/provider/pkg (loader, types, capabilities, session) — 4 files
- [ ] 05-05-PLAN.md — Wave 3: Split high-risk workflow package (engine, execute, engine_parse) — 3 files
- [ ] 05-06-PLAN.md — Wave 3: Split high-risk entry point + remaining (main.go, git.go, misc) — 3+ files

---

# Phase 01: Repo Reorganization - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-21
**Phase:** 01-repo-reorganization
**Areas discussed:** Root-Level File Cleanup, Duplicate/Backup Files, Documentation Consolidation, Package Reorganization, Test Artifacts & Config

---

## Root-Level File Cleanup

| Option | Description | Selected |
|--------|-------------|----------|
| Keep all at root | Preserve current ~20 audit/planning markdown files | |
| Move to docs/archive/ | Consolidate audit/planning docs into docs/archive/ | ✓ |
| Delete old docs | Remove audit reports, planning docs entirely | |

**User's choice:** Move ~20 audit/planning markdown files (DECOMPOSITION_PLAN.md, DX_AUDIT.md, FUNCTIONAL_REGRESSION_REPORT.md, HIGH_PRIORITY_VERIFICATION_REPORT.md, REGRESSION_ANALYSIS_REPORT.md, RELEASE_AUDIT_RESOLUTION.md, RELEASE_AUDIT_V1.md, etc.) to `docs/archive/`

**Notes:** Keep at root only: AGENTS.md, CHANGELOG.md, LICENSE, README.md, go.mod, go.sum, Makefile, .gitignore, .goreleaser.yaml, .golangci.yml, install.sh, .env.example, m31a.json

---

## Duplicate/Backup Files

| Option | Description | Selected |
|--------|-------------|----------|
| Keep backups | Preserve webfetch.go.bak, webfetch.go.bak2, webfetch.go.patch, session.json.bak | |
| Delete & ignore | Remove backup files, add patterns to .gitignore | ✓ |

**User's choice:** Delete backup files and add patterns to .gitignore (`*.bak*`, `*.patch`, `session.json.bak`, `coverage.out`, `*.out`)

**Notes:** These are artifacts should be ignored by git

---

## Documentation Consolidation

| Option | Description | Selected |
|--------|-------------|----------|
| Merge Wiki + /docs/ | Consolidate M31A.wiki/ into /docs/ | |
| Keep both | GitHub Wiki for web visitors, /docs/ for repo browsers | ✓ |
| Delete one | Remove either Wiki or /docs/ | |

**User's choice:** Keep both — they serve different audiences (Wiki for GitHub web visitors, /docs/ for local repo browsing)

**Notes:** Also keep `.planning/codebase/` analysis files in place

---

## Package Reorganization (Major Redesign)

| Option | Description | Selected |
|--------|-------------|----------|
| Minor cleanup | Just remove duplicates, fix naming | |
| Layered architecture (6 layers) | Core, Engine, UI, Integrations, Tools, Infrastructure | ✓ |
| Alternative layering | Different layer boundaries | |

**User's choice:** Adopt 6-layer architecture:
1. `internal/core/` — types, errors, config, constants
2. `internal/engine/` — workflow, taskrunner, bisect, rollback, session, compaction, narrative, decision, tokens
3. `internal/ui/` — tui (all screens, components, layout, theme, streaming)
4. `internal/integrations/` — provider, git, keychain, shell, context, history, ledger, metrics, logging, autodream, arbitrage, codeintel
5. `internal/tools/` — all 18 tools + dispatcher, permissions, concurrency
6. `internal/infrastructure/` — fileutil, retry, errors (public), testutil

**Notes:** This affects all 37 current packages and all 483 Go source files' import paths

---

## Test Artifacts & Config

| Option | Description | Selected |
|--------|-------------|----------|
| Keep current | e2e_test.go at root, testutil in internal/, .env.test at root | |
| Consolidate under tests/ | Move e2e_test.go → tests/e2e/, testutil → tests/testutil/, .env.test → tests/.env.test | ✓ |

**User's choice:** Consolidate test infrastructure under `tests/` at root level

**Notes:** Cleaner separation of test infrastructure from production code

---

## the agent's Discretion

- Exact file-by-file mapping from old to new package locations (37 packages → 6 layers)
- Specific .gitignore patterns for backup/artifact files
- Whether to create docs/archive/ or use existing docs/ subdirectory structure
- Update all import paths across 483 Go source files to match new package locations

---

## Deferred Ideas

None — discussion stayed within phase scope.
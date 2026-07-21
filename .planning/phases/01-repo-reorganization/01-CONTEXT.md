# Phase 1 Context — Repo Reorganization

**Project:** M31A — Project Summary  
**Phase:** 1 — Repo Reorganization  
**Date:** 2026-07-21  
**Status:** Context gathered — ready for planning

---

## Domain

**What this phase delivers:** A professionally organized Go repository structure with:
- Clean root directory (only standard project files)
- Archived audit/planning documents
- Consolidated test infrastructure
- Major package reorganization using layered architecture (37 → 6 layers)
- No backup/artifact files tracked in git

**Scope boundaries:**  
✅ IN: File moves, package renames, import path updates, .gitignore updates, doc reorganization  
❌ OUT: New features, new tools, new providers, behavioral changes, API changes

---

## Decisions Captured

### Root-Level File Cleanup

| File / Pattern | Decision | Destination |
|----------------|----------|-------------|
| DECOMPOSITION_PLAN.md, DX_AUDIT.md, FUNCTIONAL_REGRESSION_REPORT.md, HIGH_PRIORITY_VERIFICATION_REPORT.md, REGRESSION_ANALYSIS_REPORT.md, RELEASE_AUDIT_RESOLUTION.md, RELEASE_AUDIT_V1.md, and ~13 similar audit/planning docs | Move | `docs/archive/` |
| AGENTS.md, CHANGELOG.md, LICENSE, README.md | Keep at root | — |
| go.mod, go.sum, Makefile, .gitignore | Keep at root (standard Go) | — |
| .goreleaser.yaml, .golangci.yml, install.sh, .env.example, m31a.json | Keep at root | — |

### Duplicate / Backup Files

| File | Decision |
|------|----------|
| internal/tools/search/webfetch.go.bak | Delete + add pattern to .gitignore |
| internal/tools/search/webfetch.go.bak2 | Delete + add pattern to .gitignore |
| internal/tools/search/webfetch.go.patch | Delete + add pattern to .gitignore |
| .m31a/session.json.bak | Delete + add pattern to .gitignore |
| .m31a/messages.json (runtime) | Already gitignored — confirm |
| cmd/m31a/.m31a/ (runtime) | Already gitignored — confirm |
| coverage.out | Delete + add pattern to .gitignore |

### Documentation Consolidation

| Source | Decision | Notes |
|--------|----------|-------|
| M31A.wiki/ (36 files) | Keep both | Wiki serves GitHub web visitors; /docs/ serves local repo browsing |
| /docs/ (13 files) | Keep as-is | Current user-facing documentation |
| .planning/codebase/ (7 files) | Keep in place | GSD planning artifacts, not user docs |

### Package Reorganization (Major)

**Approved 6-layer architecture:**

| New Layer | Contains (from current structure) | Approx. Files |
|-----------|----------------------------------|---------------|
| `internal/core/` | types, errors, config, constants (shared vocabulary) | ~10 |
| `internal/engine/` | workflow, taskrunner, bisect, rollback, session, compaction, narrative, decision, tokens | ~100 |
| `internal/ui/` | tui (all screens, components, layout, theme, streaming) | ~250 |
| `internal/integrations/` | provider, git, keychain, shell, context, history, ledger, metrics, logging, autodream, arbitrage, codeintel | ~80 |
| `internal/tools/` | all 18 tools + dispatcher, permissions, concurrency, subpackages | ~110 |
| `internal/infrastructure/` | fileutil, retry, errors (public pkg/errors), testutil | ~20 |

**Total affected:** All 483 Go source files require import path updates

### Test Infrastructure Consolidation

| Current Location | New Location |
|------------------|--------------|
| e2e_test.go (root) | tests/e2e/e2e_test.go |
| internal/testutil/ (16 files) | tests/testutil/ |
| .env.test (root) | tests/.env.test |

---

## Canonical References

| Ref | Path | Purpose |
|-----|------|---------|
| Project summary | .planning/PROJECT.md | Project context for all agents |
| Roadmap | .planning/ROADMAP.md | Phase 1 goal and scope |
| Current structure | .planning/codebase/STRUCTURE.md | 257-line directory analysis |
| Architecture concerns | .planning/codebase/CONCERNS.md | Known bugs, security, debt |
| Code conventions | .planning/codebase/CONVENTIONS.md | Formatting, errors, naming, patterns |
| AGENTS.md | AGENTS.md | Build/test/lint commands, architecture rules |

---

## Code Context (Reusable Assets & Patterns)

| Asset | Location | Reusable For |
|-------|----------|--------------|
| Layered import pattern | STRUCTURE.md:93-106 | Enforcing dependency direction in new layers |
| Error handling conventions | CONVENTIONS.md:13-131 | Consistent error wrapping in moved files |
| Package naming | STRUCTURE.md:108-120 | New package names follow conventions |
| Test patterns | CONVENTIONS.md:162-166 | Table-driven tests, subtests in new locations |
| Dependency injection | CONVENTIONS.md:287-306 | Interfaces for external deps in integrations layer |
| CGO_ENABLED=0 | AGENTS.md, Makefile | All builds must remain static |

---

## Deferred Ideas

None — discussion stayed within phase scope.

---

## Next Steps

1. **Plan phase** — Run `/gsd-plan-phase 1` to create detailed execution plan
2. **Plan will include** — Wave-based parallel file moves, import path updates, verification steps
3. **Execution** — Run `/gsd-execute-phase 1` with checkpoints after each layer
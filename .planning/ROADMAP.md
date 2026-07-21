# Roadmap — M31A

## Phase 1: Repo Reorganization
**Goal:** Reorganize files and complete repo structure as a professional-grade open source project.

**Scope:**
- Audit current directory layout against Go project conventions
- Reorganize internal packages for clarity and maintainability
- Consolidate scattered root-level files (reports, docs) into proper locations
- Clean up duplicate/backup files (e.g., `webfetch.go.bak`, `webfetch.go.bak2`)
- Ensure consistent naming, package boundaries, and import paths
- Update documentation to reflect new structure
- Verify all tests pass after reorganization

**Out of scope:** New features, new tools, new providers, behavioral changes.

**Plans:** 6 plans

Plans:
- [ ] 01-01-PLAN.md — Foundation layer: move types, errors, config, fileutil, retry to core/ and infrastructure/
- [ ] 01-02-PLAN.md — Engine layer: move workflow, taskrunner, bisect, rollback, session, compaction, narrative, decision, tokens, coordinator to engine/
- [ ] 01-03-PLAN.md — Integrations layer: move provider, git, keychain, shell, context, history, ledger, metrics, logging, log, autodream, arbitrage, codeintel, skills to integrations/
- [ ] 01-04-PLAN.md — UI layer: move tui to ui/tui/
- [ ] 01-05-PLAN.md — Global import update: sed replacements for all remaining import paths + compile check
- [ ] 01-06-PLAN.md — Root cleanup: test consolidation, doc archival, backup deletion, final test

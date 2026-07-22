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

## Phase 2: Fix Test Hanging and CI Issues

**Goal:** Investigate and resolve test hanging issues and CI pipeline problems to ensure reliable test execution.

**Scope:**

- Deep investigation of test hanging root causes
- CI pipeline fixes and reliability improvements
- Test reliability and stability improvements
- Identify and fix flaky tests
- Improve test execution timeouts and resource management

**Out of scope:** New features, new tools, new providers, behavioral changes.

**Plans:** 3 plans

Plans:
**Wave 1**

- [x] 02-01-PLAN.md — Test hanging investigation + timeout infrastructure (Makefile timeouts, testtimeout package, full test suite run)
- [x] 02-02-PLAN.md — CI pipeline fixes (GitHub Actions timeouts, Go version pinning, module caching, artifact collection)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 02-03-PLAN.md — Fix hanging tests + CI detection (resolve identified issues, add isCI() helper)

## Phase 3: Internal Package Organization

**Goal:** Reorganize `internal/tools/` and `internal/ui/tui/` directories for better maintainability, clearer boundaries, and professional code structure.

**Scope:**

- Audit current flat structure of `internal/tools/` (71 files) and `internal/ui/tui/` (182 files)
- Group related files into logical subpackages
- Establish clear package boundaries and responsibilities
- Ensure import paths remain clean and logical
- Verify all tests pass after reorganization

**Out of scope:** New features, new tools, behavioral changes, external API changes.

**Plans:** 3 plans

Plans:
**Wave 1**

- [ ] 03-01-PLAN.md — Tools reorganization: move git, todo, codeanalysis, network tools to subdirectories

**Wave 2** *(blocked on Wave 1 completion)*

- [ ] 03-02-PLAN.md — TUI reorganization: group files by responsibility (core/, handlers/, input/, update/, routing/)

**Wave 3** *(blocked on Wave 2 completion)*

- [ ] 03-03-PLAN.md — Import path updates and test consolidation: update all imports, consolidate tests to tests/ directory

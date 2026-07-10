# M31A Wiring Audit

## What This Is

A complete wiring audit of the M31A project — a Go-based TUI application built with Bubble Tea (Elm architecture) featuring a 7-phase workflow engine, multi-provider LLM integration (OpenRouter, Zen, Nvidia), 18 built-in tools with dispatcher, session persistence with checkpoints/ledger/rollback, and cross-platform static binary distribution. This project audits every module, package, interface, service, workflow, configuration, event, command, state transition, provider, tool, UI component, and runtime system to verify correct connections and identify all broken wiring.

## Core Value

Every input must have a path. Every output must have a consumer. Every abstraction must have an implementation. Every implementation must actually be used.

## Business Context

- **Customer**: Internal development team maintaining M31A
- **Revenue model**: N/A (open-source tool)
- **Success metric**: Zero critical/high wiring issues; 75% overall coverage, 90% for pkg/taskrunner, pkg/bisect, pkg/rollback
- **Strategy notes**: This audit enables confident refactoring and feature development by establishing a verified wiring baseline

## Requirements

### Validated

(None yet — audit to validate)

### Active

- [ ] **WIRING-01**: Build complete dependency graph of the project covering all packages, workflow phases, providers, tools, UI components, configuration, and persistence layers
- [ ] **WIRING-02**: Verify every input has a traced path to its consumer and every output has a traced path from its producer
- [ ] **WIRING-03**: Verify every abstraction has a verified implementation and every implementation is verified as actually used
- [ ] **WIRING-04**: Produce comprehensive audit report with all 20 sections (Executive Summary, Dependency Graph, Startup Wiring, Workflow Wiring, UI Wiring, Tool Wiring, Provider Wiring, Configuration Wiring, Persistence Wiring, Package Dependencies, Missing Registrations, Dead Code, Unreachable Code, Documentation Drift, Missing Tests, Severity Matrix, Exact File Locations, Root Cause Analysis, Recommended Fix, Priority Order)
- [ ] **REMED-01**: Fix all Critical severity wiring issues
- [ ] **REMED-02**: Fix all High severity wiring issues
- [ ] **REMED-03**: Fix all Medium severity wiring issues
- [ ] **REMED-04**: Fix all Low severity wiring issues
- [ ] **REMED-05**: Add missing tests for every public package, exported API, workflow phase, provider, tool, and integration identified as uncovered

### Out of Scope

- [ ] Code quality review (style, naming, optimization) — Wiring audit only; architecture reviewed only if it causes broken wiring
- [ ] Performance optimization — Unless it breaks wiring
- [ ] Feature development — This is audit/remediation, not feature work
- [ ] Refactoring for maintainability alone — Only fix wiring issues
- [ ] Security audit beyond wiring — Use `/gsd-secure-phase` for that

## Context

- Go 1.25+, CGO_ENABLED=0 (static binary hard constraint)
- Bubble Tea TUI with Elm architecture — all state mutations through Update() only, never mutate AppState from goroutines
- Workflow engine: 7 phases (Initialize → Discuss → Plan → Execute → Verify → Runtime → Ship)
- Provider layer: 3 providers (OpenRouter, Zen, Nvidia) with dynamic model discovery
- Tools: 18 built-in tools in internal/tools/, registered in defaults.go, dispatcher handles permissions/rate-limiting/concurrency
- Dependency rule: pkg/ must NOT import internal/ (enforced by Go module system)
- internal/types is shared type vocabulary across all layers
- Keychain via pkg/keychain/ — API keys never written to disk in plaintext
- Cross-compilation targets: linux/amd64, linux/arm64, darwin/amd64, darwin/arm64, windows/amd64
- Coverage targets: 75% overall, 90% for pkg/taskrunner, pkg/bisect, pkg/rollback

## Constraints

- **Tech stack**: Go 1.25+, Bubble Tea, Cobra (CLI), multiple LLM provider APIs
- **Timeline**: Phase 1 (audit) then Phase 2 (remediation) — no fixed dates
- **Dependencies**: Existing codebase, no external dependencies beyond what's in go.mod
- **Compatibility**: Must maintain CGO_ENABLED=0, cross-compilation targets
- **Performance**: Static binary requirement; no CGO anywhere
- **Security**: API keys through OS keychain only; .env files gitignored

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Wiring audit as 2-phase project (audit → remediate) | Separation of concerns; audit must be complete before fixing | — Pending |
| Use GSD workflow for project management | Consistent with M31A's existing GSD setup | — Pending |
| 75%/90% coverage targets | Match existing Makefile targets | — Pending |
| No CGO anywhere | Hard constraint from AGENTS.md | — Pending |

---

*Last updated: 2026-07-10 after initialization*

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd-transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd-complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state (users, feedback, metrics)
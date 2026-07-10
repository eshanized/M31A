# Roadmap: M31A Wiring Audit

## Overview

This roadmap covers a complete wiring audit of the M31A codebase (Go TUI with Bubble Tea, 7-phase workflow engine, 3 LLM providers, 18 tools, session persistence) in two phases: Phase 1 performs the comprehensive audit producing a 20-section report; Phase 2 remediates all findings by severity order (Critical → High → Medium → Low) and adds missing test coverage.

## Phases

- [x] **Phase 1: Wiring Audit** - Complete dependency graph and wiring verification of all modules, packages, interfaces, services, workflows, configuration, events, commands, state transitions, providers, tools, UI components, and runtime systems (completed 2026-07-10)
- [ ] **Phase 2: Wiring Remediation** - Fix all wiring issues by severity order and add missing test coverage for uncovered public packages and exported APIs

## Phase Details

### Phase 1: Wiring Audit

**Goal**: Produce comprehensive wiring audit report with 20 sections covering every connection in the M31A codebase

**Depends on**: Nothing (first phase)

**Requirements**: [WIRING-01, WIRING-02, WIRING-03, WIRING-04]

**Success Criteria** (what must be TRUE):

1. Complete dependency graph exists mapping every module, package, interface, service, workflow phase, configuration, event, command, state transition, provider, tool, UI component, and runtime system
2. Every input has a traced path to its consumer; every output has a traced path from its producer
3. Every abstraction has a verified implementation; every implementation is verified as actually used
4. Report with all 20 sections delivered: Executive Summary, Dependency Graph Overview, Startup Wiring Issues, Workflow Wiring Issues, UI Wiring Issues, Tool Wiring Issues, Provider Wiring Issues, Configuration Wiring Issues, Persistence Wiring Issues, Package Dependency Issues, Missing Registrations, Dead Code, Unreachable Code, Documentation Drift, Missing Tests, Severity Matrix, Exact File Locations, Root Cause Analysis, Recommended Fix, Priority Order

**Plans**: 3/3 plans complete

Plans:

- [x] 01-01-PLAN.md: Build complete dependency graph and trace startup wiring (cmd/m31a/main.go → config, logger, keychain, providers, dispatcher, workflow engine, session, TUI, shutdown, signals, background workers, hot reload)
- [x] 01-02-PLAN.md: Trace workflow engine (7 phases), provider layer, tools dispatcher, Bubble Tea state machine, configuration, persistence, pkg/ public APIs, subagents
- [x] 01-03-PLAN.md: Generate 20-section wiring audit report with severity matrix and remediation priority

### Phase 2: Wiring Remediation

**Goal**: Fix all wiring issues identified in Phase 1 by severity order and add missing test coverage

**Depends on**: Phase 1

**Requirements**: [REMED-01, REMED-02, REMED-03, REMED-04, REMED-05]

**Success Criteria** (what must be TRUE):

1. All Critical severity wiring issues fixed and verified
2. All High severity wiring issues fixed and verified
3. All Medium severity wiring issues fixed and verified
4. All Low severity wiring issues fixed and verified
5. Missing tests added for every public package, exported API, workflow phase, provider, tool, and integration identified in audit; coverage targets met (75% overall, 90% for pkg/taskrunner, pkg/bisect, pkg/rollback)

**Plans**: 5 plans

Plans:

- [ ] 02-01: Fix Critical severity wiring issues (missing registrations, orphan interfaces, nil paths, impossible execution paths, broken DI, lifecycle leaks, goroutine leaks, context leaks, channel leaks)
- [ ] 02-02: Fix High severity wiring issues (dead code, unreachable code, duplicate systems/providers/config/tools, incorrect dependency direction, startup/shutdown ordering bugs, race conditions, resource leaks)
- [ ] 02-03: Fix Medium severity wiring issues (unused registrations, implementations never instantiated, events never consumed, messages never handled, handlers never called, commands never triggered, partially implemented features, abandoned features)
- [ ] 02-04: Fix Low severity wiring issues (stale architecture docs, documentation drift, hidden coupling, minor ownership/lifecycle issues)
- [ ] 02-05: Add missing tests for all uncovered public packages, exported APIs, workflow phases, providers, tools, and integrations; verify coverage targets met

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Wiring Audit | 3/3 | Complete   | 2026-07-10 |
| 2. Wiring Remediation | 0/5 | Not started | - |

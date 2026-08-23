# M31A — Project Instructions

## Project Context
See: `.planning/PROJECT.md` (updated 2026-08-23)

**Core value:** A persistent software-engineering agent runtime that turns natural-language intent into verified, resumable engineering work.

## Current Focus
Phase 1: Foundation — Domain Model & Event Store

## Key Documents
- `.planning/CONTEXT_M31A.md` — Canonical architecture, domain model, runtime model, safety model
- `.planning/UI-SPEC.md` — 36-screen TUI specification
- `.planning/REQUIREMENTS.md` — 118 v1 requirements with traceability
- `.planning/ROADMAP.md` — 12 phases, fine granularity
- `.planning/STATE.md` — Project memory and session continuity
- `.planning/research/SUMMARY.md` — Research synthesis

## Workflow Configuration
- **Mode:** YOLO (auto-approve)
- **Granularity:** Fine (12 phases)
- **Execution:** Parallel
- **Research:** Enabled before each phase
- **Plan Check:** Enabled
- **Verifier:** Enabled
- **Drift Guard:** Enabled
- **Model Profile:** Adaptive (role-based)

## Key Constraints
- Go 1.26+, CGO_ENABLED=0, static binaries
- Six-plane architecture: Interaction, Intelligence, Engineering, Execution, Assurance, Memory
- TUI is pure projection (Bubble Tea) — no domain logic in UI
- SQLite event sourcing for durable state
- Capability-based permissions evaluated at execution time
- Worktree-first execution for safety
- Evidence-backed verification (no thresholds)
- Provider-agnostic LLM interface (NVIDIA Build for v1)

## Next Action
Run `/gsd-plan-phase 1` to create detailed plan for Foundation phase.

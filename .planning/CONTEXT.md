# Phase 0 — Context & Decisions

## What Phase 0 Establishes
- AGENTS.md: OpenCode project memory with all architectural constraints
- opencode.json: Instruction file chain for all future sessions
- Real Go interface files: internal/errors/, internal/types/, internal/provider/interface.go, internal/tools/interface.go, internal/config/types.go
- docs/INTERFACES.md: Mirrors Go interfaces for LLM context in future sessions
- docs/TYPES.md: Error sentinels, env vars, constants reference
- docs/ARCHITECTURE.md: Package dependency and data flow
- Go module structure: directory skeleton, go.mod, Makefile
- CI/CD: GitHub Actions matrix, goreleaser, golangci-lint
- Logging: structured slog with rotation to ~/.m31a/m31a.log

## Interface Decisions
See internal/types/ and internal/provider/interface.go for canonical definitions.
docs/INTERFACES.md mirrors these for human/LLM readability.

## State Persistence Decision
All workflow state stored as human-readable Markdown + JSON in
~/.m31a/sessions/<id>/planning/. Never binary formats. Always resumable.

## Token Estimation Decision
Two-phase: client-side tiktoken-go during streaming, server calibration
from final SSE chunk usage field. EMA alpha=0.3 correction factor per
model family.

## Context Pruning Decision
Each workflow phase discards prior conversation. Reads only structured
state files (PROJECT.md, TASKS.md, STATE.md) plus system prompt. No
conversation history carried between phases.

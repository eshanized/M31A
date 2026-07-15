# M31A — Project Context

## Vision
M31A is a terminal-based AI coding agent that guides tasks through a structured seven-phase workflow: **Initialize → Discuss → Plan → Execute → Verify → Runtime → Ship**. It combines a terminal user interface (TUI) built with Bubble Tea with an autonomous workflow engine that can plan, execute, and verify coding tasks.

## Problem Statement
M31A addresses three interconnected gaps in current AI coding tools:
1. **Missing workflow structure** — Most AI coding assistants are chat-based with no built-in process for task decomposition, verification, or shipping
2. **Poor TUI/visual feedback** — CLI tools lack rich visual interfaces for monitoring multi-step workflows, tool execution, and context
3. **Tool fragmentation** — Developers context-switch between editor, terminal, browser, and AI chat; M31A unifies these in a single TUI

## Current State
- **Codebase**: Go 1.25+, Bubble Tea (Elm architecture), ~30K lines
- **Architecture**: Clean separation — `cmd/` entry, `internal/tui/` for Bubble Tea models, `internal/workflow/` for engine, `internal/provider/` for LLM providers, `internal/tools/` for 18 built-in tools
- **Status**: Both core workflow logic AND TUI rendering have issues. All TUI screens appear blank when running the binary.
- **Known working**: Unit tests pass (screen rendering, view logic). Headless mode works for simple prompts.
- **Known broken**: TUI screens render blank in interactive terminal but tests pass. Cannot complete full workflow end-to-end.

## Target Users
- Solo developers and small teams wanting AI-assisted coding with process discipline
- Users comfortable in terminal who want visual workflow tracking
- Projects requiring audit trails (decisions, plans, verification records)

## Success Criteria (v1)
1. **TUI**: Run `m31a` → see FirstRun wizard → configure provider → see Home screen → send prompts → see responses
2. **Workflow**: Run `m31a --goal "..."` → completes all 7 phases → produces verified, committed code
3. **Reliability**: No blank screens, no crashes, proper error messages

## Technical Constraints
- Go 1.25+, CGO_ENABLED=0 (static binary)
- Bubble Tea single-threaded contract — all state mutations in Update()
- Three LLM providers: OpenRouter, Zen, Nvidia (dynamic model discovery)
- API keys via OS keychain, never plaintext on disk
- Cross-compile targets: linux/amd64, linux/arm64, darwin/amd64, darwin/arm64, windows/amd64

## Key Decisions (Locked)
- Framework: Bubble Tea (Elm architecture)
- Language: Go
- Workflow: 7 fixed phases
- Providers: Dynamic model discovery, never hardcoded
- Config: TOML file + OS keychain for secrets
- Testing: Race-enabled, 75% overall coverage, 90% for critical packages

## Deferred Ideas
- Light/auto themes (currently dark only)
- Windows ARM64 target
- Plugin system for custom tools
- Multi-repo workspace support

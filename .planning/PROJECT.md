# M31A UI Refactor Project

## What This Is

M31A is a terminal-based AI coding assistant built with Bubble Tea (Elm architecture). The UI has critical issues: the REPL screen is blank (no messages, no keyboard input), and model selection is hardcoded throughout the codebase instead of being dynamically fetched from providers.

## Core Value

Fix the REPL screen so users can interact with the AI assistant, and make model selection dynamic and configurable.

## Requirements

### Validated

- ✓ App compiles successfully
- ✓ Existing tests pass

### Active

- [ ] Fix blank REPL screen - messages should appear and keyboard input should work
- [ ] Fix hardcoded model names - models should be dynamically fetched from providers
- [ ] Fix hardcoded functions - provider wiring should be configurable
- [ ] Ensure both TUI and provider layers work correctly together

### Out of Scope

- New features beyond fixing existing functionality
- Performance optimizations
- UI redesign (keep existing patterns)

## Context

### Technical Environment

- **Language**: Go 1.25.12
- **Framework**: Bubble Tea (Elm architecture)
- **Package**: `internal/ui/tui/` (~180 files) - largest package
- **Providers**: OpenRouter, Zen, NVIDIA (3 providers)
- **Build**: `CGO_ENABLED=0` (static binary)

### Current Issues

1. **Blank REPL Screen**:
   - Structure renders (borders, layout visible)
   - No messages appear in viewport
   - Keyboard input completely unresponsive
   - Can only exit with Ctrl+C

2. **Hardcoded Models**:
   - Model names hardcoded in test files (gpt-4, claude-3, etc.)
   - Provider abbreviations hardcoded in `helpers_string.go`
   - Model selection not dynamically fetched from providers
   - Configuration not properly wired through the system

### Key Files

- `cmd/m31a/main.go` - Entry point, provider registration
- `internal/ui/tui/app.go` - Main app model, Init() function
- `internal/ui/tui/repl_model.go` - REPL model structure
- `internal/ui/tui/repl_view.go` - REPL view rendering
- `internal/ui/tui/modelselector_model.go` - Model selector UI
- `internal/ui/tui/provider_registration.go` - Provider registration
- `internal/ui/tui/helpers_string.go` - Provider abbreviations

## Constraints

- **Tech Stack**: Must use Bubble Tea framework
- **Build**: Must maintain `CGO_ENABLED=0`
- **Tests**: All existing tests must pass
- **Patterns**: Follow existing codebase conventions

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Full refactor approach | User explicitly requested | — Pending |
| Fix REPL first priority | Most critical user-facing issue | — Pending |
| Both TUI and provider layers | Issues span both areas | — Pending |

---
*Last updated: 2025-07-31 after initialization*
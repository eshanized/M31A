# Changelog

All notable changes to M31A are documented in this file.

## v1.0.0 — 2026-05-29

### Features

- Six-phase workflow: Initialize, Discuss, Plan, Execute, Verify, Ship
- Dual provider support: OpenRouter + OpenCode Zen
- 10-screen Bubble Tea TUI with dark/light themes
- 5 core tools: Bash, FileRead, FileWrite, Glob, Grep
- Model selector with fuzzy search and cost comparison
- Auto-fallback on provider failure (429/503)
- Cross-session learning ledger
- Commit rollback chain with git bisect integration
- AutoDream context consolidation
- Model arbitrage for cost optimization
- Session persistence and resume
- OS keychain integration (Linux/macOS/Windows)
- 16+ slash commands
- First-run setup wizard
- Settings screen with 6-tab config editor
- Permission modal for dangerous tool operations

### Technical

- Static binary (`CGO_ENABLED=0`) for Linux, macOS, Windows
- ~24,000 LOC Go with 50+ test files
- Structured logging (slog) with rotation
- No telemetry, no analytics, no phone-home
- Cross-platform build verification (5 platforms)
- CI pipeline with lint, test, and build matrix

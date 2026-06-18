# Changelog

All notable changes to M31 Autonomous will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

## [1.1.0] - 2026-06-18

### Fixed
- **errcheck**: Wrap `f.Close()` return value in `codecomplexity.go`
- **govet**: Resolve variable shadowing in `execute.go` (renamed inner `err` to `unmarshalErr`)
- **ineffassign**: Remove dead `w` assignment in `chathistory_view.go`
- **ineffassign**: Remove dead `w` assignment in `helpers.go`
- **staticcheck**: Simplify redundant type assertion in `interface_test.go`
- **staticcheck**: Simplify redundant type assertion in `tooldefs_test.go` and remove unused import
- **staticcheck**: Fill empty branch with assertion in `commands_all_test.go`
- **staticcheck**: Simplify redundant type assertion in `git_test.go`

### Removed
- Remove unused `renderPlaceholder` function from `helpers.go`
- Remove unused `renderContextMeter` duplicate from `repl_view.go`
- Remove unused `phaseElapsed` field from `SidebarModel`

### Added
- **WP-A03**: GitClient interface in internal/types/ for testability
- **WP-D02**: Package-level doc.go files for all 7 public packages
- **WP-D03**: This CHANGELOG.md
- **WP-M08**: Documented all unsafe.Pointer usages in Windows keychain
- **WP-P02**: Configurable session list cache TTL via ManagerOpts
- **WP-S01**: Comprehensive test suite for API key redaction (14 subtests)
- **WP-S03**: Expanded pass CLI regex to allow digits and hyphens
- **WP-L02**: Centralized timeout constants (DefaultSessionCacheTTL, DefaultVerifyTimeout, DefaultFetchModelsTimeout)
- **WP-L08**: Added documentation to firstrunpreview dev tool
- NVIDIA NIM provider integration
- Command palette with fuzzy search
- Chat history model
- Code complexity analysis tool (`/complexity`)
- Sidebar todo mode, phase pipeline, and metrics tracking
- Mouse wheel support across all viewports
- Factory reset flow via `/reset` command
- New slash commands (exit, chat, flush, search, about, keychain, dream)
- Panic recovery across all goroutines
- Thread-safety fixes for caches and maps
- Provider error detection for NVIDIA/OpenAI-compatible streams
- Agent progress tracking and TTL-based file watcher

### Changed
- **WP-D01**: Updated AGENTS.md package layout to reflect main.go reality

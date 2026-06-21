# Changelog

All notable changes to M31 Autonomous will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Released]

## [1.2.0] - 2026-06-21

### Added
- LLM-based intent classification for REPL input routing
- Comprehensive session observability pipeline (metrics)
- Chunked plan generation with outline and wave expansion
- Pre-plan research step and pre-ship checklist with memory flock
- Deep project analysis and environment preflight
- Execute preflight, quality gates, and loop detection
- Discuss quality checking and completeness scoring
- Plan checker with revision loop and coverage gates
- Structured verification report generation
- Prompt templates and extended PromptRegistry
- Workflow enhancement feature flags and event message types
- Auto-compression for context window overflow
- Context overflow detection patterns for providers
- Context usage sparkline in header context meter
- Visual screen transitions with slide-left/right and cross-fade effects
- Improved toast notification styling with rounded cards and depth-based shadows
- Incremental render diff engine using ANSI cursor positioning
- Flex box model with row/column solver, card renderer, modal overlay, and Z-order stack compositing
- Base Context/Component interface, fuzzy search, focus ring, empty state, shortcut tips, and virtual viewport
- High-contrast accessibility theme and StyleCache for pre-computed lipgloss styles
- Screen reader announcement helpers using iTerm2 protocol
- `reduced_motion` accessibility option in UIConfig
- Subagent depth limit (`MaxAgentDepth=2`)
- Subagent token budget enforcement and workspace context in system prompt
- Subagent orphaned worktree directory cleanup
- Dispatcher concurrency limit and per-risk-level rate limits
- `ToolError` type for structured error returns with hints
- FileMove context cancellation check
- FileList sorting control
- Glob file type filtering
- Grep context lines and fixed-string search
- FileRead line-level offset/limit for efficient partial reads
- Edit 7-strategy cascade, replace-all, and collision-safe backups
- Bash dangerous command blocklist for defense-in-depth
- WebSearch DNS cache to prevent TOCTOU rebinding
- CodeComplexity polyglot support (multiple languages)

### Fixed
- Drain stale responses from channels on dispatcher stop
- Short-circuit fallback search on first live provider
- Use staleTTL for cache expiry check
- Recursively skip empty SSE events
- Fix `/flush` to use `tea.ClearScreen` instead of screen switch
- Log subagent cleanup errors and fix transition tick to use configurable FPS

### Changed
- Unified file locking abstraction for cross-platform support
- Optimize import graph and symbol indexing
- Replace `WriteString` with `fmt.Fprintf` in metrics and workflow
- Auto-truncation to preflight context check
- Remove redundant agent tool progress tracking from sidebar
- Extract `forwardMsgToScreen` helper
- Consolidate subagent budget exhaustion messages
- Update README workflow, features, and docs for major enhancements

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

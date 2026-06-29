# Changelog

All notable changes to M31 Autonomous will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Released]

## [1.5.0] - 2026-06-29

### Added
- **Decision transparency**: In-memory decision logger with channel-backed buffered writes, ring buffer overflow, and redaction of sensitive patterns (API keys, emails, IPs)
- **Project knowledge**: In-memory knowledge store tracking conventions, patterns, facts, and per-file intelligence with growth caps and LLM context injection via DynamicContextRegistry
- **Checkpoint resume**: Basic checkpoint data persistence (phase, goal, plan version, decisions) with disk-backed session checkpoints
- **Self-heal explanation**: HealReport struct recording error type, strategy, files used, and duration for each self-healing attempt
- **File-level selective rollback**: ChangedFiles, FileDiff, and RevertFiles for reverting individual files to a previous commit state
- **Decision log browser**: /decisions screen for browsing the in-memory decision log

### Fixed
- **provider**: Context-exceeded detection for "input...exceeds" provider error patterns (was using literal substring match on regex-like pattern)
- **workflow**: Data race in concurrent task tool call counting (totalToolCalls/groupToolCalls modified without synchronization)
- **workflow**: Phase Transition() now protected by mutex to prevent interleaved checkpoint saves
- **session**: UpdateWorkflowState and RenameSession now acquire file lock to prevent concurrent write corruption
- **ledger**: Cross-process file locking via flock to prevent LEDGER.md corruption from multiple M31A instances
- **ledger**: New() now returns nil on directory creation failure instead of silently producing a broken ledger
- **workflow**: findFreePort TOCTOU race mitigated with retry-and-verify loop

### Changed
- PhaseRuntime uses Verify model config slot (was shared without explicit mapping)
- WorkflowState struct extracted from Engine to group mutable session state

## [1.4.0] - 2026-06-26

### Added
- **TODO cancelled status**: New `cancelled` state for TODO items with refactored file writing
- **TODO sync**: Auto-sync TODO.md from task runner state
- **NVIDIA provider**: NVIDIA NIM provider support in TUI command registry
- **CITATION.cff**: Citation file, instructions, and arXiv paper link
- **Security policy**: GitHub community health security policy

### Fixed
- **security**: Enhanced input validation and resource usage limits
- **workflow**: Enforce FileWrite tool to prevent code-as-text output
- **main**: Improved signal handling and config checks

### Changed
- **provider**: Centralized health check and error handling in BaseClient
- **tui**: Unified UI components and improved dashboard metrics display
- **tui**: Updated screen navigation order and improved REPL screen switch
- **website**: Updated Next.js build instructions and UI components
- **docs(joss)**: Added required sections for JOSS submission
- **test**: Added extensive unit tests for codeintel, config, and context packages
- **chore**: Updated wiki submodule reference, removed obsolete files

## [1.3.0] - 2026-06-24

### Added
- **EFIE backend**: Enhanced File Intelligence Engine with graph analysis, bloom filters, centrality scoring, community detection, and polyglot parsers
- **EFIE integration**: `USE_EFIE` environment toggle for code intelligence backend selection
- **Runtime verification phase**: Dev server lifecycle management with smoke tests and HTTP endpoint validation
- **Home screen**: New UI with logo, prompt input, and shortcut tips
- **Subagent profiles**: Built-in agent profiles with profile resolution, allowlist/denylist filtering, and native streaming tool calls
- **Session coordinator**: Concurrent session control with file locking to prevent corruption
- **Session compaction**: Automatic context compaction with serialization and template-based summarization
- **Dynamic context registry**: Environment and git sources for context injection
- **Retry policy**: Exponential backoff with error classification
- **Skill discovery**: Package for loading and registering skills as slash commands
- **Persistent permissions**: Last-match-wins rule evaluation for tool permissions
- **Output store**: Bounding tool output size to prevent context overflow
- **DevServer tool**: Managing dev servers with process group cleanup (Unix/Windows)
- **HTTPCheck tool**: Validating HTTP endpoints during runtime phase
- **Diff summaries**: Workflow integration for change summarization
- **Agent switching**: Dynamic agent profile switching during workflow execution
- **Compaction config**: Instructions, skills, and compaction configuration sections
- **Website template**: Embedded Next.js website template with shadcn/ui components
- **Post-ship validation**: Content validation, placeholder detection, and HTML checks
- **Model prompt templates**: Per-provider prompt customization
- **Runtime UI**: RuntimeModel, runtime view renderer, and sidebar pipeline phase

### Fixed
- **errcheck**: Wrap `os.RemoveAll` return value in `engine.go`
- **unused**: Remove `nCommunities`, `maxCentrality` fields, `min` function, and `walkDir` function
- **layout**: Fix modal overlay ANSI code corruption in `RenderModalOverlay`
- **workflow**: Fix intent classify stream EOF handling
- **workflow**: Optimize code intel invalidation and add quality gate re-check after heal
- **workflow**: Reset heal counter for verify phase and clean bisect state
- **config**: Warn on unknown top-level config keys
- **provider**: Add model-not-found detection for unavailable/deprecated models
- **tui**: Update dimension handling and permission tick logic

### Changed
- **perf**: Parallelize `BuildGraph` file parsing with worker pool
- **refactor**: Unify shell command execution across platforms
- **refactor**: Clean up code and fix error handling in codeintel
- **config**: Enable all quality features by default
- **workflow**: Improve greenfield project complexity classification and project type detection
- **ci**: Bump `actions/checkout` from v4 to v7

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

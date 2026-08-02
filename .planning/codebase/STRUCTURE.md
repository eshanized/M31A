# Directory Structure

**Last mapped:** 2026-08-02
**Project:** M31 Autonomous (Terminal AI Coding Agent)

## Root Directory Layout

```
M31A/
├── cmd/                    # Application entry points
│   └── m31a/              # Main binary
├── internal/               # Private implementation code
│   ├── core/              # Core types, config, errors
│   ├── engine/            # Workflow engine and session management
│   ├── infrastructure/    # Cross-cutting concerns (retry, file utils)
│   ├── integrations/      # External service integrations
│   ├── tools/             # Built-in tool implementations
│   └── ui/                # Terminal user interface
├── pkg/                    # Public packages (safe for external import)
├── tests/                  # Test utilities and helpers
├── scripts/                # Build and release scripts
├── .github/                # GitHub Actions workflows
├── .m31a/                  # Runtime data (gitignored)
├── dist/                   # Build output (gitignored)
├── go.mod                  # Go module definition
├── go.sum                  # Dependency checksums
├── Makefile                # Build automation
├── README.md               # Project documentation
└── m31a.json               # Application metadata
```

## Detailed Directory Breakdown

### `cmd/m31a/`

Main application entry point.

```
cmd/m31a/
├── main.go                 # Application bootstrap, flag parsing
├── main_test.go            # Integration tests for main
├── usage.go                # Usage/help text generation
└── usage_test.go           # Usage tests
```

### `internal/core/`

Core types, configuration, and error definitions.

```
internal/core/
├── types/                  # Shared type definitions
│   ├── constants.go        # Default values, URLs, timeouts
│   ├── messages.go         # Message types for LLM communication
│   ├── phases.go           # Workflow phase definitions
│   └── tools.go            # Tool-related types
├── config/                 # Configuration loading and validation
│   ├── loader.go           # TOML config file loading
│   ├── types.go            # Config struct definitions
│   └── defaults.go         # Default configuration values
└── errors/                 # Custom error types
    └── errors.go           # Application-specific errors
```

### `internal/engine/`

Workflow engine, session management, and execution logic.

```
internal/engine/
├── workflow/               # Seven-phase workflow engine
│   ├── engine.go           # Main engine implementation
│   ├── execute.go          # Execute phase logic
│   ├── verify.go           # Verify phase logic
│   ├── runtime.go          # Runtime phase logic
│   ├── ship.go             # Ship phase logic
│   ├── retry.go            # Retry logic for failed operations
│   ├── compaction.go       # Context compaction
│   └── templates/          # Project templates
│       └── website-nextjs/ # Next.js website template
├── session/                # Session persistence
│   ├── manager.go          # Session lifecycle management
│   └── types.go            # Session data structures
├── tokens/                 # Token counting and estimation
│   └── estimator.go        # Token estimation for LLM context
├── rollback/               # Git rollback chain management
│   └── chain.go            # Rollback chain implementation
├── decision/               # Decision logging
│   └── logger.go           # Decision audit trail
└── compaction/             # Context window management
    └── compactor.go        # Message compaction logic
```

### `internal/infrastructure/`

Cross-cutting infrastructure concerns.

```
internal/infrastructure/
├── retry/                  # Retry logic with backoff
│   └── policy.go           # Retry policy configuration
├── ratelimit/              # Rate limiting
│   └── limiter.go          # Token bucket implementation
└── fileutil/               # File system utilities
    └── helpers.go          # Safe file operations
```

### `internal/integrations/`

External service integrations.

```
internal/integrations/
├── provider/               # LLM provider integrations
│   ├── interface.go        # Provider interface definition
│   ├── registry.go         # Provider registration and discovery
│   ├── base_client.go      # Shared HTTP client logic
│   ├── fallback.go         # Provider failover logic
│   ├── model_metadata.go   # Model discovery from APIs
│   ├── cache.go            # Response caching
│   ├── sse.go              # Server-Sent Events handling
│   ├── common.go           # Shared utilities
│   ├── capabilities.go     # Provider capability detection
│   ├── reasoning.go        # Reasoning model support
│   ├── openrouter/         # OpenRouter provider
│   │   └── client.go
│   ├── zen/                # Zen provider
│   │   └── client.go
│   └── nvidia/             # Nvidia provider
│       └── client.go
├── git/                    # Git integration
│   └── client.go           # Git command execution
├── keychain/               # OS keychain integration
│   └── keychain.go         # Secure credential storage
├── ledger/                 # Learning ledger
│   └── ledger.go           # Cross-session learning
├── autodream/              # AutoDream context consolidation
│   └── consolidation.go    # Context optimization
├── codeintel/              # Code intelligence
│   ├── graph.go            # Dependency graph analysis
│   └── bench.go            # Benchmarking support
├── context/                # Context sources
│   └── sources.go          # Dynamic context loading
└── log/                    # Logging infrastructure
    └── logger.go           # Structured logging setup
```

### `internal/tools/`

Built-in tool implementations (18+ tools).

```
internal/tools/
├── defaults.go             # Tool registration
├── dispatcher.go           # Tool execution dispatcher
├── tools.go                # Tool interface definitions
├── fileops/                # File operations
│   ├── fileread.go         # Read files
│   ├── filewrite.go        # Write files
│   ├── edit.go             # Edit files
│   ├── filedelete.go       # Delete files
│   ├── filemove.go         # Move files
│   ├── filelist.go         # List files
│   ├── backup.go           # Backup before edit
│   ├── pathhelpers.go      # Path utilities
│   ├── helpers.go          # General helpers
│   └── constants.go        # File operation constants
├── exec/                   # Command execution
│   ├── bash.go             # Bash command execution
│   ├── bash_unix.go        # Unix-specific bash
│   ├── bash_windows.go     # Windows-specific bash
│   ├── bash_sandbox_*.go   # Platform-specific sandboxing
│   ├── prockill_*.go       # Process killing
│   ├── devserver.go        # Dev server management
│   ├── concurrency.go      # Concurrent execution
│   └── output_store.go     # Output storage
├── search/                 # Search operations
│   ├── websearch.go        # Web search (SearXNG)
│   ├── webfetch.go         # Web content fetching
│   ├── webfetch_html.go    # HTML processing
│   ├── glob.go             # File globbing
│   ├── grep.go             # Content search
│   ├── dns_cache.go        # DNS caching
│   └── ip_filter.go        # IP filtering
├── ai/                     # AI-related tools
│   └── subagent.go         # Subagent orchestration
├── git/                    # Git tools
│   └── git.go              # Git operations
├── network/                # Network tools
│   └── httpcheck.go        # HTTP health checks
├── codeanalysis/           # Code analysis
│   └── analyzer.go         # Language-specific analysis
├── todo/                   # Task management
│   └── todo.go             # Todo list management
└── subagent/               # Subagent infrastructure
    ├── manager.go          # Subagent lifecycle
    ├── loop.go             # Execution loop
    ├── events.go           # Event handling
    ├── profile.go          # Profile management
    └── worktree.go         # Git worktree management
```

### `internal/ui/`

Terminal user interface.

```
internal/ui/
└── tui/                    # Bubble Tea TUI
    ├── app.go              # Main application model
    ├── app_agent.go        # Agent-specific logic
    ├── command_registry.go # Slash command handling
    ├── README.md           # TUI documentation
    └── components/         # Reusable UI components
        ├── permission_desc.go  # Permission descriptions
        └── ...             # Other components
```

### `pkg/`

Public packages (safe for external import).

```
pkg/
└── keychain/               # OS keychain integration
    └── keychain.go         # Cross-platform credential storage
```

### `tests/`

Test utilities and helpers.

```
tests/
├── testutil/               # Test utilities
│   ├── integration/        # Integration test helpers
│   │   └── README.md
│   └── e2e/                # End-to-end test helpers
│       └── README.md
└── fixtures/               # Test data
```

### `scripts/`

Build and release scripts.

```
scripts/
└── validate-release.sh     # Release validation harness
```

## Key Files

### Configuration Files

- `go.mod` - Go module definition and dependencies
- `go.sum` - Dependency checksums
- `Makefile` - Build automation (30+ targets)
- `m31a.json` - Application metadata
- `.github/workflows/ci.yml` - CI/CD pipeline

### Documentation Files

- `README.md` - Main project documentation
- `CONTRIBUTING.md` - Contribution guidelines
- `LICENSE` - MIT license
- `internal/ui/tui/README.md` - TUI documentation
- `tests/testutil/*/README.md` - Test documentation

### Runtime Files (Gitignored)

- `.m31a/` - Runtime data directory
- `dist/` - Build output
- `coverage.out` - Test coverage data
- `coverage.html` - HTML coverage report

## Naming Conventions

### Files

- **Go files:** `snake_case.go`
- **Test files:** `*_test.go`
- **Platform-specific:** `*_unix.go`, `*_windows.go`, `*_linux.go`, `*_darwin.go`

### Directories

- **Lowercase:** `internal/tools/`, `internal/core/`
- **No underscores:** `fileops/` not `file_ops/`

### Packages

- **Single word:** `workflow`, `session`, `provider`
- **Avoid abbreviations:** `fileoperations` not `fileops` (except where established)

## Import Organization

Go files follow standard import grouping:

1. **Standard library**
2. **Third-party packages**
3. **Internal packages**

Example from `cmd/m31a/main.go`:

```go
import (
    // Standard library
    "context"
    "flag"
    "fmt"
    
    // Third-party
    tea "github.com/charmbracelet/bubbletea"
    
    // Internal
    "github.com/eshanized/M31A/internal/core/config"
    "github.com/eshanized/M31A/internal/engine/workflow"
)
```
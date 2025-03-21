# Phase 5: Session State & Configuration — Context

**Gathered:** 2026-05-28
**Status:** Ready for planning
**Source:** Derived from rush/prompt_5.md (implementation spec) + ROADMAP.md + REQUIREMENTS.md

<domain>
## Phase Boundary

This phase adds session persistence, configuration management, OS keychain integration, token estimation, and two new TUI screens. It enables the application to persist state across restarts, securely manage API keys, and estimate/cap context window usage.

**What this phase delivers:**
- Config loading with env var → OS keychain → config file resolution order
- OS keychain integration for Linux (Secret Service + pass CLI fallback), macOS (Keychain Services), and Windows (stub)
- Session lifecycle management (create, load, list, delete, archive) with atomic writes
- Planning file read/write (PROJECT.md, TASKS.md, STATE.md as human-readable Markdown)
- Checkpoint system (max 2 retained, used by /undo)
- Token estimation with tiktoken-go + EMA calibration
- Settings screen (two-column tabbed config editor)
- Resume screen (session browser with bubbles/list)

**What is NOT in scope:**
- Integration of sessions into workflow phases (Phase 6)
- AutoDream context consolidation (Phase 7)
- `/undo` command wiring (Phase 7)
</domain>

<decisions>
## Implementation Decisions

### Config Loader (`internal/config/loader.go`)
- Parse `~/.m31a/config.toml` using `BurntSushi/toml`
- Resolution order: env var (`M31A_OPENROUTER_API_KEY`, `M31A_ZEN_API_KEY`) → OS keychain → config file
- `M31A_CONFIG` env var overrides config file path
- `M31A_THEME` env var overrides theme setting
- `M31A_DEFAULT_MODEL` env var overrides model setting
- Missing config file returns `DefaultConfig()` without creating file (first-run flow handles creation)
- `Save()` uses atomic write pattern (temp file + rename)
- `ResolveAPIKeys()` method on Config struct

### OS Keychain (`pkg/keychain/`)
- Unified `Keychain` interface: `Get(service)`, `Set(service, value)`, `Delete(service)`
- Compile-tag separated implementations: linux, darwin, windows
- Linux: freedesktop Secret Service via `godbus/dbus/v5` with `pass` CLI fallback
- Darwin: `keyring` package (go-keyring), CGO-less
- Windows: stub returning `ErrNotImplemented`
- Service name format: `m31a/<provider>` (e.g., `m31a/openrouter`)
- Sentinel errors: `ErrKeychainUnavailable`, `ErrKeyNotFound`, `ErrNotImplemented`

### Session Lifecycle (`pkg/session/`)
- `Manager` struct with `baseDir = ~/.m31a/sessions`
- Session ID: 8-char hex via `crypto/rand`
- Directory structure: `sessions/<id>/session.json`, `messages.json`, `planning/{PROJECT,TASKS,STATE}.md`
- Session struct embeds `types.Session` with Messages, Tasks, Project fields
- All writes atomic (temp file + rename in same directory)
- Load tolerates missing files (graceful degradation)
- ListSessions returns `[]SessionInfo{ID, Model, Provider, StartedAt, LastModified, MessageCount, Corrupted}`
- ArchiveSession moves to `sessions/archived/<id>/`

### Planning File Format
- PROJECT.md: goal, type, framework, Q&A sections
- TASKS.md: markdown table (ID | Action | Description | Deps | Status | Files)
- STATE.md: Phase, Progress, Last Action, Timestamp (RFC3339)
- Parsers tolerate extra whitespace, blank lines, missing optional sections

### Checkpoint System (`pkg/session/checkpoint.go`)
- Stored as `checkpoint.json` in session directory
- Max 2 checkpoints retained (append new, trim oldest)
- Fields: Phase, Timestamp, MessageCount, TaskCount
- Used by `/undo` to restore previous phase

### Token Estimation (`internal/tokens/`)
- `Estimator` struct with modelID, tokenizer (tiktoken or nil), EMA calibration
- tiktoken-go for GPT/Claude families
- Fallback: `len([]rune(text)) / 4 * 1.3`
- EMA calibration: `factor = alpha*(actual/estimated) + (1-alpha)*previousFactor`
- `FormatUsage()`: `"used / total (XX%)"`
- `ContextWarningBanner()`: styled warning above threshold (default 80%)

### Settings Screen (`internal/tui/settings.go`)
- `SettingsModel` implementing tea.Model
- Two-column layout with tab navigation: General, Provider, Permissions, Features
- API key fields masked by default (`••••••••`)
- "Store in keychain" option for each provider key
- Atomic save to config, Tab cycles tabs, Enter saves, Esc discards

### Resume Screen (`internal/tui/resume.go`)
- `ResumeModel` implementing tea.Model
- Uses `bubbles/list` for session browser
- Sessions sorted by last-modified
- Corrupted sessions show `[!]` badge in red
- Keys: Enter = resume, N = new session, D = delete with confirmation, Esc = back to REPL

### the agent's Discretion
- Exact test helper utilities and mock setup patterns
- Specific table formatting in TASKS.md (column widths, alignment)
- Theme integration details for settings/resume screens
- Error message styling in the TUI context
- Temp file naming convention for atomic writes

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Core Types & Interfaces
- `internal/types/types.go` — Session, Config, WorkflowPhase, Task, Message, ProjectState structs
- `internal/config/types.go` — Full Config struct with TOML tags (already defined)
- `internal/errors/errors.go` — Sentinel errors
- `internal/provider/interface.go` — LLMProvider interface patterns (reference for interface design)

### Tool Patterns (Phase 4 reference for tool implementations)
- `internal/tools/glob.go` — Example of tool struct pattern with Execute method
- `internal/tools/filewrite.go` — Atomic write pattern (temp file + rename)

### TUI Patterns (reference for new screens)
- `internal/tui/repl.go` — Example of Bubble Tea model (Init/Update/View)
- `internal/tui/theme/` — Theme package with Style definitions
- `internal/tui/permission.go` — Modal overlay pattern (reference for settings/resume modals)

### Project Constraints
- `AGENTS.md` — CGO_ENABLED=0, no telemetry, no plaintext API keys
- `docs/ARCHITECTURE.md` — Package dependency graph, data flow
- `docs/INTERFACES.md` — Interface definitions and contracts
- `docs/TYPES.md` — Constants, sentinel errors, enum types

### Dependencies to Add
- `BurntSushi/toml` — TOML config parsing
- `tiktoken-go` — Token estimation for GPT/Claude families
- `godbus/dbus/v5` — Linux Secret Service (freedesktop)
- `keyring` — macOS Keychain Services

### No external specs — requirements fully captured in decisions above
</canonical_refs>

<specifics>
## Specific Requirements from rush/prompt_5.md

### Test Counts
- Config loader: 12 tests
- OS Keychain: 8 tests
- Session lifecycle: 18 tests
- Checkpoint system: 4 tests
- Token estimation: 14 tests
- Settings screen: 6 tests
- Resume screen: 8 tests
- **Total: 62 new tests minimum**

### File Creation Order
1. `pkg/keychain/` — interface, errors, linux, darwin, windows
2. `pkg/session/` — manager, session CRUD, planning files, checkpoint
3. `internal/config/loader.go` — config loader
4. `internal/tokens/` — token estimator
5. `internal/tui/settings.go` — settings screen
6. `internal/tui/resume.go` — resume screen

### Validation Commands
```
go mod tidy
CGO_ENABLED=0 go build -o m31a ./cmd/m31a
go vet ./...
go test -race -cover ./...
```
</specifics>

<deferred>
## Deferred Ideas
- Windows keychain full implementation (stub returning ErrNotImplemented for V1)
- Integration of session lifecycle into workflow engine phases (Phase 6)
- `/undo` command wiring (Phase 7)
- AutoDream consolidation (Phase 7)
</deferred>

---

*Phase: 05-session-state-configuration*
*Context gathered: 2026-05-28 via rush/prompt_5.md derivation*

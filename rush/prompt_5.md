# Phase 5 — Session State & Configuration

You are a senior Go developer implementing Phase 5 of the M31A terminal AI coding assistant.

## Context

M31A is a Go 1.22+ terminal AI coding assistant. We are building it phase by phase. Phases 0-4 are complete with 319 passing tests. This phase adds session persistence, configuration management, OS keychain integration, and token estimation.

**Existing code you MUST NOT break:**
- `internal/provider/` — LLM provider abstraction (Phases 1)
- `internal/tui/` — Bubble Tea app (Phase 2)
- `internal/tui/components/` — Message renderer, tool cards, thinking blocks (Phases 2-3)
- `internal/tools/` — Bash, FileRead, FileWrite, Glob, Grep, Dispatcher (Phase 4)
- `internal/types/` — Core types including `Session`, `Config` structs, `WorkflowPhase`, etc.
- `internal/config/types.go` — Full config struct already defined (TOML tags in place)

**Current state of Phase 5 packages:**
- `pkg/session/` — does not exist yet
- `pkg/keychain/` — does not exist yet
- `internal/state/` — does not exist yet
- `internal/tokens/` — does not exist yet

## What to Build

### 1. Config Loader (`internal/config/loader.go`)

Implement config loading with the resolution order: env var → OS keychain → config file.

```go
func Load(path string) (*Config, error)
func (c *Config) ResolveAPIKeys(keychain KeychainProvider) error
func DefaultConfig() *Config
func (c *Config) Save(path string) error
```

- Parse `~/.m31a/config.toml` using `BurntSushi/toml`
- If file doesn't exist, return `DefaultConfig()` (do NOT create it automatically — first-run flow handles that)
- `ResolveAPIKeys()` checks env vars first (`M31A_OPENROUTER_API_KEY`, `M31A_ZEN_API_KEY`), then keychain, then config file
- `Save()` writes atomically (temp file + rename)
- `M31A_CONFIG` env var overrides the config file path
- `M31A_THEME` env var overrides theme setting
- `M31A_DEFAULT_MODEL` env var overrides model setting

**Tests:** 12 tests covering: default config, env var override, TOML parse, atomic save, missing file, M31A_CONFIG override, key resolution order

---

### 2. OS Keychain (`pkg/keychain/`)

Create compile-tag separated implementations with a unified interface.

**`pkg/keychain/keychain.go` — interface + common types:**
```go
type Keychain interface {
    Get(service string) (string, error)
    Set(service, value string) error
    Delete(service string) error
}
func New() (Keychain, error)
```

**`pkg/keychain/keychain_linux.go`** (build tag `linux`):
- Use freedesktop Secret Service via `godbus/dbus/v5`
- Fallback: if dbus unavailable, use `pass` CLI (`pass show m31a/<service>`)
- Service name: `m31a/<provider>` (e.g., `m31a/openrouter`, `m31a/zen`)

**`pkg/keychain/keychain_darwin.go`** (build tag `darwin`):
- Use `keyring` package (go-keyring) for macOS Keychain Services
- No CGO required

**`pkg/keychain/keychain_windows.go`** (build tag `windows`):
- Stub implementation returning `ErrNotImplemented` (V1 focus is Linux/macOS; Windows keychain is low priority)

**`pkg/keychain/errors.go`:**
```go
var (
    ErrKeychainUnavailable = errors.New("keychain unavailable")
    ErrKeyNotFound         = errors.New("key not found")
    ErrNotImplemented      = errors.New("not implemented on this platform")
)
```

**Tests:** 8 tests covering: set/get/delete roundtrip (mocked dbus), fallback to pass CLI, service name format, error cases

---

### 3. Session Lifecycle (`pkg/session/`)

```go
type Manager struct {
    baseDir string // ~/.m31a/sessions
}

func NewManager(baseDir string) *Manager
func (m *Manager) NewSession(model, provider string) (*Session, error)
func (m *Manager) LoadSession(id string) (*Session, error)
func (m *Manager) ListSessions() ([]SessionInfo, error)
func (m *Manager) DeleteSession(id string) error
func (m *Manager) ArchiveSession(id string) error
```

**Session directory structure:**
```
~/.m31a/sessions/<id>/
├── session.json      # Session metadata
├── messages.json     # Full conversation history ([]types.Message)
└── planning/
    ├── PROJECT.md    # Goal, project type, framework, Discuss Q&A
    ├── TASKS.md      # Task list with dependencies and status
    └── STATE.md      # Current phase, progress, last action
```

**Session struct** (embeds `types.Session`):
```go
type Session struct {
    types.Session
    Messages []types.Message  `json:"messages"`
    Tasks    []types.Task     `json:"tasks"`
    Project  *types.ProjectState `json:"project"`
}
```

- `NewSession()`: generate 8-char ID via `crypto/rand` (hex), create directory, write `session.json` with `StartedAt` timestamp
- `LoadSession()`: parse `session.json`, `messages.json`, `planning/*.md`; reconstruct full state; tolerate missing files
- `ListSessions()`: scan base directory, parse each `session.json`, return `[]SessionInfo{ID, Model, Provider, StartedAt, LastModified, MessageCount, Corrupted bool}`
- `DeleteSession()`: remove directory
- `ArchiveSession()`: move to `~/.m31a/sessions/archived/<id>/`
- All writes atomic (temp file + rename in same directory)
- `SaveMessages()`: write `messages.json` atomically
- `SaveProject()`: write `planning/PROJECT.md` atomically
- `SaveTasks()`: write `planning/TASKS.md` atomically
- `SaveState()`: write `planning/STATE.md` atomically

**Markdown format for planning files:**

`PROJECT.md`:
```markdown
# Project

**Goal:** <goal>
**Type:** <project_type>
**Framework:** <framework>

## Questions

- **Q:** <question> → **A:** <answer>
```

`TASKS.md`:
```markdown
# Tasks

| ID | Action | Description | Deps | Status | Files |
|----|--------|-------------|------|--------|-------|
| 1 | Add | Create main.go | - | done | main.go |
```

`STATE.md`:
```markdown
# State

**Phase:** <phase>
**Progress:** <description>
**Last Action:** <action>
**Timestamp:** <RFC3339>
```

**Tests:** 18 tests covering: new session creation, load roundtrip, list sessions, archive, delete, corrupted session detection, missing files tolerance, atomic writes, markdown parse/write for all three planning files

---

### 4. Checkpoint System (`pkg/session/checkpoint.go`)

```go
type Checkpoint struct {
    Phase       types.WorkflowPhase `json:"phase"`
    Timestamp   time.Time           `json:"timestamp"`
    MessageCount int                `json:"message_count"`
    TaskCount   int                 `json:"task_count"`
}

func (m *Manager) SaveCheckpoint(sessionID string, cp Checkpoint) error
func (m *Manager) LoadCheckpoints(sessionID string) ([]Checkpoint, error)
func (m *Manager) LatestCheckpoint(sessionID string) (*Checkpoint, error)
```

- Store as `checkpoint.json` in session directory
- Retain only last 2 checkpoints (append new, trim oldest if > 2)
- Used by `/undo` to restore previous phase

**Tests:** 4 tests covering: save/load, max 2 retention, latest retrieval, empty state

---

### 5. Token Estimation (`internal/tokens/`)

```go
type Estimator struct {
    modelID     string
    tokenizer   *tiktoken.Tiktoken // nil if unsupported
    emaAlpha    float64
    emaFactor   float64 // calibration factor, starts at 1.0
}

func NewEstimator(modelID string) *Estimator
func (e *Estimator) Estimate(text string) int
func (e *Estimator) Calibrate(estimated int, actual int)
func (e *Estimator) FormatUsage(used int, total int64) string
```

- Use `tiktoken-go` for GPT/Claude tokenizer families
- Fallback for unknown models: `len([]rune(text)) / 4 * 1.3`
- EMA calibration: `emaFactor = emaAlpha * (actual / estimated) + (1 - emaAlpha) * emaFactor`
- `FormatUsage()`: returns `"used / total (XX%)"` string
- `ContextWarningBanner(used int, total int64, threshold float64) string`: returns styled warning when usage exceeds threshold (default 80%)

**Tests:** 14 tests covering: tiktoken estimation, fallback calculation, EMA calibration convergence (< 5% after 3 turns), format usage, context warning banner, empty text, multi-byte characters

---

### 6. Config Screen Integration (`internal/tui/settings.go`)

Create the settings screen that ties into the config system.

```go
type SettingsModel struct {
    config  *config.Config
    theme   theme.Theme
    // ... fields for form state
}

func NewSettingsModel(cfg *config.Config, t theme.Theme) *SettingsModel
func (m *SettingsModel) Update(msg tea.Msg) ([]tea.Cmd, bool)
func (m *SettingsModel) View() string
func (m *SettingsModel) Save() error
```

- Two-column layout with tab navigation: General, Provider, Permissions, Features
- API key fields masked by default (`••••••••`)
- "Store in keychain" option for each provider key
- Atomic save to `~/.m31a/config.toml`
- Keys: `Tab` = cycle tab, `Enter` = save, `Esc` = discard

**Tests:** 6 tests covering: render settings view, tab cycling, key masking, save action, discard on escape

---

### 7. Resume Screen (`internal/tui/resume.go`)

Create the session browser screen.

```go
type ResumeModel struct {
    theme    theme.Theme
    sessions []session.SessionInfo
    selected int
    // ... fields for list state
}

func NewResumeModel(t theme.Theme, mgr *session.Manager) *ResumeModel
func (m *ResumeModel) Update(msg tea.Msg) ([]tea.Cmd, bool)
func (m *ResumeModel) View() string
```

- On startup: scan sessions directory via `Manager.ListSessions()`, sort by last-modified
- Render session list with metadata: ID, model, provider, started date, message count
- Corrupted sessions show `[!]` badge in red
- Keys: `Enter` = resume selected, `N` = new session, `D` = delete with confirmation, `Esc` = back to REPL
- Use `bubbles/list` for the session list

**Tests:** 8 tests covering: render session list, corrupted session badge, select/deselect navigation, delete confirmation flow, new session action

---

## Absolute Rules

1. **CGO_ENABLED=0** — No CGO in any package. Keychain implementations must use pure Go or external CLI.
2. **Atomic writes** — Every file write uses temp file + `os.Rename()`. No direct `os.WriteFile()` for state files.
3. **No telemetry** — Zero phone-home, analytics, or external calls beyond what's specified.
4. **Bubble Tea single-threaded** — All state mutations in `Update()`. No goroutine mutations.
5. **Existing tests pass** — Do not modify any files from Phases 0-4 except to add the new settings/resume screens.
6. **Markdown tolerance** — Planning file parsers tolerate extra whitespace, blank lines, missing optional sections.
7. **Error handling** — Return typed errors, not fmt.Errorf strings. Use `internal/errors/` sentinel errors where applicable.

## Deliverables

- `pkg/keychain/` — 3 platform implementations + interface + errors
- `pkg/session/` — Session manager with CRUD, planning file read/write, checkpoint system
- `internal/config/loader.go` — Config loading with env/keychain/file resolution
- `internal/tokens/` — Token estimation with tiktoken-go + EMA calibration
- `internal/tui/settings.go` — Settings screen
- `internal/tui/resume.go` — Resume/session browser screen
- **62 new tests minimum** (12 + 8 + 18 + 4 + 14 + 6 = 62)
- All existing 319 tests still pass
- `go vet ./...` clean
- `CGO_ENABLED=0 go build -o m31a ./cmd/m31a` succeeds

## File Creation Order

1. `pkg/keychain/` — interface, errors, linux, darwin, windows implementations
2. `pkg/session/` — manager, session CRUD, planning files, checkpoint
3. `internal/config/loader.go` — config loader
4. `internal/tokens/` — token estimator
5. `internal/tui/settings.go` — settings screen
6. `internal/tui/resume.go` — resume screen

Run `go mod tidy` after adding new dependencies (`BurntSushi/toml`, `tiktoken-go`, `godbus/dbus/v5`, `keyring`).

After implementation, run:
```
go mod tidy
CGO_ENABLED=0 go build -o m31a ./cmd/m31a
go vet ./...
go test -race -cover ./...
```

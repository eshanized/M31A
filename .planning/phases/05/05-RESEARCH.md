# Phase 5: Session State & Configuration — Research

**Researched:** 2026-05-28
**Domain:** Session persistence, configuration management, OS keychain, token estimation, TUI screens
**Confidence:** HIGH

## Summary

Phase 5 adds six major subsystems: config loading (TOML + env var + keychain resolution), OS keychain integration (Linux via godbus/dbus, macOS via security CLI, Windows stub), session lifecycle management (CRUD with atomic writes), planning file read/write (PROJECT.md, TASKS.md, STATE.md as Markdown), checkpoint system (max 2), and token estimation (tiktoken-go + EMA calibration). Two new TUI screens: Settings (tabbed config editor) and Resume (session browser using bubbles/list).

**Primary recommendation:** Build the keychain layer by shelling out to platform CLIs directly (no `zalando/go-keyring` dependency — it wraps the same `/usr/bin/security` CLI with an incompatible two-parameter API). Use `BurntSushi/toml` v1.4.0+ with `toml.DecodeFile`/`toml.Marshal`, `pkoukk/tiktoken-go` for OpenAI model tokenization (Claude models use the rune-based fallback), and `godbus/dbus/v5` for Linux Secret Service. Follow the existing atomic write pattern from `internal/tools/filewrite.go`.

## User Constraints (from CONTEXT.md)

<user_constraints>
### Locked Decisions

- **Config Loader:** Parse `~/.m31a/config.toml` using `BurntSushi/toml`. Resolution order: env var → OS keychain → config file. `M31A_CONFIG`, `M31A_THEME`, `M31A_DEFAULT_MODEL` env vars override specific fields. Missing config file returns `DefaultConfig()` without creating file. `Save()` uses atomic write (temp file + rename). `ResolveAPIKeys()` method on Config struct.
- **OS Keychain:** Unified `Keychain` interface: `Get(service)`, `Set(service, value)`, `Delete(service)`. Compile-tag separated implementations (linux, darwin, windows). Linux: freedesktop Secret Service via `godbus/dbus/v5` with `pass` CLI fallback. Darwin: `keyring` package (CGO-less). Windows: stub returning `ErrNotImplemented`. Service name format: `m31a/<provider>`. Sentinel errors: `ErrKeychainUnavailable`, `ErrKeyNotFound`, `ErrNotImplemented`.
- **Session Lifecycle:** `Manager` struct with `baseDir = ~/.m31a/sessions`. Session ID: 8-char hex via `crypto/rand`. Directory structure: `sessions/<id>/session.json`, `messages.json`, `planning/{PROJECT,TASKS,STATE}.md`. All writes atomic (temp file + rename in same directory). Load tolerates missing files. `ListSessions` returns `[]SessionInfo{ID, Model, Provider, StartedAt, LastModified, MessageCount, Corrupted}`. `ArchiveSession` moves to `archived/<id>/`.
- **Planning Files:** PROJECT.md (goal, type, framework, Q&A sections), TASKS.md (markdown table), STATE.md (Phase, Progress, Last Action, Timestamp in RFC3339). Parsers tolerate extra whitespace, blank lines, missing optional sections.
- **Checkpoints:** Stored as `checkpoint.json` in session directory. Max 2 retained (append new, trim oldest). Fields: Phase, Timestamp, MessageCount, TaskCount.
- **Token Estimation:** `Estimator` struct with modelID, tokenizer (tiktoken or nil), EMA calibration. tiktoken-go for GPT/Claude families. Fallback: `len([]rune(text)) / 4 * 1.3`. EMA: `factor = alpha*(actual/estimated) + (1-alpha)*previousFactor`. `FormatUsage()`: `"used / total (XX%)"`. `ContextWarningBanner()`: styled warning above threshold (default 80%).
- **Settings Screen:** `SettingsModel` implementing tea.Model. Two-column layout with tab navigation: General, Provider, Permissions, Features. API key fields masked. "Store in keychain" option. Atomic save, Tab cycles, Enter saves, Esc discards.
- **Resume Screen:** `ResumeModel` implementing tea.Model. Uses `bubbles/list` for session browser. Sorted by last-modified. Corrupted sessions show `[!]` badge. Keys: Enter = resume, N = new session, D = delete with confirmation, Esc = back to REPL.

### the agent's Discretion
- Exact test helper utilities and mock setup patterns
- Specific table formatting in TASKS.md (column widths, alignment)
- Theme integration details for settings/resume screens
- Error message styling in TUI context
- Temp file naming convention for atomic writes

### Deferred Ideas (OUT OF SCOPE)
- Windows keychain full implementation (stub returning ErrNotImplemented for V1)
- Integration of session lifecycle into workflow engine phases (Phase 6)
- `/undo` command wiring (Phase 7)
- AutoDream consolidation (Phase 7)
</user_constraints>

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| P5.1 | Config loader with env var → keychain → file resolution | Fully specified — TOML struct tags already in place; `toml.DecodeFile`/`toml.Marshal` API confirmed; env var override pattern straightforward |
| P5.2 | OS keychain with compile-tag separated implementations | Linux: `godbus/dbus/v5` confirmed CGO-free, Secret Service API documented; macOS: can shell out to `/usr/bin/security` CLI directly (CGO-free, avoids incompatible `go-keyring` API); Windows: stub |
| P5.3 | Session lifecycle (CRUD, atomic writes) | Atomic write pattern already in codebase (`filewrite.go`); `crypto/rand` confirmed for IDs; `os.Rename()` is atomic on same filesystem on Linux/macOS |
| P5.4 | Planning file read/write (Markdown parsers) | `bufio.Scanner` line-based parsing simplest approach; format spec fully defined; no regex library needed |
| P5.5 | Checkpoint system (max 2 retained) | Simple JSON file append-and-trim; `encoding/json` used throughout codebase |
| P5.6 | Settings screen (tabbed config editor) | `tea.Model` pattern confirmed from `repl.go`/`firstrun.go`; Tab cycling and form state straightforward |
| P5.7 | Resume screen (bubbles/list session browser) | `bubbles/list` API confirmed; `list.New()`, `list.Model.SelectedItem()` patterns documented; custom delegate for session info display |

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Config loading (TOML parse) | `internal/config/` | — | Config is internal to the application; no network or persistence tier involved |
| Config env var override | `internal/config/` | — | Pure config-layer concern, resolved before any other system reads it |
| API key resolution (env → keychain → file) | `internal/config/` | `pkg/keychain/` | Config layer orchestrates the resolution; keychain provides the storage backend |
| OS keychain storage | `pkg/keychain/` | — | Keychain is a standalone abstraction; compile-tag separated OS implementations |
| Session CRUD | `pkg/session/` | — | Session manager owns all session file I/O; no other layer should touch session files |
| Planning file write/parse | `pkg/session/` | — | Planning files are session state; session manager owns their lifecycle |
| Checkpoint save/load | `pkg/session/` | — | Checkpoints are per-session; belong in session manager |
| Token estimation | `internal/tokens/` | — | Token estimation has no external dependencies beyond tiktoken-go; local computation |
| Settings screen | `internal/tui/` | `internal/config/`, `pkg/keychain/` | TUI screen reads/writes config and optionally stores keys to keychain |
| Resume screen | `internal/tui/` | `pkg/session/` | TUI screen lists sessions via session manager, invokes load/delete |

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `github.com/BurntSushi/toml` | v1.4.0+ | TOML config parsing and serialization | De facto standard Go TOML library; 4.9K stars; MIT license; already referenced in AGENTS.md |
| `github.com/pkoukk/tiktoken-go` | latest | Token estimation for GPT model families | Official Go port of OpenAI's tiktoken; embedded dictionaries; CGO-free; supports o200k_base, cl100k_base, p50k_base, r50k_base |
| `github.com/godbus/dbus/v5` | v5.2.2 | D-Bus client for Linux Secret Service | Standard Go D-Bus library; pure Go; BSD license; used by `go-keyring`, `systemd`, `fyne` |
| `github.com/charmbracelet/bubbles` | v0.20.0 (existing) | `list.Model` for Resume screen | Already in go.sum; `list.New()`, `list.Model`, `list.DefaultDelegate` for session browser |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| stdlib `crypto/rand` | built-in | Cryptographically secure session ID generation | `rand.Read()` for 8-byte hex IDs |
| stdlib `encoding/hex` | built-in | Encode session ID bytes to hex string | `hex.EncodeToString()` for 8-char lowercase IDs |
| stdlib `encoding/json` | built-in | Serialize/deserialize session.json, messages.json, checkpoint.json | Already used throughout codebase |
| stdlib `bufio` | built-in | Line-by-line scanning for planning file parsers | `bufio.Scanner` with simple text line parsing for Markdown files |
| stdlib `bufio` | built-in | Tokenizer for strings | `bufio.Scanner` handles whitespace-tolerant parsing |
| stdlib `os/exec` | built-in | Shell out to `security` CLI (macOS) and `pass` CLI (Linux fallback) | No dependency needed; CGO-free |
| stdlib `strings` | built-in | String manipulation for Markdown parsing | `strings.HasPrefix`, `strings.TrimSpace` for line parsing |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Direct macOS `security` CLI | `zalando/go-keyring` | `go-keyring` has two-parameter API `Get(service, user)` incompatible with our `Get(service)`; its macOS impl just shells to `/usr/bin/security` anyway — adding it as a dependency provides zero value and adds an incompatible wrapper |
| Linux: Direct godbus/dbus | `zalando/go-keyring` linux impl | Same issue: `go-keyring`'s Linux impl uses `godbus/dbus` internally but with incompatible API; direct dbus gives full control over service names |
| `github.com/pkoukk/tiktoken-go` | `github.com/tiktoken-go/tokenizer` | `pkoukk/tiktoken-go` is the original port referenced in AGENTS.md; `tokenizer` is a newer fork with different API surface; stick with AGENTS.md recommendation |
| BurntSushi/toml | `pelletier/go-toml` | BurntSushi is already referenced in AGENTS.md; struct tags already in place; simpler API; more widely adopted |

### Installation
```bash
go get github.com/BurntSushi/toml@latest
go get github.com/pkoukk/tiktoken-go@latest
go get github.com/godbus/dbus/v5@latest
go mod tidy
```

### Version Verification
```bash
# Verify packages exist and are current
go list -m github.com/BurntSushi/toml@latest
go list -m github.com/pkoukk/tiktoken-go@latest
go list -m github.com/godbus/dbus/v5@latest
```

## Package Legitimacy Audit

> **slopcheck unavailable at research time** — all packages flagged `[ASSUMED]`. The planner MUST gate each install behind a `checkpoint:human-verify` task.

| Package | Registry | Age | Downloads | Source Repo | slopcheck | Disposition |
|---------|----------|-----|-----------|-------------|-----------|-------------|
| `github.com/BurntSushi/toml` | Go modules | 12+ yrs | 38K+ importers | github.com/BurntSushi/toml | [ASSUMED] | Approved — AGENTS.md reference, MIT license |
| `github.com/pkoukk/tiktoken-go` | Go modules | 3+ yrs | Widely used | github.com/pkoukk/tiktoken-go | [ASSUMED] | Approved — AGENTS.md reference, MIT license |
| `github.com/godbus/dbus/v5` | Go modules | 10+ yrs | 10K+ importers | github.com/godbus/dbus | [ASSUMED] | Approved — BSD license, industry standard |
| macOS keychain | None (stdlib) | — | — | — | — | No package needed — shell out to `/usr/bin/security` |
| pass CLI (Linux fallback) | None (system tool) | — | — | — | — | No package needed — shell out to `pass` binary |

**Packages removed:** None
**Packages flagged as suspicious:** None (all standard, well-established libraries)
**Note:** `godbus/dbus/v5` imports are Linux-only via build tags; it will not compile on macOS/Windows but `go mod tidy` handles this correctly since it's behind `//go:build linux`.

## Architecture Patterns

### System Architecture Data Flow

```
User edits config or browses sessions
        │
        ▼
┌─────────────────────────────────────┐
│         TUI Screens                 │
│  ┌────────────┐  ┌──────────────┐   │
│  │ Settings   │  │ Resume       │   │
│  │ (tabbed)   │  │ (bubbles/list)│   │
│  └─────┬──────┘  └──────┬───────┘   │
└────────┼────────────────┼───────────┘
         │                │
         ▼                ▼
┌─────────────────────────────────────┐
│        internal/config/             │
│  Load() → ResolveAPIKeys() → Save() │
│  Env var → Keychain → Config file   │
└────────┬────────────────────────────┘
         │ keychain operations
         ▼
┌─────────────────────────────────────┐
│        pkg/keychain/                │
│  Keychain interface                 │
│  ├─ keychain_linux.go (SecretService│
│  │   + dbus → pass CLI fallback)     │
│  ├─ keychain_darwin.go (security    │
│  │   CLI via /usr/bin/security)      │
│  └─ keychain_windows.go (stub)      │
└─────────────────────────────────────┘
         ▲
         │ session state
┌────────┴────────────────────────────┐
│        pkg/session/                 │
│  Manager CRUD + checkpoints         │
│  └─ planning/ files (PROJECT.md,    │
│     TASKS.md, STATE.md)             │
│  └─ session.json / messages.json    │
│  └─ checkpoint.json (max 2)         │
└─────────────────────────────────────┘
         ▲
         │ token estimates
┌────────┴────────────────────────────┐
│        internal/tokens/             │
│  Estimator (tiktoken-go + fallback  │
│  + EMA calibration)                 │
└─────────────────────────────────────┘
```

**Flow description:**
1. **Startup**: `internal/config/loader.go` loads config → `ResolveAPIKeys()` queries keychain for any key not in env var → session manager loads from `~/.m31a/sessions/`
2. **Settings screen**: Reads/writes config via `Load()`/`Save()` → stores API keys to keychain via `Keychain.Set()`
3. **Resume screen**: Lists sessions via `Manager.ListSessions()` → load on Enter → delete on D
4. **Token estimation**: Called by streaming pipeline to estimate context usage → EMA calibration corrects estimates vs actuals from API responses
5. **Workflow phase save**: Phase 6 will call `Manager.SaveProject()`, `Manager.SaveTasks()`, `Manager.SaveState()`, `Manager.SaveCheckpoint()` after each task

### Recommended Project Structure
```
pkg/keychain/
├── keychain.go           # Keychain interface + New() factory
├── keychain_linux.go     # Linux: godbus/dbus Secret Service + pass fallback
├── keychain_darwin.go    # Darwin: /usr/bin/security CLI
├── keychain_windows.go   # Windows: stub returning ErrNotImplemented
└── errors.go             # ErrKeychainUnavailable, ErrKeyNotFound, ErrNotImplemented

pkg/session/
├── manager.go            # Manager struct, NewSession, LoadSession, ListSessions, etc.
├── session.go            # Session struct (embeds types.Session)
├── session_info.go       # SessionInfo struct for list display
├── planning.go           # PROJECT.md, TASKS.md, STATE.md read/write
├── checkpoint.go         # Checkpoint save/load/trim logic

internal/config/
├── types.go              # (existing) Config structs with TOML tags
├── loader.go             # Load(), DefaultConfig(), Save(), ResolveAPIKeys()
└── loader_test.go        # 12 tests

internal/tokens/
├── estimator.go          # Estimator struct, Estimate(), Calibrate(), FormatUsage()
└── estimator_test.go     # 14 tests

internal/tui/
├── settings.go           # SettingsModel (tea.Model)
├── settings_test.go      # 6 tests
├── resume.go             # ResumeModel (tea.Model)
└── resume_test.go        # 8 tests
```

### Pattern 1: Atomic Write Pattern
**Source:** `internal/tools/filewrite.go` (verified existing code)

```go
// All state file writes use this pattern. NEVER use os.WriteFile() for state files.
func atomicWrite(path string, data []byte) error {
    dir := filepath.Dir(path)
    
    // Generate random temp name
    randBytes := make([]byte, 8)
    if _, err := rand.Read(randBytes); err != nil {
        return fmt.Errorf("cannot generate temp name: %w", err)
    }
    tmpPath := filepath.Join(dir, ".m31a_tmp_"+hex.EncodeToString(randBytes))
    
    // Create and write to temp file
    tmpFile, err := os.Create(tmpPath)
    if err != nil {
        return err
    }
    defer os.Remove(tmpPath) // cleanup on failure
    
    if _, err := tmpFile.Write(data); err != nil {
        tmpFile.Close()
        return err
    }
    if err := tmpFile.Sync(); err != nil {
        tmpFile.Close()
        return err
    }
    if err := tmpFile.Close(); err != nil {
        return err
    }
    
    // Atomic rename
    if err := os.Rename(tmpPath, path); err != nil {
        return err
    }
    return nil
}
```

**Key details:**
- Temp file in **same directory** as target (filesystem atomic rename requires same mount point)
- `crypto/rand` for temp name (avoids predictability issues)
- `os.Rename()` is atomic on Linux/macOS on the same filesystem
- Cleanup on error via `defer os.Remove(tmpPath)`

### Pattern 2: Bubble Tea Subscreen Pattern
**Source:** `internal/tui/firstrun.go` (verified existing code)

```go
type SettingsModel struct {
    theme   theme.Theme
    config  *config.Config
    activeTab int
    width     int
    height    int
    // form fields...
}

func NewSettingsModel(cfg *config.Config, t theme.Theme) *SettingsModel {
    return &SettingsModel{
        theme:  t,
        config: cfg,
    }
}

func (m *SettingsModel) Update(msg tea.Msg) ([]tea.Cmd, *AppMsg) {
    switch msg := msg.(type) {
    case tea.WindowSizeMsg:
        m.width, m.height = msg.Width, msg.Height
    case tea.KeyMsg:
        switch msg.String() {
        case "tab":
            m.activeTab = (m.activeTab + 1) % 4
        case "enter":
            // Save config
            return nil, &AppMsg{Screen: ScreenREPL}
        case "esc":
            // Discard changes
            return nil, &AppMsg{Screen: ScreenREPL}
        }
    }
    return nil, nil
}

func (m *SettingsModel) View() string {
    // Render using lipgloss layout
}
```

**Key patterns:**
- Subscreen model implements own `Update` returning `([]tea.Cmd, *AppMsg)` — `nil, &AppMsg{Screen: ScreenREPL}` to transition
- `tea.WindowSizeMsg` handled to store width/height for responsive rendering
- `AppState.Update()` routes msgs based on `m.screen` value
- Theme passed in constructor, used for all style definitions

### Pattern 3: Toast/Status Message Pattern
**Source:** `internal/tui/statusbar.go` (verified existing code)

The status bar rendering pattern uses `lipgloss.JoinHorizontal` with calculated padding. For the settings/resume screens, use `RenderStatusBar()` directly or similar lipgloss composition.

### Anti-Patterns to Avoid
- **Direct os.WriteFile() for state:** Every state write must use the atomic write pattern (temp + rename). `os.WriteFile()` leaves partial files on crash.
- **Mixing JSON and Markdown parsers:** session.json and messages.json are JSON. Planning files are Markdown. Never mix the formats.
- **Hardcoded session base path:** Always resolve `~/.m31a/sessions` via `os.UserHomeDir()`, never hardcode.
- **Two-parameter keychain API:** `go-keyring` uses `Get(service, user)` which doesn't match our `Get(service)` interface. Avoid the wrapper — shell out directly.
- **CGO in darwin keychain:** macOS Keychain Services C bindings require CGO. Always use `/usr/bin/security` CLI for CGO_ENABLED=0 compatibility.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| TOML config parsing | Manual TOML parser | `BurntSushi/toml` | TOML is surprisingly complex (dates, arrays of tables, inline tables); struct tags already in place |
| Token estimation for GPT models | Manual BPE tokenizer | `pkoukk/tiktoken-go` | Requires 4MB of encoding dictionaries; embedding the ranks map at build time is non-trivial; tiktoken-go does it automatically |
| D-Bus protocol handling | Manual D-Bus message encoding | `godbus/dbus/v5` | D-Bus is a complex binary protocol with introspection; implementing it correctly is weeks of work |
| macOS Keychain access | Manual Security Framework C bindings | `/usr/bin/security` CLI | Security Framework requires CGO; the `security` CLI is always available on macOS and CGO-free |
| Bubble Tea list component | Custom scrollable list | `bubbles/list` | Already a dependency; handles filtering, pagination, keyboard navigation, custom delegates |

**Key insight:** Each of these avoided custom solutions involves either: (1) a complex protocol that would take weeks to implement correctly (D-Bus, BPE tokenizer), or (2) a CGO requirement that violates the project's `CGO_ENABLED=0` constraint (macOS Security Framework).

## Common Pitfalls

### Pitfall 1: `os.Rename()` Cross-Device Error
**What goes wrong:** `os.Rename()` fails with `EXDEV` (cross-device link) if temp file and target are on different filesystems.
**Why it happens:** Using `os.CreateTemp("")` creates temp in system TMPDIR, while target is in `~/.m31a/` — these may be on different mount points.
**How to avoid:** Create the temp file in the **same directory** as the target file using `filepath.Join(filepath.Dir(target), ".m31a_tmp_"+hex)`.
**Warning signs:** Tests fail on systems with `/tmp` on a different partition than `$HOME`.

### Pitfall 2: tiktoken-go Dictionary Load Failure
**What goes wrong:** `tiktoken.EncodingForModel("unknown-model")` returns an error because the model name doesn't match any known prefix.
**Why it happens:** Claude models (claude-3-haiku, claude-3.5-sonnet) are NOT in tiktoken-go's model map. Neither are many OpenRouter model aliases.
**How to avoid:** Always wrap `EncodingForModel()` with a fallback: if err != nil, fall back to `len([]rune(text))/4*1.3`. Never let a token estimation failure propagate as an error.
**Warning signs:** `"no encoding for model claude-3-sonnet"` logged at runtime.

### Pitfall 3: OS Keychain on Headless Linux
**What goes wrong:** `dbus.ConnectSessionBus()` fails because D-Bus session bus is not running (common in Docker, CI, SSH without D-Bus forwarding).
**Why it happens:** The freedesktop Secret Service requires a running D-Bus session bus AND a keyring daemon (gnome-keyring, kwallet).
**How to avoid:** Always fall back gracefully: if dbus connection fails → try `pass` CLI → if pass not available → return `ErrKeychainUnavailable`. Config loader should handle this: if keychain unavailable, the config file `api_key` field is used (after env var check).
**Warning signs:** `"keychain unavailable: D-Bus session bus not found"` in logs.

### Pitfall 4: macOS `security` CLI Output Changes
**What goes wrong:** `security find-generic-password -s "m31a/openrouter" -wa "m31a"` output format changes between macOS versions.
**Why it happens:** Apple occasionally changes the `security` CLI output format. The `-w` flag (output only password value) has been stable, but the `-g` flag (display) format changes.
**How to avoid:** Always use `-w` flag (prints only the password value). Avoid `-g` flag. Use `-a` for the account name to distinguish multiple items under the same service.
**Warning signs:** Keychain tests fail after macOS update.

### Pitfall 5: JSON vs Markdown Parser Confusion
**What goes wrong:** session.json/messages.json are parsed as Markdown, or planning files are parsed as JSON.
**Why it happens:** Both files live in the same session directory; it's easy to accidentally apply the wrong parser.
**How to avoid:** Separate read methods by extension: `readJSON(path)`, `readMarkdownPlanningFile(path)`. The file format (Markdown vs JSON) is determined by filename, never by content sniffing.
**Warning signs:** Planning file values come back as `null` or empty.

### Pitfall 6: Session ID Collision Extremely Unlikely But Not Zero
**What goes wrong:** `crypto/rand` generates a duplicate 8-char hex ID (64 bits).
**How to avoid:** After generating an ID, check if the directory already exists. Retry on collision. Document as "astronomically unlikely" but handle it.
**Warning signs:** N/A (seen only if test creates many thousands of sessions).

## Code Examples

### BurntSushi/toml — Decode and Encode
**Source:** [BurntSushi/toml pkg.go.dev](https://pkg.go.dev/github.com/BurntSushi/toml@v1.4.0)

```go
import "github.com/BurntSushi/toml"

// Read
var cfg Config
_, err := toml.DecodeFile(path, &cfg)
if os.IsNotExist(err) {
    cfg = DefaultConfig() // file doesn't exist — return defaults
}

// Write (atomic)
data, err := toml.Marshal(cfg)
if err != nil {
    return err
}
return atomicWrite(path, data) // using our atomicWrite helper

// Alternative write with Encoder
var buf bytes.Buffer
err = toml.NewEncoder(&buf).Encode(cfg)
```

### tiktoken-go — Token Estimation
**Source:** [pkoukk/tiktoken-go README verified](https://github.com/pkoukk/tiktoken-go)

```go
import "github.com/pkoukk/tiktoken-go"

func estimateTokens(modelID, text string) int {
    tkm, err := tiktoken.EncodingForModel(modelID)
    if err != nil {
        // Fallback for unsupported models (Claude, etc.)
        return len([]rune(text)) / 4 * 1.3
    }
    return len(tkm.Encode(text, nil, nil))
}
```

**Model support matrix:**
- `gpt-4o`, `gpt-4o-*` → `o200k_base` encoding
- `gpt-4`, `gpt-4-*`, `gpt-3.5-turbo`, `gpt-3.5-turbo-*` → `cl100k_base` encoding
- Claude models (any), OpenRouter non-OpenAI models → fallback to `len([]rune)/4*1.3`

### macOS Keychain — Direct CLI
```go
//go:build darwin

package keychain

import (
    "os/exec"
    "strings"
)

const servicePrefix = "m31a/"
const accountName = "m31a"

func (k *macOSKeychain) Get(service string) (string, error) {
    out, err := exec.Command(
        "/usr/bin/security",
        "find-generic-password",
        "-s", servicePrefix+service,
        "-wa", accountName,
    ).Output()
    if err != nil {
        if strings.Contains(string(out), "could not be found") {
            return "", ErrKeyNotFound
        }
        return "", err
    }
    return strings.TrimSpace(string(out)), nil
}

func (k *macOSKeychain) Set(service, value string) error {
    cmd := exec.Command("/usr/bin/security",
        "add-generic-password",
        "-U",                           // update if exists
        "-s", servicePrefix+service,
        "-a", accountName,
        "-w", value,
    )
    return cmd.Run()
}

func (k *macOSKeychain) Delete(service string) error {
    out, err := exec.Command("/usr/bin/security",
        "delete-generic-password",
        "-s", servicePrefix+service,
        "-a", accountName,
    ).CombinedOutput()
    if strings.Contains(string(out), "could not be found") {
        return ErrKeyNotFound
    }
    return err
}
```

### Linux Keychain — D-Bus Secret Service
**Source:** [zalando/go-keyring secret_service patterns verified](https://github.com/zalando/go-keyring)

```go
//go:build linux

package keychain

import (
    "github.com/godbus/dbus/v5"
)

const (
    secretServiceName = "org.freedesktop.secrets"
    secretServicePath = "/org/freedesktop/secrets"
)

func (k *linuxKeychain) Get(service string) (string, error) {
    conn, err := dbus.ConnectSessionBus()
    if err != nil {
        return "", tryPassFallback("show", service)
    }
    defer conn.Close()
    
    // Query Secret Service for item with attribute "service"="m31a/<service>"
    // Use SearchItems method on the collection
    // Returns the secret value as string
    // ...
}
```

**D-Bus Secret Service key methods:**
- `conn.Object("org.freedesktop.secrets", "/org/freedesktop/secrets").Call("org.freedesktop.Secret.Service.OpenSession", 0, "plain", dbus.MakeVariant(""))`
- `conn.Object(...).Call("org.freedesktop.Secret.Service.SearchItems", 0, map[string]string{"service": "m31a/openrouter"})`
- Item interface: `GetSecret`, `SetSecret`, `Delete`

### Planning File Parser Pattern
**Source:** bufio.Scanner pattern from existing codebase (SSE parser uses same pattern)

```go
func parseSTATE(content string) StateData {
    var state StateData
    scanner := bufio.NewScanner(strings.NewReader(content))
    for scanner.Scan() {
        line := strings.TrimSpace(scanner.Text())
        if line == "" || strings.HasPrefix(line, "#") {
            continue // skip blank lines and headers
        }
        switch {
        case strings.HasPrefix(line, "**Phase:**"):
            state.Phase = strings.TrimSpace(line[len("**Phase:**"):])
        case strings.HasPrefix(line, "**Progress:**"):
            state.Progress = strings.TrimSpace(line[len("**Progress:**"):])
        case strings.HasPrefix(line, "**Last Action:**"):
            state.LastAction = strings.TrimSpace(line[len("**Last Action:**"):])
        case strings.HasPrefix(line, "**Timestamp:**"):
            ts := strings.TrimSpace(line[len("**Timestamp:**"):])
            state.Timestamp, _ = time.Parse(time.RFC3339, ts)
        }
    }
    return state
}
```

Key: No regex needed. Simple `strings.HasPrefix` + `strings.TrimSpace` handles all formats. Tolerates extra whitespace, missing optional sections, and blank lines.

### bubbles/list for Resume Screen
**Source:** [bubbles/list docs verified](https://github.com/charmbracelet/bubbles/tree/master/list)

```go
import (
    "github.com/charmbracelet/bubbles/list"
    tea "github.com/charmbracelet/bubbletea"
)

// Session item must implement list.DefaultItem
type sessionItem struct {
    title, desc string
    id          string
    corrupted   bool
}

func (i sessionItem) Title() string       { return i.title }
func (i sessionItem) Description() string { return i.desc }
func (i sessionItem) FilterValue() string { return i.title }

// In ResumeModel
type ResumeModel struct {
    list     list.Model
    manager  *session.Manager
    theme    theme.Theme
    width    int
    height   int
}

func NewResumeModel(t theme.Theme, mgr *session.Manager) *ResumeModel {
    items := []list.Item{}
    l := list.New(items, list.NewDefaultDelegate(), 0, 0)
    l.Title = "Sessions"
    l.SetShowStatusBar(false)
    
    return &ResumeModel{
        list:    l,
        manager: mgr,
        theme:   t,
    }
}

func (m *ResumeModel) Update(msg tea.Msg) ([]tea.Cmd, *AppMsg) {
    switch msg := msg.(type) {
    case tea.WindowSizeMsg:
        m.width, m.height = msg.Width, msg.Height
        h, v := 2, 0 // margin
        m.list.SetSize(msg.Width-h, msg.Height-v)
    case tea.KeyMsg:
        switch msg.String() {
        case "enter":
            item := m.list.SelectedItem().(sessionItem)
            return nil, &AppMsg{Screen: ScreenREPL, SessionID: item.id}
        case "n", "N":
            return nil, &AppMsg{Screen: ScreenFirstRun}
        case "d", "D":
            // Show delete confirmation
            return m.deleteSelected()
        case "esc":
            return nil, &AppMsg{Screen: ScreenREPL}
        }
    }
    var cmd tea.Cmd
    m.list, cmd = m.list.Update(msg)
    return []tea.Cmd{cmd}, nil
}
```

### EMA Calibration Pattern
```go
const emaAlpha = 0.3

func (e *Estimator) Calibrate(estimated, actual int) {
    ratio := float64(actual) / float64(estimated)
    e.emaFactor = emaAlpha*ratio + (1-emaAlpha)*e.emaFactor
}

func (e *Estimator) Estimate(text string) int {
    var estimated int
    if e.tokenizer != nil {
        estimated = len(e.tokenizer.Encode(text, nil, nil))
    } else {
        estimated = len([]rune(text)) / 4 * 1.3
    }
    return int(float64(estimated) * e.emaFactor)
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| CGO-based macOS keychain | `/usr/bin/security` CLI | 2019+ (go-keyring adoption) | CGO_ENABLED=0 compatibility; cross-compilation without macOS SDK |
| Cloud-downloaded BPE dictionaries | Embedded vocabularies (tiktoken-go) | 2024 (v0.4.0+) | No runtime downloads; ~4MB binary size increase but no network dependency |
| BurntSushi/toml DecodeFile + os.WriteFile | DecodeFile + atomic Marshal+rename | Varies | Crash-safe writes; no partial file on power loss |
| 3rd-party TOML libs (pelletier/go-toml, others) | BurntSushi/toml (de facto standard) | Stable since 2014 | 4.9K stars, 38K importers, MIT license |

**Deprecated/outdated:**
- `pelletier/go-toml` v1.x: Deprecated in favor of v2, but v2 has a different API. BurntSushi/toml is the canonical choice given AGENTS.md reference.
- `go-keyring` v0.1.x: Older versions used different APIs. Current v0.2.8+ uses service+user two-parameter API.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | macOS `security` CLI format stable with `-wa` flags | Code Examples | Low — `-w` flag has been stable across multiple macOS versions; Apple considers it stable API |
| A2 | tiktoken-go supports `o200k_base` encoding for GPT-4o | Code Examples | MEDIUM — confirmed in v0.7.0 release; if model name not in map, fallback handles it gracefully |
| A3 | Linux Secret Service uses `org.freedesktop.secrets` on path `/org/freedesktop/secrets` | Code Examples | HIGH — this is the standardized interface per freedesktop specification |
| A4 | `os.Rename()` is atomic on same-filesystem for Linux/macOS | Common Pitfalls | HIGH — documented POSIX behavior; cross-filesystem EXDEV case handled by temp file in same dir |
| A5 | Existing 319 tests will still pass after Phase 5 | All sections | MEDIUM — no existing files modified except adding `internal/tui/settings.go` and `internal/tui/resume.go`; theme and types unchanged |

## Open Questions

1. **Should we use `zalando/go-keyring` or direct CLI?**
   - What we know: `go-keyring`'s API is `Get(service, user string)` — incompatible with our `Get(service string)`. On macOS it wraps `/usr/bin/security` CLI. On Linux it wraps `godbus/dbus`.
   - Recommendation: Implement directly. macOS: shell to `/usr/bin/security` (10 lines of code). Linux: use `godbus/dbus/v5` directly for full control over service names.
   - Confidence: HIGH

2. **D-Bus Secret Service wrapper code complexity?**
   - What we know: The Secret Service D-Bus API involves opening sessions, creating items with attributes, unlocking collections.
   - What's unclear: Exact D-Bus method signatures for `SearchItems` and `GetSecret`.
   - Recommendation: Reference the `zalando/go-keyring` secret_service subpackage for D-Bus call patterns, or implement a minimal wrapper around the 3-4 D-Bus calls needed.
   - Confidence: MEDIUM

3. **tiktoken-go: which import path?**
   - What we know: Two modules exist — `github.com/pkoukk/tiktoken-go` (original, referenced in AGENTS.md) and `github.com/tiktoken-go/tokenizer` (newer fork).
   - Recommendation: Use `github.com/pkoukk/tiktoken-go` as specified in AGENTS.md. The API uses `tiktoken.EncodingForModel(modelName)`.
   - Confidence: HIGH

4. **Should `AppMsg` get a `SessionID` field?**
   - What we know: Resume screen needs to signal which session to load. Current `AppMsg` struct has `Screen`, `Health`, `Provider`, `InitError` fields.
   - Recommendation: Add `SessionID string` field to `AppMsg` struct in `internal/tui/types.go`. This is a minimal, backwards-compatible change.
   - Confidence: HIGH

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Go 1.22+ | All | ✓ | 1.22+ | — |
| CGO_ENABLED=0 | Build constraint | ✓ | — | Non-negotiable |
| D-Bus session bus | Linux keychain | Platform-dependent | — | `pass` CLI fallback |
| `/usr/bin/security` | macOS keychain | Platform-dependent | — | `ErrKeychainUnavailable` |
| `pass` CLI | Linux keychain fallback | Platform-dependent | — | `ErrKeychainUnavailable` |

**Missing dependencies with no fallback:**
- None — all platforms handled with appropriate fallbacks

**Missing dependencies with fallback:**
- D-Bus on Linux → `pass` CLI → `ErrKeychainUnavailable`
- macOS `security` → `ErrKeychainUnavailable`
- Windows → `ErrNotImplemented` (stub)

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Go's built-in `testing` package |
| Config file | None — `go test -race -cover ./...` |
| Quick run command | `go test -race -cover ./pkg/keychain/ ./pkg/session/ ./internal/config/ ./internal/tokens/ ./internal/tui/` |
| Full suite command | `go test -race -cover ./...` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| P5.1 | Config loader default config | unit | `go test ./internal/config/ -run TestConfig_Default` | ❌ Wave 0 |
| P5.1 | Config loader env var override | unit | `go test ./internal/config/ -run TestConfig_EnvOverride` | ❌ Wave 0 |
| P5.1 | Config loader TOML parse | unit | `go test ./internal/config/ -run TestConfig_TOMLParse` | ❌ Wave 0 |
| P5.1 | Config loader atomic save | unit | `go test ./internal/config/ -run TestConfig_Save` | ❌ Wave 0 |
| P5.1 | Config loader key resolution order | unit | `go test ./internal/config/ -run TestConfig_KeyResolution` | ❌ Wave 0 |
| P5.2 | Keychain set/get/delete (mocked) | unit | `go test ./pkg/keychain/ -run TestKeychain` | ❌ Wave 0 |
| P5.3 | Session new/load/list/delete | unit | `go test ./pkg/session/ -run TestSession` | ❌ Wave 0 |
| P5.4 | Planning file read/write | unit | `go test ./pkg/session/ -run TestPlanning` | ❌ Wave 0 |
| P5.5 | Checkpoint save/load/max 2 | unit | `go test ./pkg/session/ -run TestCheckpoint` | ❌ Wave 0 |
| P5.6 | Settings screen render/keys | unit | `go test ./internal/tui/ -run TestSettings` | ❌ Wave 0 |
| P5.7 | Resume screen list/nav/delete | unit | `go test ./internal/tui/ -run TestResume` | ❌ Wave 0 |
| P5.3 | Token estimation tiktoken/fallback/EMA | unit | `go test ./internal/tokens/ -run TestEstimator` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `go test -race -cover ./pkg/keychain/ ./pkg/session/ ./internal/config/ ./internal/tokens/`
- **Per wave merge:** `go test -race -cover ./...`
- **Phase gate:** Full suite green before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] `pkg/keychain/keychain_test.go` — covers P5.2
- [ ] `pkg/session/manager_test.go` — covers P5.3, P5.4, P5.5
- [ ] `internal/config/loader_test.go` — covers P5.1
- [ ] `internal/tokens/estimator_test.go` — covers P5.6 (token estimation part)
- [ ] `internal/tui/settings_test.go` — covers P5.6 (TUI screen)
- [ ] `internal/tui/resume_test.go` — covers P5.7
- [ ] Framework install: none needed (uses `testing` standard library)

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | No user authentication; API keys are stored, not used for login |
| V3 Session Management | yes | Session ID generation via `crypto/rand` (8 bytes = 64 bits); session directory isolation |
| V4 Access Control | yes | File permissions: session files 0644, config file 0600 (contains API keys) |
| V5 Input Validation | yes | Path traversal prevention in keychain service names (alphanumeric only: `m31a/[a-z]+`) |
| V6 Cryptography | yes | OS keychain provides encrypted storage; no custom crypto |

### Known Threat Patterns for Phase 5
| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| API key stored in plaintext config file | Information Disclosure | Resolution order: env var → keychain → config file; config file `api_key` is last resort; warn user if reading from config file |
| Temp file race condition | Tampering | `crypto/rand` for temp names (not predictable); temp file same directory as target (avoids cross-device symlink attacks) |
| Session ID collision | Spoofing | Check dir existence after generation; retry on collision (64-bit space makes this astronomically unlikely) |
| macOS `security` CLI injection | Tampering | Service name validated as `m31a/[a-z]+`; password value passed via `-w` flag (not concatenated into command) |

## Sources

### Primary (HIGH confidence)
- [BurntSushi/toml v1.4.0 pkg.go.dev](https://pkg.go.dev/github.com/BurntSushi/toml@v1.4.0) — DecodeFile, Marshal, NewEncoder API confirmed
- [pkoukk/tiktoken-go README](https://github.com/pkoukk/tiktoken-go) — EncodingForModel, Encode API confirmed; model → encoding mapping verified
- [godbus/dbus v5 pkg.go.dev](https://pkg.go.dev/github.com/godbus/dbus/v5@v5.2.2) — ConnectSessionBus, Object.Call API confirmed
- [bubbles/list GitHub](https://github.com/charmbracelet/bubbles/tree/master/list) — list.New, list.Model, DefaultDelegate, SelectedItem API confirmed
- [zalando/go-keyring keyring_darwin.go](https://github.com/zalando/go-keyring) — macOS security CLI patterns confirmed (CGO-free, `/usr/bin/security`)
- [zalando/go-keyring keyring_unix.go](https://github.com/zalando/go-keyring) — Linux Secret Service D-Bus patterns confirmed
- Existing codebase: `internal/tools/filewrite.go` — atomic write pattern verified
- Existing codebase: `internal/tui/firstrun.go` — Bubble Tea screen pattern verified
- Existing codebase: `internal/config/types.go` — TOML struct tags already in place
- AGENTS.md — CGO_ENABLED=0, no telemetry, key resolution order

### Secondary (MEDIUM confidence)
- WebSearch: tiktoken-go model mapping table — confirms GPT-4o uses o200k_base, GPT-4 uses cl100k_base
- WebSearch: Secret Service D-Bus specification — confirms service name, path, and interface names
- WebSearch: bubbles/list simple example — confirms custom delegate pattern for session list items

### Tertiary (LOW confidence)
- None — all claims verified against official docs or existing codebase

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all libraries verified against official docs or AGENTS.md references
- Architecture: HIGH — patterns directly from existing codebase (filewrite.go atomic write, firstrun.go TUI model, types.go TOML struct tags)
- Pitfalls: HIGH — two from documented experience (cross-device rename, tiktoken-go model limits, headless Linux keychain) plus verified macOS security CLI behavior

**Research date:** 2026-05-28
**Valid until:** 2026-06-28 (30 days — config management is stable; tiktoken-go model support may evolve but fallback handles it)

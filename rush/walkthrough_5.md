# Walkthrough 5 — What Phase 5 Should Produce

## Package Layout After Phase 5

```
pkg/keychain/
├── keychain.go          # Keychain interface + New() factory
├── keychain_linux.go    # Secret Service via dbus + pass fallback
├── keychain_darwin.go   # macOS Keychain via go-keyring
├── keychain_windows.go  # Stub returning ErrNotImplemented
└── errors.go            # Sentinel errors

pkg/session/
├── manager.go           # SessionManager: NewSession, LoadSession, ListSessions, Delete, Archive
├── save.go              # SaveMessages, SaveProject, SaveTasks, SaveState (all atomic)
├── checkpoint.go        # SaveCheckpoint, LoadCheckpoints, LatestCheckpoint (max 2 retained)
└── markdown.go          # Parse/write PROJECT.md, TASKS.md, STATE.md

internal/config/
├── types.go             # Already exists (Config struct with TOML tags)
└── loader.go            # Load(), DefaultConfig(), ResolveAPIKeys(), Save()

internal/tokens/
└── estimator.go         # NewEstimator, Estimate, Calibrate, FormatUsage, ContextWarningBanner

internal/tui/
├── settings.go          # SettingsModel: two-column tabs, masked keys, atomic save
└── resume.go            # ResumeModel: session browser, corrupted detection, delete confirmation
```

## Config Loader (`internal/config/loader.go`)

**Key behaviors:**
- `Load(path)` reads `~/.m31a/config.toml`. If file missing, returns `DefaultConfig()` without error.
- `M31A_CONFIG` env var overrides the path argument.
- `ResolveAPIKeys(keychain)` checks: env vars → keychain → config file, in that order.
- `Save(path)` writes atomically: temp file in same directory → `os.Rename()`.
- Env var overrides: `M31A_OPENROUTER_API_KEY`, `M31A_ZEN_API_KEY`, `M31A_THEME`, `M31A_DEFAULT_MODEL`.

**What it looks like:**
```
Load("~/.m31a/config.toml")
  → config file missing → returns DefaultConfig()
  → provider.Default = "openrouter"
  → model.Default = ""
  → ui.Theme = "dark"

ResolveAPIKeys(keychain)
  → os.Getenv("M31A_OPENROUTER_API_KEY") → if set, use it
  → keychain.Get("m31a/openrouter") → if set, use it
  → config.Provider.OpenRouter.APIKey → fallback
```

## OS Keychain (`pkg/keychain/`)

**Linux implementation:**
```go
// keychain_linux.go
// +build linux

// Tries dbus Secret Service first
// Falls back to `pass` CLI: `pass show m31a/openrouter`
// Service names: "m31a/openrouter", "m31a/zen"
```

**macOS implementation:**
```go
// keychain_darwin.go
// +build darwin

// Uses github.com/keyring/keyring (no CGO)
// Service name: "m31a"
// Account name: provider name (openrouter, zen)
```

**Windows implementation:**
```go
// keychain_windows.go
// +build windows

// Returns ErrNotImplemented for all methods
```

**Interface:**
```go
type Keychain interface {
    Get(service string) (string, error)  // ErrKeyNotFound if missing
    Set(service, value string) error
    Delete(service string) error
}
```

## Session Manager (`pkg/session/`)

**Session directory created by NewSession:**
```
~/.m31a/sessions/a1b2c3d4/
├── session.json          # {id, model, provider, started_at, message_count, workflow_phase}
├── messages.json         # []types.Message (initially empty array)
└── planning/
    ├── PROJECT.md        # Initially: "# Project\n\n**Goal:** \n**Type:** \n**Framework:** \n"
    ├── TASKS.md          # Initially: "# Tasks\n\n| ID | Action | Description | Deps | Status | Files |\n|----|--------|-------------|------|--------|-------|\n"
    └── STATE.md          # Initially: "# State\n\n**Phase:** idle\n**Progress:** \n**Last Action:** \n**Timestamp:** <now>\n"
```

**Session ID generation:**
```go
// 8-char hex string via crypto/rand
// e.g., "a1b2c3d4", "f0e9d8c7"
```

**LoadSession tolerance:**
- `session.json` missing → error (required)
- `messages.json` missing → empty slice (tolerated)
- `planning/PROJECT.md` missing → empty ProjectState (tolerated)
- `planning/TASKS.md` missing → empty task slice (tolerated)
- `planning/STATE.md` missing → zero WorkflowPhase (tolerated)

**Markdown parsing:**
- `PROJECT.md`: regex or line-by-line parse for `**Goal:**`, `**Type:**`, `**Framework:**`, and `## Questions` section
- `TASKS.md`: parse markdown table rows into `[]types.Task` (ID, Action, Description, Dependencies, Status, Files)
- `STATE.md`: parse `**Phase:**`, `**Progress:**`, `**Last Action:**`, `**Timestamp:**`

**Atomic writes:**
```go
func atomicWrite(path string, data []byte) error {
    dir := filepath.Dir(path)
    tmp, _ := os.CreateTemp(dir, ".m31a_tmp_*")
    tmp.Write(data)
    tmp.Close()
    os.Rename(tmp.Name(), path)
}
```

**ListSessions:**
```
~/.m31a/sessions/
├── a1b2c3d4/          → SessionInfo{ID: "a1b2c3d4", Model: "claude-sonnet-4", Provider: "openrouter", ...}
├── f0e9d8c7/          → SessionInfo{ID: "f0e9d8c7", Model: "gemini-2.5-pro", Provider: "zen", ...}
└── archived/          → skipped (not listed)
```

**Corrupted session detection:**
- If `session.json` exists but fails JSON parse → `Corrupted: true`
- If directory exists but no `session.json` → `Corrupted: true`

**Archive:**
```
mv ~/.m31a/sessions/<id>/ ~/.m31a/sessions/archived/<id>/
```

## Checkpoint System

```go
// checkpoint.json
{
    "checkpoints": [
        {"phase": "plan", "timestamp": "2026-05-28T10:00:00Z", "message_count": 5, "task_count": 8},
        {"phase": "execute", "timestamp": "2026-05-28T10:15:00Z", "message_count": 12, "task_count": 8}
    ]
}
```
- Max 2 retained. New checkpoint appended, oldest dropped if > 2.
- `/undo` reads latest checkpoint to know which phase to restore.

## Token Estimator (`internal/tokens/`)

**Estimation:**
```go
e := NewEstimator("claude-sonnet-4-20250514")
// tiktoken-go recognizes "claude" family → uses appropriate tokenizer
e.Estimate("Hello, world!") → ~4 tokens

e := NewEstimator("unknown-model-xyz")
// Fallback: len([]rune(text)) / 4 * 1.3
e.Estimate("Hello, world!") → 13 * 1.3 ≈ 17 tokens
```

**EMA Calibration:**
```go
// Start: emaFactor = 1.0, emaAlpha = 0.3
e.Calibrate(estimated=100, actual=120)
// emaFactor = 0.3 * (120/100) + 0.7 * 1.0 = 0.3 * 1.2 + 0.7 = 1.06

e.Calibrate(estimated=100, actual=115)
// emaFactor = 0.3 * (115/100) + 0.7 * 1.06 = 0.3 * 1.15 + 0.742 = 1.087

// After 3+ calibrations, error should be < 5%
```

**Format usage:**
```go
e.FormatUsage(80000, 128000) → "80,000 / 128,000 (62%)"
e.FormatUsage(110000, 128000) → "110,000 / 128,000 (86%)"  // above 80% threshold
```

**Context warning:**
```go
ContextWarningBanner(110000, 128000, 0.80)
// Returns styled string: "⚠ Context usage: 86% — approaching limit"
```

## Settings Screen (`internal/tui/settings.go`)

**Visual layout:**
```
┌──────────────────────── Settings ────────────────────────┐
│ [General]  Provider  Permissions  Features               │
│                                                         │
│  Theme:           [dark ▼]                              │
│  Default Model:   [claude-sonnet-4               ]      │
│  Context Warning: [80%                           ]      │
│  Compact Mode:    [✓]                                   │
│  Show Token Usage:[✓]                                   │
│  Show Cost Estimate:[✓]                                 │
│                                                         │
│  [Save]          [Discard]                              │
└─────────────────────────────────────────────────────────┘
```

**Provider tab:**
```
│  OpenRouter API Key: [••••••••••••••••] [Show] [Store in keychain] │
│  Zen API Key:        [••••••••••••••••] [Show] [Store in keychain] │
│  Default Provider:   [openrouter ▼]                                │
│  Auto Fallback:      [✓]                                           │
```

## Resume Screen (`internal/tui/resume.go`)

**Visual layout:**
```
┌────────────────────── Resume Session ────────────────────┐
│                                                         │
│  > a1b2c3d4  claude-sonnet-4  [OR]  2026-05-28  12 msgs │
│    f0e9d8c7  gemini-2.5-pro   [ZEN] 2026-05-27   8 msgs │
│    [!] b3c4d5e6  (corrupted)                            │
│                                                         │
│  Enter=Resume  N=New  D=Delete  Esc=Back                │
└─────────────────────────────────────────────────────────┘
```

## Test Expectations

| Package | Tests | Key Scenarios |
|---------|-------|---------------|
| `internal/config/` | 12 | Default config, env override, TOML parse, atomic save, M31A_CONFIG override, key resolution order |
| `pkg/keychain/` | 8 | Set/get/delete roundtrip, pass CLI fallback, service name format, errors |
| `pkg/session/` | 18 | New session, load roundtrip, list, archive, delete, corrupted detection, missing files tolerance, markdown parse/write |
| `pkg/session/` (checkpoint) | 4 | Save/load, max 2 retention, latest, empty |
| `internal/tokens/` | 14 | Tiktoken estimate, fallback, EMA convergence, format usage, context warning |
| `internal/tui/settings.go` | 6 | Render view, tab cycling, key masking, save, discard |
| `internal/tui/resume.go` | 8 | Session list render, corrupted badge, navigation, delete confirm |
| **Total** | **70** | |

## Build Verification

After implementation:
```
go mod tidy                    # no errors
CGO_ENABLED=0 go build -o m31a ./cmd/m31a  # success
go vet ./...                   # no issues
go test -race -cover ./...     # 389+ tests pass (319 existing + 70 new)
```

## Things to Watch For

1. **Build tags** — `//go:build linux`, `//go:build darwin`, `//go:build windows` must be correct. On Linux, `keychain_darwin.go` and `keychain_windows.go` must NOT compile.
2. **tiktoken-go model matching** — The library matches by model family prefix (e.g., "claude" → Claude tokenizer, "gpt" → GPT tokenizer). Unknown models must fall back to the `len(runes)/4*1.3` formula.
3. **Atomic write directory** — Temp file must be in the SAME directory as the target for `os.Rename()` to work (no cross-device rename).
4. **Markdown parser tolerance** — Extra blank lines, trailing whitespace, and missing optional fields should NOT cause parse failures.
5. **Session ID uniqueness** — `crypto/rand` hex encoding of 4 random bytes = 8 hex chars. Collision probability is negligible for local use.

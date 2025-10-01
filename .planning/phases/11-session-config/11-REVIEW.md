---
phase: 11-session-config
reviewed: 2026-06-06T00:00:00Z
depth: deep
files_reviewed: 23
files_reviewed_list:
  - cmd/m31a/main.go
  - internal/config/loader.go
  - internal/errors/errors.go
  - internal/git/git.go
  - internal/log/log.go
  - internal/provider/cache.go
  - internal/provider/common.go
  - internal/provider/fallback.go
  - internal/provider/openrouter/client.go
  - internal/provider/sse.go
  - internal/provider/zen/client.go
  - internal/tokens/estimator.go
  - internal/tools/bash_unix.go
  - internal/tools/bash_windows.go
  - internal/tools/constants.go
  - internal/tools/dispatcher.go
  - internal/tools/permissions.go
  - internal/tools/webfetch.go
  - internal/tui/commands_config.go
  - internal/tui/components/permission.go
  - internal/tui/components/question.go
  - pkg/keychain/keychain_linux.go
  - pkg/keychain/keychain_windows.go
findings:
  critical: 1
  warning: 3
  info: 1
  total: 5
status: issues_found
---

# Phase 11: Code Review Report — Structural Wiring

**Reviewed:** 2026-06-06T00:00:00Z
**Depth:** deep
**Files Reviewed:** 23
**Status:** issues_found

## Summary

Structural wiring review of the entire M31A Go codebase. Primary checks: import cycles, cross-platform build correctness, architecture dependency rules, go.mod hygiene, and build tag coverage. The codebase is well-structured overall — no import cycles, all tests pass on Linux, `go vet` is clean. One **critical** finding: a Windows build failure from an unguarded Linux-only syscall. Three architecture dependency rule violations (warnings) that don't cause build failures but contradict the documented architecture.

---

## Build & Test Status

| Check | Result |
|-------|--------|
| `go vet ./...` | ✅ Clean (0 warnings) |
| `go build ./...` (linux) | ✅ Clean |
| `go build ./...` (darwin) | ✅ Clean |
| `go build ./...` (windows) | ❌ **FAIL** — `internal/tui/commands_config.go:400-401` |
| `go test ./...` | ✅ All 21 packages pass |

---

## Critical Issues

### CR-01: Windows build failure — `syscall.Statfs_t` / `syscall.Statfs` without build tag

**File:** `internal/tui/commands_config.go:400-401`
**Issue:** The `/status` command uses `syscall.Statfs_t` and `syscall.Statfs()`, which are Linux-only syscalls. This file has **no build tag** and is compiled on all platforms. Windows builds fail with:

```
# github.com/eshanized/M31A/internal/tui
internal/tui/commands_config.go:400:22: undefined: syscall.Statfs_t
internal/tui/commands_config.go:401:21: undefined: syscall.Statfs
```

This is a **release-blocking issue** — the project's `goreleaser` config targets `windows/amd64` (per ROADMAP.md Phase 8) and this code makes the entire `internal/tui` package uncompilable on Windows.

**Fix:** Extract the disk-space logic into platform-specific files:

```go
// internal/tui/disk_unix.go
//go:build !windows

package tui

import "syscall"

func diskAvailableGB(homeDir string) (float64, bool) {
    var statfs syscall.Statfs_t
    if err := syscall.Statfs(homeDir, &statfs); err != nil {
        return 0, false
    }
    return float64(statfs.Bavail*uint64(statfs.Bsize)) / 1e9, true
}
```

```go
// internal/tui/disk_windows.go
//go:build windows

package tui

func diskAvailableGB(homeDir string) (float64, bool) {
    return 0, false // or use win32 API via syscall
}
```

Then `commands_config.go` calls `diskAvailableGB()` instead of inline syscall code.

---

## Warnings

### WR-01: Architecture violation — `internal/tools` imports `internal/config`

**File:** `internal/tools/dispatcher.go:12`, `internal/tools/permissions.go:11`, `internal/tools/defaults.go:3`
**Issue:** Per `docs/ARCHITECTURE.md:34`:

> `internal/tools/` may import `internal/types/`, `internal/errors/`.

But `internal/tools` imports `internal/config` in 3 files:

- `internal/tools/dispatcher.go:12` — reads `PermissionsConfig` for permission rules
- `internal/tools/permissions.go:11` — reads permission rule patterns
- `internal/tools/defaults.go:3` — reads default tool settings

This creates a bidirectional coupling risk: `internal/config` is a higher-level package that configures the application, while `internal/tools` is supposed to be a lower-level execution layer. The documented rule prevents this dependency direction.

**Fix:** Either update the architecture doc to explicitly allow `internal/tools → internal/config`, or inject configuration into tools at construction time (pass config values as parameters to `NewDispatcher()` rather than importing config directly).

### WR-02: Architecture violation — `internal/tui/components` imports `internal/tools`

**File:** `internal/tui/components/permission.go:8`, `internal/tui/components/question.go:7`
**Issue:** `internal/tui/components/` is a rendering subpackage of the TUI layer. It imports `internal/tools` for `tools.PermissionRequest`, `tools.PermissionResponse`, `tools.QuestionRequest`, `tools.QuestionResponse` types. While `internal/tui` is allowed to import `internal/tools` per the architecture doc, this creates tight coupling between a **UI rendering component** and the **tool execution layer**. If `tools.PermissionRequest` changes shape, the TUI rendering component must also change — violating separation of concerns.

**Fix:** Define shared request/response types in `internal/types/` (e.g., `types.PermissionRequest`, `types.QuestionRequest`) so both `internal/tools` and `internal/tui/components` depend only on the leaf `types` package. This eliminates the transitive `components → tools` dependency.

### WR-03: Architecture violation — `internal/config` imports `pkg/keychain`

**File:** `internal/config/loader.go:19`
**Issue:** Per `docs/ARCHITECTURE.md:33`:

> `internal/config/` may import `internal/types/`.

But `internal/config` imports `pkg/keychain` for API key resolution. While `pkg/keychain` is a leaf package (no internal imports itself), this dependency is not listed in the architecture rules. The `pkg/` packages are generally consumed by higher-level packages (`internal/tui`, `internal/workflow`), not by `internal/config` which is supposed to be a mid-level configuration layer.

**Fix:** Either:
1. Move `pkg/keychain` to `internal/keychain/` (since it's an internal implementation detail, not a public API) and update the architecture doc, or
2. Accept this as a documented exception and update `docs/ARCHITECTURE.md:33` to: `internal/config/ may import internal/types/, pkg/keychain/`.

---

## Info

### IN-01: No import cycles detected

**Files:** all `internal/` and `pkg/` packages
**Issue:** No findings. The import graph is a clean DAG:
- Leaf packages: `internal/types`, `internal/errors`, `internal/log`
- No circular dependencies between any packages
- `go build ./...` confirms this (Go compiler rejects cycles)

**Fix:** N/A — architecture is sound.

---

## Additional Structural Findings (No Action Required)

### Import cycle analysis — CLEAN

Full project-internal dependency graph (edges only):

```
cmd/m31a → config, log, provider, provider/openrouter, provider/zen, tui, types, keychain
internal/config → types, pkg/keychain
internal/errors → (none)
internal/git → (none)
internal/log → (none)
internal/provider → errors, types
internal/provider/openrouter → errors, provider, types
internal/provider/zen → errors, provider, types
internal/tokens → (none)
internal/tools → config, errors, types
internal/tui → config, errors, git, provider, tokens, tools, components, theme, types, workflow,
                pkg/arbitrage, pkg/autodream, pkg/keychain, pkg/ledger, pkg/rollback, pkg/session
internal/tui/components → tools, theme, types
internal/tui/theme → (none)
internal/types → (none)
internal/workflow → config, errors, git, provider, tokens, tools, types, pkg/bisect, pkg/ledger,
                    pkg/session, pkg/taskrunner
pkg/arbitrage → types
pkg/autodream → types
pkg/bisect → errors, git
pkg/keychain → (none)
pkg/ledger → errors, types
pkg/rollback → git, types
pkg/session → errors, types
pkg/taskrunner → errors, types
```

**No cycles found.** The dependency graph is a valid DAG.

### go.mod hygiene — CLEAN

- `go mod tidy -diff` produces no output (go.mod is clean)
- All 10 direct dependencies (`BurntSushi/toml`, `doublestar/v4`, `bubbles`, `bubbletea`, `glamour`, `lipgloss`, `godbus/dbus/v5`, `go-runewidth`, `tiktoken-go`, `golang.org/x/sync`) are imported in production code
- All indirect dependencies are transitively required by direct dependencies
- The `replace` directive for `tiktoken-go` is a no-op (replaces with the same version)

### Build tags — CORRECT (except CR-01)

| File | Build Tag | Purpose |
|------|-----------|---------|
| `internal/tools/bash_unix.go` | `//go:build !windows` | Unix PTY, signal handling |
| `internal/tools/bash_windows.go` | `//go:build windows` | Windows pipe fallback |
| `pkg/keychain/keychain_linux.go` | `//go:build linux` | Secret Service D-Bus |
| `pkg/keychain/keychain_darwin.go` | `//go:build darwin` | macOS Keychain CLI |
| `pkg/keychain/keychain_windows.go` | `//go:build windows` | Windows Credential Manager |

All platform-specific files have correct build tags **except** `internal/tui/commands_config.go` which uses Linux-only syscalls without a tag (CR-01).

### Unused imports — NONE DETECTED

All imports verified as used via `grep` of import symbols against file contents. Go compiler enforces this at build time (unused imports are compile errors in Go).

---

## Corrected Frontmatter (recalculated)

```yaml
findings:
  critical: 1
  warning: 3
  info: 1
  total: 5
```

---

_Reviewed: 2026-06-06T00:00:00Z_
_Reviewer: the agent (gsd-code-reviewer)_
_Depth: deep_

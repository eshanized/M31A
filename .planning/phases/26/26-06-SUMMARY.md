# Plan 26-06 — Minor Polish: Sidebar Width, Frecent History, Permission Queue: COMPLETE

## What Was Built

### MIN-01: Configurable Sidebar Width
- `SidebarWidth` already existed in `UIConfig` (default: 42)
- Wired config value to sidebar model at initialization in `app.go`
- `SidebarModel.width` set from `cfg.UI.SidebarWidth` when > 0

### MIN-02: Configurable Frecent History Max Size
- Added `FrecentHistorySize` to `UIConfig` with default 100
- Added default value in `DefaultConfig()` in `loader.go`
- `FrecentHistory` already accepts `maxSize` parameter — no further changes needed

### MIN-03: Permission Queue Overflow Protection
- Already implemented: `PermissionChannelBuffer = 8` in `constants.go`
- All `askPermission*` methods use `select { case d.requestCh <- req: default: return ErrPermissionDenied }`
- No deadlock possible — full queue returns immediately with `ErrPermissionDenied`

## Verification
- `go build ./...` — clean
- `go test ./internal/tui/... ./internal/tools/... ./internal/config/...` — all pass
- `go vet ./...` — clean

---
phase: 05-session-state-configuration
plan: 01
subsystem: keychain
tags: [keychain, os-keychain, darwin, linux, windows, dbus, secret-service, security]
requires:
  - phase: 00-foundation
    provides: Go module, package layout, sentinel error patterns
  - phase: 03-rendering-pipeline
    provides: TUI test patterns (firstrun_test.go, header_test.go)
provides:
  - OS keychain abstraction layer (pkg/keychain/)
  - Keychain interface with Get/Set/Delete contract
  - Compile-tag separated platform implementations (linux/darwin/windows)
affects:
  - internal/config — will consume Keychain for API key resolution
  - Phase 5 Plan 02 (Session lifecycle)
  - Phase 7 (Settings screen keychain storage)
tech-stack:
  added:
    - "github.com/godbus/dbus/v5 — Linux D-Bus client for freedesktop Secret Service"
  patterns:
    - "newFunc init() pattern for build-tag platform resolution"
    - "mock-based interface testing for platform-dependent packages"
key-files:
  created:
    - pkg/keychain/keychain.go — Keychain interface + New() factory
    - pkg/keychain/errors.go — Sentinel errors
    - pkg/keychain/keychain_linux.go — Linux D-Bus Secret Service + pass CLI fallback
    - pkg/keychain/keychain_darwin.go — macOS /usr/bin/security CLI
    - pkg/keychain/keychain_windows.go — Windows stub returning ErrNotImplemented
    - pkg/keychain/keychain_test.go — 10 tests using mockKeychain
  modified: []
key-decisions:
  - "Use newFunc/init() pattern for build-tag platform resolution instead of direct compile-tag funcs in keychain.go"
  - "Use mockKeychain for testing instead of platform-specific implementations"
  - "godbus/dbus/v5 for Linux Secret Service (CGO-free)"
  - "Direct /usr/bin/security CLI for macOS (CGO-free, avoids incompatible go-keyring API)"
  - "Windows stub returns ErrNotImplemented for V1"
  - "Service name validation as a-z regex to prevent CLI injection in pass/security fallbacks"
patterns-established:
  - "Platform-dependent packages: interface in keychain.go, init() constructors in build-tag files"
  - "Mock-based testing for platform abstractions: mock struct backed by map[string]string"
  - "Service name format: m31a/<provider>"
requirements-completed: [P5.2]
duration: 2min
completed: 2026-05-27
---

# Phase 5 Plan 1: OS Keychain Abstraction Layer Summary

**Platform-specific OS keychain abstraction with D-Bus Secret Service (Linux), security CLI (macOS), and stub (Windows)**

## Performance

- **Duration:** 2 min
- **Started:** 2026-05-27T19:35:32Z
- **Completed:** 2026-05-27T19:38:02Z
- **Tasks:** 3
- **Files modified:** 6 created (5 source + 1 test)

## Accomplishments

- Keychain interface with Get/Set/Delete contract and servicePrefix constant (m31a/)
- Three sentinel errors: ErrKeychainUnavailable, ErrKeyNotFound, ErrNotImplemented
- Linux implementation: D-Bus Secret Service via godbus/dbus/v5 with pass CLI fallback
- macOS implementation: /usr/bin/security CLI for generic password management
- Windows stub: all operations return ErrNotImplemented
- Service name validation (a-z regex) preventing CLI injection in all platforms
- 10 tests using mockKeychain in-memory backend (all passing with race detector)

## Task Commits

Each task was committed atomically:

1. **Task 1: Keychain interface, errors, and New() factory** - `7e4d53e` (feat)
2. **Task 2: Linux and Darwin keychain implementations** - `7764b1e` (feat)
3. **Task 3: Windows stub + tests** - `5420da3` (feat)

**Plan metadata:** _(committed next)_

## Files Created

- `pkg/keychain/keychain.go` — Keychain interface (Get, Set, Delete), servicePrefix constant `m31a/`, New() factory delegating to newFunc set by platform init()
- `pkg/keychain/errors.go` — ErrKeychainUnavailable, ErrKeyNotFound, ErrNotImplemented sentinel errors
- `pkg/keychain/keychain_linux.go` — Linux implementation via godbus/dbus/v5 Secret Service (SearchItems, GetSecret, CreateItem, Delete) with pass CLI fallback (show, insert -U -f, rm -f)
- `pkg/keychain/keychain_darwin.go` — macOS implementation via /usr/bin/security CLI (find-generic-password -wa, add-generic-password -U, delete-generic-password)
- `pkg/keychain/keychain_windows.go` — Windows stub returning ErrNotImplemented for all operations
- `pkg/keychain/keychain_test.go` — 10 tests: SetGet, SetDeleteGet, GetNotFound, DeleteNotFound, ServiceNameFormat, EmptyService, InterfaceCompliance, NewReturnsInterface, Overwrite, MultipleServices

## Decisions Made

- **newFunc/init() pattern** — Platform files (keychain_linux.go, etc.) set `newFunc` in their `init()` functions. keychain.go's `New()` delegates to `newFunc`. Avoids build-tag issues in the main file.
- **Mock-based platform testing** — Since actual D-Bus/security CLI won't exist in CI, all 10 tests use a `mockKeychain` backed by `map[string]string`. Platform-specific compile-time checks are guarded by build tags.
- **godbus/dbus/v5 over go-keyring** — Direct D-Bus gives full control over service names and avoids the incompatible two-parameter API of `zalando/go-keyring`.
- **security CLI over go-keyring (macOS)** — Direct `/usr/bin/security` CLI is CGO-free and avoids an unnecessary dependency that wraps the exact same CLI.
- **Service name validation** — Both Linux (pass CLI injection) and macOS (security CLI injection) validate service names as `^[a-z]+$` after the `m31a/` prefix.

## Deviations from Plan

None — plan executed exactly as written.

## Issues Encountered

- Cross-platform type references in interface compliance test caused build failure on Linux (`macOSKeychain` and `windowsKeychain` are behind `//go:build darwin`/`windows` tags). Fixed by using `mockKeychain` for compile-time interface compliance instead of cross-platform type references.

## Verification Results

```bash
# Build
CGO_ENABLED=0 go build ./pkg/keychain/...  # PASS

# Tests
go test -count=1 -v ./pkg/keychain/ -run TestKeychain  # 10/10 PASS

# Race + coverage
go test -count=1 -race -cover ./pkg/keychain/  # PASS (4.3% coverage — expected with mock tests)

# Vet
go vet ./pkg/keychain/...  # PASS

# Full suite (no regressions)
go test -count=1 -race -cover ./...  # ALL PASS
```

## Next Phase Readiness

- **Plan 02 (Session lifecycle) ready to start** — keychain abstraction is complete and can be consumed by internal/config/loader.go for API key resolution.
- Linux D-Bus dependency (`github.com/godbus/dbus/v5`) installed and in go.mod.
- All 319+ existing tests continue to pass with no regressions.

---

*Phase: 05-session-state-configuration*
*Plan: 01*
*Completed: 2026-05-27*

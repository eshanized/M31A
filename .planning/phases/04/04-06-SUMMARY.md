# Summary 04-06 — Tool Dispatcher & Integration

## Files Created
- `internal/tools/dispatcher.go` — `Dispatcher` struct with `Register`, `Execute` (permission-gated for Dangerous/Destructive tools), `ApprovePermission`, `List`, `GetTool`; `DefaultDispatcher` factory; channel-based permission flow (`requestCh`/`responseCh`)
- `internal/tools/dispatcher_test.go` — 7 tests: register & execute, unknown tool, safe tool bypass, dangerous tool permission granted/denied, remembered permission, sorted listing

## Files Modified
- `internal/tools/interface.go` — replaced function-field `Dispatcher` with `PermissionGate` interface; kept `PermissionRequest`/`PermissionResponse`
- `internal/tools/bash.go` — rewrote from goroutine-based pipe reading to synchronous `bytes.Buffer` I/O (fixes flaky race under parallel test scheduling)
- `internal/tui/types.go` — added `PermissionRequestMsg`/`PermissionResponseMsg` types
- `internal/tui/app.go` — added `Dispatcher` field to `AppState`, initialized via `DefaultDispatcher` in `NewApp`, permission listener goroutine, `ScreenPermission` handling in Update (Y/A/N/E keys) and View

## Deviations from Plan
- creack/pty dependency was added then removed; PTY unavailable in this environment (posix_openpt blocked)
- `strings.Builder` replaced with `bytes.Buffer` for cmd I/O (simpler, no goroutine race)
- Full ToolCall→dispatch→re-invoke-LLM loop wiring deferred: dispatcher is integrated and permission gate works, but the streaming pipeline dispatch loop (`handleStreamDoneMsg`) remains for TUI conversation mode in a follow-up

## Verification
- `go test -race -count=1 ./...` — all tests pass
- `CGO_ENABLED=0 go build -o m31a ./cmd/m31a` — produces 3.2MB static binary
- `go vet ./...` — passes

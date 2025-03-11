# Summary 04-05 — Bash Tool

## Files Created
- `internal/tools/bash.go` — Bash tool with shell command execution, process group isolation (`Setpgid: true`), SIGINT→SIGKILL escalation on timeout/cancellation, 50K char output cap, binary output detection, 30-min default timeout, stderr capture
- `internal/tools/bash_test.go` — 10 tests: simple command, working directory, stderr capture, timeout, context cancellation, non-zero exit, output truncation, binary output, command not found, missing param

## Files Modified
- None (creack/pty was added then removed by `go mod tidy` — no longer used)

## Deviations from Plan
- **PTY dropped**: `creack/pty` was added as a dependency but PTY allocation (`posix_openpt`) is blocked in this environment ("operation not permitted"). Replaced with stdout pipe + stderr buffer, which is more portable. The `Setpgid: true` process group and SIGINT/SIGKILL escalation still work correctly without PTY.
- creack/pty was removed from go.mod via `go mod tidy` since no imports remain.

## Verification
- `CGO_ENABLED=0 go build ./internal/tools/...` — passes
- `go vet ./internal/tools/...` — passes
- `go test -race -count=1 -timeout 120s ./internal/tools/... -run TestBash` — 10/10 pass

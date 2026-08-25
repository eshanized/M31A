# Phase 4 Deferred Items

Items discovered during execution that are OUT OF SCOPE for the executing plan (pre-existing issues in unrelated files, per executor scope-boundary rule).

## From 04-01 execution (2026-08-25)

1. **Four legacy packages fail to compile (pre-existing, blocks `go build ./...` and full `make test-fast`):**
   - `internal/engine/session` — references `types.ProjectState`, `Session.MessageCount`, `Session.ChildrenIDs`, string `session.ID`
   - `internal/engine/taskrunner` — references `types.StatusPending/StatusDone/StatusFailed/StatusSkipped/StatusRunning/StatusUnrecoverable`
   - `internal/integrations/ledger` — references `Session.Project`, string `session.ID`
   - `internal/tools/todo` — references `types.StatusDone/StatusRunning/StatusFailed/StatusSkipped/StatusUnrecoverable`

   These packages target a pre-reset domain vocabulary that no longer exists in `internal/core/types`. Verified identical failure set at HEAD before any 04-01 change. Belongs to the architectural reset migration work, not intelligence features.

2. **golangci-lint toolchain mismatch (environment):** golangci-lint v2.12.2 built with go1.26.3 crashes typechecking the stdlib under system go1.27.0 ("method must have no type parameters" inside `math/rand/v2`) on any package. `gofmt -l` and `go vet` pass and substitute for the lint guarantees this plan.

3. **`TestDetectWorkspaceRoot_None` (internal/integrations/codeintel/workspace_test.go) is environment-dependent:** `os.MkdirTemp(filepath.Join(projectRoot, "test-tmp"), ...)` requires `<repo>/test-tmp/` to already exist; fails on a fresh checkout with "no such file or directory". Pre-existing test bug, unrelated to types/config changes.

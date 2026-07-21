# E2E Tests

Canonical location for end-to-end tests.

## Current Status

The primary e2e test (`e2e_test.go`) remains at the project root because it compiles
and runs the actual binary via `go build ./cmd/m31a`. Moving it to this directory
would break the relative build path resolution (`./cmd/m31a` must be resolved from
the project root, not from a subdirectory).

The `prompt_simple.txt` fixture is available here for e2e tests via `go:embed`
(see `fixtures/e2e/sample.go`).

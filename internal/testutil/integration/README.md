# Integration Tests

Centralized location for cross-package integration tests.

## What Lives Here

- `tool_integration_test.go` — tests the Edit tool via real file I/O

## What Stays Colocated

Some integration tests remain in their source packages because they need access
to unexported symbols:

- `internal/workflow/integration_test.go` — uses unexported `multiTurnMockProvider`
  and `NewEngine` internal types. All APIs used are exported, but the mock provider
  pattern requires package-local types.
- `internal/config/integration_test.go` — uses unexported `newMockKeychain()`.
- `internal/provider/*/integration_test.go` — use unexported constructors
  (`New(apiKey, Options{})`) from their respective provider packages.

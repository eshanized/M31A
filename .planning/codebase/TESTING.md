# Testing Patterns

**Analysis Date:** [YYYY-MM-DD]

## Test Framework

**Runner:**
- Go test `testing` framework (built-in).
- Config: Invoked via Make targets (`Makefile`).

**Assertion Library:**
- Standard Go testing assertion logic. Conditionals with `if err != nil { t.Fatal(err) }` or `t.Errorf()` are used exclusively. No external testing assertions like `testify` are established in active module tests.

**Run Commands:**
```bash
make test              # Run all tests with race detector and coverage
make test-fast         # Run tests without race detector (faster)
make test-verbose      # Run tests with verbose output
make cover             # Generate HTML coverage report
```

## Test File Organization

**Location:**
- Co-located with the implementation. For example, `internal/config/loader.go` is tested alongside `internal/config/loader_test.go`.

**Naming:**
- Suffix `_test.go` on files matching the base implementation.

**Structure:**
```
internal/errors/
├── errors.go
└── errors_test.go
```

## Test Structure

**Suite Organization:**
```go
func TestBash_SimpleCommand(t *testing.T) {
        t.Parallel()
        // Setup ...
        
        // Execution ...
        
        // Assertion ...
}
```

**Patterns:**
- **Table-Driven Tests:** Substantial usage of `tests := []struct{...}` array looping to perform exhaustive permutation testing in logic-heavy files.
- **Parallel Testing:** `t.Parallel()` is aggressively utilized at the beginning of top-level test components and subtests.
- **Subtests:** Iterative tests are bundled inside `t.Run("Case Description", func(t *testing.T) { ... })`.

## Mocking

**Framework:** Custom interface mocking using vanilla Go.

**Patterns:**
```go
// mockKeychain implements the keychain.Keychain interface for testing.
type mockKeychain struct {
        store map[string]string
}

func newMockKeychain() *mockKeychain {
        return &mockKeychain{store: make(map[string]string)}
}

func (m *mockKeychain) Get(service string) (string, error) {
        if v, ok := m.store[service]; ok {
                return v, nil
        }
        return "", errors.New("not found")
}
```

**What to Mock:**
- System-level infrastructure boundaries (e.g., Keychains).
- External API calls (e.g., API Clients and Providers).

**What NOT to Mock:**
- Deterministic logic structures, parsers, algorithms, and models.

## Fixtures and Factories

**Test Data:**
```go
params := map[string]any{
        "command": `echo "hello world"`,
}
```

**Location:**
- Fixtures are embedded as struct inputs directly within the respective `_test.go` suite variables. Temporary file directories are constructed automatically using `t.TempDir()`.

## Coverage

**Requirements:** None rigorously enforced in linters natively, but HTML/CLI coverage outputs are a standard phase deliverable.

**View Coverage:**
```bash
make cover
```

## Test Types

**Unit Tests:**
- High priority. Logic modules are isolated with heavy table-driven permutations. Component-level tests directly call interface functions.

**Integration Tests:**
- Not strictly segregated. Some command workflow tests encompass broader component integration implicitly.

**E2E Tests:**
- Not strictly defined as separate runner suites in core tools, though test commands often exercise deep end-to-end tool chains logic locally.

## Common Patterns

**Error Testing:**
```go
if err != nil {
        t.Fatal(err)
}
if !strings.Contains(result.Output, "expected snippet") {
        t.Errorf("expected snippet in output, got: %s", result.Output)
}
```

**Resource Lifecycle:**
```go
b := NewBash(t.TempDir()) // Auto-cleaned via TempDir feature.
```

---

*Testing analysis: [YYYY-MM-DD]*
# Testing

**Last mapped:** 2026-08-02
**Project:** M31 Autonomous (Terminal AI Coding Agent)

## Test Framework

### Go Standard Testing

- **Package:** `testing` (standard library)
- **Runner:** `go test`
- **No external test frameworks** ( testify, gomock, etc.)

## Test Types

### Unit Tests

- **Location:** Same package as source (`*_test.go`)
- **Purpose:** Test individual functions and methods
- **Pattern:** Table-driven tests with parallel execution

### Integration Tests

- **Location:** `tests/testutil/integration/`
- **Purpose:** Test component interactions
- **Pattern:** Test helper utilities and mock dependencies

### End-to-End Tests

- **Location:** `tests/testutil/e2e/`
- **Purpose:** Test complete workflows
- **Pattern:** Full application simulation

## Test Organization

### File Naming

- **Standard:** `*_test.go` suffix
- **Example:** `file_read.go` → `file_read_test.go`
- **Location:** Same directory as source

### Function Naming

- **Pattern:** `TestFunctionName_Scenario`
- **Examples:**
  - `TestParseInput_ValidInput`
  - `TestReadFile_NotFound`
  - `TestWorkflowEngine_Execute`

### Table-Driven Tests

```go
func TestParseInput(t *testing.T) {
    t.Parallel()
    
    tests := []struct {
        name    string
        input   string
        want    Result
        wantErr bool
    }{
        {
            name:    "valid input",
            input:   "test",
            want:    Result{Value: "test"},
            wantErr: false,
        },
        {
            name:    "empty input",
            input:   "",
            wantErr: true,
        },
    }
    
    for _, tt := range tests {
        t.Run(tt.name, func(t *testing.T) {
            t.Parallel()
            got, err := ParseInput(tt.input)
            if (err != nil) != tt.wantErr {
                t.Errorf("ParseInput() error = %v, wantErr %v", err, tt.wantErr)
                return
            }
            if got != tt.want {
                t.Errorf("ParseInput() = %v, want %v", got, tt.want)
            }
        })
    }
}
```

## Test Execution

### Basic Commands

```bash
# Run all tests
go test ./...

# Run with race detector
go test -race ./...

# Run with coverage
go test -cover ./...

# Run specific test
go test -run TestFunctionName ./...

# Run verbose
go test -v ./...
```

### Makefile Targets

```bash
# Run tests with race detector and coverage
make test

# Run tests without race detector (faster)
make test-fast

# Run specific test
make test-specific TEST=TestFunctionName

# Generate HTML coverage report
make cover
```

### CI Integration

- **Race detector:** Always enabled in CI
- **Coverage threshold:** 75% overall, 90% for critical paths
- **Linting:** golangci-lint with 5-minute timeout

## Test Utilities

### Test Helpers (`tests/testutil/`)

- **`envtest.go`** - Environment variable testing utilities
- **`mocks/`** - Mock implementations
  - `dispatcher.go` - Tool dispatcher mock
  - `tool.go` - Tool interface mock
  - `provider.go` - Provider interface mock

### Mocking Patterns

#### Interface Mocks

```go
// Mock tool implementation
type MockTool struct {
    ExecuteFunc func(ctx context.Context, input []byte) ([]byte, error)
}

func (m *MockTool) Execute(ctx context.Context, input []byte) ([]byte, error) {
    return m.ExecuteFunc(ctx, input)
}
```

#### HTTP Mocking

```go
func TestWebFetch(t *testing.T) {
    server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
        w.WriteHeader(http.StatusOK)
        w.Write([]byte("<html><body>Test content</body></html>"))
    }))
    defer server.Close()
    
    // Use server.URL in tests
}
```

#### File System Mocking

```go
func TestReadFile(t *testing.T) {
    tempDir := t.TempDir()
    testFile := filepath.Join(tempDir, "test.txt")
    
    err := os.WriteFile(testFile, []byte("test content"), 0644)
    if err != nil {
        t.Fatal(err)
    }
    
    // Test with testFile
}
```

## Coverage

### Configuration

- **Overall target:** 75%
- **Critical paths:** 90% (`pkg/taskrunner`, `pkg/bisect`, `pkg/rollback`)
- **Exclusions:** Test files, generated code

### Commands

```bash
# Generate coverage profile
go test -coverprofile=coverage.out ./...

# Generate HTML report
go tool cover -html=coverage.out -o coverage.html

# View coverage in browser
make cover
```

### Analysis

```bash
# View coverage by function
go tool cover -func=coverage.out

# View coverage by package
go test -coverprofile=coverage.out ./... && \
go tool cover -func=coverage.out | grep total
```

## Benchmark Tests

### Location

- **Pattern:** `BenchmarkFunctionName` functions
- **Files:** `*_test.go` (same as unit tests)

### Commands

```bash
# Run benchmarks
go test -bench=. ./...

# Run with memory allocation stats
go test -bench=. -benchmem ./...

# Run specific benchmark
go test -bench=BenchmarkFunctionName ./...

# Run verbose benchmarks
go test -v -bench=. -benchmem ./...
```

### Example

```go
func BenchmarkParseInput(b *testing.B) {
    input := "test input for benchmark"
    
    b.ResetTimer()
    for i := 0; i < b.N; i++ {
        ParseInput(input)
    }
}
```

## Race Conditions

### Detection

- **Tool:** Go race detector (`-race` flag)
- **CI:** Always enabled in CI pipeline
- **Local:** `make test` enables race detection

### Common Issues

1. **Goroutine data races:** Use mutexes or channels
2. **Map concurrent access:** Use `sync.Map` or mutex
3. **Channel close races:** Use proper synchronization

### Debugging

```bash
# Run with race detector
go test -race ./...

# Run with verbose output
go test -race -v ./...

# Check for race conditions in specific test
go test -race -run TestFunctionName ./...
```

## Test Data

### Fixtures

- **Location:** `tests/fixtures/`
- **Purpose:** Test data files
- **Pattern:** Loaded in tests, cleaned up after

### Temporary Files

- **Pattern:** `t.TempDir()` for automatic cleanup
- **Example:**

```go
func TestWriteFile(t *testing.T) {
    tempDir := t.TempDir()
    testFile := filepath.Join(tempDir, "output.txt")
    
    // Test file operations
}
```

## Mocking Strategies

### Interface-Based Mocking

- **Pattern:** Create structs that implement interfaces
- **Location:** `tests/testutil/mocks/`
- **Usage:** Inject mocks in tests

### Table-Driven Mocks

```go
func TestToolExecution(t *testing.T) {
    tests := []struct {
        name     string
        tool     Tool
        input    []byte
        expected []byte
    }{
        {
            name: "successful execution",
            tool: &MockTool{
                ExecuteFunc: func(ctx context.Context, input []byte) ([]byte, error) {
                    return []byte("success"), nil
                },
            },
            input:    []byte("test"),
            expected: []byte("success"),
        },
    }
    
    for _, tt := range tests {
        t.Run(tt.name, func(t *testing.T) {
            result, err := tt.tool.Execute(context.Background(), tt.input)
            if err != nil {
                t.Fatalf("unexpected error: %v", err)
            }
            if !reflect.DeepEqual(result, tt.expected) {
                t.Errorf("got %v, want %v", result, tt.expected)
            }
        })
    }
}
```

## Test Helpers

### Common Patterns

```go
// Assert no error
func assertNoError(t *testing.T, err error) {
    t.Helper()
    if err != nil {
        t.Fatalf("unexpected error: %v", err)
    }
}

// Assert error
func assertError(t *testing.T, err error) {
    t.Helper()
    if err == nil {
        t.Fatal("expected error, got nil")
    }
}

// Assert equal
func assertEqual(t *testing.T, got, want interface{}) {
    t.Helper()
    if !reflect.DeepEqual(got, want) {
        t.Errorf("got %v, want %v", got, want)
    }
}
```

### Test Context

```go
func TestWithContext(t *testing.T) {
    ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
    defer cancel()
    
    // Use ctx in tests
}
```

## Continuous Integration

### GitHub Actions

- **Workflow:** `.github/workflows/ci.yml`
- **Triggers:** Push, pull request
- **Steps:**
  1. Checkout code
  2. Setup Go
  3. Run `make check` (fmt, tidy, vet, lint, test)
  4. Run `make test` with race detector
  5. Generate coverage report

### Quality Gates

- **Linting:** golangci-lint with 5-minute timeout
- **Testing:** All tests must pass
- **Coverage:** Must meet thresholds
- **Race detection:** No race conditions allowed
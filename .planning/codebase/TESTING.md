# TESTING.md — Testing Strategy & Practices

> Last mapped: 2026-07-10

## Test Framework & Commands

### Framework

- **Standard library `testing`** — No external test framework (testify not used despite being in `go.sum` as transitive dependency)
- **Table-driven tests** — Primary pattern using `t.Run()` with subtests
- **Parallel execution** — `t.Parallel()` used extensively for isolated tests
- **Test helpers** — `t.Helper()` for assertion wrappers, `t.TempDir()` for isolated temp directories

### Make Targets

| Target | Command | Purpose |
|--------|---------|---------|
| `make test` | `go test -race -cover -coverprofile=coverage.out ./...` | Full test suite with race detector + coverage |
| `make test-fast` | `go test -cover ./...` | Tests without race detector (faster) |
| `make test-verbose` | `go test -v -race -cover ./...` | Verbose output with race detector |
| `make test-specific TEST=TestFoo` | `go test -v -race -run TestFoo ./...` | Run single test by name |
| `make cover` | `go tool cover -html=coverage.out -o coverage.html` | Generate HTML coverage report |
| `make bench` | `go test -bench=. -benchmem -run=^$ ./...` | Run benchmarks |
| `make check` | `fmt → tidy → vet → lint → test` | Full CI pipeline |

### Coverage Targets

| Package | Target | Enforcement |
|---------|--------|-------------|
| Overall | 75% | CI gate via `make check` |
| `pkg/taskrunner` | 90% | High-coverage requirement |
| `pkg/bisect` | 90% | High-coverage requirement |
| `pkg/rollback` | 90% | High-coverage requirement |

Coverage is not enforced per-package via tooling — the 90% targets for critical packages are documented expectations validated during code review and release validation.

## Test Organization

### Unit Tests

- **Location**: `*_test.go` alongside source files
- **Package naming**: Same package as source (white-box) or `*_test` package (black-box, e.g., `m31a_test` for `e2e_test.go`)
- **Naming**: `Test<Function>_<Scenario>` or `Test<Feature>_<Behavior>`
- **Pattern**: Table-driven with `t.Run()` subtests

**Example** — `pkg/taskrunner/runner_test.go:130-144` (table-driven circular dependency test):

```go
func TestRunner_CircularDependency(t *testing.T) {
	tasks := []types.Task{
		newTask(1, "a", []int{2}),
		newTask(2, "b", []int{1}),
	}
	r := New(tasks)

	_, err := r.Schedule()
	if err == nil {
		t.Fatal("Expected error for circular dependency")
	}
	if err != m31errors.ErrCircularDependency {
		t.Errorf("Expected ErrCircularDependency, got %v", err)
	}
}
```

**Example** — `pkg/rollback/rollback_test.go:490-501` (table-driven with subtests):

```go
func TestMatchToolName(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name    string
		pattern string
		tool    string
		want    bool
	}{
		{"exact match", "Bash", "Bash", true},
		{"wildcard", "*", "Bash", true},
		{"glob prefix", "B*", "Bash", true},
		{"no match", "Bash", "FileRead", false},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := matchToolName(tt.pattern, tt.tool)
			if got != tt.want {
				t.Errorf("matchToolName(%q, %q) = %v, want %v", tt.pattern, tt.tool, got, tt.want)
			}
		})
	}
}
```

### Integration Tests

- **Provider integration tests**: `internal/provider/*/integration_test.go` — Test real APIs with `httptest` servers and optional live API keys
- **Real API tests**: Skip gracefully when env vars not set (see `internal/testutil/envtest.go`)

### E2E Tests (`e2e_test.go`)

- **Package**: `m31a_test` (external test package)
- **Approach**: Compile binary via `go build`, execute as subprocess
- **Real API tests**: `TestBinary_Prompt_*RealAPI` — require `OPENROUTER_API_KEY`, `ZEN_API_KEY`, `NVIDIA_API_KEY`
- **Skip behavior**: `t.Skip()` when env vars absent

**Example** — `e2e_test.go:52-73` (NVIDIA real API test):

```go
func TestBinary_Prompt_NvidiaRealAPI(t *testing.T) {
	apiKey := os.Getenv("NVIDIA_API_KEY")
	if apiKey == "" {
		t.Skip("NVIDIA_API_KEY not set — skipping real API test")
	}

	bin := buildBinary(t)
	cmd := exec.Command(bin, "--prompt", "What is 2+2? Reply with just the number.", "--model", "meta/llama-3.1-8b-instruct")
	cmd.Env = append(cleanEnv(), "NVIDIA_API_KEY="+apiKey)
	cmd.Dir = t.TempDir()

	out, err := cmd.CombinedOutput()
	if err != nil {
		t.Fatalf("headless mode failed: %v\noutput: %s", err, string(out))
	}
	resp := strings.TrimSpace(string(out))
	if resp == "" {
		t.Fatal("expected non-empty response from NVIDIA API")
	}
}
```

**Helper functions** (`e2e_test.go:150-191`):

```go
func buildBinary(t *testing.T) string {
	t.Helper()
	bin := filepath.Join(t.TempDir(), "m31a")
	cmd := exec.Command("go", "build", "-o", bin, "./cmd/m31a")
	cmd.Dir = filepath.Join(mustGetwd(t), ".")
	out, err := cmd.CombinedOutput()
	if err != nil {
		t.Fatalf("build failed: %v\n%s", err, string(out))
	}
	return bin
}

func cleanEnv() []string {
	return []string{
		"PATH=" + os.Getenv("PATH"),
		"HOME=" + os.TempDir(),
		"M31A_CONFIG=" + filepath.Join(os.TempDir(), "m31a-test-config.toml"),
	}
}
```

## Test Patterns

### Table-Driven Tests (Standard Pattern)

Used throughout the codebase. Structure:

```go
tests := []struct {
	name     string
	input    InputType
	want     ExpectedType
	wantErr  error // or wantErrCount int
}{
	{name: "case 1", input: ..., want: ..., wantErr: nil},
	{name: "case 2", input: ..., want: ..., wantErr: ErrSomething},
}
for _, tt := range tests {
	t.Run(tt.name, func(t *testing.T) {
		got, err := FunctionUnderTest(tt.input)
		if tt.wantErr != nil {
			if err == nil || !errors.Is(err, tt.wantErr) {
				t.Errorf("expected error %v, got %v", tt.wantErr, err)
			}
			return
		}
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}
		if got != tt.want {
			t.Errorf("expected %v, got %v", tt.want, got)
		}
	})
}
```

### Subtest Naming Convention

- `t.Run("description", func(t *testing.T) { ... })`
- Descriptive names: `"empty list"`, `"circular dependency"`, `"secret key denied by first rule"`

### Test Helpers

**Common patterns:**

```go
// Constructor helper with temp dir
func newTestManager(t *testing.T) (*Manager, string) {
	t.Helper()
	dir, err := os.MkdirTemp("", "m31a-session-test-*")
	if err != nil {
		t.Fatalf("Failed to create temp dir: %v", err)
	}
	return NewManager(dir, dir, ManagerOpts{}), dir
}

// Setup function returning cleanup
func setupTestEngine(t *testing.T) (*Engine, func()) {
	t.Helper()
	// ... setup ...
	return engine, func() { /* cleanup */ }
}

// Reusable test data builders
func newTask(id int, desc string, deps []int) types.Task {
	return types.Task{
		ID:           id,
		Description:  desc,
		Action:       "Create",
		Dependencies: deps,
	}
}
```

### Mocking Strategies

#### 1. Interface-Based Mocks (Primary)

Define interface in `internal/provider/interface.go`, implement mock in test file:

```go
// internal/workflow/engine_test.go:70-110
type mockProvider struct {
	response       string
	err            error
	callCount      int
	multiResponses []string
}

func (m *mockProvider) Name() string   { return "mock" }
func (m *mockProvider) APIKey() string { return "test-key" }
func (m *mockProvider) FetchModels(ctx context.Context) ([]m31types.ModelInfo, error) {
	return nil, nil
}
func (m *mockProvider) ChatCompletionStream(ctx context.Context, req provider.ChatRequest) (*m31types.StreamIterator, error) {
	m.callCount++
	content := m.response
	if len(m.multiResponses) > 0 {
		idx := m.callCount - 1
		if idx < len(m.multiResponses) {
			content = m.multiResponses[idx]
		}
	}
	done := false
	next := func() (*m31types.StreamChunk, error) {
		if done {
			return nil, io.EOF
		}
		done = true
		return &m31types.StreamChunk{Delta: content}, nil
	}
	close := func() error { return nil }
	return &m31types.StreamIterator{Next: next, Close: close}, m.err
}
// ... other interface methods
```

#### 2. Embedded Mock Extension

Extend base mock for specific behavior:

```go
// internal/workflow/engine_test.go:714-728
type mockProviderWithModel struct {
	mockProvider
	model *m31types.ModelInfo
}

func (m *mockProviderWithModel) GetModel(id string) (*m31types.ModelInfo, error) {
	return m.model, nil
}
func (m *mockProviderWithModel) CachedModels() []m31types.ModelInfo {
	if m.model != nil {
		return []m31types.ModelInfo{*m.model}
	}
	return nil
}
```

#### 3. `httptest.Server` for HTTP Clients

Used extensively in provider client tests (`internal/provider/zen/client_test.go`):

```go
ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
	if r.URL.Path == "/models" {
		w.WriteHeader(http.StatusOK)
		w.Write([]byte(`{"object":"list","data":[{"id":"test/model"}]}`))
	}
}))
defer ts.Close()

c, _ := New("test-key", Options{})
c.BaseURLField = ts.URL // Override base URL for test
models, err := c.FetchModels(context.Background())
```

#### 4. Permission/Callback Mocks via Channels

`internal/tools/dispatcher_test.go:89-112` — Test permission flow with goroutines and channels:

```go
errCh := make(chan error, 1)
go func() {
	_, err := d.Execute(context.Background(), types.ToolCall{
		ID: "call1", Name: "bash", Input: []byte(`{}`),
	})
	errCh <- err
}()

req := <-d.RequestCh() // Receive permission request
d.ApprovePermission(req.ID, true, false) // Approve
if err := <-errCh; err != nil {
	t.Errorf("expected nil error after approval, got: %v", err)
}
```

### Test Fixtures & Testdata

- **No dedicated `testdata/` directories** — Test data created inline via `t.TempDir()` and `os.WriteFile()`
- **Git repos for git-dependent tests**: Created on-the-fly in `pkg/bisect/bisect_test.go:14-45` and `pkg/rollback/rollback_test.go:16-40`
- **Golden files**: Not used; expected outputs encoded in test cases

**Example** — `pkg/bisect/bisect_test.go:14-45`:

```go
func setupBisectRepo(t *testing.T) (string, *Bisect) {
	t.Helper()
	dir := t.TempDir()
	runGit(t, dir, "init")
	runGit(t, dir, "config", "user.name", "Test")
	runGit(t, dir, "config", "user.email", "test@test.com")

	writeFile(t, dir, "main.go", "package main\nfunc main() {}\n")
	runGit(t, dir, "add", "-A")
	runGit(t, dir, "commit", "-m", "initial")
	// ... more commits ...
	b := New(dir, slog.Default())
	return dir, b
}
```

### Parallel Tests

- `t.Parallel()` used by default for isolated unit tests
- Tests that share state (e.g., modifying global config) omit `t.Parallel()`
- E2E tests don't run in parallel (binary build + execution)

## Race Detector

- **Enabled by default** in `make test` via `-race` flag
- **Required for CI** — `make check` runs full test suite with race detector
- **Bubble Tea constraint**: TUI is single-threaded; race detector catches cross-goroutine state mutations
- **Test patterns for race safety**: Channel-based communication, `t.Parallel()` isolation, no shared mutable state

## Linting & Static Analysis

### golangci-lint Config (`.golangci.yml`)

```yaml
version: "2"
run:
  timeout: 5m
linters:
  enable:
    - govet
    - staticcheck
    - errcheck
    - ineffassign
    - unused
settings:
  govet:
    enable:
      - shadow
  errcheck:
    check-type-assertions: false
    check-blank: false
exclusions:
  rules:
    - path: _test\.go
      linters:
        - errcheck
        - unused
```

### Enabled Linters

| Linter | Purpose |
|--------|---------|
| `govet` (+ `shadow`) | Standard Go vet checks + shadowed variable detection |
| `staticcheck` | Static analysis for bugs, performance, style |
| `errcheck` | Unchecked errors (disabled for test files) |
| `ineffassign` | Ineffective assignments |
| `unused` | Unused code (disabled for test files) |

### Test File Exclusions

- `_test.go` files excluded from `errcheck` and `unused`
- Allows `t.Fatal()` without checking return, test helpers without use

## Key Test Files by Category

### Core Package Tests (High Coverage Targets)

| File | Package | Focus |
|------|---------|-------|
| `pkg/taskrunner/runner_test.go` | `pkg/taskrunner` | Task scheduling, dependency resolution, execution (90% target) |
| `pkg/bisect/bisect_test.go` | `pkg/bisect` | Git bisect logic, log parsing, diff extraction (90% target) |
| `pkg/bisect/extra_test.go` | `pkg/bisect` | Edge cases, error paths |
| `pkg/rollback/rollback_test.go` | `pkg/rollback` | Git rollback operations, soft/hard/safe reset (90% target) |
| `pkg/rollback/doc_test.go` | `pkg/rollback` | Package documentation coverage |

### Provider Tests

| File | Focus |
|------|-------|
| `internal/provider/zen/client_test.go` | Zen provider HTTP client, streaming, error mapping |
| `internal/provider/zen/integration_test.go` | Live API integration (requires `ZEN_API_KEY`) |
| `internal/provider/openrouter/client_test.go` | OpenRouter client logic |
| `internal/provider/openrouter/integration_test.go` | Live OpenRouter API (requires `OPENROUTER_API_KEY`) |
| `internal/provider/nvidia/integration_test.go` | Live NVIDIA API (requires `NVIDIA_API_KEY`) |
| `internal/provider/interface_test.go` | Interface compliance verification |
| `internal/provider/fallback_test.go` | Fallback provider logic |
| `internal/provider/registry_test.go` | Provider registry, model discovery |

### Workflow Engine Tests

| File | Focus |
|------|-------|
| `internal/workflow/engine_test.go` | Engine initialization, phase transitions, prompt building, tool parsing |
| `internal/workflow/state_machine_test.go` | Phase state machine |
| `internal/workflow/phase_coordinator_test.go` | Phase coordination |
| `internal/workflow/intent_accuracy_test.go` | Intent classification accuracy |
| `internal/workflow/execute_quality_test.go` | Execution quality checks |
| `internal/workflow/verify_test.go` | Task verification |
| `internal/workflow/integration_test.go` | End-to-end workflow tests |

### Session & Persistence Tests

| File | Focus |
|------|-------|
| `pkg/session/manager_test.go` | Session CRUD, loading, timestamps |
| `pkg/session/checkpoint_test.go` | Checkpoint save/restore |
| `pkg/session/planning_test.go` | Planning artifacts |
| `pkg/session/session_resumedat_test.go` | Resume data serialization |

### Tooling Tests

| File | Focus |
|------|-------|
| `internal/tools/dispatcher_test.go` | Tool dispatch, permissions, rate limiting, agents |
| `internal/tools/bash_test.go` | Bash tool execution |
| `internal/tools/edit_test.go` | Edit tool |
| `internal/tools/glob_test.go` | Glob tool |
| `internal/tools/grep_test.go` | Grep tool |

### E2E & Binary Tests

| File | Focus |
|------|-------|
| `e2e_test.go` | Binary build, version/help, headless prompt, real API integration |

### Test Utilities

| File | Purpose |
|------|---------|
| `internal/testutil/envtest.go` | `RequireAPIKey()`, `RequireAnyAPIKey()`, `LoadTestDotEnv()` for live API tests |

## Running Tests Locally

### Full Suite (CI-equivalent)

```bash
make check
# Runs: fmt → tidy → vet → lint → test (with -race)
```

### Quick Iteration

```bash
make test-fast          # No race detector, faster feedback
make test-specific TEST=TestRunner_CircularDependency  # Single test
make test-verbose       # Verbose output with race detector
```

### Coverage Analysis

```bash
make cover              # Generates coverage.html
# Open coverage.html in browser
```

### Live API Integration Tests

```bash
# Set API keys in environment or .env file
export OPENROUTER_API_KEY=sk-or-...
export ZEN_API_KEY=sk-zen-...
export NVIDIA_API_KEY=nvapi-...

# Run integration tests (will skip if keys not set)
go test -v -run TestIntegration ./internal/provider/...

# Run E2E real API tests
go test -v -run TestBinary_Prompt_.*RealAPI .
```

### Benchmarks

```bash
make bench              # All benchmarks
make bench-verbose      # Verbose benchmark output
```

## Writing New Tests

### Conventions

1. **File location**: `*_test.go` next to source file
2. **Package**: Same as source for white-box; `*_test` for black-box (e.g., `e2e_test.go` uses `m31a_test`)
3. **Naming**: `Test<Function>_<Scenario>` or `Test<Feature>_<Behavior>`
4. **Structure**: Table-driven with `t.Run()` subtests
5. **Parallel**: Add `t.Parallel()` for isolated tests
6. **Helpers**: Use `t.Helper()` in test helper functions
7. **Temp dirs**: Use `t.TempDir()` for isolation
8. **Assertions**: Standard library only — `if err != nil { t.Fatalf(...) }`, `if got != want { t.Errorf(...) }`
9. **Error wrapping**: Use `errors.Is(err, ExpectedErr)` for sentinel errors
10. **Mocking**: Define interface in source, implement mock in test file

### Adding Coverage-Critical Tests

For `pkg/taskrunner`, `pkg/bisect`, `pkg/rollback` (90% target):

1. Run `make cover` and open `coverage.html`
2. Identify uncovered branches/functions
3. Add table-driven test cases covering those paths
4. Focus on error paths, edge cases, boundary conditions

### Integration Test Template

```go
func TestIntegration_Feature(t *testing.T) {
	testutil.LoadTestDotEnv(t) // Loads .env if present
	apiKey := testutil.RequireAnyAPIKey(t, "M31A_XXX_API_KEY", "XXX_API_KEY")
	
	// Test with real API
	// Use t.Skip() for permission errors, t.Fatal() for unexpected failures
}
```

## CI Integration

### GitHub Actions (Inferred from Makefile)

```yaml
# Equivalent to make check
- name: Run checks
  run: make check

# Coverage upload (if configured)
- name: Upload coverage
  uses: codecov/codecov-action@v3
  with:
    files: ./coverage.out
```

### Release Validation

```bash
make validate-release     # Runs scripts/validate-release.sh
# Produces VALIDATION_REPORT.md
```

## Common Patterns Reference

### Error Assertion

```go
// Sentinel error
if !errors.Is(err, m31errors.ErrCircularDependency) {
    t.Errorf("expected ErrCircularDependency, got %v", err)
}

// Error contains string
if !strings.Contains(err.Error(), "circular") {
    t.Errorf("expected error to mention 'circular', got %v", err)
}

// No error expected
if err != nil {
    t.Fatalf("unexpected error: %v", err)
}
```

### Stream Iterator Testing

```go
iterator := &m31types.StreamIterator{
    Next: func() (*m31types.StreamChunk, error) {
        // Return chunks, then io.EOF
    },
    Close: func() error { return nil },
}

chunk, err := iterator.Next()
if err != nil && err != io.EOF {
    t.Fatalf("unexpected error: %v", err)
}
```

### Goroutine Coordination in Tests

```go
errCh := make(chan error, 1)
go func() {
    _, err := functionUnderTest()
    errCh <- err
}()

// Wait for event or timeout
select {
case req := <-requestCh:
    // Handle request
case <-time.After(2 * time.Second):
    t.Fatal("timeout waiting for request")
}
```

---

*Testing analysis: 2026-07-10*
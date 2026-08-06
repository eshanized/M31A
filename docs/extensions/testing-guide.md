# Extension Testing Guide

**Audience:** Extension authors, M31A contributors  
**Goal:** Ensure extensions work correctly and maintain compatibility

---

## Testing Pyramid

```
        /\
       /  \     Contract Tests (few)
      /____\    Integration Tests (some)
     /      \   Unit Tests (many)
    /________\
```

---

## 1. Unit Testing

Test adapter logic in isolation using mock `SubprocessManager`.

### Mock SubprocessManager

```go
package extensions_test

import (
    "context"
    "encoding/json"
    "testing"

    "github.com/eshanized/M31A/pkg/extensions"
)

type MockSubprocessManager struct {
    CallFunc func(ctx context.Context, req extensions.JSONRPCRequest, timeout time.Duration) (extensions.JSONRPCResponse, error)
    StartFunc func(ctx context.Context) error
    StopFunc func() error
}

func (m *MockSubprocessManager) Call(ctx context.Context, req extensions.JSONRPCRequest, timeout time.Duration) (extensions.JSONRPCResponse, error) {
    if m.CallFunc != nil {
        return m.CallFunc(ctx, req, timeout)
    }
    return extensions.JSONRPCResponse{}, nil
}

func (m *MockSubprocessManager) Start(ctx context.Context) error {
    if m.StartFunc != nil {
        return m.StartFunc(ctx)
    }
    return nil
}

func (m *MockSubprocessManager) Stop() error {
    if m.StopFunc != nil {
        return m.StopFunc()
    }
    return nil
}

func (m *MockSubprocessManager) IsRunning() bool { return true }
func (m *MockSubprocessManager) ProtocolVersion() string { return "1.0" }
func (m *MockSubprocessManager) SupportedMethods() []string { return []string{} }
func (m *MockSubprocessManager) SetWorkDir(dir string) {}
```

### Test ExternalToolAdapter

```go
func TestExternalToolAdapter_Execute(t *testing.T) {
    mock := &MockSubprocessManager{}
    adapter := extensions.NewExternalToolAdapter("test-tool", mock)

    // Mock metadata fetch
    mock.CallFunc = func(ctx context.Context, req extensions.JSONRPCRequest, timeout time.Duration) (extensions.JSONRPCResponse, error) {
        switch req.Method {
        case extensions.MethodToolName:
            return extensions.JSONRPCResponse{JSONRPC: "2.0", ID: req.ID, Result: json.RawMessage(`"test-tool"`)}, nil
        case extensions.MethodToolDescription:
            return extensions.JSONRPCResponse{JSONRPC: "2.0", ID: req.ID, Result: json.RawMessage(`"A test tool"`)}, nil
        case extensions.MethodToolRiskLevel:
            return extensions.JSONRPCResponse{JSONRPC: "2.0", ID: req.ID, Result: json.RawMessage(`"safe"`)}, nil
        case extensions.MethodToolSchema:
            return extensions.JSONRPCResponse{JSONRPC: "2.0", ID: req.ID, Result: json.RawMessage(`{}`)}, nil
        case extensions.MethodToolExecute:
            return extensions.JSONRPCResponse{JSONRPC: "2.0", ID: req.ID, Result: json.RawMessage(`{"output":"done"}`)}, nil
        }
        return extensions.JSONRPCResponse{}, nil
    }

    // Test Execute
    result, err := adapter.Execute(context.Background(), types.ToolInput{})
    if err != nil {
        t.Fatalf("Execute failed: %v", err)
    }
    if result.Output != "done" {
        t.Errorf("expected output 'done', got %v", result.Output)
    }
}

func TestExternalToolAdapter_Metadata(t *testing.T) {
    mock := &MockSubprocessManager{}
    adapter := extensions.NewExternalToolAdapter("test-tool", mock)

    // Mock metadata calls
    mock.CallFunc = func(ctx context.Context, req extensions.JSONRPCRequest, timeout time.Duration) (extensions.JSONRPCResponse, error) {
        switch req.Method {
        case extensions.MethodToolName:
            return extensions.JSONRPCResponse{JSONRPC: "2.0", ID: req.ID, Result: json.RawMessage(`"my-tool"`)}, nil
        case extensions.MethodToolDescription:
            return extensions.JSONRPCResponse{JSONRPC: "2.0", ID: req.ID, Result: json.RawMessage(`"A test tool"`)}, nil
        case extensions.MethodToolRiskLevel:
            return extensions.JSONRPCResponse{JSONRPC: "2.0", ID: req.ID, Result: json.RawMessage(`"safe"`)}, nil
        case extensions.MethodToolSchema:
            return extensions.JSONRPCResponse{JSONRPC: "2.0", ID: req.ID, Result: json.RawMessage(`{}`)}, nil
        }
        return extensions.JSONRPCResponse{}, nil
    }

    if adapter.Name() != "my-tool" {
        t.Errorf("Name() = %q, want %q", adapter.Name(), "my-tool")
    }
    if adapter.Description() != "A test tool" {
        t.Errorf("Description() = %q, want %q", adapter.Description(), "A test tool")
    }
    if adapter.RiskLevel() != types.RiskSafe {
        t.Errorf("RiskLevel() = %v, want %v", adapter.RiskLevel(), types.RiskSafe)
    }
}
```

### Test ExternalProviderAdapter

```go
func TestExternalProviderAdapter_FetchModels(t *testing.T) {
    mock := &MockSubprocessManager{}
    adapter := extensions.NewExternalProviderAdapter("test-provider", mock)

    models := []types.ModelInfo{{ID: "model-1", Name: "Test Model"}}
    modelsData, _ := json.Marshal(map[string]any{"models": models})

    mock.CallFunc = func(ctx context.Context, req extensions.JSONRPCRequest, timeout time.Duration) (extensions.JSONRPCResponse, error) {
        if req.Method == extensions.MethodProviderFetchModels {
            return extensions.JSONRPCResponse{JSONRPC: "2.0", ID: req.ID, Result: modelsData}, nil
        }
        return extensions.JSONRPCResponse{}, nil
    }

    result, err := adapter.FetchModels(context.Background())
    if err != nil {
        t.Fatal(err)
    }
    if len(result) != 1 || result[0].ID != "model-1" {
        t.Errorf("unexpected models: %+v", result)
    }
}
```

### Test PhaseHookAdapter

```go
func TestPhaseHookAdapter_PrePhase(t *testing.T) {
    mock := &MockSubprocessManager{}
    adapter := extensions.NewPhaseHookAdapter("test-hook", mock, nil)

    payload := extensions.PhaseHookPayload{
        PhaseName: types.PhaseExecute,
        WorkflowState: extensions.WorkflowStateSnapshot{
            CurrentPhase: types.PhaseExecute,
            Goal:         "Test goal",
        },
    }

    mock.CallFunc = func(ctx context.Context, req extensions.JSONRPCRequest, timeout time.Duration) (extensions.JSONRPCResponse, error) {
        if req.Method == extensions.MethodHookPrePhase {
            return extensions.JSONRPCResponse{JSONRPC: "2.0", ID: req.ID, Result: json.RawMessage(`{}`)}, nil
        }
        return extensions.JSONRPCResponse{}, nil
    }

    err := adapter.PrePhase(context.Background(), payload)
    if err != nil {
        t.Fatal(err)
    }
}
```

---

## 2. Integration Testing

Test with real subprocess.

### Test Helper: Build Extension Binary

```go
func buildTestExtension(t *testing.T, dir string) string {
    t.Helper()
    binary := filepath.Join(t.TempDir(), "test-ext")
    cmd := exec.Command("go", "build", "-o", binary, ".")
    cmd.Dir = dir
    if out, err := cmd.CombinedOutput(); err != nil {
        t.Fatalf("build failed: %v\n%s", err, out)
    }
    return binary
}
```

### Test Tool Integration

```go
func TestToolIntegration(t *testing.T) {
    // Build test extension
    binary := buildTestExtension(t, "testdata/echo-tool")

    // Create config
    cfg := &config.Config{
        Extensions: extensions.ExtensionsConfig{
            Tools: map[string]extensions.ExternalToolConfig{
                "echo": {Command: binary, Timeout: "30s"},
            },
        },
    }

    // Start registry
    registry := extensions.NewExtensionRegistry(&cfg.Extensions, nil)
    ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
    defer cancel()

    if err := registry.Start(ctx); err != nil {
        t.Fatal(err)
    }
    defer registry.Stop()

    // Get tool
    tool, ok := registry.GetTool("echo")
    if !ok {
        t.Fatal("tool not registered")
    }

    // Execute
    result, err := tool.Execute(ctx, types.ToolInput{Data: map[string]any{"message": "hello"}})
    if err != nil {
        t.Fatal(err)
    }

    // Verify
    var output map[string]any
    json.Unmarshal([]byte(result.Output.(string)), &output)
    if output["output"] != "Echo: hello" {
        t.Errorf("unexpected output: %v", output)
    }
}
```

### Test Provider Integration

```go
func TestProviderIntegration(t *testing.T) {
    binary := buildTestExtension(t, "testdata/fake-provider")

    cfg := &config.Config{
        Extensions: extensions.ExtensionsConfig{
            Providers: map[string]extensions.ExternalProviderConfig{
                "test": {Command: binary, Timeout: "30s"},
            },
        },
    }

    registry := extensions.NewExtensionRegistry(&cfg.Extensions, nil)
    ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
    defer cancel()

    if err := registry.Start(ctx); err != nil {
        t.Fatal(err)
    }
    defer registry.Stop()

    provider, ok := registry.GetProvider("test")
    if !ok {
        t.Fatal("provider not registered")
    }

    models, err := provider.FetchModels(ctx)
    if err != nil {
        t.Fatal(err)
    }
    if len(models) == 0 {
        t.Error("expected at least one model")
    }
}
```

### Test Hook Integration

```go
func TestHookIntegration(t *testing.T) {
    binary := buildTestExtension(t, "testdata/pre-check")

    cfg := &config.Config{
        Extensions: extensions.ExtensionsConfig{
            Hooks: map[string]extensions.PhaseHookConfig{
                "pre-check": {
                    Command:    binary,
                    Phases:     []string{"execute"},
                    HookTypes:  []string{"pre"},
                    Timeout:    "30s",
                },
            },
        },
    }

    registry := extensions.NewExtensionRegistry(&cfg.Extensions, nil)
    ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
    defer cancel()

    if err := registry.Start(ctx); err != nil {
        t.Fatal(err)
    }
    defer registry.Stop()

    hooks := registry.GetHooks(types.PhaseExecute)
    if len(hooks) == 0 {
        t.Fatal("no hooks registered")
    }

    payload := extensions.PhaseHookPayload{
        PhaseName: types.PhaseExecute,
        WorkflowState: extensions.WorkflowStateSnapshot{CurrentPhase: types.PhaseExecute},
    }

    if err := hooks[0].PrePhase(ctx, payload); err != nil {
        t.Fatal(err)
    }
}
```

---

## 3. Contract Testing

Verify extension implements all required methods.

### Contract Test Template

```go
func TestExtensionContract(t *testing.T) {
    binary := buildTestExtension(t, "my-extension")

    ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
    defer cancel()

    proc := extensions.NewSubprocessManager(binary, nil, nil, 5*time.Second)
    if err := proc.Start(ctx); err != nil {
        t.Fatal(err)
    }
    defer proc.Stop()

    // Verify handshake
    resp, err := proc.Call(ctx, extensions.JSONRPCRequest{
        JSONRPC: "2.0", ID: json.RawMessage(`1`), Method: extensions.MethodHandshake,
    }, 5*time.Second)
    if err != nil {
        t.Fatal(err)
    }

    var handshake extensions.HandshakeResult
    json.Unmarshal(resp.Result, &handshake)
    if handshake.ProtocolVersion != "1.0" {
        t.Errorf("protocol version: got %s, want 1.0", handshake.ProtocolVersion)
    }

    // Verify all required methods present
    required := []string{
        "tool.name", "tool.description", "tool.risk_level", "tool.schema", "tool.execute",
    }
    for _, method := range required {
        found := false
        for _, m := range handshake.SupportedMethods {
            if m == method {
                found = true
                break
            }
        }
        if !found {
            t.Errorf("missing required method: %s", method)
        }
    }
}
```

---

## 4. CI Integration

### GitHub Actions Workflow

```yaml
# .github/workflows/extension-test.yml
name: Extension Tests
on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - uses: actions/setup-go@v7
        with:
          go-version: "1.25"
      - name: Build extension
        run: go build -o my-extension .
      - name: Run unit tests
        run: go test -race ./...
      - name: Run integration tests
        run: go test -race -tags=integration ./...
      - name: Contract test
        run: |
          ./my-extension <<< '{"jsonrpc":"2.0","id":1,"method":"handshake"}' | jq -e '.result.protocol_version == "1.0"'
```

### Test Matrix for Sample Extensions

```yaml
strategy:
  matrix:
    extension: [custom-linter, ollama-provider, pre-commit-hook]
    go-version: ["1.25"]
```

---

## 4. Debugging Tips

### Run Extension Manually

```bash
# Test handshake
echo '{"jsonrpc":"2.0","id":1,"method":"handshake"}' | ./my-extension

# Test tool execute
echo '{"jsonrpc":"2.0","id":2,"method":"tool.execute","params":{"input":{"message":"test"}}}' | ./my-extension

# Test with jq for pretty output
echo '{"jsonrpc":"2.0","id":1,"method":"tool.execute","params":{"input":{"path":"/tmp/test"}}}' | ./my-extension | jq
```

### Inspect JSON-RPC Traffic

```go
// In SubprocessManager, enable debug logging
slog.Debug("JSON-RPC request", "method", req.Method, "params", string(req.Params))
slog.Debug("JSON-RPC response", "id", req.ID, "result", string(resp.Result))
```

### Common Issues

| Issue | Cause | Fix |
|-------|-------|-----|
| Handshake timeout | Extension not reading stdin | Ensure reading loop in extension |
| Method not found | Missing method in handshake | Add to supported_methods |
| Execute hangs | Extension not writing stdout | Ensure response written to stdout |
| Zombie processes | Stop() not called | Call registry.Stop() in defer |
| JSON parse error | Stdout pollution | Log to stderr only |
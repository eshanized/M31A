# Migration Guide: Internal → External Extensions

**Audience:** M31A maintainers, extension authors  
**Goal:** Convert internal tools/providers/hooks to external extensions

---

## Why Migrate?

| Benefit | Description |
|---------|-------------|
| **Language Freedom** | Write in Python, Rust, Node.js, Bash, Go, etc. |
| **Independent Deployment** | Update extension without rebuilding M31A |
| **Isolation** | Crash in extension doesn't crash M31A |
| **Distribution** | Share via GitHub Releases, Homebrew, Scoop |
| **Team Autonomy** | Teams own their extensions independently |

---

## Tool Migration

### Before (Internal Tool)

```go
// internal/tools/mytool/mytool.go
package mytool

type MyTool struct{}

func (t *MyTool) Name() string        { return "my-tool" }
func (t *MyTool) Description() string { return "Does something" }
func (t *MyTool) RiskLevel() types.RiskLevel { return types.RiskSafe }
func (t *MyTool) ParameterSchema() string { return `{"type":"object"}` }

func (t *MyTool) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
    // Business logic here
    return types.ToolResult{Output: "done"}, nil
}

// Registration in dispatcher.go
dispatcher.Register(&MyTool{})
```

### After (External Tool)

#### 1. Extract Business Logic

```go
// external/mytool/main.go
package main

import (
    "context"
    "encoding/json"
    "log"
    "os"
    "os/signal"
    "syscall"
)

type Request struct {
    JSONRPC string          `json:"jsonrpc"`
    ID      json.RawMessage `json:"id"`
    Method  string          `json:"method"`
    Params  json.RawMessage `json:"params,omitempty"`
}

type Response struct {
    JSONRPC string          `json:"jsonrpc"`
    ID      json.RawMessage `json:"id"`
    Result  any             `json:"result,omitempty"`
    Error   *Error          `json:"error,omitempty"`
}

type Error struct {
    Code    int    `json:"code"`
    Message string `json:"message"`
}

func main() {
    ctx, cancel := context.WithCancel(context.Background())
    defer cancel()

    sigCh := make(chan os.Signal, 1)
    signal.Notify(sigCh, syscall.SIGTERM, syscall.SIGINT)
    go func() { <-sigCh; cancel() }()

    dec := json.NewDecoder(os.Stdin)
    enc := json.NewEncoder(os.Stdout)

    for {
        select {
        case <-ctx.Done():
            return
        default:
            var req Request
            if err := dec.Decode(&req); err != nil {
                log.Printf("decode: %v", err)
                continue
            }
            enc.Encode(handle(req))
        }
    }
}

func handle(req Request) Response {
    switch req.Method {
    case "handshake":
        return Response{JSONRPC: "2.0", ID: req.ID, Result: map[string]any{
            "protocol_version":  "1.0",
            "supported_methods": []string{"tool.name", "tool.description", "tool.risk_level", "tool.schema", "tool.execute"},
        }}
    case "tool.name":
        return Response{JSONRPC: "2.0", ID: req.ID, Result: "my-tool"}
    case "tool.description":
        return Response{JSONRPC: "2.0", ID: req.ID, Result: "Does something"}
    case "tool.risk_level":
        return Response{JSONRPC: "2.0", ID: req.ID, Result: "safe"}
    case "tool.schema":
        return Response{JSONRPC: "2.0", ID: req.ID, Result: `{}`}
    case "tool.execute":
        var p struct{ Input map[string]any `json:"input"` }
        json.Unmarshal(req.Params, &p)
        
        // ORIGINAL BUSINESS LOGIC HERE
        // (copy from internal tool's Execute method)
        
        return Response{JSONRPC: "2.0", ID: req.ID, Result: map[string]any{"output": "done"}}
    default:
        return Response{JSONRPC: "2.0", ID: req.ID, Error: &Error{Code: -32601, Message: "Method not found"}}
    }
}
```

#### 2. Update Config

```json
// m31a.json
{
  "extensions": {
    "tools": {
      "my-tool": {
        "command": "./my-tool",
        "args": [],
        "env": {},
        "timeout": "30s"
      }
    }
  }
}
```

#### 3. Remove Internal Registration

```go
// REMOVE from dispatcher.go:
// dispatcher.Register(&MyTool{})
```

---

## Provider Migration

### Before (Internal Provider)

```go
// internal/integrations/provider/myprovider/myprovider.go
type MyProvider struct{}

func (p *MyProvider) Name() string { return "my-provider" }
func (p *MyProvider) FetchModels() ([]types.ModelInfo, error) { ... }
func (p *MyProvider) ChatCompletionStream(req types.ChatRequest) (types.StreamIterator, error) { ... }
func (p *MyProvider) EstimateCost(req types.ChatRequest) float64 { ... }
func (p *MyProvider) HealthCheck() error { ... }

// Registration in registry.go
registry.Register(&MyProvider{})
```

### After (External Provider)

```go
// external/myprovider/main.go
func handle(req Request) Response {
    switch req.Method {
    case "handshake":
        return Response{JSONRPC: "2.0", ID: req.ID, Result: map[string]any{
            "protocol_version":  "1.0",
            "supported_methods": []string{"provider.name", "provider.fetch_models", "provider.chat_completion_stream", "provider.estimate_cost", "provider.health_check", "provider.get_model"},
        }}
    case "provider.name":
        return Response{JSONRPC: "2.0", ID: req.ID, Result: "my-provider"}
    case "provider.fetch_models":
        // Call original FetchModels logic
        models := originalFetchModels()
        data, _ := json.Marshal(map[string]any{"models": models})
        return Response{JSONRPC: "2.0", ID: req.ID, Result: json.RawMessage(data)}
    case "provider.chat_completion_stream":
        // Return ack with stream_id, then send notifications
        streamID := generateID()
        // Send initial ack
        ack := Response{JSONRPC: "2.0", ID: req.ID, Result: map[string]any{"stream_id": streamID}}
        // Then send notifications via stdout
        go func() {
            stream := originalChatCompletionStream(req)
            for chunk := range stream {
                notification := map[string]any{
                    "jsonrpc": "2.0",
                    "method":  "stream.chunk",
                    "params":  map[string]any{"stream_id": streamID, "chunk": chunk},
                }
                json.NewEncoder(os.Stdout).Encode(notification)
            }
            // End notification
            json.NewEncoder(os.Stdout).Encode(map[string]any{
                "jsonrpc": "2.0", "method": "stream.end", "params": map[string]any{"stream_id": streamID},
            })
        }()
        return ack
    case "provider.estimate_cost":
        return Response{JSONRPC: "2.0", ID: req.ID, Result: map[string]any{"cost_usd": 0.0}}
    case "provider.health_check":
        return Response{JSONRPC: "2.0", ID: req.ID, Result: map[string]any{"status": "healthy", "latency_ms": 10}}
    case "provider.get_model":
        // Handle get_model
        return Response{JSONRPC: "2.0", ID: req.ID, Result: map[string]any{"model": modelInfo}}
    }
}
```

---

## Hook Migration

### Before (Internal Hook)

```go
// internal/engine/workflow/myhook.go
func MyPreExecuteHook(ctx context.Context, payload HookPayload) error {
    // Pre-execute logic
    return nil
}

func MyPostExecuteHook(ctx context.Context, payload HookPayload, result *PhaseResult) error {
    // Post-execute logic
    return nil
}

// Registration in engine.go
e.hookRegistry.Register(types.PhaseExecute, MyPreExecuteHook)
e.hookRegistry.Register(types.PhaseExecute, MyPostExecuteHook)
```

### After (External Hook)

```go
// external/myhook/main.go
func handle(req Request) Response {
    switch req.Method {
    case "handshake":
        return Response{JSONRPC: "2.0", ID: req.ID, Result: map[string]any{
            "protocol_version":  "1.0",
            "supported_methods": []string{"hook.pre_phase", "hook.post_phase"},
        }}
    case "hook.pre_phase":
        // Unmarshal payload
        var payload struct {
            Payload HookPayload `json:"payload"`
        }
        json.Unmarshal(req.Params, &payload)
        
        // ORIGINAL PRE-PHASE LOGIC
        // MyPreExecuteHook(context.Background(), payload.Payload)
        
        return Response{JSONRPC: "2.0", ID: req.ID, Result: `{}`}
    case "hook.post_phase":
        var payload struct {
            Payload HookPayload   `json:"payload"`
            Result  PhaseResult   `json:"result"`
        }
        json.Unmarshal(req.Params, &payload)
        
        // ORIGINAL POST-PHASE LOGIC
        // MyPostExecuteHook(context.Background(), payload.Payload, &payload.Result)
        
        return Response{JSONRPC: "2.0", ID: req.ID, Result: `{}`}
    }
}
```

Config:
```json
"hooks": {
  "my-hook": {
    "command": "./my-hook",
    "phases": ["execute"],
    "hook_types": ["pre", "post"],
    "timeout": "30s"
  }
}
```

---

## Configuration Changes

| Internal | External |
|----------|----------|
| Code registration (`dispatcher.Register()`) | Config declaration (`m31a.json`) |
| Compile-time dependency | Runtime subprocess |
| Shared memory/state | JSON-RPC over stdio |
| Direct function calls | Async message passing |
| Panic recovery in M31A | Extension manages own crashes |

---

## Testing Parity

### Before Migration
```go
func TestMyTool(t *testing.T) {
    tool := &MyTool{}
    result, _ := tool.Execute(ctx, input)
    assert.Equal(t, "expected", result.Output)
}
```

### After Migration
```go
func TestMyToolExternal(t *testing.T) {
    binary := buildTestExtension(t, "my-tool")
    
    proc := extensions.NewSubprocessManager(binary, nil, nil, 30*time.Second)
    proc.Start(ctx)
    defer proc.Stop()
    
    adapter := extensions.NewExternalToolAdapter("my-tool", proc)
    result, _ := adapter.Execute(ctx, input)
    
    assert.Equal(t, "expected", result.Output)
}
```

---

## Rollback Strategy

1. **Keep internal version** during transition
2. **Feature flag** in config to toggle internal/external
3. **Run both in parallel** for comparison
4. **Monitor metrics** (latency, error rate, resource usage)
5. **Remove internal** after 2+ successful releases

```go
// Feature flag approach
if cfg.Features.UseExternalTools {
    RegisterExternalTools(dispatcher, extRegistry)
} else {
    dispatcher.Register(&MyTool{})  // Internal fallback
}
```

---

## Checklist

- [ ] Business logic extracted to standalone executable
- [ ] All required JSON-RPC methods implemented
- [ ] Handshake returns protocol_version "1.0"
- [ ] Supported methods listed in handshake
- [ ] Config updated with command, args, env, timeout
- [ ] Internal registration removed
- [ ] Unit tests updated to use external adapter
- [ ] Integration tests pass
- [ ] Performance benchmarked (target: <10ms overhead)
- [ ] Documentation updated
- [ ] Binary published to distribution channel
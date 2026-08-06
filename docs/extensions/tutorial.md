# Interactive Tutorial: Building Your First M31A Extension

**Time:** ~20 minutes  
**Prerequisites:** Go 1.25+, M31A installed

---

## Step 1: Create an Echo Tool (5 min)

Create a simple tool that echoes back input.

### 1.1 Create project structure

```bash
mkdir -p my-echo-tool
cd my-echo-tool
```

### 1.2 Create `main.go`

```go
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
        return Response{JSONRPC: "2.0", ID: req.ID, Result: "echo"}
    case "tool.description":
        return Response{JSONRPC: "2.0", ID: req.ID, Result: "Echoes input back"}
    case "tool.risk_level":
        return Response{JSONRPC: "2.0", ID: req.ID, Result: "safe"}
    case "tool.schema":
        s, _ := json.Marshal(map[string]any{
            "type": "object",
            "properties": map[string]any{
                "message": map[string]string{"type": "string"},
            },
        })
        return Response{JSONRPC: "2.0", ID: req.ID, Result: json.RawMessage(s)}
    case "tool.execute":
        var p struct{ Input map[string]any `json:"input"` }
        json.Unmarshal(req.Params, &p)
        msg := p.Input["message"].(string)
        return Response{JSONRPC: "2.0", ID: req.ID, Result: map[string]any{"output": "Echo: " + msg}}
    default:
        return Response{JSONRPC: "2.0", ID: req.ID, Error: &Error{Code: -32601, Message: "Method not found"}}
    }
}
```

### 1.3 Build and test

```bash
go build -o echo-tool .
echo '{"jsonrpc":"2.0","id":1,"method":"handshake"}' | ./echo-tool
# Expected: {"jsonrpc":"2.0","id":1,"result":{"protocol_version":"1.0","supported_methods":[...]}}

echo '{"jsonrpc":"2.0","id":2,"method":"tool.execute","params":{"input":{"message":"Hello"}}}' | ./echo-tool
# Expected: {"jsonrpc":"2.0","id":2,"result":{"output":"Echo: Hello"}}
```

✅ **Step 1 complete!**

---

## Step 2: Register in M31A Config (3 min)

### 2.1 Create `m31a.json` in project root

```json
{
  "extensions": {
    "tools": {
      "echo": {
        "command": "./my-echo-tool/echo-tool",
        "args": [],
        "timeout": "30s"
      }
    }
  }
}
```

### 2.2 Test with M31A

```bash
# Run M31A with a goal that uses your tool
m31a --goal "Use the echo tool to say hello"
```

✅ **Step 2 complete!**

---

## Step 3: Add Parameters & Schema (5 min)

### 3.1 Update schema to accept multiple fields

```go
case "tool.schema":
    s, _ := json.Marshal(map[string]any{
        "type": "object",
        "properties": map[string]any{
            "message": map[string]string{"type": "string", "description": "Message to echo"},
            "repeat":  map[string]any{"type": "integer", "description": "Times to repeat", "default": 1},
        },
        "required": []string{"message"},
    })
    return Response{JSONRPC: "2.0", ID: req.ID, Result: json.RawMessage(s)}
```

### 3.2 Update execute to use repeat

```go
case "tool.execute":
    var p struct{ Input map[string]any `json:"input"` }
    json.Unmarshal(req.Params, &p)
    msg := p.Input["message"].(string)
    repeat := 1
    if v, ok := p.Input["repeat"]; ok {
        repeat = int(v.(float64))
    }
    output := ""
    for i := 0; i < repeat; i++ {
        output += msg + " "
    }
    return Response{JSONRPC: "2.0", ID: req.ID, Result: map[string]any{"output": output}}
```

### 3.3 Rebuild and test

```bash
go build -o echo-tool .
echo '{"jsonrpc":"2.0","id":1,"method":"tool.execute","params":{"input":{"message":"Hi","repeat":3}}}' | ./echo-tool
# Expected: {"jsonrpc":"2.0","id":1,"result":{"output":"Hi Hi Hi "}}
```

✅ **Step 3 complete!**

---

## Step 4: Build a Provider Stub (5 min)

Create a minimal provider that returns fake models.

### 4.1 Create `provider/main.go`

```go
package main

import (
    "context"
    "encoding/json"
    "log"
    "os"
    "os/signal"
    "syscall"
)

// ... same Request/Response/Error structs as before ...

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
        case <-ctx.Done(): return
        default:
            var req Request
            if err := dec.Decode(&req); err != nil { log.Printf("decode: %v", err); continue }
            enc.Encode(handle(req))
        }
    }
}

func handle(req Request) Response {
    switch req.Method {
    case "handshake":
        return Response{JSONRPC: "2.0", ID: req.ID, Result: map[string]any{
            "protocol_version":  "1.0",
            "supported_methods": []string{"provider.name", "provider.fetch_models", "provider.chat_completion_stream", "provider.estimate_cost", "provider.health_check"},
        }}
    case "provider.name":
        return Response{JSONRPC: "2.0", ID: req.ID, Result: "fake-provider"}
    case "provider.fetch_models":
        models := []map[string]any{
            {"id": "fake-model", "name": "Fake Model", "context_length": 4096},
        }
        data, _ := json.Marshal(map[string]any{"models": models})
        return Response{JSONRPC: "2.0", ID: req.ID, Result: json.RawMessage(data)}
    case "provider.health_check":
        return Response{JSONRPC: "2.0", ID: req.ID, Result: map[string]any{"status": "healthy", "latency_ms": 10}}
    case "provider.estimate_cost":
        return Response{JSONRPC: "2.0", ID: req.ID, Result: map[string]any{"cost_usd": 0.0}}
    default:
        return Response{JSONRPC: "2.0", ID: req.ID, Error: &Error{Code: -32601, Message: "Method not found"}}
    }
}
```

### 4.2 Build and register

```bash
cd provider && go build -o fake-provider .
```

Add to `m31a.json`:

```json
"providers": {
  "fake-provider": {
    "command": "./provider/fake-provider",
    "args": [],
    "timeout": "30s"
  }
}
```

✅ **Step 4 complete!**

---

## Step 5: Build a Hook Extension (5 min)

Create a pre-execute hook that runs a style check.

### 5.1 Create `hook/main.go`

```go
package main

import (
    "context"
    "encoding/json"
    "log"
    "os"
    "os/signal"
    "syscall"
)

// ... same Request/Response/Error ...

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
        case <-ctx.Done(): return
        default:
            var req Request
            if err := dec.Decode(&req); err != nil { log.Printf("decode: %v", err); continue }
            enc.Encode(handle(req))
        }
    }
}

func handle(req Request) Response {
    switch req.Method {
    case "handshake":
        return Response{JSONRPC: "2.0", ID: req.ID, Result: map[string]any{
            "protocol_version":  "1.0",
            "supported_methods": []string{"hook.pre_phase", "hook.post_phase"},
        }}
    case "hook.pre_phase":
        // Run gofmt or similar check
        log.Println("Pre-phase hook triggered!")
        return Response{JSONRPC: "2.0", ID: req.ID, Result: map[string]any{}}
    case "hook.post_phase":
        return Response{JSONRPC: "2.0", ID: req.ID, Result: map[string]any{}}
    default:
        return Response{JSONRPC: "2.0", ID: req.ID, Error: &Error{Code: -32601, Message: "Method not found"}}
    }
}
```

### 5.2 Register hook

```bash
cd hook && go build -o pre-check .
```

Add to `m31a.json`:

```json
"hooks": {
  "pre-check": {
    "command": "./hook/pre-check",
    "args": [],
    "phases": ["execute", "verify"],
    "hook_types": ["pre"],
    "timeout": "30s"
  }
}
```

✅ **Step 5 complete!**

---

## Step 6: Package & Distribute (2 min)

### 6.1 Build for all platforms

```bash
# Linux
GOOS=linux GOARCH=amd64 CGO_ENABLED=0 go build -o echo-tool-linux-amd64 .

# macOS
GOOS=darwin GOARCH=arm64 CGO_ENABLED=0 go build -o echo-tool-darwin-arm64 .

# Windows
GOOS=windows GOARCH=amd64 CGO_ENABLED=0 go build -o echo-tool-windows-amd64.exe .
```

### 6.2 Create GitHub Release

1. Push to GitHub
2. Create release with tag `v1.0.0`
3. Upload all 3 binaries

### 6.3 Homebrew (optional)

```ruby
class EchoTool < Formula
  url "https://github.com/you/echo-tool/releases/download/v1.0.0/echo-tool-darwin-arm64"
  sha256 "..."
  def install; bin.install "echo-tool-darwin-arm64" => "echo-tool"; end
end
```

---

## Verification Checklist

- [ ] Handshake returns protocol_version 1.0
- [ ] All required methods implemented
- [ ] Tool executes and returns correct output
- [ ] Provider returns models and health status
- [ ] Hook fires at correct phase transitions
- [ ] Config loads without errors
- [ ] Graceful shutdown on SIGTERM

---

## Next Steps

- Read [Authoring Guide](authoring-guide.md) for advanced patterns
- Read [Testing Guide](testing-guide.md) for CI integration
- Explore [API Reference](api-reference.md) for full method signatures
- Join M31A Discord for extension discussions
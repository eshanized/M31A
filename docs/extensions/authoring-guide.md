# Extension Authoring Guide

**Audience:** Developers building M31A extensions
**Prerequisites:** Go 1.25+, basic JSON-RPC 2.0 understanding

## Overview

M31A extensions are standalone executables that communicate via JSON-RPC 2.0 over stdin/stdout. They can be written in any language (Go, Python, Rust, Node.js, Bash, etc.) — no CGO, no plugins, no SDK required.

## Extension Structure

```
my-extension/
├── main.go          # Entry point (or any executable)
├── go.mod           # Module definition (Go)
└── README.md        # Documentation
```

**Requirements:**
- Single executable file
- Reads JSON-RPC requests from stdin
- Writes JSON-RPC responses to stdout
- Logs to stderr (never stdout)
- Handles SIGTERM/SIGINT for graceful shutdown
- Responds to `handshake` method with protocol version and supported methods

## Building a Tool Extension

Tool extensions implement 5 JSON-RPC methods:

| Method | Purpose |
|--------|---------|
| `tool.name` | Return tool name |
| `tool.description` | Return description |
| `tool.risk_level` | Return "safe", "caution", or "dangerous" |
| `tool.schema` | Return JSON Schema for parameters |
| `tool.execute` | Execute the tool |

### Go Implementation Template

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

    // Handle shutdown signals
    sigCh := make(chan os.Signal, 1)
    signal.Notify(sigCh, syscall.SIGTERM, syscall.SIGINT)
    go func() {
        <-sigCh
        cancel()
    }()

    decoder := json.NewDecoder(os.Stdin)
    encoder := json.NewEncoder(os.Stdout)

    for {
        select {
        case <-ctx.Done():
            return
        default:
            var req Request
            if err := decoder.Decode(&req); err != nil {
                log.Printf("decode error: %v", err)
                continue
            }

            resp := handleRequest(ctx, req)
            if err := encoder.Encode(resp); err != nil {
                log.Printf("encode error: %v", err)
            }
        }
    }
}

func handleRequest(ctx context.Context, req Request) Response {
    switch req.Method {
    case "handshake":
        return Response{
            JSONRPC: "2.0",
            ID:      req.ID,
            Result: map[string]any{
                "protocol_version":  "1.0",
                "supported_methods": []string{"tool.name", "tool.description", "tool.risk_level", "tool.schema", "tool.execute"},
            },
        }
    case "tool.name":
        return Response{JSONRPC: "2.0", ID: req.ID, Result: "my-tool"}
    case "tool.description":
        return Response{JSONRPC: "2.0", ID: req.ID, Result: "Does something useful"}
    case "tool.risk_level":
        return Response{JSONRPC: "2.0", ID: req.ID, Result: "safe"}
    case "tool.schema":
        schema := map[string]any{
            "type": "object",
            "properties": map[string]any{
                "path": map[string]string{"type": "string"},
            },
        }
        data, _ := json.Marshal(schema)
        return Response{JSONRPC: "2.0", ID: req.ID, Result: json.RawMessage(data)}
    case "tool.execute":
        var params struct {
            Input map[string]any `json:"input"`
        }
        json.Unmarshal(req.Params, &params)
        
        // Your tool logic here
        output := "Tool executed with: " + params.Input["path"].(string)
        
        return Response{
            JSONRPC: "2.0",
            ID:      req.ID,
            Result: map[string]any{
                "output": output,
            },
        }
    default:
        return Response{
            JSONRPC: "2.0",
            ID:      req.ID,
            Error:   &Error{Code: -32601, Message: "Method not found"},
        }
    }
}
```

### Configuration

Add to `m31a.json`:

```json
{
  "extensions": {
    "tools": {
      "my-tool": {
        "command": "./my-tool",
        "args": [],
        "env": {"MY_VAR": "value"},
        "timeout": "30s"
      }
    }
  }
}
```

## Building a Provider Extension

Provider extensions implement 6 JSON-RPC methods:

| Method | Purpose |
|--------|---------|
| `provider.name` | Return provider name |
| `provider.fetch_models` | Return available models |
| `provider.chat_completion_stream` | Streaming chat completion |
| `provider.estimate_cost` | Estimate request cost |
| `provider.health_check` | Health check |
| `provider.get_model` | Get specific model |

### Streaming via Notifications

For streaming, use JSON-RPC **notifications** (no `id` field):

```go
// Initial request returns ack with stream_id
// Then extension sends notifications for each chunk:
{"jsonrpc":"2.0","method":"stream.chunk","params":{"stream_id":"abc","chunk":{"content":"Hello"}}}
// Final notification:
{"jsonrpc":"2.0","method":"stream.end","params":{"stream_id":"abc"}}
```

### Configuration

```json
{
  "extensions": {
    "providers": {
      "my-provider": {
        "command": "./my-provider",
        "args": ["--model", "my-model"],
        "env": {"API_HOST": "http://localhost:8080"},
        "timeout": "120s"
      }
    }
  }
}
```

## Building a Hook Extension

Hook extensions implement 2 JSON-RPC methods:

| Method | Purpose |
|--------|---------|
| `hook.pre_phase` | Called before phase starts |
| `hook.post_phase` | Called after phase ends |

### Payload

```json
{
  "phase_name": "execute",
  "workflow_state": {
    "current_phase": "execute",
    "goal": "Build a CLI tool",
    "tasks": [...],
    "session_id": "abc123",
    "budget_spent_usd": 0.05
  },
  "extension_config": {}
}
```

### Configuration

```json
{
  "extensions": {
    "hooks": {
      "pre-commit-style": {
        "command": "./pre-commit-hook",
        "args": ["--check"],
        "phases": ["execute", "verify"],
        "hook_types": ["pre"],
        "timeout": "30s"
      }
    }
  }
}
```

## JSON-RPC Patterns

### Request/Response

```json
// Request
{"jsonrpc":"2.0","id":1,"method":"tool.execute","params":{"input":{"path":"/tmp/test"}}}

// Response
{"jsonrpc":"2.0","id":1,"result":{"output":"Done"}}
```

### Notifications (for streaming)

```json
// Chunk notification
{"jsonrpc":"2.0","method":"stream.chunk","params":{"stream_id":"abc","chunk":{"content":"Hello"}}}

// End notification
{"jsonrpc":"2.0","method":"stream.end","params":{"stream_id":"abc"}}
```

### Error Response

```json
{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"Method not found"}}
```

## Testing Locally

### Manual Test

```bash
# Build extension
go build -o my-tool .

# Test handshake
echo '{"jsonrpc":"2.0","id":1,"method":"handshake"}' | ./my-tool

# Test tool execution
echo '{"jsonrpc":"2.0","id":2,"method":"tool.execute","params":{"input":{"path":"/tmp/test"}}}' | ./my-tool
```

### M31A Integration Test

```bash
# Add to m31a.json
# Run M31A with goal that uses your tool
m31a --goal "Test my tool"
```

## Distribution

### GitHub Releases

```bash
# Build for multiple platforms
GOOS=linux GOARCH=amd64 CGO_ENABLED=0 go build -o my-tool-linux-amd64 .
GOOS=darwin GOARCH=arm64 CGO_ENABLED=0 go build -o my-tool-darwin-arm64 .
GOOS=windows GOARCH=amd64 CGO_ENABLED=0 go build -o my-tool-windows-amd64.exe .

# Create release on GitHub with binaries
```

### Homebrew Formula

```ruby
class MyTool < Formula
  desc "My M31A extension"
  homepage "https://github.com/user/my-tool"
  url "https://github.com/user/my-tool/releases/download/v1.0.0/my-tool-darwin-arm64"
  sha256 "..."
  
  def install
    bin.install "my-tool-darwin-arm64" => "my-tool"
  end
end
```

### Scoop Manifest

```json
{
  "version": "1.0.0",
  "homepage": "https://github.com/user/my-tool",
  "license": "MIT",
  "architecture": {
    "64bit": {
      "url": "https://github.com/user/my-tool/releases/download/v1.0.0/my-tool-windows-amd64.exe",
      "hash": "sha256:..."
    }
  },
  "bin": "my-tool-windows-amd64.exe"
}
```

## Best Practices

1. **Timeouts:** Respect the timeout from config; use context cancellation
2. **Error Handling:** Return proper JSON-RPC errors with codes
3. **Logging:** Log to stderr only; stdout is for JSON-RPC
4. **Graceful Shutdown:** Handle SIGTERM/SIGINT; clean up resources
5. **No Stdout Pollution:** Never write to stdout except JSON-RPC
5. **Stdin/Stdout:** Don't close stdin/stdout; M31A manages pipes
6. **Working Directory:** Use `SetWorkDir` or respect the current directory
7. **Protocol Version:** Always return `protocol_version: "1.0"` in handshake
8. **Supported Methods:** List all methods you implement in handshake
# Ollama Provider Extension

A sample M31A provider extension that connects to a local Ollama instance.

## Build

```bash
cd docs/examples/ollama-provider
go build -o ollama-provider .
```

## Requirements

- Ollama running locally (or accessible via `OLLAMA_HOST`)
- Go 1.25+

## Configuration

Add to your `m31a.json`:

```json
{
  "extensions": {
    "providers": {
      "ollama": {
        "command": "./docs/examples/ollama-provider/ollama-provider",
        "args": ["--model", "llama3"],
        "env": {
          "OLLAMA_HOST": "http://localhost:11434"
        },
        "timeout": "120s"
      }
    }
  }
}
```

## Usage

In M31A, the Ollama provider will be available for model selection.

## Environment Variables

- `OLLAMA_HOST` - Ollama API endpoint (default: http://localhost:11434)
- `OLLAMA_MODEL` - Default model to use (default: llama3)

## Testing Manually

```bash
# Test handshake
echo '{"jsonrpc":"2.0","id":1,"method":"handshake"}' | ./ollama-provider

# Test fetch models
echo '{"jsonrpc":"2.0","id":2,"method":"provider.fetch_models"}' | ./ollama-provider

# Test health check
echo '{"jsonrpc":"2.0","id":3,"method":"provider.health_check"}' | ./ollama-provider
```

## JSON-RPC Methods Implemented

- `handshake` - Protocol negotiation
- `provider.name` - Returns "ollama"
- `provider.fetch_models` - Calls Ollama /api/tags
- `provider.chat_completion_stream` - Streams from /api/chat via JSON-RPC notifications
- `provider.estimate_cost` - Returns 0 (local)
- `provider.health_check` - GET /api/version
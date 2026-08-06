# Custom Linter Tool Extension

A sample M31A tool extension that runs `golangci-lint` on specified files/directories.

## Build

```bash
cd docs/examples/custom-linter
go build -o custom-linter .
```

## Configuration

Add to your `m31a.json`:

```json
{
  "extensions": {
    "tools": {
      "custom-linter": {
        "command": "./docs/examples/custom-linter/custom-linter",
        "args": [],
        "timeout": "60s"
      }
    }
  }
}
```

## Usage

In M31A, ask it to lint code:

> "Use the custom-linter tool to check the internal/engine package"

The extension will run `golangci-lint run internal/engine` and return the output.

## Requirements

- `golangci-lint` installed and in PATH
- Go 1.25+

## Testing Manually

```bash
# Test handshake
echo '{"jsonrpc":"2.0","id":1,"method":"handshake"}' | ./custom-linter

# Test tool execution
echo '{"jsonrpc":"2.0","id":2,"method":"tool.execute","params":{"input":{"path":"./internal/core"}}}' | ./custom-linter
```

## JSON-RPC Methods Implemented

- `handshake` - Protocol negotiation
- `tool.name` - Returns "custom-linter"
- `tool.description` - Returns description
- `tool.risk_level` - Returns "safe"
- `tool.schema` - Returns JSON Schema for parameters
- `tool.execute` - Runs golangci-lint on specified path
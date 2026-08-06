# Pre-Commit Hook Extension

A sample M31A workflow hook extension that runs code style checks before execute/verify phases.

## Build

```bash
cd docs/examples/pre-commit-hook
go build -o pre-commit-hook .
```

## Configuration

Add to your `m31a.json`:

```json
{
  "extensions": {
    "hooks": {
      "pre-commit-style": {
        "command": "./docs/examples/pre-commit-hook/pre-commit-hook",
        "args": ["--check"],
        "phases": ["execute", "verify"],
        "hook_types": ["pre"],
        "timeout": "30s"
      }
    }
  }
}
```

## Behavior

When M31A enters the `execute` or `verify` phase, this hook runs `gofmt -l` on all staged Go files. If any files need formatting, the hook returns an error and the phase execution is blocked.

## Requirements

- Git repository
- Go toolchain (for `gofmt`)
- Go 1.25+

## Testing Manually

```bash
# Test handshake
echo '{"jsonrpc":"2.0","id":1,"method":"handshake"}' | ./pre-commit-hook

# Test pre-phase
echo '{"jsonrpc":"2.0","id":2,"method":"hook.pre_phase","params":{"payload":{"phase_name":"execute","workflow_state":{"current_phase":"execute","goal":"Test"}}}}' | ./pre-commit-hook
```

## JSON-RPC Methods Implemented

- `handshake` - Protocol negotiation
- `hook.pre_phase` - Runs gofmt on staged Go files
- `hook.post_phase` - No-op (optional)
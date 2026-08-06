# Extension FAQ

## Architecture

### Why subprocess instead of WASM/Go plugins?

| Approach | Verdict | Reason |
|----------|---------|--------|
| Go plugins (`-buildmode=plugin`) | ❌ | Linux-only, requires CGO, same Go version |
| WASM (wazero/wasmtime) | ❌ | CGO or large runtime, conflicts with `CGO_ENABLED=0` |
| **Subprocess + JSON-RPC** | ✅ | Cross-platform, language-agnostic, no CGO, reversible |

Subprocess model leverages OS process isolation — extensions can be written in any language. JSON-RPC is simple enough to implement in 50 lines of any language.

### Can extensions access M31A internals?

**No.** Extensions only receive:
- `ToolInput` / `ToolResult` for tools
- `ChatRequest` / `StreamChunk` for providers
- `PhaseHookPayload` / `PhaseResult` for hooks

No direct access to M31A's internal state, types, or APIs.

---

## Protocol

### How to handle streaming?

Use **JSON-RPC notifications** (no `id` field):

```json
// 1. M31A sends request
{"jsonrpc":"2.0","id":1,"method":"provider.chat_completion_stream","params":{"request":{...}}}

// 2. Extension responds with ack
{"jsonrpc":"2.0","id":1,"result":{"stream_id":"abc123"}}

// 3. Extension sends chunk notifications
{"jsonrpc":"2.0","method":"stream.chunk","params":{"stream_id":"abc123","chunk":{"content":"Hello"}}}
{"jsonrpc":"2.0","method":"stream.chunk","params":{"stream_id":"abc123","chunk":{"content":" World"}}}

// 4. Extension sends end notification
{"jsonrpc":"2.0","method":"stream.end","params":{"stream_id":"abc123"}}
```

### Protocol versioning?

Handshake returns `protocol_version`:

```json
{"jsonrpc":"2.0","id":1,"result":{"protocol_version":"1.0","supported_methods":[...]}}
```

- **Major** = breaking changes (method signatures, protocol)
- **Minor** = new methods, backward compatible
- M31A rejects extensions with incompatible `protocol_version`

---

## Performance

### Performance overhead?

~5-10ms per call (subprocess spawn + JSON-RPC serialization). For long-running tools, overhead is negligible.

### Can extensions call other extensions?

**No direct access.** Use M31A as mediator:
1. Tool A calls M31A API (if exposed)
2. Or: Extension registers a tool that other extensions can call via M31A's Dispatcher

### How to debug?

Run extension manually with JSON-RPC on stdin/stdout:

```bash
# Test handshake
echo '{"jsonrpc":"2.0","id":1,"method":"handshake"}' | ./my-extension

# Test tool execute
echo '{"jsonrpc":"2.0","id":2,"method":"tool.execute","params":{"input":{"message":"test"}}}' | ./my-extension

# Pretty output with jq
echo '{"jsonrpc":"2.0","id":2,"method":"tool.execute","params":{"input":{"path":"/tmp/test"}}}' | ./my-extension | jq
```

---

## Configuration

### Config precedence?

1. Project (`m31a.json`) — highest
2. Workspace (`.m31a/workspace.toml`)
3. Global (`~/.m31a/config.toml`)
4. Env vars (`M31A_*`) — highest priority override

### Variable substitution?

Yes, `${VAR}` patterns resolved from environment:

```json
{"command": "/path/to/tool", "env": {"API_KEY": "${MY_API_KEY}"}}
```

---

## Distribution

### How do users install extensions?

**Decentralized:**
- GitHub Releases (binaries)
- Homebrew (`brew install user/tap/extension`)
- Scoop (`scoop install extension`)
- Direct binary download

No central registry — user points config to local executable path.

### How to publish?

1. Build binaries for all platforms (`GOOS=linux GOARCH=amd64 CGO_ENABLED=0 go build`)
2. Create GitHub Release with binaries
3. Optionally: Homebrew formula, Scoop manifest
4. Document config snippet in README

---

## Development

### Can I use external dependencies?

Yes, but:
- **Go:** Use `CGO_ENABLED=0` compatible libraries
- **Other languages:** Bundle dependencies or document requirements
- **Prefer stdlib** for simplicity and auditability

### How to handle graceful shutdown?

```go
sigCh := make(chan os.Signal, 1)
signal.Notify(sigCh, syscall.SIGTERM, syscall.SIGINT)
go func() { <-sigCh; cancel() }()

// In main loop:
select {
case <-ctx.Done():
    return  // Clean exit
default:
    // Process requests
}
```

### Can extensions run background tasks?

Yes, but:
- Spawn goroutines/threads within extension process
- M31A only manages stdin/stdout pipes
- Extension manages its own lifecycle

### How to test locally?

```bash
# 1. Build extension
go build -o my-tool .

# 2. Test handshake
echo '{"jsonrpc":"2.0","id":1,"method":"handshake"}' | ./my-tool

# 3. Test execute
echo '{"jsonrpc":"2.0","id":2,"method":"tool.execute","params":{"input":{"message":"test"}}}' | ./my-tool

# 4. Add to m31a.json and run M31A
```

### Common issues?

| Issue | Cause | Fix |
|-------|-------|-----|
| Handshake timeout | Extension not reading stdin | Ensure reading loop |
| Method not found | Missing in handshake | Add to `supported_methods` |
| Execute hangs | Not writing stdout | Ensure response to stdout |
| Zombie processes | Stop() not called | Call `registry.Stop()` in defer |
| JSON parse error | Stdout pollution | Log to stderr only |

---

## Security

### Can extensions run arbitrary code?

Yes — extensions run as the user with full filesystem/network access. **Trust model:** User controls which extensions to install via config.

### How to verify extension authenticity?

- Build from source (`go build`)
- Verify checksums from GitHub Releases
- Use Homebrew/Scoop (they verify signatures)

### Environment variable leakage?

M31A scrubs env vars before spawn (like `bash_sandbox_linux.go`). Only explicitly configured `env` passed to extension.

---

## Troubleshooting

### Extension not loading?

1. Check config syntax (`m31a config show --extensions`)
2. Verify command path exists and is executable
3. Check M31A logs for handshake errors
4. Test extension manually: `echo '{"jsonrpc":"2.0","id":1,"method":"handshake"}' | /path/to/extension`

### Extension crashes M31A?

Extensions run in separate process — cannot crash M31A. If extension hangs, M31A enforces timeout and kills subprocess.

### Performance too slow?

- Reduce JSON-RPC payload size
- Cache metadata (name, schema) on first call
- Use streaming for large outputs
- Profile with `pprof` if needed
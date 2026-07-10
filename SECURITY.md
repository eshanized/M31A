# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 1.6.x   | :white_check_mark: |
| 1.5.x   | :white_check_mark: |
| < 1.5   | :x:                |

## Reporting a Vulnerability

If you discover a security vulnerability in M31 Autonomous, please report it responsibly.

**Do NOT open a public GitHub issue for security vulnerabilities.**

Instead, please email: **eshanized@proton.me**

Include:
- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if any)

### What to expect

- **Acknowledgment** within 48 hours
- **Assessment** within 7 days
- **Fix or mitigation** for confirmed vulnerabilities will be prioritized
- Credit will be given to reporters (unless anonymity is preferred)

## Security Considerations

M31 Autonomous is an AI coding agent that executes code and shell commands. By design, it requires trust in the LLM it uses. Key security features:

| Layer | Mechanism |
|-------|-----------|
| **Permission gating** | Every `Bash` command gated: allow / allow always / deny, with configurable timeout and default `ask` mode |
| **Rate limiting** | Token bucket: 20 burst / 10 sustained for normal tools; 5 burst / 2 sustained for dangerous tools |
| **Concurrency control** | Max 8 concurrent tool executions via semaphore |
| **Command blocklist** | Dangerous command patterns blocked at the tool boundary (defense-in-depth) |
| **Process sandboxing** | Environment scrubbing (API keys removed); Landlock filesystem restriction (Linux 5.13+); sandbox-exec (macOS) |
| **Path traversal** | Blocked at tool boundary; size limits (50MB per file read), stream-size caps |
| **SSRF protection** | WebFetch blocks private, loopback, and link-local IP addresses |
| **DNS rebinding** | WebSearch uses a DNS cache (5 min TTL) to prevent TOCTOU rebinding attacks |
| **Edit safety** | 7-strategy cascade with collision-safe backups |
| **Subagent depth** | Max nesting depth of 2 to prevent runaway agent spawning |
| **Zero telemetry** | No data is sent anywhere except your configured LLM provider |

### Bash Tool Sandbox Guarantees

The Bash tool applies multiple layers of process-level sandboxing:

1. **Environment scrubbing** (all platforms): Sensitive environment variables (API keys, tokens, secrets) are removed from the subprocess environment before execution.

2. **Landlock filesystem restriction** (Linux 5.13+): When the kernel supports Landlock, the subprocess is restricted to accessing only the work directory, `/tmp`, `/dev`, `/usr`, `/bin`, and `/lib`. This prevents reading/writing files outside the allowed scope.

3. **sandbox-exec profile** (macOS): A restrictive sandbox profile limits filesystem access and process execution to allowed paths.

4. **Graceful degradation**: On platforms where OS-level sandboxing is not available, only environment scrubbing is applied. A warning is logged.

**Limitations:**
- Landlock requires Linux 5.13+ with the feature enabled in the kernel. On older kernels, only env-scrub-only mode is available.
- sandbox-exec is a best-effort defense-in-depth layer, not a security boundary.
- The blocklist is bypassable with creative syntax; sandboxing provides process-level restrictions that complement the blocklist.
- These restrictions do not prevent network access (required for API calls) or process execution within allowed paths.

## Best Practices

1. **Use the default permission mode** (`prompt`) rather than `allow` for dangerous tools
2. **Keep your LLM API keys secure** — use a keychain when available
3. **Review tool outputs** — the LLM can see file contents and command outputs
4. **Run in isolated environments** — use containers or VMs for untrusted code

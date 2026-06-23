# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 1.3.x   | :white_check_mark: |
| 1.2.x   | :white_check_mark: |
| < 1.2   | :x:                |

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

- **Permission gating** — Dangerous operations require explicit user approval
- **SSRF protection** — DNS pinning and private IP blocking for web requests
- **Path traversal guards** — Symlink resolution and workspace containment
- **Rate limiting** — Token bucket rate limiting on tool execution
- **Zero telemetry** — No data is sent anywhere except your configured LLM provider

For the full security audit, see [SECURITY_AUDIT.md](SECURITY_AUDIT.md).

## Best Practices

1. **Use the default permission mode** (`prompt`) rather than `allow` for dangerous tools
2. **Keep your LLM API keys secure** — use a keychain when available
3. **Review tool outputs** — the LLM can see file contents and command outputs
4. **Run in isolated environments** — use containers or VMs for untrusted code

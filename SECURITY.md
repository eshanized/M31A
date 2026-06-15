# Security Policy

## Supported Versions

M31 Autonomous follows semantic versioning. Security fixes are applied to the latest minor release and backported to the previous minor release for 90 days.

| Version | Supported |
| ------- | --------- |
| `1.0.x` | Yes |
| `< 1.0` | No |

## Reporting a Vulnerability

**Do not open a public GitHub issue for security vulnerabilities.**

Please email `security@eshanized.com` (or, if that bounces, DM `@eshanized` on the platform listed in the repository's GitHub profile). Include:

- A description of the vulnerability
- Steps to reproduce or a proof-of-concept
- The affected version(s) and your environment (OS, Go version)
- Any suggested remediation, if you have one

### What to expect

- **Acknowledgement** within 72 hours
- **Triage + severity assessment** within 7 days
- **Fix + coordinated disclosure** within 30 days for critical/high issues, 90 days for low/medium

If the issue is accepted, you'll be credited in the release notes (unless you request otherwise). If it's declined, we'll explain why and you're free to disclose publicly after the 90-day window.

## Security model

M31 Autonomous is a terminal agent with shell access, so the security posture is intentionally defensive:

- **Permission gating** — the `Bash` tool runs in `ask` mode by default; every invocation requires explicit user approval (`y`/`a`/`n`/`e`). The mode and timeout are configurable in `~/.m31a/config.toml`.
- **No plaintext secrets** — API keys resolve in order: environment variable → OS keychain → config file. They are never written to disk outside the keychain, never echoed to the TUI, and never included in ledger entries.
- **Path traversal validation** — the Python compile step in the verify phase, and all `FileRead`/`FileWrite` calls, validate paths against the session working directory.
- **Size limits** — all session file reads are capped at 50MB (`readFileLimited`), and the LLM response stream enforces `MaxLLMResponseBytes` to prevent memory exhaustion.
- **Static binary** — `CGO_ENABLED=0` on every release build. No dynamic linking, no hidden native dependencies.
- **No telemetry** — M31 Autonomous does not phone home. No analytics, no crash reporting, no usage pings.
- **Dependency hygiene** — `govulncheck` runs on every CI build. `go.sum` is committed and verified.

## Known historical issues

See [`CHANGELOG.md`](CHANGELOG.md) entries tagged `WP-H*` for hardening fixes applied before v1.0.0.

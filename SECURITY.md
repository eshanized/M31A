# M31A Security Policy

The security and containment of autonomous agent operations is the primary foundation of the M31A runtime.

---

## Reporting a Vulnerability

If you discover a security vulnerability, sandbox escape, prompt injection boundary compromise, or policy bypass in M31A, **please do not open a public issue.**

Instead, report it confidentially via GitHub Private Vulnerability Reporting:
- Navigate to the **Security** tab of this repository.
- Click **"Report a vulnerability"**.
- Provide details, reproduction steps, and impact assessment.

Alternatively, contact the security maintainers directly at:
- **Email**: `security@m31a.dev` (or author: `m.eshanized@gmail.com`)

### Vulnerability Response SLA

- **Initial Response**: Within 24 hours acknowledging receipt.
- **Triage & Reproduction**: Within 72 hours.
- **Fix & Disclosure**: Coordinated security patch released within 14 days, with public CVE advisory published following release deployment.

---

## Supported Versions

Only the latest release receives active security fixes.

| Version | Supported |
|:---|:---|
| `v0.1.x` | :white_check_mark: Yes |
| `< v0.1.0` | :x: No (Legacy alpha / deprecated) |

---

## Security Invariants & Threat Mitigation

M31A implements the canonical 11-threat hardening matrix, ASVS L1 compliance, and root-scoped workspace confinement:

1. **Filesystem Confinement**: Strict lexical and canonical path sanitization preventing `../` path traversal, symlink escapes, and null-byte injection (`SEC-02`).
2. **Protected Repositories**: Non-bypassable protection for `.git` and `.m31a` directories preventing tampering by agent tools (`SEC-02`).
3. **Command Execution Safety**: Direct argument vector execution (`argv`) bypassing shell interpreters, preventing argument injection and command concatenation attacks (`SEC-03`).
4. **Secret Scrubbing**: Zero-leak redaction of API keys, tokens, and authorization credentials before logging, persistent storage, or telemetry export (`SEC-04`).
5. **Untrusted Content Sanitization**: Repository files, commit logs, and model inputs are wrapped in structured XML `<trust_envelope>` blocks to prevent indirect prompt injection (`SEC-05`).
6. **Policy Engine Primacy**: An 11-stage policy gate with immutable veto power over every mutating tool call (`SEC-06`).
7. **Process Tree Containment**: Multi-stage signal escalation (`SIGTERM` -> `SIGKILL` or Windows Job Object termination) preventing zombie child leaks (`SEC-03`).
8. **Crash-Resilient State**: Two-phase atomic checkpoints and SQLite foreign-key write ahead logs preventing state tampering (`SEC-01`).

For the complete technical threat modeling specification, see [docs/subsystems/SECURITY.md](docs/subsystems/SECURITY.md).

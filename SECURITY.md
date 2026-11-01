# Security Policy

M31A is an autonomous agentic coding assistant designed with defence-in-depth safety boundaries, isolating autonomous code execution and workspace mutation. We take the security of M31A and the developers using it seriously.

---

## Supported Versions

We provide security updates and patches for the following versions:

| Version | Supported          | Security Patches |
| ------- | ------------------ | ---------------- |
| 0.1.x   | :white_check_mark: | Active           |
| < 0.1.0 | :x:                | End of life      |

---

## Security Architecture & Threat Model

M31A incorporates several architectural layers designed to safeguard developer machines:

1. **Linux Bubblewrap Sandbox (`m31a-sandbox`)**:
   - Isolates execution using unprivileged Linux namespaces (`PID`, `IPC`, `UTS`, `Mount`).
   - Root filesystem is mounted read-only (`ro-bind`).
   - Workspace paths are mounted writable only when `workspace-write` policy is active.
   - Sensitive subpaths are strictly protected by read-only masks (`.git`, `.m31a`, `.codex`, `.ssh`, `.env`, `*.key`, `*.pem`).
   - Optional network namespace unsharing (`--unshare-net`) for air-gapped environments.

2. **Prefix & AST ExecPolicy Engine (`m31a-execpolicy`)**:
   - Static analysis and prefix pattern token matching over CLI commands prior to dispatch.
   - Network destination guards limiting outbound socket calls by host, protocol, and port.
   - Tri-state decisions: `allow`, `prompt`, or `deny` with explicit human-readable rationales.

3. **Guardian Autonomous Review Supervisor (`m31a-guardian`)**:
   - Out-of-band evaluation of mutating operations and destructive commands.
   - Sliding-window circuit breaker (`AUTO_REVIEW_DENIAL_WINDOW_SIZE = 5`, `MAX_DENIALS_BEFORE_TRIP = 3`) trips session into immediate freeze if anomalous destructive sequences occur.

4. **Hardened Local Proxy & Credential Isolation (`m31a-cli responses-proxy`)**:
   - Memory-locked API credential storage preventing token exfiltration via memory dumps or shell process inspection.

---

## Reporting a Vulnerability

**Please do not report security vulnerabilities through public GitHub issues.**

If you discover a security vulnerability, sandbox escape, or unexpected privilege escalation in M31A, report it responsibly:

### Disclosure Channels

1. **GitHub Private Vulnerability Reporting (Preferred)**:
   - Navigate to the **Security** tab of the M31A GitHub repository.
   - Click **Report a vulnerability** to submit a private draft.
2. **Security Email**:
   - Send an encrypted or plain text email to: **security@tonmoyinfrastructure.org**
   - If desired, request our PGP public key before transmitting sensitive proof-of-concept material.

### Information to Include

To help us triage and reproduce the issue rapidly, please include:
- A detailed description of the vulnerability.
- Affected component or crate (e.g. `m31a-sandbox`, `m31a-execpolicy`, `m31a-app-server`).
- Steps to reproduce or proof-of-concept (PoC) script.
- Operating system, kernel version, and container/WSL details.
- Potential impact or attack scenario.

---

## Response Timeline

- **Initial Acknowledgment**: Within 48 hours of report submission.
- **Triage & Assessment**: Within 5 business days, confirming vulnerability scope and severity score (CVSS).
- **Patch Development & Testing**: Priority resolution with private backports.
- **Public Disclosure**: Coordinated release with CVE assignment where applicable, typically 30 days after patch verification or by mutual agreement.

---

## Hall of Fame & Responsible Disclosure

We appreciate the security research community. Researchers who report valid security vulnerabilities in accordance with this policy will be credited in our release notes and Security Hall of Fame (unless anonymity is requested).

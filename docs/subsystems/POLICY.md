# M31A Policy Engine & Security Governance

This document details the multi-layer policy engine, 11-stage evaluation pipeline, decision outcomes, layer precedence hierarchy, and default developer rules in the M31 Autonomous (M31A) runtime.

## Core Policy Principle

> **"Higher authority always wins. Lower layers can only restrict, never expand, permissions."**

Security decisions are computed deterministically across a 10-layer authority stack. Invariant built-in safety rules sit at Layer 0 and can never be overridden, shadowed, or bypassed by user configuration, prompt text, or session approval grants.

---

## The 4 Policy Outcomes

When an action or tool call is evaluated by `PolicyGate`, it produces one of four mutually exclusive decisions:

| Decision | Meaning | Action Taken |
|:---|:---|:---|
| **`ALLOW`** | Permitted unconditionally. | Tool execution proceeds immediately within sandbox limits. |
| **`DENY`** | Forbidden fail-closed. | Tool execution is aborted immediately with zero side effects. |
| **`ASK`** | Requires human operator authorization. | Pauses mission in interactive modes; **converts to DENY fail-closed in unattended mode**. |
| **`ESCALATE`** | Requires elevated administrative consent. | Routes approval request to out-of-band security coordinator or halts mission. |

---

## 10-Tier Layer Precedence Hierarchy

Layers are sorted in descending order of authority (Layer 0 has highest authority; Layer 9 is fallback):

```
Layer 0: BuiltInSafety    (Hardcoded immutable safety vetoes)
  ↓
Layer 1: SystemAdmin      (/etc/m31/policy.toml)
  ↓
Layer 2: Organization     (Enterprise compliance rules)
  ↓
Layer 3: Workspace        (<workspace>/.m31/policy.toml)
  ↓
Layer 4: User             (~/.config/m31/policy.toml)
  ↓
Layer 5: Mission          (Mission-specific policy constraints)
  ↓
Layer 6: AgentRole        (Role-based boundary limits)
  ↓
Layer 7: Task             (Task-specific scope restrictions)
  ↓
Layer 8: SessionApproval  (Durable SQLite policy_grants from user prompts)
  ↓
Layer 9: DeveloperDefault (Sensible baseline engineering fallbacks)
```

### Monotonic Non-Weakening Merger Rule

When rules from multiple layers match a proposed action:
- A `DENY` decision at **any higher layer** completely overrides an `ALLOW` or `ASK` at all lower layers.
- An `ASK` at a higher layer cannot be weakened to `ALLOW` by a lower layer.
- Lower layers can only narrow scopes (e.g. restricting paths to `src/` or setting smaller byte caps).

---

## The 11-Stage Policy Evaluation Pipeline

Every evaluation request proceeds through an 11-stage verification pipeline:

1. **Request Intake & Context Digest**: Normalizes tool ID, target paths, agent role, and mission context.
2. **Path Canonicalization**: Resolves target paths against workspace root, resolving symlinks and eliminating redundant traversal (`..`).
3. **Boundary Verification**: Verifies target paths do not escape the workspace root.
4. **Layer 0 Veto Check**: Evaluates immutable safety vetoes (credential paths, SSH keys, shadow files).
5. **System & Admin Evaluation**: Applies system administrator policies (`/etc/m31/policy.toml`).
6. **Workspace Rule Matching**: Evaluates project-specific workspace policies (`.m31/policy.toml`).
7. **Role & Capability Filtering**: Verifies the calling agent role possesses the required capability.
8. **Grant Lookup**: Checks existing durable approval grants in SQLite (`policy_grants` table).
9. **Developer Defaults Fallback**: Applies baseline developer rules if no higher layer matched.
10. **Autonomy Mode Translation**: Translates `ASK` decisions based on autonomy mode (unattended -> `DENY`).
11. **Durable Audit Logging**: Records the evaluation record, active policy hash, and matched rule ID in SQLite `policy_decisions`.

---

## Built-In Safety Rules (Layer 0 Vetoes)

The following paths and operations are hardcoded at Layer 0 and **cannot be permitted**:

- **Credentials & Secrets**:
  - `**/.ssh/**`, `**/.aws/**`, `**/.gnupg/**`
  - `**/.env*`, `**/*id_rsa*`, `**/*id_ed25519*`, `**/*token*`, `**/*secret*`
- **System Policy & OS Tampering**:
  - `/etc/m31/**`, `/etc/sudoers*`, `/etc/passwd`, `/etc/shadow`
- **Shell Startup Profiles**:
  - `**/.bashrc`, `**/.zshrc`, `**/.profile`, `/etc/profile`, `**/.bash_profile`
- **Destructive Commands**:
  - `rm -rf /`, `mkfs*`, `dd if=*`, `shutdown*`, `reboot*`

---

## Developer Baseline Defaults (Layer 9)

In the absence of custom rules, M31A applies developer-friendly defaults:
- **Filesystem**: Read and mutate files freely inside workspace root (`ALLOW`).
- **Local Git**: Status, diff, log, branch, add, and commit inside local repo (`ALLOW`).
- **QA & Testing**: Run test runners, linters, and formatters (`ALLOW`).
- **Network Access**: Outbound network requests require explicit approval (`ASK`).
- **Git Push**: Pushing commits to remote repositories requires explicit approval (`ASK`).
- **Destructive Host Mutations**: Unsandboxed host alterations are rejected (`DENY`).

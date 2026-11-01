# `m31a-trust`

Workspace trust boundary evaluation and Git hook safety armor for **M31A**.

---

## Overview

`m31a-trust` protects developers against untrusted repositories. When opening a new codebase, it checks whether the project is trusted before allowing automated tool executions or executing repository-controlled Git hooks (`.git/hooks/*`).

---

## User-Facing Capabilities

- **Workspace Trust Boundary (`m31a trust`)**: Inspect or grant trust for local project paths.
- **Git Hook Armor**: Detects and neutralizes malicious or unexpected Git hooks that could execute arbitrary code during `git checkout` or `git commit`.

---

## Developer Guide & Architecture

### Key Modules

- [`trust_store`](src/trust_store.rs): `TrustStore` maintaining persistent trust decisions (`TrustLevel::Trusted`, `TrustLevel::Untrusted`) in `~/.m31a/trusted_dirs.json`.
- [`git_armor`](src/git_armor.rs): `GitArmor` auditing `.git/hooks` for executable scripts and reporting `GitRisk` levels.
- [`capability`](src/capability.rs): `OperationCapability` gating high-risk operations based on trust authorization outcomes.

---

## Testing

```bash
cargo test -p m31a-trust
```

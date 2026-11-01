# `m31a-features`

Structured feature flag registry, staged rollouts, and runtime capability toggles for **M31A**.

---

## Overview

`m31a-features` provides runtime feature flag management across M31A. It enables safe progressive rollouts of experimental subsystems (such as `code_mode`, `guardian_v2`, `visual_tdd`, and `worktree_management`), dependency validation between flags, and audit trail logging.

---

## User-Facing Capabilities

- **Inspect & Toggle Flags (`m31a features`)**:
  ```bash
  m31a features list
  m31a features enable code_mode
  m31a features disable guardian_approval
  ```
- **CLI Flag Overrides**: Temporarily enable or disable features using `--enable <flag>` and `--disable <flag>`.

---

## Developer Guide & Architecture

### Key Types

- `FeatureFlag`: Defines flag ID, human-readable description, category, and rollout state.
- `FeatureKey`: Strongly typed enum preventing invalid feature key strings across the codebase.
- `FeatureRegistry`: Thread-safe registry loaded from `config.toml` with programmatic override APIs.

---

## Testing

```bash
cargo test -p m31a-features
```

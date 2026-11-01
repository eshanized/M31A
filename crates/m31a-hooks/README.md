# `m31a-hooks`

Deterministic lifecycle hook dispatcher executing shell and tool commands across 14 lifecycle events in **M31A**.

---

## Overview

`m31a-hooks` provides lifecycle triggers modeled after Claude-style `hooks.json` semantics. It dispatches configured actions before and after tool calls, turn completions, phase transitions, and session starts/stops, with exit code gating (`exit 2` blocks execution with explanatory feedback).

---

## User-Facing Capabilities

- **Lifecycle Automation (`hooks.json`)**: Configure project-specific checks, such as running a linter automatically before applying patches, or running tests after every tool completion.
- **Pre-Tool Gating**: `PreToolUse` hooks can rewrite arguments or block risky actions.
- **Post-Tool Acceptance**: `PostToolUse` hooks can reject tool outputs if invariants are broken.

---

## Developer Guide & Architecture

### Key Modules

- [`events`](src/events.rs): Defines 14 `HookEvent` variants (`PreToolUse`, `PostToolUse`, `SessionStart`, `SessionStop`, `PhaseTransition`) and `HookDisposition`.
- [`runtime`](src/runtime.rs): `HookRuntime` executing hooks asynchronously with timeout enforcement.
- [`config`](src/config.rs): `HookConfig` and `HookRule` parsing configuration from `.m31a/hooks.json`.

---

## Testing

```bash
cargo test -p m31a-hooks
```

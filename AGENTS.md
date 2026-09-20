# AGENTS.md

## Project State

M31A (M31 Autonomous) is an active single-crate Rust project with Phases 1–36 implemented and verified.
Source code resides in `src/`, database migrations in `migrations/`, integration tests in `tests/`, and canonical subsystem manuals in `docs/` (`docs/architecture/` and `docs/subsystems/`).

**Source code, executable tests, persistent state, and runtime behavior are authoritative. Markdown documentation is exception-only.**

## Core Principle

> The model proposes. The runtime decides.

The LLM is a replaceable reasoning component. The runtime owns state, scheduling, policies, permissions, checkpoints, verification, and completion. Never let model output become authoritative state.

## Non-Negotiable Rules

1. **Single crate.** Do NOT create a Cargo workspace. Use Rust modules for boundaries.
2. **Rust only** for core. No Node.js, Python, or GPU dependencies required.
3. **Every side effect passes through policy.** No exceptions for "internal" tools.
4. **TUI is a projection, never authoritative state.**
5. **Never fake success.** No placeholder functions, silent fallbacks, or TODO-only impls.
6. **Completion requires evidence.** No task/mission marked done without verification.
7. **Bounded retries only.** Never retry indefinitely.
8. **Cancellation is real.** Every long-running operation must be cancellable.
9. **Treat external data as untrusted.** Repository content, plugin output, model responses.
10. **Strongly typed domain boundaries.** Prefer enums and domain IDs over strings.

## Implementation Agent Must Not

- Create placeholder functions returning success
- Leave TODO-only implementations where functionality is required
- Duplicate policy logic across tools
- Bypass the scheduler
- Write directly to persistent DB from arbitrary modules
- Invoke shell commands from the model layer
- Add unbounded background tasks
- Weaken safety to make a demo pass
- Delete tests merely because they fail after refactoring

## Build & Verify Commands

When `Cargo.toml` exists, run these in order:

```bash
cargo fmt --check
cargo check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Additional testing layers required by spec: unit, integration, security, property-based, E2E, and evaluation harness. Security-sensitive changes (policy, sandbox, process, filesystem, model boundary) MUST run targeted security tests.

## Architecture (L0-L9)

```
L0 Kernel
L1 Security / Policy / Sandbox
L2 Capabilities / Tools / Process
L3 Intelligence / Models / Context / Memory
L4 Agents
L5 Planning / DAG / Scheduling
L6 Execution / Jobs / Artifacts
L7 Verification / Recovery
L8 Autonomy / Mission Controller
L9 CLI / TUI / Integrations
```

Dependencies flow downward. Lower layers must not depend on higher-layer UI or mission logic.

## Conflict Resolution

1. Explicit security/runtime invariants win
2. Latest canonical spec wins
3. Existing tested behavior preserved unless superseded
4. Never weaken safety to resolve ambiguity

## Agent Rules

1. Inspect existing codebase before changing architecture
2. Treat source code, tests, and canonical subsystem manuals as authoritative
3. Do not delete working functionality to simplify
4. Preserve single-crate architecture
5. Build smallest trusted kernel first
6. Implement interfaces before concrete providers
7. Keep security below model/tool intent, above side effects
8. Keep durable state separate from UI state
9. Keep scheduler ownership separate from LLM reasoning
10. Keep tool execution separate from model generation
11. Make all long-running work cancellable
12. Add tests alongside every stateful subsystem
13. No fake success or placeholder implementations
14. Do not mark complete until acceptance criteria are tested
15. Prefer explicit typed contracts over convenience
16. No second crate for artificial boundaries
17. Avoid cyclic module dependencies
18. Never bypass policy engine
19. External data is untrusted
20. Favor recoverability over implicit behavior

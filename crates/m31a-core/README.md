# `m31a-core`

Foundational abstractions, configuration models, cryptographic vaults, and runtime primitives for **M31A**.

---

## Overview

`m31a-core` is the base dependency crate in the M31A monorepo. It establishes the domain models, shared types, configuration parsing (`config.toml`), cryptographic secret stores, skills frontmatter parsers, session representations, and system prompt builders used throughout all other crates.

---

## User-Facing Capabilities

- **Configuration Loading**: Reads and merges user settings from `~/.m31a/config.toml`, `.m31a/config.toml`, environment variables, and CLI overrides.
- **Skills Discovery**: Discovers domain skills from `~/.m31a/skills/` and `.m31a/skills/` with YAML frontmatter.
- **Cryptographic Vaults**: Stores sensitive API credentials encrypted at rest using ChaCha20-Poly1305.

---

## Developer Guide & Architecture

### Key Modules

- [`config`](src/config.rs): `Config`, `SandboxPolicy`, `ApprovalPolicy`, and configuration resolution logic.
- [`types`](src/types.rs): Fundamental conversation data structures (`ChatMessage`, `Role`, `ToolCall`, `TokenUsage`).
- [`prompts`](src/prompts.rs): `SystemPromptBuilder` composing model instructions, tool manuals, and collaboration roles.
- [`skills`](src/skills.rs): Parses YAML frontmatter (`name`, `description`) and renders instruction blocks.
- [`agent_roles`](src/agent_roles.rs): Role definitions (`Architect`, `Coder`, `Tester`, `Guardian`).
- [`worktree`](src/worktree.rs): `WorktreeManager` managing ephemeral Git worktree isolation for parallel agent sessions.

---

## Testing

```bash
cargo test -p m31a-core
```

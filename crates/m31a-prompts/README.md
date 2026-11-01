# `m31a-prompts`

Prompt library, model preambles, fragment registry, and variable substitution engine for **M31A**.

---

## Overview

`m31a-prompts` manages system prompts, model-specific preambles (tailoring prompt structures for DeepSeek R1, Nemotron, Codestral, or Llama models), and reusable prompt templates with variable interpolation.

---

## User-Facing Capabilities

- **Custom Prompt Templates (`m31a prompt`)**: Store and invoke reusable user prompt templates with parameter variables (`{{variable}}`).
- **Model-Optimized Preamble**: Automatically injects instructions calibrated specifically for the active model's reasoning style and function-calling tokens.

---

## Developer Guide & Architecture

### Key Modules

- [`fragment_registry`](src/fragment_registry.rs): Resolves prompt fragments (system instructions, role preambles, Codex patch formatting rules) dynamically.
- `PromptTemplate`: Manages template versioning, categories, tags, and interpolation maps.

---

## Testing

```bash
cargo test -p m31a-prompts
```

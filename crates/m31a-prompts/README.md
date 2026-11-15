# `m31a-prompts`

> System prompt template library, versioned engineering prompt catalogs, and model fragment resolution for M31A.

---

## Architecture Overview & Canonical Pipeline

M31A maintains a **single authoritative prompt compiler** located in [`m31a-core::prompts`](../m31a-core/src/prompts.rs).

- **Canonical Prompt Compiler (`m31a-core::prompts`)**: Assembles immutable `PromptRuntimeSnapshot` records into deterministic, diffable `WorldState` sections. Used across CLI, TUI, App Server, and child subagent engines.
- **Prompt Library (`m31a-prompts::PromptLibrary`)**: Manages versioned prompt templates (`PromptTemplate`), category filtering, variable substitution, and user prompt catalogs (`~/.config/m31a/prompts`).
- **Fragment Registry (`m31a-prompts::fragment_registry`)**: Provides fragment-level resolution for diagnostic inspection and model-tailored instruction blocks, sharing the authoritative `ModelFamily` definition with `m31a-core`.

For complete system prompt architectural documentation, refer to [SYSTEM_PROMPT_ARCHITECTURE.md](../../docs/architecture/SYSTEM_PROMPT_ARCHITECTURE.md).

---

## Core Concepts

### 1. Versioned Prompt Templates (`src/lib.rs`)
Pre-packaged, parameterized templates for specialized engineering activities:
- `Coding`, `Debugging`, `CodeReview`, `Architecture`, `Testing`, `Security`, `Performance`.
- Supports typed variables (`FilePath`, `CodeSnippet`, `Language`, `Enum`) with validation and fallback defaults.

### 2. Fragment Registry (`src/fragment_registry.rs`)
Condition-gated prompt fragment evaluation:
- Condition types: `ToolEnabled`, `FeatureEnabled`, `SandboxPolicy`, `ApprovalPolicy`, `ContextUsageAbove`.
- Model family alignment with `m31a_core::prompts::ModelFamily`.

---

## Key Modules & Types

| Module | Key Structs / Enums | Purpose |
| :--- | :--- | :--- |
| [`src/lib.rs`](src/lib.rs) | `PromptTemplate`, `PromptLibrary`, `PromptCategory`, `PromptVariable` | Stores, searches, and renders versioned prompt templates with variable substitution. |
| [`src/fragment_registry.rs`](src/fragment_registry.rs) | `FragmentRegistry`, `Fragment`, `FragmentCondition`, `ModelFamily` | Resolves modular system prompt fragments against active runtime conditions. |

---

## Developer Usage

### Listing & Rendering Templates
```rust
use m31a_prompts::{PromptCategory, PromptLibrary};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let library = PromptLibrary::load_builtin()?;
    let review_prompts = library.find_by_category(PromptCategory::CodeReview);
    for prompt in review_prompts {
        println!("Template: {} (v{})", prompt.name, prompt.version);
    }
    Ok(())
}
```

### Inspecting Canonical System Prompts via CLI
```bash
m31a prompt inspect --model nvidia/nemotron-3.5-70b-instruct --role reviewer
m31a prompt inspect --model deepseek-ai/deepseek-r1 --json
```

---

## Testing & Verification

```bash
cargo test -p m31a-prompts
```

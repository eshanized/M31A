# `m31a-prompts`

> System prompt builders, versioned prompt templates, fragment registries, and context injection templates for M31A.

---

## Why Does This Crate Exist?

Prompt engineering in an autonomous agent is not a static text file. Different LLMs require distinct preambles:
- DeepSeek R1 models need explicit instruction formatting to trigger reasoning traces without leaking raw formatting tags.
- NVIDIA Nemotron and Codestral models excel with concise, structured tool instructions.
- Different agent modes (`[⚡ BUILD]` vs `[📋 PLAN]`) require different permission boundaries described directly in their system instructions.
- Custom skills, workspace boundaries, and active tool definitions must be dynamically composed into the prompt at runtime.

`m31a-prompts` provides a modular prompt library and fragment registry. It allows dynamically assembling system prompts from versioned, tested fragments while supporting user-defined custom templates with variable interpolation.

---

## Core Concepts & How It Works

### 1. Dynamic Fragment Registry (`fragment_registry.rs`)
Prompts are composed of composable blocks:
- **`IdentityFragment`**: Defines agent persona, sovereignty, and role.
- **`ToolGuidelinesFragment`**: Describes the 18 available tools, argument formatting, and patch standards.
- **`SafetyPolicyFragment`**: Details sandbox rules, protected paths (`.git`, `.m31a`, `.env`), and approval gates.
- **`SkillInjectionFragment`**: Injects user skills discovered from `~/.m31a/skills`.
- **`ModelSpecificFragment`**: Applies model-tailored formatting rules for NVIDIA NIM slugs.

### 2. Versioned Prompt Templates
Provides pre-packaged, parameterized templates for common engineering workflows:
- `Coding`, `Debugging`, `CodeReview`, `Architecture`, `Testing`, `Security`, `Performance`.
- Templates support variable substitution (e.g., `{{file_path}}`, `{{error_trace}}`) with fallback defaults.

---

## Key Modules & Types

| Module | Key Structs / Enums | Purpose |
| :--- | :--- | :--- |
| [`src/fragment_registry.rs`](src/fragment_registry.rs) | `FragmentRegistry`, `SystemPromptBuilder` | Composes modular system prompt fragments into cohesive context instructions. |
| [`src/lib.rs`](src/lib.rs) | `PromptTemplate`, `PromptLibrary`, `PromptCategory` | Stores, searches, and renders versioned prompt templates. |

---

## Practical Usage Example

```rust
use m31a_prompts::fragment_registry::FragmentRegistry;
use m31a_prompts::{PromptCategory, PromptLibrary};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Compose a model-tailored system prompt
    let registry = FragmentRegistry::default();
    let system_prompt = registry.build_system_prompt(
        "nvidia/nemotron-3.5-lightning-30b-a3b",
        /* is_plan_mode = */ false,
        /* active_tools = */ &["read_file", "write_file", "apply_patch", "exec_command"]
    );

    println!("Constructed System Prompt (first 200 chars):");
    println!("{}", &system_prompt[..200.min(system_prompt.len())]);

    // 2. Query built-in prompt templates
    let library = PromptLibrary::load_builtin()?;
    let review_prompts = library.find_by_category(PromptCategory::CodeReview);
    for prompt in review_prompts {
        println!("Available Review Template: {} (v{})", prompt.name, prompt.version);
    }

    Ok(())
}
```

---

## Testing & Verification

```bash
cargo test -p m31a-prompts
```

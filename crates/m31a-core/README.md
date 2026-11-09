# `m31a-core`

> The foundational data structures, configuration engine, session primitives, and workflow models that power the M31A autonomous agent ecosystem.

---

## Why Does This Crate Exist?

When building a complex autonomous coding assistant composed of 39 crates, you quickly face a classic systems challenge: where do shared types, domain models, and runtime primitives live without creating tangled circular dependencies?

`m31a-core` is the shared foundation of M31A. It doesn't contain the heavy LLM networking logic (that's [`m31a-provider`](../m31a-provider)), the terminal UI widgets (that's [`m31a-tui`](../m31a-tui)), or the Linux sandbox syscalls (that's [`m31a-sandbox`](../m31a-sandbox)). Instead, it defines the *canonical truth* for the entire workspace:

- What does an agent session look like?
- How is `~/.m31a/config.toml` loaded, layered, and validated?
- What are the states and task kinds in an autonomous workflow DAG?
- How are custom skills discovered and parsed from YAML frontmatter?
- How do we enforce filesystem boundary constraints so agents don't escape their working tree?

By keeping these primitives cleanly isolated in `m31a-core`, all downstream crates can communicate using strongly typed, serialized structures.

---

## Core Concepts & How It Works

### 1. Configuration Layering (`Config`)
M31A uses a tiered configuration system. Base configuration is loaded from `~/.m31a/config.toml`, layered with workspace-local overrides (`.m31a/config.toml`), profile overlays (`--profile <name>`), and dynamic CLI flags (`-c key=value`). `m31a-core` parses these using Serde, validates security invariants, and enforces strict schema checks when `--strict-config` is active.

### 2. Autonomous Workflow Models (`workflow.rs`)
Workflows represent complex engineering tasks executed as Directed Acyclic Graphs (DAGs). `m31a-core` defines:
- **`TaskKind`**: Categorizes work into `Plan`, `Execute`, `Verify`, and `Review`.
- **`WorkflowPhase`**: Sequences lifecycle stages (`Discovery` → `Planning` → `Execution` → `Verification` → `Completed`).
- **`ExecutionEvent`**: Append-only event models recording tool runs, diff applications, and state transitions.
- **`ArtifactMetadata`**: Content-addressed metadata for generated code patches, build logs, and test reports.

### 3. Custom Skills Subsystem (`skills.rs`)
Skills extend agent intelligence with specialized instructions. `m31a-core` automatically scans `~/.m31a/skills/`, `.m31a/skills/`, and legacy `.codex/skills/` directories, parses YAML frontmatter (`name`, `description`), and renders instruction blocks into LLM context prompts.

### 4. Filesystem Boundary Guard (`file_system_bounds.rs`)
To prevent accidental path traversals or malicious symlink escapes, `PathBounds` normalizes and validates all target file operations against the project workspace root before tool execution.

---

## Key Modules & Types

| Module | Key Structs / Enums | Purpose |
| :--- | :--- | :--- |
| [`src/config.rs`](src/config.rs) | `Config`, `SandboxPolicy`, `ApprovalPolicy` | Loads, parses, and validates user and profile configuration. |
| [`src/workflow.rs`](src/workflow.rs) | `WorkflowTask`, `TaskKind`, `WorkflowPhase` | Directed Acyclic Graph (DAG) task specifications and phase states. |
| [`src/session.rs`](src/session.rs) | `Session`, `SessionId`, `TurnRecord` | In-memory representations of active conversation sessions. |
| [`src/skills.rs`](src/skills.rs) | `SkillDefinition`, `SkillLoader` | Discovers and parses Markdown skills with YAML frontmatter. |
| [`src/secrets.rs`](src/secrets.rs) | `SecretStore`, `RedactedString` | Encrypted credential storage and in-memory token masking. |
| [`src/file_system_bounds.rs`](src/file_system_bounds.rs) | `PathBounds`, `PathBoundaryCheck` | Validates relative paths against workspace root boundaries. |
| [`src/realtime_voice.rs`](src/realtime_voice.rs) | `VoiceActivityDetector`, `VadConfig` | Real-time adaptive RMS energy voice activity detection. |

---

## Practical Usage Example

Here is how you load configuration, validate workspace bounds, and inspect skills:

```rust
use m31a_core::config::Config;
use m31a_core::file_system_bounds::PathBounds;
use m31a_core::skills::SkillLoader;
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    // 1. Load configuration with defaults and environment overrides
    let config = Config::load_with_overrides(None, &[])?;
    println!("Selected NIM model: {}", config.model);
    println!("Active sandbox policy: {:?}", config.sandbox);

    // 2. Validate that an agent path operation stays inside the workspace
    let workspace_root = PathBuf::from("/home/developer/project");
    let bounds = PathBounds::new(&workspace_root);
    
    let safe_target = bounds.resolve_and_check("src/main.rs")?;
    println!("Safe target resolved: {:?}", safe_target);

    // 3. Discover custom skills in project directory
    let loader = SkillLoader::new();
    let discovered = loader.discover_in(&workspace_root.join(".m31a/skills"))?;
    for skill in discovered {
        println!("Loaded skill: {} - {}", skill.name, skill.description);
    }

    Ok(())
}
```

---

## Integration in M31A Monorepo

- **Upstream Dependencies**: Pure foundation crate relying only on low-level crates like `serde`, `toml`, `dirs`, and `chrono`.
- **Downstream Consumers**: Used by virtually all 38 other crates in the workspace.

---

## Testing & Verification

```bash
# Run all unit tests for m31a-core
cargo test -p m31a-core
```

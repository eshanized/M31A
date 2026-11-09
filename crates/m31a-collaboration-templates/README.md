# `m31a-collaboration-templates`

> Pre-configured multi-agent collaboration patterns, session templates, and guided team workflow blueprints for M31A.

---

## Why Does This Crate Exist?

Setting up effective multi-agent engineering workflows from scratch can be tedious. A security audit requires a completely different tool configuration, approval policy, and persona from a high-speed pair programming session or a speculative refactoring task.

`m31a-collaboration-templates` provides ready-to-run collaborative blueprints:
- Pre-packaged templates for `PairProgramming`, `CodeReview`, `Debugging`, `ArchitectureDesign`, `SecurityAudit`, and `PerformanceOptimization`.
- Defines optimal tool combinations, sandbox isolation levels, and approval presets for each workflow.
- Structures guided multi-step processes so agents methodically progress through planning, execution, and verification.

---

## Core Concepts & How It Works

### 1. Template Categories (`TemplateCategory`)
- **`PairProgramming`**: Interactive back-and-forth session with high verbosity and instant patch verification.
- **`CodeReview`**: Read-only exploration focusing on security, API drift, and code style.
- **`SecurityAudit`**: Airgapped sandbox profile running taint analysis, secret scanning, and dependency legitimacy checks.
- **`PerformanceOptimization`**: Focuses on memory profiling, benchmark generation, and hot-loop optimization.

### 2. Parameterized Blueprint Steps (`TemplateStep`)
Each template contains sequential milestones that guide the agent through its objective without losing focus.

---

## Key Modules & Types

| Struct / Enum | Description |
| :--- | :--- |
| `CollaborationTemplate` | Full specification: ID, name, category, icon, step milestones, and config overrides. |
| `TemplateCategory` | Categorization enum (`PairProgramming`, `CodeReview`, `Debugging`, etc.). |
| `TemplateRegistry` | Central store discovering, validating, and instantiating collaboration templates. |
| `TemplateConfig` | Custom overrides for sandbox policy, approval mode, and active tools. |

---

## Practical Usage Example

```rust
use m31a_collaboration_templates::{CollaborationTemplate, TemplateCategory, TemplateRegistry};

fn main() -> anyhow::Result<()> {
    let registry = TemplateRegistry::load_builtin()?;

    // Find security audit templates
    let templates = registry.find_by_category(TemplateCategory::SecurityAudit);
    for template in templates {
        println!("Template: {} (Steps: {})", template.name, template.steps.len());
    }

    Ok(())
}
```

---

## Testing & Verification

```bash
cargo test -p m31a-collaboration-templates
```

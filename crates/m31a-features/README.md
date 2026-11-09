# `m31a-features`

> Runtime feature flag registry, staged rollouts, capability targeting, and configuration binding for M31A.

---

## Why Does This Crate Exist?

Shipping bleeding-edge AI capabilities—such as Code Mode runtime pragmas, speculative multi-candidate execution, streaming multi-file patch engines, and Guardian v2 supervisors—presents a stability dilemma. You want to iterate rapidly on experimental features without breaking standard daily developer workflows.

`m31a-features` solves this by decoupling code deployment from feature release:
- Developers can safely gate experimental capabilities behind typed flags.
- Users can toggle features via `~/.m31a/config.toml` under `[features]` or dynamically via the CLI (`m31a features enable <name>`).
- Features can specify dependencies (e.g., Feature B requires Feature A) and rollout categories (`Experimental`, `Beta`, `Stable`, `Deprecated`).

---

## Core Concepts & How It Works

### 1. Strongly Typed Feature Keys
All builtin features are registered with typed metadata, descriptions, categories, and default states:
- `code_mode`: High-velocity programmatic tool execution runtime.
- `guardian_v2`: Out-of-band safety review supervisor with circuit breaker.
- `apply_patch_streaming`: Low-latency incremental patch parsing and streaming verification.
- `unified_exec`: Unified subprocess dispatcher across native and sandboxed execution.
- `worktree_management`: Automated Git worktree isolation for temporary agent workspaces.
- `multi_agent_v2`: Enhanced multi-agent coordination with pub-sub message board integration.

### 2. Targeting & Staged Rollouts
`FeatureFlagManager` supports targeting rules, rollout percentages, and environment overrides (e.g., enabled only in CI or staging).

### 3. Dependency Graphs
If a feature depends on another capability (for example, `speculative_v2` depending on `worktree_management`), `m31a-features` ensures dependent flags cannot be enabled without their prerequisites.

---

## Key Types & Structs

| Struct / Enum | Description |
| :--- | :--- |
| `FeatureFlag` | Complete metadata record for a feature (ID, state, dependencies, category). |
| `FeatureFlagManager` | In-memory and persistent manager for registering, inspecting, and toggling flags. |
| `FeatureCategory` | Categorization: `Experimental`, `Beta`, `Stable`, `Deprecated`, `Internal`. |
| `FeatureState` | State enumeration: `Enabled`, `Disabled`, `Archived`. |
| `TargetingRules` | Rules for selective enablement based on user groups or environment criteria. |

---

## Practical Usage Example

```rust
use m31a_features::{FeatureCategory, FeatureFlag, FeatureFlagManager, FeatureState};
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let manager = Arc::new(FeatureFlagManager::new());

    // 1. Check if a feature is enabled
    if manager.is_enabled("code_mode").await {
        println!("Code Mode is enabled! Running programmatic scripts.");
    } else {
        println!("Code Mode disabled. Using single-turn tool calls.");
    }

    // 2. Dynamically toggle a feature flag
    manager.set_state("guardian_v2", FeatureState::Enabled).await?;
    println!("Guardian v2 has been enabled.");

    // 3. Inspect all active experimental features
    let all_flags = manager.list_flags().await;
    for flag in all_flags.into_iter().filter(|f| f.category == FeatureCategory::Experimental) {
        println!("Experimental Flag: {} ({:?})", flag.name, flag.state);
    }

    Ok(())
}
```

---

## CLI Integration

Users can inspect and toggle feature flags directly from their shell:

```bash
# List all registered flags and their states
m31a features list

# Enable or disable a feature flag
m31a features enable code_mode
m31a features disable guardian_v2
```

---

## Testing & Verification

```bash
cargo test -p m31a-features
```

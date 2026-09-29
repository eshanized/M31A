# M31A Plugin Architecture & Lifecycle Hooks

This document details the in-process plugin architecture, extension contracts, lifecycle hooks, and security subordination of the M31 Autonomous (M31A) runtime.

## Core Architectural Invariant

> **"Plugins extend capabilities, but policy governs execution. No plugin tool may bypass the PolicyGate."**

M31A avoids dynamic foreign language runtimes or unverified dynamic link libraries (.so/.dylib) in the core kernel. Instead, plugins are **in-process Rust extensions** implementing the `Plugin` trait, registered during kernel bootstrap.

---

## The `Plugin` Trait Contract

Extensions implement the `Plugin` trait defined in `src/plugin/traits.rs`:

```rust
pub trait Plugin: Send + Sync + 'static {
    /// Return the immutable manifest declaring identity, version, and author.
    fn manifest(&self) -> PluginManifest;

    /// Tools contributed by this plugin (strictly subordinated to PolicyGate).
    fn tools(&self) -> Vec<Arc<dyn AnyTool>> {
        Vec::new()
    }

    /// Capabilities contributed by this plugin.
    fn capabilities(&self) -> Vec<CapabilityDescriptor> {
        Vec::new()
    }

    /// Lifecycle hooks contributed by this plugin.
    fn hooks(&self) -> Vec<Arc<dyn LifecycleHook>> {
        Vec::new()
    }

    /// Declarative policy rules required by this plugin.
    fn policy_declarations(&self) -> Vec<PolicyRule> {
        Vec::new()
    }

    /// JSON schema defining configuration options accepted by this plugin.
    fn config_schema(&self) -> Option<serde_json::Value> {
        None
    }
}
```

---

## Policy Subordination (`PluginToolAdapter`)

Every tool contributed by a plugin is automatically wrapped inside a `PluginToolAdapter`:
1. When a model proposes a plugin tool call, the adapter intercepts the invocation.
2. The invocation payload is passed to the authoritative `PolicyGate`.
3. If the active policy evaluates to `DENY`, the invocation is rejected immediately with zero side effects.
4. If evaluated to `ASK`, interactive confirmation is sought (or denied fail-closed in unattended mode).
5. Only when evaluated to `ALLOW` does the inner tool logic execute.

---

## Lifecycle Hook Pipeline

Plugins can register hooks targeting 16 distinct `LifecyclePoint` events:

- `mission.before` / `mission.after`
- `plan.before` / `plan.after`
- `task.before` / `task.after`
- `agent.spawn` / `agent.complete`
- `tool.before` / `tool.after`
- `command.before` / `command.after`
- `verification.before` / `verification.after`
- `failure.detected`
- `checkpoint.created`

### 4-Stage Deterministic Execution Pipeline

When a lifecycle point triggers, registered hooks execute in strict deterministic order across 4 stages:

1. **Stage 1: `SecurityGates`**: Fast-failing security and permission checks. A failure here aborts the action immediately.
2. **Stage 2: `RequiredTransform`**: Deterministic input/argument sanitization and normalization.
3. **Stage 3: `CoreObservers`**: Subsystem state synchronization, audit recording, and progress updates.
4. **Stage 4: `TelemetryAndCleanup`**: Non-blocking metric emission, span completion, and ephemeral cache eviction.

---

## Authoring an In-Process Plugin

Example implementation of a custom linting and auditing plugin:

```rust
use std::sync::Arc;
use m31a::plugin::traits::Plugin;
use m31a::plugin::dto::{PluginManifest, PluginId};
use m31a::tools::definition::AnyTool;
use m31a::hook::traits::LifecycleHook;

pub struct AuditPlugin;

impl Plugin for AuditPlugin {
    fn manifest(&self) -> PluginManifest {
        PluginManifest {
            id: PluginId::new("enterprise-auditor"),
            name: "Enterprise Security Auditor".to_string(),
            version: "1.0.0".to_string(),
            description: "Enforces enterprise compliance hooks on task completion".to_string(),
            author: "Security Team".to_string(),
        }
    }

    fn tools(&self) -> Vec<Arc<dyn AnyTool>> {
        vec![/* Arc::new(CustomAuditTool) */]
    }

    fn hooks(&self) -> Vec<Arc<dyn LifecycleHook>> {
        vec![/* Arc::new(TaskCompletionAuditHook) */]
    }
}
```

Plugins are registered in the kernel during system startup:

```rust
let mut kernel = KernelBuilder::new();
kernel.register_plugin(Arc::new(AuditPlugin));
```

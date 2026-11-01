# `m31a-extension-api`

Dynamic extension and plugin API for **M31A** supporting `cdylib` loading and capability contributions.

---

## Overview

`m31a-extension-api` allows third-party developers to extend M31A by loading dynamic libraries (`cdylib`) and extension bundles declared via `m31a-plugin.toml`. Plugins can contribute custom tools, lifecycle hooks, and system prompt modifiers.

---

## User-Facing Capabilities

- **Plugin Management (`m31a plugin`)**: Install, list, enable, and inspect external M31A plugins from local directories or package registries.
- **Dynamic Extensibility**: Adds proprietary domain capabilities (e.g. enterprise database connectors, internal CI dispatchers) without recompiling the main binary.

---

## Developer Guide & Architecture

### Plugin Manifest (`m31a-plugin.toml`)

```toml
name = "my-enterprise-plugin"
version = "0.1.0"
description = "Enterprise build and deploy tool integrations"
entry_point = "target/release/libmy_plugin.so"
min_m31a_version = "0.1.0"

[[capabilities]]
type = "tool"
name = "deploy_staging"
description = "Trigger canary deployment to staging cluster"
```

### Key Modules

- `PluginManifest`: Deserializes plugin metadata and version compatibility checks.
- `PluginManager`: Loads symbols using `libloading` with strict memory bounds and safety checks.

---

## Testing

```bash
cargo test -p m31a-extension-api
```

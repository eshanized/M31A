# `m31a-extension-api`

> Stable C ABI vtable interfaces, dynamic native plugin loader (`cdylib`), capability manifests, and extension lifecycle management for M31A.

---

## Why Does This Crate Exist?

While MCP servers run as external subprocesses communicating over JSON-RPC, certain extensions require maximum in-process performance:
- High-frequency AST linters and static analyzers.
- Direct hardware and GPU sensor telemetry.
- Custom in-memory caching and proprietary protocol bridges.

If you compile plugins directly as Rust trait objects, you run into Rust's unstable ABI problem: plugins compiled with compiler version $X$ crash or segfault when loaded by a host compiled with version $Y$.

`m31a-extension-api` solves this by establishing a **Stable Versioned C ABI Function Table (`PluginVTableV1`)**:
- External developers can write compiled dynamic libraries (`cdylib`: `.so`, `.dylib`, `.dll`) in Rust, C, C++, or Zig.
- The host dynamically loads libraries via `libloading` with panic-catching barriers (`catch_unwind`).
- Communication flows across the C ABI boundary using JSON payloads and versioned function pointers, ensuring long-term ABI stability.

---

## Core Concepts & How It Works

### 1. Versioned C ABI Function Table (`PluginVTableV1`)
Defines the binary layout contract:
- `abi_version`: Matches `M31A_PLUGIN_ABI_VERSION = 1`.
- `initialize`: Called on load with configuration JSON.
- `shutdown`: Cleanup hook prior to library unloading.
- `get_manifest_json`: Returns extension ID, version, author, and requested capabilities.
- `get_tools_json`: Declares custom tool definitions.
- `invoke_tool`: Executes a tool call in-process with error buffers.

### 2. Panic Isolation & Memory Safety
Native plugins run in the host process memory. To prevent a buggy plugin from crashing the entire M31A daemon, all vtable calls are wrapped in panic catchers and return standard error codes (`M31A_PLUGIN_OK`, `M31A_PLUGIN_ERR_PANIC`, `M31A_PLUGIN_ERR_EXECUTION`).

---

## Key Modules & Types

| Struct / Enum | Description |
| :--- | :--- |
| `PluginVTableV1` | `#[repr(C)]` function pointer table forming the binary interface contract. |
| `PluginLoader` | Scans `~/.m31a/plugins/`, verifies cryptographic signatures, and loads libraries. |
| `PluginManifest` | Metadata declaring extension name, version, author, tools, and permissions. |
| `DynamicPlugin` | Safe Rust wrapper managing an active loaded `libloading::Library` handle. |

---

## Practical Usage Example

Declaring a native plugin in Rust:

```rust
use m31a_extension_api::*;
use std::ffi::{c_char, c_int, c_void};

#[no_mangle]
pub extern "C" fn m31a_plugin_entry_v1() -> *const PluginVTableV1 {
    static VTABLE: PluginVTableV1 = PluginVTableV1 {
        abi_version: M31A_PLUGIN_ABI_VERSION,
        initialize: my_initialize,
        shutdown: my_shutdown,
        get_manifest_json: my_manifest,
        get_tools_json: my_tools,
        get_context_providers_json: my_empty_providers,
        get_prompt_templates_json: my_empty_templates,
        invoke_tool: my_invoke_tool,
        provide_context: my_empty_context,
        format_prompt: my_empty_prompt,
    };
    &VTABLE
}

unsafe extern "C" fn my_initialize(_handle: *mut c_void, _cfg: *const c_char, _err: *mut *mut c_char) -> c_int {
    M31A_PLUGIN_OK
}

unsafe extern "C" fn my_shutdown(_handle: *mut c_void, _err: *mut *mut c_char) -> c_int {
    M31A_PLUGIN_OK
}

// Additional vtable function implementations...
```

---

## Testing & Verification

```bash
cargo test -p m31a-extension-api
```

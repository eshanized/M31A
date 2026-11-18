//! WebAssembly Plugin Runtime Engine.
//!
//! Provides isolated WebAssembly module instantiation, execution limits (fuel/memory),
//! capability-gated host bindings, and structured error handling.

use super::errors::WasmError;
use super::host::HostContext;
use super::manifest::WasmPluginManifest;
use std::path::PathBuf;
use tracing::{debug, warn};
use wasmi::{Caller, Config, Engine, Linker, Memory, Module, Store, TypedFunc};

/// A running, sandboxed WebAssembly plugin instance.
pub struct WasmPluginInstance {
    manifest: WasmPluginManifest,
    store: Store<HostContext>,
    memory: Memory,
    entry_tool_call: Option<TypedFunc<(i32, i32, i32, i32), i32>>,
}

impl WasmPluginInstance {
    pub fn manifest(&self) -> &WasmPluginManifest {
        &self.manifest
    }

    /// Invokes a declared tool exported by the WebAssembly plugin.
    pub fn call_tool(&mut self, tool_name: &str, args_json: &str) -> Result<String, WasmError> {
        let entry = self
            .entry_tool_call
            .ok_or_else(|| WasmError::ModuleValidationFailed("Plugin does not export 'm31a_tool_call'".to_string()))?;

        // 1. Write tool_name and args_json into guest memory buffer (e.g. at offset 1024 and 2048)
        let name_bytes = tool_name.as_bytes();
        let args_bytes = args_json.as_bytes();

        let name_offset = 1024u32;
        let args_offset = 2048u32;

        self.memory
            .write(&mut self.store, name_offset as usize, name_bytes)
            .map_err(|_e| WasmError::BufferOutOfBounds {
                offset: name_offset as usize,
                len: name_bytes.len(),
                mem_size: self.memory.data(&self.store).len(),
            })?;

        self.memory
            .write(&mut self.store, args_offset as usize, args_bytes)
            .map_err(|_e| WasmError::BufferOutOfBounds {
                offset: args_offset as usize,
                len: args_bytes.len(),
                mem_size: self.memory.data(&self.store).len(),
            })?;

        // 2. Call guest entrypoint
        let res_code = entry
            .call(
                &mut self.store,
                (
                    name_offset as i32,
                    name_bytes.len() as i32,
                    args_offset as i32,
                    args_bytes.len() as i32,
                ),
            )
            .map_err(|trap| {
                let trap_str = trap.to_string();
                if trap_str.contains("fuel")
                    || self.store.get_fuel().map(|f| f == 0).unwrap_or(false)
                {
                    return WasmError::FuelExhausted {
                        consumed: self.manifest.limits.max_fuel,
                        limit: self.manifest.limits.max_fuel,
                    };
                }
                WasmError::GuestTrap(trap_str)
            })?;

        if res_code < 0 {
            return Err(WasmError::GuestTrap(format!(
                "Guest function returned error code {}",
                res_code
            )));
        }

        // 3. Read output from guest memory (offset 4096, with length in res_code)
        let out_len = res_code as usize;
        let out_offset = 4096usize;
        let mem_data = self.memory.data(&self.store);

        if out_offset + out_len > mem_data.len() {
            return Err(WasmError::BufferOutOfBounds {
                offset: out_offset,
                len: out_len,
                mem_size: mem_data.len(),
            });
        }

        let slice = &mem_data[out_offset..out_offset + out_len];
        let out_str = std::str::from_utf8(slice)
            .map_err(|e| WasmError::GuestTrap(format!("Invalid UTF-8 output from guest: {e}")))?;

        Ok(out_str.to_string())
    }

    /// Returns remaining fuel budget in store.
    pub fn remaining_fuel(&self) -> Option<u64> {
        self.store.get_fuel().ok()
    }
}

pub struct WasmRuntime {
    engine: Engine,
}

impl Default for WasmRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl WasmRuntime {
    pub fn new() -> Self {
        let mut config = Config::default();
        config.consume_fuel(true);

        let engine = Engine::new(&config);
        Self { engine }
    }

    /// Loads and validates a WebAssembly plugin from raw bytecode or WAT text.
    pub fn load_plugin(
        &self,
        manifest: WasmPluginManifest,
        wasm_bytes: &[u8],
        workspace_root: impl Into<PathBuf>,
        config_json: impl Into<String>,
    ) -> Result<WasmPluginInstance, WasmError> {
        manifest.validate()?;

        let module = Module::new(&self.engine, wasm_bytes)
            .map_err(|e| WasmError::ModuleValidationFailed(format!("Invalid Wasm module: {e}")))?;

        let host_ctx = HostContext::new(manifest.clone(), workspace_root, config_json);
        let mut store = Store::new(&self.engine, host_ctx);

        // Configure initial fuel budget
        store
            .set_fuel(manifest.limits.max_fuel)
            .map_err(|e| WasmError::Internal(anyhow::anyhow!("Failed to set fuel: {e}")))?;

        let mut linker = Linker::<HostContext>::new(&self.engine);

        // Bind host logging
        linker
            .define(
                "m31a_host",
                "host_log",
                wasmi::Func::wrap(&mut store, |caller: Caller<'_, HostContext>, level: i32, ptr: i32, len: i32| {
                    if let Some(mem) = caller.data().memory {
                        if let Ok(msg) = HostContext::read_guest_string(&mem, &caller, ptr as u32, len as u32) {
                            match level {
                                1 => warn!(target: "wasm_guest", "{}", msg),
                                _ => debug!(target: "wasm_guest", "{}", msg),
                            }
                        }
                    }
                }),
            )
            .map_err(|e| WasmError::Internal(anyhow::anyhow!("Failed to link host_log: {e}")))?;

        // Bind host filesystem read
        linker
            .define(
                "m31a_host",
                "host_fs_read",
                wasmi::Func::wrap(
                    &mut store,
                    |mut caller: Caller<'_, HostContext>, path_ptr: i32, path_len: i32, out_ptr: i32, max_len: i32| -> i32 {
                        caller.data_mut().host_calls_count += 1;
                        let mem = match caller.data().memory {
                            Some(m) => m,
                            None => return -1,
                        };

                        let path_str = match HostContext::read_guest_string(&mem, &caller, path_ptr as u32, path_len as u32) {
                            Ok(s) => s,
                            Err(_) => return -2,
                        };

                        let validated_path = match caller.data().validate_path(&path_str, false) {
                            Ok(p) => p,
                            Err(_) => return -3, // Unauthorized capability
                        };

                        let content = match std::fs::read(&validated_path) {
                            Ok(c) => c,
                            Err(_) => return -4,
                        };

                        if content.len() > max_len as usize {
                            return -5; // Buffer too small
                        }

                        if HostContext::write_guest_bytes(&mem, &mut caller, out_ptr as u32, &content).is_err() {
                            return -6;
                        }

                        content.len() as i32
                    },
                ),
            )
            .map_err(|e| WasmError::Internal(anyhow::anyhow!("Failed to link host_fs_read: {e}")))?;

        // Instantiate and start
        let instance = linker
            .instantiate_and_start(&mut store, &module)
            .map_err(|e| WasmError::ModuleValidationFailed(format!("Failed to instantiate module: {e}")))?;

        // Extract linear memory
        let memory = instance
            .get_memory(&store, "memory")
            .ok_or_else(|| WasmError::ModuleValidationFailed("Guest module does not export 'memory'".to_string()))?;

        // Enforce memory page limits
        let initial_pages = memory.size(&store) as u32;
        if initial_pages > manifest.limits.max_memory_pages {
            return Err(WasmError::MemoryLimitExceeded {
                pages: initial_pages,
                max_pages: manifest.limits.max_memory_pages,
            });
        }

        store.data_mut().memory = Some(memory);

        let entry_tool_call = instance.get_typed_func::<(i32, i32, i32, i32), i32>(&store, "m31a_tool_call").ok();

        Ok(WasmPluginInstance {
            manifest,
            store,
            memory,
            entry_tool_call,
        })
    }
}

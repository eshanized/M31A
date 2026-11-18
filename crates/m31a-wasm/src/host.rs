//! Capability-Gated Host Interface for WebAssembly Guests.
//!
//! Exposes bounded, strictly audited host functions to WebAssembly modules.
//! Ambient access to host filesystem, network, or OS functions is completely eliminated.

use super::errors::WasmError;
use super::manifest::WasmPluginManifest;
use std::path::PathBuf;
use wasmi::{Caller, Memory};

/// Host state shared with guest invocations.
pub struct HostContext {
    pub manifest: WasmPluginManifest,
    pub workspace_root: PathBuf,
    pub config_json: String,
    pub host_calls_count: u64,
    pub memory: Option<Memory>,
}

impl HostContext {
    pub fn new(
        manifest: WasmPluginManifest,
        workspace_root: impl Into<PathBuf>,
        config_json: impl Into<String>,
    ) -> Self {
        Self {
            manifest,
            workspace_root: workspace_root.into(),
            config_json: config_json.into(),
            host_calls_count: 0,
            memory: None,
        }
    }

    /// Validates and canonicalizes a requested path against workspace root and manifest permissions.
    pub fn validate_path(&self, relative_path: &str, is_write: bool) -> Result<PathBuf, WasmError> {
        let clean = relative_path.trim_start_matches(['/', '\\']);
        if clean.contains("..") {
            return Err(WasmError::UnauthorizedCapability {
                capability: format!("Directory traversal (..) rejected: '{}'", relative_path),
            });
        }

        if is_write {
            if !self.manifest.can_write_path(clean) {
                return Err(WasmError::UnauthorizedCapability {
                    capability: format!("Filesystem write permission not declared for path '{}'", clean),
                });
            }
        } else if !self.manifest.can_read_path(clean) {
            return Err(WasmError::UnauthorizedCapability {
                capability: format!("Filesystem read permission not declared for path '{}'", clean),
            });
        }

        let full_path = self.workspace_root.join(clean);
        Ok(full_path)
    }

    /// Reads UTF-8 string from guest linear memory with bounds checking.
    pub fn read_guest_string(
        memory: &Memory,
        caller: &Caller<'_, HostContext>,
        ptr: u32,
        len: u32,
    ) -> Result<String, WasmError> {
        let ptr = ptr as usize;
        let len = len as usize;
        let data = memory.data(caller);

        if ptr.checked_add(len).map(|end| end > data.len()).unwrap_or(true) {
            return Err(WasmError::BufferOutOfBounds {
                offset: ptr,
                len,
                mem_size: data.len(),
            });
        }

        let slice = &data[ptr..ptr + len];
        std::str::from_utf8(slice)
            .map(|s| s.to_string())
            .map_err(|e| WasmError::GuestTrap(format!("Invalid UTF-8 in guest memory: {e}")))
    }

    /// Writes raw bytes to guest linear memory with bounds checking.
    pub fn write_guest_bytes(
        memory: &Memory,
        caller: &mut Caller<'_, HostContext>,
        ptr: u32,
        bytes: &[u8],
    ) -> Result<(), WasmError> {
        let ptr = ptr as usize;
        let len = bytes.len();
        let mem_size = memory.data(&mut *caller).len();

        if ptr.checked_add(len).map(|end| end > mem_size).unwrap_or(true) {
            return Err(WasmError::BufferOutOfBounds {
                offset: ptr,
                len,
                mem_size,
            });
        }

        memory
            .write(caller, ptr, bytes)
            .map_err(|e| WasmError::GuestTrap(format!("Failed to write to guest memory: {e}")))?;
        Ok(())
    }
}

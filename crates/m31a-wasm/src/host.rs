//! Capability-Gated Host Interface for WebAssembly Guests.
//!
//! Exposes bounded, strictly audited host functions to WebAssembly modules.
//! Ambient access to host filesystem, network, or OS functions is completely eliminated.

use super::errors::WasmError;
use super::manifest::WasmPluginManifest;
use std::path::{Component, Path, PathBuf};
use wasmi::{Caller, Memory, StoreLimits};

/// Host state shared with guest invocations.
pub struct HostContext {
    pub manifest: WasmPluginManifest,
    pub workspace_root: PathBuf,
    pub config_json: String,
    pub host_calls_count: u64,
    pub memory: Option<Memory>,
    pub limits: StoreLimits,
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
            limits: StoreLimits::default(),
        }
    }

    /// Validates and resolves a workspace-relative path against manifest permissions.
    ///
    /// Parent directories are canonicalized and the final component is rejected when
    /// it is a symlink. Host filesystem access is limited to regular files contained
    /// within the canonical workspace root.
    pub fn validate_path(&self, relative_path: &str, is_write: bool) -> Result<PathBuf, WasmError> {
        if relative_path.trim().is_empty()
            || relative_path.contains('\\')
            || relative_path.contains(':')
            || relative_path.contains('\0')
        {
            return Err(WasmError::UnauthorizedCapability {
                capability: format!("Invalid workspace-relative path: '{}'", relative_path),
            });
        }

        let relative = Path::new(relative_path);
        if relative.is_absolute()
            || !relative
                .components()
                .all(|component| matches!(component, Component::Normal(_)))
        {
            return Err(WasmError::UnauthorizedCapability {
                capability: format!("Absolute or traversing path rejected: '{}'", relative_path),
            });
        }

        let allowed = if is_write {
            self.manifest.can_write_path(relative_path)
        } else {
            self.manifest.can_read_path(relative_path)
        };
        if !allowed {
            return Err(WasmError::UnauthorizedCapability {
                capability: format!(
                    "Filesystem {} permission not declared for path '{}'",
                    if is_write { "write" } else { "read" },
                    relative_path
                ),
            });
        }

        let root = self.workspace_root.canonicalize().map_err(|error| {
            WasmError::UnauthorizedCapability {
                capability: format!("Workspace root cannot be canonicalized: {error}"),
            }
        })?;
        let candidate = root.join(relative);
        let parent = candidate.parent().ok_or_else(|| WasmError::UnauthorizedCapability {
            capability: "Requested path has no workspace parent".to_string(),
        })?;
        let canonical_parent = parent.canonicalize().map_err(|error| {
            WasmError::UnauthorizedCapability {
                capability: format!("Requested parent directory is unavailable: {error}"),
            }
        })?;

        if !canonical_parent.starts_with(&root) {
            return Err(WasmError::UnauthorizedCapability {
                capability: format!("Path escapes workspace through a symlink: '{}'", relative_path),
            });
        }

        let file_name = candidate.file_name().ok_or_else(|| {
            WasmError::UnauthorizedCapability {
                capability: "Requested path does not identify a file".to_string(),
            }
        })?;
        let resolved = canonical_parent.join(file_name);

        match std::fs::symlink_metadata(&resolved) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(WasmError::UnauthorizedCapability {
                    capability: format!("Symbolic-link targets are not accessible to WASM host calls: '{}'", relative_path),
                });
            }
            Ok(metadata) if !metadata.is_file() => {
                return Err(WasmError::UnauthorizedCapability {
                    capability: format!("Requested path is not a regular file: '{}'", relative_path),
                });
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && is_write => {}
            Err(error) => {
                return Err(WasmError::UnauthorizedCapability {
                    capability: format!("Requested file cannot be inspected: {error}"),
                });
            }
        }

        if !is_write {
            let canonical_file = resolved.canonicalize().map_err(|error| {
                WasmError::UnauthorizedCapability {
                    capability: format!("Requested file cannot be canonicalized: {error}"),
                }
            })?;
            if !canonical_file.starts_with(&root) {
                return Err(WasmError::UnauthorizedCapability {
                    capability: format!("Path escapes workspace: '{}'", relative_path),
                });
            }
            return Ok(canonical_file);
        }

        Ok(resolved)
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

        if ptr
            .checked_add(len)
            .map(|end| end > data.len())
            .unwrap_or(true)
        {
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

        if ptr
            .checked_add(len)
            .map(|end| end > mem_size)
            .unwrap_or(true)
        {
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

//! Structured Errors for WebAssembly Extension Execution.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum WasmError {
    #[error("Plugin manifest validation failed: {0}")]
    InvalidManifest(String),

    #[error("WebAssembly module validation failed: {0}")]
    ModuleValidationFailed(String),

    #[error("WebAssembly ABI version mismatch: expected {expected}, module declared {got}")]
    AbiVersionMismatch { expected: u32, got: u32 },

    #[error("Execution terminated: fuel limit exhausted ({consumed}/{limit} fuel units consumed). Possible infinite loop in guest.")]
    FuelExhausted { consumed: u64, limit: u64 },

    #[error("Memory allocation ceiling exceeded: guest requested {pages} pages, maximum allowed is {max_pages} pages")]
    MemoryLimitExceeded { pages: u32, max_pages: u32 },

    #[error("Unauthorized capability violation: guest attempted '{capability}' without declared permission in manifest")]
    UnauthorizedCapability { capability: String },

    #[error("Host call '{call}' timed out after {timeout_ms}ms")]
    HostCallTimeout { call: String, timeout_ms: u64 },

    #[error("WebAssembly guest trap: {0}")]
    GuestTrap(String),

    #[error("Guest buffer access out of bounds: offset {offset}, length {len}, memory size {mem_size}")]
    BufferOutOfBounds {
        offset: usize,
        len: usize,
        mem_size: usize,
    },

    #[error("Guest execution cancelled by host")]
    Cancelled,

    #[error("Internal runtime error: {0}")]
    Internal(#[from] anyhow::Error),
}

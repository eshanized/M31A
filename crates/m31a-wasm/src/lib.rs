//! `m31a-wasm`: Sandboxed WebAssembly extension runtime for M31A.
//!
//! Provides portable, capability-based WebAssembly extension execution for plugins
//! written in Rust, Go, or AssemblyScript. Enforces strict linear memory ceilings,
//! fuel-based instruction limits, and capability-gated host interfaces.

pub mod benchmark;
pub mod errors;
pub mod host;
pub mod manifest;
pub mod runtime;

pub use benchmark::{WasmBenchmarkReport, WasmBenchmarker};
pub use errors::WasmError;
pub use host::HostContext;
pub use manifest::{M31A_WASM_ABI_VERSION, WasmCapability, WasmPluginManifest, WasmResourceLimits};
pub use runtime::{WasmPluginInstance, WasmRuntime};

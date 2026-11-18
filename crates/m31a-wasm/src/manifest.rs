//! WebAssembly Plugin Manifest and Capability Declarations.
//!
//! Enforces least-privilege capability manifests. Untrusted modules have zero ambient
//! filesystem, network, or tool execution privileges unless explicitly declared and granted.

use super::errors::WasmError;
use serde::{Deserialize, Serialize};

pub const M31A_WASM_ABI_VERSION: u32 = 1;

/// Specific capabilities a WebAssembly guest plugin may request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WasmCapability {
    /// Provides custom tools callable by M31A agents
    ToolProvider,
    /// Injects domain-specific context into prompts
    ContextProvider,
    /// Approval and security hooks
    ApprovalHook,
    /// Filesystem read access restricted to specific subpaths inside the workspace
    RestrictedFsRead { allowed_paths: Vec<String> },
    /// Filesystem write access restricted to specific subpaths inside the workspace
    RestrictedFsWrite { allowed_paths: Vec<String> },
    /// Outbound HTTP fetch restricted to specific destination domains
    RestrictedNetwork { allowed_domains: Vec<String> },
}

/// Resource bounds configured for guest module execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmResourceLimits {
    /// Maximum linear memory pages (1 page = 64 KiB). 64 pages = 4 MiB, 256 pages = 16 MiB.
    pub max_memory_pages: u32,
    /// Maximum fuel units (CPU instructions) allowed per invocation.
    pub max_fuel: u64,
    /// Execution deadline in milliseconds.
    pub execution_deadline_ms: u64,
}

impl Default for WasmResourceLimits {
    fn default() -> Self {
        Self {
            max_memory_pages: 128,  // 8 MiB default ceiling
            max_fuel: 2_000_000,   // 2M instructions default
            execution_deadline_ms: 10_000, // 10s timeout
        }
    }
}

/// Declarative plugin manifest (`m31a-wasm-plugin.toml`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmPluginManifest {
    pub name: String,
    pub version: String,
    #[serde(default = "default_abi_version")]
    pub abi_version: u32,
    pub description: String,
    pub author: Option<String>,
    #[serde(default)]
    pub capabilities: Vec<WasmCapability>,
    #[serde(default)]
    pub limits: WasmResourceLimits,
}

fn default_abi_version() -> u32 {
    M31A_WASM_ABI_VERSION
}

impl WasmPluginManifest {
    pub fn parse_toml(toml_str: &str) -> Result<Self, WasmError> {
        let manifest: Self = toml::from_str(toml_str)
            .map_err(|e| WasmError::InvalidManifest(format!("Failed to parse TOML: {e}")))?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn validate(&self) -> Result<(), WasmError> {
        if self.name.trim().is_empty() {
            return Err(WasmError::InvalidManifest("Plugin name cannot be empty".to_string()));
        }
        if self.abi_version != M31A_WASM_ABI_VERSION {
            return Err(WasmError::AbiVersionMismatch {
                expected: M31A_WASM_ABI_VERSION,
                got: self.abi_version,
            });
        }
        if self.limits.max_memory_pages == 0 || self.limits.max_memory_pages > 1024 {
            return Err(WasmError::InvalidManifest(format!(
                "max_memory_pages must be between 1 and 1024, got {}",
                self.limits.max_memory_pages
            )));
        }
        if self.limits.max_fuel == 0 {
            return Err(WasmError::InvalidManifest("max_fuel must be greater than zero".to_string()));
        }
        Ok(())
    }

    /// Checks if the manifest has declared permission to read from `path`.
    pub fn can_read_path(&self, path: &str) -> bool {
        for cap in &self.capabilities {
            if let WasmCapability::RestrictedFsRead { allowed_paths } = cap {
                for allowed in allowed_paths {
                    if path.starts_with(allowed) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Checks if the manifest has declared permission to write to `path`.
    pub fn can_write_path(&self, path: &str) -> bool {
        for cap in &self.capabilities {
            if let WasmCapability::RestrictedFsWrite { allowed_paths } = cap {
                for allowed in allowed_paths {
                    if path.starts_with(allowed) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Checks if the manifest has declared permission to contact `domain`.
    pub fn can_access_domain(&self, domain: &str) -> bool {
        for cap in &self.capabilities {
            if let WasmCapability::RestrictedNetwork { allowed_domains } = cap {
                for allowed in allowed_domains {
                    if domain.eq_ignore_ascii_case(allowed)
                        || domain.ends_with(&format!(".{}", allowed))
                    {
                        return true;
                    }
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_parsing_and_capability_checks() {
        let toml_str = r#"
name = "markdown-linter"
version = "1.0.0"
description = "A WebAssembly markdown linting extension"
author = "Dev Team"

[[capabilities]]
type = "restricted_fs_read"
allowed_paths = ["docs/", "README.md"]

[[capabilities]]
type = "restricted_network"
allowed_domains = ["api.linter.org"]

[limits]
max_memory_pages = 64
max_fuel = 1000000
execution_deadline_ms = 5000
"#;

        let manifest = WasmPluginManifest::parse_toml(toml_str).unwrap();
        assert_eq!(manifest.name, "markdown-linter");
        assert_eq!(manifest.abi_version, M31A_WASM_ABI_VERSION);
        assert!(manifest.can_read_path("docs/architecture.md"));
        assert!(manifest.can_read_path("README.md"));
        assert!(!manifest.can_read_path("src/main.rs"));
        assert!(!manifest.can_write_path("docs/architecture.md"));

        assert!(manifest.can_access_domain("api.linter.org"));
        assert!(!manifest.can_access_domain("malicious.site"));
    }

    #[test]
    fn test_manifest_rejects_invalid_abi() {
        let toml_str = r#"
name = "test-plugin"
version = "0.1.0"
abi_version = 999
description = "Invalid ABI"
"#;

        let err = WasmPluginManifest::parse_toml(toml_str).unwrap_err();
        assert!(matches!(err, WasmError::AbiVersionMismatch { expected: 1, got: 999 }));
    }
}

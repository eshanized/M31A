//! WebAssembly Plugin Manifest and Capability Declarations.
//!
//! Enforces least-privilege capability manifests. Untrusted modules have zero ambient
//! filesystem, network, or tool execution privileges unless explicitly declared and granted.

use super::errors::WasmError;
use serde::{Deserialize, Serialize};
use std::path::{Component, Path};

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
            max_memory_pages: 128,         // 8 MiB default ceiling
            max_fuel: 2_000_000,           // 2M instructions default
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

/// Returns true only for normalized, non-empty workspace-relative paths.
fn is_safe_relative_path(value: &str) -> bool {
    if value.trim().is_empty()
        || value.contains('\\')
        || value.contains(':')
        || value.contains('\0')
    {
        return false;
    }

    let path = Path::new(value);
    !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

/// Component-aware prefix matching prevents sibling-name and parent traversal bypasses.
fn path_is_within_allowlist(path: &str, allowed: &str) -> bool {
    if !is_safe_relative_path(path) || !is_safe_relative_path(allowed) {
        return false;
    }
    Path::new(path).starts_with(Path::new(allowed))
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
            return Err(WasmError::InvalidManifest(
                "Plugin name cannot be empty".to_string(),
            ));
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
            return Err(WasmError::InvalidManifest(
                "max_fuel must be greater than zero".to_string(),
            ));
        }
        for capability in &self.capabilities {
            match capability {
                WasmCapability::RestrictedFsRead { allowed_paths }
                | WasmCapability::RestrictedFsWrite { allowed_paths } => {
                    if allowed_paths.is_empty()
                        || allowed_paths.iter().any(|path| !is_safe_relative_path(path))
                    {
                        return Err(WasmError::InvalidManifest(
                            "Filesystem capability paths must be non-empty normalized relative paths without traversal".to_string(),
                        ));
                    }
                }
                WasmCapability::RestrictedNetwork { allowed_domains } => {
                    if allowed_domains.is_empty()
                        || allowed_domains.iter().any(|domain| {
                            domain.trim().is_empty()
                                || domain.contains('/')
                                || domain.contains('\\')
                                || domain.contains('@')
                                || domain.contains(':')
                        })
                    {
                        return Err(WasmError::InvalidManifest(
                            "Network capability domains must be non-empty hostnames without ports or URL syntax".to_string(),
                        ));
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Checks if the manifest has declared permission to read from `path`.
    pub fn can_read_path(&self, path: &str) -> bool {
        self.capabilities.iter().any(|cap| match cap {
            WasmCapability::RestrictedFsRead { allowed_paths } => allowed_paths
                .iter()
                .any(|allowed| path_is_within_allowlist(path, allowed)),
            _ => false,
        })
    }

    /// Checks if the manifest has declared permission to write to `path`.
    pub fn can_write_path(&self, path: &str) -> bool {
        self.capabilities.iter().any(|cap| match cap {
            WasmCapability::RestrictedFsWrite { allowed_paths } => allowed_paths
                .iter()
                .any(|allowed| path_is_within_allowlist(path, allowed)),
            _ => false,
        })
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
        assert!(!manifest.can_read_path("README.md.bak"));
        assert!(!manifest.can_read_path("docs-private/secret.txt"));
        assert!(!manifest.can_read_path("docs/../secret.txt"));
        assert!(!manifest.can_read_path("/etc/passwd"));
        assert!(!manifest.can_write_path("docs/architecture.md"));

        assert!(manifest.can_access_domain("api.linter.org"));
        assert!(!manifest.can_access_domain("malicious.site"));
    }

    #[test]
    fn test_manifest_rejects_empty_and_traversal_allowlist_entries() {
        for bad_path in ["", "../secret", "docs/../secret", "/etc/passwd", "C:\\secret"] {
            let manifest = WasmPluginManifest {
                name: "invalid-paths".to_string(),
                version: "1.0.0".to_string(),
                abi_version: M31A_WASM_ABI_VERSION,
                description: String::new(),
                author: None,
                capabilities: vec![WasmCapability::RestrictedFsRead {
                    allowed_paths: vec![bad_path.to_string()],
                }],
                limits: WasmResourceLimits::default(),
            };
            assert!(manifest.validate().is_err(), "accepted {bad_path:?}");
        }
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
        assert!(matches!(
            err,
            WasmError::AbiVersionMismatch {
                expected: 1,
                got: 999
            }
        ));
    }
}

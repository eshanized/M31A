//! Dynamic workspace environment selection and network egress permission sets.
//!
//! Inspects host and workspace project toolchains (Rust, Node, Python, Go, Java, Docker, C/C++),
//! configures fine-grained network domain and unix socket egress permissions,
//! and determines the optimal sandbox runtime environment.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

/// Known developer project toolchain types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ToolchainKind {
    Rust,
    NodeJs,
    Python,
    Go,
    Java,
    Docker,
    CCpp,
    Generic,
}

/// Discovered toolchain metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolchainInfo {
    pub kind: ToolchainKind,
    pub primary_binary: String,
    pub version: Option<String>,
    pub package_manager: Option<String>,
}

/// Allowed outbound domain entry with optional wildcard support.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkDomainPermissionEntry {
    pub pattern: String,
    pub allowed_ports: Vec<u16>,
    pub reason: String,
}

impl NetworkDomainPermissionEntry {
    pub fn new(pattern: impl Into<String>, ports: Vec<u16>, reason: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            allowed_ports: ports,
            reason: reason.into(),
        }
    }

    /// Match a target hostname against the pattern (supports `*` wildcard prefixes e.g. `*.nvidia.com`).
    pub fn matches(&self, hostname: &str) -> bool {
        let host = hostname.to_ascii_lowercase();
        let pat = self.pattern.to_ascii_lowercase();

        if pat == "*" {
            return true;
        }

        if let Some(suffix) = pat.strip_prefix("*.") {
            host == suffix || host.ends_with(&format!(".{}", suffix))
        } else {
            host == pat
        }
    }
}

/// Set of allowed outbound network domains for an agent or execution environment.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkDomainPermissions {
    pub entries: Vec<NetworkDomainPermissionEntry>,
}

impl NetworkDomainPermissions {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Standard sovereign developer default domains (NVIDIA NIM, GitHub, crates.io, npmjs).
    pub fn standard_defaults() -> Self {
        Self {
            entries: vec![
                NetworkDomainPermissionEntry::new(
                    "*.nvidia.com",
                    vec![443],
                    "Model provider inference",
                ),
                NetworkDomainPermissionEntry::new(
                    "*.github.com",
                    vec![443, 22],
                    "Git repository operations",
                ),
                NetworkDomainPermissionEntry::new(
                    "github.com",
                    vec![443, 22],
                    "Git repository operations",
                ),
                NetworkDomainPermissionEntry::new(
                    "*.crates.io",
                    vec![443],
                    "Rust dependency index",
                ),
                NetworkDomainPermissionEntry::new("crates.io", vec![443], "Rust dependency index"),
                NetworkDomainPermissionEntry::new(
                    "*.npmjs.org",
                    vec![443],
                    "NodeJS dependency registry",
                ),
                NetworkDomainPermissionEntry::new(
                    "registry.npmjs.org",
                    vec![443],
                    "NodeJS dependency registry",
                ),
                NetworkDomainPermissionEntry::new("pypi.org", vec![443], "Python package index"),
                NetworkDomainPermissionEntry::new(
                    "files.pythonhosted.org",
                    vec![443],
                    "Python package distribution",
                ),
            ],
        }
    }

    pub fn add_permission(
        &mut self,
        pattern: impl Into<String>,
        ports: Vec<u16>,
        reason: impl Into<String>,
    ) {
        self.entries
            .push(NetworkDomainPermissionEntry::new(pattern, ports, reason));
    }

    pub fn is_allowed(&self, hostname: &str, port: u16) -> bool {
        for entry in &self.entries {
            if entry.matches(hostname)
                && (entry.allowed_ports.is_empty() || entry.allowed_ports.contains(&port))
            {
                return true;
            }
        }
        false
    }
}

/// Allowed Unix domain sockets accessible within a sandboxed environment.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkUnixSocketPermissions {
    pub allowed_paths: HashSet<String>,
}

impl NetworkUnixSocketPermissions {
    pub fn new() -> Self {
        Self {
            allowed_paths: HashSet::new(),
        }
    }

    pub fn allow_socket(&mut self, path: impl Into<String>) {
        self.allowed_paths.insert(path.into());
    }

    pub fn is_allowed(&self, path: &str) -> bool {
        self.allowed_paths.contains(path)
    }
}

/// Selected workspace environment profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SelectedEnvironment {
    pub detected_toolchains: Vec<ToolchainInfo>,
    pub domain_permissions: NetworkDomainPermissions,
    pub socket_permissions: NetworkUnixSocketPermissions,
    pub recommended_sandbox_profile: String,
}

/// Probe a directory to detect active toolchains.
pub fn detect_workspace_toolchains(workspace_root: &Path) -> Vec<ToolchainInfo> {
    let mut toolchains = Vec::new();

    if workspace_root.join("Cargo.toml").exists() {
        toolchains.push(ToolchainInfo {
            kind: ToolchainKind::Rust,
            primary_binary: "cargo".to_string(),
            version: None,
            package_manager: Some("cargo".to_string()),
        });
    }

    if workspace_root.join("package.json").exists() {
        let pm = if workspace_root.join("pnpm-lock.yaml").exists() {
            "pnpm"
        } else if workspace_root.join("yarn.lock").exists() {
            "yarn"
        } else {
            "npm"
        };
        toolchains.push(ToolchainInfo {
            kind: ToolchainKind::NodeJs,
            primary_binary: "node".to_string(),
            version: None,
            package_manager: Some(pm.to_string()),
        });
    }

    if workspace_root.join("pyproject.toml").exists()
        || workspace_root.join("requirements.txt").exists()
        || workspace_root.join("setup.py").exists()
    {
        toolchains.push(ToolchainInfo {
            kind: ToolchainKind::Python,
            primary_binary: "python3".to_string(),
            version: None,
            package_manager: Some("pip".to_string()),
        });
    }

    if workspace_root.join("go.mod").exists() {
        toolchains.push(ToolchainInfo {
            kind: ToolchainKind::Go,
            primary_binary: "go".to_string(),
            version: None,
            package_manager: Some("go".to_string()),
        });
    }

    if workspace_root.join("Dockerfile").exists()
        || workspace_root.join("docker-compose.yml").exists()
    {
        toolchains.push(ToolchainInfo {
            kind: ToolchainKind::Docker,
            primary_binary: "docker".to_string(),
            version: None,
            package_manager: None,
        });
    }

    if workspace_root.join("CMakeLists.txt").exists() || workspace_root.join("Makefile").exists() {
        toolchains.push(ToolchainInfo {
            kind: ToolchainKind::CCpp,
            primary_binary: "make".to_string(),
            version: None,
            package_manager: None,
        });
    }

    if toolchains.is_empty() {
        toolchains.push(ToolchainInfo {
            kind: ToolchainKind::Generic,
            primary_binary: "sh".to_string(),
            version: None,
            package_manager: None,
        });
    }

    toolchains
}

/// Selects the complete environment configuration for a workspace.
pub fn select_environment(workspace_root: &Path) -> SelectedEnvironment {
    let toolchains = detect_workspace_toolchains(workspace_root);
    let domain_perms = NetworkDomainPermissions::standard_defaults();
    let mut socket_perms = NetworkUnixSocketPermissions::new();

    // If Docker is present, allow docker socket
    if toolchains.iter().any(|t| t.kind == ToolchainKind::Docker) {
        socket_perms.allow_socket("/var/run/docker.sock");
    }

    // Recommend sandbox profile
    let recommended_sandbox = if toolchains.iter().any(|t| t.kind == ToolchainKind::Docker) {
        "standard"
    } else {
        "workspace-write"
    };

    SelectedEnvironment {
        detected_toolchains: toolchains,
        domain_permissions: domain_perms,
        socket_permissions: socket_perms,
        recommended_sandbox_profile: recommended_sandbox.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::tempdir;

    #[test]
    fn test_network_domain_permissions_wildcards_and_ports() {
        let mut perms = NetworkDomainPermissions::new();
        perms.add_permission("*.nvidia.com", vec![443], "Model API");
        perms.add_permission("localhost", vec![8000, 8080], "Local proxy");

        assert!(perms.is_allowed("api.nvidia.com", 443));
        assert!(perms.is_allowed("integrate.api.nvidia.com", 443));
        assert!(!perms.is_allowed("api.nvidia.com", 80)); // port not allowed
        assert!(!perms.is_allowed("malicious.com", 443));

        assert!(perms.is_allowed("localhost", 8000));
        assert!(perms.is_allowed("localhost", 8080));
        assert!(!perms.is_allowed("localhost", 9000));
    }

    #[test]
    fn test_network_unix_socket_permissions() {
        let mut perms = NetworkUnixSocketPermissions::new();
        perms.allow_socket("/tmp/m31a-daemon.sock");

        assert!(perms.is_allowed("/tmp/m31a-daemon.sock"));
        assert!(!perms.is_allowed("/var/run/docker.sock"));
    }

    #[test]
    fn test_detect_workspace_toolchains_rust_and_node() {
        let dir = tempdir().unwrap();
        File::create(dir.path().join("Cargo.toml")).unwrap();
        File::create(dir.path().join("package.json")).unwrap();
        File::create(dir.path().join("pnpm-lock.yaml")).unwrap();

        let toolchains = detect_workspace_toolchains(dir.path());
        assert_eq!(toolchains.len(), 2);

        let rust = toolchains
            .iter()
            .find(|t| t.kind == ToolchainKind::Rust)
            .unwrap();
        assert_eq!(rust.primary_binary, "cargo");

        let node = toolchains
            .iter()
            .find(|t| t.kind == ToolchainKind::NodeJs)
            .unwrap();
        assert_eq!(node.package_manager.as_deref(), Some("pnpm"));

        let env = select_environment(dir.path());
        assert!(env.domain_permissions.is_allowed("crates.io", 443));
        assert!(env.domain_permissions.is_allowed("registry.npmjs.org", 443));
    }
}

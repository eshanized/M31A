//! Host capability and development toolchain discovery.
//!
//! Discovers host compilers, runtimes, package managers, and development toolchains:
//! - Inventories installed compilers (rustc, gcc, clang, go).
//! - Inventories runtimes (python3, node, bun, deno).
//! - Inventories package managers (cargo, npm, pnpm, yarn, pip, uv).
//! - Probes sandbox backends (Bubblewrap bwrap, seccomp).
//! - Discovers project manifests in workspace (Cargo.toml, package.json, pyproject.toml, go.mod).
//! - Discovers capability files (SKILL.md, .mcp.json, AGENTS.md).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Status and metadata for a discovered developer tool or compiler.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveredTool {
    pub name: String,
    pub is_available: bool,
    pub path: Option<String>,
    pub version: Option<String>,
}

/// Project types detected in a workspace directory.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceProjectSignatures {
    pub is_rust: bool,
    pub is_node: bool,
    pub is_python: bool,
    pub is_go: bool,
    pub is_c_cpp: bool,
    pub detected_manifests: Vec<String>,
}

/// Discovered capability roots (skills, MCP configs, agent instructions).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveredCapabilityFiles {
    pub skill_files: Vec<PathBuf>,
    pub mcp_configs: Vec<PathBuf>,
    pub agents_md_files: Vec<PathBuf>,
}

/// Complete host development capability and toolchain report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostCapabilityReport {
    pub compilers: Vec<DiscoveredTool>,
    pub runtimes: Vec<DiscoveredTool>,
    pub package_managers: Vec<DiscoveredTool>,
    pub sandboxing: HashMap<String, bool>,
    pub workspace_signatures: WorkspaceProjectSignatures,
    pub capability_files: DiscoveredCapabilityFiles,
}

impl HostCapabilityReport {
    /// Renders a concise Markdown summary for injection into model prompts or diagnostics.
    pub fn summary(&self) -> String {
        let mut s = String::from("### Host Development Capabilities & Toolchains\n");

        s.push_str("**Available Compilers & Toolchains:**\n");
        for tool in &self.compilers {
            if tool.is_available {
                let ver = tool.version.as_deref().unwrap_or("detected");
                s.push_str(&format!("- `{}`: {}\n", tool.name, ver));
            }
        }

        s.push_str("\n**Available Runtimes & Package Managers:**\n");
        for tool in self.runtimes.iter().chain(self.package_managers.iter()) {
            if tool.is_available {
                let ver = tool.version.as_deref().unwrap_or("detected");
                s.push_str(&format!("- `{}`: {}\n", tool.name, ver));
            }
        }

        s.push_str("\n**Sandbox Backends:**\n");
        for (name, avail) in &self.sandboxing {
            s.push_str(&format!(
                "- {}: {}\n",
                name,
                if *avail { "Available" } else { "Not installed" }
            ));
        }

        s.push_str("\n**Detected Project Stacks:** ");
        let mut stacks = Vec::new();
        if self.workspace_signatures.is_rust {
            stacks.push("Rust (Cargo)");
        }
        if self.workspace_signatures.is_node {
            stacks.push("Node.js / TypeScript");
        }
        if self.workspace_signatures.is_python {
            stacks.push("Python");
        }
        if self.workspace_signatures.is_go {
            stacks.push("Go");
        }
        if self.workspace_signatures.is_c_cpp {
            stacks.push("C/C++");
        }
        if stacks.is_empty() {
            s.push_str("Generic / Undetected\n");
        } else {
            s.push_str(&stacks.join(", "));
            s.push('\n');
        }

        s
    }
}

/// Probes a command binary name and extracts its version string if available.
pub fn probe_binary(name: &str, version_flag: &str) -> DiscoveredTool {
    let which_res = which::which(name);
    let is_available = which_res.is_ok();
    let path = which_res.ok().map(|p| p.to_string_lossy().to_string());

    let version = if is_available {
        std::process::Command::new(name)
            .arg(version_flag)
            .output()
            .ok()
            .and_then(|out| {
                if out.status.success() {
                    let first_line = String::from_utf8_lossy(&out.stdout)
                        .lines()
                        .next()
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    if first_line.is_empty() {
                        String::from_utf8_lossy(&out.stderr)
                            .lines()
                            .next()
                            .map(|s| s.trim().to_string())
                    } else {
                        Some(first_line)
                    }
                } else {
                    None
                }
            })
    } else {
        None
    };

    DiscoveredTool {
        name: name.to_string(),
        is_available,
        path,
        version,
    }
}

/// Discovers workspace project signatures based on root directory manifests.
pub fn inspect_workspace_signatures(workspace_root: &Path) -> WorkspaceProjectSignatures {
    let mut detected_manifests = Vec::new();

    let manifests = [
        ("Cargo.toml", true, false, false, false, false),
        ("package.json", false, true, false, false, false),
        ("tsconfig.json", false, true, false, false, false),
        ("pyproject.toml", false, false, true, false, false),
        ("requirements.txt", false, false, true, false, false),
        ("go.mod", false, false, false, true, false),
        ("CMakeLists.txt", false, false, false, false, true),
        ("Makefile", false, false, false, false, true),
    ];

    let mut is_rust = false;
    let mut is_node = false;
    let mut is_python = false;
    let mut is_go = false;
    let mut is_c_cpp = false;

    for (file_name, r, n, p, g, c) in manifests {
        let path = workspace_root.join(file_name);
        if path.is_file() {
            detected_manifests.push(file_name.to_string());
            is_rust |= r;
            is_node |= n;
            is_python |= p;
            is_go |= g;
            is_c_cpp |= c;
        }
    }

    WorkspaceProjectSignatures {
        is_rust,
        is_node,
        is_python,
        is_go,
        is_c_cpp,
        detected_manifests,
    }
}

/// Scans for capability manifests in workspace.
pub fn discover_capability_files(workspace_root: &Path) -> DiscoveredCapabilityFiles {
    let mut skill_files = Vec::new();
    let mut mcp_configs = Vec::new();
    let mut agents_md_files = Vec::new();

    // Check common locations
    let skill_candidate = workspace_root.join("SKILL.md");
    if skill_candidate.is_file() {
        skill_files.push(skill_candidate);
    }
    let skills_dir = workspace_root.join(".skills");
    if skills_dir.is_dir()
        && let Ok(entries) = std::fs::read_dir(skills_dir)
    {
        for e in entries.flatten() {
            let p = e.path().join("SKILL.md");
            if p.is_file() {
                skill_files.push(p);
            }
        }
    }

    let mcp_json = workspace_root.join(".mcp.json");
    if mcp_json.is_file() {
        mcp_configs.push(mcp_json);
    }

    let agents_md = workspace_root.join("AGENTS.md");
    if agents_md.is_file() {
        agents_md_files.push(agents_md);
    }

    DiscoveredCapabilityFiles {
        skill_files,
        mcp_configs,
        agents_md_files,
    }
}

/// Executes full capability and toolchain discovery for the host environment and workspace.
pub fn discover_host_capabilities(workspace_root: &Path) -> HostCapabilityReport {
    let compilers = vec![
        probe_binary("rustc", "--version"),
        probe_binary("gcc", "--version"),
        probe_binary("clang", "--version"),
        probe_binary("go", "version"),
    ];

    let runtimes = vec![
        probe_binary("python3", "--version"),
        probe_binary("node", "--version"),
        probe_binary("bun", "--version"),
        probe_binary("deno", "--version"),
    ];

    let package_managers = vec![
        probe_binary("cargo", "--version"),
        probe_binary("npm", "--version"),
        probe_binary("pnpm", "--version"),
        probe_binary("yarn", "--version"),
        probe_binary("pip", "--version"),
        probe_binary("uv", "--version"),
        probe_binary("git", "--version"),
    ];

    let mut sandboxing = HashMap::new();
    sandboxing.insert(
        "bubblewrap_bwrap".to_string(),
        which::which("bwrap").is_ok(),
    );
    sandboxing.insert("docker".to_string(), which::which("docker").is_ok());
    sandboxing.insert("podman".to_string(), which::which("podman").is_ok());

    #[cfg(target_os = "linux")]
    sandboxing.insert("linux_seccomp".to_string(), true);

    let workspace_signatures = inspect_workspace_signatures(workspace_root);
    let capability_files = discover_capability_files(workspace_root);

    HostCapabilityReport {
        compilers,
        runtimes,
        package_managers,
        sandboxing,
        workspace_signatures,
        capability_files,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_inspect_workspace_signatures_rust() {
        let temp = tempdir().unwrap();
        let cargo_toml = temp.path().join("Cargo.toml");
        std::fs::write(&cargo_toml, "[package]\nname = \"test\"").unwrap();

        let sigs = inspect_workspace_signatures(temp.path());
        assert!(sigs.is_rust);
        assert!(!sigs.is_python);
        assert_eq!(sigs.detected_manifests, vec!["Cargo.toml".to_string()]);
    }

    #[test]
    fn test_discover_capability_files() {
        let temp = tempdir().unwrap();
        let skill_path = temp.path().join("SKILL.md");
        let agents_path = temp.path().join("AGENTS.md");
        std::fs::write(&skill_path, "---\nname: test\n---").unwrap();
        std::fs::write(&agents_path, "# Agents config").unwrap();

        let caps = discover_capability_files(temp.path());
        assert_eq!(caps.skill_files, vec![skill_path]);
        assert_eq!(caps.agents_md_files, vec![agents_path]);
    }

    #[test]
    fn test_probe_binary_fallback() {
        let tool = probe_binary("non_existent_binary_xyz_123", "--version");
        assert!(!tool.is_available);
        assert!(tool.path.is_none());
        assert!(tool.version.is_none());
    }

    #[test]
    fn test_host_capability_report_summary() {
        let report = HostCapabilityReport {
            compilers: vec![DiscoveredTool {
                name: "rustc".to_string(),
                is_available: true,
                path: Some("/usr/bin/rustc".to_string()),
                version: Some("rustc 1.85.0".to_string()),
            }],
            runtimes: vec![],
            package_managers: vec![],
            sandboxing: HashMap::new(),
            workspace_signatures: WorkspaceProjectSignatures {
                is_rust: true,
                is_node: false,
                is_python: false,
                is_go: false,
                is_c_cpp: false,
                detected_manifests: vec!["Cargo.toml".to_string()],
            },
            capability_files: DiscoveredCapabilityFiles {
                skill_files: vec![],
                mcp_configs: vec![],
                agents_md_files: vec![],
            },
        };

        let summary = report.summary();
        assert!(summary.contains("rustc 1.85.0"));
        assert!(summary.contains("Rust (Cargo)"));
    }
}

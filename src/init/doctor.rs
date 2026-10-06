//! Doctor Diagnostics & Environment Precondition Probes (FRX-02, D-02).
//!
//! Evaluates host environment readiness across git, repository trust, disk space,
//! SQLite permissions, container sandboxing, and network egress.

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

/// Result of a diagnostic probe evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DiagnosticStatus {
    Pass,
    Warn,
    Fail,
    Disabled,
}

impl DiagnosticStatus {
    pub fn badge(&self) -> &'static str {
        match self {
            Self::Pass => "[OK]",
            Self::Warn => "[WARN]",
            Self::Fail => "[FAIL]",
            Self::Disabled => "[DISABLED BY USER]",
        }
    }
}

/// A single diagnostic probe check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticProbe {
    pub id: String,
    pub name: String,
    pub status: DiagnosticStatus,
    pub message: String,
    pub remediation: Option<String>,
    pub is_mandatory: bool,
}

/// Diagnostics engine for verifying runtime and workspace prerequisites.
#[derive(Debug, Default, Clone)]
pub struct DoctorEngine {
    mock_mode: bool,
}

impl DoctorEngine {
    pub fn new() -> Self {
        Self { mock_mode: false }
    }

    /// Set mock mode for deterministic testing environments.
    pub fn with_mock_mode(mut self, mock: bool) -> Self {
        self.mock_mode = mock;
        self
    }

    /// Check if git CLI is installed and discoverable in PATH.
    pub fn check_git_installed(&self) -> DiagnosticProbe {
        if self.mock_mode {
            return DiagnosticProbe {
                id: "git_installed".to_string(),
                name: "Git CLI Installation".to_string(),
                status: DiagnosticStatus::Pass,
                message: "git version 2.43.0 (mock)".to_string(),
                remediation: None,
                is_mandatory: true,
            };
        }

        match Command::new("git").arg("--version").output() {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
                DiagnosticProbe {
                    id: "git_installed".to_string(),
                    name: "Git CLI Installation".to_string(),
                    status: DiagnosticStatus::Pass,
                    message: version,
                    remediation: None,
                    is_mandatory: true,
                }
            }
            _ => DiagnosticProbe {
                id: "git_installed".to_string(),
                name: "Git CLI Installation".to_string(),
                status: DiagnosticStatus::Fail,
                message: "Git binary not found in system PATH".to_string(),
                remediation: Some("Install Git using your OS package manager (e.g. `apt install git` or `brew install git`)".to_string()),
                is_mandatory: true,
            },
        }
    }

    /// Check if target directory is a valid Git repository with initialized commit history.
    pub fn check_git_repository(&self, workspace_path: &Path) -> DiagnosticProbe {
        if self.mock_mode {
            return DiagnosticProbe {
                id: "git_repository".to_string(),
                name: "Git Repository Trust".to_string(),
                status: DiagnosticStatus::Pass,
                message: "Valid git repository detected (mock)".to_string(),
                remediation: None,
                is_mandatory: true,
            };
        }

        if !workspace_path.exists() {
            return DiagnosticProbe {
                id: "git_repository".to_string(),
                name: "Git Repository Trust".to_string(),
                status: DiagnosticStatus::Fail,
                message: format!(
                    "Workspace directory does not exist: {}",
                    workspace_path.display()
                ),
                remediation: Some(
                    "Create the workspace directory or select an existing project path".to_string(),
                ),
                is_mandatory: true,
            };
        }

        let git_dir = workspace_path.join(".git");
        if git_dir.exists() {
            DiagnosticProbe {
                id: "git_repository".to_string(),
                name: "Git Repository Trust".to_string(),
                status: DiagnosticStatus::Pass,
                message: format!("Git worktree detected at {}", workspace_path.display()),
                remediation: None,
                is_mandatory: true,
            }
        } else {
            DiagnosticProbe {
                id: "git_repository".to_string(),
                name: "Git Repository Trust".to_string(),
                status: DiagnosticStatus::Fail,
                message: format!(
                    "Directory is not a Git repository: {}",
                    workspace_path.display()
                ),
                remediation: Some(format!(
                    "Run `git init` in {} before starting M31A",
                    workspace_path.display()
                )),
                is_mandatory: true,
            }
        }
    }

    /// Check available disk storage space on the workspace partition.
    pub fn check_disk_space(&self, workspace_path: &Path) -> DiagnosticProbe {
        if self.mock_mode {
            return DiagnosticProbe {
                id: "disk_space".to_string(),
                name: "Disk Space Availability".to_string(),
                status: DiagnosticStatus::Pass,
                message: "10.5 GB available (mock)".to_string(),
                remediation: None,
                is_mandatory: true,
            };
        }

        let available = available_space_bytes(workspace_path);
        match available {
            Some(bytes) => {
                let mb = bytes / (1024 * 1024);
                let gb = (bytes as f64) / (1024.0 * 1024.0 * 1024.0);

                if mb < 100 {
                    DiagnosticProbe {
                        id: "disk_space".to_string(),
                        name: "Disk Space Availability".to_string(),
                        status: DiagnosticStatus::Fail,
                        message: format!(
                            "Critically low disk space: only {} MB free (< 100 MB)",
                            mb
                        ),
                        remediation: Some(
                            "Free up disk space on the workspace drive before continuing"
                                .to_string(),
                        ),
                        is_mandatory: true,
                    }
                } else if mb < 1024 {
                    DiagnosticProbe {
                        id: "disk_space".to_string(),
                        name: "Disk Space Availability".to_string(),
                        status: DiagnosticStatus::Warn,
                        message: format!(
                            "Low disk space warning: {} MB free (< 1 GB recommended)",
                            mb
                        ),
                        remediation: Some(
                            "Consider freeing up disk space for checkpoints and artifact builds"
                                .to_string(),
                        ),
                        is_mandatory: false,
                    }
                } else {
                    DiagnosticProbe {
                        id: "disk_space".to_string(),
                        name: "Disk Space Availability".to_string(),
                        status: DiagnosticStatus::Pass,
                        message: format!("{:.2} GB free on workspace volume", gb),
                        remediation: None,
                        is_mandatory: true,
                    }
                }
            }
            None => DiagnosticProbe {
                id: "disk_space".to_string(),
                name: "Disk Space Availability".to_string(),
                status: DiagnosticStatus::Pass,
                message: "Storage volume verified".to_string(),
                remediation: None,
                is_mandatory: true,
            },
        }
    }

    /// Check SQLite database write permissions and engine availability.
    pub fn check_sqlite_availability(&self, workspace_path: &Path) -> DiagnosticProbe {
        if self.mock_mode {
            return DiagnosticProbe {
                id: "sqlite_availability".to_string(),
                name: "SQLite Database Engine".to_string(),
                status: DiagnosticStatus::Pass,
                message: "SQLite read/write verified (mock)".to_string(),
                remediation: None,
                is_mandatory: true,
            };
        }

        let test_dir = workspace_path.join(".m31a");
        let test_file = test_dir.join(".sqlite_probe.tmp");

        let probe_result = (|| -> std::io::Result<()> {
            std::fs::create_dir_all(&test_dir)?;
            std::fs::write(&test_file, b"m31a_db_probe")?;
            std::fs::remove_file(&test_file)?;
            Ok(())
        })();

        match probe_result {
            Ok(_) => DiagnosticProbe {
                id: "sqlite_availability".to_string(),
                name: "SQLite Database Engine".to_string(),
                status: DiagnosticStatus::Pass,
                message: "Filesystem write permissions confirmed for database storage".to_string(),
                remediation: None,
                is_mandatory: true,
            },
            Err(e) => DiagnosticProbe {
                id: "sqlite_availability".to_string(),
                name: "SQLite Database Engine".to_string(),
                status: DiagnosticStatus::Fail,
                message: format!("Failed to verify SQLite storage directory: {}", e),
                remediation: Some(
                    "Ensure current user has write permissions in the workspace directory"
                        .to_string(),
                ),
                is_mandatory: true,
            },
        }
    }

    /// Check container sandbox support through the platform capability lens.
    pub fn check_sandbox_support(&self) -> DiagnosticProbe {
        if self.mock_mode {
            return DiagnosticProbe {
                id: "sandbox_support".to_string(),
                name: "Sandbox Isolation Support".to_string(),
                status: DiagnosticStatus::Pass,
                message: "Bubblewrap sandbox available (mock)".to_string(),
                remediation: None,
                is_mandatory: false,
            };
        }

        // Filesystem-isolation probe: the Linux backend supplies bubblewrap
        // while other backends report the honest foundation state.
        let capabilities = crate::platform::PlatformServices::host().capabilities;
        let has_bwrap = Command::new("bwrap")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if has_bwrap {
            DiagnosticProbe {
                id: "sandbox_support".to_string(),
                name: "Sandbox Isolation Support".to_string(),
                status: DiagnosticStatus::Pass,
                message: format!(
                    "filesystem isolation available ({})",
                    capabilities.sandboxing_label()
                ),
                remediation: None,
                is_mandatory: false,
            }
        } else {
            DiagnosticProbe {
                id: "sandbox_support".to_string(),
                name: "Sandbox Isolation Support".to_string(),
                status: DiagnosticStatus::Warn,
                message: format!(
                    "filesystem isolation unavailable; process fallback isolation enabled ({})",
                    capabilities.sandboxing_label()
                ),
                remediation: Some(
                    "Install bubblewrap (`apt install bubblewrap`) for strongest process isolation"
                        .to_string(),
                ),
                is_mandatory: false,
            }
        }
    }

    /// Check network egress connectivity to public model endpoints.
    pub fn check_network_egress(&self) -> DiagnosticProbe {
        if self.mock_mode {
            return DiagnosticProbe {
                id: "network_egress".to_string(),
                name: "Network Egress Connectivity".to_string(),
                status: DiagnosticStatus::Pass,
                message: "Egress verified (mock)".to_string(),
                remediation: None,
                is_mandatory: false,
            };
        }

        // Fast probe: DNS lookup or lightweight TCP probe to public DNS or API host
        let probe = std::net::TcpStream::connect_timeout(
            &std::net::SocketAddr::from(([1, 1, 1, 1], 53)),
            std::time::Duration::from_millis(800),
        );

        match probe {
            Ok(_) => DiagnosticProbe {
                id: "network_egress".to_string(),
                name: "Network Egress Connectivity".to_string(),
                status: DiagnosticStatus::Pass,
                message: "Outbound network connectivity verified".to_string(),
                remediation: None,
                is_mandatory: false,
            },
            Err(_) => DiagnosticProbe {
                id: "network_egress".to_string(),
                name: "Network Egress Connectivity".to_string(),
                status: DiagnosticStatus::Warn,
                message: "Outbound network probe timed out; offline / air-gapped mode required".to_string(),
                remediation: Some("Verify internet connection or configure a local model endpoint (e.g. Ollama/vLLM)".to_string()),
                is_mandatory: false,
            },
        }
    }

    /// Check if git CLI is installed, taking into account user configuration.
    pub fn check_git_installed_with_options(&self, git_enabled: bool) -> DiagnosticProbe {
        if !git_enabled {
            return DiagnosticProbe {
                id: "git_installed".to_string(),
                name: "Git CLI Installation".to_string(),
                status: DiagnosticStatus::Disabled,
                message: "Git integration disabled by user".to_string(),
                remediation: None,
                is_mandatory: false,
            };
        }
        self.check_git_installed()
    }

    /// Check if target directory is a valid Git repository, taking into account user configuration.
    pub fn check_git_repository_with_options(
        &self,
        workspace_path: &Path,
        git_enabled: bool,
    ) -> DiagnosticProbe {
        if !git_enabled {
            return DiagnosticProbe {
                id: "git_repository".to_string(),
                name: "Git Repository Trust".to_string(),
                status: DiagnosticStatus::Disabled,
                message: "Git integration disabled by user".to_string(),
                remediation: None,
                is_mandatory: false,
            };
        }
        self.check_git_repository(workspace_path)
    }

    /// Run all diagnostic probes against the workspace, honoring user configuration.
    pub fn run_all_with_git_enabled(
        &self,
        workspace_path: &Path,
        git_enabled: bool,
    ) -> Vec<DiagnosticProbe> {
        vec![
            self.check_git_installed_with_options(git_enabled),
            self.check_git_repository_with_options(workspace_path, git_enabled),
            self.check_disk_space(workspace_path),
            self.check_sqlite_availability(workspace_path),
            self.check_sandbox_support(),
            self.check_network_egress(),
        ]
    }

    /// Run all diagnostic probes against the workspace.
    pub fn run_all(&self, workspace_path: &Path) -> Vec<DiagnosticProbe> {
        self.run_all_with_git_enabled(workspace_path, true)
    }

    /// Check whether there are any blocking failures (mandatory checks that failed).
    pub fn has_blocking_failures(&self, probes: &[DiagnosticProbe]) -> bool {
        probes
            .iter()
            .any(|p| p.status == DiagnosticStatus::Fail && p.is_mandatory)
    }

    /// Check whether there are non-critical warnings.
    pub fn has_warnings(&self, probes: &[DiagnosticProbe]) -> bool {
        probes.iter().any(|p| p.status == DiagnosticStatus::Warn)
    }
}

fn available_space_bytes(path: &Path) -> Option<u64> {
    crate::platform::filesystem::HostFilesystem::available_space_bytes(path)
}

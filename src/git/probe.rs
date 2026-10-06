//! Git workspace status probe (GST-01, FRX-02).

use std::path::Path;
use std::process::Command;

/// Detailed git workspace status inspected during startup and onboarding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitWorkspaceInfo {
    pub is_git_repo: bool,
    pub branch: Option<String>,
    pub is_clean: bool,
    pub modified_count: usize,
}

impl GitWorkspaceInfo {
    pub fn probe(workspace_path: &Path) -> Self {
        let git_dir = workspace_path.join(".git");
        if !git_dir.exists() {
            return Self {
                is_git_repo: false,
                branch: None,
                is_clean: true,
                modified_count: 0,
            };
        }

        let branch = Command::new("git")
            .arg("-C")
            .arg(workspace_path)
            .args(["branch", "--show-current"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|b| !b.is_empty())
            .or_else(|| Some("detached HEAD".to_string()));

        let (is_clean, modified_count) = match Command::new("git")
            .arg("-C")
            .arg(workspace_path)
            .args(["status", "--porcelain"])
            .output()
        {
            Ok(o) if o.status.success() => {
                let lines: Vec<&str> = std::str::from_utf8(&o.stdout)
                    .unwrap_or_default()
                    .lines()
                    .filter(|l| !l.trim().is_empty())
                    .collect();
                (lines.is_empty(), lines.len())
            }
            _ => (true, 0),
        };

        Self {
            is_git_repo: true,
            branch,
            is_clean,
            modified_count,
        }
    }
}

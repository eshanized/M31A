//! Fine-grained filesystem permission profiles and sandbox mount transformations.
//! Translates high-level sandbox policies into concrete filesystem mount permissions and access controls.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Access level for a filesystem path within the sandbox.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileSystemAccessMode {
    ReadOnly,
    ReadWrite,
    Deny,
}

/// An entry defining permissions for a specific path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileSystemSandboxEntry {
    pub path: PathBuf,
    pub access: FileSystemAccessMode,
}

impl FileSystemSandboxEntry {
    pub fn new(path: impl Into<PathBuf>, access: FileSystemAccessMode) -> Self {
        Self {
            path: path.into(),
            access,
        }
    }
}

/// Comprehensive filesystem permission profile.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileSystemPermissions {
    pub entries: Vec<FileSystemSandboxEntry>,
    pub glob_scan_max_depth: Option<usize>,
}

impl FileSystemPermissions {
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn add_entry(&mut self, path: impl Into<PathBuf>, access: FileSystemAccessMode) {
        let entry = FileSystemSandboxEntry::new(path, access);
        if !self.entries.contains(&entry) {
            self.entries.push(entry);
        }
    }
}

/// Effective mount configuration translated for sandbox executors (e.g. bubblewrap).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EffectiveMountPlan {
    pub ro_binds: Vec<PathBuf>,
    pub rw_binds: Vec<PathBuf>,
    pub masked_paths: Vec<PathBuf>,
}

/// Computes the effective mount plan from configured filesystem permissions.
pub fn build_effective_mount_plan(
    permissions: &FileSystemPermissions,
    _cwd: &Path,
) -> Result<EffectiveMountPlan, String> {
    let mut plan = EffectiveMountPlan::default();

    for entry in &permissions.entries {
        if !entry.path.is_absolute() {
            return Err(format!(
                "sandbox permission path '{}' must be absolute",
                entry.path.display()
            ));
        }

        match entry.access {
            FileSystemAccessMode::ReadOnly => {
                if !plan.ro_binds.contains(&entry.path) {
                    plan.ro_binds.push(entry.path.clone());
                }
            }
            FileSystemAccessMode::ReadWrite => {
                if !plan.rw_binds.contains(&entry.path) {
                    plan.rw_binds.push(entry.path.clone());
                }
            }
            FileSystemAccessMode::Deny => {
                if !plan.masked_paths.contains(&entry.path) {
                    plan.masked_paths.push(entry.path.clone());
                }
            }
        }
    }

    // Deny masks take precedence over binds
    plan.ro_binds.retain(|p| !plan.masked_paths.contains(p));
    plan.rw_binds.retain(|p| !plan.masked_paths.contains(p));

    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mount_plan_derivation() {
        let mut perms = FileSystemPermissions::default();
        perms.add_entry("/usr", FileSystemAccessMode::ReadOnly);
        perms.add_entry("/home/user/workspace", FileSystemAccessMode::ReadWrite);
        perms.add_entry("/home/user/.ssh", FileSystemAccessMode::Deny);

        let plan = build_effective_mount_plan(&perms, Path::new("/home/user/workspace"))
            .expect("valid plan");

        assert_eq!(plan.ro_binds, vec![PathBuf::from("/usr")]);
        assert_eq!(plan.rw_binds, vec![PathBuf::from("/home/user/workspace")]);
        assert_eq!(plan.masked_paths, vec![PathBuf::from("/home/user/.ssh")]);
    }

    #[test]
    fn test_deny_overrides_read_write() {
        let mut perms = FileSystemPermissions::default();
        perms.add_entry("/etc/secret", FileSystemAccessMode::ReadWrite);
        perms.add_entry("/etc/secret", FileSystemAccessMode::Deny);

        let plan = build_effective_mount_plan(&perms, Path::new("/")).expect("valid plan");
        assert!(plan.rw_binds.is_empty());
        assert_eq!(plan.masked_paths, vec![PathBuf::from("/etc/secret")]);
    }

    #[test]
    fn test_relative_path_rejected() {
        let mut perms = FileSystemPermissions::default();
        perms.add_entry("relative/path", FileSystemAccessMode::ReadOnly);

        let err = build_effective_mount_plan(&perms, Path::new("/")).unwrap_err();
        assert!(err.contains("must be absolute"));
    }
}

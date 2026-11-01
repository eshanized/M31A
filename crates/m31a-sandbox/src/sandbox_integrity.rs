//! Containment Dependency and Sandbox Binary Integrity Validator.
//!
//! Verifies that sandbox containment dependencies (such as the sandbox launcher,
//! helper binaries, shell interpreters, and pinned file descriptors) cannot be modified
//! or overwritten from within sandbox write roots or symlink aliases.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::policy_transforms::EffectiveMountPlan;

/// Diagnostic finding when a sandbox containment dependency is at risk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrityFinding {
    pub target: PathBuf,
    pub write_roots: Vec<PathBuf>,
    pub description: String,
}

/// Evaluates containment dependencies against effective mount plans.
#[derive(Debug, Clone)]
pub struct SandboxIntegrityChecker {
    ro_binds: Vec<PathBuf>,
    rw_binds: Vec<PathBuf>,
    masked_paths: Vec<PathBuf>,
}

impl SandboxIntegrityChecker {
    pub fn new(mount_plan: &EffectiveMountPlan) -> Self {
        Self {
            ro_binds: mount_plan.ro_binds.clone(),
            rw_binds: mount_plan.rw_binds.clone(),
            masked_paths: mount_plan.masked_paths.clone(),
        }
    }

    pub fn from_roots(
        rw_binds: Vec<PathBuf>,
        ro_binds: Vec<PathBuf>,
        masked_paths: Vec<PathBuf>,
    ) -> Self {
        Self {
            ro_binds,
            rw_binds,
            masked_paths,
        }
    }

    /// Check if a specific containment binary or dependency target is writable by the sandbox.
    pub fn check_target(&self, target: &Path) -> std::io::Result<Option<IntegrityFinding>> {
        // If file does not exist, nothing to check
        if !target.exists() {
            return Ok(None);
        }

        // Canonicalize path to resolve symlinks
        let resolved = match target.canonicalize() {
            Ok(p) => p,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };

        // Check if denied / masked
        if self.is_path_masked(&resolved) {
            return Ok(None);
        }

        // If explicitly read-only, it is protected
        if self.is_path_read_only(&resolved) {
            return Ok(None);
        }

        // Check if path is covered by any writable mount
        let matching_write_roots: Vec<PathBuf> = self
            .rw_binds
            .iter()
            .filter(|rw| resolved.starts_with(rw))
            .cloned()
            .collect();

        if !matching_write_roots.is_empty() {
            return Ok(Some(IntegrityFinding {
                target: target.to_path_buf(),
                write_roots: matching_write_roots,
                description: format!(
                    "Containment dependency '{}' is located inside sandbox writable root(s) and could be overwritten",
                    target.display()
                ),
            }));
        }

        // Also check if the binary on host is world-writable or group-writable
        if let Ok(meta) = fs::metadata(&resolved) {
            let mode = meta.permissions().mode();
            if mode & 0o002 != 0 {
                return Ok(Some(IntegrityFinding {
                    target: target.to_path_buf(),
                    write_roots: Vec::new(),
                    description: format!(
                        "Containment dependency '{}' is world-writable (mode {:o}) on host",
                        target.display(),
                        mode & 0o777
                    ),
                }));
            }
        }

        Ok(None)
    }

    /// Check an inventory of containment dependencies. Returns all findings.
    pub fn check_containment_dependencies(
        &self,
        dependencies: &[PathBuf],
    ) -> std::io::Result<Vec<IntegrityFinding>> {
        let mut findings = Vec::new();
        for dep in dependencies {
            if let Some(finding) = self.check_target(dep)? {
                findings.push(finding);
            }
        }
        Ok(findings)
    }

    fn is_path_masked(&self, path: &Path) -> bool {
        self.masked_paths.iter().any(|mask| path.starts_with(mask))
    }

    fn is_path_read_only(&self, path: &Path) -> bool {
        self.ro_binds.iter().any(|ro| path.starts_with(ro))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_clean_containment_dependency_under_ro_root() {
        let dir = tempdir().unwrap();
        let bin_path = dir.path().join("safe_bin");
        fs::write(&bin_path, b"#!/bin/sh\necho safe\n").unwrap();

        let plan = EffectiveMountPlan {
            ro_binds: vec![dir.path().to_path_buf()],
            rw_binds: vec![PathBuf::from("/tmp/workspace")],
            masked_paths: Vec::new(),
        };

        let checker = SandboxIntegrityChecker::new(&plan);
        let finding = checker.check_target(&bin_path).unwrap();
        assert!(finding.is_none());
    }

    #[test]
    fn test_vulnerable_containment_dependency_inside_rw_root() {
        let dir = tempdir().unwrap();
        let bin_path = dir.path().join("helper_bin");
        fs::write(&bin_path, b"#!/bin/sh\necho helper\n").unwrap();

        let plan = EffectiveMountPlan {
            ro_binds: vec![PathBuf::from("/usr")],
            rw_binds: vec![dir.path().to_path_buf()],
            masked_paths: Vec::new(),
        };

        let checker = SandboxIntegrityChecker::new(&plan);
        let finding = checker.check_target(&bin_path).unwrap();
        assert!(finding.is_some());
        let f = finding.unwrap();
        assert_eq!(f.target, bin_path);
        assert!(f.description.contains("inside sandbox writable root"));
    }
}

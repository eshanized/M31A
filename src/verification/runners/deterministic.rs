//! Tier 1 Deterministic Verification Runner (VER-01).
//!
//! Validates manifest file existence (`Cargo.toml`), workspace structure (`src/`),
//! and deterministic format/syntax invariants without spending compiler or model tokens.

use async_trait::async_trait;
use std::path::Path;

use super::VerificationRunner;
use crate::ids::{MissionId, TaskId};
use crate::verification::types::{CheckTier, VerificationCheck};

#[derive(Debug, Clone)]
pub struct DeterministicRunner {
    pub manifest_file: String,
    pub source_dir: Option<String>,
}

impl Default for DeterministicRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl DeterministicRunner {
    pub fn new() -> Self {
        Self {
            manifest_file: "Cargo.toml".to_string(),
            source_dir: Some("src".to_string()),
        }
    }

    pub fn with_manifest(manifest: impl Into<String>) -> Self {
        let manifest_str = manifest.into();
        let source_dir = if manifest_str == "Cargo.toml" {
            Some("src".to_string())
        } else {
            None
        };
        Self {
            manifest_file: manifest_str,
            source_dir,
        }
    }

    pub fn with_manifest_and_source(
        manifest: impl Into<String>,
        source_dir: Option<String>,
    ) -> Self {
        Self {
            manifest_file: manifest.into(),
            source_dir,
        }
    }
}

#[async_trait]
impl VerificationRunner for DeterministicRunner {
    async fn execute(
        &self,
        mission_id: MissionId,
        task_id: TaskId,
        workspace_root: &Path,
        snapshot_hash: &str,
    ) -> Result<VerificationCheck, String> {
        let mut errors = Vec::new();

        if !self.manifest_file.is_empty() {
            let manifest_path = workspace_root.join(&self.manifest_file);
            if !manifest_path.exists() {
                errors.push(format!("Missing {} manifest", self.manifest_file));
            } else if let Ok(meta) = tokio::fs::metadata(&manifest_path).await
                && meta.len() == 0
            {
                errors.push(format!("{} is empty (0 bytes)", self.manifest_file));
            }
        }

        if let Some(ref s_dir) = self.source_dir {
            let src_path = workspace_root.join(s_dir);
            if !src_path.exists() || !src_path.is_dir() {
                errors.push(format!("Missing '{}/' source directory", s_dir));
            }
        }

        if errors.is_empty() {
            Ok(VerificationCheck::passed(
                mission_id,
                task_id,
                CheckTier::Deterministic,
                "deterministic_structure_check",
                format!("workspace_root={}", workspace_root.display()),
                None,
                "Workspace file structure and manifest validated successfully",
                snapshot_hash,
            ))
        } else {
            let summary = format!("Deterministic checks failed: {}", errors.join("; "));
            Ok(VerificationCheck::failed(
                mission_id,
                task_id,
                CheckTier::Deterministic,
                "deterministic_structure_check",
                format!("workspace_root={}", workspace_root.display()),
                None,
                summary,
                Some("Environment".to_string()),
                snapshot_hash,
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_deterministic_runner_success() {
        let dir = tempdir().unwrap();
        let cargo_toml = dir.path().join("Cargo.toml");
        tokio::fs::write(
            &cargo_toml,
            b"[package]\nname = \"test\"\nversion = \"0.1.0\"\n",
        )
        .await
        .unwrap();
        tokio::fs::create_dir(dir.path().join("src")).await.unwrap();

        let runner = DeterministicRunner::new();
        let mid = MissionId::new();
        let tid = TaskId::new();

        let check = runner
            .execute(mid, tid, dir.path(), "hash-1")
            .await
            .unwrap();
        assert!(check.status.is_passed());
        assert_eq!(check.tier, CheckTier::Deterministic);
    }

    #[tokio::test]
    async fn test_deterministic_runner_missing_manifest() {
        let dir = tempdir().unwrap();
        let runner = DeterministicRunner::new();
        let mid = MissionId::new();
        let tid = TaskId::new();

        let check = runner
            .execute(mid, tid, dir.path(), "hash-1")
            .await
            .unwrap();
        assert!(check.status.is_failed());
        assert!(check.summary.contains("Missing Cargo.toml"));
    }
}

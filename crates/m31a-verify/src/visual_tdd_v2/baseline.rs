//! Screenshot Baseline Management & Integrity Verification.
//!
//! Enforces immutable, cryptographic baseline management for visual test scenarios.
//! Baselines are indexed by scenario name and viewport configuration, hashed with SHA-256,
//! and require explicit user/guardian approval to update (never auto-overwritten).

use super::multi_res::ViewportConfig;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Status of a baseline entry for a scenario and viewport.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BaselineStatus {
    /// Baseline exists, file matches recorded SHA-256 cryptographic hash
    Valid,
    /// No baseline snapshot has been approved yet
    Missing,
    /// Baseline file on disk does not match the manifest SHA-256 hash (tampering/corruption)
    IntegrityMismatch { expected_sha256: String, actual_sha256: String },
    /// Manifest references a file that does not exist on disk
    FileNotFound { path: PathBuf },
}

/// Metadata record for an approved golden screenshot baseline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineRecord {
    pub scenario_name: String,
    pub viewport_id: String,
    pub width: u32,
    pub height: u32,
    pub device_scale_factor: f64,
    pub sha256_hash: String,
    pub relative_image_path: String,
    pub approved_at: String,
    pub approved_by: String,
}

/// Persistent manifest tracking all approved visual baselines.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BaselineManifest {
    pub version: u32,
    pub records: HashMap<String, BaselineRecord>,
}

impl BaselineManifest {
    pub fn new() -> Self {
        Self {
            version: 1,
            records: HashMap::new(),
        }
    }

    pub fn make_key(scenario_name: &str, viewport_id: &str) -> String {
        format!("{}:{}", scenario_name.replace(' ', "_"), viewport_id)
    }
}

/// Storage manager for visual regression golden baselines.
#[derive(Debug, Clone)]
pub struct BaselineStore {
    root_dir: PathBuf,
}

impl BaselineStore {
    pub const MANIFEST_FILENAME: &'static str = "manifest.json";

    pub fn new(root_dir: impl Into<PathBuf>) -> Self {
        Self {
            root_dir: root_dir.into(),
        }
    }

    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    pub fn manifest_path(&self) -> PathBuf {
        self.root_dir.join(Self::MANIFEST_FILENAME)
    }

    /// Loads the baseline manifest or returns an empty manifest if none exists yet.
    pub fn load_manifest(&self) -> Result<BaselineManifest> {
        let path = self.manifest_path();
        if !path.exists() {
            return Ok(BaselineManifest::new());
        }

        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read baseline manifest from {:?}", path))?;
        let manifest: BaselineManifest = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse baseline manifest JSON at {:?}", path))?;
        Ok(manifest)
    }

    /// Saves the baseline manifest to disk.
    fn save_manifest(&self, manifest: &BaselineManifest) -> Result<()> {
        std::fs::create_dir_all(&self.root_dir)?;
        let path = self.manifest_path();
        let content = serde_json::to_string_pretty(manifest)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// Checks the baseline status for a scenario and viewport.
    pub fn check_baseline_status(
        &self,
        scenario_name: &str,
        viewport: &ViewportConfig,
    ) -> Result<BaselineStatus> {
        let manifest = self.load_manifest()?;
        let key = BaselineManifest::make_key(scenario_name, &viewport.id());

        let record = match manifest.records.get(&key) {
            Some(r) => r,
            None => return Ok(BaselineStatus::Missing),
        };

        let file_path = self.root_dir.join(&record.relative_image_path);
        if !file_path.exists() {
            return Ok(BaselineStatus::FileNotFound { path: file_path });
        }

        let bytes = std::fs::read(&file_path)
            .with_context(|| format!("Failed to read baseline file at {:?}", file_path))?;
        let actual_hash = format!("{:x}", Sha256::digest(&bytes));

        if actual_hash == record.sha256_hash {
            Ok(BaselineStatus::Valid)
        } else {
            Ok(BaselineStatus::IntegrityMismatch {
                expected_sha256: record.sha256_hash.clone(),
                actual_sha256: actual_hash,
            })
        }
    }

    /// Loads the golden baseline image bytes, verifying cryptographic integrity against the manifest.
    pub fn get_baseline_image(
        &self,
        scenario_name: &str,
        viewport: &ViewportConfig,
    ) -> Result<Option<Vec<u8>>> {
        let manifest = self.load_manifest()?;
        let key = BaselineManifest::make_key(scenario_name, &viewport.id());

        let record = match manifest.records.get(&key) {
            Some(r) => r,
            None => return Ok(None),
        };

        let file_path = self.root_dir.join(&record.relative_image_path);
        if !file_path.exists() {
            bail!(
                "Baseline file referenced in manifest does not exist on disk: {:?}",
                file_path
            );
        }

        let bytes = std::fs::read(&file_path)
            .with_context(|| format!("Failed to read baseline image at {:?}", file_path))?;
        let actual_hash = format!("{:x}", Sha256::digest(&bytes));

        if actual_hash != record.sha256_hash {
            bail!(
                "Baseline integrity violation for scenario '{}' [{}]: manifest expects SHA-256 {} but file has {}",
                scenario_name,
                viewport.id(),
                record.sha256_hash,
                actual_hash
            );
        }

        Ok(Some(bytes))
    }

    /// Explicitly approves and commits a new golden baseline.
    ///
    /// NOTE: M31A never auto-overwrites baselines. Baseline modification
    /// must be triggered through this explicit method upon verified approval.
    pub fn approve_baseline(
        &self,
        scenario_name: &str,
        viewport: &ViewportConfig,
        image_bytes: &[u8],
        approved_by: &str,
    ) -> Result<BaselineRecord> {
        let scenario_clean = scenario_name.replace(['/', '\\', ' '], "_");
        let vp_id = viewport.id();
        let rel_filename = format!("{}/{}.png", scenario_clean, vp_id);
        let target_path = self.root_dir.join(&rel_filename);

        if let Some(parent) = target_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(&target_path, image_bytes)?;

        let sha256_hash = format!("{:x}", Sha256::digest(image_bytes));
        let record = BaselineRecord {
            scenario_name: scenario_name.to_string(),
            viewport_id: vp_id.clone(),
            width: viewport.width,
            height: viewport.height,
            device_scale_factor: viewport.device_scale_factor,
            sha256_hash,
            relative_image_path: rel_filename,
            approved_at: chrono::Utc::now().to_rfc3339(),
            approved_by: approved_by.to_string(),
        };

        let mut manifest = self.load_manifest()?;
        let key = BaselineManifest::make_key(scenario_name, &vp_id);
        manifest.records.insert(key, record.clone());
        self.save_manifest(&manifest)?;

        Ok(record)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_baseline_lifecycle_and_integrity_check() {
        let dir = tempdir().unwrap();
        let store = BaselineStore::new(dir.path());
        let vp = ViewportConfig::new("desktop", 1280, 800, 1.0, false);

        // Initially missing
        let status = store.check_baseline_status("login_flow", &vp).unwrap();
        assert_eq!(status, BaselineStatus::Missing);

        // Approve baseline
        let sample_bytes = b"fake-png-data-for-baseline-test";
        let record = store
            .approve_baseline("login_flow", &vp, sample_bytes, "developer@m31a.local")
            .unwrap();
        assert_eq!(record.scenario_name, "login_flow");

        // Now valid
        let status2 = store.check_baseline_status("login_flow", &vp).unwrap();
        assert_eq!(status2, BaselineStatus::Valid);

        let retrieved = store
            .get_baseline_image("login_flow", &vp)
            .unwrap()
            .unwrap();
        assert_eq!(retrieved, sample_bytes);

        // Simulate tampering by editing file on disk directly
        let file_path = dir.path().join(&record.relative_image_path);
        std::fs::write(&file_path, b"tampered-bytes").unwrap();

        let tampered_status = store.check_baseline_status("login_flow", &vp).unwrap();
        match tampered_status {
            BaselineStatus::IntegrityMismatch { .. } => {}
            other => panic!("Expected IntegrityMismatch but got {:?}", other),
        }

        // Fetching tampered image must fail-closed
        let fetch_res = store.get_baseline_image("login_flow", &vp);
        assert!(fetch_res.is_err());
        assert!(fetch_res.unwrap_err().to_string().contains("integrity violation"));
    }
}

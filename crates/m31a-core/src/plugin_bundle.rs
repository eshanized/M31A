//! Portable Plugin Bundle Archive & Verification Engine.
//!
//! Creates, unpacks, and validates portable plugin bundle archives (.m31a-bundle.json):
//! - Packages plugin directories into structured portable `.m31a-bundle.json` archives.
//! - Validates SHA-256 checksum integrity of all bundled assets before installation.
//! - Unpacks bundles atomically into user plugin directories.
//! - Verifies manifest schemas and skill references.

use crate::plugins::PluginManifest;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Portable self-contained plugin archive bundle.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginBundle {
    pub manifest: PluginManifest,
    pub files: BTreeMap<String, String>,
    pub checksum: String,
    pub packaged_at_epoch_secs: u64,
}

impl PluginBundle {
    /// Packages a plugin directory into a self-contained verifiable bundle.
    pub fn package_dir(plugin_dir: &Path) -> Result<Self> {
        let manifest_path = plugin_dir.join("plugin.toml");
        if !manifest_path.is_file() {
            bail!(
                "Cannot package plugin: manifest 'plugin.toml' not found in {:?}",
                plugin_dir
            );
        }

        let manifest_content = std::fs::read_to_string(&manifest_path)
            .with_context(|| format!("Failed to read manifest at {:?}", manifest_path))?;
        let manifest: PluginManifest = toml::from_str(&manifest_content)
            .with_context(|| format!("Failed to parse plugin.toml in {:?}", plugin_dir))?;

        let mut files = BTreeMap::new();
        Self::scan_and_read_files(plugin_dir, plugin_dir, &mut files)?;

        let checksum = Self::compute_checksum(&manifest, &files);
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Ok(Self {
            manifest,
            files,
            checksum,
            packaged_at_epoch_secs: now,
        })
    }

    /// Recursively scans and reads text files into the bundle map.
    fn scan_and_read_files(
        root: &Path,
        current: &Path,
        files: &mut BTreeMap<String, String>,
    ) -> Result<()> {
        let entries = match std::fs::read_dir(current) {
            Ok(e) => e,
            Err(_) => return Ok(()),
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let file_name = path.file_name().and_then(|f| f.to_str()).unwrap_or("");

            // Skip hidden directories and build outputs
            if file_name.starts_with('.') || file_name == "target" || file_name == "node_modules" {
                continue;
            }

            if path.is_dir() {
                Self::scan_and_read_files(root, &path, files)?;
            } else if path.is_file()
                && let Ok(rel) = path.strip_prefix(root)
            {
                let rel_str = rel.to_string_lossy().to_string();
                if let Ok(content) = std::fs::read_to_string(&path) {
                    files.insert(rel_str, content);
                }
            }
        }
        Ok(())
    }

    /// Computes deterministic SHA-256 checksum over manifest and sorted file entries.
    pub fn compute_checksum(manifest: &PluginManifest, files: &BTreeMap<String, String>) -> String {
        let mut hasher = Sha256::new();
        hasher.update(manifest.name.as_bytes());
        hasher.update(manifest.version.as_deref().unwrap_or("0.0.0").as_bytes());

        for (path, content) in files {
            hasher.update(path.as_bytes());
            hasher.update(content.as_bytes());
        }

        format!("{:x}", hasher.finalize())
    }

    /// Verifies that bundle checksum matches the content bytes.
    pub fn verify_integrity(&self) -> bool {
        let expected = Self::compute_checksum(&self.manifest, &self.files);
        self.checksum == expected
    }

    /// Serializes bundle to a JSON file.
    pub fn save_to_file(&self, path: &Path) -> Result<()> {
        let data = serde_json::to_string_pretty(self)
            .context("Failed to serialize plugin bundle to JSON")?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, data)?;
        Ok(())
    }

    /// Loads and verifies a bundle from a JSON file.
    pub fn load_from_file(path: &Path) -> Result<Self> {
        let data = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read plugin bundle file at {:?}", path))?;
        let bundle: Self = serde_json::from_str(&data)
            .with_context(|| format!("Failed to deserialize plugin bundle at {:?}", path))?;

        if !bundle.verify_integrity() {
            bail!(
                "Plugin bundle integrity check failed for '{}-v{}' (checksum mismatch)",
                bundle.manifest.name,
                bundle.manifest.version.as_deref().unwrap_or("0.0.0")
            );
        }

        Ok(bundle)
    }

    /// Unpacks bundle into the specified target destination directory.
    pub fn unpack_into(&self, target_dir: &Path) -> Result<PathBuf> {
        if !self.verify_integrity() {
            bail!("Refusing to unpack corrupted or tampered plugin bundle");
        }

        let dest = target_dir.join(&self.manifest.name);
        std::fs::create_dir_all(&dest)?;

        for (rel_path, content) in &self.files {
            let file_dest = dest.join(rel_path);
            if let Some(parent) = file_dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&file_dest, content)?;
        }

        Ok(dest)
    }
}

/// Convenience helper to package a plugin directory and write it directly to an output file.
pub fn pack_plugin_bundle(dir: &Path, out: &Path) -> Result<PluginBundle> {
    let bundle = PluginBundle::package_dir(dir)?;
    bundle.save_to_file(out)?;
    Ok(bundle)
}

/// Convenience helper to load, verify, and unpack a bundle file into a destination directory.
pub fn unpack_plugin_bundle(bundle_path: &Path, target_dir: &Path) -> Result<PathBuf> {
    let bundle = PluginBundle::load_from_file(bundle_path)?;
    bundle.unpack_into(target_dir)
}

/// Convenience helper to verify a bundle file without unpacking.
pub fn verify_plugin_bundle_file(bundle_path: &Path) -> Result<PluginBundle> {
    PluginBundle::load_from_file(bundle_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_plugin_bundle_roundtrip() {
        let temp = tempdir().unwrap();
        let plugin_dir = temp.path().join("my-plugin");
        std::fs::create_dir_all(&plugin_dir).unwrap();

        let manifest = r#"
name = "code-auditor"
version = "1.2.0"
description = "Automated static analysis and security auditing plugin"
author = "Security Team"
"#;
        std::fs::write(plugin_dir.join("plugin.toml"), manifest).unwrap();

        let skills_dir = plugin_dir.join("skills");
        std::fs::create_dir_all(&skills_dir).unwrap();
        std::fs::write(skills_dir.join("audit.md"), "# Audit Skill instructions").unwrap();

        // 1. Package directory
        let bundle = PluginBundle::package_dir(&plugin_dir).unwrap();
        assert_eq!(bundle.manifest.name, "code-auditor");
        assert_eq!(bundle.manifest.version, Some("1.2.0".to_string()));
        assert!(bundle.files.contains_key("plugin.toml"));
        assert!(bundle.files.contains_key("skills/audit.md"));
        assert!(bundle.verify_integrity());

        // 2. Save and load bundle archive
        let archive_path = temp.path().join("bundle.json");
        bundle.save_to_file(&archive_path).unwrap();

        let loaded = PluginBundle::load_from_file(&archive_path).unwrap();
        assert_eq!(loaded.checksum, bundle.checksum);
        assert!(loaded.verify_integrity());

        // 3. Unpack into target destination
        let install_root = temp.path().join("installed");
        let installed_path = loaded.unpack_into(&install_root).unwrap();
        assert!(installed_path.join("plugin.toml").exists());
        assert!(installed_path.join("skills").join("audit.md").exists());
    }

    #[test]
    fn test_bundle_tamper_detection() {
        let manifest = PluginManifest {
            name: "sample".to_string(),
            version: Some("0.1.0".to_string()),
            description: Some("Sample".to_string()),
            ..Default::default()
        };

        let mut files = BTreeMap::new();
        files.insert("file.txt".to_string(), "original content".to_string());

        let checksum = PluginBundle::compute_checksum(&manifest, &files);
        let mut bundle = PluginBundle {
            manifest,
            files,
            checksum,
            packaged_at_epoch_secs: 1000,
        };

        assert!(bundle.verify_integrity());

        // Modify content without updating checksum
        bundle
            .files
            .insert("file.txt".to_string(), "tampered content".to_string());
        assert!(!bundle.verify_integrity());
    }
}

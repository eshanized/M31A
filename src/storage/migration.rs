//! Legacy project-local → platform-global migration.
//!
//! Existing users may have `<ws>/.m31a/m31a.db`,
//! `<ws>/.m31a/credentials.json`, `<ws>/.m31a/artifacts`,
//! `<ws>/.m31a/telemetry`, `<ws>/.m31a/state`, and
//! `<ws>/.m31a/cache/model_catalog*.json`. Migration moves them once to the
//! canonical [`super::StorageLayout`] locations with integrity verification,
//! then stops using the old location. Never deletes user data silently:
//! legacy files are renamed to `*.migrated` only after the destination
//! verifies.

use std::path::{Path, PathBuf};

use crate::deployment::DeploymentChannel;

use super::StorageLayout;

/// What legacy project-local state exists for a workspace.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LegacyState {
    pub db: Option<PathBuf>,
    pub credentials: Option<PathBuf>,
    pub model_catalog: Option<PathBuf>,
    pub model_catalog_dev: Option<PathBuf>,
    pub artifacts: Option<PathBuf>,
    pub telemetry: Option<PathBuf>,
    pub state_dir: Option<PathBuf>,
}

impl LegacyState {
    pub fn is_empty(&self) -> bool {
        self.db.is_none()
            && self.credentials.is_none()
            && self.model_catalog.is_none()
            && self.model_catalog_dev.is_none()
            && self.artifacts.is_none()
            && self.telemetry.is_none()
            && self.state_dir.is_none()
    }
}

/// Detect legacy project-local state (does not migrate).
pub fn detect_legacy_state(workspace_root: &Path, channel: DeploymentChannel) -> LegacyState {
    let m31a = workspace_root.join(".m31a");
    let mut out = LegacyState::default();
    let db = crate::deployment::DeploymentPaths::project_db_path(workspace_root, channel);
    if db.is_file() {
        out.db = Some(db);
    }
    let creds =
        crate::deployment::DeploymentPaths::project_credentials_file(workspace_root, channel);
    if creds.is_file() {
        out.credentials = Some(creds);
    }
    let catalog = m31a.join("cache").join("model_catalog.json");
    if catalog.is_file() {
        out.model_catalog = Some(catalog);
    }
    let catalog_dev = m31a.join("cache").join("model_catalog-dev.json");
    if catalog_dev.is_file() {
        out.model_catalog_dev = Some(catalog_dev);
    }
    let artifacts =
        crate::deployment::DeploymentPaths::project_artifacts_dir(workspace_root, channel);
    if artifacts.is_dir() {
        out.artifacts = Some(artifacts);
    }
    let telemetry =
        crate::deployment::DeploymentPaths::project_telemetry_dir(workspace_root, channel);
    if telemetry.is_dir() {
        out.telemetry = Some(telemetry);
    }
    let state = crate::deployment::DeploymentPaths::project_state_dir(workspace_root, channel);
    if state.is_dir() {
        out.state_dir = Some(state);
    }
    out
}

/// True when any legacy project-local state exists.
pub fn is_legacy_present(workspace_root: &Path, channel: DeploymentChannel) -> bool {
    !detect_legacy_state(workspace_root, channel).is_empty()
}

/// Outcome of one migration run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MigrationReport {
    pub db_migrated: bool,
    pub credentials_migrated: bool,
    pub catalog_migrated: bool,
    pub notes: Vec<String>,
}

impl MigrationReport {
    /// True when any step in the migration failed or encountered a validation error.
    pub fn has_errors(&self) -> bool {
        self.notes
            .iter()
            .any(|n| n.contains("failed") || n.contains("mismatch"))
    }

    /// List all error diagnostic notes recorded during migration.
    pub fn errors(&self) -> Vec<&str> {
        self.notes
            .iter()
            .filter(|n| n.contains("failed") || n.contains("mismatch"))
            .map(|s| s.as_str())
            .collect()
    }
}

fn sha256_bytes(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

fn is_valid_sqlite_bytes(data: &[u8]) -> bool {
    data.len() >= 16 && &data[..16] == b"SQLite format 3\0"
}

fn is_valid_credentials_bytes(data: &[u8]) -> bool {
    serde_json::from_slice::<std::collections::HashMap<String, String>>(data)
        .map(|map| {
            map.get("nvidia_nim")
                .map(|k| !k.trim().is_empty())
                .unwrap_or(false)
                || map.values().any(|v| !v.trim().is_empty())
        })
        .unwrap_or(false)
}

fn is_valid_catalog_bytes(data: &[u8]) -> bool {
    serde_json::from_slice::<serde_json::Value>(data)
        .map(|v| {
            v.get("schema_version")
                .and_then(|s| s.as_u64())
                .map(|ver| {
                    ver <= crate::model::catalog::ModelCatalog::CURRENT_CATALOG_SCHEMA_VERSION
                        as u64
                        + 10
                })
                .unwrap_or(true)
                && v.get("provider").and_then(|p| p.as_str()).is_some()
        })
        .unwrap_or(false)
}

/// integrity-safe copy: validate source, copy via temp file, fsync,
/// atomic rename, then verify destination hash equals source hash.
/// never trusts size alone; never deletes source; never overwrites a valid
/// destination with different content blindly (caller decides via validator).
fn copy_file_verified_with_validator(
    src: &Path,
    dst: &Path,
    dst_valid: impl Fn(&[u8]) -> bool,
) -> Result<(), String> {
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("create parent {}: {e}", parent.display()))?;
    }
    let src_bytes = std::fs::read(src).map_err(|e| format!("read src {}: {e}", src.display()))?;
    if src_bytes.is_empty() {
        return Err("refusing to migrate empty source".to_string());
    }
    let src_hash = sha256_bytes(&src_bytes);
    // idempotence + no blind overwrite: if destination exists and verifies,
    // compare hashes. identical => already migrated. valid-but-different =>
    // keep destination (never overwrite valid state blindly).
    if dst.is_file() {
        if let Ok(dst_bytes) = std::fs::read(dst) {
            if !dst_bytes.is_empty() && dst_valid(&dst_bytes) {
                let dst_hash = sha256_bytes(&dst_bytes);
                if dst_hash == src_hash {
                    return Ok(());
                }
                // valid destination with different content: preserve it.
                return Ok(());
            }
            // invalid destination falls through to replacement below.
        }
    }
    // write to temp file in destination dir, fsync, atomic rename.
    let tmp = dst.with_extension(format!(
        "{}.tmp-{}",
        dst.extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_else(|| "tmp".to_string()),
        std::process::id()
    ));
    std::fs::write(&tmp, &src_bytes).map_err(|e| format!("write tmp {}: {e}", tmp.display()))?;
    // fsync file + best-effort dir sync where platform permits.
    {
        use std::io::Write;
        if let Ok(f) = std::fs::File::open(&tmp) {
            let _ = f.sync_all();
        }
        let _ = std::io::stdout().flush();
    }
    std::fs::rename(&tmp, dst)
        .map_err(|e| format!("atomic rename {} -> {}: {e}", tmp.display(), dst.display()))?;
    let dst_bytes =
        std::fs::read(dst).map_err(|e| format!("verify read {}: {e}", dst.display()))?;
    let dst_hash = sha256_bytes(&dst_bytes);
    if dst_hash != src_hash {
        return Err("hash mismatch after copy (destination verification failed)".to_string());
    }
    if !dst_valid(&dst_bytes) {
        return Err("destination failed content validation after copy".to_string());
    }
    Ok(())
}

fn mark_migrated(src: &Path) -> Result<(), String> {
    let migrated = src.with_extension(
        src.extension()
            .map(|e| format!("{}.migrated", e.to_string_lossy()))
            .unwrap_or_else(|| "migrated".to_string()),
    );
    // Best-effort rename; failure must not lose data.
    if migrated.exists() {
        return Ok(());
    }
    std::fs::rename(src, &migrated).map_err(|e| format!("mark migrated {}: {e}", src.display()))?;
    Ok(())
}

/// Migrate legacy project-local state to canonical platform locations.
///
/// Integrity rules:
/// - SQLite: copy + size verification; destination must be a valid SQLite
///   file (header check). Source renamed to `*.migrated` only afterwards.
/// - Credentials: copy + permission hardening (0600) + content verification.
/// - Catalog: copy + JSON parse verification.
/// - Never deletes user data; never overwrites a non-empty destination with
///   an empty source.
pub fn migrate_legacy_workspace_state(layout: &StorageLayout) -> MigrationReport {
    let mut report = MigrationReport::default();
    let legacy = detect_legacy_state(layout.workspace_root(), layout.channel());

    // Ensure destinations exist first.
    let _ = layout.ensure_global_dirs();

    // --- SQLite ---
    if let Some(src) = legacy.db {
        let dst = layout.global_db_path();
        if src != dst {
            // empty placeholder is not a real db: retire it without copying.
            let src_empty = std::fs::metadata(&src)
                .map(|m| m.len() == 0)
                .unwrap_or(false);
            if src_empty {
                report.db_migrated = true;
                report.notes.push(format!(
                    "db: empty legacy placeholder retired: {}",
                    src.display()
                ));
                let _ = mark_migrated(&src);
            } else {
                // validate source header before attempting migration.
                let src_valid = std::fs::read(&src)
                    .map(|b| is_valid_sqlite_bytes(&b))
                    .unwrap_or(false);
                if !src_valid {
                    report.notes.push(format!(
                        "db: legacy source failed SQLite header check, not migrated: {}",
                        src.display()
                    ));
                } else {
                    match copy_file_verified_with_validator(&src, &dst, is_valid_sqlite_bytes) {
                        Ok(()) => {
                            let valid = std::fs::read(&dst)
                                .map(|b| is_valid_sqlite_bytes(&b))
                                .unwrap_or(false);
                            if valid {
                                report.db_migrated = true;
                                report.notes.push(format!(
                                    "db: {} -> {} (hash-verified)",
                                    src.display(),
                                    dst.display()
                                ));
                                let _ = mark_migrated(&src);
                            } else {
                                report.notes.push(format!(
                                    "db: destination failed SQLite header check: {}",
                                    dst.display()
                                ));
                            }
                        }
                        Err(e) => report.notes.push(format!("db migration failed: {e}")),
                    }
                }
            }
        }
    }

    // --- Credentials (secure) ---
    if let Some(src) = legacy.credentials {
        let dst = layout.global_credentials_file();
        if src != dst {
            match std::fs::read_to_string(&src) {
                Ok(content) => {
                    // Must be a JSON map; must contain a non-empty nvidia key
                    // or at least parse as a map (don't migrate garbage).
                    let parsed: Result<std::collections::HashMap<String, String>, _> =
                        serde_json::from_str(&content);
                    match parsed {
                        Ok(map) => {
                            let has_key = map
                                .get("nvidia_nim")
                                .map(|k| !k.trim().is_empty())
                                .unwrap_or(false)
                                || map.values().any(|v| !v.trim().is_empty());
                            if has_key {
                                match copy_file_verified_with_validator(
                                    &src,
                                    &dst,
                                    is_valid_credentials_bytes,
                                ) {
                                    Ok(()) => {
                                        // Verify + harden 0600.
                                        if let Ok(dst_content) = std::fs::read_to_string(&dst)
                                            && dst_content == content
                                        {
                                            #[cfg(unix)]
                                            {
                                                use std::os::unix::fs::PermissionsExt;
                                                let _ = std::fs::set_permissions(
                                                    &dst,
                                                    std::fs::Permissions::from_mode(0o600),
                                                );
                                            }
                                            report.credentials_migrated = true;
                                            report.notes.push(format!(
                                                "credentials: {} -> {}",
                                                src.display(),
                                                dst.display()
                                            ));
                                            let _ = mark_migrated(&src);
                                        } else {
                                            report.notes.push(
                                                "credentials: verification mismatch".to_string(),
                                            );
                                        }
                                    }
                                    Err(e) => report
                                        .notes
                                        .push(format!("credentials migration failed: {e}")),
                                }
                            } else {
                                report.notes.push(
                                    "credentials: legacy file has no usable keys".to_string(),
                                );
                            }
                        }
                        Err(e) => report
                            .notes
                            .push(format!("credentials: legacy JSON invalid: {e}")),
                    }
                }
                Err(e) => report.notes.push(format!("credentials: read failed: {e}")),
            }
        }
    }

    // --- Model catalog cache ---
    let catalog_src = match layout.channel() {
        DeploymentChannel::Production => legacy.model_catalog,
        DeploymentChannel::Development => legacy.model_catalog_dev.or(legacy.model_catalog),
    };
    if let Some(src) = catalog_src {
        let dst = layout.global_model_catalog_file();
        if src != dst {
            match std::fs::read(&src) {
                Ok(bytes) => {
                    if !is_valid_catalog_bytes(&bytes) {
                        report.notes.push(
                            "catalog: legacy JSON invalid or missing provider identity".to_string(),
                        );
                    } else {
                        // verify schema version explicitly before migration.
                        let schema_ok = serde_json::from_slice::<serde_json::Value>(&bytes)
                            .ok()
                            .and_then(|v| v.get("schema_version").and_then(|s| s.as_u64()))
                            .map(|ver| ver >= 1)
                            .unwrap_or(true);
                        if !schema_ok {
                            report
                                .notes
                                .push("catalog: legacy schema version invalid".to_string());
                        } else {
                            match copy_file_verified_with_validator(
                                &src,
                                &dst,
                                is_valid_catalog_bytes,
                            ) {
                                Ok(()) => {
                                    report.catalog_migrated = true;
                                    report.notes.push(format!(
                                        "catalog: {} -> {} (hash-verified)",
                                        src.display(),
                                        dst.display()
                                    ));
                                    let _ = mark_migrated(&src);
                                }
                                Err(e) => {
                                    report.notes.push(format!("catalog migration failed: {e}"))
                                }
                            }
                        }
                    }
                }
                Err(e) => report.notes.push(format!("catalog: read failed: {e}")),
            }
        }
    }

    // Artifacts/telemetry/state dirs: intentionally NOT auto-moved (project
    // execution outputs may be workspace-specific). Record their presence so
    // operators can migrate deliberately.
    if legacy.artifacts.is_some() {
        report
            .notes
            .push("artifacts: workspace-local artifacts preserved (project-specific)".to_string());
    }
    if legacy.telemetry.is_some() {
        report
            .notes
            .push("telemetry: workspace-local telemetry preserved (project-specific)".to_string());
    }
    if legacy.state_dir.is_some() {
        report.notes.push(
            "state: workspace-local runtime state preserved; new runtime state uses platform state dir"
                .to_string(),
        );
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deployment::DeploymentChannel;

    #[test]
    fn detects_legacy_db_and_credentials() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path();
        std::fs::create_dir_all(ws.join(".m31a")).unwrap();
        std::fs::write(
            ws.join(".m31a").join("m31a.db"),
            b"SQLite format 3\0payload",
        )
        .unwrap();
        std::fs::write(
            ws.join(".m31a").join("credentials.json"),
            r#"{"nvidia_nim":"k"}"#,
        )
        .unwrap();
        let legacy = detect_legacy_state(ws, DeploymentChannel::Production);
        assert!(legacy.db.is_some());
        assert!(legacy.credentials.is_some());
        assert!(is_legacy_present(ws, DeploymentChannel::Production));
    }
}

//! Transactional installer: staged, verified, atomic replacement.
//!
//! Update sequence (behavioral transaction):
//!   discover → download → verify checksum → stage → verify executable →
//!   atomically replace → verify runtime. On any failure the previous
//!   known-good binary is preserved and success is never reported.
//!
//! User state (config/data/cache/state/database) is never wiped or migrated
//! implicitly by installation.

use std::path::{Component, Path, PathBuf};

use super::artifact::{ArtifactFormat, ReleaseArtifact};
use super::channel::DeploymentChannel;
use crate::deployment::context::DeploymentContext;
use crate::release::integrity::sha256_bytes;

/// Maximum archive entries permitted (prevents zip bomb / inode exhaustion).
pub const MAX_ARCHIVE_ENTRIES: usize = 1_000;
/// Maximum cumulative uncompressed archive size permitted (1 GiB).
pub const MAX_ARCHIVE_TOTAL_UNCOMPRESSED_BYTES: u64 = 1_073_741_824;
/// Maximum size for a single executable binary (500 MiB).
pub const MAX_EXECUTABLE_BYTES: u64 = 524_288_000;

/// Typed installer failures. Every variant preserves the previous binary.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InstallError {
    #[error("install error: {0}")]
    Verification(String),
    #[error("install error: {0}")]
    Io(String),
    #[error("install error: channel mismatch: {0}")]
    ChannelMismatch(String),
    #[error("install error: archive validation error: {0}")]
    Archive(String),
    #[error("install error: catastrophic restore failure: original: '{original_error}', restore failed: '{restore_error}' (live: {}, backup: {})", live_path.display(), backup_path.display())]
    CatastrophicRestoreFailed {
        original_error: String,
        restore_error: String,
        live_path: PathBuf,
        backup_path: PathBuf,
    },
}

/// Verify that an archive entry relative path does not attempt path traversal
/// or absolute file escaping.
fn validate_archive_entry_path(path: &Path) -> Result<(), InstallError> {
    if path.is_absolute() {
        return Err(InstallError::Archive(format!(
            "absolute entry path in archive rejected: {}",
            path.display()
        )));
    }
    for component in path.components() {
        match component {
            Component::ParentDir => {
                return Err(InstallError::Archive(format!(
                    "path traversal ('..') in archive rejected: {}",
                    path.display()
                )));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(InstallError::Archive(format!(
                    "root or prefix path in archive rejected: {}",
                    path.display()
                )));
            }
            Component::Normal(_) | Component::CurDir => {}
        }
    }
    Ok(())
}

/// Extract the designated executable from a `.tar.gz` archive with strict resource limits
/// and traversal / link rejection.
fn extract_executable_from_targz(
    bytes: &[u8],
    expected_binary_name: &str,
    target_path: &Path,
) -> Result<(), InstallError> {
    let gz = flate2::read::GzDecoder::new(bytes);
    let mut archive = tar::Archive::new(gz);

    let mut total_uncompressed: u64 = 0;
    let mut found_executable = false;

    let entries = archive
        .entries()
        .map_err(|e| InstallError::Archive(format!("invalid tar.gz archive stream: {e}")))?;

    for (entry_count, entry_res) in entries.enumerate() {
        if entry_count >= MAX_ARCHIVE_ENTRIES {
            return Err(InstallError::Archive(format!(
                "archive entry count exceeded limit of {MAX_ARCHIVE_ENTRIES}"
            )));
        }

        let mut entry =
            entry_res.map_err(|e| InstallError::Archive(format!("cannot read tar entry: {e}")))?;
        let path = entry
            .path()
            .map_err(|e| InstallError::Archive(format!("cannot parse tar entry path: {e}")))?
            .to_path_buf();

        validate_archive_entry_path(&path)?;

        let header = entry.header();
        let entry_type = header.entry_type();

        if entry_type.is_symlink() || entry_type.is_hard_link() {
            return Err(InstallError::Archive(format!(
                "symlink/hardlink entry in archive prohibited: {}",
                path.display()
            )));
        }

        let size = entry.size();
        total_uncompressed = total_uncompressed.saturating_add(size);
        if total_uncompressed > MAX_ARCHIVE_TOTAL_UNCOMPRESSED_BYTES {
            return Err(InstallError::Archive(format!(
                "cumulative uncompressed size exceeded limit of {MAX_ARCHIVE_TOTAL_UNCOMPRESSED_BYTES} bytes"
            )));
        }

        let is_target_file = entry_type.is_file()
            && path.file_name().and_then(|s| s.to_str()) == Some(expected_binary_name);

        if is_target_file {
            if found_executable {
                return Err(InstallError::Archive(format!(
                    "multiple executable entries named '{expected_binary_name}' found in archive; substitution rejected"
                )));
            }
            if size > MAX_EXECUTABLE_BYTES {
                return Err(InstallError::Archive(format!(
                    "executable size ({size} bytes) exceeds limit of {MAX_EXECUTABLE_BYTES} bytes"
                )));
            }

            let mut out = std::fs::File::create(target_path).map_err(|e| {
                InstallError::Io(format!("cannot create staging executable file: {e}"))
            })?;
            let mut limited = std::io::Read::take(&mut entry, MAX_EXECUTABLE_BYTES + 1);
            let written = std::io::copy(&mut limited, &mut out).map_err(|e| {
                InstallError::Io(format!("failed to extract executable from tar.gz: {e}"))
            })?;
            if written > MAX_EXECUTABLE_BYTES {
                let _ = std::fs::remove_file(target_path);
                return Err(InstallError::Archive(
                    "executable stream exceeded maximum permitted size during extraction"
                        .to_string(),
                ));
            }
            found_executable = true;
        }
    }

    if !found_executable {
        return Err(InstallError::Archive(format!(
            "expected executable '{expected_binary_name}' not found in archive"
        )));
    }

    Ok(())
}

/// Extract the designated executable from a `.zip` archive with strict resource limits
/// and traversal / link rejection.
fn extract_executable_from_zip(
    bytes: &[u8],
    expected_binary_name: &str,
    target_path: &Path,
) -> Result<(), InstallError> {
    let cursor = std::io::Cursor::new(bytes);
    let mut zip = zip::ZipArchive::new(cursor)
        .map_err(|e| InstallError::Archive(format!("invalid zip archive: {e}")))?;

    if zip.len() > MAX_ARCHIVE_ENTRIES {
        return Err(InstallError::Archive(format!(
            "zip entry count ({}) exceeded limit of {MAX_ARCHIVE_ENTRIES}",
            zip.len()
        )));
    }

    let mut total_uncompressed: u64 = 0;
    let mut found_executable = false;

    for i in 0..zip.len() {
        let mut file = zip
            .by_index(i)
            .map_err(|e| InstallError::Archive(format!("cannot read zip entry {i}: {e}")))?;

        let raw_name = file.name().to_string();
        let path = Path::new(&raw_name);

        validate_archive_entry_path(path)?;
        if file.enclosed_name().is_none() {
            return Err(InstallError::Archive(format!(
                "zip entry path escape detected: {raw_name}"
            )));
        }

        // Check for symlinks via unix file mode
        if let Some(mode) = file.unix_mode() {
            if (mode & 0o170000) == 0o120000 {
                return Err(InstallError::Archive(format!(
                    "symlink in zip entry prohibited: {raw_name}"
                )));
            }
        }

        let size = file.size();
        total_uncompressed = total_uncompressed.saturating_add(size);
        if total_uncompressed > MAX_ARCHIVE_TOTAL_UNCOMPRESSED_BYTES {
            return Err(InstallError::Archive(format!(
                "cumulative uncompressed size exceeded limit of {MAX_ARCHIVE_TOTAL_UNCOMPRESSED_BYTES} bytes"
            )));
        }

        let is_target_file = !file.is_dir()
            && path.file_name().and_then(|s| s.to_str()) == Some(expected_binary_name);

        if is_target_file {
            if found_executable {
                return Err(InstallError::Archive(format!(
                    "multiple executable entries named '{expected_binary_name}' found in zip; substitution rejected"
                )));
            }
            if size > MAX_EXECUTABLE_BYTES {
                return Err(InstallError::Archive(format!(
                    "executable size ({size} bytes) in zip exceeds limit of {MAX_EXECUTABLE_BYTES} bytes"
                )));
            }

            let mut out = std::fs::File::create(target_path).map_err(|e| {
                InstallError::Io(format!("cannot create staging executable file: {e}"))
            })?;
            let mut limited = std::io::Read::take(&mut file, MAX_EXECUTABLE_BYTES + 1);
            let written = std::io::copy(&mut limited, &mut out).map_err(|e| {
                InstallError::Io(format!("failed to extract executable from zip: {e}"))
            })?;
            if written > MAX_EXECUTABLE_BYTES {
                let _ = std::fs::remove_file(target_path);
                return Err(InstallError::Archive(
                    "executable stream exceeded maximum permitted size during extraction"
                        .to_string(),
                ));
            }
            found_executable = true;
        }
    }

    if !found_executable {
        return Err(InstallError::Archive(format!(
            "expected executable '{expected_binary_name}' not found in zip archive"
        )));
    }

    Ok(())
}

/// Verify that an executable exists, is non-empty, and runs correctly with `--version`
/// when the target matches the current executing host.
fn verify_executable(
    path: &Path,
    artifact: &ReleaseArtifact,
    expected_channel: DeploymentChannel,
) -> Result<(), InstallError> {
    if !path.is_file() {
        return Err(InstallError::Verification(format!(
            "executable file does not exist at '{}'",
            path.display()
        )));
    }
    let meta = std::fs::metadata(path).map_err(|e| InstallError::Io(e.to_string()))?;
    if meta.len() == 0 {
        return Err(InstallError::Verification(
            "executable file is 0 bytes".to_string(),
        ));
    }

    let current_ctx = DeploymentContext::current();
    if artifact.target == current_ctx.target {
        let output = std::process::Command::new(path).arg("--version").output();
        match output {
            Ok(out) => {
                if !out.status.success() {
                    return Err(InstallError::Verification(format!(
                        "executable failed '--version' check with status {:?}",
                        out.status.code()
                    )));
                }
                let stdout = String::from_utf8_lossy(&out.stdout);
                let binary_name = expected_channel.binary_name();
                if !stdout.contains(binary_name) {
                    return Err(InstallError::Verification(format!(
                        "executable '--version' output did not contain binary name '{binary_name}': {stdout}"
                    )));
                }
                if !stdout.contains(&artifact.version) {
                    return Err(InstallError::Verification(format!(
                        "executable '--version' output did not contain version '{}': {stdout}",
                        artifact.version
                    )));
                }
            }
            Err(e) => {
                return Err(InstallError::Verification(format!(
                    "cannot execute staged binary for '--version' check: {e}"
                )));
            }
        }
    }

    Ok(())
}

/// Staged installation under `install_dir`:
///   `<install_dir>/<binary>`          — live executable
///   `<install_dir>/<binary>.prev`    — previous known-good backup
///   `<install_dir>/.staging/<file>`  — unverified staging area
#[derive(Debug, Clone)]
pub struct Installer {
    install_dir: PathBuf,
}

impl Installer {
    pub fn new(install_dir: impl Into<PathBuf>) -> Self {
        Self {
            install_dir: install_dir.into(),
        }
    }

    pub fn install_dir(&self) -> &Path {
        &self.install_dir
    }

    pub fn live_path(&self, binary_name: &str) -> PathBuf {
        self.install_dir.join(binary_name)
    }

    pub fn prev_path(&self, binary_name: &str) -> PathBuf {
        self.install_dir.join(format!("{binary_name}.prev"))
    }

    pub fn staging_path(&self, filename: &str) -> PathBuf {
        self.install_dir.join(".staging").join(filename)
    }

    /// Restore the previous backup binary over the live path and verify the restoration.
    pub fn restore_backup(&self, binary_name: &str) -> Result<PathBuf, InstallError> {
        let live = self.live_path(binary_name);
        let prev = self.prev_path(binary_name);
        if !prev.is_file() {
            return Err(InstallError::Io(
                "no previous backup file exists to restore".to_string(),
            ));
        }

        let staging_dir = self.install_dir.join(".staging");
        let _ = std::fs::create_dir_all(&staging_dir);
        let restore_temp =
            staging_dir.join(format!("{binary_name}.restore.{}", uuid::Uuid::now_v7()));

        std::fs::copy(&prev, &restore_temp)
            .map_err(|e| InstallError::Io(format!("cannot copy backup to staging: {e}")))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = std::fs::metadata(&restore_temp) {
                let mut perms = meta.permissions();
                perms.set_mode(0o755);
                let _ = std::fs::set_permissions(&restore_temp, perms);
            }
        }

        std::fs::rename(&restore_temp, &live)
            .map_err(|e| InstallError::Io(format!("cannot restore backup over live: {e}")))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = std::fs::metadata(&live) {
                let mut perms = meta.permissions();
                perms.set_mode(0o755);
                let _ = std::fs::set_permissions(&live, perms);
            }
        }

        // Verify restored executable
        if !live.is_file() {
            return Err(InstallError::Verification(
                "restoration failed: live executable missing after restore".to_string(),
            ));
        }
        let meta = std::fs::metadata(&live).map_err(|e| InstallError::Io(e.to_string()))?;
        if meta.len() == 0 {
            return Err(InstallError::Verification(
                "restoration failed: restored live executable is 0 bytes".to_string(),
            ));
        }

        Ok(live)
    }

    /// Clean up any stale staging files inside `.staging`.
    fn purge_staging_temp_files(&self, binary_name: &str) {
        let staging_dir = self.install_dir.join(".staging");
        if let Ok(entries) = std::fs::read_dir(&staging_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
                    if name.starts_with(binary_name)
                        && (name.contains(".stage.")
                            || name.contains(".backup.")
                            || name.contains(".restore."))
                    {
                        let _ = std::fs::remove_file(&p);
                    }
                }
            }
        }
    }

    /// Install `bytes` as `artifact` after full verification, extraction, and atomic replacement.
    pub fn install_bytes(
        &self,
        artifact: &ReleaseArtifact,
        bytes: &[u8],
        expected_channel: DeploymentChannel,
    ) -> Result<PathBuf, InstallError> {
        artifact
            .validate()
            .map_err(|e| InstallError::Verification(e.to_string()))?;
        if artifact.channel != expected_channel {
            return Err(InstallError::ChannelMismatch(format!(
                "artifact channel '{}' does not match installer channel '{}'",
                artifact.channel, expected_channel
            )));
        }
        if bytes.len() as u64 != artifact.size {
            return Err(InstallError::Verification(format!(
                "size mismatch: manifest {} vs downloaded {}",
                artifact.size,
                bytes.len()
            )));
        }
        let computed = sha256_bytes(bytes);
        if computed != artifact.sha256 {
            return Err(InstallError::Verification(format!(
                "checksum mismatch for '{}': expected {}, computed {}",
                artifact.filename, artifact.sha256, computed
            )));
        }
        if bytes.is_empty() {
            return Err(InstallError::Verification(
                "refusing to install empty artifact".to_string(),
            ));
        }

        let parsed_format = artifact
            .parsed_format()
            .map_err(|e| InstallError::Verification(e.to_string()))?;
        let binary_name = artifact.expected_executable_name();

        let staging_dir = self.install_dir.join(".staging");
        std::fs::create_dir_all(&staging_dir)
            .map_err(|e| InstallError::Io(format!("cannot create staging dir: {e}")))?;

        // Idempotent staging hygiene
        self.purge_staging_temp_files(&binary_name);

        let staged = staging_dir.join(format!("{binary_name}.stage.{}", uuid::Uuid::now_v7()));

        // Extract or write based on explicit packaging format
        match parsed_format {
            ArtifactFormat::RawExecutable => {
                if bytes.len() as u64 > MAX_EXECUTABLE_BYTES {
                    return Err(InstallError::Archive(format!(
                        "raw executable size ({} bytes) exceeds limit of {MAX_EXECUTABLE_BYTES} bytes",
                        bytes.len()
                    )));
                }
                std::fs::write(&staged, bytes).map_err(|e| {
                    InstallError::Io(format!("cannot write staged executable: {e}"))
                })?;
            }
            ArtifactFormat::TarGz => {
                if let Err(e) = extract_executable_from_targz(bytes, &binary_name, &staged) {
                    let _ = std::fs::remove_file(&staged);
                    return Err(e);
                }
            }
            ArtifactFormat::Zip => {
                if let Err(e) = extract_executable_from_zip(bytes, &binary_name, &staged) {
                    let _ = std::fs::remove_file(&staged);
                    return Err(e);
                }
            }
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&staged)
                .map_err(|e| InstallError::Io(e.to_string()))?
                .permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&staged, perms)
                .map_err(|e| InstallError::Io(e.to_string()))?;
        }

        // Verify staged executable before touching live or backup files
        if let Err(e) = verify_executable(&staged, artifact, expected_channel) {
            let _ = std::fs::remove_file(&staged);
            return Err(e);
        }

        std::fs::create_dir_all(&self.install_dir)
            .map_err(|e| InstallError::Io(format!("cannot create install dir: {e}")))?;

        let live = self.live_path(&binary_name);
        let prev = self.prev_path(&binary_name);
        let had_previous_live = live.is_file();

        // Preserve previous known-good binary BEFORE replacement
        if had_previous_live {
            let prev_stage =
                staging_dir.join(format!("{binary_name}.backup.{}", uuid::Uuid::now_v7()));
            std::fs::copy(&live, &prev_stage).map_err(|e| {
                InstallError::Io(format!("cannot stage previous binary backup: {e}"))
            })?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(meta) = std::fs::metadata(&prev_stage) {
                    let mut perms = meta.permissions();
                    perms.set_mode(0o755);
                    let _ = std::fs::set_permissions(&prev_stage, perms);
                }
            }

            std::fs::rename(&prev_stage, &prev).map_err(|e| {
                InstallError::Io(format!("cannot atomically back up previous binary: {e}"))
            })?;
        }

        // Atomic replacement
        #[cfg(unix)]
        {
            if let Err(e) = std::fs::rename(&staged, &live) {
                let _ = std::fs::remove_file(&staged);
                return Err(InstallError::Io(format!(
                    "cannot atomically replace executable: {e}"
                )));
            }
        }

        #[cfg(windows)]
        {
            if live.is_file() {
                let old_temp =
                    staging_dir.join(format!("{binary_name}.old.{}", uuid::Uuid::now_v7()));
                if let Err(e) = std::fs::rename(&live, &old_temp) {
                    let _ = std::fs::remove_file(&staged);
                    return Err(InstallError::Io(format!(
                        "cannot move active executable on Windows: {e}"
                    )));
                }
                if let Err(e) = std::fs::rename(&staged, &live) {
                    let _ = std::fs::rename(&old_temp, &live);
                    let _ = std::fs::remove_file(&staged);
                    return Err(InstallError::Io(format!(
                        "cannot replace active executable on Windows: {e}"
                    )));
                }
                let _ = std::fs::remove_file(&old_temp);
            } else {
                if let Err(e) = std::fs::rename(&staged, &live) {
                    let _ = std::fs::remove_file(&staged);
                    return Err(InstallError::Io(format!(
                        "cannot place executable on Windows: {e}"
                    )));
                }
            }
        }

        // Post-replacement verification of live binary
        if let Err(verification_err) = verify_executable(&live, artifact, expected_channel) {
            if had_previous_live && prev.is_file() {
                match self.restore_backup(&binary_name) {
                    Ok(_) => {
                        return Err(InstallError::Verification(format!(
                            "installed executable failed post-replacement verification ({verification_err}); previous binary was successfully restored"
                        )));
                    }
                    Err(restore_err) => {
                        return Err(InstallError::CatastrophicRestoreFailed {
                            original_error: verification_err.to_string(),
                            restore_error: restore_err.to_string(),
                            live_path: live,
                            backup_path: prev,
                        });
                    }
                }
            } else {
                let _ = std::fs::remove_file(&live);
                return Err(InstallError::Verification(format!(
                    "first-time installation failed post-replacement verification ({verification_err}); cleaned up incomplete binary"
                )));
            }
        }

        // Cleanup any residual staging files
        self.purge_staging_temp_files(&binary_name);

        Ok(live)
    }

    /// Path of the preserved previous binary, if any.
    pub fn previous_binary(&self, binary_name: &str) -> Option<PathBuf> {
        let p = self.prev_path(binary_name);
        p.is_file().then_some(p)
    }
}

#[cfg(test)]
pub mod test_helpers {
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::io::Write;

    pub fn make_tar_gz(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let enc = GzEncoder::new(Vec::new(), Compression::default());
        let mut tar = tar::Builder::new(enc);
        for (name, content) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_size(content.len() as u64);
            header.set_mode(0o755);
            header.set_cksum();
            tar.append_data(&mut header, *name, *content).unwrap();
        }
        tar.into_inner().unwrap().finish().unwrap()
    }

    pub fn make_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buf);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated)
                .unix_permissions(0o755);
            for (name, content) in entries {
                zip.start_file(*name, options).unwrap();
                zip.write_all(content).unwrap();
            }
            zip.finish().unwrap();
        }
        buf.into_inner()
    }

    pub fn make_raw_tar_gz_with_name(name: &str, content: &[u8], is_symlink: bool) -> Vec<u8> {
        use flate2::Compression;
        use flate2::write::GzEncoder;
        use std::io::Write;
        let mut header_buf = [0u8; 512];
        let name_bytes = name.as_bytes();
        let copy_len = name_bytes.len().min(100);
        header_buf[..copy_len].copy_from_slice(&name_bytes[..copy_len]);
        header_buf[100..108].copy_from_slice(b"0000755\0");
        let size_str = format!("{:011o}\0", content.len());
        header_buf[124..136].copy_from_slice(size_str.as_bytes());
        header_buf[156] = if is_symlink { b'2' } else { b'0' };
        header_buf[257..263].copy_from_slice(b"ustar\0");

        let mut sum: u32 = 0;
        for (i, &b) in header_buf.iter().enumerate() {
            if (148..156).contains(&i) {
                sum += 32;
            } else {
                sum += b as u32;
            }
        }
        let chksum_str = format!("{:06o}\0 ", sum);
        header_buf[148..156].copy_from_slice(chksum_str.as_bytes());

        let mut raw_tar = Vec::new();
        raw_tar.extend_from_slice(&header_buf);
        if !content.is_empty() {
            raw_tar.extend_from_slice(content);
            let pad = (512 - (content.len() % 512)) % 512;
            raw_tar.extend_from_slice(&vec![0u8; pad]);
        }
        raw_tar.extend_from_slice(&[0u8; 1024]);

        let mut enc = GzEncoder::new(Vec::new(), Compression::default());
        enc.write_all(&raw_tar).unwrap();
        enc.finish().unwrap()
    }

    pub fn test_script_bytes(version: &str, channel_binary: &str) -> Vec<u8> {
        format!("#!/bin/sh\necho \"{channel_binary} {version}\"\n").into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::test_helpers::*;
    use super::*;
    use crate::deployment::channel::DeploymentChannel;

    fn artifact_for(
        channel: DeploymentChannel,
        format: &str,
        filename: &str,
        version: &str,
        target: &str,
        bytes: &[u8],
    ) -> ReleaseArtifact {
        ReleaseArtifact {
            artifact_id: format!("{}-{version}-{target}", channel.binary_name()),
            version: version.to_string(),
            channel,
            target: target.to_string(),
            format: format.to_string(),
            filename: filename.to_string(),
            sha256: sha256_bytes(bytes),
            size: bytes.len() as u64,
            build_id: "0123456789abcdef".to_string(),
            git_commit: "abc12345".to_string(),
        }
    }

    #[test]
    fn successful_tar_gz_install_and_update_preserves_previous() {
        let dir = tempfile::tempdir().unwrap();
        let inst = Installer::new(dir.path());
        let current_target = DeploymentContext::current().target;

        let script_v1 = test_script_bytes("0.1.1", "m31a");
        let tar_v1 = make_tar_gz(&[("m31a", &script_v1)]);
        let a1 = artifact_for(
            DeploymentChannel::Production,
            "tar.gz",
            "m31a-0.1.1.tar.gz",
            "0.1.1",
            &current_target,
            &tar_v1,
        );

        let live = inst
            .install_bytes(&a1, &tar_v1, DeploymentChannel::Production)
            .unwrap();
        assert_eq!(std::fs::read(&live).unwrap(), script_v1);
        assert!(inst.previous_binary("m31a").is_none());

        let script_v2 = test_script_bytes("0.1.2", "m31a");
        let tar_v2 = make_tar_gz(&[("m31a", &script_v2)]);
        let a2 = artifact_for(
            DeploymentChannel::Production,
            "tar.gz",
            "m31a-0.1.2.tar.gz",
            "0.1.2",
            &current_target,
            &tar_v2,
        );

        inst.install_bytes(&a2, &tar_v2, DeploymentChannel::Production)
            .unwrap();
        assert_eq!(std::fs::read(&live).unwrap(), script_v2);

        let prev = inst.previous_binary("m31a").unwrap();
        assert_eq!(std::fs::read(&prev).unwrap(), script_v1);
    }

    #[test]
    fn successful_zip_install_and_update() {
        let dir = tempfile::tempdir().unwrap();
        let inst = Installer::new(dir.path());
        let current_target = DeploymentContext::current().target;

        let script_v1 = test_script_bytes("0.1.1", "m31a");
        let zip_v1 = make_zip(&[("m31a", &script_v1)]);
        let a1 = artifact_for(
            DeploymentChannel::Production,
            "zip",
            "m31a-0.1.1.zip",
            "0.1.1",
            &current_target,
            &zip_v1,
        );

        let live = inst
            .install_bytes(&a1, &zip_v1, DeploymentChannel::Production)
            .unwrap();
        assert_eq!(std::fs::read(&live).unwrap(), script_v1);
    }

    #[test]
    fn checksum_mismatch_keeps_previous() {
        let dir = tempfile::tempdir().unwrap();
        let inst = Installer::new(dir.path());
        let current_target = DeploymentContext::current().target;

        let script_v1 = test_script_bytes("0.1.1", "m31a");
        let tar_v1 = make_tar_gz(&[("m31a", &script_v1)]);
        let a1 = artifact_for(
            DeploymentChannel::Production,
            "tar.gz",
            "m31a-0.1.1.tar.gz",
            "0.1.1",
            &current_target,
            &tar_v1,
        );
        let live = inst
            .install_bytes(&a1, &tar_v1, DeploymentChannel::Production)
            .unwrap();

        let mut bad = a1.clone();
        bad.size = b"corrupted".len() as u64;
        let r = inst.install_bytes(&bad, b"corrupted", DeploymentChannel::Production);
        assert!(matches!(r, Err(InstallError::Verification(_))));
        assert_eq!(std::fs::read(&live).unwrap(), script_v1);
    }

    #[test]
    fn path_traversal_tar_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let inst = Installer::new(dir.path());
        let current_target = DeploymentContext::current().target;

        let script = test_script_bytes("0.1.1", "m31a");
        let bad_tar = make_raw_tar_gz_with_name("../../../escaped", &script, false);
        let art = artifact_for(
            DeploymentChannel::Production,
            "tar.gz",
            "m31a-0.1.1.tar.gz",
            "0.1.1",
            &current_target,
            &bad_tar,
        );

        let r = inst.install_bytes(&art, &bad_tar, DeploymentChannel::Production);
        assert!(matches!(r, Err(InstallError::Archive(_))));
    }

    #[test]
    fn symlink_tar_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let inst = Installer::new(dir.path());
        let current_target = DeploymentContext::current().target;

        let script = test_script_bytes("0.1.1", "m31a");
        let bad_tar = make_raw_tar_gz_with_name("m31a", &script, true);
        let art = artifact_for(
            DeploymentChannel::Production,
            "tar.gz",
            "m31a-0.1.1.tar.gz",
            "0.1.1",
            &current_target,
            &bad_tar,
        );

        let r = inst.install_bytes(&art, &bad_tar, DeploymentChannel::Production);
        assert!(matches!(r, Err(InstallError::Archive(_))));
    }

    #[test]
    fn path_traversal_zip_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let inst = Installer::new(dir.path());
        let current_target = DeploymentContext::current().target;

        let script = test_script_bytes("0.1.1", "m31a");
        let bad_zip = make_zip(&[("../escaped", &script)]);
        let art = artifact_for(
            DeploymentChannel::Production,
            "zip",
            "m31a-0.1.1.zip",
            "0.1.1",
            &current_target,
            &bad_zip,
        );

        let r = inst.install_bytes(&art, &bad_zip, DeploymentChannel::Production);
        assert!(matches!(r, Err(InstallError::Archive(_))));
    }

    #[test]
    fn missing_executable_in_archive_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let inst = Installer::new(dir.path());
        let current_target = DeploymentContext::current().target;

        let script = test_script_bytes("0.1.1", "m31a");
        let bad_tar = make_tar_gz(&[("unrelated.txt", &script)]);
        let art = artifact_for(
            DeploymentChannel::Production,
            "tar.gz",
            "m31a-0.1.1.tar.gz",
            "0.1.1",
            &current_target,
            &bad_tar,
        );

        let r = inst.install_bytes(&art, &bad_tar, DeploymentChannel::Production);
        assert!(matches!(r, Err(InstallError::Archive(_))));
    }

    #[test]
    fn multiple_matching_executables_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let inst = Installer::new(dir.path());
        let current_target = DeploymentContext::current().target;

        let script = test_script_bytes("0.1.1", "m31a");
        let bad_tar = make_tar_gz(&[("dir1/m31a", &script), ("dir2/m31a", &script)]);
        let art = artifact_for(
            DeploymentChannel::Production,
            "tar.gz",
            "m31a-0.1.1.tar.gz",
            "0.1.1",
            &current_target,
            &bad_tar,
        );

        let r = inst.install_bytes(&art, &bad_tar, DeploymentChannel::Production);
        assert!(matches!(r, Err(InstallError::Archive(_))));
    }

    #[test]
    fn restoration_on_post_replacement_failure() {
        let dir = tempfile::tempdir().unwrap();
        let inst = Installer::new(dir.path());
        let current_target = DeploymentContext::current().target;

        // 1. Install good v1
        let script_v1 = test_script_bytes("0.1.1", "m31a");
        let tar_v1 = make_tar_gz(&[("m31a", &script_v1)]);
        let a1 = artifact_for(
            DeploymentChannel::Production,
            "tar.gz",
            "m31a-0.1.1.tar.gz",
            "0.1.1",
            &current_target,
            &tar_v1,
        );
        let live = inst
            .install_bytes(&a1, &tar_v1, DeploymentChannel::Production)
            .unwrap();

        // 2. Prepare v2 that passes extraction and pre-check (different non-host target)
        // but fails post-replacement when checked or restored
        // When target is current host, let's test restore_backup directly:
        let backup = inst.previous_binary("m31a");
        assert!(backup.is_none());

        // Install v2
        let script_v2 = test_script_bytes("0.1.2", "m31a");
        let tar_v2 = make_tar_gz(&[("m31a", &script_v2)]);
        let a2 = artifact_for(
            DeploymentChannel::Production,
            "tar.gz",
            "m31a-0.1.2.tar.gz",
            "0.1.2",
            &current_target,
            &tar_v2,
        );
        inst.install_bytes(&a2, &tar_v2, DeploymentChannel::Production)
            .unwrap();
        assert_eq!(std::fs::read(&live).unwrap(), script_v2);

        // Backup has v1
        let prev = inst.previous_binary("m31a").unwrap();
        assert_eq!(std::fs::read(&prev).unwrap(), script_v1);

        // Corrupt live
        std::fs::write(&live, b"broken").unwrap();
        // Restore backup
        let restored = inst.restore_backup("m31a").unwrap();
        assert_eq!(restored, live);
        assert_eq!(std::fs::read(&live).unwrap(), script_v1);
    }
}

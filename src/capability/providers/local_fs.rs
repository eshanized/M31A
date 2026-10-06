//! Local filesystem capability provider with strict workspace path containment (CTL-02, D-01, D-03).

use crate::capability::error::CapabilityError;
use crate::capability::traits::fs::{FileMetadata, FileSystemService};
use async_trait::async_trait;
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, SeekFrom};

/// Native local filesystem provider operating within a constrained workspace root.
pub struct LocalFileSystemProvider {
    workspace_root: PathBuf,
}

impl LocalFileSystemProvider {
    /// Create a new LocalFileSystemProvider rooted at the given workspace directory.
    pub fn new(workspace_root: impl Into<PathBuf>) -> Result<Self, CapabilityError> {
        let root = workspace_root.into();
        let canon = root.canonicalize().map_err(|e| {
            CapabilityError::Io(format!(
                "failed to canonicalize workspace root {}: {e}",
                root.display()
            ))
        })?;
        Ok(Self {
            workspace_root: strip_verbatim_prefix(&canon),
        })
    }

    /// Access the canonical workspace root.
    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    /// Resolve and strictly verify that a path resides within the workspace root.
    /// Rejects directory traversal escapes (`..`), symlink escapes, absolute out-of-boundary paths,
    /// and any access to protected repository/runtime state (`.git` and `.m31a`).
    pub fn resolve_and_verify(&self, path: &Path) -> Result<PathBuf, CapabilityError> {
        let path_str = path.to_string_lossy();
        let stripped = path_str.strip_prefix('@').unwrap_or(&path_str);
        let normalized_slashes = stripped.replace('\\', "/");
        let cleaned_path = Path::new(&normalized_slashes);

        // 1. Immediate lexical check on raw requested path components
        if contains_protected_component(cleaned_path) {
            return Err(CapabilityError::PermissionDenied(format!(
                "access to protected path '{}' is denied: cannot access repository or runtime control directories (.git, .m31a)",
                path.display()
            )));
        }

        let full_path = if cleaned_path.is_absolute() {
            cleaned_path.to_path_buf()
        } else {
            self.workspace_root.join(cleaned_path)
        };

        // Lexically normalize away `.` and `..`
        let normalized = normalize_path(&full_path);

        // Lexical boundary check
        if !normalized.starts_with(&self.workspace_root) {
            return Err(CapabilityError::PathOutOfBounds {
                path: path.display().to_string(),
                workspace: self.workspace_root.display().to_string(),
            });
        }

        // 2. Check normalized path relative to workspace root
        if normalized
            .strip_prefix(&self.workspace_root)
            .map(contains_protected_component)
            .unwrap_or(false)
        {
            return Err(CapabilityError::PermissionDenied(format!(
                "access to protected path '{}' is denied: normalized path contains protected component (.git or .m31a)",
                path.display()
            )));
        }

        // If the path exists on disk, check its canonical path to guard against symlink escapes
        if normalized.exists() {
            let canon = normalized.canonicalize().map_err(|e| {
                CapabilityError::Io(format!(
                    "failed to canonicalize path {}: {e}",
                    path.display()
                ))
            })?;
            let clean_canon = strip_verbatim_prefix(&canon);
            if !clean_canon.starts_with(&self.workspace_root) {
                return Err(CapabilityError::PathOutOfBounds {
                    path: path.display().to_string(),
                    workspace: self.workspace_root.display().to_string(),
                });
            }

            // 3. Check canonical path relative to workspace root (guards against symlink aliases to .git or .m31a)
            if clean_canon
                .strip_prefix(&self.workspace_root)
                .map(contains_protected_component)
                .unwrap_or(false)
            {
                return Err(CapabilityError::PermissionDenied(format!(
                    "access to protected path '{}' is denied: resolves through symlink to protected component (.git or .m31a)",
                    path.display()
                )));
            }

            Ok(clean_canon)
        } else {
            // For not-yet-existing paths (e.g. for write_file), canonicalize existing ancestor
            let mut ancestor = normalized.clone();
            while !ancestor.exists() {
                if let Some(parent) = ancestor.parent() {
                    ancestor = parent.to_path_buf();
                } else {
                    break;
                }
            }
            if ancestor.exists() {
                let canon_ancestor = ancestor.canonicalize().map_err(|e| {
                    CapabilityError::Io(format!(
                        "failed to canonicalize ancestor {}: {e}",
                        ancestor.display()
                    ))
                })?;
                let clean_ancestor = strip_verbatim_prefix(&canon_ancestor);
                if !clean_ancestor.starts_with(&self.workspace_root) {
                    return Err(CapabilityError::PathOutOfBounds {
                        path: path.display().to_string(),
                        workspace: self.workspace_root.display().to_string(),
                    });
                }

                // 4. Check canonical ancestor relative to workspace root (guards against creating files inside symlinks pointing to .git or .m31a)
                if clean_ancestor
                    .strip_prefix(&self.workspace_root)
                    .map(contains_protected_component)
                    .unwrap_or(false)
                {
                    return Err(CapabilityError::PermissionDenied(format!(
                        "access to protected path '{}' is denied: ancestor resolves to protected component (.git or .m31a)",
                        path.display()
                    )));
                }
            }
            Ok(normalized)
        }
    }
}

pub use crate::kernel::invariants::{contains_protected_component, is_protected_component};

/// Strip Windows verbatim prefixes (`\\?\` or `\\?\UNC\`) so paths can be
/// compared uniformly against non-verbatim paths across platforms.
pub fn strip_verbatim_prefix(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{}", stripped))
    } else if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        path.to_path_buf()
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                components.pop();
            }
            c => components.push(c),
        }
    }
    components.into_iter().collect()
}

#[async_trait]
impl FileSystemService for LocalFileSystemProvider {
    async fn read_file(
        &self,
        path: &Path,
        offset: Option<u64>,
        limit: Option<usize>,
    ) -> Result<Vec<u8>, CapabilityError> {
        let verified = self.resolve_and_verify(path)?;
        let mut file = fs::File::open(&verified)
            .await
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => {
                    CapabilityError::NotFound(path.display().to_string())
                }
                _ => CapabilityError::Io(e.to_string()),
            })?;

        if let Some(off) = offset {
            file.seek(SeekFrom::Start(off))
                .await
                .map_err(|e| CapabilityError::Io(e.to_string()))?;
        }

        let mut buffer = Vec::new();
        if let Some(lim) = limit {
            let mut handle = (&mut file).take(lim as u64);
            handle
                .read_to_end(&mut buffer)
                .await
                .map_err(|e| CapabilityError::Io(e.to_string()))?;
        } else {
            file.read_to_end(&mut buffer)
                .await
                .map_err(|e| CapabilityError::Io(e.to_string()))?;
        }

        Ok(buffer)
    }

    async fn write_file(&self, path: &Path, content: &[u8]) -> Result<usize, CapabilityError> {
        let verified = self.resolve_and_verify(path)?;
        if let Some(parent) = verified.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| CapabilityError::Io(e.to_string()))?;
        }

        let mut file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&verified)
            .await
            .map_err(|e| CapabilityError::Io(e.to_string()))?;

        file.write_all(content)
            .await
            .map_err(|e| CapabilityError::Io(e.to_string()))?;
        file.flush()
            .await
            .map_err(|e| CapabilityError::Io(e.to_string()))?;

        Ok(content.len())
    }

    async fn edit_file(
        &self,
        path: &Path,
        old_content: &str,
        new_content: &str,
    ) -> Result<(), CapabilityError> {
        let verified = self.resolve_and_verify(path)?;
        let bytes = fs::read(&verified).await.map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => CapabilityError::NotFound(path.display().to_string()),
            _ => CapabilityError::Io(e.to_string()),
        })?;

        let text = String::from_utf8(bytes).map_err(|e| {
            CapabilityError::InvalidArgument(format!("file is not valid UTF-8: {e}"))
        })?;

        let op = crate::tools::fs::editor::FileEditOp::Substring {
            old_content,
            new_content,
        };

        match crate::tools::fs::editor::RobustFileEditor::apply(&text, &op) {
            Ok(success) => {
                fs::write(&verified, success.new_content.as_bytes())
                    .await
                    .map_err(|e| CapabilityError::Io(e.to_string()))?;
                Ok(())
            }
            Err(crate::tools::fs::editor::EditDiagnostic::AmbiguousMatch { message, .. }) => {
                Err(CapabilityError::InvalidArgument(message))
            }
            Err(diag) => Err(CapabilityError::NotFound(diag.to_string())),
        }
    }

    async fn list_files(
        &self,
        path: &Path,
        recursive: bool,
    ) -> Result<Vec<PathBuf>, CapabilityError> {
        let verified = self.resolve_and_verify(path)?;
        let mut results = Vec::new();
        let mut dirs = vec![verified];

        while let Some(current_dir) = dirs.pop() {
            let mut read_dir = fs::read_dir(&current_dir)
                .await
                .map_err(|e| match e.kind() {
                    std::io::ErrorKind::NotFound => {
                        CapabilityError::NotFound(path.display().to_string())
                    }
                    _ => CapabilityError::Io(e.to_string()),
                })?;

            while let Some(entry) = read_dir
                .next_entry()
                .await
                .map_err(|e| CapabilityError::Io(e.to_string()))?
            {
                let file_name = entry.file_name();
                let file_name_str = file_name.to_string_lossy();
                if is_protected_component(&file_name_str) {
                    continue;
                }

                let entry_path = entry.path();
                if entry_path
                    .strip_prefix(&self.workspace_root)
                    .map(contains_protected_component)
                    .unwrap_or(false)
                {
                    continue;
                }

                let file_type = entry
                    .file_type()
                    .await
                    .map_err(|e| CapabilityError::Io(e.to_string()))?;

                // If symlink, verify its target does not resolve to protected path
                if file_type.is_symlink() {
                    let is_unsafe_symlink = match entry_path.canonicalize() {
                        Ok(canon) => {
                            !canon.starts_with(&self.workspace_root)
                                || canon
                                    .strip_prefix(&self.workspace_root)
                                    .map(contains_protected_component)
                                    .unwrap_or(false)
                        }
                        Err(_) => false,
                    };
                    if is_unsafe_symlink {
                        continue;
                    }
                }

                if file_type.is_dir() {
                    if recursive {
                        dirs.push(entry_path.clone());
                    }
                    if let Ok(rel) = entry_path.strip_prefix(&self.workspace_root) {
                        results.push(rel.to_path_buf());
                    }
                } else if let Ok(rel) = entry_path.strip_prefix(&self.workspace_root) {
                    results.push(rel.to_path_buf());
                }
            }
        }

        results.sort();
        Ok(results)
    }

    async fn file_metadata(&self, path: &Path) -> Result<FileMetadata, CapabilityError> {
        let verified = self.resolve_and_verify(path)?;
        let meta = fs::metadata(&verified).await.map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => CapabilityError::NotFound(path.display().to_string()),
            _ => CapabilityError::Io(e.to_string()),
        })?;

        let modified_ms = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64);

        Ok(FileMetadata {
            size_bytes: meta.len(),
            is_file: meta.is_file(),
            is_dir: meta.is_dir(),
            is_readonly: meta.permissions().readonly(),
            modified_ms,
        })
    }

    async fn delete_file(&self, path: &Path) -> Result<(), CapabilityError> {
        let verified = self.resolve_and_verify(path)?;
        fs::remove_file(&verified)
            .await
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => {
                    CapabilityError::NotFound(path.display().to_string())
                }
                _ => CapabilityError::Io(e.to_string()),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_workspace_containment() {
        let dir = tempdir().unwrap();
        let provider = LocalFileSystemProvider::new(dir.path()).unwrap();

        // Safe relative path
        assert!(provider.resolve_and_verify(Path::new("test.txt")).is_ok());

        // Subdirectory relative path
        assert!(
            provider
                .resolve_and_verify(Path::new("sub/dir/test.txt"))
                .is_ok()
        );

        // Path traversal escape
        let traversal_res = provider.resolve_and_verify(Path::new("../escape.txt"));
        assert!(matches!(
            traversal_res,
            Err(CapabilityError::PathOutOfBounds { .. })
        ));

        // Absolute out-of-bounds path
        let abs_escape = provider.resolve_and_verify(Path::new("/etc/passwd"));
        assert!(matches!(
            abs_escape,
            Err(CapabilityError::PathOutOfBounds { .. })
        ));
    }

    #[tokio::test]
    async fn test_fs_operations() {
        let dir = tempdir().unwrap();
        let provider = LocalFileSystemProvider::new(dir.path()).unwrap();

        // Write
        let written = provider
            .write_file(Path::new("hello.txt"), b"Hello, world!")
            .await
            .unwrap();
        assert_eq!(written, 13);

        // Read
        let content = provider
            .read_file(Path::new("hello.txt"), None, None)
            .await
            .unwrap();
        assert_eq!(content, b"Hello, world!");

        // Edit
        provider
            .edit_file(Path::new("hello.txt"), "world", "M31A")
            .await
            .unwrap();
        let updated = provider
            .read_file(Path::new("hello.txt"), None, None)
            .await
            .unwrap();
        assert_eq!(updated, b"Hello, M31A!");

        // Metadata
        let meta = provider
            .file_metadata(Path::new("hello.txt"))
            .await
            .unwrap();
        assert!(meta.is_file);
        assert_eq!(meta.size_bytes, 12);

        // Delete
        provider.delete_file(Path::new("hello.txt")).await.unwrap();
        let not_found = provider.read_file(Path::new("hello.txt"), None, None).await;
        assert!(matches!(not_found, Err(CapabilityError::NotFound(_))));
    }
}

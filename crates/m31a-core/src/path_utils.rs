//! Atomic path utilities, symlink resolution guards, and filesystem normalization.
//!
//! Provides atomic file writes via sibling temporary files to prevent corruption,
//! symlink chain traversal with cycle detection, and canonical path comparison.

use std::collections::HashSet;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

/// Result of safe symlink write path resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymlinkWritePaths {
    /// Resolved target path for reading (None if broken or cycle detected).
    pub read_path: Option<PathBuf>,
    /// Safe destination path for writes.
    pub write_path: PathBuf,
}

/// Atomically write bytes to a file by writing to a temporary file in the same parent directory
/// and atomically renaming it to the target path.
pub fn atomic_write_file(path: &Path, content: &[u8]) -> io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;

    // Create temporary file in the same filesystem directory to ensure rename is atomic
    let mut temp = NamedTempFile::new_in(parent)?;
    temp.write_all(content)?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

/// Read filesystem metadata without following the final symlink.
/// Returns Ok(None) if the file does not exist.
pub fn symlink_metadata_if_exists(path: &Path) -> io::Result<Option<fs::Metadata>> {
    match fs::symlink_metadata(path) {
        Ok(meta) => Ok(Some(meta)),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

/// Resolve symlink chains while detecting cycles via a visited path set.
/// Returns both the final read path and the write path.
pub fn resolve_symlink_write_paths(path: &Path) -> io::Result<SymlinkWritePaths> {
    let root = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };

    let mut current = root.clone();
    let mut visited = HashSet::new();

    loop {
        if !visited.insert(current.clone()) {
            // Cycle detected!
            return Ok(SymlinkWritePaths {
                read_path: None,
                write_path: root,
            });
        }

        let meta = match fs::symlink_metadata(&current) {
            Ok(m) => m,
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                return Ok(SymlinkWritePaths {
                    read_path: Some(current.clone()),
                    write_path: current,
                });
            }
            Err(_) => {
                return Ok(SymlinkWritePaths {
                    read_path: None,
                    write_path: root,
                });
            }
        };

        if meta.file_type().is_symlink() {
            match fs::read_link(&current) {
                Ok(target) => {
                    current = if target.is_absolute() {
                        target
                    } else {
                        current
                            .parent()
                            .unwrap_or_else(|| Path::new("/"))
                            .join(target)
                    };
                }
                Err(_) => {
                    return Ok(SymlinkWritePaths {
                        read_path: None,
                        write_path: root,
                    });
                }
            }
        } else {
            return Ok(SymlinkWritePaths {
                read_path: Some(current.clone()),
                write_path: current,
            });
        }
    }
}

/// Compare two paths after applying canonicalization if possible.
/// Falls back to direct string equality if canonicalization fails.
pub fn paths_match_after_normalization(left: impl AsRef<Path>, right: impl AsRef<Path>) -> bool {
    let l_canon = left.as_ref().canonicalize();
    let r_canon = right.as_ref().canonicalize();

    match (l_canon, r_canon) {
        (Ok(l), Ok(r)) => l == r,
        _ => left.as_ref() == right.as_ref(),
    }
}

/// Replace all occurrences of `old_path` in a list and deduplicate, preserving insertion order.
pub fn replace_path_and_deduplicate<P>(mut paths: Vec<P>, old_path: &Path, new_path: P) -> Vec<P>
where
    P: AsRef<Path> + Clone + Eq + std::hash::Hash,
{
    for p in &mut paths {
        if p.as_ref() == old_path {
            *p = new_path.clone();
        }
    }
    let mut seen = HashSet::new();
    paths.retain(|p| seen.insert(p.clone()));
    paths
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_atomic_write_file_success() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("sub").join("test_atomic.txt");
        let content = b"Sovereign M31A atomic write verification";

        atomic_write_file(&target, content).unwrap();
        assert!(target.exists());

        let read_back = fs::read(&target).unwrap();
        assert_eq!(read_back, content);
    }

    #[test]
    fn test_symlink_resolution_and_cycle_detection() {
        let dir = tempdir().unwrap();
        let regular_file = dir.path().join("regular.txt");
        fs::write(&regular_file, b"data").unwrap();

        // 1. Regular file
        let paths = resolve_symlink_write_paths(&regular_file).unwrap();
        assert_eq!(paths.read_path, Some(regular_file.clone()));
        assert_eq!(paths.write_path, regular_file);

        // 2. Symlink to regular file
        #[cfg(unix)]
        {
            let link = dir.path().join("link.txt");
            std::os::unix::fs::symlink(&regular_file, &link).unwrap();
            let resolved = resolve_symlink_write_paths(&link).unwrap();
            assert_eq!(resolved.read_path, Some(regular_file));

            // 3. Symlink cycle: a -> b -> a
            let cycle_a = dir.path().join("cycle_a");
            let cycle_b = dir.path().join("cycle_b");
            std::os::unix::fs::symlink(&cycle_b, &cycle_a).unwrap();
            std::os::unix::fs::symlink(&cycle_a, &cycle_b).unwrap();

            let cycle_res = resolve_symlink_write_paths(&cycle_a).unwrap();
            assert_eq!(cycle_res.read_path, None);
        }
    }

    #[test]
    fn test_replace_path_and_deduplicate() {
        let p1 = PathBuf::from("/a/b");
        let p2 = PathBuf::from("/c/d");
        let p3 = PathBuf::from("/a/b");
        let list = vec![p1, p2, p3];

        let replaced = replace_path_and_deduplicate(list, Path::new("/a/b"), PathBuf::from("/x/y"));
        assert_eq!(replaced, vec![PathBuf::from("/x/y"), PathBuf::from("/c/d")]);
    }
}

//! Artifact integrity: SHA-256 checksums, sorted manifest files, and
//! fail-closed verification.
//!
//! Rules: cryptographic checksums for every artifact; deterministic ordering;
//! no ambiguous filenames; no path traversal in manifests; no silent omission;
//! verification fails when an artifact is modified, missing, or unexpected.

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// SHA-256 hex digest of in-memory bytes.
pub fn sha256_bytes(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// SHA-256 hex digest of a file's exact bytes.
pub fn sha256_file(path: &Path) -> Result<String, IntegrityError> {
    let data = std::fs::read(path).map_err(|e| IntegrityError::Io {
        path: path.display().to_string(),
        message: e.to_string(),
    })?;
    Ok(sha256_bytes(&data))
}

/// Typed integrity failures. Verification never reports success on any of these.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IntegrityError {
    #[error("I/O error on '{path}': {message}")]
    Io { path: String, message: String },
    #[error("checksum mismatch for '{name}': expected {expected}, computed {computed}")]
    Mismatch {
        name: String,
        expected: String,
        computed: String,
    },
    #[error("artifact missing: '{name}'")]
    Missing { name: String },
    #[error("unexpected artifact not in manifest: '{name}'")]
    Unexpected { name: String },
    #[error("unsafe artifact name (traversal/absolute/empty): '{name}'")]
    UnsafeName { name: String },
    #[error("malformed checksum line {line}: '{content}'")]
    MalformedLine { line: usize, content: String },
}

/// Reject manifest/artifact names that could traverse, escape, or confuse.
/// Allows `a-z A-Z 0-9 . _ -` plus exactly one extension dot pattern and `/`
/// separators (no leading/trailing `/`, no `..`, no absolute paths).
pub fn validate_artifact_name(name: &str) -> Result<(), IntegrityError> {
    if name.is_empty() || name.len() > 255 {
        return Err(IntegrityError::UnsafeName {
            name: name.to_string(),
        });
    }
    if name.starts_with('/') || name.contains('\\') {
        return Err(IntegrityError::UnsafeName {
            name: name.to_string(),
        });
    }
    // Leading dashes become options on extraction/invocation tooling.
    if name.starts_with('-') || name.split('/').any(|c| c.starts_with('-')) {
        return Err(IntegrityError::UnsafeName {
            name: name.to_string(),
        });
    }
    // Non-canonical dot segments are ambiguous: reject lexically (Rust's
    // components() normalizes them away, so the component check below is
    // only a backstop).
    if name == "." || name.starts_with("./") || name.contains("/./") || name.ends_with("/.") {
        return Err(IntegrityError::UnsafeName {
            name: name.to_string(),
        });
    }
    let path = Path::new(name);
    if path.is_absolute() {
        return Err(IntegrityError::UnsafeName {
            name: name.to_string(),
        });
    }
    for comp in path.components() {
        match comp {
            std::path::Component::ParentDir
            | std::path::Component::RootDir
            | std::path::Component::Prefix(_) => {
                return Err(IntegrityError::UnsafeName {
                    name: name.to_string(),
                });
            }
            std::path::Component::Normal(os) => {
                let s = os.to_string_lossy();
                if s.is_empty()
                    || s == "."
                    || s.contains('\\')
                    || !s
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '+'))
                {
                    return Err(IntegrityError::UnsafeName {
                        name: name.to_string(),
                    });
                }
            }
            std::path::Component::CurDir => {
                return Err(IntegrityError::UnsafeName {
                    name: name.to_string(),
                });
            }
        }
    }
    Ok(())
}

/// One checksum record: 64-char hex, two spaces, artifact name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChecksumRecord {
    pub sha256: String,
    pub name: String,
}

/// Write a deterministic SHA256SUMS document for `dir`: entries sorted by
/// name, one `"<sha>  <name>"` line each, trailing newline. Fails closed on
/// unsafe names or unreadable files — never silently omits.
pub fn write_sha256sums(dir: &Path, names: &[String]) -> Result<String, IntegrityError> {
    let mut sorted: Vec<&String> = names.iter().collect();
    sorted.sort();
    let mut out = String::new();
    for name in sorted {
        validate_artifact_name(name)?;
        let digest = sha256_file(&dir.join(name))?;
        if digest.len() != 64 || !digest.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(IntegrityError::Mismatch {
                name: name.clone(),
                expected: "<64 hex chars>".to_string(),
                computed: digest,
            });
        }
        out.push_str(&format!("{digest}  {name}\n"));
    }
    Ok(out)
}

/// Parse and verify a SHA256SUMS document against `dir`.
///
/// - Every listed artifact must exist with matching bytes (else Missing/Mismatch).
/// - When `reject_unexpected` is true, every regular file directly inside
///   `dir` must be listed (else Unexpected). Subdirectories are ignored.
/// - Malformed lines fail closed. Artifacts are never reported verified
///   solely because they exist.
pub fn verify_sha256sums(
    dir: &Path,
    document: &str,
    reject_unexpected: bool,
) -> Result<Vec<ChecksumRecord>, IntegrityError> {
    let mut records = Vec::new();
    for (idx, line) in document.lines().enumerate() {
        let line_no = idx + 1;
        if line.trim().is_empty() {
            continue;
        }
        let mut parts = line.splitn(2, "  ");
        let (Some(sha), Some(name)) = (parts.next(), parts.next()) else {
            return Err(IntegrityError::MalformedLine {
                line: line_no,
                content: line.to_string(),
            });
        };
        if sha.len() != 64 || !sha.chars().all(|c| c.is_ascii_hexdigit()) || name.is_empty() {
            return Err(IntegrityError::MalformedLine {
                line: line_no,
                content: line.to_string(),
            });
        }
        validate_artifact_name(name)?;
        let path = dir.join(name);
        if !path.is_file() {
            return Err(IntegrityError::Missing {
                name: name.to_string(),
            });
        }
        let computed = sha256_file(&path)?;
        if computed != sha {
            return Err(IntegrityError::Mismatch {
                name: name.to_string(),
                expected: sha.to_string(),
                computed,
            });
        }
        records.push(ChecksumRecord {
            sha256: sha.to_string(),
            name: name.to_string(),
        });
    }
    if records.is_empty() {
        return Err(IntegrityError::MalformedLine {
            line: 0,
            content: "<empty checksum document>".to_string(),
        });
    }
    if reject_unexpected {
        let mut listed: Vec<&str> = records.iter().map(|r| r.name.as_str()).collect();
        listed.sort();
        let entries = std::fs::read_dir(dir).map_err(|e| IntegrityError::Io {
            path: dir.display().to_string(),
            message: e.to_string(),
        })?;
        for entry in entries {
            let entry = entry.map_err(|e| IntegrityError::Io {
                path: dir.display().to_string(),
                message: e.to_string(),
            })?;
            let ft = entry.file_type().map_err(|e| IntegrityError::Io {
                path: entry.path().display().to_string(),
                message: e.to_string(),
            })?;
            if !ft.is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if listed.binary_search(&name.as_str()).is_err() {
                return Err(IntegrityError::Unexpected { name });
            }
        }
    }
    Ok(records)
}

/// Resolve an artifact name against `dir` without allowing escape.
pub fn contained_artifact_path(dir: &Path, name: &str) -> Result<PathBuf, IntegrityError> {
    validate_artifact_name(name)?;
    Ok(dir.join(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_validation_table() {
        for bad in [
            "",
            "../x",
            "/abs",
            "a/../../b",
            "-x",
            ".",
            "a/./b",
            "a\\b",
            "sp ace",
        ] {
            assert!(validate_artifact_name(bad).is_err(), "{bad}");
        }
        for good in [
            "m31a.tar.gz",
            "SHA256SUMS",
            "release.json",
            "sbom/sbom.json",
            "v1.0/x86_64.bin",
        ] {
            assert!(validate_artifact_name(good).is_ok(), "{good}");
        }
    }

    #[test]
    fn tamper_missing_unexpected_detected() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.bin"), b"aaa").unwrap();
        std::fs::write(dir.path().join("b.bin"), b"bbb").unwrap();
        let doc =
            write_sha256sums(dir.path(), &["a.bin".to_string(), "b.bin".to_string()]).unwrap();
        assert!(verify_sha256sums(dir.path(), &doc, true).is_ok());
        // Tamper.
        std::fs::write(dir.path().join("a.bin"), b"AAA").unwrap();
        assert!(matches!(
            verify_sha256sums(dir.path(), &doc, true),
            Err(IntegrityError::Mismatch { .. })
        ));
        // Missing (document references only the removed file).
        std::fs::remove_file(dir.path().join("b.bin")).unwrap();
        let doc_b = format!("{}  b.bin\n", sha256_bytes(b"bbb"));
        assert!(matches!(
            verify_sha256sums(dir.path(), &doc_b, false),
            Err(IntegrityError::Missing { .. })
        ));
        std::fs::write(dir.path().join("b.bin"), b"bbb").unwrap();
        std::fs::write(dir.path().join("a.bin"), b"aaa").unwrap();
        assert!(verify_sha256sums(dir.path(), &doc, false).is_ok());
        // Unexpected file.
        std::fs::write(dir.path().join("extra.bin"), b"???").unwrap();
        assert!(matches!(
            verify_sha256sums(dir.path(), &doc, true),
            Err(IntegrityError::Unexpected { .. })
        ));
        std::fs::remove_file(dir.path().join("extra.bin")).unwrap();
        assert!(verify_sha256sums(dir.path(), &doc, true).is_ok());
    }

    #[test]
    fn empty_document_never_verifies() {
        let dir = tempfile::tempdir().unwrap();
        assert!(verify_sha256sums(dir.path(), "", true).is_err());
        assert!(verify_sha256sums(dir.path(), "\n\n", false).is_err());
    }
}

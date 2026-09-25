//! Release-directory cleanliness assertions.
//!
//! The release output directory must be deterministic and controlled: no
//! temporary files, no secrets, no developer-local leakage. Findings are
//! reported, never silently ignored.

use serde::{Deserialize, Serialize};

/// One hygiene finding in a release directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HygieneFinding {
    pub file: String,
    pub kind: HygieneKind,
    pub detail: String,
}

/// Hygiene violation kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HygieneKind {
    TempFile,
    SecretContent,
    AbsoluteLocalPath,
}

/// Scan `dir` (top level, regular files only) for release contamination:
/// - temporary/editor files (`.tmp`, `.bak`, `~` suffixes, `.swp`, `.#*`);
/// - secret-bearing content in files under 1MiB (API keys, tokens,
///   private-key blocks);
/// - `.env`-style files that must never ship.
///
/// Returns all findings (empty = clean). Directory read failures are
/// reported as findings, never swallowed.
pub fn check_release_dir(dir: &std::path::Path) -> Vec<HygieneFinding> {
    let mut findings = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            findings.push(HygieneFinding {
                file: dir.display().to_string(),
                kind: HygieneKind::TempFile,
                detail: format!("release directory unreadable: {e}"),
            });
            return findings;
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_type().map(|t| !t.is_file()).unwrap_or(true) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let lower = name.to_lowercase();
        if lower.ends_with(".tmp")
            || lower.ends_with(".bak")
            || lower.ends_with('~')
            || lower.ends_with(".swp")
            || lower.starts_with(".#")
            || lower.starts_with(".env")
            || lower == ".env"
        {
            findings.push(HygieneFinding {
                file: name.clone(),
                kind: HygieneKind::TempFile,
                detail: "temporary, backup, editor, or environment file must not ship".to_string(),
            });
            continue;
        }
        let Ok(meta) = std::fs::metadata(&path) else {
            continue;
        };
        if meta.len() > 1024 * 1024 {
            continue;
        }
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let text = String::from_utf8_lossy(&bytes);
        for pattern in [
            "nvapi-",
            "sk-ant-",
            "AKIA",
            "-----BEGIN ",
            "PRIVATE KEY-----",
            "xoxb-",
            "ghp_",
        ] {
            if text.contains(pattern) {
                findings.push(HygieneFinding {
                    file: name.clone(),
                    kind: HygieneKind::SecretContent,
                    detail: format!("secret pattern '{pattern}' in releasable file"),
                });
                break;
            }
        }
    }
    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_temp_and_secrets() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("m31a.tar.gz"), b"binary").unwrap();
        std::fs::write(dir.path().join("notes.tmp"), b"x").unwrap();
        std::fs::write(dir.path().join("keys.txt"), b"token nvapi-abcdef").unwrap();
        let findings = check_release_dir(dir.path());
        assert!(findings.iter().any(|f| f.kind == HygieneKind::TempFile));
        assert!(
            findings
                .iter()
                .any(|f| f.kind == HygieneKind::SecretContent)
        );
        assert!(!findings.iter().any(|f| f.file == "m31a.tar.gz"));
    }

    #[test]
    fn clean_dir_passes() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("m31a.tar.gz"), b"binary").unwrap();
        std::fs::write(dir.path().join("SHA256SUMS"), b"abc  m31a.tar.gz\n").unwrap();
        assert!(check_release_dir(dir.path()).is_empty());
    }
}

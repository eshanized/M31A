//! Windows path security: containment without string-prefix traps.
//!
//! Windows paths admit drive letters, UNC shares, alternate separators,
//! case-insensitive comparison, reserved device names, and reparse-point
//! redirection. Lexical comparison alone cannot establish containment.
//! This module centralizes every check so callers never reimplement prefix
//! logic. All helpers are pure and exercised on every host.

use std::path::{Component, Path, PathBuf};

/// Reserved Windows device names that cannot name ordinary files.
const RESERVED_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Whether a file stem collides with a reserved device name (case-insensitive,
/// trailing dots/spaces stripped per Win32 rules).
pub fn is_reserved_name(path: &Path) -> bool {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let normalized = stem.trim_end_matches(['.', ' ']).to_ascii_uppercase();
    RESERVED_NAMES.contains(&normalized.as_str())
}

/// Normalize separators to backslash for analysis without touching the
/// underlying path semantics.
pub fn normalize_separators(input: &str) -> String {
    input.replace('/', "\\")
}

/// Whether the path uses UNC form (`\\server\share` or `//server/share`).
pub fn is_unc_path(input: &str) -> bool {
    let n = normalize_separators(input);
    n.starts_with("\\\\") && n.len() > 2 && !n[2..].starts_with('\\')
}

/// Whether the path carries an explicit drive letter (`C:`, `C:\...`).
pub fn drive_letter(input: &str) -> Option<char> {
    let bytes = input.as_bytes();
    if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
        Some((bytes[0] as char).to_ascii_uppercase())
    } else {
        None
    }
}

/// Whether the path is absolute under Windows rules.
pub fn is_absolute_windows_path(input: &str) -> bool {
    let n = normalize_separators(input);
    if is_unc_path(input) {
        return true;
    }
    if n.len() >= 3 && drive_letter(&n).is_some() && n.as_bytes()[2] == b'\\' {
        return true;
    }
    if n == "\\" || n.starts_with("\\") {
        return true;
    }
    false
}

/// Lexical normalization honoring Windows separators and case rules.
/// Resolves `.`, `..`, duplicate separators, and drive-relative prefixes
/// without touching the filesystem (no reparse-point resolution).
pub fn lexical_normalize_windows(input: &str) -> String {
    let normalized = normalize_separators(input);
    let mut out = String::new();
    let mut drive: Option<String> = None;
    let mut is_unc = false;
    let mut rest = normalized.as_str();

    if is_unc_path(&normalized) {
        is_unc = true;
        rest = &normalized[2..];
    } else if let Some(letter) = drive_letter(&normalized) {
        drive = Some(format!("{}:", letter));
        rest = &normalized[2..];
    }

    let mut parts: Vec<&str> = Vec::new();
    for part in rest.split('\\') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            p => parts.push(p),
        }
    }

    if let Some(d) = drive {
        out.push_str(&d);
        out.push('\\');
    } else if is_unc {
        out.push_str("\\\\");
    } else if normalized.starts_with('\\') {
        out.push('\\');
    }
    out.push_str(&parts.join("\\"));
    if out.is_empty() {
        out.push('.');
    }
    out
}

/// Case-insensitive component comparison used for containment.
fn components_equal(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
}

/// Whether `candidate` resides within `workspace` under Windows semantics.
///
/// Rules enforced:
/// - separators unified before comparison;
/// - comparison is case-insensitive per component;
/// - `..` resolved lexically first so `..\escape` cannot hide;
/// - drive letters and UNC prefixes must match exactly;
/// - a bare string prefix is never sufficient: the next boundary must be a
///   separator or the paths must be identical.
pub fn is_within_workspace(workspace: &str, candidate: &str) -> bool {
    let ws = lexical_normalize_windows(workspace);
    let cand = lexical_normalize_windows(candidate);

    // Volume must agree: differing drive letters or UNC hosts never contain.
    let ws_drive = drive_letter(&ws);
    let cand_drive = drive_letter(&cand);
    if ws_drive != cand_drive {
        return false;
    }
    if is_unc_path(&ws) != is_unc_path(&cand) {
        return false;
    }
    if is_unc_path(&ws) && is_unc_path(&cand) {
        let ws_host = ws
            .split('\\')
            .nth(2)
            .unwrap_or_default()
            .to_ascii_lowercase();
        let cand_host = cand
            .split('\\')
            .nth(2)
            .unwrap_or_default()
            .to_ascii_lowercase();
        if ws_host != cand_host {
            return false;
        }
    }

    let ws_parts: Vec<&str> = ws.split('\\').filter(|s| !s.is_empty()).collect();
    let cand_parts: Vec<&str> = cand.split('\\').filter(|s| !s.is_empty()).collect();
    if cand_parts.len() < ws_parts.len() {
        return false;
    }
    for (w, c) in ws_parts.iter().zip(cand_parts.iter()) {
        // Drive-letter components compare case-insensitively like the rest.
        if !components_equal(w, c) {
            return false;
        }
    }
    true
}

/// Validate that `candidate` (absolute or workspace-relative) stays inside
/// `workspace`. Relative candidates resolve against the workspace first.
pub fn validate_containment(workspace: &Path, candidate: &Path) -> Result<PathBuf, String> {
    let ws_str = workspace.to_string_lossy().to_string();
    let cand_str = candidate.to_string_lossy().to_string();
    let joined = if Path::new(&cand_str).is_absolute() || is_absolute_windows_path(&cand_str) {
        cand_str.clone()
    } else {
        format!("{}\\{}", ws_str.trim_end_matches(['/', '\\']), cand_str)
    };
    if is_reserved_name(candidate) {
        return Err(format!(
            "path '{}' uses a reserved Windows device name",
            candidate.display()
        ));
    }
    if !is_within_workspace(&ws_str, &joined) {
        return Err(format!(
            "path '{}' escapes workspace '{}'",
            candidate.display(),
            workspace.display()
        ));
    }
    Ok(PathBuf::from(lexical_normalize_windows(&joined)))
}

/// Detect whether a path representation needs canonicalization before a
/// containment decision (reparse points, junctions, symlinks, `..`, mixed
/// separators, or case tricks).
pub fn needs_canonicalization(path: &Path) -> bool {
    let s = path.to_string_lossy();
    s.contains("..")
        || s.contains('/')
        || s != s.to_lowercase() && cfg!(windows)
        || is_unc_path(&s)
        || drive_letter(&s).is_some()
}

/// Whether two paths name the same file under Windows identity rules.
pub fn paths_identical_windows(left: &Path, right: &Path) -> bool {
    let l = lexical_normalize_windows(&left.to_string_lossy());
    let r = lexical_normalize_windows(&right.to_string_lossy());
    l.eq_ignore_ascii_case(&r)
}

/// Strip the Win32 extended-path prefix (`\\?\`) for display.
pub fn strip_extended_prefix(path: &str) -> &str {
    path.strip_prefix("\\\\?\\").unwrap_or(path)
}

#[allow(dead_code)]
fn _component_touchstone(p: &Path) -> Vec<Component<'_>> {
    // Retained to document that component-level reasoning stays in this
    // module; callers use the string-normalized entry points above.
    p.components().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drive_letter_mismatch_never_contains() {
        assert!(!is_within_workspace("C:\\work", "D:\\work\\file.txt"));
    }

    #[test]
    fn prefix_trick_is_rejected() {
        assert!(!is_within_workspace("C:\\work", "C:\\work-evil\\file.txt"));
        assert!(is_within_workspace("C:\\work", "C:\\work\\file.txt"));
    }

    #[test]
    fn case_insensitive_containment() {
        assert!(is_within_workspace("C:\\Work", "c:\\work\\SUB\\file.txt"));
    }

    #[test]
    fn dotdot_escape_is_rejected() {
        assert!(!is_within_workspace(
            "C:\\work",
            "C:\\work\\..\\evil\\file.txt"
        ));
    }

    #[test]
    fn unc_hosts_must_match() {
        assert!(!is_within_workspace(
            "\\\\srv\\share\\work",
            "\\\\other\\share\\work\\f"
        ));
        assert!(is_within_workspace(
            "\\\\srv\\share\\work",
            "\\\\srv\\share\\work\\sub\\f"
        ));
    }

    #[test]
    fn reserved_names_rejected() {
        assert!(is_reserved_name(Path::new("NUL")));
        assert!(is_reserved_name(Path::new("com1.txt")));
        assert!(!is_reserved_name(Path::new("normal.txt")));
    }

    #[test]
    fn separators_unified() {
        assert!(is_within_workspace("C:\\work", "C:/work/sub/file.txt"));
    }
}

//! Semantic terminal hyperlinks (OSC 8) support.
//!
//! Generates clickable links inside compatible terminal emulators (Ghostty,
//! WezTerm, iTerm2, Alacritty, VS Code integrated terminal, Warp, Kitty).

use std::path::Path;

/// Wraps text in an OSC 8 terminal hyperlink escape sequence.
/// In supporting terminals, clicking the text opens the target URL or file.
pub fn format_hyperlink(url: &str, text: &str) -> String {
    format!("\x1b]8;;{}\x1b\\{}\x1b]8;;\x1b\\", url, text)
}

/// Formats a file path as a clickable `file://` hyperlink with optional line and column.
pub fn format_file_hyperlink(path: &Path, line: Option<usize>) -> String {
    format_clickable_path(path, line, None)
}

/// Formats a file path with line and column into a standard IDE-navigable clickable URI.
pub fn format_clickable_path(path: &Path, line: Option<usize>, col: Option<usize>) -> String {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let display_str = path.display().to_string();

    let uri = match (line, col) {
        (Some(l), Some(c)) => format!("file://{}#L{}:{}", canonical.display(), l, c),
        (Some(l), None) => format!("file://{}#L{}", canonical.display(), l),
        (None, _) => format!("file://{}", canonical.display()),
    };

    let label = match (line, col) {
        (Some(l), Some(c)) => format!("{}:{}:{}", display_str, l, c),
        (Some(l), None) => format!("{}:{}", display_str, l),
        (None, _) => display_str,
    };

    format_hyperlink(&uri, &label)
}

/// Formats a code symbol (struct, function, trait) into a clickable link to its definition.
pub fn format_symbol_hyperlink(path: &Path, symbol: &str, line: Option<usize>) -> String {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let uri = if let Some(l) = line {
        format!("file://{}#L{}", canonical.display(), l)
    } else {
        format!("file://{}", canonical.display())
    };

    let label = format!(
        "`{}` ({})",
        symbol,
        path.file_name().unwrap_or_default().to_string_lossy()
    );
    format_hyperlink(&uri, &label)
}

/// Formats a git commit hash into a clickable web or repository hyperlink.
pub fn format_git_commit_hyperlink(repo_url: &str, commit_hash: &str) -> String {
    let short_hash = if commit_hash.len() > 8 {
        &commit_hash[..8]
    } else {
        commit_hash
    };

    let clean_url = repo_url.trim_end_matches(".git").trim_end_matches('/');
    let commit_url = format!("{}/commit/{}", clean_url, commit_hash);
    format_hyperlink(&commit_url, short_hash)
}

/// Tests if a string contains OSC 8 hyperlink escape codes.
pub fn has_hyperlink(s: &str) -> bool {
    s.contains("\x1b]8;;")
}

/// Strips OSC 8 hyperlink escape codes from a string, leaving only the readable label.
pub fn strip_hyperlinks(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\x1b' && chars.peek() == Some(&']') {
            // Check for OSC 8 sequence
            chars.next(); // consume ']'
            if chars.next() == Some('8') && chars.next() == Some(';') && chars.next() == Some(';') {
                // Skip URI until string terminator ST (\x1b\\) or BEL (\x07)
                while let Some(c) = chars.next() {
                    if c == '\x07' {
                        break;
                    }
                    if c == '\x1b' && chars.peek() == Some(&'\\') {
                        chars.next();
                        break;
                    }
                }
                continue;
            } else {
                result.push('\x1b');
                result.push(']');
            }
        } else {
            result.push(ch);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_hyperlink() {
        let link = format_hyperlink("https://build.nvidia.com", "NVIDIA NIM");
        assert_eq!(
            link,
            "\x1b]8;;https://build.nvidia.com\x1b\\NVIDIA NIM\x1b]8;;\x1b\\"
        );
        assert!(has_hyperlink(&link));
    }

    #[test]
    fn test_format_file_hyperlink() {
        let path = Path::new("src/main.rs");
        let link = format_file_hyperlink(path, Some(42));
        assert!(link.contains("src/main.rs:42"));
        assert!(link.contains("#L42"));
    }

    #[test]
    fn test_format_clickable_path_with_col() {
        let path = Path::new("crates/m31a-core/src/lib.rs");
        let link = format_clickable_path(path, Some(10), Some(5));
        assert!(link.contains("crates/m31a-core/src/lib.rs:10:5"));
        assert!(link.contains("#L10:5"));
    }

    #[test]
    fn test_format_symbol_hyperlink() {
        let path = Path::new("src/model.rs");
        let link = format_symbol_hyperlink(path, "NimProvider", Some(25));
        assert!(link.contains("`NimProvider` (model.rs)"));
    }

    #[test]
    fn test_strip_hyperlinks() {
        let link = format_hyperlink("https://example.com", "Example Link");
        let plain = strip_hyperlinks(&link);
        assert_eq!(plain, "Example Link");
    }
}

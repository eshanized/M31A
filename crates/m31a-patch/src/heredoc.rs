//! Heredoc extraction for `apply_patch <<'EOF'` shell invocations.
//!
//! Language models frequently format multi-file patch payloads within shell command
//! heredoc blocks (e.g. `apply_patch <<'EOF'`). A bare shell would fail
//! with `apply_patch: command not found`; instead, the patch payload is
//! extracted from the heredoc and routed to the patch engine.
//!
//! Strict shape per upstream `apply-patch/src/heredoc.rs`:
//!   `apply_patch <<'EOF' … EOF`
//!   `cd <path> && apply_patch <<'EOF' … EOF`
//!
//! Returns `None` for anything else so the command falls through to normal
//! shell execution (where the OS produces the natural error).

/// Extracts the patch text from an `apply_patch <<'TAG' … TAG` heredoc command.
/// The terminator line must be exactly `TAG`; non-empty content after the
/// terminator rejects the extraction (strict upstream shape).
pub fn extract_apply_patch_heredoc(command: &str) -> Option<String> {
    let trimmed = command.trim();

    // Accept an optional `cd <path> && ` prefix; the path is ignored because
    // patch paths are resolved against the tool's cwd.
    let body = if let Some(rest) = trimmed.strip_prefix("cd ") {
        // `cd <path> && apply_patch <<'EOF'` — skip past the first `&&`.
        let idx = rest.find("&&")?;
        rest[idx + 2..].trim_start()
    } else {
        trimmed
    };

    // `apply_patch <<'EOF'` (single-quoted tag is the canonical shape).
    let rest = body.strip_prefix("apply_patch <<")?;
    let rest = rest.trim_start();

    // Split the marker line from the heredoc body. No newline at all means an
    // unterminated heredoc (no payload, no terminator).
    let (marker_line, payload_region) = rest.split_once('\n')?;

    let (tag, after_tag) = if let Some(r) = marker_line.strip_prefix('\'') {
        let end = r.find('\'')?;
        let (t, tail) = r.split_at(end);
        (t, tail.strip_prefix('\'')?)
    } else if let Some(r) = marker_line.strip_prefix('"') {
        let end = r.find('"')?;
        let (t, tail) = r.split_at(end);
        (t, tail.strip_prefix('"')?)
    } else {
        // Bare (unquoted) tag: read until whitespace or end of line.
        let end = marker_line
            .find(char::is_whitespace)
            .unwrap_or(marker_line.len());
        marker_line.split_at(end)
    };

    // Strict: nothing but whitespace may follow the heredoc marker on that line.
    if tag.is_empty() || !after_tag.trim().is_empty() {
        return None;
    }

    let mut lines = payload_region.split('\n');
    let mut payload: Vec<&str> = Vec::new();
    loop {
        match lines.next() {
            Some(line) if line.trim_end() == tag => {
                // Strict: only an empty remainder (trailing newline) may follow
                // the terminator; real trailing commands reject extraction.
                if lines.next().is_some_and(|extra| !extra.trim().is_empty()) {
                    return None;
                }
                return Some(payload.join("\n"));
            }
            Some(line) => {
                // Strip a trailing CR so CRLF-terminated commands yield a clean
                // patch payload (trailing spaces/tabs are preserved).
                payload.push(line.strip_suffix('\r').unwrap_or(line));
            }
            None => return None, // unterminated heredoc — fall through to shell
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PATCH: &str = "*** Begin Patch\n*** Update File: a.rs\n@@\n-old\n+new\n*** End Patch";

    #[test]
    fn test_extract_canonical_shape() {
        let cmd = format!("apply_patch <<'EOF'\n{}\nEOF", PATCH);
        assert_eq!(extract_apply_patch_heredoc(&cmd).as_deref(), Some(PATCH));
    }

    #[test]
    fn test_extract_with_cd_prefix() {
        let cmd = format!("cd /tmp && apply_patch <<'EOF'\n{}\nEOF", PATCH);
        assert_eq!(extract_apply_patch_heredoc(&cmd).as_deref(), Some(PATCH));
    }

    #[test]
    fn test_extract_custom_quoted_tag() {
        let cmd = format!("apply_patch <<\"PATCH\"\n{}\nPATCH", PATCH);
        assert_eq!(extract_apply_patch_heredoc(&cmd).as_deref(), Some(PATCH));
    }

    #[test]
    fn test_extract_crlf_terminated() {
        let cmd = format!("apply_patch <<'EOF'\r\n{}\r\nEOF", PATCH);
        assert_eq!(extract_apply_patch_heredoc(&cmd).as_deref(), Some(PATCH));
    }

    #[test]
    fn test_rejects_unterminated() {
        let cmd = format!("apply_patch <<'EOF'\n{}", PATCH);
        assert_eq!(extract_apply_patch_heredoc(&cmd), None);
    }

    #[test]
    fn test_rejects_trailing_content_after_terminator() {
        let cmd = format!("apply_patch <<'EOF'\n{}\nEOF\necho done", PATCH);
        assert_eq!(extract_apply_patch_heredoc(&cmd), None);
    }

    #[test]
    fn test_rejects_non_apply_patch_commands() {
        assert_eq!(extract_apply_patch_heredoc("ls -la"), None);
        assert_eq!(extract_apply_patch_heredoc("apply_patch --help"), None);
        // Heredoc to a different program is not intercepted.
        assert_eq!(extract_apply_patch_heredoc("cat <<'EOF'\nhi\nEOF"), None);
        // Trailing args after the marker are rejected (strict shape).
        assert_eq!(
            extract_apply_patch_heredoc("apply_patch <<'EOF' --verbose\nx\nEOF"),
            None
        );
    }

    #[test]
    fn test_empty_patch_body() {
        assert_eq!(
            extract_apply_patch_heredoc("apply_patch <<'EOF'\nEOF"),
            Some(String::new())
        );
    }
}

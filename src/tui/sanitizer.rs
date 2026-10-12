//! Terminal Text Sanitization & ANSI Escape Stripping (TUI-06, THREAT-01).
//!
//! Mitigates T-11-18 (terminal control injection) by stripping dangerous
//! ANSI sequences, OSC title overrides, and cursor repositioning commands
//! before rendering untrusted repository, tool, web, or MCP content.

/// Sanitize untrusted external text before rendering in the terminal.
///
/// Strips ANSI escape codes, cursor repositioning sequences, and OSC commands
/// while preserving clean printable UTF-8 text.
pub fn sanitize_terminal_text(input: &str) -> String {
    // 1. Strip raw ANSI escape sequences via strip-ansi-escapes crate
    let stripped_bytes = strip_ansi_escapes::strip(input.as_bytes());
    let stripped_str = String::from_utf8_lossy(&stripped_bytes);

    // 2. Filter out remaining non-printable control codes, except standard newlines and tabs
    stripped_str
        .chars()
        .filter(|&c| c == '\n' || c == '\r' || c == '\t' || (!c.is_control() && c != '\x07'))
        .collect()
}

/// Sanitize diff or unified patch content for terminal preview.
pub fn sanitize_diff(diff: &str) -> String {
    sanitize_terminal_text(diff)
}

/// Sanitize tool execution standard output or standard error spool.
pub fn sanitize_tool_spool(spool: &str) -> String {
    sanitize_terminal_text(spool)
}

/// Truncate `s` to fit within `max_width` display columns in terminal output.
///
/// If `s` already fits within `max_width` columns, it is returned as a borrowed slice.
/// When truncation is required:
/// - `max_width == 0`: returns `""`
/// - `max_width == 1`: returns `"…"`
/// - `max_width > 1`: fits characters up to `max_width - 1` columns followed by `"…"`.
///
/// Never panics on multibyte character boundaries and accounts for wide characters.
pub fn truncate_to_width(s: &str, max_width: usize) -> std::borrow::Cow<'_, str> {
    use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

    if max_width == 0 {
        return std::borrow::Cow::Borrowed("");
    }
    let total_width = s.width();
    if total_width <= max_width {
        return std::borrow::Cow::Borrowed(s);
    }
    if max_width == 1 {
        return std::borrow::Cow::Borrowed("…");
    }

    let budget = max_width.saturating_sub(1);
    let mut current_width = 0;
    let mut byte_end = 0;

    for (idx, ch) in s.char_indices() {
        let ch_width = ch.width().unwrap_or(0);
        if current_width + ch_width > budget {
            break;
        }
        current_width += ch_width;
        byte_end = idx + ch.len_utf8();
    }

    let mut out = String::with_capacity(byte_end + 3);
    out.push_str(&s[..byte_end]);
    out.push('…');
    std::borrow::Cow::Owned(out)
}

/// Truncate `s` to `max_width` display columns, returning an owned `String`.
pub fn truncate_str_safe(s: &str, max_width: usize) -> String {
    truncate_to_width(s, max_width).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitizer_strips_color_and_formatting() {
        let input = "\x1b[31;1mRed Bold Text\x1b[0m and \x1b[32mGreen\x1b[0m";
        let clean = sanitize_terminal_text(input);
        assert_eq!(clean, "Red Bold Text and Green");
    }

    #[test]
    fn test_sanitizer_strips_cursor_repositioning_and_clearing() {
        let input = "Normal Text\x1b[2J\x1b[H\x1b[10;20HInjected Overwrite";
        let clean = sanitize_terminal_text(input);
        assert_eq!(clean, "Normal TextInjected Overwrite");
    }

    #[test]
    fn test_sanitizer_strips_osc_terminal_title_injection() {
        let input = "Safe Output\x1b]0;Hacked Title\x07More Safe Output";
        let clean = sanitize_terminal_text(input);
        assert!(!clean.contains("Hacked Title"));
        assert_eq!(clean, "Safe OutputMore Safe Output");
    }

    #[test]
    fn test_unicode_safe_truncation_ascii() {
        assert_eq!(truncate_to_width("hello world", 5), "hell…");
        assert_eq!(truncate_to_width("hello", 5), "hello");
        assert_eq!(truncate_to_width("hello", 10), "hello");
    }

    #[test]
    fn test_unicode_safe_truncation_limits_zero_and_one() {
        assert_eq!(truncate_to_width("hello", 0), "");
        assert_eq!(truncate_to_width("hello", 1), "…");
        assert_eq!(truncate_to_width("", 0), "");
        assert_eq!(truncate_to_width("", 1), "");
        assert_eq!(truncate_to_width("a", 1), "a");
    }

    #[test]
    fn test_unicode_safe_truncation_multibyte_and_emoji() {
        // "🦀" is 4 bytes, width 2
        let text = "🦀 rust 🦀";
        // Should never panic inside UTF-8 bytes
        for w in 0..15 {
            let res = truncate_to_width(text, w);
            assert!(res.len() <= 20);
        }
        assert_eq!(truncate_to_width("🦀🦀🦀", 4), "🦀…");
    }

    #[test]
    fn test_unicode_safe_truncation_wide_and_combining() {
        // CJK characters have width 2
        let text = "日本語の文字";
        assert_eq!(truncate_to_width(text, 5), "日本…"); // 2 + 2 = 4 + 1('…') = 5
        assert_eq!(truncate_to_width(text, 6), "日本…"); // 4 + 2 would be 6 + 1 = 7 > 6, so stops at 4
        assert_eq!(truncate_to_width(text, 7), "日本語…"); // 2 + 2 + 2 = 6 + 1 = 7
    }

    #[test]
    fn test_unicode_safe_truncation_no_allocation_when_fits() {
        let s = "short";
        match truncate_to_width(s, 10) {
            std::borrow::Cow::Borrowed(b) => assert_eq!(b, "short"),
            std::borrow::Cow::Owned(_) => panic!("Expected borrowed cow"),
        }
    }
}

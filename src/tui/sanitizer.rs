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
}

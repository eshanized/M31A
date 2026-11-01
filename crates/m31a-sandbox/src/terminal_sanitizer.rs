//! Terminal Output Sanitizer for Sandboxed Command Execution.
//!
//! Filters dangerous ANSI escape sequences, terminal control modes, and OS commands (OSC)
//! from untrusted child process outputs to prevent terminal hijacking, clipboard manipulation
//! (OSC 52), window title spoofing, and cursor reflection attacks, while preserving safe
//! formatting (colors, bold, underlines, and standard cursor movements).

/// Security policy for terminal escape sanitization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TerminalSanitizationPolicy {
    /// Allow safe styling (colors, text attributes) while stripping dangerous control escapes.
    #[default]
    FilterDangerous,
    /// Strip all ANSI escape sequences, leaving only clean plain text.
    StripAll,
}

/// Sanitizer engine for processing terminal streams.
#[derive(Debug, Default, Clone)]
pub struct TerminalSanitizer {
    policy: TerminalSanitizationPolicy,
}

impl TerminalSanitizer {
    pub fn new(policy: TerminalSanitizationPolicy) -> Self {
        Self { policy }
    }

    /// Convenience helper to sanitize a UTF-8 string with the default policy.
    pub fn sanitize(input: &str) -> String {
        Self::new(TerminalSanitizationPolicy::FilterDangerous).process_str(input)
    }

    /// Convenience helper to strip all escape sequences from a string.
    pub fn strip_all(input: &str) -> String {
        Self::new(TerminalSanitizationPolicy::StripAll).process_str(input)
    }

    /// Sanitize a string according to the configured policy.
    pub fn process_str(&self, input: &str) -> String {
        let bytes = input.as_bytes();
        let sanitized = self.process_bytes(bytes);
        String::from_utf8_lossy(&sanitized).into_owned()
    }

    /// Sanitize raw byte stream.
    pub fn process_bytes(&self, bytes: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(bytes.len());
        let mut i = 0;

        while i < bytes.len() {
            let b = bytes[i];

            if b == b'\x1b' {
                // Potential escape sequence
                if i + 1 < bytes.len() {
                    let next = bytes[i + 1];

                    // 1. OSC (Operating System Command): ESC ]
                    if next == b']' {
                        // Scan until BEL (\x07) or ST (ESC \)
                        let mut j = i + 2;
                        let mut found_end = false;
                        while j < bytes.len() {
                            if bytes[j] == 0x07 {
                                i = j + 1;
                                found_end = true;
                                break;
                            }
                            if bytes[j] == b'\x1b' && j + 1 < bytes.len() && bytes[j + 1] == b'\\' {
                                i = j + 2;
                                found_end = true;
                                break;
                            }
                            j += 1;
                        }
                        if !found_end {
                            // Unterminated OSC, skip ESC ]
                            i += 2;
                        }
                        continue;
                    }

                    // 2. CSI (Control Sequence Introducer): ESC [
                    if next == b'[' {
                        let mut j = i + 2;
                        // Scan parameter/intermediate bytes: 0x20..=0x3F
                        while j < bytes.len() && (0x20..=0x3F).contains(&bytes[j]) {
                            j += 1;
                        }

                        if j < bytes.len() && (0x40..=0x7E).contains(&bytes[j]) {
                            let final_byte = bytes[j];
                            let csi_slice = &bytes[i..=j];

                            if self.policy == TerminalSanitizationPolicy::StripAll {
                                // Strip completely
                                i = j + 1;
                                continue;
                            }

                            // Filter dangerous CSI commands
                            if is_safe_csi_sequence(csi_slice, final_byte) {
                                out.extend_from_slice(csi_slice);
                            }
                            i = j + 1;
                            continue;
                        } else {
                            // Incomplete CSI, skip ESC [
                            i += 2;
                            continue;
                        }
                    }

                    // 3. Simple two-byte escapes: ESC c (RIS reset), ESC = / ESC >
                    if next == b'c' {
                        // Reset to Initial State - drop
                        i += 2;
                        continue;
                    }
                }

                // Unrecognized lone ESC
                if self.policy == TerminalSanitizationPolicy::StripAll {
                    i += 1;
                } else {
                    out.push(b);
                    i += 1;
                }
                continue;
            }

            // Filter dangerous non-printable control characters
            // Preserve \n (10), \r (13), \t (9)
            if b < 0x20 && b != b'\n' && b != b'\r' && b != b'\t' {
                // Drop bell (7), backspace (8), formfeed (12), etc.
                i += 1;
                continue;
            }

            out.push(b);
            i += 1;
        }

        out
    }
}

/// Checks if a CSI sequence is safe for terminal rendering.
fn is_safe_csi_sequence(_seq: &[u8], final_byte: u8) -> bool {
    match final_byte {
        // SGR (Select Graphic Rendition): colors, bold, dim, italic, underline
        b'm' => true,
        // Cursor movement: Up (A), Down (B), Forward (C), Back (D), NextLine (E), PrevLine (F), Col (G), Pos (H/f)
        b'A' | b'B' | b'C' | b'D' | b'E' | b'F' | b'G' | b'H' | b'f' => true,
        // Erase in Display (J), Erase in Line (K)
        b'J' | b'K' => true,
        // Scroll Up (S), Scroll Down (T)
        b'S' | b'T' => true,
        _ => {
            // Block DSR (Device Status Report b'n'), Soft Reset (b'p'), Mode set/reset (b'h'/b'l'), etc.
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preserves_safe_sgr_colors_and_cursor() {
        let text = "\x1b[32mSuccess\x1b[0m: compilation \x1b[1mfinished\x1b[0m\n";
        let sanitized = TerminalSanitizer::sanitize(text);
        assert_eq!(sanitized, text);
    }

    #[test]
    fn test_strips_osc_52_clipboard_exploit() {
        // OSC 52 clipboard set sequence: \x1b]52;c;ZXhpdAo=\x07
        let attack = "Running test...\x1b]52;c;ZXhpdAo=\x07Completed.\n";
        let sanitized = TerminalSanitizer::sanitize(attack);
        assert_eq!(sanitized, "Running test...Completed.\n");
    }

    #[test]
    fn test_strips_osc_title_manipulation() {
        // Title setting: \x1b]0;Compromised Shell\x07
        let input = "\x1b]0;Compromised Shell\x07Hello world";
        let sanitized = TerminalSanitizer::sanitize(input);
        assert_eq!(sanitized, "Hello world");
    }

    #[test]
    fn test_strips_dangerous_csi_dsr_and_resets() {
        // DSR cursor query: \x1b[6n
        // RIS reset: \x1bc
        let input = "Safe\x1b[6nMiddle\x1bcEnd";
        let sanitized = TerminalSanitizer::sanitize(input);
        assert_eq!(sanitized, "SafeMiddleEnd");
    }

    #[test]
    fn test_strips_non_printable_control_bytes() {
        // 0x07 (BEL), 0x08 (BS), 0x0C (FF)
        let input = "Beep\x07Back\x08End";
        let sanitized = TerminalSanitizer::sanitize(input);
        assert_eq!(sanitized, "BeepBackEnd");
    }

    #[test]
    fn test_strip_all_mode() {
        let input = "\x1b[1;31mError:\x1b[0m file not found \x1b[2K\r";
        let plain = TerminalSanitizer::strip_all(input);
        assert_eq!(plain, "Error: file not found \r");
    }
}

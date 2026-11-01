//! ANSI escape code processing, tab expansion, and text alignment utilities for terminal rendering.
//!
//! Sanitizes escape sequences, expands tab characters to fixed spaces, and formats text for terminal layouts.

use std::borrow::Cow;

/// Expands tab characters (`\t`) to 4 spaces to ensure consistent gutter and column rendering.
pub fn expand_tabs(s: &str) -> Cow<'_, str> {
    if s.contains('\t') {
        Cow::Owned(s.replace('\t', "    "))
    } else {
        Cow::Borrowed(s)
    }
}

/// Strips standard ANSI CSI escape sequences (e.g. colors, bold, cursor moves) from a string.
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if let Some(&'[') = chars.peek() {
                chars.next(); // Consume '['
                // Consume parameters until a final command character (ASCII 0x40..=0x7E)
                while let Some(&next_c) = chars.peek() {
                    chars.next();
                    if ('@'..='~').contains(&next_c) {
                        break;
                    }
                }
            } else if let Some(&']') = chars.peek() {
                // OSC sequence, consume until BEL (\x07) or ST (\x1b\\)
                chars.next();
                for next_c in chars.by_ref() {
                    if next_c == '\x07' || next_c == '\x1b' {
                        break;
                    }
                }
            }
        } else {
            out.push(c);
        }
    }

    out
}

/// Computes the visual character length of a string ignoring invisible ANSI escape codes.
pub fn visual_len(s: &str) -> usize {
    strip_ansi(s).chars().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_tabs() {
        assert_eq!(expand_tabs("hello\tworld"), "hello    world");
        assert_eq!(expand_tabs("no_tabs"), "no_tabs");
    }

    #[test]
    fn test_strip_ansi_colors_and_bold() {
        let colored = "\x1b[31;1mRed Bold Text\x1b[0m normal \x1b[32mGreen\x1b[0m";
        let clean = strip_ansi(colored);
        assert_eq!(clean, "Red Bold Text normal Green");
        assert_eq!(visual_len(colored), 26);
    }

    #[test]
    fn test_strip_ansi_cursor_and_osc() {
        let with_osc = "\x1b]0;Title\x07Prompt: \x1b[2KDone";
        let clean = strip_ansi(with_osc);
        assert_eq!(clean, "Prompt: Done");
    }
}

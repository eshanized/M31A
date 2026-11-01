//! High-performance ANSI stripping and sanitization.

/// Strips all ANSI SGR color, bold, underline, OSC hyperlinks, and cursor movement escapes.
pub fn strip_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            if let Some(&next) = chars.peek() {
                if next == '[' {
                    // CSI sequence: \x1b[ ... [a-zA-Z~]
                    chars.next();
                    for c in chars.by_ref() {
                        if ('@'..='~').contains(&c) || c == 'm' {
                            break;
                        }
                    }
                } else if next == ']' {
                    // OSC sequence: \x1b] ... (\x07 | \x1b\\)
                    chars.next();
                    while let Some(c) = chars.next() {
                        if c == '\x07' {
                            break;
                        }
                        if c == '\x1b' && chars.peek() == Some(&'\\') {
                            chars.next();
                            break;
                        }
                    }
                } else {
                    chars.next();
                }
            }
        } else if ch != '\r' {
            out.push(ch);
        }
    }
    out
}

/// Expands tab characters to a specified tab width (default: 4 spaces).
pub fn expand_tabs(input: &str, tab_width: usize) -> String {
    let mut out = String::with_capacity(input.len());
    let mut col = 0;

    for ch in input.chars() {
        if ch == '\t' {
            let spaces = tab_width - (col % tab_width);
            for _ in 0..spaces {
                out.push(' ');
            }
            col += spaces;
        } else if ch == '\n' {
            out.push(ch);
            col = 0;
        } else {
            out.push(ch);
            col += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_ansi() {
        let text = "\x1b[32mHello\x1b[0m \x1b[1mWorld\x1b[0m! \x1b]8;;https://example.com\x1b\\link\x1b]8;;\x1b\\";
        assert_eq!(strip_ansi(text), "Hello World! link");
    }

    #[test]
    fn test_expand_tabs() {
        let text = "a\tb\tc";
        assert_eq!(expand_tabs(text, 4), "a   b   c");
    }
}

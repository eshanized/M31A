//! Terminal Query Interceptor and Automatic Responder for Sandboxed PTY Processes.
//!
//! Captures and answers blocking ANSI terminal queries (Device Status Reports, Cursor
//! Position inquiries, Window Size requests, and DEC Private Mode queries) from
//! untrusted child processes executing in non-interactive or headless PTY sessions,
//! preventing processes (such as editors, pagers, or CLI tools) from hanging indefinitely
//! waiting for a physical user terminal.

const MAX_MODE_DIGITS: usize = 10;
const MAX_QUERY_BYTES: usize = MAX_MODE_DIGITS + 5;

// Query sequence -> automatic synthetic terminal response
const QUERY_RESPONSES: [(&[u8], &[u8]); 3] = [
    // Device status report (DSR): report operating normally (0n)
    (b"\x1b[5n", b"\x1b[0n"),
    // Window size query: report standard 24 rows, 80 columns (8;24;80t)
    (b"\x1b[18t", b"\x1b[8;24;80t"),
    // Cursor position report (CPR): return row 1, column 1 (1;1R)
    (b"\x1b[6n", b"\x1b[1;1R"),
];

/// State machine for intercepting terminal queries in streaming stdout.
#[derive(Debug, Default, Clone)]
pub struct TerminalQueryResponder {
    pending: Vec<u8>,
}

impl TerminalQueryResponder {
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
        }
    }

    /// Process a stream chunk from child stdout.
    /// Returns `(filtered_stdout, replies_to_write_to_stdin)`.
    pub fn process(&mut self, bytes: &[u8]) -> (Vec<u8>, Vec<u8>) {
        if self.pending.is_empty() && !bytes.contains(&b'\x1b') {
            return (bytes.to_vec(), Vec::new());
        }

        let mut output = Vec::with_capacity(bytes.len());
        let mut responses = Vec::new();

        for &byte in bytes {
            if self.pending.is_empty() && byte != b'\x1b' {
                output.push(byte);
                continue;
            }

            if byte == b'\x1b' && !self.pending.is_empty() {
                output.append(&mut self.pending);
            }
            self.pending.push(byte);

            // If we are in the middle of an escape sequence
            if self.pending.len() == 1
                || self.pending.as_slice() == b"\x1b["
                || (self.pending.len() > 1
                    && self.pending[1] == b'['
                    && !(0x40..=0x7e).contains(&byte)
                    && self.pending.len() < MAX_QUERY_BYTES)
            {
                continue;
            }

            // Check against fixed query table
            if let Some((_, response)) = QUERY_RESPONSES
                .iter()
                .find(|(query, _)| *query == self.pending.as_slice())
            {
                responses.extend_from_slice(response);
            } else if self.pending.starts_with(b"\x1b[?") && self.pending.ends_with(b"$p") {
                // DEC private-mode query: \x1b[?<digits>$p
                let mode = &self.pending[3..self.pending.len() - 2];
                if !mode.is_empty()
                    && mode.len() <= MAX_MODE_DIGITS
                    && mode.iter().all(u8::is_ascii_digit)
                {
                    // Report mode unrecognized: \x1b[?<digits>;0$y
                    responses.extend_from_slice(b"\x1b[?");
                    responses.extend_from_slice(mode);
                    responses.extend_from_slice(b";0$y");
                } else {
                    output.append(&mut self.pending);
                }
            } else {
                // Not a recognized terminal query, emit as regular output
                output.append(&mut self.pending);
            }

            self.pending.clear();
        }

        (output, responses)
    }

    /// Flush any remaining pending bytes upon stream EOF.
    pub fn finish(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.pending)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plain_text_passthrough() {
        let mut responder = TerminalQueryResponder::new();
        let (out, replies) = responder.process(b"hello world\n");
        assert_eq!(out, b"hello world\n");
        assert!(replies.is_empty());
    }

    #[test]
    fn test_cursor_position_query_intercepted_and_answered() {
        let mut responder = TerminalQueryResponder::new();
        // Child emits cursor position query \x1b[6n
        let input = b"prefix \x1b[6n suffix";
        let (out, replies) = responder.process(input);

        // Query must be stripped from output
        assert_eq!(out, b"prefix  suffix");
        // Reply row 1, col 1 must be sent to child stdin
        assert_eq!(replies, b"\x1b[1;1R");
    }

    #[test]
    fn test_device_status_report_and_window_size() {
        let mut responder = TerminalQueryResponder::new();

        // Device Status Report (\x1b[5n)
        let (out1, rep1) = responder.process(b"\x1b[5n");
        assert!(out1.is_empty());
        assert_eq!(rep1, b"\x1b[0n");

        // Window Size (\x1b[18t)
        let (out2, rep2) = responder.process(b"\x1b[18t");
        assert!(out2.is_empty());
        assert_eq!(rep2, b"\x1b[8;24;80t");
    }

    #[test]
    fn test_dec_private_mode_query() {
        let mut responder = TerminalQueryResponder::new();
        // Query DEC private mode 25 (cursor visibility)
        let (out, replies) = responder.process(b"\x1b[?25$p");
        assert!(out.is_empty());
        assert_eq!(replies, b"\x1b[?25;0$y");
    }

    #[test]
    fn test_split_across_chunk_boundaries() {
        let mut responder = TerminalQueryResponder::new();
        let (out1, rep1) = responder.process(b"status: \x1b[");
        assert_eq!(out1, b"status: ");
        assert!(rep1.is_empty());

        let (out2, rep2) = responder.process(b"6n and more");
        assert_eq!(out2, b" and more");
        assert_eq!(rep2, b"\x1b[1;1R");
    }
}

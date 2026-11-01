//! Bounded head-tail stream buffer and output truncation engine.
//!
//! Preserves a stable head prefix and the most recent sliding tail suffix,
//! dropping the middle once the configured byte or token budget is exceeded.

use std::collections::VecDeque;

/// Default format for omitted bytes marker.
pub fn format_output_omission_marker(omitted_bytes: usize) -> String {
    format!("... [{} bytes omitted] ...", omitted_bytes)
}

/// A compile-time bounded buffer that preserves a stable head and sliding tail,
/// symmetrically allocating capacity (50% head, 50% tail).
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct HeadTailBuffer<const MAX_BYTES: usize> {
    head: Vec<u8>,
    tail: VecDeque<u8>,
    omitted_bytes: usize,
}

impl<const MAX_BYTES: usize> Default for HeadTailBuffer<MAX_BYTES> {
    fn default() -> Self {
        Self {
            head: Vec::new(),
            tail: VecDeque::new(),
            omitted_bytes: 0,
        }
    }
}

impl<const MAX_BYTES: usize> HeadTailBuffer<MAX_BYTES> {
    pub const HEAD_BUDGET: usize = MAX_BYTES / 2;
    pub const TAIL_BUDGET: usize = MAX_BYTES.saturating_sub(Self::HEAD_BUDGET);

    pub fn new() -> Self {
        Self::default()
    }

    /// Total bytes currently retained in head + tail.
    pub fn retained_bytes(&self) -> usize {
        self.head.len().saturating_add(self.tail.len())
    }

    /// Total bytes dropped from the middle due to capacity limits.
    pub fn omitted_bytes(&self) -> usize {
        self.omitted_bytes
    }

    /// Total bytes observed (retained + omitted).
    pub fn total_bytes(&self) -> usize {
        self.retained_bytes().saturating_add(self.omitted_bytes)
    }

    /// Append a slice of bytes to the buffer.
    pub fn push_chunk(&mut self, chunk: &[u8]) {
        let remaining_head = Self::HEAD_BUDGET.saturating_sub(self.head.len());
        let (head_slice, tail_slice) = if remaining_head > 0 {
            if chunk.len() <= remaining_head {
                (chunk, &[][..])
            } else {
                chunk.split_at(remaining_head)
            }
        } else {
            (&[][..], chunk)
        };

        if !head_slice.is_empty() {
            self.head.extend_from_slice(head_slice);
        }

        if !tail_slice.is_empty() {
            let remaining_tail = Self::TAIL_BUDGET.saturating_sub(self.tail.len());
            let excess_tail = tail_slice.len().saturating_sub(remaining_tail);
            self.omitted_bytes = self.omitted_bytes.saturating_add(excess_tail);

            let chunk_to_add = match excess_tail.checked_sub(self.tail.len()) {
                None => {
                    self.tail.drain(..excess_tail);
                    tail_slice
                }
                Some(skip) => {
                    self.tail.clear();
                    &tail_slice[skip..]
                }
            };
            self.tail.extend(chunk_to_add);
        }
    }

    /// Return the retained buffer as a single contiguous vector of bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.retained_bytes());
        out.extend_from_slice(&self.head);
        out.extend(self.tail.iter().copied());
        out
    }

    /// Return the retained buffer with an omission marker if bytes were dropped.
    pub fn to_bytes_with_omission_marker(&self) -> Vec<u8> {
        if self.omitted_bytes == 0 {
            return self.to_bytes();
        }

        let marker = format_output_omission_marker(self.omitted_bytes);
        let mut out = Vec::with_capacity(self.retained_bytes() + marker.len() + 2);
        out.extend_from_slice(&self.head);
        if !self.head.ends_with(b"\n") {
            out.push(b'\n');
        }
        out.extend_from_slice(marker.as_bytes());
        out.push(b'\n');
        out.extend(self.tail.iter().copied());
        out
    }

    /// Merge another buffer into this one, preserving total omission count.
    pub fn push_buffer(&mut self, other: Self) {
        self.omitted_bytes = self.omitted_bytes.saturating_add(other.omitted_bytes);
        self.push_chunk(&other.head);
        let tail_bytes: Vec<u8> = other.tail.into_iter().collect();
        self.push_chunk(&tail_bytes);
    }
}

/// A dynamically sized buffer that supports configurable head/tail budget ratios
/// and line-boundary snapping to avoid splitting code/log lines in half.
#[derive(Debug, Clone)]
pub struct DynamicHeadTailBuffer {
    max_bytes: usize,
    head_budget: usize,
    tail_budget: usize,
    head: Vec<u8>,
    tail: VecDeque<u8>,
    omitted_bytes: usize,
}

impl DynamicHeadTailBuffer {
    /// Create a new dynamic buffer with a given capacity and head ratio (e.g. 0.5 for 50/50).
    pub fn new(max_bytes: usize, head_ratio: f32) -> Self {
        let ratio = head_ratio.clamp(0.1, 0.9);
        let head_budget = ((max_bytes as f32) * ratio).round() as usize;
        let tail_budget = max_bytes.saturating_sub(head_budget);

        Self {
            max_bytes,
            head_budget,
            tail_budget,
            head: Vec::new(),
            tail: VecDeque::new(),
            omitted_bytes: 0,
        }
    }

    pub fn push_chunk(&mut self, chunk: &[u8]) {
        let remaining_head = self.head_budget.saturating_sub(self.head.len());
        let (head_slice, tail_slice) = if remaining_head > 0 {
            if chunk.len() <= remaining_head {
                (chunk, &[][..])
            } else {
                chunk.split_at(remaining_head)
            }
        } else {
            (&[][..], chunk)
        };

        if !head_slice.is_empty() {
            self.head.extend_from_slice(head_slice);
        }

        if !tail_slice.is_empty() {
            let remaining_tail = self.tail_budget.saturating_sub(self.tail.len());
            let excess_tail = tail_slice.len().saturating_sub(remaining_tail);
            self.omitted_bytes = self.omitted_bytes.saturating_add(excess_tail);

            let chunk_to_add = match excess_tail.checked_sub(self.tail.len()) {
                None => {
                    self.tail.drain(..excess_tail);
                    tail_slice
                }
                Some(skip) => {
                    self.tail.clear();
                    &tail_slice[skip..]
                }
            };
            self.tail.extend(chunk_to_add);
        }
    }

    pub fn retained_bytes(&self) -> usize {
        self.head.len().saturating_add(self.tail.len())
    }

    pub fn omitted_bytes(&self) -> usize {
        self.omitted_bytes
    }

    pub fn max_bytes(&self) -> usize {
        self.max_bytes
    }

    /// Render to string lossy, inserting an omission marker if truncated.
    pub fn to_string_lossy_with_marker(&self) -> String {
        if self.omitted_bytes == 0 {
            let mut bytes = Vec::with_capacity(self.retained_bytes());
            bytes.extend_from_slice(&self.head);
            bytes.extend(self.tail.iter().copied());
            return String::from_utf8_lossy(&bytes).into_owned();
        }

        let marker = format_output_omission_marker(self.omitted_bytes);
        let head_str = String::from_utf8_lossy(&self.head);
        let tail_vec: Vec<u8> = self.tail.iter().copied().collect();
        let tail_str = String::from_utf8_lossy(&tail_vec);

        let head_trimmed = head_str.trim_end_matches('\n');
        let tail_trimmed = tail_str.trim_start_matches('\n');

        format!("{}\n{}\n{}", head_trimmed, marker, tail_trimmed)
    }

    /// Render to string lossy, snapping head and tail to line breaks where possible
    /// so that partial terminal or log lines are not presented as corrupt output.
    pub fn to_string_lossy_snapped_to_lines(&self) -> String {
        if self.omitted_bytes == 0 {
            return self.to_string_lossy_with_marker();
        }

        let head_str = String::from_utf8_lossy(&self.head);
        let tail_vec: Vec<u8> = self.tail.iter().copied().collect();
        let tail_str = String::from_utf8_lossy(&tail_vec);

        // Snap head back to last newline
        let snapped_head = match head_str.rfind('\n') {
            Some(idx) if idx > 0 => &head_str[..idx],
            _ => &head_str,
        };

        // Snap tail forward to first newline
        let snapped_tail = match tail_str.find('\n') {
            Some(idx) if idx + 1 < tail_str.len() => &tail_str[idx + 1..],
            _ => &tail_str,
        };

        let marker = format_output_omission_marker(self.omitted_bytes);
        format!(
            "{}\n{}\n{}",
            snapped_head.trim_end(),
            marker,
            snapped_tail.trim_start()
        )
    }
}

/// Truncation policy specifying byte or token budgeting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TruncationPolicy {
    Bytes(usize),
    Tokens(usize),
}

impl TruncationPolicy {
    pub fn byte_budget(&self) -> usize {
        match *self {
            TruncationPolicy::Bytes(b) => b,
            TruncationPolicy::Tokens(t) => t.saturating_mul(4),
        }
    }
}

/// Approximate token count using 4 characters per token heuristic.
pub fn approx_token_count(text: &str) -> usize {
    if text.is_empty() {
        0
    } else {
        text.len().div_ceil(4)
    }
}

/// Truncate text according to a policy, keeping head and tail.
pub fn truncate_text_with_policy(text: &str, policy: TruncationPolicy) -> (String, bool) {
    let max_bytes = policy.byte_budget();
    if text.len() <= max_bytes {
        return (text.to_string(), false);
    }

    let mut buf = DynamicHeadTailBuffer::new(max_bytes, 0.5);
    buf.push_chunk(text.as_bytes());
    (buf.to_string_lossy_snapped_to_lines(), true)
}

/// Truncate tool output with warning prefix if oversized.
pub fn truncate_tool_output(output: &str, max_bytes: usize) -> String {
    if output.len() <= max_bytes {
        return output.to_string();
    }

    let original_tokens = approx_token_count(output);
    let total_lines = output.lines().count();
    let (truncated, was_truncated) =
        truncate_text_with_policy(output, TruncationPolicy::Bytes(max_bytes));

    if was_truncated {
        format!(
            "Warning: output truncated (original tokens: ~{}, total lines: {})\n\n{}",
            original_tokens, total_lines, truncated
        )
    } else {
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_head_tail_buffer_within_budget() {
        let mut buf = HeadTailBuffer::<64>::new();
        buf.push_chunk(b"hello world");
        assert_eq!(buf.retained_bytes(), 11);
        assert_eq!(buf.omitted_bytes(), 0);
        assert_eq!(buf.to_bytes(), b"hello world");
        assert_eq!(buf.to_bytes_with_omission_marker(), b"hello world");
    }

    #[test]
    fn test_head_tail_buffer_exceeding_budget() {
        let mut buf = HeadTailBuffer::<20>::new();
        // Head budget: 10, Tail budget: 10
        buf.push_chunk(b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ");
        assert_eq!(buf.head, b"0123456789");
        assert_eq!(buf.tail.iter().copied().collect::<Vec<u8>>(), b"QRSTUVWXYZ");
        assert_eq!(buf.omitted_bytes(), 16);

        let formatted = String::from_utf8_lossy(&buf.to_bytes_with_omission_marker()).into_owned();
        assert!(formatted.starts_with("0123456789"));
        assert!(formatted.contains("... [16 bytes omitted] ..."));
        assert!(formatted.ends_with("QRSTUVWXYZ"));
    }

    #[test]
    fn test_head_tail_buffer_merge() {
        let mut b1 = HeadTailBuffer::<16>::new();
        b1.push_chunk(b"head-1234");

        let mut b2 = HeadTailBuffer::<16>::new();
        b2.push_chunk(b"tail-5678");

        b1.push_buffer(b2);
        assert!(b1.omitted_bytes() > 0 || b1.retained_bytes() <= 16);
    }

    #[test]
    fn test_dynamic_head_tail_snapped_to_lines() {
        let input = "Line 1: init\nLine 2: setup\nLine 3: middle A\nLine 4: middle B\nLine 5: middle C\nLine 6: finish\nLine 7: done";
        let mut buf = DynamicHeadTailBuffer::new(50, 0.5);
        buf.push_chunk(input.as_bytes());

        let snapped = buf.to_string_lossy_snapped_to_lines();
        assert!(snapped.contains("... ["));
        assert!(snapped.contains("Line 1: init"));
        assert!(snapped.contains("Line 7: done"));
    }

    #[test]
    fn test_truncate_tool_output() {
        let long_output = "INFO starting build\n".to_string()
            + &"DEBUG trace step\n".repeat(100)
            + "SUCCESS build completed in 1.2s\n";
        let truncated = truncate_tool_output(&long_output, 200);
        assert!(truncated.starts_with("Warning: output truncated"));
        assert!(truncated.contains("INFO starting build"));
        assert!(truncated.contains("SUCCESS build completed in 1.2s"));
    }
}

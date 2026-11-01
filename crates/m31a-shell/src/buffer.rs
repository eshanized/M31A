//! Head-tail buffer truncation for shell command outputs.

#[derive(Debug, Clone)]
pub struct HeadTailBuffer {
    max_head_lines: usize,
    max_tail_lines: usize,
    max_total_bytes: usize,
}

impl Default for HeadTailBuffer {
    fn default() -> Self {
        Self {
            max_head_lines: 200,
            max_tail_lines: 300,
            max_total_bytes: 256 * 1024, // 256 KB
        }
    }
}

impl HeadTailBuffer {
    pub fn new(max_head_lines: usize, max_tail_lines: usize, max_total_bytes: usize) -> Self {
        Self {
            max_head_lines,
            max_tail_lines,
            max_total_bytes,
        }
    }

    /// Truncates raw output lines, retaining the head and tail if the total exceeds budget.
    pub fn process(&self, raw: &str) -> String {
        if raw.len() <= self.max_total_bytes {
            let lines: Vec<&str> = raw.lines().collect();
            if lines.len() <= (self.max_head_lines + self.max_tail_lines) {
                return raw.to_string();
            }
        }

        let lines: Vec<&str> = raw.lines().collect();
        let total_lines = lines.len();

        if total_lines <= (self.max_head_lines + self.max_tail_lines) {
            // Lines are within budget, but bytes might exceed; truncate tail by byte
            let mut out = lines.join("\n");
            if out.len() > self.max_total_bytes {
                out.truncate(self.max_total_bytes);
                out.push_str("\n... [output truncated by byte budget] ...");
            }
            return out;
        }

        let head = &lines[..self.max_head_lines];
        let tail = &lines[total_lines - self.max_tail_lines..];
        let truncated_count = total_lines - self.max_head_lines - self.max_tail_lines;

        let mut result = String::new();
        result.push_str(&head.join("\n"));
        result.push_str(&format!(
            "\n\n... [{} lines truncated to preserve token context] ...\n\n",
            truncated_count
        ));
        result.push_str(&tail.join("\n"));

        if raw.ends_with('\n') {
            result.push('\n');
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_head_tail_truncation() {
        let buffer = HeadTailBuffer::new(5, 5, 1024 * 1024);
        let mut raw = String::new();
        for i in 1..=50 {
            raw.push_str(&format!("Line {}\n", i));
        }

        let processed = buffer.process(&raw);
        assert!(processed.contains("Line 1"));
        assert!(processed.contains("Line 5"));
        assert!(processed.contains("40 lines truncated"));
        assert!(processed.contains("Line 46"));
        assert!(processed.contains("Line 50"));
    }
}

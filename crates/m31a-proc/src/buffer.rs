//! Circular bounded output buffer for background processes.
//!
//! Retains process standard output and standard error lines in memory with head-tail truncation.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputStream {
    Stdout,
    Stderr,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessLine {
    pub index: usize,
    pub timestamp: DateTime<Utc>,
    pub stream: OutputStream,
    pub line: String,
}

#[derive(Debug, Clone)]
pub struct CircularOutputBuffer {
    max_capacity: usize,
    buffer: VecDeque<ProcessLine>,
    total_lines_pushed: usize,
}

impl CircularOutputBuffer {
    pub fn new(max_capacity: usize) -> Self {
        Self {
            max_capacity: if max_capacity == 0 {
                1000
            } else {
                max_capacity
            },
            buffer: VecDeque::with_capacity(max_capacity.min(1000)),
            total_lines_pushed: 0,
        }
    }

    pub fn push(&mut self, stream: OutputStream, line: String) {
        let index = self.total_lines_pushed;
        self.total_lines_pushed += 1;

        if self.buffer.len() >= self.max_capacity {
            self.buffer.pop_front();
        }

        self.buffer.push_back(ProcessLine {
            index,
            timestamp: Utc::now(),
            stream,
            line,
        });
    }

    pub fn total_lines(&self) -> usize {
        self.total_lines_pushed
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    pub fn tail(&self, limit: usize) -> Vec<ProcessLine> {
        let count = limit.min(self.buffer.len());
        let start = self.buffer.len() - count;
        self.buffer.iter().skip(start).cloned().collect()
    }

    pub fn slice(&self, offset: usize, limit: usize) -> (Vec<ProcessLine>, usize) {
        let total = self.total_lines_pushed;
        if self.buffer.is_empty() {
            return (Vec::new(), total);
        }

        let items: Vec<ProcessLine> = self
            .buffer
            .iter()
            .filter(|l| l.index >= offset)
            .take(limit)
            .cloned()
            .collect();

        (items, total)
    }

    pub fn as_text(&self, limit: usize) -> String {
        let lines = self.tail(limit);
        let mut out = String::new();
        for l in lines {
            let prefix = match l.stream {
                OutputStream::Stdout => "",
                OutputStream::Stderr => "[stderr] ",
                OutputStream::System => "[sys] ",
            };
            out.push_str(prefix);
            out.push_str(&l.line);
            out.push('\n');
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circular_output_buffer() {
        let mut buf = CircularOutputBuffer::new(5);
        assert_eq!(buf.len(), 0);

        for i in 0..10 {
            buf.push(OutputStream::Stdout, format!("Line {}", i));
        }

        assert_eq!(buf.len(), 5);
        assert_eq!(buf.total_lines(), 10);

        let tail = buf.tail(3);
        assert_eq!(tail.len(), 3);
        assert_eq!(tail[0].line, "Line 7");
        assert_eq!(tail[1].line, "Line 8");
        assert_eq!(tail[2].line, "Line 9");

        let (slice, total) = buf.slice(6, 2);
        assert_eq!(total, 10);
        assert_eq!(slice.len(), 2);
        assert_eq!(slice[0].line, "Line 6");
        assert_eq!(slice[1].line, "Line 7");
    }
}

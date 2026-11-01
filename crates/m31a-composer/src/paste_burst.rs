//! Paste burst detection and compact pill widget generation.

use std::time::Instant;

const RAPID_CHAR_INTERVAL_MS: u128 = 25;
const LARGE_PASTE_LINE_THRESHOLD: usize = 4;
const LARGE_PASTE_CHAR_THRESHOLD: usize = 160;

#[derive(Debug, Clone)]
pub struct PastedSnippetPill {
    pub id: String,
    pub title: String,
    pub line_count: usize,
    pub preview: String,
    pub full_content: String,
}

#[derive(Debug)]
pub struct PasteBurstDetector {
    buffer: String,
    last_char_time: Option<Instant>,
    consecutive_chars: usize,
    in_bracketed_paste: bool,
}

impl Default for PasteBurstDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl PasteBurstDetector {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            last_char_time: None,
            consecutive_chars: 0,
            in_bracketed_paste: false,
        }
    }

    pub fn start_bracketed_paste(&mut self) {
        self.in_bracketed_paste = true;
        self.buffer.clear();
    }

    pub fn push_bracketed_str(&mut self, text: &str) {
        if self.in_bracketed_paste {
            self.buffer.push_str(text);
        }
    }

    pub fn end_bracketed_paste(&mut self) -> Option<PastedSnippetPill> {
        self.in_bracketed_paste = false;
        let content = std::mem::take(&mut self.buffer);
        self.evaluate_snippet(&content)
    }

    pub fn on_char(&mut self, ch: char) -> Option<PastedSnippetPill> {
        if self.in_bracketed_paste {
            self.buffer.push(ch);
            return None;
        }

        let now = Instant::now();

        if let Some(last) = self.last_char_time
            && now.duration_since(last).as_millis() <= RAPID_CHAR_INTERVAL_MS
        {
            self.consecutive_chars += 1;
            self.buffer.push(ch);
            self.last_char_time = Some(now);
            return None;
        }

        // New burst or normal typing
        self.last_char_time = Some(now);
        self.consecutive_chars = 1;
        None
    }

    pub fn flush_if_large_paste(&mut self, text: &str) -> Option<PastedSnippetPill> {
        self.evaluate_snippet(text)
    }

    fn evaluate_snippet(&self, content: &str) -> Option<PastedSnippetPill> {
        let lines: Vec<&str> = content.lines().collect();
        let line_count = lines.len();

        if line_count >= LARGE_PASTE_LINE_THRESHOLD || content.len() >= LARGE_PASTE_CHAR_THRESHOLD {
            let first_line = lines.first().unwrap_or(&"Code Snippet").trim();
            let title = if first_line.is_empty() {
                "Pasted Code".to_string()
            } else {
                first_line.chars().take(30).collect::<String>()
            };

            let preview = lines
                .iter()
                .take(3)
                .cloned()
                .collect::<Vec<&str>>()
                .join("\n");

            Some(PastedSnippetPill {
                id: uuid::Uuid::new_v4().to_string(),
                title,
                line_count,
                preview,
                full_content: content.to_string(),
            })
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evaluate_snippet() {
        let mut detector = PasteBurstDetector::new();
        let multiline = "fn main() {\n    let a = 1;\n    let b = 2;\n    let c = 3;\n    println!(\"{}\", a + b + c);\n}\n";
        let pill = detector.flush_if_large_paste(multiline);
        assert!(pill.is_some());
        let p = pill.unwrap();
        assert_eq!(p.line_count, 6);
        assert!(p.full_content.contains("println!"));
    }
}

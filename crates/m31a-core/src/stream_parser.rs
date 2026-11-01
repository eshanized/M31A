//! Streaming text, UTF-8 multi-byte boundary, and inline hidden tag parser.
//!
//! Handles fragmented UTF-8 code points split across network packet boundaries
//! and withholds/extracts hidden XML tags (e.g. `<thought>`, `<citation>`, `<proposed_plan>`)
//! from assistant streaming chunks before visible text is emitted to the user.

use std::error::Error;
use std::fmt;

/// A chunk of streamed text containing visible text and extracted hidden items.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamTextChunk<T> {
    pub visible_text: String,
    pub extracted: Vec<T>,
}

impl<T> Default for StreamTextChunk<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> StreamTextChunk<T> {
    pub fn new() -> Self {
        Self {
            visible_text: String::new(),
            extracted: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.visible_text.is_empty() && self.extracted.is_empty()
    }
}

/// Trait for text stream parsers.
pub trait StreamTextParser {
    type Extracted;

    /// Feed a string slice and receive visible text + extracted items.
    fn push_str(&mut self, chunk: &str) -> StreamTextChunk<Self::Extracted>;

    /// Flush any remaining buffered content at end-of-stream.
    fn finish(&mut self) -> StreamTextChunk<Self::Extracted>;
}

/// Errors occurring during UTF-8 stream parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Utf8StreamParserError {
    InvalidUtf8 {
        valid_up_to: usize,
        error_len: usize,
    },
    IncompleteUtf8AtEof,
}

impl fmt::Display for Utf8StreamParserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUtf8 {
                valid_up_to,
                error_len,
            } => {
                write!(
                    f,
                    "Invalid UTF-8 at offset {} (length {})",
                    valid_up_to, error_len
                )
            }
            Self::IncompleteUtf8AtEof => write!(f, "Incomplete UTF-8 sequence at end of stream"),
        }
    }
}

impl Error for Utf8StreamParserError {}

/// Wraps any `StreamTextParser` and accepts raw byte slices, safely buffering
/// partial multi-byte code points split across chunk boundaries.
#[derive(Debug)]
pub struct Utf8StreamParser<P> {
    inner: P,
    pending_bytes: Vec<u8>,
}

impl<P: StreamTextParser> Utf8StreamParser<P> {
    pub fn new(inner: P) -> Self {
        Self {
            inner,
            pending_bytes: Vec::new(),
        }
    }

    pub fn push_bytes(
        &mut self,
        bytes: &[u8],
    ) -> Result<StreamTextChunk<P::Extracted>, Utf8StreamParserError> {
        let old_len = self.pending_bytes.len();
        self.pending_bytes.extend_from_slice(bytes);

        match std::str::from_utf8(&self.pending_bytes) {
            Ok(valid_str) => {
                let out = self.inner.push_str(valid_str);
                self.pending_bytes.clear();
                Ok(out)
            }
            Err(err) => {
                if let Some(error_len) = err.error_len() {
                    self.pending_bytes.truncate(old_len);
                    return Err(Utf8StreamParserError::InvalidUtf8 {
                        valid_up_to: err.valid_up_to(),
                        error_len,
                    });
                }

                let valid_up_to = err.valid_up_to();
                if valid_up_to == 0 {
                    return Ok(StreamTextChunk::default());
                }

                let valid_str = match std::str::from_utf8(&self.pending_bytes[..valid_up_to]) {
                    Ok(s) => s,
                    Err(_) => {
                        self.pending_bytes.truncate(old_len);
                        return Err(Utf8StreamParserError::InvalidUtf8 {
                            valid_up_to: 0,
                            error_len: 1,
                        });
                    }
                };

                let out = self.inner.push_str(valid_str);
                self.pending_bytes.drain(..valid_up_to);
                Ok(out)
            }
        }
    }

    pub fn finish(&mut self) -> Result<StreamTextChunk<P::Extracted>, Utf8StreamParserError> {
        if !self.pending_bytes.is_empty() {
            return Err(Utf8StreamParserError::IncompleteUtf8AtEof);
        }
        Ok(self.inner.finish())
    }
}

/// An inline hidden tag spec definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineTagSpec<T> {
    pub tag: T,
    pub open: &'static str,
    pub close: &'static str,
}

/// An extracted tag event with its classification and inner content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedInlineTag<T> {
    pub tag: T,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ActiveTagState<T> {
    tag: T,
    close: &'static str,
    content: String,
}

/// Generic streaming parser that withholds configured inline XML tags
/// and extracts their inner contents while streaming visible text.
#[derive(Debug, Clone)]
pub struct InlineHiddenTagParser<T: Clone + Eq> {
    specs: Vec<InlineTagSpec<T>>,
    pending: String,
    active: Option<ActiveTagState<T>>,
}

impl<T: Clone + Eq> InlineHiddenTagParser<T> {
    pub fn new(specs: Vec<InlineTagSpec<T>>) -> Self {
        assert!(!specs.is_empty(), "Must provide at least one InlineTagSpec");
        Self {
            specs,
            pending: String::new(),
            active: None,
        }
    }

    fn longest_partial_open_suffix(pending: &str, open: &str) -> usize {
        let max_check = pending.len().min(open.len() - 1);
        for len in (1..=max_check).rev() {
            if pending.ends_with(&open[..len]) {
                return len;
            }
        }
        0
    }

    fn longest_partial_close_suffix(pending: &str, close: &str) -> usize {
        let max_check = pending.len().min(close.len() - 1);
        for len in (1..=max_check).rev() {
            if pending.ends_with(&close[..len]) {
                return len;
            }
        }
        0
    }
}

impl<T: Clone + Eq> StreamTextParser for InlineHiddenTagParser<T> {
    type Extracted = ExtractedInlineTag<T>;

    fn push_str(&mut self, chunk: &str) -> StreamTextChunk<Self::Extracted> {
        self.pending.push_str(chunk);
        let mut out = StreamTextChunk::new();

        loop {
            if let Some(active) = &mut self.active {
                // Currently accumulating inside an open tag: look for close delimiter
                if let Some(pos) = self.pending.find(active.close) {
                    active.content.push_str(&self.pending[..pos]);
                    let tag_content = std::mem::take(&mut active.content);
                    let tag_kind = active.tag.clone();
                    let close_len = active.close.len();

                    out.extracted.push(ExtractedInlineTag {
                        tag: tag_kind,
                        content: tag_content,
                    });

                    self.pending.drain(..pos + close_len);
                    self.active = None;
                } else {
                    // Check if the end of pending could be the start of the close tag
                    let keep = Self::longest_partial_close_suffix(&self.pending, active.close);
                    let emit_len = self.pending.len() - keep;
                    if emit_len > 0 {
                        active.content.push_str(&self.pending[..emit_len]);
                        self.pending.drain(..emit_len);
                    }
                    break;
                }
            } else {
                // Not inside a tag: look for any open delimiter
                let mut best_match: Option<(usize, usize)> = None; // (pos, spec_idx)

                for (idx, spec) in self.specs.iter().enumerate() {
                    if let Some(pos) = self.pending.find(spec.open) {
                        match best_match {
                            None => best_match = Some((pos, idx)),
                            Some((best_pos, _)) if pos < best_pos => best_match = Some((pos, idx)),
                            _ => {}
                        }
                    }
                }

                if let Some((pos, spec_idx)) = best_match {
                    let spec = &self.specs[spec_idx];
                    out.visible_text.push_str(&self.pending[..pos]);
                    let open_len = spec.open.len();
                    self.pending.drain(..pos + open_len);

                    self.active = Some(ActiveTagState {
                        tag: spec.tag.clone(),
                        close: spec.close,
                        content: String::new(),
                    });
                } else {
                    // No complete open tag found: check for partial open tag prefix at the end of pending
                    let mut max_keep = 0;
                    for spec in &self.specs {
                        let keep = Self::longest_partial_open_suffix(&self.pending, spec.open);
                        if keep > max_keep {
                            max_keep = keep;
                        }
                    }

                    let emit_len = self.pending.len() - max_keep;
                    if emit_len > 0 {
                        out.visible_text.push_str(&self.pending[..emit_len]);
                        self.pending.drain(..emit_len);
                    }
                    break;
                }
            }
        }

        out
    }

    fn finish(&mut self) -> StreamTextChunk<Self::Extracted> {
        let mut out = StreamTextChunk::new();
        if let Some(active) = self.active.take() {
            let mut final_content = active.content;
            final_content.push_str(&self.pending);
            out.extracted.push(ExtractedInlineTag {
                tag: active.tag,
                content: final_content,
            });
        } else if !self.pending.is_empty() {
            out.visible_text.push_str(&self.pending);
        }
        self.pending.clear();
        out
    }
}

/// Plain text pass-through stream parser.
#[derive(Debug, Default, Clone)]
pub struct PlainTextStreamParser;

impl StreamTextParser for PlainTextStreamParser {
    type Extracted = ();

    fn push_str(&mut self, chunk: &str) -> StreamTextChunk<Self::Extracted> {
        StreamTextChunk {
            visible_text: chunk.to_string(),
            extracted: Vec::new(),
        }
    }

    fn finish(&mut self) -> StreamTextChunk<Self::Extracted> {
        StreamTextChunk::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum HiddenTag {
        Thought,
        Plan,
    }

    #[test]
    fn test_utf8_split_multi_byte_code_point() {
        let mut parser = Utf8StreamParser::new(PlainTextStreamParser);

        // 'é' is 0xC3 0xA9 in UTF-8
        let chunk1 = [0x61, 0x62, 0xC3]; // "ab" + first byte of 'é'
        let chunk2 = [0xA9, 0x63]; // second byte of 'é' + "c"

        let out1 = parser.push_bytes(&chunk1).unwrap();
        assert_eq!(out1.visible_text, "ab");

        let out2 = parser.push_bytes(&chunk2).unwrap();
        assert_eq!(out2.visible_text, "éc");

        let out_fin = parser.finish().unwrap();
        assert!(out_fin.is_empty());
    }

    #[test]
    fn test_inline_hidden_tag_parser() {
        let specs = vec![
            InlineTagSpec {
                tag: HiddenTag::Thought,
                open: "<thought>",
                close: "</thought>",
            },
            InlineTagSpec {
                tag: HiddenTag::Plan,
                open: "<plan>",
                close: "</plan>",
            },
        ];

        let mut parser = InlineHiddenTagParser::new(specs);

        // Feed chunks split across tag boundaries
        let mut out = parser.push_str("Hello world! <thou");
        assert_eq!(out.visible_text, "Hello world! ");
        assert!(out.extracted.is_empty());

        let out2 = parser.push_str("ght>Deep internal reasoning</thought> And here is the answer.");
        out.visible_text.push_str(&out2.visible_text);
        out.extracted.extend(out2.extracted);

        assert_eq!(out.visible_text, "Hello world!  And here is the answer.");
        assert_eq!(
            out.extracted,
            vec![ExtractedInlineTag {
                tag: HiddenTag::Thought,
                content: "Deep internal reasoning".to_string(),
            }]
        );
    }
}

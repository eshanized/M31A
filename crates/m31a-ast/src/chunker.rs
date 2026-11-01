//! AST-directed semantic code chunker.
//!
//! Slices source files along structural AST boundaries (classes, functions, impl blocks)
//! rather than arbitrary line counts, ensuring LLM context blocks never truncate
//! statements mid-expression.

use crate::types::{AstSymbol, SemanticChunk, SourceRange};
use std::path::Path;

pub struct AstChunker;

impl AstChunker {
    /// Chunks a source file along its AST symbols, filling gaps between symbols
    /// with preamble or interstitial code chunks.
    pub fn chunk_file(_path: &Path, content: &str, symbols: &[AstSymbol]) -> Vec<SemanticChunk> {
        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();
        if total_lines == 0 {
            return Vec::new();
        }

        let mut chunks = Vec::new();

        // Sort symbols by start_line
        let mut sorted_symbols = symbols.to_vec();
        sorted_symbols.sort_by_key(|s| s.range.start_line);

        let mut current_line = 1;

        for sym in sorted_symbols {
            // Gap before symbol (e.g. imports, comments, type definitions)
            if sym.range.start_line > current_line {
                let gap_start = current_line;
                let gap_end = sym.range.start_line - 1;
                let gap_code = extract_lines(&lines, gap_start, gap_end);
                if !gap_code.trim().is_empty() {
                    chunks.push(SemanticChunk {
                        symbol_name: format!("preamble_{}_{}", gap_start, gap_end),
                        kind: crate::types::AstSymbolKind::Module,
                        range: SourceRange::new(gap_start, gap_end, 0, 0),
                        token_estimate: approx_tokens(&gap_code),
                        code: gap_code,
                    });
                }
            }

            // Symbol body
            let sym_code = extract_lines(&lines, sym.range.start_line, sym.range.end_line);
            chunks.push(SemanticChunk {
                symbol_name: sym.name.clone(),
                kind: sym.kind,
                range: sym.range,
                token_estimate: approx_tokens(&sym_code),
                code: sym_code,
            });

            current_line = sym.range.end_line + 1;
        }

        // Trailing code after last symbol
        if current_line <= total_lines {
            let tail_code = extract_lines(&lines, current_line, total_lines);
            if !tail_code.trim().is_empty() {
                chunks.push(SemanticChunk {
                    symbol_name: format!("trailing_{}_{}", current_line, total_lines),
                    kind: crate::types::AstSymbolKind::Module,
                    range: SourceRange::new(current_line, total_lines, 0, 0),
                    token_estimate: approx_tokens(&tail_code),
                    code: tail_code,
                });
            }
        }

        chunks
    }
}

fn extract_lines(lines: &[&str], start_1based: usize, end_1based: usize) -> String {
    let start_idx = start_1based.saturating_sub(1);
    let end_idx = end_1based.min(lines.len());
    if start_idx >= end_idx {
        return String::new();
    }
    lines[start_idx..end_idx].join("\n")
}

fn approx_tokens(text: &str) -> usize {
    // 1 token approx 3.5 - 4 chars in code
    text.len().div_ceil(4)
}

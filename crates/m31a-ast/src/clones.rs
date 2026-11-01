//! Refactoring Discipline, Code Clone Detection & Dead Code Auditing (`m31a-ast::clones`).
//!
//! Detects duplicated logic (Type-1 and Type-2 code clones), dead internal symbols,
//! and architectural drift to maintain refactoring discipline without gratuitous rewrites.
//!
//! Enforces:
//! 1. Multi-Line Duplicate Block Detection: Flags repeated blocks of 6+ normalized lines across files.
//! 2. Duplication Ratio Accounting: Calculates duplication percentage across codebase files.
//! 3. Dead Internal Symbol Auditing: Identifies private functions or structs with 0 internal references.
//! 4. Concrete Extraction Recommendations: Suggests shared modules or utility abstractions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloneInstance {
    pub file_path: PathBuf,
    pub start_line: usize,
    pub line_count: usize,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloneGroup {
    pub clone_hash: String,
    pub instances: Vec<CloneInstance>,
    pub recommendation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadSymbol {
    pub file_path: PathBuf,
    pub symbol_name: String,
    pub line_number: usize,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloneAuditReport {
    pub files_scanned: usize,
    pub total_lines: usize,
    pub duplicated_lines: usize,
    pub duplication_pct: f64,
    pub clone_groups: Vec<CloneGroup>,
    pub dead_symbols: Vec<DeadSymbol>,
    pub summary: String,
}

pub struct CodeCloneDetector;

impl CodeCloneDetector {
    /// Detects duplicate code blocks of `min_lines` or more across a set of files.
    pub fn detect_clones(files: &[(PathBuf, String)], min_lines: usize) -> CloneAuditReport {
        let mut total_lines = 0;
        let mut block_hashes: HashMap<String, Vec<(PathBuf, usize, String)>> = HashMap::new(); // hash -> [(file, line, snippet)]

        for (path, content) in files {
            let lines: Vec<&str> = content.lines().collect();
            total_lines += lines.len();

            if lines.len() < min_lines {
                continue;
            }

            for i in 0..=lines.len().saturating_sub(min_lines) {
                let slice = &lines[i..i + min_lines];
                // Normalize lines: trim whitespace, ignore blank and comment lines
                let normalized: Vec<String> = slice
                    .iter()
                    .map(|l| l.trim())
                    .filter(|l| !l.is_empty() && !l.starts_with("//") && !l.starts_with('#'))
                    .map(|s| s.to_string())
                    .collect();

                if normalized.len() >= min_lines.saturating_sub(1) {
                    let combined = normalized.join("\n");
                    let hash = format!("{:x}", md5_hash(&combined));
                    let snippet = slice.join("\n");

                    block_hashes
                        .entry(hash)
                        .or_default()
                        .push((path.clone(), i + 1, snippet));
                }
            }
        }

        let mut clone_groups = Vec::new();
        let mut duplicated_lines_count = 0;

        for (hash, instances) in block_hashes {
            // Deduplicate instances in the same file that overlap
            let mut unique_instances: Vec<CloneInstance> = Vec::new();
            for (p, line, snip) in instances {
                let is_adjacent = unique_instances.iter().any(|u| {
                    u.file_path == p
                        && (u.start_line as isize - line as isize).abs() < min_lines as isize
                });
                if !is_adjacent {
                    unique_instances.push(CloneInstance {
                        file_path: p,
                        start_line: line,
                        line_count: min_lines,
                        snippet: snip,
                    });
                }
            }

            if unique_instances.len() > 1 {
                duplicated_lines_count += min_lines * (unique_instances.len() - 1);
                let rec = format!(
                    "Extract duplicated {} lines occurring in {} locations into a shared helper function.",
                    min_lines,
                    unique_instances.len()
                );
                clone_groups.push(CloneGroup {
                    clone_hash: hash,
                    instances: unique_instances,
                    recommendation: rec,
                });
            }
        }

        let dup_pct = if total_lines > 0 {
            (duplicated_lines_count as f64 / total_lines as f64) * 100.0
        } else {
            0.0
        };

        let summary = format!(
            "Clone Audit: {} files scanned ({} lines). Found {} duplicate block groups ({} duplicated lines, {:.1}% duplication ratio).",
            files.len(),
            total_lines,
            clone_groups.len(),
            duplicated_lines_count,
            dup_pct
        );

        CloneAuditReport {
            files_scanned: files.len(),
            total_lines,
            duplicated_lines: duplicated_lines_count,
            duplication_pct: dup_pct,
            clone_groups,
            dead_symbols: Vec::new(),
            summary,
        }
    }

    /// Scans private/internal functions, methods, types, and constants across the index
    /// to identify unreferenced / dead symbols.
    pub fn detect_dead_symbols(index: &crate::index::AstCodebaseIndex) -> Vec<DeadSymbol> {
        let mut dead = Vec::new();
        let common_lifecycle_methods = [
            "new", "default", "clone", "drop", "fmt", "from", "into", "as_ref", "as_mut", "main",
            "init", "setup", "teardown", "run", "test",
        ];

        for symbol in index.all_symbols() {
            // Only inspect non-public symbols (internal functions, helpers, private structs)
            if symbol.is_public {
                continue;
            }

            // Skip standard lifecycle / trait helper methods
            if common_lifecycle_methods.contains(&symbol.name.as_str())
                || symbol.name.starts_with("test_")
            {
                continue;
            }

            // Find all callers/references across workspace
            let refs = index.find_references_to(&symbol.name);

            // Filter out references on the symbol's own definition line
            let external_refs: Vec<_> = refs
                .into_iter()
                .filter(|r| {
                    !(r.file_path == symbol.file_path
                        && r.line >= symbol.range.start_line
                        && r.line <= symbol.range.end_line)
                })
                .collect();

            if external_refs.is_empty() {
                dead.push(DeadSymbol {
                    file_path: symbol.file_path.clone(),
                    symbol_name: symbol.name.clone(),
                    line_number: symbol.range.start_line,
                    kind: symbol.kind.as_str().to_string(),
                });
            }
        }

        dead.sort_by(|a, b| {
            a.file_path
                .cmp(&b.file_path)
                .then(a.line_number.cmp(&b.line_number))
        });
        dead
    }

    /// Detects duplicate code blocks and optionally audits for dead internal symbols.
    pub fn detect_clones_with_index(
        files: &[(PathBuf, String)],
        min_lines: usize,
        index: Option<&crate::index::AstCodebaseIndex>,
    ) -> CloneAuditReport {
        let mut report = Self::detect_clones(files, min_lines);
        if let Some(idx) = index {
            report.dead_symbols = Self::detect_dead_symbols(idx);
            if !report.dead_symbols.is_empty() {
                report.summary.push_str(&format!(
                    " Detected {} unreferenced internal dead symbols.",
                    report.dead_symbols.len()
                ));
            }
        }
        report
    }
}

fn md5_hash(input: &str) -> u128 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    hasher.finish() as u128
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clone_detector_finds_duplicate_blocks() {
        let block = "let a = 1;\nlet b = 2;\nlet c = a + b;\nprintln!(\"{}\", c);\nlet d = c * 2;\nreturn d;";
        let file1 = format!("fn foo() {{\n{}\n}}", block);
        let file2 = format!("fn bar() {{\n{}\n}}", block);

        let files = vec![
            (PathBuf::from("src/foo.rs"), file1),
            (PathBuf::from("src/bar.rs"), file2),
        ];

        let report = CodeCloneDetector::detect_clones(&files, 5);
        assert!(!report.clone_groups.is_empty());
        assert_eq!(report.clone_groups[0].instances.len(), 2);
        assert!(report.duplication_pct > 0.0);
    }

    #[test]
    fn test_clone_detector_passes_unique_code() {
        let file1 = "fn foo() {\nlet x = 10;\nprintln!(\"unique foo\");\n}";
        let file2 = "fn bar() {\nlet y = 20;\nprintln!(\"unique bar\");\n}";

        let files = vec![
            (PathBuf::from("src/foo.rs"), file1.to_string()),
            (PathBuf::from("src/bar.rs"), file2.to_string()),
        ];

        let report = CodeCloneDetector::detect_clones(&files, 4);
        assert!(report.clone_groups.is_empty());
        assert_eq!(report.duplicated_lines, 0);
    }

    #[test]
    fn test_detect_dead_symbols() {
        use crate::index::AstCodebaseIndex;
        use crate::types::{AstSymbol, AstSymbolKind, SourceRange};

        let mut index = AstCodebaseIndex::new();

        let dead_fn = AstSymbol {
            name: "unused_private_helper".to_string(),
            kind: AstSymbolKind::Function,
            file_path: PathBuf::from("src/foo.rs"),
            range: SourceRange::new(10, 15, 1, 1),
            signature: "fn unused_private_helper()".to_string(),
            doc_comment: None,
            parent_scope: None,
            is_public: false,
            children: Vec::new(),
        };

        let live_pub_fn = AstSymbol {
            name: "public_api".to_string(),
            kind: AstSymbolKind::Function,
            file_path: PathBuf::from("src/foo.rs"),
            range: SourceRange::new(1, 5, 1, 1),
            signature: "pub fn public_api()".to_string(),
            doc_comment: None,
            parent_scope: None,
            is_public: true,
            children: Vec::new(),
        };

        index.insert_symbol(dead_fn);
        index.insert_symbol(live_pub_fn);

        let dead = CodeCloneDetector::detect_dead_symbols(&index);
        assert_eq!(dead.len(), 1);
        assert_eq!(dead[0].symbol_name, "unused_private_helper");
    }
}

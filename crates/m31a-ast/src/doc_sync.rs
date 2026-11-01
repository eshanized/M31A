//! Documentation Synchronization Gate (`m31a-ast::doc_sync`).
//!
//! Verifies that public functions, types, and architectural modules have
//! synchronized, accurate documentation matching actual code signatures.
//!
//! Enforces:
//! 1. Public API Doc Coverage: Flags undocumented public structs, traits, enums, and functions.
//! 2. Stale Parameter Docs: Verifies that documented arguments align with actual signature parameters.
//! 3. Architecture Doc Sync: Audits README/ARCHITECTURE mentions for removed or renamed symbols.

use crate::types::AstSymbol;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissingDocItem {
    pub file_path: PathBuf,
    pub symbol_name: String,
    pub kind: String,
    pub line_number: usize,
    pub signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaleDocItem {
    pub file_path: PathBuf,
    pub symbol_name: String,
    pub line_number: usize,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocSyncReport {
    pub total_public_symbols: usize,
    pub documented_symbols: usize,
    pub coverage_pct: f64,
    pub missing_docs: Vec<MissingDocItem>,
    pub stale_docs: Vec<StaleDocItem>,
    pub passed: bool,
    pub summary: String,
}

pub struct DocSyncAuditor;

impl DocSyncAuditor {
    /// Audits public symbols for documentation completeness and signature alignment.
    pub fn audit_symbols(symbols: &[AstSymbol], min_coverage_pct: f64) -> DocSyncReport {
        let mut missing = Vec::new();
        let mut stale = Vec::new();
        let mut public_count = 0;
        let mut documented_count = 0;

        for sym in symbols.iter().filter(|s| s.is_public) {
            public_count += 1;

            if let Some(ref doc) = sym.doc_comment {
                if doc.trim().is_empty() {
                    missing.push(MissingDocItem {
                        file_path: sym.file_path.clone(),
                        symbol_name: sym.name.clone(),
                        kind: sym.kind.as_str().to_string(),
                        line_number: sym.range.start_line,
                        signature: sym.signature.clone(),
                    });
                } else {
                    documented_count += 1;

                    // Parameter sync check: if doc contains "Arguments" or "param", verify names
                    let doc_lower = doc.to_lowercase();
                    if doc_lower.contains("param") || doc_lower.contains("arguments") {
                        // Check if signature contains parentheses
                        if let Some(params_str) = sym
                            .signature
                            .split('(')
                            .nth(1)
                            .and_then(|s| s.split(')').next())
                        {
                            for raw_param in params_str.split(',') {
                                let param_name = raw_param.split(':').next().unwrap_or("").trim();
                                if !param_name.is_empty()
                                    && param_name != "self"
                                    && param_name != "&self"
                                    && param_name != "&mut self"
                                    && !doc.contains(param_name)
                                {
                                    stale.push(StaleDocItem {
                                            file_path: sym.file_path.clone(),
                                            symbol_name: sym.name.clone(),
                                            line_number: sym.range.start_line,
                                            reason: format!("Parameter '{}' in signature is missing from documentation block.", param_name),
                                        });
                                }
                            }
                        }
                    }
                }
            } else {
                missing.push(MissingDocItem {
                    file_path: sym.file_path.clone(),
                    symbol_name: sym.name.clone(),
                    kind: sym.kind.as_str().to_string(),
                    line_number: sym.range.start_line,
                    signature: sym.signature.clone(),
                });
            }
        }

        let coverage = if public_count > 0 {
            (documented_count as f64 / public_count as f64) * 100.0
        } else {
            100.0
        };

        let passed = coverage >= min_coverage_pct && stale.is_empty();

        let summary = format!(
            "Documentation Sync: {}/{} public APIs documented ({:.1}% coverage, min {:.1}%). Missing: {}, Stale: {}. Status: {}",
            documented_count,
            public_count,
            coverage,
            min_coverage_pct,
            missing.len(),
            stale.len(),
            if passed { "PASSED" } else { "FAILED" }
        );

        DocSyncReport {
            total_public_symbols: public_count,
            documented_symbols: documented_count,
            coverage_pct: coverage,
            missing_docs: missing,
            stale_docs: stale,
            passed,
            summary,
        }
    }
}

/// Category of documentation drift detected in markdown or guide files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocDriftKind {
    BrokenSymbolReference,
    StaleCodeSnippet,
    MissingArchitectureSection,
}

/// Individual documentation drift item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocDriftItem {
    pub doc_path: PathBuf,
    pub line_number: usize,
    pub kind: DocDriftKind,
    pub reference: String,
    pub explanation: String,
}

/// Prose and Markdown documentation synchronization report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProseSyncReport {
    pub total_docs_scanned: usize,
    pub code_snippets_scanned: usize,
    pub references_scanned: usize,
    pub drift_items: Vec<DocDriftItem>,
    pub passed: bool,
    pub summary: String,
}

/// Auditor that synchronizes prose markdown documents (READMEs, architecture specifications)
/// with the live AST codebase index.
pub struct ProseDocSynchronizer;

impl ProseDocSynchronizer {
    /// Audits markdown documentation files (README, ARCHITECTURE, guides) against known AST symbols
    /// to detect stale backticked symbol references, renamed APIs, or invalid code blocks.
    pub fn audit_markdown_docs(
        docs: &[(PathBuf, String)],
        known_symbols: &[AstSymbol],
    ) -> ProseSyncReport {
        let mut drift_items = Vec::new();
        let mut total_snippets = 0;
        let mut total_refs = 0;

        let known_names: HashSet<&str> = known_symbols.iter().map(|s| s.name.as_str()).collect();

        let ignored_keywords = [
            "true",
            "false",
            "None",
            "Some",
            "Ok",
            "Err",
            "Result",
            "Option",
            "Vec",
            "String",
            "u8",
            "u16",
            "u32",
            "u64",
            "usize",
            "i32",
            "i64",
            "bool",
            "str",
            "self",
            "Self",
            "cargo",
            "test",
            "build",
            "run",
            "git",
            "npm",
            "json",
            "yaml",
            "toml",
            "bash",
            "sh",
            "cd",
            "ls",
            "env",
            "main",
            "init",
            "README",
            "LICENSE",
            "Cargo.toml",
            "package.json",
            "null",
            "undefined",
            "any",
            "id",
            "status",
            "http",
            "https",
        ];

        for (doc_path, content) in docs {
            let mut in_code_fence = false;
            let mut current_fence_lang = String::new();
            let mut snippet_lines = Vec::new();
            let mut fence_start_line = 0;

            for (idx, line) in content.lines().enumerate() {
                let line_num = idx + 1;
                let trimmed = line.trim();

                if trimmed.starts_with("```") {
                    if in_code_fence {
                        // End of code fence: analyze snippet
                        total_snippets += 1;
                        in_code_fence = false;
                        Self::check_snippet(
                            doc_path,
                            fence_start_line,
                            &current_fence_lang,
                            &snippet_lines,
                            &known_names,
                            &mut drift_items,
                        );
                        snippet_lines.clear();
                        current_fence_lang.clear();
                    } else {
                        // Start of code fence
                        in_code_fence = true;
                        fence_start_line = line_num;
                        current_fence_lang = trimmed.trim_start_matches('`').trim().to_string();
                    }
                    continue;
                }

                if in_code_fence {
                    snippet_lines.push(line.to_string());
                } else {
                    // Outside code fence: check inline backticks `symbol`
                    let mut start_idx = None;
                    for (char_idx, ch) in line.char_indices() {
                        if ch == '`' {
                            if let Some(s) = start_idx {
                                let ref_str = line[s + 1..char_idx].trim();
                                if !ref_str.is_empty() && ref_str.len() > 3 {
                                    total_refs += 1;
                                    if ref_str
                                        .chars()
                                        .all(|c| c.is_alphanumeric() || c == '_' || c == ':')
                                        && !ignored_keywords.contains(&ref_str)
                                        && !ref_str.starts_with("--")
                                        && !ref_str.contains(' ')
                                    {
                                        let base_name =
                                            ref_str.split("::").last().unwrap_or(ref_str);
                                        let looks_like_code_sym = base_name
                                            .chars()
                                            .next()
                                            .is_some_and(|c| c.is_uppercase())
                                            || (base_name.contains('_')
                                                && !base_name.starts_with('_'));

                                        if looks_like_code_sym
                                            && !known_names.is_empty()
                                            && !known_names.contains(base_name)
                                        {
                                            drift_items.push(DocDriftItem {
                                                doc_path: doc_path.clone(),
                                                line_number: line_num,
                                                kind: DocDriftKind::BrokenSymbolReference,
                                                reference: ref_str.to_string(),
                                                explanation: format!(
                                                    "Document references symbol '{}', but it is not found in codebase AST index.",
                                                    ref_str
                                                ),
                                            });
                                        }
                                    }
                                }
                                start_idx = None;
                            } else {
                                start_idx = Some(char_idx);
                            }
                        }
                    }
                }
            }
        }

        let passed = drift_items.is_empty();
        let summary = format!(
            "Prose Doc Sync: {} documents scanned ({} code snippets, {} references). Found {} documentation drifts. Status: {}",
            docs.len(),
            total_snippets,
            total_refs,
            drift_items.len(),
            if passed { "PASSED" } else { "DRIFT DETECTED" }
        );

        ProseSyncReport {
            total_docs_scanned: docs.len(),
            code_snippets_scanned: total_snippets,
            references_scanned: total_refs,
            drift_items,
            passed,
            summary,
        }
    }

    fn check_snippet(
        doc_path: &Path,
        start_line: usize,
        lang: &str,
        lines: &[String],
        known_names: &HashSet<&str>,
        drifts: &mut Vec<DocDriftItem>,
    ) {
        if known_names.is_empty() {
            return;
        }
        if lang == "rust" || lang == "ts" || lang == "js" || lang == "python" || lang.is_empty() {
            for (offset, line) in lines.iter().enumerate() {
                for word in line.split(|c: char| !c.is_alphanumeric() && c != '_') {
                    if word.len() > 6 && word.starts_with("M31a") && !known_names.contains(word) {
                        drifts.push(DocDriftItem {
                            doc_path: doc_path.to_path_buf(),
                            line_number: start_line + offset,
                            kind: DocDriftKind::StaleCodeSnippet,
                            reference: word.to_string(),
                            explanation: format!(
                                "Code snippet in docs references unknown symbol '{}'. Snippet may be outdated.",
                                word
                            ),
                        });
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AstSymbolKind, SourceRange};

    fn make_test_sym(name: &str, sig: &str, doc: Option<&str>) -> AstSymbol {
        AstSymbol {
            name: name.to_string(),
            kind: AstSymbolKind::Function,
            file_path: PathBuf::from("src/lib.rs"),
            range: SourceRange::new(1, 10, 1, 1),
            signature: sig.to_string(),
            doc_comment: doc.map(|s| s.to_string()),
            parent_scope: None,
            is_public: true,
            children: Vec::new(),
        }
    }

    #[test]
    fn test_doc_sync_detects_undocumented_public_api() {
        let syms = vec![
            make_test_sym(
                "documented_fn",
                "pub fn documented_fn()",
                Some("Does something."),
            ),
            make_test_sym("undocumented_fn", "pub fn undocumented_fn()", None),
        ];

        let report = DocSyncAuditor::audit_symbols(&syms, 80.0);
        assert_eq!(report.total_public_symbols, 2);
        assert_eq!(report.documented_symbols, 1);
        assert_eq!(report.coverage_pct, 50.0);
        assert!(!report.passed);
        assert_eq!(report.missing_docs.len(), 1);
    }

    #[test]
    fn test_doc_sync_detects_missing_parameter_in_doc() {
        let syms = vec![make_test_sym(
            "process_payment",
            "pub fn process_payment(amount: u64, currency: &str)",
            Some("# Arguments\n* `amount` - Total in cents"),
        )];

        let report = DocSyncAuditor::audit_symbols(&syms, 80.0);
        assert_eq!(report.stale_docs.len(), 1);
        assert!(report.stale_docs[0].reason.contains("currency"));
    }

    #[test]
    fn test_prose_doc_synchronizer_detects_broken_references() {
        let docs = vec![(
            PathBuf::from("README.md"),
            r#"# Architecture
Use `ActiveAgentExecutor` to run tasks.
Do not use `DeletedOldRunner` anymore.
"#
            .to_string(),
        )];

        let syms = vec![make_test_sym(
            "ActiveAgentExecutor",
            "pub struct ActiveAgentExecutor",
            None,
        )];

        let report = ProseDocSynchronizer::audit_markdown_docs(&docs, &syms);
        assert!(!report.passed);
        assert_eq!(report.drift_items.len(), 1);
        assert_eq!(report.drift_items[0].reference, "DeletedOldRunner");
        assert_eq!(
            report.drift_items[0].kind,
            DocDriftKind::BrokenSymbolReference
        );
    }
}

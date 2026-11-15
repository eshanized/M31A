//! Task-Aware Context Retrieval & Token-Budget Assembly Engine (`m31a-ast::retrieval`).
//!
//! Synthesizes ranked, explainable repository context for engineering tasks,
//! error messages, stack traces, and proposed changes within a strict token budget.

use crate::graph::SymbolGraph;
use crate::test_discovery::{RelatedTestTarget, TestDiscoveryEngine};
use crate::types::{AstSymbol, CallReference, SymbolRelationship};
use crate::workspace::WorkspaceInventory;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

/// Input query for repository context retrieval.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RetrievalQuery {
    pub task_description: Option<String>,
    pub error_message: Option<String>,
    pub file_path: Option<PathBuf>,
    pub symbol_name: Option<String>,
    pub stack_trace: Option<String>,
    pub changed_files: Vec<PathBuf>,
    pub token_budget: Option<usize>,
}

/// A ranked symbol definition with its callers, callees, and code excerpt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievedSymbolContext {
    pub symbol: AstSymbol,
    pub score: f64,
    pub match_reasons: Vec<String>,
    pub callers: Vec<CallReference>,
    pub callees: Vec<CallReference>,
    pub body_excerpt: Option<String>,
    pub estimated_tokens: usize,
}

/// Structured, bounded context packet ready for LLM consumption.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextPacket {
    pub query_summary: String,
    pub relevant_files: Vec<PathBuf>,
    pub symbols: Vec<RetrievedSymbolContext>,
    pub related_tests: Vec<RelatedTestTarget>,
    pub manifest_context: Vec<String>,
    pub architecture_notes: Vec<String>,
    pub diagnostics_and_uncertainty: Vec<String>,
    pub total_estimated_tokens: usize,
    pub is_truncated: bool,
    pub formatted_markdown: String,
}

impl ContextPacket {
    pub fn render_markdown(&self) -> String {
        self.formatted_markdown.clone()
    }
}

pub struct SemanticRetriever;

impl SemanticRetriever {
    /// Retrieves task-aware context packet combining symbol matching, call graphs, tests, and token budget.
    pub fn retrieve_context(
        query: &RetrievalQuery,
        all_symbols: &[AstSymbol],
        relationships: &[SymbolRelationship],
        symbol_graph: &SymbolGraph,
        inventory: &WorkspaceInventory,
        test_files: &[PathBuf],
    ) -> ContextPacket {
        let token_budget = query.token_budget.unwrap_or(4000);
        let mut relevant_files_set = HashSet::new();

        // 1. Extract search tokens from query inputs
        let mut target_terms = HashSet::new();
        if let Some(ref sym) = query.symbol_name {
            target_terms.insert(sym.to_lowercase());
        }
        if let Some(ref task) = query.task_description {
            for word in task.split(&[' ', ',', '.', '`', '"', '\'', ':', ';', '(', ')'][..]) {
                let trimmed = word.trim().to_lowercase();
                if trimmed.len() >= 3 && !is_stop_word(&trimmed) {
                    target_terms.insert(trimmed);
                }
            }
        }
        if let Some(ref err) = query.error_message {
            for word in err.split(&[' ', ',', ':', '`', '\'', '"', '\n', '\t'][..]) {
                let trimmed = word.trim().to_lowercase();
                if trimmed.len() >= 3 && !is_stop_word(&trimmed) {
                    target_terms.insert(trimmed);
                }
            }
        }

        for f in &query.changed_files {
            relevant_files_set.insert(f.clone());
        }
        if let Some(ref f) = query.file_path {
            relevant_files_set.insert(f.clone());
        }

        // 2. Score candidate symbols
        let mut candidate_symbols: Vec<(AstSymbol, f64, Vec<String>)> = Vec::new();

        for sym in all_symbols {
            let mut score = 0.0;
            let mut reasons = Vec::new();

            // A. Exact symbol name match
            if let Some(ref target_sym) = query.symbol_name {
                if sym.name == *target_sym {
                    score += 100.0;
                    reasons.push(format!("Exact symbol name match with `{}`", target_sym));
                } else if sym.name.to_lowercase() == target_sym.to_lowercase() {
                    score += 80.0;
                    reasons.push(format!(
                        "Case-insensitive symbol match with `{}`",
                        target_sym
                    ));
                } else if sym.name.contains(target_sym.as_str()) {
                    score += 40.0;
                    reasons.push(format!("Sub-identifier match with `{}`", target_sym));
                }
            }

            // B. File match
            if let Some(ref target_file) = query.file_path {
                if sym.file_path == *target_file {
                    score += 50.0;
                    reasons.push(format!(
                        "Defined in requested target file `{}`",
                        target_file.display()
                    ));
                }
            }

            // C. Changed file proximity
            if query.changed_files.iter().any(|cf| cf == &sym.file_path) {
                score += 35.0;
                reasons.push("Defined in one of the changed files".to_string());
            }

            // D. Keyword overlap with task / error message
            let sym_name_lower = sym.name.to_lowercase();
            for term in &target_terms {
                if sym_name_lower == *term {
                    score += 30.0;
                    reasons.push(format!("Matches task keyword `{}`", term));
                } else if sym_name_lower.contains(term) {
                    score += 15.0;
                    reasons.push(format!("Contains task keyword `{}`", term));
                }
            }

            // E. Public export bonus
            if sym.is_public {
                score += 5.0;
            }

            if score > 0.0 {
                candidate_symbols.push((sym.clone(), score, reasons));
            }
        }

        // Sort candidates by score descending
        candidate_symbols
            .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // 3. Assemble top candidates with call graph excerpts within token budget
        let mut retrieved_symbols = Vec::new();
        let mut related_tests = Vec::new();
        let mut total_tokens = 0;
        let mut is_truncated = false;

        for (sym, score, reasons) in candidate_symbols.into_iter().take(20) {
            relevant_files_set.insert(sym.file_path.clone());

            let callers = symbol_graph.find_callers(&sym.name);
            let callees = symbol_graph.find_callees(&sym.name);

            // Read excerpt if file exists
            let excerpt = read_symbol_excerpt(&sym);
            let est_tokens = estimate_tokens(&sym.signature)
                + excerpt.as_ref().map(|e| estimate_tokens(e)).unwrap_or(0)
                + 50;

            if total_tokens + est_tokens > token_budget {
                is_truncated = true;
                break;
            }

            total_tokens += est_tokens;

            // Also find related tests for high-scoring symbols
            if score >= 30.0 && related_tests.len() < 10 {
                let tests = TestDiscoveryEngine::find_related_tests(
                    &sym.name,
                    &sym.file_path,
                    all_symbols,
                    relationships,
                    test_files,
                );
                for t in tests {
                    if !related_tests.iter().any(|rt: &RelatedTestTarget| {
                        rt.test_name == t.test_name && rt.test_file == t.test_file
                    }) {
                        related_tests.push(t);
                    }
                }
            }

            retrieved_symbols.push(RetrievedSymbolContext {
                symbol: sym,
                score,
                match_reasons: reasons,
                callers,
                callees,
                body_excerpt: excerpt,
                estimated_tokens: est_tokens,
            });
        }

        // 4. Manifest context
        let mut manifest_context = Vec::new();
        for m in &inventory.manifests {
            manifest_context.push(format!(
                "- {} ({})",
                m.relative_path.display(),
                m.package_name.as_deref().unwrap_or("package")
            ));
        }

        // 5. Relevant files list
        let mut relevant_files: Vec<PathBuf> = relevant_files_set.into_iter().collect();
        relevant_files.sort();

        // 6. Build formatted markdown
        let formatted_markdown = format_packet_markdown(
            query,
            &relevant_files,
            &retrieved_symbols,
            &related_tests,
            &manifest_context,
            total_tokens,
            is_truncated,
        );

        let query_summary = format!(
            "Task: {} | Target File: {} | Target Symbol: {}",
            query.task_description.as_deref().unwrap_or("None"),
            query
                .file_path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "None".to_string()),
            query.symbol_name.as_deref().unwrap_or("None")
        );

        ContextPacket {
            query_summary,
            relevant_files,
            symbols: retrieved_symbols,
            related_tests,
            manifest_context,
            architecture_notes: Vec::new(),
            diagnostics_and_uncertainty: Vec::new(),
            total_estimated_tokens: total_tokens,
            is_truncated,
            formatted_markdown,
        }
    }
}

fn estimate_tokens(text: &str) -> usize {
    // Standard approximation: ~4 characters per token
    (text.len() / 4).max(1)
}

fn is_stop_word(w: &str) -> bool {
    matches!(
        w,
        "the"
            | "and"
            | "for"
            | "with"
            | "this"
            | "that"
            | "from"
            | "into"
            | "when"
            | "where"
            | "does"
            | "have"
            | "should"
            | "would"
            | "could"
            | "what"
            | "make"
            | "need"
    )
}

fn read_symbol_excerpt(sym: &AstSymbol) -> Option<String> {
    if let Ok(content) = fs::read_to_string(&sym.file_path) {
        let lines: Vec<&str> = content.lines().collect();
        let start = sym.range.start_line.saturating_sub(1);
        let end = sym.range.end_line.min(lines.len());
        if start < end {
            // Cap at 40 lines to respect context budgets
            let capped_end = end.min(start + 40);
            let mut excerpt = lines[start..capped_end].join("\n");
            if capped_end < end {
                excerpt.push_str("\n    // ... [truncated remaining implementation lines] ...");
            }
            return Some(excerpt);
        }
    }
    None
}

fn format_packet_markdown(
    query: &RetrievalQuery,
    relevant_files: &[PathBuf],
    symbols: &[RetrievedSymbolContext],
    tests: &[RelatedTestTarget],
    manifests: &[String],
    total_tokens: usize,
    is_truncated: bool,
) -> String {
    let mut s = String::new();
    s.push_str("# Repository Task Context\n\n");

    if let Some(ref task) = query.task_description {
        s.push_str(&format!("**Task Objective:** {}\n", task));
    }
    if let Some(ref err) = query.error_message {
        s.push_str(&format!("**Diagnostic Error:** {}\n", err));
    }
    s.push_str(&format!(
        "**Context Budget Consumption:** ~{} tokens (Truncated: {})\n\n",
        total_tokens, is_truncated
    ));

    s.push_str("## Relevant Files\n");
    for f in relevant_files {
        s.push_str(&format!("- `{}`\n", f.display()));
    }
    s.push('\n');

    if !symbols.is_empty() {
        s.push_str("## Key Symbols & Signatures\n\n");
        for sym_ctx in symbols {
            s.push_str(&format!(
                "### `{}` (`{}`) — Score: {:.1}\n",
                sym_ctx.symbol.name,
                sym_ctx.symbol.kind.as_str(),
                sym_ctx.score
            ));
            s.push_str(&format!(
                "- **File:** `{}:{}`\n",
                sym_ctx.symbol.file_path.display(),
                sym_ctx.symbol.range.start_line
            ));
            s.push_str(&format!(
                "- **Signature:** `{}`\n",
                sym_ctx.symbol.signature
            ));
            if !sym_ctx.match_reasons.is_empty() {
                s.push_str(&format!(
                    "- **Match Rationale:** {}\n",
                    sym_ctx.match_reasons.join(", ")
                ));
            }

            if !sym_ctx.callers.is_empty() {
                let caller_names: Vec<String> = sym_ctx
                    .callers
                    .iter()
                    .take(4)
                    .map(|c| format!("`{}` ({}:{})", c.caller_name, c.file_path.display(), c.line))
                    .collect();
                s.push_str(&format!(
                    "- **Direct Callers:** {}\n",
                    caller_names.join(", ")
                ));
            }

            if !sym_ctx.callees.is_empty() {
                let callee_names: Vec<String> = sym_ctx
                    .callees
                    .iter()
                    .take(4)
                    .map(|c| format!("`{}`", c.callee_name))
                    .collect();
                s.push_str(&format!(
                    "- **Callees Invoked:** {}\n",
                    callee_names.join(", ")
                ));
            }

            if let Some(ref body) = sym_ctx.body_excerpt {
                s.push_str("\n```\n");
                s.push_str(body);
                s.push_str("\n```\n\n");
            }
        }
    }

    if !tests.is_empty() {
        s.push_str("## Recommended Tests to Run\n");
        for t in tests {
            s.push_str(&format!(
                "- `{}` in `{}:{}` (Confidence: {:.2}) — {}\n",
                t.test_name,
                t.test_file.display(),
                t.line,
                t.confidence,
                t.explanation
            ));
        }
        s.push('\n');
    }

    if !manifests.is_empty() {
        s.push_str("## Discovered Manifests\n");
        for m in manifests {
            s.push_str(&format!("{}\n", m));
        }
    }

    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AstSymbolKind, SourceRange};

    #[test]
    fn test_semantic_retrieval_ranking() {
        let sym_a = AstSymbol::new(
            "authenticate_user",
            AstSymbolKind::Function,
            PathBuf::from("src/auth.rs"),
            SourceRange::new(10, 25, 0, 0),
            "pub async fn authenticate_user(token: &str) -> Result<User>",
        );
        let sym_b = AstSymbol::new(
            "render_widget",
            AstSymbolKind::Function,
            PathBuf::from("src/ui.rs"),
            SourceRange::new(1, 10, 0, 0),
            "pub fn render_widget()",
        );

        let symbols = vec![sym_a, sym_b];
        let symbol_graph = SymbolGraph::new();
        let inventory = WorkspaceInventory {
            root_path: PathBuf::from("/repo"),
            repo_name: "test".to_string(),
            git_branch: None,
            git_head_sha: None,
            detected_languages: vec![],
            manifests: vec![],
            packages: vec![],
            is_monorepo: false,
        };

        let query = RetrievalQuery {
            task_description: Some("Fix failure in authenticate_user verification".to_string()),
            symbol_name: Some("authenticate_user".to_string()),
            token_budget: Some(2000),
            ..Default::default()
        };

        let packet = SemanticRetriever::retrieve_context(
            &query,
            &symbols,
            &[],
            &symbol_graph,
            &inventory,
            &[],
        );

        assert!(!packet.symbols.is_empty());
        assert_eq!(packet.symbols[0].symbol.name, "authenticate_user");
        assert!(packet.symbols[0].score >= 100.0);
        assert!(packet.formatted_markdown.contains("authenticate_user"));
    }
}

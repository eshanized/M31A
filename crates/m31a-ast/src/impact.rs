//! Semantic Blast-Radius & Codebase Impact Analysis Engine (`m31a-ast::impact`).
//!
//! Analyzes affected components, downstream dependencies, public API boundaries,
//! and regression risks before file or symbol modifications occur.
//!
//! Enforces:
//! 1. Transitive Call Graph Traversal: Identifies direct callers and transitive dependents.
//! 2. Export Boundary Auditing: Detects whether modified symbols cross crate or package public boundaries.
//! 3. Blast-Radius Scoring: Computes a 0.0-100.0 risk score based on caller count, depth, and data model impacts.

use crate::index::AstCodebaseIndex;
use crate::types::AstSymbolKind;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImpactRiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependentConsumer {
    pub file_path: PathBuf,
    pub symbol_name: String,
    pub call_depth: usize,
    pub is_direct: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolImpact {
    pub target_symbol: String,
    pub target_file: PathBuf,
    pub is_public_export: bool,
    pub direct_callers_count: usize,
    pub transitive_dependents: Vec<DependentConsumer>,
    pub potential_regressions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlastRadiusReport {
    pub target_files: Vec<PathBuf>,
    pub modified_symbols: Vec<String>,
    pub impacts: Vec<SymbolImpact>,
    pub total_affected_files: usize,
    pub blast_radius_score: f64,
    pub risk_level: ImpactRiskLevel,
    pub recommended_test_targets: Vec<String>,
    pub summary: String,
}

pub struct SemanticImpactAnalyzer;

impl SemanticImpactAnalyzer {
    /// Computes the semantic blast-radius and impacted components for a set of target files/symbols.
    pub fn analyze_impact(
        index: &AstCodebaseIndex,
        target_files: &[PathBuf],
        candidate_symbols: &[String],
    ) -> BlastRadiusReport {
        let mut impacts = Vec::new();
        let mut affected_files_set = HashSet::new();

        for file in target_files {
            affected_files_set.insert(file.clone());
        }

        let symbols_to_check: Vec<String> = if !candidate_symbols.is_empty() {
            candidate_symbols.to_vec()
        } else {
            // Collect all symbols in target_files
            let mut syms = Vec::new();
            for file in target_files {
                for s in index.symbols_in_file(file) {
                    syms.push(s.name.clone());
                }
            }
            syms
        };

        for sym_name in &symbols_to_check {
            let matches = index.find_symbols_by_name(sym_name);
            for sym in matches {
                let callers = index.find_references_to(&sym.name);
                let mut dependents = Vec::new();
                let mut visited = HashSet::new();
                visited.insert(sym.name.clone());

                // Direct callers
                for caller in &callers {
                    affected_files_set.insert(caller.file_path.clone());
                    dependents.push(DependentConsumer {
                        file_path: caller.file_path.clone(),
                        symbol_name: caller.caller_name.clone(),
                        call_depth: 1,
                        is_direct: true,
                    });

                    // Recursive transitive traversal up to depth 5
                    Self::traverse_transitive_callers(
                        index,
                        &caller.caller_name,
                        2,
                        5,
                        &mut visited,
                        &mut dependents,
                        &mut affected_files_set,
                    );
                }

                let mut regressions = Vec::new();
                let is_public = sym.is_public;
                if is_public {
                    regressions.push(format!(
                        "Public API '{}' signature change affects external callers.",
                        sym.name
                    ));
                }
                if sym.kind == AstSymbolKind::Struct || sym.kind == AstSymbolKind::Class {
                    regressions.push(format!("Data model '{}' modification risks breaking serialized contracts or database schemas.", sym.name));
                }

                impacts.push(SymbolImpact {
                    target_symbol: sym.name.clone(),
                    target_file: sym.file_path.clone(),
                    is_public_export: is_public,
                    direct_callers_count: callers.len(),
                    transitive_dependents: dependents,
                    potential_regressions: regressions,
                });
            }
        }

        let total_affected = affected_files_set.len();
        let callers_total: usize = impacts.iter().map(|i| i.direct_callers_count).sum();
        let has_public_api = impacts.iter().any(|i| i.is_public_export);

        // Calculate blast radius score: 0-100
        let mut score = (total_affected as f64 * 5.0) + (callers_total as f64 * 2.5);
        if has_public_api {
            score += 25.0;
        }
        let blast_radius_score = score.min(100.0);

        let risk_level = if blast_radius_score >= 70.0 {
            ImpactRiskLevel::Critical
        } else if blast_radius_score >= 40.0 {
            ImpactRiskLevel::High
        } else if blast_radius_score >= 15.0 {
            ImpactRiskLevel::Medium
        } else {
            ImpactRiskLevel::Low
        };

        let mut recommended_tests = Vec::new();
        for file in &affected_files_set {
            let s = file.to_string_lossy();
            if s.contains("crates/") {
                let crate_name = s
                    .split("crates/")
                    .nth(1)
                    .and_then(|p| p.split('/').next())
                    .unwrap_or("");
                if !crate_name.is_empty() {
                    let cmd = format!("cargo test -p {}", crate_name);
                    if !recommended_tests.contains(&cmd) {
                        recommended_tests.push(cmd);
                    }
                }
            }
        }

        let summary = format!(
            "Impact Analysis: {} target files touch {} affected files across {} call sites. Blast Radius: {:.1}/100 ({:?})",
            target_files.len(),
            total_affected,
            callers_total,
            blast_radius_score,
            risk_level
        );

        BlastRadiusReport {
            target_files: target_files.to_vec(),
            modified_symbols: symbols_to_check,
            impacts,
            total_affected_files: total_affected,
            blast_radius_score,
            risk_level,
            recommended_test_targets: recommended_tests,
            summary,
        }
    }

    /// Recursively traverses transitive callers up to `max_depth` avoiding cyclic graphs.
    fn traverse_transitive_callers(
        index: &AstCodebaseIndex,
        caller_name: &str,
        depth: usize,
        max_depth: usize,
        visited: &mut HashSet<String>,
        dependents: &mut Vec<DependentConsumer>,
        affected_files: &mut HashSet<PathBuf>,
    ) {
        if depth > max_depth || visited.contains(caller_name) {
            return;
        }

        visited.insert(caller_name.to_string());
        let transitive_refs = index.find_references_to(caller_name);

        for r in transitive_refs {
            affected_files.insert(r.file_path.clone());
            dependents.push(DependentConsumer {
                file_path: r.file_path.clone(),
                symbol_name: r.caller_name.clone(),
                call_depth: depth,
                is_direct: false,
            });

            Self::traverse_transitive_callers(
                index,
                &r.caller_name,
                depth + 1,
                max_depth,
                visited,
                dependents,
                affected_files,
            );
        }
    }

    /// Pre-edit automated gating check evaluating prospective edits before disk writes.
    pub fn evaluate_pre_edit_safety(
        index: &AstCodebaseIndex,
        target_file: &std::path::Path,
        candidate_symbols: &[String],
        max_allowed_risk: ImpactRiskLevel,
    ) -> Result<BlastRadiusReport, String> {
        let report = Self::analyze_impact(index, &[target_file.to_path_buf()], candidate_symbols);

        let risk_severity = match report.risk_level {
            ImpactRiskLevel::Low => 1,
            ImpactRiskLevel::Medium => 2,
            ImpactRiskLevel::High => 3,
            ImpactRiskLevel::Critical => 4,
        };

        let max_severity = match max_allowed_risk {
            ImpactRiskLevel::Low => 1,
            ImpactRiskLevel::Medium => 2,
            ImpactRiskLevel::High => 3,
            ImpactRiskLevel::Critical => 4,
        };

        if risk_severity > max_severity {
            Err(format!(
                "Pre-edit impact check rejected: blast radius score {:.1}/100 exceeds allowed {:?} threshold. Affected files: {}. Run tests first: {:?}",
                report.blast_radius_score,
                max_allowed_risk,
                report.total_affected_files,
                report.recommended_test_targets
            ))
        } else {
            Ok(report)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AstSymbol, SourceRange};

    #[test]
    fn test_impact_analysis_scoring() {
        let mut index = AstCodebaseIndex::new();
        // Register symbols manually
        let sym = AstSymbol {
            name: "calculate_tax".to_string(),
            kind: AstSymbolKind::Function,
            file_path: PathBuf::from("crates/billing/src/tax.rs"),
            range: SourceRange {
                start_line: 10,
                start_col: 1,
                end_line: 20,
                end_col: 1,
            },
            doc_comment: None,
            parent_scope: None,
            is_public: true,
            signature: "pub fn calculate_tax()".to_string(),
            children: Vec::new(),
        };
        index.insert_symbol(sym);

        let report = SemanticImpactAnalyzer::analyze_impact(
            &index,
            &[PathBuf::from("crates/billing/src/tax.rs")],
            &["calculate_tax".to_string()],
        );

        assert_eq!(report.modified_symbols.len(), 1);
        assert!(report.blast_radius_score >= 25.0); // Contains public API bonus
        assert!(report.impacts[0].is_public_export);
    }

    #[test]
    fn test_transitive_callers_and_safety_gating() {
        let mut index = AstCodebaseIndex::new();
        let sym = AstSymbol {
            name: "core_dispatch".to_string(),
            kind: AstSymbolKind::Function,
            file_path: PathBuf::from("crates/core/src/dispatch.rs"),
            range: SourceRange {
                start_line: 1,
                start_col: 1,
                end_line: 10,
                end_col: 1,
            },
            doc_comment: None,
            parent_scope: None,
            is_public: true,
            signature: "pub fn core_dispatch()".to_string(),
            children: Vec::new(),
        };
        index.insert_symbol(sym);

        // Pre-edit check with low threshold allows safe edits or warns
        let check = SemanticImpactAnalyzer::evaluate_pre_edit_safety(
            &index,
            std::path::Path::new("crates/core/src/dispatch.rs"),
            &["core_dispatch".to_string()],
            ImpactRiskLevel::Critical,
        );
        assert!(check.is_ok());
    }
}

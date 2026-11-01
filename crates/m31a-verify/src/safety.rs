//! Semantic Blast-Radius & Codebase Safety Gate (`m31a-verify::safety`).
//!
//! Synthesizes AST symbol cross-referencing, semantic blast-radius calculation,
//! and variable data-flow taint analysis into an autonomous verification gate.

use anyhow::Result;
use m31a_ast::data_flow::{DataFlowAnalyzer, DataFlowReport};
use m31a_ast::impact::{BlastRadiusReport, ImpactRiskLevel, SemanticImpactAnalyzer};
use m31a_ast::index::AstCodebaseIndex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;

/// High-level verdict on safety of proposed or active modifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SafetyGateVerdict {
    Passed,
    Warning,
    Blocked,
}

/// Comprehensive safety and impact report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyAuditReport {
    pub verdict: SafetyGateVerdict,
    pub blast_radius: BlastRadiusReport,
    pub data_flow_reports: Vec<DataFlowReport>,
    pub total_taints_found: usize,
    pub unconsumed_mutations_count: usize,
    pub affected_downstream_symbols_count: usize,
    pub recommended_test_commands: Vec<String>,
    pub summary: String,
}

pub struct CodebaseSafetyGate;

impl CodebaseSafetyGate {
    /// Audits a set of modified files against the codebase AST and data-flow invariants.
    pub fn audit_workspace_changes(
        workspace_root: &Path,
        modified_files: &[PathBuf],
        candidate_symbols: &[String],
    ) -> Result<SafetyAuditReport> {
        info!(
            root = ?workspace_root,
            files_count = modified_files.len(),
            "Running CodebaseSafetyGate audit"
        );

        // 1. Build AST Index (capped at 2,000 files for fast verification)
        let index = AstCodebaseIndex::build(workspace_root, 2000);

        // 2. Perform Blast Radius Impact Analysis
        let blast_radius =
            SemanticImpactAnalyzer::analyze_impact(&index, modified_files, candidate_symbols);

        // 3. Perform Data-Flow and Taint Analysis across all modified files
        let mut data_flow_reports = Vec::new();
        let mut total_taints_found = 0;
        let mut unconsumed_mutations_count = 0;

        for file in modified_files {
            let full_path = if file.is_absolute() {
                file.clone()
            } else {
                workspace_root.join(file)
            };

            if full_path.exists()
                && full_path.is_file()
                && let Ok(content) = fs::read_to_string(&full_path)
            {
                let df_report = DataFlowAnalyzer::analyze_source(&full_path, &content);
                total_taints_found += df_report.source_to_sink_taints.len();
                unconsumed_mutations_count += df_report.unconsumed_mutations.len();
                data_flow_reports.push(df_report);
            }
        }

        // 4. Derive Verdict
        let verdict = if blast_radius.risk_level == ImpactRiskLevel::Critical
            || total_taints_found > 0
        {
            SafetyGateVerdict::Blocked
        } else if blast_radius.risk_level == ImpactRiskLevel::High || unconsumed_mutations_count > 3
        {
            SafetyGateVerdict::Warning
        } else {
            SafetyGateVerdict::Passed
        };

        // 5. Build recommended test commands
        let mut recommended_tests = Vec::new();
        for target in &blast_radius.recommended_test_targets {
            recommended_tests.push(format!("cargo test {}", target));
        }
        if recommended_tests.is_empty() {
            recommended_tests.push("cargo test".to_string());
        }

        let summary = format!(
            "Safety Gate Verdict: {:?} | Blast Radius Score: {:.1}/100 ({:?}) | Affected Files: {} | Taints: {} | Dead Mutations: {}",
            verdict,
            blast_radius.blast_radius_score,
            blast_radius.risk_level,
            blast_radius.total_affected_files,
            total_taints_found,
            unconsumed_mutations_count
        );

        Ok(SafetyAuditReport {
            verdict,
            blast_radius,
            data_flow_reports,
            total_taints_found,
            unconsumed_mutations_count,
            affected_downstream_symbols_count: 0,
            recommended_test_commands: recommended_tests,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_codebase_safety_gate() {
        let temp_dir = tempfile::tempdir().unwrap();
        let src_file = temp_dir.path().join("main.rs");
        fs::write(
            &src_file,
            r#"
            fn process_user_input(input: &str) {
                let sanitized = input.trim();
                println!("Processed: {}", sanitized);
            }
            "#,
        )
        .unwrap();

        let report =
            CodebaseSafetyGate::audit_workspace_changes(temp_dir.path(), &[src_file], &[]).unwrap();
        assert_ne!(report.verdict, SafetyGateVerdict::Blocked);
        assert!(!report.recommended_test_commands.is_empty());
    }
}

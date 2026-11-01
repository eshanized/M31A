//! Algorithmic Root-Cause Analysis & Causal Diagnostic Graph Engine (`m31a-verify::root_cause`).
//!
//! Investigates underlying systemic defects and distinguishes true root-cause solutions
//! from superficial patches (swallowed errors, empty catch blocks, dummy stub returns, unwrap silencing).

use crate::diagnostics::Diagnostic;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Classification of a code patch or proposed modification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatchClassification {
    /// True architectural or logical root-cause correction.
    RootCauseFix,
    /// Superficial bandage that masks symptoms without fixing the root fault.
    SuperficialBandage,
}

/// Specific category of superficial bandage detected in diffs or code changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SuperficialPatchKind {
    /// Empty or fallback catch block that silently suppresses runtime exceptions.
    EmptyFallbackCatch { reason: String },
    /// Converting panicking unwrap/expect into silent None/default without error propagation.
    UnwrapSuppression { location: String },
    /// Hardcoding stub return values (`return true;`, `return Ok(());`, `return vec![];`) to satisfy tests.
    StubReturn { pattern: String },
    /// Discarding error values (`let _ = ...`, `.map_err(|_| ())`).
    SwallowError { details: String },
    /// Weakening or deleting test assertions instead of fixing production behavior.
    TestAssertionWeakening { details: String },
}

/// A node in the causal dependency failure graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalDiagnosticNode {
    pub id: String,
    pub symptom: String,
    pub source_location: Option<String>,
    pub probable_causes: Vec<String>,
    pub is_root: bool,
}

/// Directed Causal Graph modeling error cascade propagation from root defect to symptoms.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalDiagnosticGraph {
    pub nodes: Vec<CausalDiagnosticNode>,
    /// Directed edge representing: `(cause_id) -> causes -> (symptom_id)`
    pub edges: Vec<(String, String)>,
    pub identified_root_causes: Vec<String>,
    pub recommended_investigation_sites: Vec<String>,
}

/// Analysis report evaluating proposed edits against superficial anti-patterns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchIntegrityReport {
    pub classification: PatchClassification,
    pub detected_bandages: Vec<SuperficialPatchKind>,
    pub passed_root_cause_gate: bool,
    pub explanation: String,
}

pub struct RootCauseAnalyzer;

impl RootCauseAnalyzer {
    /// Constructs a causal diagnostic graph from compiler / runtime diagnostics and backtrace output.
    pub fn build_causal_graph(
        diagnostics: &[Diagnostic],
        trace_output: &str,
    ) -> CausalDiagnosticGraph {
        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        let mut node_ids = HashSet::new();
        let mut non_root_ids = HashSet::new();
        let mut investigation_sites = Vec::new();

        // 1. Ingest compiler/linter diagnostics
        for (i, diag) in diagnostics.iter().enumerate() {
            let id = format!("diag_{}", i + 1);
            node_ids.insert(id.clone());

            let loc = format!("{}:{}", diag.file_path.display(), diag.line);
            investigation_sites.push(loc.clone());

            let mut probable = Vec::new();
            if let Some(ref code) = diag.code {
                probable.push(format!("Compiler violation: {}", code));
            }
            if let Some(ref fix) = diag.suggested_fix {
                probable.push(format!("Compiler suggestion: {}", fix));
            }

            nodes.push(CausalDiagnosticNode {
                id: id.clone(),
                symptom: diag.message.clone(),
                source_location: Some(loc),
                probable_causes: probable,
                is_root: false,
            });
        }

        // 2. Parse panic / stacktrace propagation if present
        let mut prior_frame_id: Option<String> = None;
        let mut frame_count = 0;

        for line in trace_output.lines() {
            let trimmed = line.trim();
            if trimmed.contains("panicked at")
                || trimmed.contains("Traceback (most recent call last):")
            {
                let id = "panic_entry".to_string();
                if node_ids.insert(id.clone()) {
                    nodes.push(CausalDiagnosticNode {
                        id: id.clone(),
                        symptom: trimmed.to_string(),
                        source_location: None,
                        probable_causes: vec!["Unhandled runtime panic / exception".to_string()],
                        is_root: false,
                    });
                    prior_frame_id = Some(id);
                }
            } else if trimmed.starts_with("at ")
                || trimmed.starts_with("File ")
                || (trimmed.contains(".rs:") && trimmed.contains(':'))
            {
                frame_count += 1;
                let id = format!("frame_{}", frame_count);
                if node_ids.insert(id.clone()) {
                    nodes.push(CausalDiagnosticNode {
                        id: id.clone(),
                        symptom: format!("Call frame: {}", trimmed),
                        source_location: Some(trimmed.to_string()),
                        probable_causes: vec!["Upstream propagation path".to_string()],
                        is_root: false,
                    });

                    if let Some(ref prior) = prior_frame_id {
                        // Edge: current deeper frame caused prior failure
                        edges.push((id.clone(), prior.clone()));
                        non_root_ids.insert(prior.clone());
                    }
                    prior_frame_id = Some(id);
                }
            }
        }

        // 3. Identify root causes: nodes with no incoming causal edges
        let mut root_cause_ids = Vec::new();
        for node in &mut nodes {
            if !non_root_ids.contains(&node.id) {
                node.is_root = true;
                root_cause_ids.push(node.id.clone());
            }
        }

        CausalDiagnosticGraph {
            nodes,
            edges,
            identified_root_causes: root_cause_ids,
            recommended_investigation_sites: investigation_sites,
        }
    }

    /// Scans a git diff hunk or code edit to audit whether it applies superficial bandaging
    /// (e.g. empty catch blocks, swallowed errors, unwrap suppressions, dummy returns).
    pub fn audit_patch(diff_or_code: &str) -> PatchIntegrityReport {
        let mut bandages = Vec::new();

        for line in diff_or_code.lines() {
            let trimmed = line.trim();

            // Only examine added or modified lines in diffs, or all lines if raw code
            let inspect_line = if trimmed.starts_with('+') && !trimmed.starts_with("+++") {
                trimmed.strip_prefix('+').unwrap().trim()
            } else if trimmed.starts_with('-') {
                continue;
            } else {
                trimmed
            };

            // 1. Detect Empty Fallback Catches
            if (inspect_line.contains("catch") || inspect_line.contains("except"))
                && (inspect_line.contains("{}")
                    || inspect_line.contains("{ }")
                    || inspect_line.contains("pass")
                    || inspect_line.contains("{ /* ignore */ }"))
            {
                bandages.push(SuperficialPatchKind::EmptyFallbackCatch {
                    reason: "Swallows caught exception without logging, mitigation, or rethrowing."
                        .to_string(),
                });
            }

            // 2. Detect Unwrap Suppression without error handling
            if inspect_line.contains(".unwrap_or_default()")
                && (diff_or_code.contains(".expect(") || diff_or_code.contains(".unwrap()"))
            {
                bandages.push(SuperficialPatchKind::UnwrapSuppression {
                    location: inspect_line.to_string(),
                });
            }

            // 3. Detect Dummy Stub Returns in production code
            if (inspect_line == "return Ok(());"
                || inspect_line == "return true;"
                || inspect_line == "return \"\";")
                && diff_or_code.contains("TODO")
            {
                bandages.push(SuperficialPatchKind::StubReturn {
                    pattern: inspect_line.to_string(),
                });
            }

            // 4. Detect Swallowed Errors
            if inspect_line.contains("let _ =")
                && (inspect_line.contains(".send(")
                    || inspect_line.contains(".write(")
                    || inspect_line.contains(".save("))
            {
                bandages.push(SuperficialPatchKind::SwallowError {
                    details: inspect_line.to_string(),
                });
            }

            // 5. Detect Test Assertion Weakening
            if inspect_line.contains("// assert")
                || inspect_line.contains("//assert")
                || inspect_line == "assert!(true);"
            {
                bandages.push(SuperficialPatchKind::TestAssertionWeakening {
                    details: inspect_line.to_string(),
                });
            }
        }

        let passed = bandages.is_empty();
        let classification = if passed {
            PatchClassification::RootCauseFix
        } else {
            PatchClassification::SuperficialBandage
        };

        let explanation = if passed {
            "Patch passed root-cause integrity audit: no superficial error swallowing or unwrap masking found.".to_string()
        } else {
            format!(
                "Patch rejected by root-cause gate: detected {} superficial bandages that mask faults without resolving them.",
                bandages.len()
            )
        };

        PatchIntegrityReport {
            classification,
            detected_bandages: bandages,
            passed_root_cause_gate: passed,
            explanation,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::DiagnosticSeverity;
    use std::path::PathBuf;

    #[test]
    fn test_audit_patch_detects_empty_catch() {
        let diff = r#"
+ try {
+     db.execute();
+ } catch (err) {}
"#;
        let report = RootCauseAnalyzer::audit_patch(diff);
        assert_eq!(
            report.classification,
            PatchClassification::SuperficialBandage
        );
        assert!(!report.passed_root_cause_gate);
        assert_eq!(report.detected_bandages.len(), 1);
    }

    #[test]
    fn test_audit_patch_passes_valid_error_handling() {
        let diff = r#"
+ match db.execute() {
+     Ok(res) => Ok(res),
+     Err(err) => {
+         tracing::error!("Database execution failed: {:?}", err);
+         Err(AppError::Db(err))
+     }
+ }
"#;
        let report = RootCauseAnalyzer::audit_patch(diff);
        assert_eq!(report.classification, PatchClassification::RootCauseFix);
        assert!(report.passed_root_cause_gate);
        assert!(report.detected_bandages.is_empty());
    }

    #[test]
    fn test_build_causal_graph() {
        let diags = vec![Diagnostic {
            file_path: PathBuf::from("src/server.rs"),
            line: 42,
            column: Some(5),
            severity: DiagnosticSeverity::Error,
            code: Some("E0308".to_string()),
            message: "mismatched types".to_string(),
            snippet: None,
            suggested_fix: None,
        }];

        let trace = "panicked at 'called Option::unwrap() on a None value'\nat src/main.rs:12";
        let graph = RootCauseAnalyzer::build_causal_graph(&diags, trace);
        assert!(!graph.nodes.is_empty());
        assert!(!graph.identified_root_causes.is_empty());
    }
}

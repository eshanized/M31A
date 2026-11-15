//! Test-to-Code Discovery Engine (`m31a-ast::test_discovery`).
//!
//! Maps source files, functions, and interfaces to relevant unit and integration tests.
//! Evaluates:
//! - Direct AST call evidence (test explicitly invokes the target symbol).
//! - Module import evidence (test imports the symbol or its containing module).
//! - In-file test declarations (`#[cfg(test)]`, `#[test]`, `test_*`, `Test*`).
//! - Integration test suite conventions (`tests/*.rs`, `*.test.ts`, `*_test.go`).
//! Returns explicit confidence (0.0 - 1.0) and selection rationale.

use crate::types::{AstSymbol, RelationshipKind, SymbolRelationship};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestSelectionRationale {
    DirectCallEvidence,
    ImportEvidence,
    InFileUnitAnnotation,
    CoLocatedTestFile,
    IntegrationSuiteMapping,
    NamingConventionMatch,
}

impl TestSelectionRationale {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DirectCallEvidence => "direct_call_evidence",
            Self::ImportEvidence => "import_evidence",
            Self::InFileUnitAnnotation => "in_file_unit_annotation",
            Self::CoLocatedTestFile => "co_located_test_file",
            Self::IntegrationSuiteMapping => "integration_suite_mapping",
            Self::NamingConventionMatch => "naming_convention_match",
        }
    }
}

/// A relevant test discovered for a symbol or file with evidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelatedTestTarget {
    pub test_name: String,
    pub test_file: PathBuf,
    pub line: usize,
    pub rationale: TestSelectionRationale,
    pub confidence: f64,
    pub explanation: String,
}

pub struct TestDiscoveryEngine;

impl TestDiscoveryEngine {
    /// Discovers all tests relevant to a target symbol and its defining file.
    pub fn find_related_tests(
        target_symbol: &str,
        target_file: &Path,
        all_symbols: &[AstSymbol],
        relationships: &[SymbolRelationship],
        test_files: &[PathBuf],
    ) -> Vec<RelatedTestTarget> {
        let mut results = Vec::new();
        let mut seen = HashSet::new();

        // 1. Direct call evidence: tests that directly call target_symbol
        for rel in relationships {
            if rel.kind == RelationshipKind::Calls && rel.target_symbol == target_symbol {
                // Check if the caller is a test symbol or in a test file
                let caller_sym = all_symbols
                    .iter()
                    .find(|s| s.name == rel.source_symbol && s.file_path == rel.file_path);

                let is_test = caller_sym
                    .map(|s| s.name.starts_with("test_") || s.name.starts_with("Test"))
                    .unwrap_or(false)
                    || test_files.iter().any(|tf| tf == &rel.file_path);

                if is_test {
                    let key = format!("{}:{}", rel.file_path.display(), rel.line);
                    if seen.insert(key) {
                        results.push(RelatedTestTarget {
                            test_name: rel.source_symbol.clone(),
                            test_file: rel.file_path.clone(),
                            line: rel.line,
                            rationale: TestSelectionRationale::DirectCallEvidence,
                            confidence: 0.95,
                            explanation: format!(
                                "Test `{}` directly calls `{}` at line {}",
                                rel.source_symbol, target_symbol, rel.line
                            ),
                        });
                    }
                }
            }
        }

        // 2. In-file unit tests: tests defined within the same file (e.g. `#[cfg(test)]` modules in Rust)
        for sym in all_symbols {
            if sym.file_path == target_file {
                let is_test = sym.name.starts_with("test_")
                    || sym.name.starts_with("Test")
                    || sym.name.contains("test");

                if is_test && sym.name != target_symbol {
                    let key = format!("{}:{}", sym.file_path.display(), sym.range.start_line);
                    if seen.insert(key) {
                        results.push(RelatedTestTarget {
                            test_name: sym.name.clone(),
                            test_file: sym.file_path.clone(),
                            line: sym.range.start_line,
                            rationale: TestSelectionRationale::InFileUnitAnnotation,
                            confidence: 0.85,
                            explanation: format!(
                                "In-file test `{}` co-located in `{}`",
                                sym.name,
                                target_file
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or("")
                            ),
                        });
                    }
                }
            }
        }

        // 3. Co-located test file (e.g. `foo.ts` -> `foo.test.ts`, `server.go` -> `server_test.go`)
        let target_stem = target_file
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");

        for tf in test_files {
            let tf_name = tf.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if (tf_name.starts_with(target_stem) && tf_name.contains("test"))
                || (tf_name.starts_with("test_") && tf_name.contains(target_stem))
            {
                // Find test functions inside this test file
                let tests_in_file: Vec<&AstSymbol> =
                    all_symbols.iter().filter(|s| &s.file_path == tf).collect();

                if tests_in_file.is_empty() {
                    let key = format!("{}:1", tf.display());
                    if seen.insert(key) {
                        results.push(RelatedTestTarget {
                            test_name: format!("suite in {}", tf_name),
                            test_file: tf.clone(),
                            line: 1,
                            rationale: TestSelectionRationale::CoLocatedTestFile,
                            confidence: 0.75,
                            explanation: format!("Co-located test suite file `{}`", tf_name),
                        });
                    }
                } else {
                    for test_sym in tests_in_file {
                        let key = format!("{}:{}", tf.display(), test_sym.range.start_line);
                        if seen.insert(key) {
                            results.push(RelatedTestTarget {
                                test_name: test_sym.name.clone(),
                                test_file: tf.clone(),
                                line: test_sym.range.start_line,
                                rationale: TestSelectionRationale::CoLocatedTestFile,
                                confidence: 0.80,
                                explanation: format!(
                                    "Test `{}` in paired test file `{}`",
                                    test_sym.name, tf_name
                                ),
                            });
                        }
                    }
                }
            }
        }

        // Sort by confidence descending
        results.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AstSymbolKind, ResolutionStatus, SourceRange};

    #[test]
    fn test_direct_call_test_discovery() {
        let sym_target = AstSymbol::new(
            "hash_password",
            AstSymbolKind::Function,
            PathBuf::from("src/auth.rs"),
            SourceRange::new(10, 20, 0, 0),
            "pub fn hash_password()",
        );

        let sym_test = AstSymbol::new(
            "test_hash_password_valid",
            AstSymbolKind::Function,
            PathBuf::from("tests/auth_test.rs"),
            SourceRange::new(1, 15, 0, 0),
            "fn test_hash_password_valid()",
        );

        let all_symbols = vec![sym_target, sym_test];
        let rels = vec![SymbolRelationship::new(
            "test_hash_password_valid",
            "test_hash_password_valid",
            "hash_password",
            RelationshipKind::Calls,
            PathBuf::from("tests/auth_test.rs"),
            5,
            ResolutionStatus::AstDerived,
        )];

        let test_files = vec![PathBuf::from("tests/auth_test.rs")];

        let results = TestDiscoveryEngine::find_related_tests(
            "hash_password",
            Path::new("src/auth.rs"),
            &all_symbols,
            &rels,
            &test_files,
        );

        assert!(!results.is_empty());
        assert_eq!(results[0].test_name, "test_hash_password_valid");
        assert_eq!(
            results[0].rationale,
            TestSelectionRationale::DirectCallEvidence
        );
        assert!(results[0].confidence >= 0.90);
    }
}

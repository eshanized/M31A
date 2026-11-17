//! Local Repository Maintenance Signal Source (`m31a-maintenance::intake::local`).
//!
//! Analyzes local workspace state using repository intelligence, AST semantics,
//! dependency drift detection, and autonomous verification checks.

use anyhow::Result;
use chrono::Utc;
use std::fs;
use std::path::PathBuf;

use m31a_ast::architecture::ArchitecturalComplexityAnalyzer;
use m31a_ast::semver::ManifestDependencyAuditor;
use m31a_verify::AutonomousVerifier;

use crate::domain::{
    MaintenanceCategory, MaintenanceSignal, SignalProvider, SignalSeverity, SignalTrustStatus,
};

/// Local repository analysis signal generator.
#[derive(Debug, Clone)]
pub struct LocalRepositorySignalSource {
    workspace_root: PathBuf,
}

impl LocalRepositorySignalSource {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
        }
    }

    /// Performs comprehensive local repository maintenance scan.
    pub async fn scan(&self, repo_id: &str) -> Result<Vec<MaintenanceSignal>> {
        let mut signals = Vec::new();

        // 1. Check compiler / build health
        if let Ok(report) = AutonomousVerifier::verify_check(
            &self.workspace_root,
            std::time::Duration::from_secs(45),
        )
        .await
            && !report.success
            && !report.diagnostics.is_empty()
        {
            let diag_count = report.diagnostics.len();
            let top_diag = &report.diagnostics[0].message;
            let sig = MaintenanceSignal::new(
                repo_id,
                SignalProvider::LocalAnalysis,
                MaintenanceCategory::BrokenCi,
                None,
                None,
                Utc::now().timestamp_millis(),
                None,
                SignalSeverity::High,
                &format!("compiler_diagnostics:{}:{}", diag_count, top_diag),
                SignalTrustStatus::Internal,
                serde_json::json!({
                    "error_count": diag_count,
                    "primary_error": top_diag,
                    "self_healing_prompt": report.self_healing_prompt,
                }),
            );
            signals.push(sig);
        }

        // 2. Check dependency drift and manifest issues (Cargo.toml / package.json)
        let cargo_toml = self.workspace_root.join("Cargo.toml");
        if cargo_toml.exists()
            && cargo_toml.is_file()
            && let Ok(content) = fs::read_to_string(&cargo_toml)
            && let Ok(dep_audit) = ManifestDependencyAuditor::audit_cargo_toml(&content)
        {
            for issue in dep_audit.issues {
                let severity = match issue.severity {
                    m31a_ast::semver::DependencySeverity::Critical => {
                        SignalSeverity::Critical
                    }
                    m31a_ast::semver::DependencySeverity::Warning => SignalSeverity::Medium,
                    m31a_ast::semver::DependencySeverity::Info => SignalSeverity::Low,
                };

                let sig = MaintenanceSignal::new(
                    repo_id,
                    SignalProvider::LocalAnalysis,
                    MaintenanceCategory::DependencyUpdate,
                    None,
                    None,
                    Utc::now().timestamp_millis(),
                    None,
                    severity,
                    &format!("dep_issue:{}:{}", issue.package_name, issue.suggestion),
                    SignalTrustStatus::Internal,
                    serde_json::json!({
                        "package": issue.package_name,
                        "declared_version": issue.declared_version,
                        "rule": issue.rule,
                        "suggestion": issue.suggestion,
                        "affected_file": cargo_toml.to_string_lossy(),
                    }),
                );
                signals.push(sig);
            }
        }

        let pkg_json = self.workspace_root.join("package.json");
        if pkg_json.exists()
            && pkg_json.is_file()
            && let Ok(content) = fs::read_to_string(&pkg_json)
            && let Ok(dep_audit) = ManifestDependencyAuditor::audit_package_json(&content)
        {
            for issue in dep_audit.issues {
                let sig = MaintenanceSignal::new(
                    repo_id,
                    SignalProvider::LocalAnalysis,
                    MaintenanceCategory::DependencyUpdate,
                    None,
                    None,
                    Utc::now().timestamp_millis(),
                    None,
                    SignalSeverity::Medium,
                    &format!("dep_issue_pkg:{}:{}", issue.package_name, issue.suggestion),
                    SignalTrustStatus::Internal,
                    serde_json::json!({
                        "package": issue.package_name,
                        "declared_version": issue.declared_version,
                        "suggestion": issue.suggestion,
                    }),
                );
                signals.push(sig);
            }
        }

        // 3. Check architectural complexity on discovered top-level source files
        let src_dir = self.workspace_root.join("src");
        if src_dir.exists()
            && src_dir.is_dir()
            && let Ok(entries) = fs::read_dir(&src_dir)
        {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file()
                    && path.extension().and_then(|e| e.to_str()) == Some("rs")
                    && let Ok(content) = fs::read_to_string(&path)
                {
                    let smells = ArchitecturalComplexityAnalyzer::audit_source_complexity(
                        &path, &content,
                    );
                    if !smells.is_empty() {
                        let smell = &smells[0];
                        let sig = MaintenanceSignal::new(
                            repo_id,
                            SignalProvider::LocalAnalysis,
                            MaintenanceCategory::ArchitectureDrift,
                            None,
                            None,
                            Utc::now().timestamp_millis(),
                            None,
                            SignalSeverity::Medium,
                            &format!("arch_smell:{}:{}", path.display(), smell.description),
                            SignalTrustStatus::Internal,
                            serde_json::json!({
                                "file": path.to_string_lossy(),
                                "description": smell.description,
                                "smell_type": format!("{:?}", smell.smell_type),
                                "recommendation": smell.trade_off_recommendation,
                            }),
                        );
                        signals.push(sig);
                        break;
                    }
                }
            }
        }

        Ok(signals)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_local_scanner_instantiation() {
        let temp = tempdir().unwrap();
        let scanner = LocalRepositorySignalSource::new(temp.path());
        let signals = scanner.scan("test_repo").await.unwrap();
        assert!(signals.is_empty());
    }
}

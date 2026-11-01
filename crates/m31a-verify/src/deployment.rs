//! End-to-End Delivery & Production Deployment Readiness Gate (`m31a-verify::deployment`).
//!
//! Continues beyond implementation to ensure software is verified, containerized,
//! documented, and certified ready for production deployment.
//!
//! Evaluates 6 critical readiness gates:
//! 1. Build & Typecheck: Verifies clean compilation without fatal errors.
//! 2. Automated Test Verification: Verifies 100% of test suites pass.
//! 3. Test Integrity: Confirms test suites are not hollow stubs or shallow assertions.
//! 4. Infrastructure & Security: Validates Dockerfile / deployment configs.
//! 5. Documentation & Release Notes: Confirms changelog and release notes exist.
//! 6. Database & Schema Alignment: Confirms no pending un-migrated schemas.

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeploymentVerdict {
    ProductionReady,
    BlockedOnBuild,
    BlockedOnTests,
    BlockedOnSecurity,
    BlockedOnDocumentation,
    RequiresRemediation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateCheck {
    pub gate_name: String,
    pub passed: bool,
    pub details: String,
    pub blocking: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentReadinessReport {
    pub verdict: DeploymentVerdict,
    pub total_gates: usize,
    pub passed_gates: usize,
    pub checks: Vec<GateCheck>,
    pub release_version: Option<String>,
    pub certificate_markdown: String,
}

pub struct DeploymentReadinessGate;

impl DeploymentReadinessGate {
    /// Evaluates workspace readiness for production release.
    pub fn evaluate_readiness(
        _repo_root: &Path,
        build_passed: bool,
        tests_passed: bool,
        test_flaws_count: usize,
        iac_critical_count: usize,
        has_release_notes: bool,
        schema_drift_detected: bool,
    ) -> DeploymentReadinessReport {
        let mut checks = Vec::new();

        // 1. Build check
        checks.push(GateCheck {
            gate_name: "Compiler & Typecheck Verification".to_string(),
            passed: build_passed,
            details: if build_passed {
                "Build completed with 0 errors.".to_string()
            } else {
                "Build failed.".to_string()
            },
            blocking: true,
        });

        // 2. Test execution check
        checks.push(GateCheck {
            gate_name: "Automated Test Suite Execution".to_string(),
            passed: tests_passed,
            details: if tests_passed {
                "All unit and integration tests passed.".to_string()
            } else {
                "One or more tests failed.".to_string()
            },
            blocking: true,
        });

        // 3. Test integrity check
        let integrity_passed = test_flaws_count == 0;
        checks.push(GateCheck {
            gate_name: "Test Integrity & Assertion Soundness".to_string(),
            passed: integrity_passed,
            details: if integrity_passed {
                "Tests audited: 0 trivial or fake tests.".to_string()
            } else {
                format!("{} test integrity flaws detected.", test_flaws_count)
            },
            blocking: false,
        });

        // 4. Infrastructure & Security
        let security_passed = iac_critical_count == 0;
        checks.push(GateCheck {
            gate_name: "Container & Infrastructure Security".to_string(),
            passed: security_passed,
            details: if security_passed {
                "0 critical IaC / Dockerfile vulnerabilities.".to_string()
            } else {
                format!("{} critical IaC vulnerabilities found.", iac_critical_count)
            },
            blocking: true,
        });

        // 5. Documentation & Release Notes
        checks.push(GateCheck {
            gate_name: "Release Documentation & Changelog".to_string(),
            passed: has_release_notes,
            details: if has_release_notes {
                "Release notes / changelog verified.".to_string()
            } else {
                "Missing release notes or changelog entry.".to_string()
            },
            blocking: false,
        });

        // 6. Schema drift
        checks.push(GateCheck {
            gate_name: "Database Schema Synchronization".to_string(),
            passed: !schema_drift_detected,
            details: if !schema_drift_detected {
                "Data models synchronized with migrations.".to_string()
            } else {
                "Model modifications detected without corresponding migration scripts.".to_string()
            },
            blocking: true,
        });

        let total_gates = checks.len();
        let passed_gates = checks.iter().filter(|c| c.passed).count();

        let verdict = if !build_passed {
            DeploymentVerdict::BlockedOnBuild
        } else if !tests_passed {
            DeploymentVerdict::BlockedOnTests
        } else if !security_passed {
            DeploymentVerdict::BlockedOnSecurity
        } else if checks.iter().any(|c| c.blocking && !c.passed) {
            DeploymentVerdict::RequiresRemediation
        } else {
            DeploymentVerdict::ProductionReady
        };

        let mut md = String::new();
        md.push_str("# Production Release Readiness Certificate\n\n");
        md.push_str(&format!("**Verdict:** `{:?}`  \n", verdict));
        md.push_str(&format!(
            "**Gates Passed:** {} / {}\n\n",
            passed_gates, total_gates
        ));
        md.push_str("| Gate | Status | Details |\n");
        md.push_str("| :--- | :---: | :--- |\n");
        for c in &checks {
            md.push_str(&format!(
                "| {} | {} | {} |\n",
                c.gate_name,
                if c.passed { "PASSED" } else { "FAILED" },
                c.details
            ));
        }

        DeploymentReadinessReport {
            verdict,
            total_gates,
            passed_gates,
            checks,
            release_version: Some("v0.1.13".to_string()),
            certificate_markdown: md,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deployment_ready_when_all_pass() {
        let report = DeploymentReadinessGate::evaluate_readiness(
            Path::new("."),
            true,  // build
            true,  // tests
            0,     // test flaws
            0,     // security issues
            true,  // release notes
            false, // no schema drift
        );
        assert_eq!(report.verdict, DeploymentVerdict::ProductionReady);
        assert_eq!(report.passed_gates, report.total_gates);
    }

    #[test]
    fn test_deployment_blocked_on_tests() {
        let report = DeploymentReadinessGate::evaluate_readiness(
            Path::new("."),
            true,  // build
            false, // tests failed
            0,
            0,
            true,
            false,
        );
        assert_eq!(report.verdict, DeploymentVerdict::BlockedOnTests);
    }

    #[test]
    fn test_deployment_blocked_on_security() {
        let report = DeploymentReadinessGate::evaluate_readiness(
            Path::new("."),
            true,
            true,
            0,
            2, // 2 critical security issues
            true,
            false,
        );
        assert_eq!(report.verdict, DeploymentVerdict::BlockedOnSecurity);
    }
}

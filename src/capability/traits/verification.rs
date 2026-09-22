//! Verification and QA service trait (CTL-01, CTL-02).

use crate::capability::error::CapabilityError;
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Classification of verification check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VerificationKind {
    Test,
    Lint,
    Format,
    Custom(String),
}

/// Verification command or suite specification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct VerificationTarget {
    pub name: String,
    pub kind: VerificationKind,
    pub args: Vec<String>,
}

/// Output report from running verification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct VerificationReport {
    pub passed: bool,
    pub exit_code: i32,
    pub output: String,
    pub duration_ms: u64,
}

/// Asynchronous service seam for running tests, linter, and formatters.
#[async_trait]
pub trait VerificationService: Send + Sync + 'static {
    /// Execute a verification target and return comprehensive report.
    async fn run_verification(
        &self,
        target: &VerificationTarget,
    ) -> Result<VerificationReport, CapabilityError>;
}
